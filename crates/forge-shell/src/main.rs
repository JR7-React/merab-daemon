use std::io::{self, BufRead, Write};
use tokio::process::Command;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    result: Option<Value>,
    error: Option<Value>,
    id: Option<Value>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure logging to stderr
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting Forge Shell Agent...");

    // We can't block on stdin with tokio easily in a mixed sync/async way if we want full async reading.
    // However, since we are line-based JSON-RPC over stdio, reading stdin synchronously line-by-line 
    // and spawning async tasks for execution is a reasonable pattern for this simple agent.
    // Or we could use tokio::io::stdin, but that requires more framing logic.
    // Let's stick to the synchronous read loop spawning async handlers, but we need to be careful about strict ordering if required.
    // For MCP, requests can be handled in parallel if ID-based, but usually strict sequential is safer for simple implementations.
    // Let's use a simple loop that awaits execution for now.

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        tracing::debug!("Received: {}", line);

        match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(req) => handle_request(req).await,
            Err(e) => {
                tracing::error!("Failed to parse request: {}", e);
            }
        }
    }

    Ok(())
}

async fn handle_request(req: JsonRpcRequest) {
    let response = match req.method.as_str() {
        "initialize" => handle_initialize(&req),
        "notifications/initialized" => None,
        "tools/list" => Some(handle_list_tools(&req)),
        "tools/call" => Some(handle_call_tool(&req).await),
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
            "name": "forge-shell",
            "version": "0.1.0"
        }
    });
    Some(json_success(req.id.clone(), result))
}

fn handle_list_tools(req: &JsonRpcRequest) -> JsonRpcResponse {
    let result = json!({
        "tools": [
            {
                "name": "shell.execute",
                "description": "Execute a system command",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "Command to execute (e.g. 'cargo', 'git')" },
                        "args": { 
                            "type": "array", 
                            "items": { "type": "string" },
                            "description": "Arguments for the command"
                        },
                        "cwd": { "type": "string", "description": "Current working directory (optional)" }
                    },
                    "required": ["command"]
                }
            }
        ]
    });
    Some(json_success(req.id.clone(), result)).unwrap()
}

async fn handle_call_tool(req: &JsonRpcRequest) -> JsonRpcResponse {
    let params = req.params.as_ref().and_then(|p| p.as_object());
    let name = params.and_then(|p| p.get("name")).and_then(|n| n.as_str());
    let args = params.and_then(|p| p.get("arguments"));

    let result = match name {
        Some("shell.execute") => tool_execute(args).await,
        _ => return json_error(req.id.clone(), -32601, "Tool not found"),
    };

    match result {
        Ok(val) => json_success(req.id.clone(), json!({ "content": [{ "type": "text", "text": val }], "isError": false })),
        Err(e) => json_success(req.id.clone(), json!({ "content": [{ "type": "text", "text": format!("Error: {}", e) }], "isError": true })),
    }
}

async fn tool_execute(args: Option<&Value>) -> anyhow::Result<String> {
    let args = args.ok_or(anyhow::anyhow!("Missing arguments"))?;
    let command_str = args.get("command").and_then(|v| v.as_str()).ok_or(anyhow::anyhow!("Missing command"))?;
    
    let cmd_args: Vec<String> = match args.get("args") {
        Some(Value::Array(arr)) => arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect(),
        _ => Vec::new(),
    };

    let cwd = args.get("cwd").and_then(|v| v.as_str());

    tracing::info!("Executing: {} {:?} (cwd: {:?})", command_str, cmd_args, cwd);

    // If args are empty and command contains spaces, run via system shell
    // so the LLM can send full command strings like "cargo build --release"
    let output = if cmd_args.is_empty() && command_str.contains(' ') {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command_str]);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.output().await?
    } else {
        let mut cmd = Command::new(command_str);
        cmd.args(&cmd_args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.output().await?
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code().unwrap_or(-1);

    let result_str = format!(
        "Exit Code: {}\nSTDOUT:\n{}\nSTDERR:\n{}", 
        exit_code, stdout, stderr
    );

    Ok(result_str)
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
