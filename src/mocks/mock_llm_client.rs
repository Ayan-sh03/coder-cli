use crate::types::{Message, ToolCall, FunctionCall};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[async_trait]
pub trait LlmClientTrait {
    async fn chat_once(&self, messages: &[Message], tools: &Value) -> Result<Message>;
}

#[derive(Clone)]
pub struct MockLlmClient {
    responses: Arc<Mutex<Vec<Message>>>,
    call_history: Arc<Mutex<Vec<Vec<Message>>>>,
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
            call_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_text_response(&mut self, content: &str) {
        let response = Message {
            role: "assistant".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
        };
        self.responses.lock().unwrap().push(response);
    }

    pub fn add_tool_call_response(&mut self, tool_name: &str, args: &str) {
        let tool_call = ToolCall {
            id: "test-call-123".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: tool_name.to_string(),
                arguments: args.to_string(),
            },
        };

        let response = Message {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(vec![tool_call]),
            tool_call_id: None,
        };
        self.responses.lock().unwrap().push(response);
    }

    pub fn add_error_response(&mut self, error_msg: &str) {
        let response = Message {
            role: "assistant".to_string(),
            content: Some(format!("Error: {}", error_msg)),
            tool_calls: None,
            tool_call_id: None,
        };
        self.responses.lock().unwrap().push(response);
    }

    pub fn get_call_history(&self) -> Vec<Vec<Message>> {
        self.call_history.lock().unwrap().clone()
    }

    pub fn clear_responses(&mut self) {
        self.responses.lock().unwrap().clear();
        self.call_history.lock().unwrap().clear();
    }

    fn pop_response(&self) -> Option<Message> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            Some(Message {
                role: "assistant".to_string(),
                content: Some("No more mock responses configured".to_string()),
                tool_calls: None,
                tool_call_id: None,
            })
        } else {
            Some(responses.remove(0))
        }
    }
}

// Implement the same interface as real LlmClient for Agent
impl MockLlmClient {
    pub async fn chat_once(&self, messages: &[Message], _tools: &Value) -> Result<Message> {
        // Store the call for verification
        self.call_history.lock().unwrap().push(messages.to_vec());
        
        // Return the next configured response
        self.pop_response()
            .ok_or_else(|| anyhow::anyhow!("No mock response available"))
    }
}

#[async_trait]
impl crate::agent::LlmClientTrait for MockLlmClient {
    async fn chat_once(&self, messages: &[Message], tools: &Value) -> Result<Message> {
        self.chat_once(messages, tools).await
    }
}