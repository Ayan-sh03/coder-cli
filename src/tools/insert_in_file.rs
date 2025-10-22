use std::fs;
/// Inserts content at a specific location in a file.
///
/// # Arguments
///
/// * `path` - The path to the file.
/// * `anchor` - A unique string in the file to locate the insertion point.
/// * `content` - The content to insert.
/// * `position` - "before" or "after" the anchor.
///
pub fn insert_in_file(
    path: &str,
    anchor: &str,
    content: &str,
    position: &str,
) -> Result<String, String> {
    let file_content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;

    if !file_content.contains(anchor) {
        return Err(format!("Anchor '{}' not found in file", anchor));
    }

    let new_content = match position {
        "before" => file_content.replace(anchor, &format!("{}\n{}", content, anchor)),
        "after" => file_content.replace(anchor, &format!("{}\n{}", anchor, content)),
        _ => return Err("Position must be 'before' or 'after'".to_string()),
    };

    fs::write(&path, new_content).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(format!("Successfully inserted content in {}", path))
}
