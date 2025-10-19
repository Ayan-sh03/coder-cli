use std::fs;

pub fn write_file(path: &str, content: &str) -> Result<String, String> {
    // Write content to file
    fs::write(path, content).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(format!("Successfully wrote to {}", path))
}
