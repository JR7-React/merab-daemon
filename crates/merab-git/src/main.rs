use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::process::Command;

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
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting Merab Git Agent...");

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
        "notifications/initialized" => None, // Notification — no response
        "tools/list" => Some(handle_list_tools(&req)),
        "tools/call" => Some(handle_call_tool(&req).await),
        "ping" => Some(json_success(req.id.clone(), json!("pong"))),
        _ => {
            tracing::warn!("Unknown method: {}", req.method);
            Some(json_error(req.id.clone(), -32601, "Method not found"))
        }
    };

    if let Some(resp) = response {
        let resp_str = serde_json::to_string(&resp).unwrap_or_default();
        println!("{}", resp_str);
        // Flush stdout — critical when piped to daemon
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
            "name": "merab-git",
            "version": "0.1.0"
        }
    });
    Some(json_success(req.id.clone(), result))
}

fn handle_list_tools(req: &JsonRpcRequest) -> JsonRpcResponse {
    let result = json!({
        "tools": [
            {
                "name": "git.status",
                "description": "Get git status (porcelain)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": { "type": "string", "description": "Path to git repository" }
                    },
                    "required": ["repo_path"]
                }
            },
            {
                "name": "git.diff",
                "description": "Get git diff",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": { "type": "string", "description": "Path to git repository" },
                        "cached": { "type": "boolean", "description": "Show cached changes (staged)" },
                        "branch": { "type": "string", "description": "Compare against this branch (e.g., main, develop)" },
                        "path": { "type": "string", "description": "Show diff for specific file or directory" }
                    },
                    "required": ["repo_path"]
                }
            },
            {
                "name": "git.commit",
                "description": "Commit changes",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": { "type": "string", "description": "Path to git repository" },
                        "message": { "type": "string", "description": "Commit message" }
                    },
                    "required": ["repo_path", "message"]
                }
            },
            {
                "name": "git.add",
                "description": "Stage files for commit (git add)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": { "type": "string", "description": "Path to git repository" },
                        "files": { "type": "string", "description": "Files to stage (space-separated, or '.' for all)" }
                    },
                    "required": ["repo_path", "files"]
                }
            },
            {
                "name": "git.log",
                "description": "Get recent git log",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "repo_path": { "type": "string", "description": "Path to git repository" },
                        "limit": { "type": "integer", "description": "Number of commits" }
                    },
                    "required": ["repo_path"]
                }
            }
        ]
    });
    json_success(req.id.clone(), result)
}

async fn handle_call_tool(req: &JsonRpcRequest) -> JsonRpcResponse {
    let params = req.params.as_ref().and_then(|p| p.as_object());
    let name = params
        .and_then(|p| p.get("name").and_then(|n| n.as_str()));
    let args = params.and_then(|p| p.get("arguments").and_then(|a| a.as_object()));

    let result = match name {
        Some("git.status") => {
            let repo_path = args
                .and_then(|a| a.get("repo_path").and_then(|p| p.as_str()))
                .unwrap_or(".");
            run_git(repo_path, &["status", "--porcelain"]).await
        }
        Some("git.diff") => {
            let repo_path = args
                .and_then(|a| a.get("repo_path").and_then(|p| p.as_str()))
                .unwrap_or(".");
            let cached = args
                .and_then(|a| a.get("cached").and_then(|c| c.as_bool()))
                .unwrap_or(false);
            let branch = args
                .and_then(|a| a.get("branch").and_then(|b| b.as_str()));
            let path = args
                .and_then(|a| a.get("path").and_then(|p| p.as_str()));
            let mut cmd_args = vec!["diff".to_string()];
            if cached {
                cmd_args.push("--cached".to_string());
            }
            if let Some(b) = branch {
                cmd_args.push(format!("{}...HEAD", b));
            }
            if let Some(p) = path {
                cmd_args.push("--".to_string());
                cmd_args.push(p.to_string());
            }
            let cmd_refs: Vec<&str> = cmd_args.iter().map(|s| s.as_str()).collect();
            run_git(repo_path, &cmd_refs).await
        }
        Some("git.add") => {
            let repo_path = args
                .and_then(|a| a.get("repo_path").and_then(|p| p.as_str()))
                .unwrap_or(".");
            let files = args
                .and_then(|a| a.get("files").and_then(|f| f.as_str()))
                .unwrap_or(".");
            let file_args: Vec<&str> = files.split_whitespace().collect();
            let mut cmd_args = vec!["add"];
            cmd_args.extend(file_args);
            run_git(repo_path, &cmd_args).await
        }
        Some("git.commit") => {
            let repo_path = args
                .and_then(|a| a.get("repo_path").and_then(|p| p.as_str()))
                .unwrap_or(".");
            let message = args
                .and_then(|a| a.get("message").and_then(|m| m.as_str()))
                .unwrap_or("update");
            run_git(repo_path, &["commit", "-m", message]).await
        }
        Some("git.log") => {
            let repo_path = args
                .and_then(|a| a.get("repo_path").and_then(|p| p.as_str()))
                .unwrap_or(".");
            let limit = args
                .and_then(|a| a.get("limit").and_then(|l| l.as_u64()))
                .unwrap_or(10);
            let limit_str = format!("-n{}", limit);
            run_git(repo_path, &["log", "--oneline", &limit_str]).await
        }
        _ => return json_error(req.id.clone(), -32601, "Tool not found"),
    };

    match result {
        Ok(output) => json_success(
            req.id.clone(),
            json!({ "content": [{ "type": "text", "text": output }], "isError": false }),
        ),
        Err(e) => json_success(
            req.id.clone(),
            json!({ "content": [{ "type": "text", "text": format!("Error: {}", e) }], "isError": true }),
        ),
    }
}

async fn run_git(repo_path: &str, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git command failed: {}", stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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
