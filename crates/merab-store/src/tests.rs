#[cfg(test)]
mod tests {
    use crate::CacheEntry;
    use crate::db::Database;
    use merab_core::*;
    use serde_json::json;
    use uuid::Uuid;

    fn test_db() -> Database {
        Database::open_in_memory().expect("in-memory db")
    }

    fn sample_manifest(name: &str) -> AgentManifest {
        AgentManifest {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: "test agent".to_string(),
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            protocol: ProtocolKind::Native,
            working_dir: None,
            restart_on_failure: false,
        }
    }

    // --- Agent tests ---

    #[test]
    fn test_insert_and_get_agent() {
        let db = test_db();
        let record = AgentRecord::new(sample_manifest("agent1"));
        db.insert_agent(&record).unwrap();
        let fetched = db.get_agent(record.id).unwrap().unwrap();
        assert_eq!(fetched.id, record.id);
        assert_eq!(fetched.manifest.name, "agent1");
        assert_eq!(fetched.status, AgentStatus::Registered);
    }

    #[test]
    fn test_list_agents() {
        let db = test_db();
        let r1 = AgentRecord::new(sample_manifest("a1"));
        let r2 = AgentRecord::new(sample_manifest("a2"));
        db.insert_agent(&r1).unwrap();
        db.insert_agent(&r2).unwrap();
        let list = db.list_agents().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_get_agent_by_name() {
        let db = test_db();
        let record = AgentRecord::new(sample_manifest("findme"));
        db.insert_agent(&record).unwrap();
        let found = db.get_agent_by_name("findme").unwrap().unwrap();
        assert_eq!(found.id, record.id);
        assert!(db.get_agent_by_name("nope").unwrap().is_none());
    }

    #[test]
    fn test_update_agent_status() {
        let db = test_db();
        let record = AgentRecord::new(sample_manifest("s1"));
        db.insert_agent(&record).unwrap();
        let updated = db
            .update_agent_status(record.id, AgentStatus::Running, Some(1234))
            .unwrap();
        assert!(updated);
        let fetched = db.get_agent(record.id).unwrap().unwrap();
        assert_eq!(fetched.status, AgentStatus::Running);
        assert_eq!(fetched.pid, Some(1234));
    }

    #[test]
    fn test_update_agent_exit() {
        let db = test_db();
        let record = AgentRecord::new(sample_manifest("e1"));
        db.insert_agent(&record).unwrap();
        db.update_agent_exit(record.id, AgentStatus::Failed, Some(1))
            .unwrap();
        let fetched = db.get_agent(record.id).unwrap().unwrap();
        assert_eq!(fetched.status, AgentStatus::Failed);
        assert_eq!(fetched.exit_code, Some(1));
        assert!(fetched.pid.is_none());
    }

    #[test]
    fn test_delete_agent() {
        let db = test_db();
        let record = AgentRecord::new(sample_manifest("d1"));
        db.insert_agent(&record).unwrap();
        assert!(db.delete_agent(record.id).unwrap());
        assert!(db.get_agent(record.id).unwrap().is_none());
    }

    #[test]
    fn test_insert_duplicate_name_fails() {
        let db = test_db();
        let r1 = AgentRecord::new(sample_manifest("dup"));
        let r2 = AgentRecord::new(sample_manifest("dup"));
        db.insert_agent(&r1).unwrap();
        assert!(db.insert_agent(&r2).is_err());
    }

    // --- Message tests ---

    #[test]
    fn test_insert_and_get_messages() {
        let db = test_db();
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        // Register agents first (foreign key)
        let r1 = AgentRecord {
            id: from,
            ..AgentRecord::new(sample_manifest("msg_from"))
        };
        let r2 = AgentRecord {
            id: to,
            ..AgentRecord::new(sample_manifest("msg_to"))
        };
        db.insert_agent(&r1).unwrap();
        db.insert_agent(&r2).unwrap();

        let msg = Message::new(from, Some(to), "hello".to_string());
        db.insert_message(&msg).unwrap();

        let msgs = db.get_messages_for(to).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "hello");
    }

    #[test]
    fn test_acknowledge_message() {
        let db = test_db();
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let r1 = AgentRecord {
            id: from,
            ..AgentRecord::new(sample_manifest("ack_from"))
        };
        let r2 = AgentRecord {
            id: to,
            ..AgentRecord::new(sample_manifest("ack_to"))
        };
        db.insert_agent(&r1).unwrap();
        db.insert_agent(&r2).unwrap();

        let msg = Message::new(from, Some(to), "ack me".to_string());
        let msg_id = msg.id;
        db.insert_message(&msg).unwrap();

        assert!(db.acknowledge_message(msg_id).unwrap());
        // After ack, message should not appear in pending
        let msgs = db.get_messages_for(to).unwrap();
        assert_eq!(msgs.len(), 0);
    }

    #[test]
    fn test_delete_agent_messages() {
        let db = test_db();
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let r1 = AgentRecord {
            id: from,
            ..AgentRecord::new(sample_manifest("del_from"))
        };
        let r2 = AgentRecord {
            id: to,
            ..AgentRecord::new(sample_manifest("del_to"))
        };
        db.insert_agent(&r1).unwrap();
        db.insert_agent(&r2).unwrap();

        let msg = Message::new(from, Some(to), "bye".to_string());
        db.insert_message(&msg).unwrap();

        db.delete_agent_messages(from).unwrap();
        let msgs = db.get_messages_for(to).unwrap();
        assert_eq!(msgs.len(), 0);
    }

    #[test]
    fn test_broadcast_message() {
        let db = test_db();
        let from = Uuid::new_v4();
        let other = Uuid::new_v4();
        let r1 = AgentRecord {
            id: from,
            ..AgentRecord::new(sample_manifest("bc_from"))
        };
        let r2 = AgentRecord {
            id: other,
            ..AgentRecord::new(sample_manifest("bc_other"))
        };
        db.insert_agent(&r1).unwrap();
        db.insert_agent(&r2).unwrap();

        // Broadcast (to_agent = None)
        let msg = Message::new(from, None, "broadcast".to_string());
        db.insert_message(&msg).unwrap();

        // Other agent should see it
        let msgs = db.get_messages_for(other).unwrap();
        assert_eq!(msgs.len(), 1);

        // Sender should NOT see own broadcast
        let msgs = db.get_messages_for(from).unwrap();
        assert_eq!(msgs.len(), 0);
    }

    // --- Task tests ---

    #[test]
    fn test_create_and_get_task() {
        let db = test_db();
        let task = Task {
            id: Uuid::new_v4(),
            source: Some("test".to_string()),
            target: "skill1".to_string(),
            input: r#"{"key":"value"}"#.to_string(),
            status: TaskStatus::Pending,
            output: None,
            error: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        db.create_task(&task).unwrap();
        let fetched = db.get_task(&task.id.to_string()).unwrap();
        assert_eq!(fetched.id, task.id);
        assert_eq!(fetched.target, "skill1");
        assert_eq!(fetched.status, TaskStatus::Pending);
    }

    #[test]
    fn test_update_task_status() {
        let db = test_db();
        let task = Task {
            id: Uuid::new_v4(),
            source: None,
            target: "skill2".to_string(),
            input: "{}".to_string(),
            status: TaskStatus::Pending,
            output: None,
            error: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        db.create_task(&task).unwrap();
        db.update_task_status(
            &task.id.to_string(),
            TaskStatus::Completed,
            Some("result".to_string()),
            None,
        )
        .unwrap();
        let fetched = db.get_task(&task.id.to_string()).unwrap();
        assert_eq!(fetched.status, TaskStatus::Completed);
        assert_eq!(fetched.output, Some("result".to_string()));
    }

    // --- Cache tests ---

    #[test]
    fn test_store_and_get_cache() {
        let db = test_db();
        let entry = CacheEntry {
            hash: "abc123".to_string(),
            request: r#"{"model":"gpt-4"}"#.to_string(),
            response: r#"{"choices":[]}"#.to_string(),
            model: "gpt-4".to_string(),
            provider: Some("openai".to_string()),
        };
        db.store_cache(&entry).unwrap();
        let cached = db.get_cache("abc123").unwrap().unwrap();
        assert_eq!(cached.hash, "abc123");
        assert_eq!(cached.model, "gpt-4");
    }

    #[test]
    fn test_cache_miss() {
        let db = test_db();
        assert!(db.get_cache("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_cache_stats() {
        let db = test_db();
        let entry = CacheEntry {
            hash: "stat1".to_string(),
            request: "req".to_string(),
            response: "res".to_string(),
            model: "m".to_string(),
            provider: None,
        };
        db.store_cache(&entry).unwrap();
        // get_cache increments hits
        db.get_cache("stat1").unwrap();
        let (hits, total) = db.get_proxy_stats().unwrap();
        assert_eq!(total, 1);
        assert!(hits >= 2); // initial 1 + 1 from get_cache
    }

    // --- Memory tests ---

    #[test]
    fn test_put_and_get_memory() {
        let db = test_db();
        let val = json!({"foo": "bar"});
        db.put_memory("key1", &val, None, None).unwrap();
        let got = db.get_memory("key1").unwrap().unwrap();
        assert_eq!(got, val);
    }

    #[test]
    fn test_delete_memory() {
        let db = test_db();
        db.put_memory("k", &json!(1), None, None).unwrap();
        assert!(db.delete_memory("k").unwrap());
        assert!(db.get_memory("k").unwrap().is_none());
    }

    #[test]
    fn test_list_memory_keys() {
        let db = test_db();
        db.put_memory("app.setting1", &json!(1), None, None)
            .unwrap();
        db.put_memory("app.setting2", &json!(2), None, None)
            .unwrap();
        db.put_memory("other.key", &json!(3), None, None).unwrap();

        let all = db.list_memory_keys(None).unwrap();
        assert_eq!(all.len(), 3);

        let app_keys = db.list_memory_keys(Some("app.")).unwrap();
        assert_eq!(app_keys.len(), 2);
    }

    #[test]
    fn test_memory_ttl_expiration() {
        let db = test_db();
        // Insert with a normal TTL first, then override expires_at to a past date via SQL
        db.put_memory("ttl_key", &json!("temp"), None, None)
            .unwrap();
        // Override expires_at to a past date
        db.conn.execute(
            "UPDATE shared_memory SET expires_at = '2000-01-01T00:00:00+00:00' WHERE key = 'ttl_key'",
            [],
        ).unwrap();

        // Should return None because it's expired
        assert!(db.get_memory("ttl_key").unwrap().is_none());
    }

    #[test]
    fn test_memory_sql_injection_safe() {
        let db = test_db();
        db.put_memory("safe.key", &json!(1), None, None).unwrap();

        // This prefix would break an unparameterized query
        let malicious = "'; DROP TABLE shared_memory; --";
        let keys = db.list_memory_keys(Some(malicious)).unwrap();
        assert_eq!(keys.len(), 0);

        // Table should still exist and work
        let val = db.get_memory("safe.key").unwrap();
        assert!(val.is_some());
    }

    #[test]
    fn test_memory_overwrite() {
        let db = test_db();
        db.put_memory("ow", &json!(1), None, None).unwrap();
        db.put_memory("ow", &json!(2), None, None).unwrap();
        let val = db.get_memory("ow").unwrap().unwrap();
        assert_eq!(val, json!(2));
    }
}
