use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;
const TIMEOUT_SECONDS: u64 = 30;
const DENIED_COMMANDS: &[&str] = &["rm", "dd", "mkfs", ":(", "sudo", "su"];

pub fn run_shell(command: &str) -> Result<String, String> {
    // 1. Check denylist
    let mut parts = command.split_whitespace();
    let command_name = parts.next().ok_or("Empty command".to_string())?;

    if DENIED_COMMANDS.contains(&command_name) {
        return Err("Denied command".to_string());
    }

    // 2. Spawn process (don't wait yet)
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn: {}", e))?;

    // 3. Wait with timeout
    let timeout = Duration::from_secs(TIMEOUT_SECONDS);
    match child
        .wait_timeout(timeout)
        .map_err(|e| format!("Wait error: {}", e))?
    {
        Some(status) => {
            // Process finished within timeout
            let output = child
                .wait_with_output()
                .map_err(|e| format!("Failed to get output: {}", e))?;

            if status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        None => {
            // Timeout reached, kill the process
            child.kill().map_err(|e| format!("Failed to kill: {}", e))?;
            Err(format!(
                "Command timed out after {} seconds",
                TIMEOUT_SECONDS
            ))
        }
    }
}
