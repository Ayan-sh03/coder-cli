mod agent;
mod llm_client;
mod session;
mod tool_registry;
mod tools;
mod types;
mod utils;
mod ui;

#[cfg(test)]
mod mocks;
#[cfg(test)]
mod tests;

use agent::{Agent, AgentOptions};
use llm_client::LlmClient;
use session::Session;
use std::env;
use std::io::{self, Write};
use tool_registry::ToolRegistry;
use types::Message;

// ----------------------------------- Main -----------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    create_agent_dir();

    println!("\u{001b}[94m🚀 Welcome to the Rust ReAct Agent!\u{001b}[0m");

    // Environment
    let base_url = env::var("OPENAI_BASE_URL").expect("OPENAI_BASE_URL not set");
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "glm-4.5-air".to_string());

    let llm = LlmClient::new(base_url, api_key, model.clone())?;
    let tools = ToolRegistry::new();
    let opts = AgentOptions {
        max_steps: 12,
        yolo: false,
        step_timeout: tokio::time::Duration::from_secs(45),
        observation_clip: 4000,
    };
    let agent = Agent::with_real_client(llm, tools, opts);

    // Create session with system message
    let mut session = Session::new(Some("Coding Session"), Some(&model));
    session.add_message(Message {
        role: "system".to_string(),
        content: Some(
            "You are an advanced coding assistant with expert-level reasoning capabilities.

        ## CORE PRINCIPLES
        1. **Think Before Acting**: Always analyze the task thoroughly before using tools
        2. **Plan & Decompose**: Break complex tasks into clear, sequential steps
        3. **Verify Results**: Double-check your work before presenting final answers
        4. **Learn & Adapt**: Use feedback to improve your approach
        5. **Be Efficient**: Use tools in parallel when possible, avoid redundant operations

        ## TASK EXECUTION STRATEGY
        1. **Understand**: Clarify the user's goal and constraints
        2. **Plan**: Outline the steps needed to complete the task
        3. **Execute**: Use tools systematically and efficiently
        4. **Verify**: Test and validate your implementation
        5. **Summarize**: Provide clear explanation of what was accomplished

        ## TOOL USAGE GUIDELINES
        - **read_file**: Gather context before making changes
        - **list_dir**: Understand project structure
        - **search_in_files**: Find relevant code patterns
        - **edit_file/insert_in_file**: Make precise, targeted changes
        - **write_file**: Create new files with proper structure
        - **run_shell**: Execute commands when necessary

        ## QUALITY STANDARDS
        - Never fabricate file contents or code
        - Ensure code is syntactically correct and follows conventions
        - Test your changes when possible
        - Provide clear explanations of your approach
        - Ask for clarification if the task is ambiguous

        ## COMMUNICATION STYLE
        - Be concise but thorough in your explanations
        - Show your reasoning process for complex tasks
        - Highlight important changes or decisions
        - Provide context for why certain approaches were chosen

        Remember: Your goal is to deliver high-quality, working solutions while being transparent about your process."
                .to_string(),
        ),
        tool_calls: None,
        tool_call_id: None,
    });

    println!("\u{001b}[94m✨ Launching TUI...\u{001b}[0m\n");

    // Start TUI with integrated agent loop
    if let Err(e) = run_tui_with_agent(agent, session).await {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

async fn run_tui_with_agent(agent: Agent, mut session: Session) -> anyhow::Result<()> {
    // Create UI and get communication channels
    let (mut app, ui_tx) = ui::TuiApp::new();
    
    // Load initial messages (skip system message for display)
    for msg in session.messages.iter() {
        if msg.role != "system" {
            if let Some(content) = &msg.content {
                app.messages.push(ui::DisplayMessage {
                    role: msg.role.clone(),
                    content: content.clone(),
                    timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                });
            }
        }
    }

    // Create channel for user inputs
    let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    // Clone for agent task
    let agent_ui_tx = ui_tx.clone();
    let agent_task = tokio::spawn(async move {
        while let Some(user_input) = input_rx.recv().await {
            let _ = agent_ui_tx.send(ui::UiEvent::StatusUpdate("🤔 Thinking...".to_string()));
            
            // Add user message to session
            session.add_message(Message {
                role: "user".to_string(),
                content: Some(user_input.clone()),
                tool_calls: None,
                tool_call_id: None,
            });

            // Run agent loop with streaming
            match run_agent_turn_with_ui(&agent, &mut session, &agent_ui_tx).await {
                Ok(_) => {
                    let _ = agent_ui_tx.send(ui::UiEvent::Complete);
                }
                Err(e) => {
                    let _ = agent_ui_tx.send(ui::UiEvent::Error(format!("{}", e)));
                }
            }
        }
    });

    // Run UI in foreground
    let result = app.run_with_input_callback(input_tx).await;
    
    // Cleanup
    agent_task.abort();
    
    result
}

async fn run_agent_turn_with_ui(
    agent: &Agent,
    session: &mut Session,
    ui_tx: &tokio::sync::mpsc::UnboundedSender<ui::UiEvent>,
) -> anyhow::Result<()> {
    
    for step in 0..agent.max_steps() {
        let _ = ui_tx.send(ui::UiEvent::StatusUpdate(format!("🤔 Thinking... (step {}/{})", step + 1, agent.max_steps())));
        
        // Small delay to ensure status is visible
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        
        // Create a streaming handler that sends to UI
        let handler = TuiStreamHandler {
            ui_tx: ui_tx.clone(),
        };
        
        match agent.run_turn_with_streaming(session, Box::new(handler)).await? {
            Some(final_output) => {
                if !final_output.trim().is_empty() {
                    let _ = ui_tx.send(ui::UiEvent::AgentMessage(final_output));
                }
                return Ok(());
            }
            None => {
                // Continue to next turn
                let _ = ui_tx.send(ui::UiEvent::StatusUpdate(format!("🔄 Processing tools... (step {}/{})", step + 1, agent.max_steps())));
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
    }
    
    Ok(())
}

struct TuiStreamHandler {
    ui_tx: tokio::sync::mpsc::UnboundedSender<ui::UiEvent>,
}

impl agent::AgentStreamHandler for TuiStreamHandler {
    fn on_content_chunk(&mut self, chunk: &str) {
        let _ = self.ui_tx.send(ui::UiEvent::AgentMessage(chunk.to_string()));
    }
    
    fn on_tool_call(&mut self, name: &str, args: &str) {
        let _ = self.ui_tx.send(ui::UiEvent::ToolCall(name.to_string(), args.to_string()));
    }
    
    fn on_tool_result(&mut self, result: &str) {
        let _ = self.ui_tx.send(ui::UiEvent::ToolResult(result.to_string()));
    }
}

fn create_agent_dir() {
    if let Err(err) = std::fs::create_dir(".coder") {
        if err.kind() != std::io::ErrorKind::AlreadyExists {
            eprintln!("Error creating .coder: {}", err);
        }
    }
}
