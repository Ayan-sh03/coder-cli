use std::fs;

pub fn list_dir(path: &str) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    match fs::read_dir(path) {
        Ok(items) => {
            for item in items {
                if let Ok(item) = item {
                    entries.push(item.path().display().to_string());
                }
            }
        }
        Err(err) => {
            println!("Error reading directory: {}", err);
        }
    }
    entries
}
