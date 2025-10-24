use crate::llm_client::LlmClient;
use crate::types::Message;
use std::env;

pub async fn ask_orackle(query: &str) -> Result<String, String> {
    let base_url = env::var("OPENAI_BASE_URL").expect("OPENAI_BASE_URL not set");
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "glm-4.5-air".to_string());

    // Create LLM client for orackle
    let llm = match LlmClient::new(base_url, api_key, model) {
        Ok(client) => client,
        Err(e) => return Err(format!("Failed to create LLM client: {}", e)),
    };

    // Create system message for orackle - it's a specialized agent for providing insights
    let system_message = Message {
        role: "system".to_string(),
        content: Some(
            "You are Orackle, an expert coding assistant agent that provides insights and guidance to solve complex problems.
            You have deep knowledge of software engineering, debugging, system architecture, and problem-solving strategies.

            Your role is to:
            1. Analyze the problem description thoroughly
            2. Identify the core issue or bottleneck
            3. Provide strategic insights and alternative approaches
            4. Suggest specific, actionable solutions
            5. Highlight potential pitfalls and how to avoid them

            You are READ-ONLY - you cannot modify files or execute commands. Focus on analysis and guidance.
            Be concise but thorough. Provide step-by-step reasoning when helpful."
                .to_string(),
        ),
        tool_calls: None,
        tool_call_id: None,
    };

    let user_message = Message {
        role: "user".to_string(),
        content: Some(format!(
            "Main agent is stuck with this problem and needs insights:\n\n{}",
            query
        )),
        tool_calls: None,
        tool_call_id: None,
    };

    let messages = vec![system_message, user_message];

    // Define available tools for orackle (read-only tools)
    let tools = serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the content of a file to understand the codebase",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to read"
                        },
                        "start_line": {
                            "type": "number",
                            "description": "Starting line number (optional)"
                        },
                        "end_line": {
                            "type": "number",
                            "description": "Ending line number (optional)"
                        }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_dir",
                "description": "List the contents of a directory to understand project structure",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path to list"
                        }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "search_in_files",
                "description": "Search for patterns in the codebase to understand the problem",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Regular expression pattern to search for"
                        },
                        "path": {
                            "type": "string",
                            "description": "Path to search in"
                        },
                        "case_sensitive": {
                            "type": "boolean",
                            "description": "Case sensitive search"
                        }
                    },
                    "required": ["pattern", "path"]
                }
            }
        }
    ]);

    // Make the LLM call directly (already in async context)
    let response = match llm.chat_once_no_stream(&messages).await {
        Ok(response) => response,
        Err(e) => return Err(format!("LLM call failed: {}", e)),
    };

    // Extract the content from the response
    match response.content {
        Some(insights) => Ok(insights),
        None => {
            // If no content but there are tool calls, we need to execute them
            Ok("Orackle: No insights available.".to_string())
            // if let Some(tool_calls) = response.tool_calls {
            //     let mut accumulated_insights = String::new();

            //     for tool_call in tool_calls {
            //         match tool_call.function.name.as_str() {
            //             "read_file" => {
            //                 if let Ok(args) =
            //                     serde_json::from_str::<Value>(&tool_call.function.arguments)
            //                 {
            //                     if let Some(path) = args["path"].as_str() {
            //                         match crate::tools::read_file(path, None, None) {
            //                             Ok(content) => {
            //                                 accumulated_insights.push_str(&format!(
            //                                     "\n--- File: {} ---\n{}\n",
            //                                     path, content
            //                                 ));
            //                             }
            //                             Err(e) => {
            //                                 accumulated_insights.push_str(&format!(
            //                                     "\n--- Error reading {}: {} ---\n",
            //                                     path, e
            //                                 ));
            //                             }
            //                         }
            //                     }
            //                 }
            //             }
            //             "list_dir" => {
            //                 if let Ok(args) =
            //                     serde_json::from_str::<Value>(&tool_call.function.arguments)
            //                 {
            //                     if let Some(path) = args["path"].as_str() {
            //                         let contents = crate::tools::list_dir(path);
            //                         accumulated_insights.push_str(&format!(
            //                             "\n--- Directory: {} ---\n{}\n",
            //                             path,
            //                             contents.join("\n")
            //                         ));
            //                     }
            //                 }
            //             }
            //             "search_in_files" => {
            //                 if let Ok(args) =
            //                     serde_json::from_str::<Value>(&tool_call.function.arguments)
            //                 {
            //                     if let (Some(pattern), Some(path)) =
            //                         (args["pattern"].as_str(), args["path"].as_str())
            //                     {
            //                         let case_sensitive =
            //                             args.get("case_sensitive").and_then(|v| v.as_bool());
            //                         match crate::tools::search_in_files(
            //                             pattern,
            //                             path,
            //                             case_sensitive,
            //                         ) {
            //                             Ok(results) => {
            //                                 accumulated_insights.push_str(&format!(
            //                                     "\n--- Search: {} in {} ---\n{}\n",
            //                                     pattern, path, results
            //                                 ));
            //                             }
            //                             Err(e) => {
            //                                 accumulated_insights.push_str(&format!(
            //                                     "\n--- Search failed: {} ---\n",
            //                                     e
            //                                 ));
            //                             }
            //                         }
            //                     }
            //                 }
            //             }
            //             _ => {
            //                 accumulated_insights.push_str(&format!(
            //                     "\n--- Unknown tool: {} ---\n",
            //                     tool_call.function.name
            //                 ));
            //             }
            //         }
            //     }

            //     if !accumulated_insights.is_empty() {
            //         Ok(format!("Orackle insights:\n{}", accumulated_insights))
            //     } else {
            //         Ok(
            //             "Orackle: Unable to provide insights due to tool execution failures."
            //                 .to_string(),
            //         )
            //     }
            // } else {
            //     Ok("Orackle: No insights available.".to_string())
            // }
        }
    }
}
