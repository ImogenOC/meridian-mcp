pub(crate) const MAX_SOURCE_LINES: usize = 200;

/// Read a bounded source excerpt beginning at a DreamMaker declaration line.
pub(crate) fn extract_source(file_path: &str, start_line: u32) -> Option<String> {
    let source = std::fs::read_to_string(file_path).ok()?;
    extract_source_from_text(&source, start_line, MAX_SOURCE_LINES)
}

/// Extract a declaration from source text without reading the file again.
pub(crate) fn extract_source_from_text(
    source: &str,
    start_line: u32,
    max_lines: usize,
) -> Option<String> {
    if start_line == 0 || max_lines == 0 {
        return None;
    }

    let start_index = start_line.checked_sub(1)? as usize;
    let lines: Vec<&str> = source.lines().collect();
    if start_index >= lines.len() {
        return None;
    }

    let mut excerpt = Vec::new();
    let mut in_block_comment = false;
    for line in lines.iter().skip(start_index).take(max_lines) {
        let trimmed = line.trim_start();
        let is_column_zero = line
            .chars()
            .next()
            .is_some_and(|character| !character.is_whitespace());
        let is_comment = in_block_comment
            || trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*');

        if !excerpt.is_empty() && is_column_zero && !is_comment {
            break;
        }

        excerpt.push(*line);

        let mut remainder = *line;
        while let Some(start) = remainder.find("/*") {
            let after_start = &remainder[start + 2..];
            if let Some(end) = after_start.find("*/") {
                remainder = &after_start[end + 2..];
            } else {
                in_block_comment = true;
                break;
            }
        }
        if in_block_comment && remainder.contains("*/") {
            in_block_comment = false;
        }
    }

    Some(excerpt.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SOURCE_FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_source_file(contents: &str) -> PathBuf {
        let unique_suffix = format!(
            "{}_{}",
            std::process::id(),
            SOURCE_FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(format!("meridian_mcp_source_{unique_suffix}.dm"));
        std::fs::write(&path, contents).expect("source fixture should be writable");
        path
    }

    #[test]
    fn extract_source_reads_an_indented_proc_until_the_next_declaration() {
        let path = write_source_file(
            "/proc/example()\n\tvar/value = 1\n\treturn value\n/proc/next()\n\treturn\n",
        );

        let source = extract_source(path.to_str().unwrap(), 1).expect("source should exist");
        assert_eq!(source, "/proc/example()\n\tvar/value = 1\n\treturn value");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn extract_source_keeps_column_zero_comments_inside_the_declaration() {
        let path = write_source_file(
            "/proc/example()\n\treturn\n// explanation\n/proc/next()\n\treturn\n",
        );

        let source = extract_source(path.to_str().unwrap(), 1).expect("source should exist");
        assert_eq!(source, "/proc/example()\n\treturn\n// explanation");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn extract_source_returns_none_for_a_missing_line() {
        let path = write_source_file("/proc/example()\n\treturn\n");

        assert_eq!(extract_source(path.to_str().unwrap(), 99), None);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn extract_source_is_capped_at_maximum_source_lines() {
        let mut contents = String::from("/proc/example()\n");
        for _ in 0..(MAX_SOURCE_LINES + 20) {
            contents.push_str("\treturn\n");
        }
        let path = write_source_file(&contents);

        let source = extract_source(path.to_str().unwrap(), 1).expect("source should exist");
        assert_eq!(source.lines().count(), MAX_SOURCE_LINES);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn extract_source_from_text_honors_a_smaller_caller_limit() {
        let source =
            extract_source_from_text("/proc/example()\n\tvar/one\n\tvar/two\n\treturn\n", 1, 3)
                .expect("source should exist");

        assert_eq!(source.lines().count(), 3);
    }
}
