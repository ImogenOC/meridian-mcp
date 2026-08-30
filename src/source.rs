pub(crate) const MAX_SOURCE_LINES: usize = 200;

pub(crate) struct IndexedSource {
    text: String,
    line_starts: Vec<usize>,
}

impl IndexedSource {
    pub(crate) fn new(text: String) -> Self {
        let mut line_starts = Vec::new();
        if !text.is_empty() {
            line_starts.push(0);
            line_starts.extend(
                text.match_indices('\n')
                    .map(|(index, _)| index + 1)
                    .filter(|start| *start < text.len()),
            );
        }
        Self { text, line_starts }
    }

    pub(crate) fn read(path: &std::path::Path) -> std::io::Result<Self> {
        std::fs::read_to_string(path).map(Self::new)
    }

    pub(crate) fn line(&self, one_based_line: u32) -> Option<&str> {
        let index = one_based_line.checked_sub(1)? as usize;
        let start = *self.line_starts.get(index)?;
        let end = self
            .line_starts
            .get(index + 1)
            .map_or(self.text.len(), |next| next.saturating_sub(1));
        self.text.get(start..end).map(|line| {
            let line = line.strip_suffix('\n').unwrap_or(line);
            line.strip_suffix('\r').unwrap_or(line)
        })
    }

    pub(crate) fn declaration(&self, one_based_line: u32, max_lines: usize) -> Option<String> {
        if one_based_line == 0 || max_lines == 0 {
            return None;
        }
        let start_index = one_based_line.checked_sub(1)? as usize;
        self.line_starts.get(start_index)?;

        let mut excerpt = Vec::new();
        let mut in_block_comment = false;
        for index in start_index..self.line_count().min(start_index + max_lines) {
            let line = self.line((index + 1) as u32)?;
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
            excerpt.push(line);

            let mut remainder = line;
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

    pub(crate) fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

/// Read a bounded source excerpt beginning at a DreamMaker declaration line.
pub(crate) fn extract_source(file_path: &str, start_line: u32) -> Option<String> {
    IndexedSource::read(std::path::Path::new(file_path))
        .ok()?
        .declaration(start_line, MAX_SOURCE_LINES)
}

/// Extract a declaration from source text without reading the file again.
#[cfg(test)]
pub(crate) fn extract_source_from_text(
    source: &str,
    start_line: u32,
    max_lines: usize,
) -> Option<String> {
    IndexedSource::new(source.to_owned()).declaration(start_line, max_lines)
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

    #[test]
    fn indexed_source_serves_multiple_declarations_from_one_line_table() {
        let indexed =
            IndexedSource::new("/proc/one()\n\treturn 1\n/proc/two()\n\treturn 2\n".to_owned());
        assert_eq!(indexed.line(1), Some("/proc/one()"));
        assert_eq!(
            indexed.declaration(1, 80).as_deref(),
            Some("/proc/one()\n\treturn 1")
        );
        assert_eq!(
            indexed.declaration(3, 80).as_deref(),
            Some("/proc/two()\n\treturn 2")
        );
        assert_eq!(indexed.line_count(), 4);
    }

    #[test]
    fn indexed_source_preserves_crlf_line_semantics() {
        let indexed = IndexedSource::new("/proc/one()\r\n\treturn 1\r\n".to_owned());

        assert_eq!(indexed.line(1), Some("/proc/one()"));
        assert_eq!(indexed.line(2), Some("\treturn 1"));
        assert_eq!(
            indexed.declaration(1, 80).as_deref(),
            Some("/proc/one()\n\treturn 1")
        );
        assert_eq!(indexed.line_count(), 2);
    }
}
