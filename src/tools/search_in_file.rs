use std::fs;
use std::path::Path;

// Search a path (file or dir) for `pattern`.
// If path is a dir we walk it recursively (max 10k matches, 100 file-open limit).
// Uses case-insensitive regex when `case_sensitive==Some(false)`.
pub fn search_in_files(
    pattern: &str,
    path: &str,
    case_sensitive: Option<bool>,
) -> Result<String, String> {
    let regex = {
        let mut builder = regex::RegexBuilder::new(pattern);
        builder.case_insensitive(case_sensitive == Some(false));
        builder
            .build()
            .map_err(|e| format!("Invalid regex: {}", e))?
    };

    let root = Path::new(path);
    let mut hits = Vec::new();
    let mut opened = 0usize;
    let mut checked = 0usize;

    // helper: push matches of a single file.
    fn check_file(p: &Path, re: &regex::Regex, hits: &mut Vec<String>) -> Result<(), String> {
        let buf =
            fs::read_to_string(p).map_err(|_| format!("binary or unreadable: {}", p.display()))?;
        for (idx, line) in buf.lines().enumerate() {
            if re.is_match(line) {
                hits.push(format!("{}:{}:{}", p.display(), idx + 1, line.trim_end()));
                if hits.len() >= 10_000 {
                    return Ok(()); // safety cap
                }
            }
        }
        Ok(())
    }

    // actual walk
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
    {
        if opened >= 100 {
            break;
        }
        let entry = entry.map_err(|e| format!("walk error: {}", e))?;
        if entry.file_type().is_file() {
            opened += 1;
            checked += 1;
            check_file(entry.path(), &regex, &mut hits)?;
            if hits.len() >= 10_000 {
                break;
            }
        }
    }

    match (hits.len(), checked) {
        (0, _) => Err("no matches found".to_string()),
        (_, _) => Ok(format!(
            "Found {} matches in {} files:\n{}",
            hits.len(),
            checked,
            hits.join("\n")
        )),
    }
}
