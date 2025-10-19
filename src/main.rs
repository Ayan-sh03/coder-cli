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
    },
        {
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

    if let Some(content) = &parsed_message.content {
        println!("{}", content);
    }
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
                                            .unwrap_or_else(|e| format! {"Error: {}"}, e)
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
