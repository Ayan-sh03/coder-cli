use chrono::Utc;
use uuid::Uuid;

use crate::types::Message;
pub use crate::types::Session;

impl Session {
    pub fn new(title: Option<&str>, model: Option<&str>) -> Session {
        Session {
            id: Uuid::new_v4().to_string(),
            messages: Vec::<Message>::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            title: title.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
        }
    }

    // Replace all messages
    pub fn replace_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
        self.updated_at = Utc::now();
    }

    // Append one message
    pub fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
        self.updated_at = Utc::now();
    }

    pub fn set_title(&mut self, title: Option<&str>) {
        self.title = title.map(|s| s.to_string());
        self.updated_at = Utc::now();
    }

    pub fn set_model(&mut self, model: Option<&str>) {
        self.model = model.map(|s| s.to_string());
        self.updated_at = Utc::now();
    }
}
