pub fn clip(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut out = s[..max].to_string();
    out.push_str("… [truncated]");
    out
}

pub fn display_diff_side_by_side(old_str: &str, new_str: &str) {
    let old_lines: Vec<&str> = old_str.lines().collect();
    let new_lines: Vec<&str> = new_str.lines().collect();

    let max_lines = old_lines.len().max(new_lines.len()).min(10);

    // Calculate max width for left column (cap at 50 for readability)
    let left_width = old_lines.iter().map(|l| l.len()).max().unwrap_or(0).min(50);

    println!("\u{001b}[36m╭─ Changes\u{001b}[0m");
    println!(
        "\u{001b}[90m│ {:width$} │ \u{001b}[0m",
        "Before",
        width = left_width
    );
    println!(
        "\u{001b}[90m│ {:width$} │ After\u{001b}[0m",
        "",
        width = left_width
    );
    println!(
        "\u{001b}[36m├─{:─<width$}─┼─\u{001b}[0m",
        "",
        width = left_width
    );

    for i in 0..max_lines {
        let old_line = old_lines.get(i).unwrap_or(&"");
        let new_line = new_lines.get(i).unwrap_or(&"");

        // Truncate if too long
        let old_display = if old_line.len() > left_width {
            format!("{}...", &old_line[..left_width - 3])
        } else {
            old_line.to_string()
        };

        let new_display = if new_line.len() > 50 {
            format!("{}...", &new_line[..47])
        } else {
            new_line.to_string()
        };

        println!(
            "\u{001b}[31m│ {:width$}\u{001b}[0m \u{001b}[90m│\u{001b}[0m \u{001b}[32m{}\u{001b}[0m",
            old_display,
            new_display,
            width = left_width
        );
    }

    if old_lines.len() > max_lines || new_lines.len() > max_lines {
        println!("\u{001b}[90m│ ... (truncated)\u{001b}[0m");
    }

    println!("\u{001b}[36m╰─\u{001b}[0m");
}