use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use forge_config::ForgeConfig;
use forge_core::{AgentManifest, AgentRecord, AgentSummary, ProtocolKind};
use forge_daemon::mcp_manager::McpManager;
use forge_daemon::registry::AgentRegistry;
use forge_daemon::rpc::{ForgeApiServer, ForgeRpc};
use forge_daemon::supervisor::ProcessSupervisor;
use forge_store::Database;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::params::ObjectParams;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::rpc_params;
use jsonrpsee::server::Server;
use serde_json::json;
use tokio::sync::Mutex;

/// Spin up a real JSON-RPC server on an ephemeral port and return a connected client + address.
async fn setup() -> (HttpClient, SocketAddr) {
    let db = Database::open_in_memory().expect("in-memory db");
    let registry = Arc::new(AgentRegistry::new());
    let db = Arc::new(Mutex::new(db));
    let supervisor = Arc::new(ProcessSupervisor::new(registry.clone(), db.clone()));
    let mcp_manager = Arc::new(McpManager::new());
    let config = Arc::new(ForgeConfig::default());

    let rpc = ForgeRpc {
        registry,
        db,
        supervisor,
        mcp_manager,
        config,
        start_time: Instant::now(),
    };

    // Bind to port 0 to get an ephemeral port
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let server = Server::builder().build(addr).await.expect("build server");
    let bound_addr = server.local_addr().expect("get local addr");
    let handle = server.start(rpc.into_rpc());

    // Keep server alive in background
    tokio::spawn(async move { handle.stopped().await });

    let url = format!("http://{}", bound_addr);
    let client = HttpClientBuilder::default()
        .build(&url)
        .expect("build client");

    (client, bound_addr)
}

fn sample_manifest(name: &str) -> AgentManifest {
    AgentManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        description: "integration test agent".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        protocol: ProtocolKind::Native,
        working_dir: None,
        restart_on_failure: false,
    }
}

// ---- Basic RPC tests ----

#[tokio::test]
async fn test_ping() {
    let (client, _) = setup().await;
    let result: String = client
        .request("forge.ping", ObjectParams::new())
        .await
        .expect("ping");
    assert_eq!(result, "pong");
}

#[tokio::test]
async fn test_register_returns_record() {
    let (client, _) = setup().await;
    let manifest = sample_manifest("reg1");
    let mut params = ObjectParams::new();
    params.insert("manifest", &manifest).unwrap();
    let record: AgentRecord = client
        .request("forge.registerAgent", params)
        .await
        .expect("register");
    assert_eq!(record.manifest.name, "reg1");
    assert!(!record.id.is_nil());
}

#[tokio::test]
async fn test_list_contains_registered() {
    let (client, _) = setup().await;
    let manifest = sample_manifest("list1");
    let mut params = ObjectParams::new();
    params.insert("manifest", &manifest).unwrap();
    let _: AgentRecord = client
        .request("forge.registerAgent", params)
        .await
        .expect("register");

    let list: Vec<AgentSummary> = client
        .request("forge.listAgents", ObjectParams::new())
        .await
        .expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "list1");
}

#[tokio::test]
async fn test_get_agent() {
    let (client, _) = setup().await;
    let manifest = sample_manifest("get1");
    let mut params = ObjectParams::new();
    params.insert("manifest", &manifest).unwrap();
    let record: AgentRecord = client
        .request("forge.registerAgent", params)
        .await
        .expect("register");

    let mut params = ObjectParams::new();
    params.insert("id", record.id.to_string()).unwrap();
    let fetched: AgentRecord = client.request("forge.getAgent", params).await.expect("get");
    assert_eq!(fetched.id, record.id);
}

#[tokio::test]
async fn test_register_duplicate_fails() {
    let (client, _) = setup().await;
    let manifest = sample_manifest("dup1");

    let mut params = ObjectParams::new();
    params.insert("manifest", &manifest).unwrap();
    let _: AgentRecord = client
        .request("forge.registerAgent", params)
        .await
        .expect("first register");

    let mut params = ObjectParams::new();
    params.insert("manifest", &manifest).unwrap();
    let result: Result<AgentRecord, _> = client.request("forge.registerAgent", params).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_start_nonexistent_agent_fails() {
    let (client, _) = setup().await;
    let fake_id = uuid::Uuid::new_v4().to_string();
    let mut params = ObjectParams::new();
    params.insert("id", &fake_id).unwrap();
    let result: Result<AgentRecord, _> = client.request("forge.startAgent", params).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_stop_non_running_agent_fails() {
    let (client, _) = setup().await;
    let manifest = sample_manifest("stop1");
    let mut params = ObjectParams::new();
    params.insert("manifest", &manifest).unwrap();
    let record: AgentRecord = client
        .request("forge.registerAgent", params)
        .await
        .expect("register");

    let mut params = ObjectParams::new();
    params.insert("id", record.id.to_string()).unwrap();
    let result: Result<AgentRecord, _> = client.request("forge.stopAgent", params).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_unregister_cleans_up() {
    let (client, _) = setup().await;
    let manifest = sample_manifest("unreg1");
    let mut params = ObjectParams::new();
    params.insert("manifest", &manifest).unwrap();
    let record: AgentRecord = client
        .request("forge.registerAgent", params)
        .await
        .expect("register");

    let mut params = ObjectParams::new();
    params.insert("id", record.id.to_string()).unwrap();
    let ok: bool = client
        .request("forge.unregisterAgent", params)
        .await
        .expect("unregister");
    assert!(ok);

    let list: Vec<AgentSummary> = client
        .request("forge.listAgents", ObjectParams::new())
        .await
        .expect("list");
    assert_eq!(list.len(), 0);
}

// ---- Memory RPC tests ----

#[tokio::test]
async fn test_memory_put_get_delete() {
    let (client, _) = setup().await;

    // Put
    let mut params = ObjectParams::new();
    params.insert("key", "test.key").unwrap();
    params.insert("value", &json!({"hello": "world"})).unwrap();
    params.insert("ttl_seconds", Option::<u64>::None).unwrap();
    let ok: bool = client
        .request("forge.memory.put", params)
        .await
        .expect("put");
    assert!(ok);

    // Get
    let mut params = ObjectParams::new();
    params.insert("key", "test.key").unwrap();
    let val: Option<serde_json::Value> = client
        .request("forge.memory.get", params)
        .await
        .expect("get");
    assert_eq!(val, Some(json!({"hello": "world"})));

    // Delete
    let mut params = ObjectParams::new();
    params.insert("key", "test.key").unwrap();
    let deleted: bool = client
        .request("forge.memory.delete", params)
        .await
        .expect("delete");
    assert!(deleted);

    // Get returns None
    let mut params = ObjectParams::new();
    params.insert("key", "test.key").unwrap();
    let val: Option<serde_json::Value> = client
        .request("forge.memory.get", params)
        .await
        .expect("get after delete");
    assert!(val.is_none());
}

#[tokio::test]
async fn test_memory_ttl_expiration() {
    let (client, _) = setup().await;

    // Put with TTL of 1 second
    let mut params = ObjectParams::new();
    params.insert("key", "ttl.key").unwrap();
    params.insert("value", &json!("temporary")).unwrap();
    params.insert("ttl_seconds", Some(1u64)).unwrap();
    let _: bool = client
        .request("forge.memory.put", params)
        .await
        .expect("put with ttl");

    // Sleep 2 seconds to let it expire
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Get should return None
    let mut params = ObjectParams::new();
    params.insert("key", "ttl.key").unwrap();
    let val: Option<serde_json::Value> = client
        .request("forge.memory.get", params)
        .await
        .expect("get expired");
    assert!(val.is_none());
}

#[tokio::test]
async fn test_memory_list() {
    let (client, _) = setup().await;

    // Put multiple keys
    for key in &["ns.a", "ns.b", "other.c"] {
        let mut params = ObjectParams::new();
        params.insert("key", *key).unwrap();
        params.insert("value", &json!(1)).unwrap();
        params.insert("ttl_seconds", Option::<u64>::None).unwrap();
        let _: bool = client
            .request("forge.memory.put", params)
            .await
            .expect("put");
    }

    // List all
    let mut params = ObjectParams::new();
    params.insert("prefix", Option::<String>::None).unwrap();
    let keys: Vec<String> = client
        .request("forge.memory.list", params)
        .await
        .expect("list all");
    assert_eq!(keys.len(), 3);

    // List with prefix
    let mut params = ObjectParams::new();
    params.insert("prefix", Some("ns.")).unwrap();
    let keys: Vec<String> = client
        .request("forge.memory.list", params)
        .await
        .expect("list prefix");
    assert_eq!(keys.len(), 2);
}

// ---- System status ----

#[tokio::test]
async fn test_system_status() {
    let (client, _) = setup().await;
    let status: forge_core::SystemStatus = client
        .request("forge.getSystemStatus", ObjectParams::new())
        .await
        .expect("status");
    assert_eq!(status.agents.len(), 0);
    assert!(status.node_info.uptime_seconds < 10);
}

// ---- AI RPC tests ----

#[tokio::test]
async fn test_ai_chat_method_exists() {
    let (client, _addr) = setup().await;
    let msg = "Hello from test";
    let ctx: Vec<forge_ai::ChatMessage> = vec![];
    let ctx_json = serde_json::to_string(&ctx).unwrap();

    let res: Result<forge_ai::AiResponse, _> = client
        .request("forge.ai.chat", rpc_params![msg, ctx_json])
        .await;

    if let Err(e) = res {
        let err_str = format!("{:?}", e);
        assert!(
            !err_str.contains("-32601"),
            "forge.ai.chat method not found: {}",
            err_str
        );
    }
}

#[tokio::test]
async fn test_ai_orchestrate_method_exists() {
    let (client, _addr) = setup().await;
    let task = "Analyze this";

    let res: Result<forge_ai::AiResponse, _> = client
        .request("forge.ai.orchestrate", rpc_params![task])
        .await;

    if let Err(e) = res {
        let err_str = format!("{:?}", e);
        assert!(
            !err_str.contains("-32601"),
            "forge.ai.orchestrate method not found: {}",
            err_str
        );
    }
}
