use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    result: Option<Value>,
    error: Option<Value>,
    id: Option<Value>,
}

struct HttpAgent {
    client: reqwest::Client,
}

impl HttpAgent {
    fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    async fn fetch(
        &self,
        url: &str,
        headers: Option<Value>,
        timeout_secs: u64,
        max_bytes: usize,
    ) -> anyhow::Result<String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()?;

        let mut builder = client.get(url);

        if let Some(Value::Object(h)) = headers {
            for (k, v) in h {
                if let Some(v_str) = v.as_str() {
                    builder = builder.header(k, v_str);
                }
            }
        }

        let resp = builder.send().await?;
        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "HTTP {}: {}",
                status,
                &text[..200.min(text.len())]
            ));
        }

        if text.len() > max_bytes {
            Ok(format!(
                "{}\n\n[Truncated to {} bytes of {}]",
                &text[..max_bytes],
                max_bytes,
                text.len()
            ))
        } else {
            Ok(text)
        }
    }

    async fn post(
        &self,
        url: &str,
        body: Value,
        headers: Option<Value>,
        timeout_secs: u64,
    ) -> anyhow::Result<String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()?;

        let mut builder = client.post(url).json(&body);

        if let Some(Value::Object(h)) = headers {
            for (k, v) in h {
                if let Some(v_str) = v.as_str() {
                    builder = builder.header(k, v_str);
                }
            }
        }

        let resp = builder.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow::anyhow!("HTTP {}: {}", status, text));
        }
        Ok(text)
    }

    async fn download(&self, url: &str, dest: &str) -> anyhow::Result<String> {
        let bytes = self.client.get(url).send().await?.bytes().await?;
        std::fs::write(dest, &bytes)?;
        Ok(format!("Downloaded {} bytes to {}", bytes.len(), dest))
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting Merab HTTP Agent...");

    let agent = HttpAgent::new();
    let stdin = io::stdin();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            if line.trim().is_empty() {
                continue;
            }

            tracing::debug!("Received: {}", line);

            match serde_json::from_str::<JsonRpcRequest>(&line) {
                Ok(req) => handle_request(req, &agent).await,
                Err(e) => {
                    tracing::error!("Failed to parse request: {}", e);
                }
            }
        }
    });

    Ok(())
}

async fn handle_request(req: JsonRpcRequest, agent: &HttpAgent) {
    let response = match req.method.as_str() {
        "initialize" => handle_initialize(&req),
        "notifications/initialized" => None,
        "tools/list" => Some(handle_list_tools(&req)),
        "tools/call" => Some(handle_call_tool(&req, agent).await),
        "ping" => Some(json_success(req.id.clone(), json!("pong"))),
        _ => {
            tracing::warn!("Unknown method: {}", req.method);
            Some(json_error(req.id.clone(), -32601, "Method not found"))
        }
    };

    if let Some(resp) = response {
        let resp_str = serde_json::to_string(&resp).unwrap();
        println!("{}", resp_str);
        let _ = io::stdout().flush();
        tracing::debug!("Sent: {}", resp_str);
    }
}

fn handle_initialize(req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    let result = json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "merab-http",
            "version": "0.1.0"
        }
    });
    Some(json_success(req.id.clone(), result))
}

fn handle_list_tools(req: &JsonRpcRequest) -> JsonRpcResponse {
    let result = json!({
        "tools": [
            {
                "name": "http.fetch",
                "description": "Fetch content from a URL via HTTP GET. Returns text content.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "The URL to fetch (required)" },
                        "headers": { "type": "object", "description": "Optional HTTP headers as key-value pairs" },
                        "timeout_secs": { "type": "integer", "description": "Timeout in seconds (default: 30)" },
                        "max_bytes": { "type": "integer", "description": "Max response bytes to return (default: 65536 = 64KB)" }
                    },
                    "required": ["url"]
                }
            },
            {
                "name": "http.post",
                "description": "Send a JSON POST request to a URL.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "Target URL (required)" },
                        "body": { "type": "object", "description": "JSON body to send (required)" },
                        "headers": { "type": "object", "description": "Optional HTTP headers" },
                        "timeout_secs": { "type": "integer", "description": "Timeout in seconds (default: 30)" }
                    },
                    "required": ["url", "body"]
                }
            },
            {
                "name": "http.download",
                "description": "Download a file from a URL and save it to disk.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "URL to download (required)" },
                        "dest": { "type": "string", "description": "Destination file path (required)" }
                    },
                    "required": ["url", "dest"]
                }
            }
        ]
    });
    json_success(req.id.clone(), result)
}

async fn handle_call_tool(req: &JsonRpcRequest, agent: &HttpAgent) -> JsonRpcResponse {
    let params = req.params.as_ref().and_then(|p| p.as_object());
    let name = params.and_then(|p| p.get("name")).and_then(|n| n.as_str());
    let args = params.and_then(|p| p.get("arguments"));

    let result = match name {
        Some("http.fetch") => tool_fetch(args, agent).await,
        Some("http.post") => tool_post(args, agent).await,
        Some("http.download") => tool_download(args, agent).await,
        _ => return json_error(req.id.clone(), -32601, "Tool not found"),
    };

    match result {
        Ok(val) => json_success(
            req.id.clone(),
            json!({ "content": [{ "type": "text", "text": val }], "isError": false }),
        ),
        Err(e) => json_success(
            req.id.clone(),
            json!({ "content": [{ "type": "text", "text": format!("Error: {}", e) }], "isError": true }),
        ),
    }
}

async fn tool_fetch(args: Option<&Value>, agent: &HttpAgent) -> anyhow::Result<String> {
    let args = args.ok_or(anyhow::anyhow!("Missing arguments"))?;
    let url = args
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or(anyhow::anyhow!("Missing url"))?;
    let headers = args.get("headers").cloned();
    let timeout_secs = args
        .get("timeout_secs")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);
    let max_bytes = args
        .get("max_bytes")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(65536);

    agent.fetch(url, headers, timeout_secs, max_bytes).await
}

async fn tool_post(args: Option<&Value>, agent: &HttpAgent) -> anyhow::Result<String> {
    let args = args.ok_or(anyhow::anyhow!("Missing arguments"))?;
    let url = args
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or(anyhow::anyhow!("Missing url"))?;
    let body = args
        .get("body")
        .cloned()
        .ok_or(anyhow::anyhow!("Missing body"))?;
    let headers = args.get("headers").cloned();
    let timeout_secs = args
        .get("timeout_secs")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    agent.post(url, body, headers, timeout_secs).await
}

async fn tool_download(args: Option<&Value>, agent: &HttpAgent) -> anyhow::Result<String> {
    let args = args.ok_or(anyhow::anyhow!("Missing arguments"))?;
    let url = args
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or(anyhow::anyhow!("Missing url"))?;
    let dest = args
        .get("dest")
        .and_then(|v| v.as_str())
        .ok_or(anyhow::anyhow!("Missing dest"))?;

    agent.download(url, dest).await
}

fn json_success(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(result),
        error: None,
        id,
    }
}

fn json_error(id: Option<Value>, code: i32, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(json!({
            "code": code,
            "message": message
        })),
        id,
    }
}
