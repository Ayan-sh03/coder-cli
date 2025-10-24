use crate::types::{FunctionCall, Message, ToolCall};
use serde_json::Value;
use std::io::{self, Write};
use tokio::time::Duration;

#[derive(Clone)]
pub struct LlmClient {
    base_url: String,
    api_key: String,
    model: String,
    http: reqwest::Client,
}

impl LlmClient {
    pub fn new(base_url: String, api_key: String, model: String) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(8)
            .tcp_keepalive(Duration::from_secs(30))
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            base_url,
            api_key,
            model,
            http,
        })
    }

    pub async fn chat_once(&self, messages: &[Message], tools: &Value) -> anyhow::Result<Message> {
        let url = format!("{}/chat/completions", self.base_url);
        let req = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "tools": tools,
            "stream":true
            // "tool_choice": "auto", // optional, if your provider supports it
        });

        let resp = self
            .http
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?;

        // Replace the response parsing in chat_once:
        let mut stream = resp.bytes_stream();
        let mut accumulated_message = Message {
            role: "assistant".to_string(),
            content: Some(String::new()),
            tool_calls: None,
            tool_call_id: None,
        };
        let mut tool_calls_map: std::collections::HashMap<usize, ToolCall> =
            std::collections::HashMap::new();

        use futures::StreamExt;

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            let text = String::from_utf8_lossy(&bytes);

            let mut should_stop = false; // ← add this flag

            for line in text.lines() {
                if !line.starts_with("data: ") {
                    continue;
                }
                let json_str = line.strip_prefix("data: ").unwrap().trim();
                if json_str == "[DONE]" || json_str == "" {
                    should_stop = true; // ← set flag instead of break
                    break;
                }

                let delta: Value = match serde_json::from_str(json_str) {
                    Ok(v) => v,
                    Err(_) => {
                        // eprintln!(
                        //     "Warning: Failed to parse JSON chunk: '{}'. Error: {}",
                        //     json_str, e
                        // );
                        continue; // Skip malformed JSON and continue processing
                    }
                };
                let choice = &delta["choices"][0];
                let delta_obj = &choice["delta"];

                if let Some(finish) = choice["finish_reason"].as_str() {
                    if finish == "stop" || finish == "tool_calls" {
                        should_stop = true; // ← set flag
                        break;
                    }
                }

                // Accumulate content
                if let Some(content) = delta_obj["content"].as_str() {
                    print!("{}", content);
                    io::stdout().flush().unwrap();
                    accumulated_message
                        .content
                        .as_mut()
                        .unwrap()
                        .push_str(content);
                }

                // Accumulate tool_calls (indexed deltas)
                if let Some(tool_calls_arr) = delta_obj["tool_calls"].as_array() {
                    for tc_delta in tool_calls_arr {
                        let index = tc_delta["index"].as_u64().unwrap() as usize;
                        let entry = tool_calls_map.entry(index).or_insert_with(|| ToolCall {
                            id: String::new(),
                            call_type: "function".to_string(),
                            function: FunctionCall {
                                name: String::new(),
                                arguments: String::new(),
                            },
                        });

                        if let Some(id) = tc_delta["id"].as_str() {
                            entry.id = id.to_string();
                        }
                        if let Some(name) = tc_delta["function"]["name"].as_str() {
                            entry.function.name = name.to_string();
                        }
                        if let Some(args) = tc_delta["function"]["arguments"].as_str() {
                            entry.function.arguments.push_str(args);
                        }
                    }
                }
            }

            if should_stop {
                // ← break outer loop
                break;
            }
        }

        // Reconstruct tool_calls vector from map if any
        if !tool_calls_map.is_empty() {
            let mut calls: Vec<_> = tool_calls_map.into_iter().collect();
            calls.sort_by_key(|(idx, _)| *idx);
            accumulated_message.tool_calls = Some(calls.into_iter().map(|(_, tc)| tc).collect());
        }

        Ok(accumulated_message)
    }

    pub async fn chat_once_no_stream(
        &self,
        messages: &[Message],
        // tools: &Value,
    ) -> anyhow::Result<Message> {
        let url = format!("{}/chat/completions", self.base_url);
        let req = serde_json::json!({
            "model": self.model,
            "messages": messages,
            // "tools": tools,
            "stream": false
            // "tool_choice": "auto", // optional, if your provider supports it
        });

        let resp = self
            .http
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?;

        // Parse non-streaming response
        let response_text = resp.text().await?;
        let response_json: Value = serde_json::from_str(&response_text)
            .map_err(|e| anyhow::anyhow!("Failed to parse JSON response: {}", e))?;

        // Extract the message from the response
        let choice = response_json["choices"]
            .as_array()
            .and_then(|choices| choices.first())
            .ok_or_else(|| anyhow::anyhow!("No choices in response"))?;

        let message = &choice["message"];

        // // Parse tool calls if present
        // let tool_calls = if let Some(tool_calls_array) = message["tool_calls"].as_array() {
        //     Some(
        //         tool_calls_array
        //             .iter()
        //             .map(|tc| ToolCall {
        //                 id: tc["id"].as_str().unwrap_or("").to_string(),
        //                 call_type: tc["type"].as_str().unwrap_or("function").to_string(),
        //                 function: FunctionCall {
        //                     name: tc["function"]["name"].as_str().unwrap_or("").to_string(),
        //                     arguments: tc["function"]["arguments"]
        //                         .as_str()
        //                         .unwrap_or("")
        //                         .to_string(),
        //                 },
        //             })
        //             .collect(),
        //     )
        // } else {
        //     None
        // };

        Ok(Message {
            role: message["role"].as_str().unwrap_or("assistant").to_string(),
            content: message["content"].as_str().map(|s| s.to_string()),
            tool_calls: None,
            tool_call_id: None,
        })
    }
}
