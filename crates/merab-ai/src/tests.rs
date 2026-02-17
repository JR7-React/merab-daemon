#[cfg(test)]
mod tests {
    use crate::client::AiClient;
    use crate::error::AiError;
    use crate::types::*;

    fn test_config() -> AiClientConfig {
        AiClientConfig {
            proxy_url: "http://127.0.0.1:8001".to_string(),
            model: "openai/gpt-3.5-turbo".to_string(),
            system_prompt: Some("You are a helpful assistant.".to_string()),
            max_tokens: 256,
            temperature: 0.7,
        }
    }

    #[test]
    fn test_chat_message_constructors() {
        let sys = ChatMessage::system("hello");
        assert_eq!(sys.role, ChatRole::System);
        assert_eq!(sys.content, "hello");

        let user = ChatMessage::user("question");
        assert_eq!(user.role, ChatRole::User);

        let asst = ChatMessage::assistant("answer");
        assert_eq!(asst.role, ChatRole::Assistant);
    }

    #[test]
    fn test_chat_role_serialization() {
        let role = ChatRole::User;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"user\"");

        let deserialized: ChatRole = serde_json::from_str("\"assistant\"").unwrap();
        assert_eq!(deserialized, ChatRole::Assistant);
    }

    #[test]
    fn test_chat_message_roundtrip() {
        let msg = ChatMessage::user("test message");
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.role, ChatRole::User);
        assert_eq!(parsed.content, "test message");
    }

    #[test]
    fn test_ai_response_with_tool_call() {
        let response = AiResponse {
            content: "calling tool".to_string(),
            model: Some("test-model".to_string()),
            tool_call: Some(ToolCall {
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": "/tmp/test.txt"}),
            }),
        };
        let json = serde_json::to_string(&response).unwrap();
        let parsed: AiResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.tool_call.is_some());
        assert_eq!(parsed.tool_call.unwrap().name, "read_file");
    }

    #[test]
    fn test_ai_client_set_system_prompt() {
        let config = test_config();
        let mut client = AiClient::new(config);
        client.set_system_prompt("New system prompt".to_string());
        // Client created successfully with custom prompt — no panic
    }

    #[test]
    fn test_ai_error_display() {
        let err = AiError::EmptyResponse;
        assert_eq!(err.to_string(), "no content in LLM response");

        let err = AiError::NotConfigured("missing model".to_string());
        assert!(err.to_string().contains("missing model"));
    }
}
