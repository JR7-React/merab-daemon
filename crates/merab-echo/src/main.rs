use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

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

fn main() -> anyhow::Result<()> {
    // Configure logging to stderr (because stdout is for MCP)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting Merab Echo Agent...");

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        tracing::debug!("Received: {}", line);

        match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(req) => handle_request(req),
            Err(e) => {
                tracing::error!("Failed to parse request: {}", e);
                // In a real server we might send a ParseError back,
                // but for simplicity we just log it.
            }
        }
    }

    Ok(())
}

fn handle_request(req: JsonRpcRequest) {
    let response = match req.method.as_str() {
        "initialize" => handle_initialize(&req),
        "notifications/initialized" => None, // Notification, no response
        "tools/list" => Some(handle_list_tools(&req)),
        "tools/call" => Some(handle_call_tool(&req)),
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
        "protocolVersion": "2024-11-05", // MCP version
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "merab-echo",
            "version": "0.1.0"
        }
    });
    Some(json_success(req.id.clone(), result))
}

fn handle_list_tools(req: &JsonRpcRequest) -> JsonRpcResponse {
    let result = json!({
        "tools": [
            {
                "name": "echo",
                "description": "Echoes back the input message",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "The message to echo"
                        }
                    },
                    "required": ["message"]
                }
            }
        ]
    });
    Some(json_success(req.id.clone(), result)).unwrap()
}

fn handle_call_tool(req: &JsonRpcRequest) -> JsonRpcResponse {
    let params = req.params.as_ref().and_then(|p| p.as_object());

    // MCP tool call arguments are inside "arguments" field if using standard wrapper,
    // or directly in params? Let's assume standard MCP call structure.
    // Spec: params object has 'name' and 'arguments'.

    let name = params.and_then(|p| p.get("name")).and_then(|n| n.as_str());
    let args = params.and_then(|p| p.get("arguments"));

    if name == Some("echo") {
        let msg = args.and_then(|a| a.get("message")).and_then(|m| m.as_str());
        if let Some(m) = msg {
            let result = json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("Echo: {}", m)
                    }
                ],
                "isError": false
            });
            return json_success(req.id.clone(), result);
        } else {
            return json_error(req.id.clone(), -32602, "Missing 'message' argument");
        }
    }

    json_error(req.id.clone(), -32601, "Tool not found")
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
