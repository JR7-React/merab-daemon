#[cfg(test)]
mod tests {
    use crate::agent::*;
    use crate::error::MerabError;
    use crate::message::*;
    use crate::task::TaskStatus;
    use uuid::Uuid;

    fn sample_manifest() -> AgentManifest {
        AgentManifest {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            description: "A test agent".to_string(),
            command: "echo".to_string(),
            args: vec![],
            protocol: ProtocolKind::Native,
            working_dir: None,
            restart_on_failure: false,
        }
    }

    // --- AgentManifest TOML round-trip ---
    #[test]
    fn test_manifest_toml_roundtrip() {
        let manifest = sample_manifest();
        let toml_str = toml::to_string(&manifest).expect("serialize to TOML");
        let parsed: AgentManifest = toml::from_str(&toml_str).expect("parse from TOML");
        assert_eq!(parsed.name, manifest.name);
        assert_eq!(parsed.version, manifest.version);
        assert_eq!(parsed.command, manifest.command);
        assert_eq!(parsed.protocol, ProtocolKind::Native);
    }

    #[test]
    fn test_manifest_toml_with_mcp_protocol() {
        let toml_str = r#"
            name = "mcp-agent"
            version = "2.0.0"
            command = "node"
            args = ["server.js"]
            protocol = "mcp"
        "#;
        let manifest: AgentManifest = toml::from_str(toml_str).expect("parse MCP manifest");
        assert_eq!(manifest.protocol, ProtocolKind::Mcp);
        assert_eq!(manifest.args, vec!["server.js"]);
    }

    #[test]
    fn test_manifest_default_protocol_is_native() {
        let toml_str = r#"
            name = "default-proto"
            version = "1.0.0"
            command = "test"
        "#;
        let manifest: AgentManifest =
            toml::from_str(toml_str).expect("parse manifest without protocol");
        assert_eq!(manifest.protocol, ProtocolKind::Native);
    }

    // --- ProtocolKind serialization ---
    #[test]
    fn test_protocol_kind_serialization() {
        assert_eq!(
            serde_json::to_string(&ProtocolKind::Mcp).unwrap(),
            r#""mcp""#
        );
        assert_eq!(
            serde_json::to_string(&ProtocolKind::A2a).unwrap(),
            r#""a2a""#
        );
        assert_eq!(
            serde_json::to_string(&ProtocolKind::Native).unwrap(),
            r#""native""#
        );
    }

    #[test]
    fn test_protocol_kind_deserialization() {
        assert_eq!(
            serde_json::from_str::<ProtocolKind>(r#""mcp""#).unwrap(),
            ProtocolKind::Mcp
        );
        assert_eq!(
            serde_json::from_str::<ProtocolKind>(r#""a2a""#).unwrap(),
            ProtocolKind::A2a
        );
        assert_eq!(
            serde_json::from_str::<ProtocolKind>(r#""native""#).unwrap(),
            ProtocolKind::Native
        );
    }

    // --- AgentRecord::new ---
    #[test]
    fn test_agent_record_new() {
        let record = AgentRecord::new(sample_manifest());
        // Should have a valid UUID (non-nil)
        assert_ne!(record.id, Uuid::nil());
        assert_eq!(record.status, AgentStatus::Registered);
        assert!(record.pid.is_none());
        assert!(record.started_at.is_none());
        assert!(record.stopped_at.is_none());
        assert!(record.exit_code.is_none());
    }

    #[test]
    fn test_agent_record_unique_ids() {
        let r1 = AgentRecord::new(sample_manifest());
        let r2 = AgentRecord::new(sample_manifest());
        assert_ne!(r1.id, r2.id);
    }

    // --- MerabError codes ---
    #[test]
    fn test_merab_error_codes() {
        let id = Uuid::new_v4();
        assert_eq!(MerabError::AgentNotFound(id).code(), -32001);
        assert_eq!(MerabError::AlreadyExists("x".into()).code(), -32002);
        assert_eq!(MerabError::NotRunning(id).code(), -32003);
        assert_eq!(MerabError::AlreadyRunning(id).code(), -32004);
        assert_eq!(MerabError::ToolNotFound("t".into()).code(), -32005);
        assert_eq!(MerabError::MessageNotFound(id).code(), -32006);
        assert_eq!(MerabError::MemoryKeyNotFound("k".into()).code(), -32007);
        assert_eq!(MerabError::PermissionDenied("p".into()).code(), -32008);
        assert_eq!(MerabError::InvalidManifest("m".into()).code(), -32602);
        assert_eq!(MerabError::TaskFailed("f".into()).code(), -32009);
        assert_eq!(MerabError::Store("s".into()).code(), -32010);
        assert_eq!(MerabError::Transport("t".into()).code(), -32011);
        assert_eq!(MerabError::Internal("i".into()).code(), -32603);
    }

    #[test]
    fn test_merab_error_display() {
        let err = MerabError::AgentNotFound(Uuid::nil());
        assert!(err.to_string().contains("agent not found"));
    }

    // --- Message::new ---
    #[test]
    fn test_message_new() {
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let msg = Message::new(from, Some(to), "hello".to_string());
        assert_ne!(msg.id, Uuid::nil());
        assert_eq!(msg.from_agent, from);
        assert_eq!(msg.to_agent, Some(to));
        assert_eq!(msg.content, "hello");
        assert_eq!(msg.status, MessageStatus::Pending);
        assert!(msg.delivered_at.is_none());
    }

    #[test]
    fn test_message_broadcast() {
        let from = Uuid::new_v4();
        let msg = Message::new(from, None, "broadcast".to_string());
        assert!(msg.to_agent.is_none());
    }

    // --- TaskStatus ---
    #[test]
    fn test_task_status_to_string() {
        assert_eq!(TaskStatus::Pending.to_string(), "pending");
        assert_eq!(TaskStatus::Running.to_string(), "running");
        assert_eq!(TaskStatus::Completed.to_string(), "completed");
        assert_eq!(TaskStatus::Failed.to_string(), "failed");
        assert_eq!(TaskStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn test_task_status_from_str() {
        assert_eq!(TaskStatus::from("pending"), TaskStatus::Pending);
        assert_eq!(TaskStatus::from("running"), TaskStatus::Running);
        assert_eq!(TaskStatus::from("completed"), TaskStatus::Completed);
        assert_eq!(TaskStatus::from("failed"), TaskStatus::Failed);
        assert_eq!(TaskStatus::from("cancelled"), TaskStatus::Cancelled);
        assert_eq!(TaskStatus::from("unknown"), TaskStatus::Failed); // default
    }

    // --- AgentStatus serialization ---
    #[test]
    fn test_agent_status_serialization() {
        assert_eq!(
            serde_json::to_string(&AgentStatus::Registered).unwrap(),
            r#""registered""#
        );
        assert_eq!(
            serde_json::to_string(&AgentStatus::Running).unwrap(),
            r#""running""#
        );
        assert_eq!(
            serde_json::to_string(&AgentStatus::Stopped).unwrap(),
            r#""stopped""#
        );
        assert_eq!(
            serde_json::to_string(&AgentStatus::Failed).unwrap(),
            r#""failed""#
        );
    }
}
