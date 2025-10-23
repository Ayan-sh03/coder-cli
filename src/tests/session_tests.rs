use crate::session::Session;
use crate::types::Message;
use chrono::Utc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = Session::new(Some("Test Session"), Some("gpt-4"));
        
        assert!(!session.id.is_empty());
        assert_eq!(session.title, Some("Test Session".to_string()));
        assert_eq!(session.model, Some("gpt-4".to_string()));
        assert!(session.messages.is_empty());
        assert!(session.created_at <= Utc::now());
        assert!(session.updated_at <= Utc::now());
    }

    #[test]
    fn test_session_creation_without_optional_params() {
        let session = Session::new(None, None);
        
        assert!(!session.id.is_empty());
        assert_eq!(session.title, None);
        assert_eq!(session.model, None);
        assert!(session.messages.is_empty());
    }

    #[test]
    fn test_add_message() {
        let mut session = Session::new(None, None);
        let initial_updated = session.updated_at;
        
        let message = Message {
            role: "user".to_string(),
            content: Some("Hello".to_string()),
            tool_calls: None,
            tool_call_id: None,
        };
        
        session.add_message(message);
        
        assert_eq!(session.messages.len(), 1);
        assert!(session.updated_at > initial_updated);
        assert_eq!(session.messages[0].role, "user");
        assert_eq!(session.messages[0].content, Some("Hello".to_string()));
    }

    #[test]
    fn test_replace_messages() {
        let mut session = Session::new(None, None);
        let initial_updated = session.updated_at;
        
        // Add initial message
        session.add_message(Message {
            role: "user".to_string(),
            content: Some("Initial".to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
        
        // Replace all messages
        let new_messages = vec![
            Message {
                role: "system".to_string(),
                content: Some("System prompt".to_string()),
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: "user".to_string(),
                content: Some("New message".to_string()),
                tool_calls: None,
                tool_call_id: None,
            },
        ];
        
        session.replace_messages(new_messages);
        
        assert_eq!(session.messages.len(), 2);
        assert!(session.updated_at > initial_updated);
        assert_eq!(session.messages[0].role, "system");
        assert_eq!(session.messages[1].role, "user");
    }

    #[test]
    fn test_set_title() {
        let mut session = Session::new(None, None);
        let initial_updated = session.updated_at;
        
        session.set_title(Some("New Title"));
        
        assert_eq!(session.title, Some("New Title".to_string()));
        assert!(session.updated_at > initial_updated);
    }

    #[test]
    fn test_set_model() {
        let mut session = Session::new(None, None);
        let initial_updated = session.updated_at;
        
        session.set_model(Some("gpt-3.5-turbo"));
        
        assert_eq!(session.model, Some("gpt-3.5-turbo".to_string()));
        assert!(session.updated_at > initial_updated);
    }

    #[test]
    fn test_session_serialization() {
        let mut session = Session::new(Some("Test"), Some("gpt-4"));
        
        session.add_message(Message {
            role: "user".to_string(),
            content: Some("Test message".to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
        
        // Test that session can be serialized to JSON
        let json_str = serde_json::to_string(&session).expect("Failed to serialize session");
        let deserialized: Session = serde_json::from_str(&json_str).expect("Failed to deserialize session");
        
        assert_eq!(session.id, deserialized.id);
        assert_eq!(session.title, deserialized.title);
        assert_eq!(session.model, deserialized.model);
        assert_eq!(session.messages.len(), deserialized.messages.len());
    }

    #[test]
    fn test_multiple_message_operations() {
        let mut session = Session::new(None, None);
        
        // Add multiple messages
        for i in 1..=5 {
            session.add_message(Message {
                role: "user".to_string(),
                content: Some(format!("Message {}", i)),
                tool_calls: None,
                tool_call_id: None,
            });
        }
        
        assert_eq!(session.messages.len(), 5);
        
        // Verify message order
        for (i, message) in session.messages.iter().enumerate() {
            assert_eq!(message.content, Some(format!("Message {}", i + 1)));
        }
    }
}