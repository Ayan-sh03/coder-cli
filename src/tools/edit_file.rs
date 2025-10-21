use std::fs;

/// Edits a file by replacing all occurrences of a string with a new one.
///
/// # Arguments
///
/// * `path` - The path to the file to edit.
/// * `old_str` - The string to be replaced.
/// * `new_str` - The new string to replace with.
///
pub fn edit_file(path: &str, old_str: &str, new_str: &str) -> Result<String, String> {
    // Read the file's content into a string.
    let content = fs::read_to_string(&path).map_err(|e| format!("Faield to read File : {}", e))?;

    // Replace the old string with the new one.
    let new_content = content.replace(old_str, new_str);

    // Write the modified content back to the file.
    fs::write(&path, new_content).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(format!("Successfully edited file  {}", path))
}
