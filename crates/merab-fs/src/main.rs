use glob::glob;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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
    // Configure logging to stderr
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting Merab FileSystem Agent...");

    // Determine root directory (current working directory by default)
    let root_dir = std::env::current_dir()?;
    tracing::info!("Root directory: {:?}", root_dir);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        tracing::debug!("Received: {}", line);

        match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(req) => handle_request(req, &root_dir),
            Err(e) => {
                tracing::error!("Failed to parse request: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_request(req: JsonRpcRequest, root_dir: &Path) {
    let response = match req.method.as_str() {
        "initialize" => handle_initialize(&req),
        "notifications/initialized" => None,
        "tools/list" => Some(handle_list_tools(&req)),
        "tools/call" => Some(handle_call_tool(&req, root_dir)),
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
            "name": "merab-fs",
            "version": "0.1.0"
        }
    });
    Some(json_success(req.id.clone(), result))
}

fn handle_list_tools(req: &JsonRpcRequest) -> JsonRpcResponse {
    let result = json!({
        "tools": [
            {
                "name": "fs.list",
                "description": "List files and directories",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Directory to list (relative to root)" },
                        "recursive": { "type": "boolean", "description": "List recursively" }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fs.read",
                "description": "Read file content",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path to read" }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "fs.write",
                "description": "Write content to a file",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path to write" },
                        "content": { "type": "string", "description": "Content to write" }
                    },
                    "required": ["path", "content"]
                }
            },
            {
                "name": "fs.search",
                "description": "Search files using glob pattern",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "base_path": { "type": "string", "description": "Base directory for search" },
                        "pattern": { "type": "string", "description": "Glob pattern (e.g. **/*.rs)" }
                    },
                    "required": ["pattern"]
                }
            }
        ]
    });
    Some(json_success(req.id.clone(), result)).unwrap()
}

fn handle_call_tool(req: &JsonRpcRequest, root_dir: &Path) -> JsonRpcResponse {
    let params = req.params.as_ref().and_then(|p| p.as_object());
    let name = params.and_then(|p| p.get("name")).and_then(|n| n.as_str());
    let args = params.and_then(|p| p.get("arguments"));

    let result = match name {
        Some("fs.list") => tool_list(args, root_dir),
        Some("fs.read") => tool_read(args, root_dir),
        Some("fs.write") => tool_write(args, root_dir),
        Some("fs.search") => tool_search(args, root_dir),
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

// --- Tool Implementations ---

fn resolve_path(root: &Path, user_path: &str) -> anyhow::Result<PathBuf> {
    // Basic security: prevent escaping root
    // This is a naive implementation. In production, use `clean_path` or canonicalize carefully.
    let path = root.join(user_path);
    // For now we trust the path is reasonably safe or we catch errors on access.
    // Ideally check if simplified path starts with root.
    Ok(path)
}

fn tool_list(args: Option<&Value>, root: &Path) -> anyhow::Result<String> {
    let args = args.ok_or(anyhow::anyhow!("Missing arguments"))?;
    let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let target_path = resolve_path(root, path_str)?;

    if !target_path.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {}", path_str));
    }

    let mut output = String::new();
    output.push_str(&format!("Listing for {}:\n", path_str));

    if recursive {
        // Skip hidden dirs (.git, .env, etc.) and node_modules, target/
        let skip_dirs = [".git", "node_modules", "target", ".claude"];
        for entry in WalkDir::new(&target_path)
            .max_depth(3)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !skip_dirs.iter().any(|s| name == *s)
            })
        {
            match entry {
                Ok(e) => {
                    let depth = e.depth();
                    let indent = "  ".repeat(depth);
                    let name = e.file_name().to_string_lossy();
                    let suffix = if e.file_type().is_dir() { "/" } else { "" };
                    output.push_str(&format!("{}{}{}\n", indent, name, suffix));
                }
                Err(e) => output.push_str(&format!("Error accessing: {}\n", e)),
            }
        }
    } else {
        for entry in fs::read_dir(&target_path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let suffix = if entry.file_type()?.is_dir() { "/" } else { "" };
            output.push_str(&format!("- {}{}\n", name, suffix));
        }
    }

    Ok(output)
}

fn tool_read(args: Option<&Value>, root: &Path) -> anyhow::Result<String> {
    let args = args.ok_or(anyhow::anyhow!("Missing arguments"))?;
    let path_str = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or(anyhow::anyhow!("Missing path"))?;

    let target_path = resolve_path(root, path_str)?;
    let content = fs::read_to_string(&target_path)?;

    Ok(content)
}

fn tool_write(args: Option<&Value>, root: &Path) -> anyhow::Result<String> {
    let args = args.ok_or(anyhow::anyhow!("Missing arguments"))?;
    let path_str = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or(anyhow::anyhow!("Missing path"))?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or(anyhow::anyhow!("Missing content"))?;

    let target_path = resolve_path(root, path_str)?;

    // Ensure parent directory exists
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::File::create(&target_path)?;
    file.write_all(content.as_bytes())?;

    Ok(format!("Successfully wrote to {}", path_str))
}

fn tool_search(args: Option<&Value>, root: &Path) -> anyhow::Result<String> {
    let args = args.ok_or(anyhow::anyhow!("Missing arguments"))?;
    let base_path_str = args
        .get("base_path")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or(anyhow::anyhow!("Missing pattern"))?;

    let base_path = resolve_path(root, base_path_str)?;
    let glob_pattern = base_path.join(pattern);
    let glob_str = glob_pattern.to_string_lossy();

    let mut output = String::new();
    output.push_str(&format!("Search results for '{}':\n", pattern));

    for entry in glob(&glob_str)? {
        match entry {
            Ok(path) => {
                // Return relative path if possible
                let display_path = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
                output.push_str(&format!("- {}\n", display_path));
            }
            Err(e) => output.push_str(&format!("Error: {}\n", e)),
        }
    }

    Ok(output)
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
