use std::fs::{File, metadata};
use std::io::{BufRead, BufReader};

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; //10MB
const DEFAULT_MAX_LINES: usize = 200;

pub fn read_file(
    path: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<String, String> {
    let metadata = metadata(path).map_err(|e| format!("Failed to get Metadata: {}", e))?;
    //check size
    if metadata.len() > MAX_FILE_SIZE {
        return Err(format!(
            "File Size too Large: {} bytes (max: {} bytes) ",
            metadata.len(),
            MAX_FILE_SIZE
        ));
    }
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;

    let reader = BufReader::new(file);
    let start = start_line.unwrap_or(1);
    let end = end_line.unwrap_or(start + DEFAULT_MAX_LINES - 1);

    let mut lines = Vec::new();
    let mut line_num = 1;

    for line in reader.lines() {
        if line_num > end {
            break;
        }

        let line = line.map_err(|_| "Binary or invalid UTF-8 content detected".to_string())?;

        if line_num >= start {
            lines.push(format!("{}: {}", line_num, line));
        }

        line_num += 1;
    }

    if lines.is_empty() {
        return Err(format!("No lines found in range {}-{}", start, end));
    }

    Ok(lines.join("\n"))
}
