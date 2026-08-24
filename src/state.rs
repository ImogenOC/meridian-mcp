use anyhow::Result;
use dreammaker::objtree::ObjectTree;
use dreammaker::Context;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::process::Child;
use tokio::task::JoinHandle;

use crate::search::SearchIndex;
use crate::ProjectProfile;

pub(crate) const OUTPUT_LOG_CAPACITY: usize = 500;
pub(crate) const OUTPUT_LINE_MAX_BYTES: usize = 16 * 1024;
pub(crate) const OUTPUT_LOG_MAX_BYTES: usize = 1024 * 1024;
pub(crate) const OUTPUT_TRUNCATED_SUFFIX: &str = "... [truncated]";
pub type OutputLog = Arc<Mutex<VecDeque<String>>>;

/// Append a process output line while retaining only the most recent lines.
pub fn push_output_line(log: &OutputLog, line: String) {
    let line = truncate_output_line(line);
    let mut lines = log.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if lines.len() >= OUTPUT_LOG_CAPACITY {
        lines.pop_front();
    }
    lines.push_back(line);

    while output_log_byte_len(&lines) > OUTPUT_LOG_MAX_BYTES {
        lines.pop_front();
    }
}

fn truncate_output_line(line: String) -> String {
    if line.len() <= OUTPUT_LINE_MAX_BYTES {
        return line;
    }

    let content_limit = OUTPUT_LINE_MAX_BYTES.saturating_sub(OUTPUT_TRUNCATED_SUFFIX.len());
    let mut split_at = content_limit;
    while !line.is_char_boundary(split_at) {
        split_at -= 1;
    }

    let mut truncated = line[..split_at].to_string();
    truncated.push_str(OUTPUT_TRUNCATED_SUFFIX);
    truncated
}

fn output_log_byte_len(lines: &VecDeque<String>) -> usize {
    lines.iter().map(|line| line.len()).sum()
}

/// Server state that persists across tool calls
pub struct ServerState {
    /// Currently parsed environment path
    pub(crate) environment_path: Option<PathBuf>,
    /// Parsed object tree (cached)
    pub(crate) objtree: Option<Arc<ObjectTree>>,
    /// Parsing context
    pub(crate) context: Option<Context>,
    /// Ranked symbol index derived from the parsed object tree
    pub(crate) search_index: Option<Arc<SearchIndex>>,
    /// Discovered target-project metadata.
    pub(crate) project_profile: Option<ProjectProfile>,
    /// Monotonic generation of the active parsed environment.
    state_generation: u64,
    /// Running DreamDaemon process
    pub game_process: Option<Child>,
    /// Port the game is running on
    pub game_port: Option<u16>,
    /// Recent DreamDaemon stdout and stderr lines
    pub output_log: OutputLog,
    /// Background readers collecting DreamDaemon output
    pub runtime_output_tasks: Vec<JoinHandle<()>>,
    /// Exit code of the most recently exited DreamDaemon process
    pub last_exit_code: Option<i32>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            environment_path: None,
            objtree: None,
            context: None,
            search_index: None,
            project_profile: None,
            state_generation: 0,
            game_process: None,
            game_port: None,
            output_log: Arc::new(Mutex::new(VecDeque::with_capacity(OUTPUT_LOG_CAPACITY))),
            runtime_output_tasks: Vec::new(),
            last_exit_code: None,
        }
    }

    /// Clear cached parse state
    pub fn clear_cache(&mut self) {
        self.objtree = None;
        self.context = None;
        self.search_index = None;
    }

    /// Check if we have a valid parsed environment
    pub fn has_environment(&self) -> bool {
        self.objtree.is_some()
    }

    pub fn state_generation(&self) -> u64 {
        self.state_generation
    }

    pub fn project_profile(&self) -> Option<&ProjectProfile> {
        self.project_profile.as_ref()
    }

    pub(crate) fn replace_environment(
        &mut self,
        path: PathBuf,
        context: Context,
        objtree: ObjectTree,
        search_index: SearchIndex,
        project_profile: Option<ProjectProfile>,
    ) {
        self.environment_path = Some(path);
        self.context = Some(context);
        self.objtree = Some(Arc::new(objtree));
        self.search_index = Some(Arc::new(search_index));
        self.project_profile = project_profile;
        self.state_generation = self.state_generation.saturating_add(1);
    }

    /// Set the running game process
    pub fn set_game_process(&mut self, process: Child, port: u16) {
        self.game_process = Some(process);
        self.game_port = Some(port);
        self.last_exit_code = None;
    }

    /// Clear diagnostics from a previous DreamDaemon process before starting a new one.
    pub fn clear_runtime_diagnostics(&mut self) {
        self.abort_runtime_output_tasks();
        let mut lines = self
            .output_log
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        lines.clear();
        self.last_exit_code = None;
    }

    /// Track a background DreamDaemon output reader for lifecycle cleanup.
    pub fn add_runtime_output_task(&mut self, task: JoinHandle<()>) {
        self.runtime_output_tasks.push(task);
    }

    fn abort_runtime_output_tasks(&mut self) {
        for task in self.runtime_output_tasks.drain(..) {
            task.abort();
        }
    }

    /// Return up to `limit` of the most recent process output lines.
    pub fn recent_output(&self, limit: usize) -> Vec<String> {
        if limit == 0 {
            return Vec::new();
        }

        let lines = self
            .output_log
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let start = lines.len().saturating_sub(limit);
        lines.iter().skip(start).cloned().collect()
    }

    /// Check the captured process output for a literal or regular-expression pattern.
    pub fn matches_output(&self, pattern: &str, use_regex: bool) -> Result<bool, regex::Error> {
        let output = self.recent_output(OUTPUT_LOG_CAPACITY).join("\n");
        if use_regex {
            Ok(regex::Regex::new(pattern)?.is_match(&output))
        } else {
            Ok(output.contains(pattern))
        }
    }

    /// Check if the game is currently running
    pub fn is_game_running(&mut self) -> bool {
        if let Some(ref mut process) = self.game_process {
            // Check if process is still alive
            match process.try_wait() {
                Ok(Some(status)) => {
                    // Process has exited
                    self.last_exit_code = status.code();
                    self.game_process = None;
                    self.game_port = None;
                    false
                }
                Ok(None) => {
                    // Still running
                    true
                }
                Err(_) => {
                    // Error checking - assume dead
                    self.game_process = None;
                    self.game_port = None;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Stop the running game process
    pub async fn stop_game_process(&mut self) -> Result<()> {
        if let Some(mut process) = self.game_process.take() {
            process.kill().await?;
            let status = process.wait().await?;
            self.last_exit_code = status.code();
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        self.abort_runtime_output_tasks();
        self.game_port = None;
        Ok(())
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_log_evicts_oldest_lines_at_capacity() {
        let log: OutputLog = Arc::new(Mutex::new(VecDeque::new()));

        for line_number in 0..=OUTPUT_LOG_CAPACITY {
            push_output_line(&log, format!("line {line_number}"));
        }

        let lines: Vec<String> = log.lock().unwrap().iter().cloned().collect();
        assert_eq!(lines.len(), OUTPUT_LOG_CAPACITY);
        assert_eq!(lines.first().map(String::as_str), Some("line 1"));
        assert_eq!(lines.last().map(String::as_str), Some("line 500"));
    }

    #[test]
    fn output_log_truncates_single_lines_to_a_fixed_byte_limit() {
        let log: OutputLog = Arc::new(Mutex::new(VecDeque::new()));

        push_output_line(&log, "x".repeat(16_384 + 1_000));

        let lines: Vec<String> = log.lock().unwrap().iter().cloned().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() <= 16_384);
        assert!(lines[0].ends_with("... [truncated]"));
    }

    #[test]
    fn output_log_evicts_oldest_lines_to_keep_total_bytes_bounded() {
        let log: OutputLog = Arc::new(Mutex::new(VecDeque::new()));

        for line_number in 0..500 {
            push_output_line(&log, format!("{line_number:03}-{}", "x".repeat(4_096)));
        }

        let lines = log.lock().unwrap();
        let total_bytes: usize = lines.iter().map(|line| line.len()).sum();
        assert!(total_bytes <= 1_048_576);
        assert_eq!(lines.back().map(|line| &line[..3]), Some("499"));
    }

    #[test]
    fn recent_output_returns_requested_tail_without_mutating_log() {
        let state = ServerState::new();
        for line_number in 0..3 {
            push_output_line(&state.output_log, format!("line {line_number}"));
        }

        assert_eq!(state.recent_output(0), Vec::<String>::new());
        assert_eq!(
            state.recent_output(2),
            vec!["line 1".to_string(), "line 2".to_string()]
        );
        assert_eq!(
            state.recent_output(99),
            vec![
                "line 0".to_string(),
                "line 1".to_string(),
                "line 2".to_string()
            ]
        );
    }

    #[test]
    fn output_matching_supports_literal_and_regex_patterns() {
        let state = ServerState::new();
        push_output_line(
            &state.output_log,
            "Initializations complete within 12 seconds".to_string(),
        );

        assert!(state
            .matches_output("Initializations complete", false)
            .unwrap());
        assert!(state
            .matches_output(r"complete within \d+ seconds", true)
            .unwrap());
        assert!(!state.matches_output("not present", false).unwrap());
        assert!(state.matches_output("[invalid", true).is_err());
    }

    #[test]
    fn clear_cache_removes_the_search_index_with_parser_state() {
        let mut state = ServerState::new();
        state.search_index = Some(Arc::new(crate::search::SearchIndex::new(Vec::new())));

        state.clear_cache();

        assert!(state.search_index.is_none());
    }
}
