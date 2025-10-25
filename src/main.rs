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

    println!("\u{001b}[94mWelcome to the Rust ReAct agent!\u{001b}[0m");

    // Environment
    let base_url = env::var("OPENAI_BASE_URL").expect("OPENAI_BASE_URL not set");
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| {
        // Your original used "glm-4.5-air"; keep configurable
        "glm-4.5-air".to_string()
    });

    let llm = LlmClient::new(base_url, api_key, model.clone())?;
    let tools = ToolRegistry::new();
    let opts = AgentOptions {
        max_steps: 12,
        yolo: false, // set true to auto-approve tool calls
        step_timeout: tokio::time::Duration::from_secs(45),
        observation_clip: 4000, // keep large enough for code blocks
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

    println!("\u{001b}[94mWelcome to the Rust ReAct agent! Launching TUI...\u{001b}[0m");

    // Start TUI session with existing system message
    if let Err(e) = ui::run_tui_session(session.messages.clone()) {
        eprintln!("TUI error: {}, falling back to CLI", e);
        // Fallback to CLI if TUI fails
        run_cli_loop(&agent, &mut session).await?;
    }

    println!("Session ended. Session ID: {}", session.id);
    println!("Total messages: {}", session.messages.len());

    Ok(())
}

async fn run_cli_loop(agent: &Agent, session: &mut Session) -> anyhow::Result<()> {
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
            .run_agent_loop(trimmed.to_string(), session)
            .await
        {
            eprintln!("\n\u{001b}[91mError:\u{001b}[0m {}", e);
            println!(
                "\u{001b}[96mAgent:\u{001b}[0m An error occurred while processing your request. Please try again."
            );
        } else {
            println!();
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
