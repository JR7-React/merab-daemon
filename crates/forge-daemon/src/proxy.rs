use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use forge_store::{CacheEntry, Database};
use http_body_util::{BodyExt, Full};
use hyper::header::{AUTHORIZATION, CONTENT_TYPE};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use reqwest::Client as HttpClient;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

pub struct ProxyContext {
    pub db: Arc<Mutex<Database>>,
    pub upstream_url: String,
    pub api_key: Option<String>,
    pub http_client: HttpClient,
}

pub async fn start_proxy_server(addr: SocketAddr, context: Arc<ProxyContext>) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("LLM Proxy listening on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let ctx = context.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(move |req| handle_request(req, ctx.clone())))
                .await
            {
                tracing::error!("Proxy error: {:?}", err);
            }
        });
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    ctx: Arc<ProxyContext>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/v1/chat/completions") => handle_chat_completion(req, ctx).await,
        _ => {
            // Forward everything else directly (pass-through)
            // For MVP we only care about chat completions. 
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from("Not Found")))
                .unwrap())
        }
    }
}

async fn handle_chat_completion(
    req: Request<hyper::body::Incoming>,
    ctx: Arc<ProxyContext>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    // 1. Read body
    let body_bytes = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => return Ok(Response::builder().status(StatusCode::BAD_REQUEST).body(Full::new(Bytes::from("Body Error"))).unwrap()),
    };

    let body_str = match String::from_utf8(body_bytes.to_vec()) {
        Ok(s) => s,
        Err(_) => return Ok(Response::builder().status(StatusCode::BAD_REQUEST).body(Full::new(Bytes::from("Invalid UTF-8"))).unwrap()),
    };

    // 2. Parse JSON to normalize
    let json_body: Value = match serde_json::from_str(&body_str) {
        Ok(v) => v,
        Err(_) => return Ok(Response::builder().status(StatusCode::BAD_REQUEST).body(Full::new(Bytes::from("Invalid JSON"))).unwrap()),
    };

    // Check if streaming is enabled - if so, bypass cache for now
    if let Some(stream) = json_body.get("stream").and_then(|v| v.as_bool()) {
        if stream {
            return forward_request(ctx, body_bytes, true).await;
        }
    }

    // 3. Calculate Hash (SHA-256 of normalized JSON)
    // serde_json::to_string sorts keys if we use a specific formatter, but default to_string is usually deterministic enough for simple objects.
    // Ideally we should use canonical JSON.
    let normalized = serde_json::to_string(&json_body).unwrap_or(body_str.clone());
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let hash = hex::encode(hasher.finalize());

    // 4. Check Cache
    {
        let db = ctx.db.lock().await;
        if let Ok(Some(entry)) = db.get_cache(&hash) {
            tracing::info!(hash = %hash, "Cache HIT");
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(entry.response)))
                .unwrap());
        }
    }
    
    tracing::info!(hash = %hash, "Cache MISS");

    // 5. Forward to Upstream
    let upstream_res = match forward_request(ctx.clone(), body_bytes.clone(), false).await {
        Ok(res) => res,
        Err(_) => return Ok(Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Full::new(Bytes::from("Upstream Error"))).unwrap()),
    };
    
    // If successful, store in cache
    if upstream_res.status() == StatusCode::OK {
        // We need to clone the body to store it, but hyper Response body is a stream.
        // forward_request returns a full response here because we waited for it (stream=false logic in forward_request).
        // Wait, forward_request returns Result<Response<Full<Bytes>>, Infallible>.
        
        let (parts, body) = upstream_res.into_parts();
        let body_bytes = body.collect().await.unwrap().to_bytes(); // Should be safe since it's Full<Bytes>
        
        let response_str = String::from_utf8_lossy(&body_bytes).to_string();
        
        // Extract model/provider if possible
        let model = json_body.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        
        let entry = CacheEntry {
            hash: hash.clone(),
            request: normalized,
            response: response_str.clone(),
            model,
            provider: Some("openai".to_string()), // Simplified
        };
        
        {
            let db = ctx.db.lock().await;
            let _ = db.store_cache(&entry);
        }
        
        Ok(Response::from_parts(parts, Full::new(Bytes::from(body_bytes))))
    } else {
        Ok(upstream_res)
    }
}

async fn forward_request(
    ctx: Arc<ProxyContext>,
    body: Bytes,
    _is_stream: bool,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let client = &ctx.http_client;

    let url = format!("{}/chat/completions", ctx.upstream_url.trim_end_matches('/'));
    
    let mut req_builder = client.post(url)
        .header(CONTENT_TYPE, "application/json")
        .body(body);

    if let Some(key) = &ctx.api_key {
        req_builder = req_builder.header(AUTHORIZATION, format!("Bearer {}", key));
    }

    match req_builder.send().await {
        Ok(res) => {
            let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let bytes = res.bytes().await.unwrap_or_default();
            Ok(Response::builder()
                .status(status)
                .header(CONTENT_TYPE, "application/json")
                .body(Full::new(bytes))
                .unwrap())
        },
        Err(e) => {
            tracing::error!("Upstream request failed: {}", e);
             Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from(format!("Upstream Error: {}", e))))
                .unwrap())
        }
    }
}
