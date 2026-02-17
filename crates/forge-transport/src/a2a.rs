use anyhow::{Result, anyhow};
use http_body_util::{BodyExt, Empty, Full};
use hyper::Request;
use hyper::body::Bytes;
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpStream;
use url::Url;

/// A2A Agent Card — describes an agent's capabilities for discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub url: String,
    #[serde(default)]
    pub skills: Vec<AgentSkill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub input_modes: Vec<String>,
    #[serde(default)]
    pub output_modes: Vec<String>,
}

/// A request to submit a task to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    pub skill: String,
    pub input: serde_json::Value,
}

/// A response containing the task ID and initial status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResponse {
    pub task_id: String,
    pub status: String,
}

/// Full task details for status polling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetails {
    pub id: String,
    pub status: String,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
}

pub struct A2aClient;

impl A2aClient {
    pub async fn fetch_card(url_str: &str) -> Result<AgentCard> {
        let url = Url::parse(url_str)?;
        let host = url.host_str().ok_or_else(|| anyhow!("Missing host"))?;
        let port = url.port_or_known_default().unwrap_or(80);

        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(addr).await?;
        let io = TokioIo::new(stream);
        let (mut sender, conn) = http1::handshake(io).await?;

        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                tracing::error!("Connection failed: {:?}", err);
            }
        });

        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "{}/.well-known/agent.json",
                url_str.trim_end_matches('/')
            ))
            .header("Host", host)
            .body(Empty::<Bytes>::new())?;

        let res = sender.send_request(req).await?;
        let body_bytes = res.into_body().collect().await?.to_bytes();
        let card: AgentCard = serde_json::from_slice(&body_bytes)?;

        Ok(card)
    }

    pub async fn send_task(url_str: &str, skill: &str, input: Value) -> Result<TaskResponse> {
        let url = Url::parse(url_str)?;
        let host = url.host_str().ok_or_else(|| anyhow!("Missing host"))?;
        let port = url.port_or_known_default().unwrap_or(80);

        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(addr).await?;
        let io = TokioIo::new(stream);
        let (mut sender, conn) = http1::handshake(io).await?;

        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                tracing::error!("Connection failed: {:?}", err);
            }
        });

        let rpc_req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tasks/send",
            "params": {
                "skill": skill,
                "input": input
            },
            "id": 1
        });

        let req = Request::builder()
            .method("POST")
            .uri(url_str)
            .header("Host", host)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(rpc_req.to_string())))?;

        let res = sender.send_request(req).await?;
        let body_bytes = res.into_body().collect().await?.to_bytes();

        // Parse JSON-RPC response
        #[derive(Deserialize)]
        struct RpcResponse {
            result: Option<TaskResponse>,
            error: Option<Value>,
        }

        let rpc_res: RpcResponse = serde_json::from_slice(&body_bytes)?;

        if let Some(err) = rpc_res.error {
            return Err(anyhow!("RPC Error: {:?}", err));
        }

        rpc_res
            .result
            .ok_or_else(|| anyhow!("No result in RPC response"))
    }

    pub async fn get_task_status(url_str: &str, task_id: &str) -> Result<TaskDetails> {
        let url = Url::parse(url_str)?;
        let host = url.host_str().ok_or_else(|| anyhow!("Missing host"))?;
        let port = url.port_or_known_default().unwrap_or(80);

        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(addr).await?;
        let io = TokioIo::new(stream);
        let (mut sender, conn) = http1::handshake(io).await?;

        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                tracing::error!("Connection failed: {:?}", err);
            }
        });

        let rpc_req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tasks/get",
            "params": {
                "task_id": task_id
            },
            "id": 1
        });

        let req = Request::builder()
            .method("POST")
            .uri(url_str)
            .header("Host", host)
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(rpc_req.to_string())))?;

        let res = sender.send_request(req).await?;
        let body_bytes = res.into_body().collect().await?.to_bytes();

        #[derive(Deserialize)]
        struct RpcResponse {
            result: Option<TaskDetails>,
            error: Option<Value>,
        }

        let rpc_res: RpcResponse = serde_json::from_slice(&body_bytes)?;

        if let Some(err) = rpc_res.error {
            return Err(anyhow!("RPC Error: {:?}", err));
        }

        rpc_res
            .result
            .ok_or_else(|| anyhow!("No result in RPC response"))
    }
}
