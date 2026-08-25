use anyhow::Result;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::process::Child;
use tokio::sync::{Mutex, MutexGuard, RwLock};
use tokio::task::JoinHandle;

use crate::analysis_snapshot::{AnalysisBuild, AnalysisSnapshot};
use crate::spaceman::debugger::DebuggerSession;
use crate::spaceman::dmi::DmiCache;

pub(crate) const OUTPUT_LOG_CAPACITY: usize = 500;
pub(crate) const OUTPUT_LINE_MAX_BYTES: usize = 16 * 1024;
pub(crate) const OUTPUT_LOG_MAX_BYTES: usize = 1024 * 1024;
pub(crate) const OUTPUT_TRUNCATED_SUFFIX: &str = "... [truncated]";
pub type OutputLog = Arc<StdMutex<VecDeque<String>>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Standard,
    Tracy,
}

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

#[derive(Default)]
struct AnalysisState {
    active: Option<Arc<AnalysisSnapshot>>,
    generation: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("No environment loaded. Call dm_parse_environment first.")]
    ParseRequired,
}

pub struct RuntimeState {
    pub(crate) game_process: Option<Child>,
    pub(crate) game_port: Option<u16>,
    pub(crate) output_log: OutputLog,
    pub(crate) runtime_output_tasks: Vec<JoinHandle<()>>,
    pub(crate) last_exit_code: Option<i32>,
    pub(crate) kind: Option<RuntimeKind>,
    pub(crate) profiler_port: Option<u16>,
}

impl RuntimeState {
    fn new() -> Self {
        Self {
            game_process: None,
            game_port: None,
            output_log: Arc::new(StdMutex::new(VecDeque::with_capacity(OUTPUT_LOG_CAPACITY))),
            runtime_output_tasks: Vec::new(),
            last_exit_code: None,
            kind: None,
            profiler_port: None,
        }
    }

    pub(crate) fn set_game_process(&mut self, process: Child, port: u16) {
        self.game_process = Some(process);
        self.game_port = Some(port);
        self.last_exit_code = None;
        self.kind = Some(RuntimeKind::Standard);
        self.profiler_port = None;
    }

    pub(crate) fn set_profiled_game_process(
        &mut self,
        process: Child,
        port: u16,
        profiler_port: u16,
    ) {
        self.set_game_process(process, port);
        self.kind = Some(RuntimeKind::Tracy);
        self.profiler_port = Some(profiler_port);
    }

    pub(crate) fn clear_runtime_diagnostics(&mut self) {
        self.abort_runtime_output_tasks();
        let mut lines = self
            .output_log
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        lines.clear();
        self.last_exit_code = None;
    }

    pub(crate) fn add_runtime_output_task(&mut self, task: JoinHandle<()>) {
        self.runtime_output_tasks.push(task);
    }

    fn abort_runtime_output_tasks(&mut self) {
        for task in self.runtime_output_tasks.drain(..) {
            task.abort();
        }
    }

    pub(crate) fn recent_output(&self, limit: usize) -> Vec<String> {
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

    pub(crate) fn matches_output(
        &self,
        pattern: &str,
        use_regex: bool,
    ) -> Result<bool, regex::Error> {
        let output = self.recent_output(OUTPUT_LOG_CAPACITY).join("\n");
        if use_regex {
            Ok(regex::Regex::new(pattern)?.is_match(&output))
        } else {
            Ok(output.contains(pattern))
        }
    }

    pub(crate) fn is_game_running(&mut self) -> bool {
        if let Some(ref mut process) = self.game_process {
            match process.try_wait() {
                Ok(Some(status)) => {
                    self.last_exit_code = status.code();
                    self.game_process = None;
                    self.game_port = None;
                    self.kind = None;
                    self.profiler_port = None;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    self.game_process = None;
                    self.game_port = None;
                    self.kind = None;
                    self.profiler_port = None;
                    false
                }
            }
        } else {
            false
        }
    }

    pub(crate) async fn stop_game_process(&mut self) -> Result<()> {
        if let Some(mut process) = self.game_process.take() {
            process.kill().await?;
            let status = process.wait().await?;
            self.last_exit_code = status.code();
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        self.abort_runtime_output_tasks();
        self.game_port = None;
        self.kind = None;
        self.profiler_port = None;
        Ok(())
    }
}

#[derive(Default)]
pub struct TracyCaptureState {
    pub(crate) active: bool,
    pub(crate) cancellation: Option<tokio::sync::watch::Sender<bool>>,
    pub(crate) output_path: Option<std::path::PathBuf>,
    pub(crate) last_error: Option<String>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ServerState {
    analysis: RwLock<AnalysisState>,
    runtime: Mutex<RuntimeState>,
    assets: Mutex<DmiCache>,
    debugger: Mutex<Option<DebuggerSession>>,
    lifecycle: Mutex<()>,
    tracy_capture: Mutex<TracyCaptureState>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            analysis: RwLock::new(AnalysisState::default()),
            runtime: Mutex::new(RuntimeState::new()),
            assets: Mutex::new(DmiCache::default()),
            debugger: Mutex::new(None),
            lifecycle: Mutex::new(()),
            tracy_capture: Mutex::new(TracyCaptureState::default()),
        }
    }

    pub async fn snapshot(&self) -> Result<Arc<AnalysisSnapshot>, StateError> {
        self.analysis
            .read()
            .await
            .active
            .clone()
            .ok_or(StateError::ParseRequired)
    }

    pub async fn active_snapshot(&self) -> Option<Arc<AnalysisSnapshot>> {
        self.analysis.read().await.active.clone()
    }

    pub async fn install_analysis(&self, build: AnalysisBuild) -> Arc<AnalysisSnapshot> {
        let mut state = self.analysis.write().await;
        state.generation = state.generation.saturating_add(1);
        let snapshot = Arc::new(AnalysisSnapshot::from_build(build, state.generation));
        state.active = Some(Arc::clone(&snapshot));
        snapshot
    }

    pub async fn state_generation(&self) -> u64 {
        self.analysis.read().await.generation
    }

    pub async fn clear_analysis(&self) {
        self.analysis.write().await.active = None;
    }

    pub(crate) async fn runtime(&self) -> MutexGuard<'_, RuntimeState> {
        self.runtime.lock().await
    }

    pub async fn assets(&self) -> MutexGuard<'_, DmiCache> {
        self.assets.lock().await
    }

    pub async fn debugger(&self) -> MutexGuard<'_, Option<DebuggerSession>> {
        self.debugger.lock().await
    }

    pub(crate) async fn lifecycle(&self) -> MutexGuard<'_, ()> {
        self.lifecycle.lock().await
    }

    pub(crate) async fn tracy_capture(&self) -> MutexGuard<'_, TracyCaptureState> {
        self.tracy_capture.lock().await
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
        let log: OutputLog = Arc::new(StdMutex::new(VecDeque::new()));
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
        let log: OutputLog = Arc::new(StdMutex::new(VecDeque::new()));
        push_output_line(&log, "x".repeat(16_384 + 1_000));
        let lines: Vec<String> = log.lock().unwrap().iter().cloned().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() <= 16_384);
        assert!(lines[0].ends_with("... [truncated]"));
    }

    #[test]
    fn output_log_evicts_oldest_lines_to_keep_total_bytes_bounded() {
        let log: OutputLog = Arc::new(StdMutex::new(VecDeque::new()));
        for line_number in 0..500 {
            push_output_line(&log, format!("{line_number:03}-{}", "x".repeat(4_096)));
        }
        let lines = log.lock().unwrap();
        let total_bytes: usize = lines.iter().map(|line| line.len()).sum();
        assert!(total_bytes <= 1_048_576);
        assert_eq!(lines.back().map(|line| &line[..3]), Some("499"));
    }

    #[test]
    fn runtime_output_queries_are_non_destructive() {
        let state = RuntimeState::new();
        for line_number in 0..3 {
            push_output_line(&state.output_log, format!("line {line_number}"));
        }
        assert_eq!(state.recent_output(0), Vec::<String>::new());
        assert_eq!(
            state.recent_output(2),
            vec!["line 1".to_string(), "line 2".to_string()]
        );
        assert!(state.matches_output(r"line \d", true).unwrap());
        assert!(state.matches_output("[invalid", true).is_err());
    }

    #[tokio::test]
    async fn empty_state_requires_a_parse() {
        let state = ServerState::new();
        assert!(matches!(
            state.snapshot().await,
            Err(StateError::ParseRequired)
        ));
        assert_eq!(state.state_generation().await, 0);
    }
}
