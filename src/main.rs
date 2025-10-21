use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{self, Write};

mod tools;
#[derive(Serialize, Deserialize, Clone, Debug)]
struct ToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String, // "function"
    function: FunctionCall,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct FunctionCall {
    name: String,
    arguments: String, // JSON string
}
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Message {
    role: String, // "user", "assistant","tool" or "system"
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

fn create_agent_dir() {
    match fs::create_dir(".coder") {
        Ok(_) => (),
        Err(err) => {
            //suppress already exists error
            if err.kind() != std::io::ErrorKind::AlreadyExists {
                println!("Error creating directory: {}", err);
            }
        }
    }
}

fn run_openai_inference(input: Option<&str>, messages: &mut Vec<Message>) -> Result<(), String> {
    //get OPENAI_BASE_URL frmo env

    let tools = serde_json::json!([
        {
        "type": "function",
        "function": {
            "name": "list_dir",
            "description": "Lists all files and directories in the given path",
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
    },{
        "type": "function",
        "function": {
            "name": "read_file",
            "description": "returns the content of the file given the path",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path to list"
                    },
                    "start_line": { "type": "number", "description": "Starting line (optional, default 1)" },
                    "end_line": { "type": "number", "description": "Ending line (optional, default start+200)" }
                },
                "required": ["path"]
            }
        }
    },{
        "type": "function",
        "function": {
            "name": "run_shell",
            "description": "Executes a shell command with a 30-second timeout. Dangerous commands like rm, sudo, dd are blocked.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }
        }
    },{
        "type": "function",
        "function": {
            "name": "write_file",
            "description": "Writes content to a file. Creates new file if it doesn't exist.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }
        }
    }, {
      "type": "function",
      "function": {
        "name": "search_in_files",
        "description": "Search a file or directory tree for a regular-expression pattern; returns file:line:match with git-ignore style filtering.",
        "parameters": {
          "type": "object",
          "properties": {
            "pattern": { "type": "string", "description": "Regular-expression pattern (Rust-syntax) to search for" },
            "path": { "type": "string", "description": "Path to file or directory to search; directory search is recursive" },
            "case_sensitive": { "type": "boolean", "description": "If false performs case-insensitive matching; default match is case-sensitive" }
          },
          "required": ["pattern", "path"]
        }
      }
    },
    {
      "type": "function",
      "function": {
        "name": "edit_file",
        "description": "Edits a file by replacing an existing string with a new string.",
        "parameters": {
          "type": "object",
          "properties": {
            "path": {
              "type": "string",
              "description": "The path to the file to edit."
            },
            "old_str": {
              "type": "string",
              "description": "The string to be replaced."
            },
            "new_str": {
              "type": "string",
              "description": "The new string to replace with."
            }
          },
          "required": [
            "path",
            "old_str",
            "new_str"
          ]
        }
      }
    }
    ]);
    let openai_base_url = env::var("OPENAI_BASE_URL").expect("OPENAI_BASE_URL not set");
    let openai_api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    if let Some(inp) = input {
        messages.push(Message {
            role: "user".to_string(),
            content: Some(inp.to_string()),
            tool_call_id: None,
            tool_calls: None,
        });
    }
    let client = reqwest::blocking::Client::new();

    let response = client
        .post(format!("{}/chat/completions", openai_base_url))
        .header("Authorization", format!("Bearer {}", openai_api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model":"glm-4.5-air",
            "messages": messages,
            "tools":tools
        }))
        .send()
        .map_err(|e| format!("Failed to send request: {}", e))?;

    let response_json: serde_json::Value = response
        .json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // Check for API error
    if let Some(error) = response_json.get("error") {
        return Err(format!("API error: {}", error));
    }

    let assistant_message = &response_json["choices"][0]["message"];

    let parsed_message: Message = serde_json::from_value(assistant_message.clone())
        .map_err(|e| format!("Failed to parse message: {}", e))?;

    messages.push(parsed_message.clone());

    // if let Some(content) = &parsed_message.content {
    //     println!("{}", content);
    // }
    if let Some(tool_calls) = &parsed_message.tool_calls {
        for tool_call in tool_calls {
            let args: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                .unwrap_or(serde_json::json!({}));

            println!(
                "\u{001b}[35m▌🔧 {} ({})\u{001b}[0m",
                tool_call.function.name, args
            );
        }
    }

    if parsed_message.tool_calls.is_none() {
        if let Some(content) = &parsed_message.content {
            if !content.trim().is_empty() {
                println!("{}", content);
            }
        }
    }
    Ok(())
}

fn main() {
    create_agent_dir();
    println!("\u{001b}[94mHello welcome to coder cli !\u{001b}[0m");
    let mut messages: Vec<Message> = Vec::new();

    messages.push(Message {
        role: "system".to_string(),
        content: Some("You are a helpful coding assistant.".to_string()),
        tool_call_id: None,
        tool_calls: None,
    });

    loop {
        print!("\u{001b}[93mYou:\u{001b}[0m ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        match input.trim() {
            "quit" => {
                println!("{:#?}", messages);
                break;
            }
            "help" => println!("Welcome to Coder"),
            _ => {
                print!("\u{001b}[96mAgent:\u{001b}[0m ");
                if let Err(e) = run_openai_inference(Some(&input), &mut messages) {
                    eprintln!("Error: {}", e);
                }
                for _ in 0..5 {
                    let has_tool_calls = if let Some(last_msg) = messages.last() {
                        last_msg.tool_calls.is_some()
                    } else {
                        false
                    };

                    if !has_tool_calls {
                        break;
                    }
                    //if last message is tool call , run oai inference with tool handling
                    if let Some(last_msg) = &messages.last() {
                        if let Some(tool_calls) = &last_msg.tool_calls {
                            let tool_calls = tool_calls.clone();
                            for tool_call in tool_calls {
                                let tool_name = &tool_call.function.name;
                                let tool_args = &tool_call.function.arguments;

                                let args: serde_json::Value = serde_json::from_str(tool_args)
                                    .expect("Failed to parse Arguments!");

                                if tools::requires_approval(tool_name) {
                                    let approval_prompt = tools::format_tool_approval();
                                    print!("{}", approval_prompt);

                                    match tools::get_user_approval("Proceed") {
                                        Ok(true) => println!("\u{001b}[92m✓ Approved\u{001b}[0m"),
                                        Ok(false) => {
                                            println!("\u{001b}[91m✗ Denied by user\u{001b}[0m");
                                            // Add denial message to conversation
                                            messages.push(Message {
                                                role: "tool".to_string(),
                                                content: Some("User denied execution".to_string()),
                                                tool_calls: None,
                                                tool_call_id: Some(tool_call.id.clone()),
                                            });
                                            continue; // Skip this tool
                                        }
                                        Err(e) => {
                                            eprintln!("Error: {}", e);
                                            continue;
                                        }
                                    }
                                }
                                let result = match tool_name.as_str() {
                                    "list_dir" => {
                                        let path = args["path"].as_str().unwrap();
                                        let entries = tools::list_dir(path);
                                        if entries.is_empty() {
                                            "Directory is empty".to_string()
                                        } else {
                                            entries.join("\n")
                                        }
                                    }
                                    "read_file" => {
                                        let path = args["path"].as_str().unwrap();
                                        let start_line = args
                                            .get("start_line")
                                            .and_then(|v| v.as_u64())
                                            .map(|n| n as usize);
                                        let end_line = args
                                            .get("end_line")
                                            .and_then(|v| v.as_u64())
                                            .map(|n| n as usize);

                                        tools::read_file(path, start_line, end_line)
                                            .unwrap_or_else(|e| format!("Error: {}", e))
                                    }
                                    "write_file" => {
                                        let path = args["path"].as_str().unwrap();
                                        let content = args["content"].as_str().unwrap();

                                        tools::write_file(path, content)
                                            .unwrap_or_else(|e| format!("Error: {}", e))
                                    }
                                    "run_shell" => {
                                        let command = args["command"].as_str().unwrap();
                                        tools::run_shell(command)
                                            .unwrap_or_else(|e| format!("Error: {}", e))
                                    }
                                    "search_in_files" => {
                                        let path = args["path"].as_str().unwrap();
                                        let case_sensitive =
                                            args.get("case_sensitive").and_then(|v| v.as_bool());
                                        let pattern = args["pattern"].as_str().unwrap();

                                        tools::search_in_files(pattern, path, case_sensitive)
                                            .unwrap_or_else(|e| format!("Error: {}", e))
                                    }
                                    "edit_file" => {
                                        let path = args["path"].as_str().unwrap();
                                        let old_str = args["old_str"].as_str().unwrap();
                                        let new_str = args["new_str"].as_str().unwrap();

                                        tools::edit_file(path, old_str, new_str)
                                            .unwrap_or_else(|e| format!("Error: {}", e))
                                    }
                                    _ => "Unknown Tool".to_string(),
                                };

                                //add tool response back to message
                                messages.push(Message {
                                    role: "tool".to_string(),
                                    content: Some(result.to_string()),
                                    tool_calls: None,
                                    tool_call_id: Some(tool_call.id.clone()),
                                });
                            }
                        }
                    }

                    if let Err(e) = run_openai_inference(None, &mut messages) {
                        eprintln!("Error: {}", e);
                    }
                }
            }
        }
    }
}
