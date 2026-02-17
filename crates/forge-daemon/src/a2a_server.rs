use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use forge_core::{Task, TaskStatus};
use forge_store::Database;
use forge_transport::a2a::{AgentCard, AgentSkill, TaskRequest, TaskResponse};
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
    let mut skills = Vec::new();
    let mcp_manager = &ctx.mcp_manager;

    // Get all running MCP clients
    // Assuming list_tools is async and we need to iterate
    // This part is tricky because McpManager stores clients in Mutex<HashMap>
    // We need to iterate over them and call list_tools on each.
    
    // For now, let's just list registered agents from DB or assume running ones?
    // The prompt says "listing all MCP agents as skills".
    // This implies we need to query the agents and their capabilities.
    // If an agent is running, we can ask for its tools.
    // If not running, we might not know its tools unless we cached them.
    // For MVP, let's just use running agents.
    
    // We need access to running clients.
    // McpManager doesn't expose a way to iterate easily.
    // I might need to add a method to McpManager to get all tools.
    // But for now let's assume we can get client IDs and then query.
    
    // Placeholder implementation:
    let card = AgentCard {
        name: "Forge Runtime".to_string(),
        description: "A local agent runtime hosting multiple MCP agents".to_string(),
        url: "http://localhost:8080".to_string(), // Should be configurable
        skills,
    };

    let json = serde_json::to_string_pretty(&card).unwrap_or_default();
    Ok(Response::new(Full::new(Bytes::from(json))))
}

#[derive(serde::Deserialize)]
struct RpcRequest {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

async fn handle_rpc(
    req: Request<hyper::body::Incoming>,
    ctx: Arc<A2AContext>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let body_bytes = req.into_body().collect().await.unwrap().to_bytes(); // Should handle error
    let rpc_req: RpcRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(_) => return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::new(Bytes::from("Invalid JSON")))
            .unwrap()),
    };

    let result = match rpc_req.method.as_str() {
        "tasks/send" => handle_task_send(rpc_req.params, ctx).await,
        "tasks/get" => handle_task_get(rpc_req.params, ctx).await,
        // "tasks/cancel" => handle_task_cancel(rpc_req.params, ctx).await,
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

    Ok(Response::new(Full::new(Bytes::from(response_body.to_string()))))
}

async fn handle_task_send(params: Option<Value>, ctx: Arc<A2AContext>) -> Result<Value, String> {
    // 1. Parse params
    // 2. Create task in DB
    // 3. Find tool
    // 4. Execute tool
    // 5. Update task
    
    // For MVP, simplistic parsing
    // Assuming params is an object matching TaskRequest fields or similar?
    // A2A spec for tasks/send usually takes 'task' object.
    
    // Let's assume params IS the TaskRequest for now to match our struct
    let req: TaskRequest = serde_json::from_value(params.unwrap_or(Value::Null))
        .map_err(|e| format!("Invalid params: {}", e))?;

    let task_id = Uuid::new_v4();
    let task = Task {
        id: task_id,
        source: None, // Could extract from headers
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
    }).unwrap())
}

async fn handle_task_get(params: Option<Value>, ctx: Arc<A2AContext>) -> Result<Value, String> {
    // Expect params to contain 'task_id'
    let id_val = params.and_then(|p| p.get("task_id").cloned());
    let id_str = id_val.and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or("Missing task_id")?;
        
    let db = ctx.db.lock().await;
    let task = db.get_task(&id_str).map_err(|e| "Task not found".to_string())?;
    
    Ok(serde_json::json!({
        "task_id": task.id.to_string(),
        "status": task.status.to_string(),
        "output": task.output.and_then(|s| serde_json::from_str::<Value>(&s).ok()),
        "error": task.error
    }))
}

async fn execute_task(ctx: Arc<A2AContext>, task_id: Uuid, skill: String, input: Value) {
    // 1. Find the agent that has this skill (tool)
    // We need to iterate all clients to find one with this tool name.
    // This is inefficient but okay for MVP.
    
    // TODO: Implement finding the right agent.
    // For now, let's assume we find it or fail.
    
    // Update status to running
    {
        let db = ctx.db.lock().await;
        let _ = db.update_task_status(&task_id.to_string(), TaskStatus::Running, None, None);
    }
    
    // ... search logic ...
    // Since we don't have easy lookup, we might need to change McpManager
    // or iterate. Let's assume we fail for now until we add lookup.
    
    // Mock execution for compilation check
    // In real impl, we'd use ctx.mcp_manager.find_client_with_tool(&skill)
    
    let result = Err("Tool execution not yet fully wired".to_string());
    
    let (status, output, error) = match result {
        Ok(val) => (TaskStatus::Completed, Some(val), None),
        Err(e) => (TaskStatus::Failed, None, Some(e)),
    };
    
    let db = ctx.db.lock().await;
    let _ = db.update_task_status(
        &task_id.to_string(),
        status,
        output,
        error
    );
}
