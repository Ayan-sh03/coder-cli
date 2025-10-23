use crate::llm_client::LlmClient;
use std::env;
pub fn ask_orackle(query: &str) -> Result<String, String> {
    let openai_base_url =
    let llm = LlmClient::new(base_url, api_key, model)
}
