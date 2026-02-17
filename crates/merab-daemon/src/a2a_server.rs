use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use merab_core::{Task, TaskStatus};
use merab_store::Database;
use merab_transport::a2a::{AgentCard, AgentSkill, TaskRequest, TaskResponse};
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mcp_manager::McpManager;

pub struct A2AContext {
    pub db: Arc<Mutex<Database>>,
    pub mcp_manager: Arc<McpManager>,
    pub a2a_port: u16,
}

pub async fn start_a2a_server(addr: SocketAddr, context: Arc<A2AContext>) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("A2A Server listening on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let ctx = context.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(move |req| handle_request(req, ctx.clone())))
                .await
            {
                tracing::error!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    ctx: Arc<A2AContext>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/.well-known/agent.json") => handle_agent_card(ctx).await,
        (&Method::POST, "/") => handle_rpc(req, ctx).await,
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("Not Found")))
            .unwrap()),
    }
}

async fn handle_agent_card(ctx: Arc<A2AContext>) -> Result<Response<Full<Bytes>>, Infallible> {
    // Optimized: get tools from index (O(1) access to memory map)
    let tools = ctx.mcp_manager.get_all_tools().await;

    let skills = tools
        .into_iter()
        .map(|t| AgentSkill {
            id: t.name.clone(),
            name: t.name,
            description: t.description,
            input_modes: vec!["text".to_string()],
            output_modes: vec!["text".to_string()],
        })
        .collect();

    let card = AgentCard {
        name: "Forge Runtime".to_string(),
        description: "A local agent runtime hosting multiple MCP agents".to_string(),
        url: format!("http://localhost:{}", ctx.a2a_port),
        skills,
    };

    let json = serde_json::to_string_pretty(&card).unwrap_or_default();
    Ok(Response::builder()
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(json)))
        .unwrap())
}

#[derive(serde::Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

async fn handle_rpc(
    req: Request<hyper::body::Incoming>,
    ctx: Arc<A2AContext>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let body_bytes = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from("Body Error")))
                .unwrap());
        }
    };

    let rpc_req: RpcRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(_) => {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from("Invalid JSON")))
                .unwrap());
        }
    };

    let result = match rpc_req.method.as_str() {
        "tasks/send" => handle_task_send(rpc_req.params, ctx).await,
        "tasks/get" => handle_task_get(rpc_req.params, ctx).await,
        _ => Err("Method not found".to_string()),
    };

    let response_body = match result {
        Ok(val) => serde_json::json!({
            "jsonrpc": "2.0",
            "result": val,
            "id": rpc_req.id
        }),
        Err(e) => serde_json::json!({
            "jsonrpc": "2.0",
            "error": { "code": -32601, "message": e },
            "id": rpc_req.id
        }),
    };

    Ok(Response::builder()
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(response_body.to_string())))
        .unwrap())
}

async fn handle_task_send(params: Option<Value>, ctx: Arc<A2AContext>) -> Result<Value, String> {
    let req: TaskRequest = serde_json::from_value(params.unwrap_or(Value::Null))
        .map_err(|e| format!("Invalid params: {}", e))?;

    let task_id = Uuid::new_v4();
    let task = Task {
        id: task_id,
        source: None,
        target: req.skill.clone(),
        input: req.input.to_string(),
        status: TaskStatus::Pending,
        output: None,
        error: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    {
        let db = ctx.db.lock().await;
        db.create_task(&task).map_err(|e| e.to_string())?;
    }

    // Spawn execution
    let ctx_clone = ctx.clone();
    let task_id_clone = task_id;
    let skill_name = req.skill.clone();
    let input_val = req.input.clone();

    tokio::spawn(async move {
        execute_task(ctx_clone, task_id_clone, skill_name, input_val).await;
    });

    Ok(serde_json::to_value(TaskResponse {
        task_id: task_id.to_string(),
        status: "pending".to_string(),
    })
    .unwrap())
}

async fn handle_task_get(params: Option<Value>, ctx: Arc<A2AContext>) -> Result<Value, String> {
    let id_val = params.and_then(|p| p.get("task_id").cloned());
    let id_str = id_val
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or("Missing task_id")?;

    let db = ctx.db.lock().await;
    let task = db
        .get_task(&id_str)
        .map_err(|_e| "Task not found".to_string())?;

    Ok(serde_json::json!({
        "task_id": task.id.to_string(),
        "status": task.status.to_string(),
        "output": task.output.and_then(|s| serde_json::from_str::<Value>(&s).ok()),
        "error": task.error
    }))
}

async fn execute_task(ctx: Arc<A2AContext>, task_id: Uuid, skill: String, input: Value) {
    // Update status to running
    {
        let db = ctx.db.lock().await;
        let _ = db.update_task_status(&task_id.to_string(), TaskStatus::Running, None, None);
    }

    // Optimized: Find agent directly from index (O(1))
    let target_client = ctx.mcp_manager.find_agent_for_tool(&skill).await;

    let result = if let Some(client) = target_client {
        let args = match input {
            Value::Object(map) => Some(map),
            Value::Null => None,
            _ => None, // Should probably improve error handling for non-object args
        };

        match client.call_tool(skill, args).await {
            Ok(res) => match serde_json::to_string(&res.content) {
                Ok(s) => Ok(s),
                Err(e) => Err(format!("Serialization error: {}", e)),
            },
            Err(e) => Err(format!("Tool execution failed: {}", e)),
        }
    } else {
        Err(format!("No agent found with skill: {}", skill))
    };

    let (status, output, error) = match result {
        Ok(val) => (TaskStatus::Completed, Some(val), None),
        Err(e) => (TaskStatus::Failed, None, Some(e)),
    };

    let db = ctx.db.lock().await;
    let _ = db.update_task_status(&task_id.to_string(), status, output, error);
}
