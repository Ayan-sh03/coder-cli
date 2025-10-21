use std::io::{self, Write};

/// Categories of tools by risk level
const DESTRUCTIVE_TOOLS: &[&str] = &["write_file", "run_shell", "edit_file"];
// const SAFE_TOOLS: &[&str] = &["list_dir", "read_file"];

/// Get user approval with colored output
pub fn get_user_approval(prompt: &str) -> Result<bool, String> {
    print!("\u{001b}[93m⚠️  {} (y/n): \u{001b}[0m", prompt);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read input: {}", e))?;

    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        _ => {
            println!("\u{001b}[91mInvalid input. Please enter 'y' or 'n'\u{001b}[0m");
            get_user_approval(prompt) // Retry
        }
    }
}

/// Format tool call nicely for approval prompt
pub fn format_tool_approval() -> String {
    format!(
        "\n\u{001b}[93m╔════════════════════════════════════╗\n\
         ║ APPROVAL REQUIRED                  ║\n\
         ╚════════════════════════════════════╝\u{001b}[0m\n\
         ",
    )
}

/// Check if a tool requires approval
pub fn requires_approval(tool_name: &str) -> bool {
    DESTRUCTIVE_TOOLS.contains(&tool_name)
}
