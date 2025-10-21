use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::io::{self, Write};
use tokio::time::{Duration, timeout};

mod tools;

// ------------------ Types compatible with OpenAI-style API ------------------

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
    arguments: String, // raw JSON string
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Message {
    role: String, // "user" | "assistant" | "tool" | "system"
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

// ----------------------------- Tool Registry --------------------------------

#[derive(Clone)]
struct ToolRegistry {
    schemas: Value,
}

impl ToolRegistry {
    fn new() -> Self {
        // Single source of truth for "tools" schema the LLM sees
        let schemas = serde_json::json!([
            {
                "type": "function",
                "function": {
                    "name": "list_dir",
                    "description":
                        "Lists all files and directories in the given path",
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
                    "description":
                        "Returns the content of the file for the given path",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file"
                            },
                            "start_line": {
                                "type": "number",
                                "description":
                                    "Starting line (optional, default 1)"
                            },
                            "end_line": {
                                "type": "number",
                                "description":
                                    "Ending line (optional, default start+200)"
                            }
                        },
                        "required": ["path"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "run_shell",
                    "description":
                        "Executes a shell command with a 30-second timeout. \
                         Dangerous commands like rm, sudo, dd are blocked.",
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
            },
            {
                "type": "function",
                "function": {
                    "name": "write_file",
                    "description":
                        "Writes content to a file. Creates a file if absent.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path of file to write"
                            },
                            "content": {
                                "type": "string",
                                "description":
                                    "Content to write into the file"
                            }
                        },
                        "required": ["path", "content"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "search_in_files",
                    "description":
                        "Recursive search for a regex pattern with \
                         gitignore-style filtering.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "string",
                                "description":
                                    "Regular expression (Rust syntax)"
                            },
                            "path": {
                                "type": "string",
                                "description":
                                    "File or directory to search"
                            },
                            "case_sensitive": {
                                "type": "boolean",
                                "description":
                                    "Case-sensitive match (default true)"
                            }
                        },
                        "required": ["pattern", "path"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "edit_file",
                    "description":
                        "Edits a file by replacing an existing string.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file"
                            },
                            "old_str": {
                                "type": "string",
                                "description": "String to be replaced"
                            },
                            "new_str": {
                                "type": "string",
                                "description": "Replacement string"
                            }
                        },
                        "required": ["path", "old_str", "new_str"]
                    }
                }
            }
        ]);
        Self { schemas }
    }

    fn schemas(&self) -> &Value {
        &self.schemas
    }
}

// ---------------------------- LLM HTTP Client -------------------------------

#[derive(Clone)]
struct LlmClient {
    base_url: String,
    api_key: String,
    model: String,
    http: reqwest::Client,
}

impl LlmClient {
    fn new(base_url: String, api_key: String, model: String) -> anyhow::Result<Self> {
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

    async fn chat_once(&self, messages: &[Message], tools: &Value) -> anyhow::Result<Message> {
        let url = format!("{}/chat/completions", self.base_url);
        let req = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "tools": tools,
            // "tool_choice": "auto", // optional, if your provider supports it
        });

        let resp = self
            .http
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?;
        let status = resp.status();
        let body: Value = resp.json().await?;
        if !status.is_success() {
            anyhow::bail!(
                "LLM error ({}): {}",
                status,
                body.get("error").unwrap_or(&body)
            );
        }

        let msg_val = &body["choices"][0]["message"];
        let parsed: Message = serde_json::from_value(msg_val.clone())?;
        Ok(parsed)
    }
}

// ------------------------------- Agent Loop ---------------------------------

#[derive(Clone)]
struct AgentOptions {
    max_steps: usize,
    yolo: bool, // auto-approve tools
    step_timeout: Duration,
    observation_clip: usize, // chars per tool output
}

struct Agent {
    llm: LlmClient,
    tools: ToolRegistry,
    opts: AgentOptions,
}

impl Agent {
    fn new(llm: LlmClient, tools: ToolRegistry, opts: AgentOptions) -> Self {
        Self { llm, tools, opts }
    }

    // Compact older messages to keep context light. We do a simple heuristic:
    // - Keep the last N messages untouched.
    // - For older "tool" messages, clip content to a budget.
    fn compact_history(&self, msgs: &mut Vec<Message>) {
        // Example heuristic: clip any tool message content longer than budget.
        for m in msgs.iter_mut() {
            if m.role == "tool" {
                if let Some(c) = &m.content {
                    if c.len() > self.opts.observation_clip {
                        let head = &c[..self.opts.observation_clip];
                        m.content = Some(format!("{}… [truncated]", head));
                    }
                }
            }
        }
        // You can also drop very old messages if they exceed some count/size.
    }

    async fn run_turn(&self, messages: &mut Vec<Message>) -> anyhow::Result<Option<String>> {
        self.compact_history(messages);

        // Single LLM step
        let llm_step = timeout(
            self.opts.step_timeout,
            self.llm.chat_once(messages, self.tools.schemas()),
        )
        .await??;

        // Record assistant step
        messages.push(llm_step.clone());
        if let Some(tcs) = &llm_step.tool_calls {
            for tc in tcs {
                let pretty_args = serde_json::from_str::<serde_json::Value>(&tc.function.arguments)
                    .ok()
                    .and_then(|v| serde_json::to_string(&v).ok())
                    .unwrap_or_else(|| tc.function.arguments.clone());

                println!(
                    "\u{001b}[35m▌🔧 {} ({})\u{001b}[0m",
                    tc.function.name, pretty_args
                );
            }
        }

        // If no tool calls: either final content or a no-op; return final if any
        if llm_step.tool_calls.is_none() {
            if let Some(text) = llm_step.content.clone() {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    return Ok(Some(trimmed));
                }
            }
            return Ok(None);
        }

        // Tool calls present: execute them (in parallel if independent)
        let tool_calls = llm_step.tool_calls.clone().unwrap();
        let mut tasks = vec![];

        for tool_call in tool_calls {
            let name = tool_call.function.name.clone();
            let id = tool_call.id.clone();
            let args_raw = tool_call.function.arguments.clone();
            let yolo = self.opts.yolo;

            tasks.push(tokio::spawn(async move {
                // Approval (synchronous user prompt) unless YOLO
                if !yolo && tools::requires_approval(&name) {
                    let approval_prompt = tools::format_tool_approval();
                    print!("{}", approval_prompt);
                    let _ = io::stdout().flush();

                    match tools::get_user_approval("Proceed") {
                        Ok(true) => {
                            println!("\u{001b}[92m✓ Approved\u{001b}[0m");
                        }
                        Ok(false) => {
                            println!("\u{001b}[91m✗ Denied by user\u{001b}[0m");
                            return Ok::<(String, String), anyhow::Error>((
                                id,
                                "User denied execution".to_string(),
                            ));
                        }
                        Err(e) => {
                            eprintln!("Approval error: {}", e);
                            return Ok::<(String, String), anyhow::Error>((
                                id,
                                format!("Error: {}", e),
                            ));
                        }
                    }
                }

                // Parse args safely
                let args: Value = match serde_json::from_str(&args_raw) {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok::<(String, String), anyhow::Error>((
                            id,
                            format!("Error parsing args: {}", e),
                        ));
                    }
                };

                // Dispatch
                let obs = match name.as_str() {
                    "list_dir" => {
                        let path = args["path"].as_str().unwrap_or(".");
                        let list = tools::list_dir(path);
                        if list.is_empty() {
                            "Directory is empty".to_string()
                        } else {
                            list.join("\n")
                        }
                    }
                    "read_file" => {
                        let path = args["path"].as_str().unwrap_or("");
                        let start = args
                            .get("start_line")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as usize);
                        let end = args
                            .get("end_line")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as usize);
                        tools::read_file(path, start, end)
                            .unwrap_or_else(|e| format!("Error: {}", e))
                    }
                    "write_file" => {
                        let path = args["path"].as_str().unwrap_or("");
                        let content = args["content"].as_str().unwrap_or("");
                        tools::write_file(path, content).unwrap_or_else(|e| format!("Error: {}", e))
                    }
                    "run_shell" => {
                        let cmd = args["command"].as_str().unwrap_or("");
                        tools::run_shell(cmd).unwrap_or_else(|e| format!("Error: {}", e))
                    }
                    "search_in_files" => {
                        let path = args["path"].as_str().unwrap_or(".");
                        let case_sensitive = args.get("case_sensitive").and_then(|v| v.as_bool());
                        let pattern = args["pattern"].as_str().unwrap_or("");
                        tools::search_in_files(pattern, path, case_sensitive)
                            .unwrap_or_else(|e| format!("Error: {}", e))
                    }
                    "edit_file" => {
                        let path = args["path"].as_str().unwrap_or("");
                        let old_str = args["old_str"].as_str().unwrap_or("");
                        let new_str = args["new_str"].as_str().unwrap_or("");
                        tools::edit_file(path, old_str, new_str)
                            .unwrap_or_else(|e| format!("Error: {}", e))
                    }
                    _ => "Error: unknown tool".to_string(),
                };

                Ok::<(String, String), anyhow::Error>((id, obs))
            }));
        }

        // Gather results and append as tool messages (Observations)
        for t in tasks {
            match t.await {
                Ok(Ok((tool_call_id, observation))) => {
                    // Clip observation to keep context small
                    let clipped = clip(&observation, self.opts.observation_clip);
                    messages.push(Message {
                        role: "tool".to_string(),
                        content: Some(clipped),
                        tool_calls: None,
                        tool_call_id: Some(tool_call_id),
                    });
                }
                Ok(Err(e)) => {
                    messages.push(Message {
                        role: "tool".to_string(),
                        content: Some(format!("Error: {}", e)),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                Err(join_err) => {
                    messages.push(Message {
                        role: "tool".to_string(),
                        content: Some(format!("Join error: {}", join_err)),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
            }
        }

        // After appending Observations, we do not return a final answer yet.
        // The caller will call run_turn again, which lets the LLM continue.
        Ok(None)
    }

    async fn run_agent_loop(
        &self,
        initial_user_input: String,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        // Seed with user input
        messages.push(Message {
            role: "user".into(),
            content: Some(initial_user_input),
            tool_calls: None,
            tool_call_id: None,
        });

        for step in 0..self.opts.max_steps {
            let final_text = self.run_turn(messages).await?;
            if let Some(output) = final_text {
                println!("{}", output);
                return Ok(());
            }
            // If run_turn returned None, it means tools were called and
            // Observations appended. Continue the loop to let LLM react.
            if step + 1 == self.opts.max_steps {
                // If we reach max steps without final text, summarize last turn
                println!("(Reached step limit without final answer.)");
            }
        }
        Ok(())
    }
}

// ------------------------------- Utilities ----------------------------------

fn clip(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut out = s[..max].to_string();
    out.push_str("… [truncated]");
    out
}

// ----------------------------------- Main -----------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    create_agent_dir();

    println!("\u{001b}[94mWelcome to the Rust ReAct agent!\u{001b}[0m");

    // Environment
    let base_url = env::var("OPENAI_BASE_URL").expect("OPENAI_BASE_URL not set");
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| {
        // Your original used "glm-4.5-air"; keep configurable
        "glm-4.5-air".to_string()
    });

    let llm = LlmClient::new(base_url, api_key, model)?;
    let tools = ToolRegistry::new();
    let opts = AgentOptions {
        max_steps: 12,
        yolo: false, // set true to auto-approve tool calls
        step_timeout: Duration::from_secs(45),
        observation_clip: 4000, // keep large enough for code blocks
    };
    let agent = Agent::new(llm, tools, opts);

    let mut messages: Vec<Message> = vec![Message {
        role: "system".to_string(),
        content: Some(
            "You are a precise, terse coding agent. Use tools \
            whenever needed. Prefer minimal terminal output. Produce a final \
            answer when you have enough context; otherwise call appropriate \
            tools. Never fabricate file contents."
                .to_string(),
        ),
        tool_calls: None,
        tool_call_id: None,
    }];

    loop {
        print!("\u{001b}[93mYou:\u{001b}[0m ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            eprintln!("Failed to read input.");
            continue;
        }

        let trimmed = input.trim();
        if trimmed.eq_ignore_ascii_case("quit") {
            println!("Goodbye!");
            break;
        } else if trimmed.eq_ignore_ascii_case("help") {
            println!("Type a task. Type 'quit' to exit.");
            continue;
        }

        print!("\u{001b}[96mAgent:\u{001b}[0m ");
        io::stdout().flush().unwrap();

        if let Err(e) = agent
            .run_agent_loop(trimmed.to_string(), &mut messages)
            .await
        {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}

fn create_agent_dir() {
    if let Err(err) = std::fs::create_dir(".coder") {
        if err.kind() != std::io::ErrorKind::AlreadyExists {
            eprintln!("Error creating .coder: {}", err);
        }
    }
}
