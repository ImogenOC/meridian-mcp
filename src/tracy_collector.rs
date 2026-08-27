use crate::tracy_protocol::{TracyProtocolError, TRACY_HELPER_SCHEMA_VERSION};
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};

pub const MAX_TRACY_REQUEST_BYTES: usize = 1024 * 1024;
pub const MAX_TRACY_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn capture_window_started(error: &TracyProtocolError) -> bool {
    matches!(
        error,
        TracyProtocolError::Helper { details, .. }
            if details["window_started"].as_bool() == Some(true)
    )
}

pub(crate) fn capture_failure_code(error: &TracyProtocolError) -> crate::result::ToolErrorCode {
    match error {
        TracyProtocolError::Helper { code, .. }
            if !capture_window_started(error)
                && matches!(
                    code.as_str(),
                    "connect_timeout"
                        | "handshake_dropped"
                        | "client_disconnected"
                        | "profiler_busy"
                        | "health_timeout"
                ) =>
        {
            crate::result::ToolErrorCode::CaptureNotReady
        }
        _ => crate::result::ToolErrorCode::HelperFailure,
    }
}

type PendingResult = Result<Value, String>;

pub struct CollectorTransport<W> {
    writer: Arc<Mutex<W>>,
    next_request_id: Arc<AtomicU64>,
    pending: Arc<Mutex<BTreeMap<u64, oneshot::Sender<PendingResult>>>>,
    terminal_error: Arc<Mutex<Option<String>>>,
    request_timeout: Duration,
}

impl<W> Clone for CollectorTransport<W> {
    fn clone(&self) -> Self {
        Self {
            writer: Arc::clone(&self.writer),
            next_request_id: Arc::clone(&self.next_request_id),
            pending: Arc::clone(&self.pending),
            terminal_error: Arc::clone(&self.terminal_error),
            request_timeout: self.request_timeout,
        }
    }
}

impl<W> CollectorTransport<W>
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    pub fn new<R>(reader: R, writer: W, request_timeout: Duration) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
    {
        let transport = Self {
            writer: Arc::new(Mutex::new(writer)),
            next_request_id: Arc::new(AtomicU64::new(1)),
            pending: Arc::new(Mutex::new(BTreeMap::new())),
            terminal_error: Arc::new(Mutex::new(None)),
            request_timeout,
        };
        transport.spawn_reader(reader);
        transport
    }

    fn spawn_reader<R>(&self, reader: R)
    where
        R: AsyncRead + Unpin + Send + 'static,
    {
        let pending = Arc::clone(&self.pending);
        let terminal_error = Arc::clone(&self.terminal_error);
        tokio::spawn(async move {
            let mut reader = BufReader::new(reader);
            let mut line = Vec::new();
            loop {
                line.clear();
                let read = match reader.read_until(b'\n', &mut line).await {
                    Ok(read) => read,
                    Err(error) => {
                        fail_transport(
                            &pending,
                            &terminal_error,
                            format!("collector read failed: {error}"),
                        )
                        .await;
                        return;
                    }
                };
                if read == 0 {
                    fail_transport(
                        &pending,
                        &terminal_error,
                        "collector closed stdout".to_owned(),
                    )
                    .await;
                    return;
                }
                if line.len() > MAX_TRACY_RESPONSE_BYTES {
                    fail_transport(
                        &pending,
                        &terminal_error,
                        "collector response exceeded the fixed byte limit".to_owned(),
                    )
                    .await;
                    return;
                }
                while line
                    .last()
                    .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
                {
                    line.pop();
                }
                let document: Value = match serde_json::from_slice(&line) {
                    Ok(document) => document,
                    Err(error) => {
                        fail_transport(
                            &pending,
                            &terminal_error,
                            format!("collector returned invalid JSON: {error}"),
                        )
                        .await;
                        return;
                    }
                };
                if document.get("schema_version").and_then(Value::as_u64)
                    != Some(TRACY_HELPER_SCHEMA_VERSION.into())
                {
                    fail_transport(
                        &pending,
                        &terminal_error,
                        "collector response schema mismatch".to_owned(),
                    )
                    .await;
                    return;
                }
                let Some(id) = document.get("id").and_then(Value::as_u64) else {
                    fail_transport(
                        &pending,
                        &terminal_error,
                        "collector response omitted its id".to_owned(),
                    )
                    .await;
                    return;
                };
                let Some(sender) = pending.lock().await.remove(&id) else {
                    fail_transport(
                        &pending,
                        &terminal_error,
                        format!("collector returned unknown or duplicate response id {id}"),
                    )
                    .await;
                    return;
                };
                let _ = sender.send(Ok(document));
            }
        });
    }

    pub async fn request(&self, command: &str, params: Value) -> Result<Value, TracyProtocolError> {
        if !params.is_object() {
            return Err(TracyProtocolError::InvalidParams);
        }
        if let Some(error) = self.terminal_error.lock().await.clone() {
            return Err(TracyProtocolError::Transport(error));
        }
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let mut request = serde_json::to_vec(&serde_json::json!({
            "schema_version": TRACY_HELPER_SCHEMA_VERSION,
            "id": id,
            "command": command,
            "params": params,
        }))?;
        request.push(b'\n');
        if request.len() > MAX_TRACY_REQUEST_BYTES {
            return Err(TracyProtocolError::RequestTooLarge);
        }
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(id, sender);
        if let Err(error) = self.writer.lock().await.write_all(&request).await {
            self.pending.lock().await.remove(&id);
            return Err(TracyProtocolError::Transport(format!(
                "collector write failed: {error}"
            )));
        }
        let document = match tokio::time::timeout(self.request_timeout, receiver).await {
            Ok(Ok(Ok(document))) => document,
            Ok(Ok(Err(error))) => return Err(TracyProtocolError::Transport(error)),
            Ok(Err(_)) => {
                return Err(TracyProtocolError::Transport(
                    "collector response channel closed".to_owned(),
                ))
            }
            Err(_) => {
                self.pending.lock().await.remove(&id);
                return Err(TracyProtocolError::Timeout { id });
            }
        };
        if document.get("ok").and_then(Value::as_bool) == Some(false) {
            return Err(TracyProtocolError::Helper {
                code: document["error"]["code"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_owned(),
                message: document["error"]["message"]
                    .as_str()
                    .unwrap_or("helper failed")
                    .to_owned(),
                details: document["error"]
                    .get("details")
                    .cloned()
                    .unwrap_or(Value::Null),
            });
        }
        document
            .get("result")
            .cloned()
            .ok_or(TracyProtocolError::MissingResult)
    }
}

async fn fail_transport(
    pending: &Mutex<BTreeMap<u64, oneshot::Sender<PendingResult>>>,
    terminal_error: &Mutex<Option<String>>,
    message: String,
) {
    *terminal_error.lock().await = Some(message.clone());
    let requests = std::mem::take(&mut *pending.lock().await);
    for (_, sender) in requests {
        let _ = sender.send(Err(message.clone()));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TracySessionPhase {
    ProcessStarting,
    HookFailed,
    ListenerWaiting,
    CollectorConnecting,
    HealthyIdle,
    CaptureActive,
    ProducerStalled,
    Saturated,
    RecoveryRequired,
    Stopping,
    Stopped,
}

pub struct TracyCollectorSpec {
    pub helper: PathBuf,
    pub working_directory: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
    pub request_timeout: Duration,
}

pub struct TracyCollector {
    child: Mutex<Child>,
    transport: CollectorTransport<ChildStdin>,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    stderr_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    _containment: crate::process::ProcessContainment,
    exit_code: Mutex<Option<i32>>,
}

impl TracyCollector {
    pub async fn spawn(spec: TracyCollectorSpec) -> Result<Self, TracyProtocolError> {
        let containment = crate::process::ProcessContainment::new().map_err(|error| {
            TracyProtocolError::Transport(format!("cannot create collector containment: {error}"))
        })?;
        let mut child = Command::new(&spec.helper)
            .arg("--session")
            .current_dir(&spec.working_directory)
            .env_clear()
            .envs(spec.environment)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| {
                TracyProtocolError::Transport(format!("failed to spawn collector: {error}"))
            })?;
        let process_id = child.id().ok_or_else(|| {
            TracyProtocolError::Transport("collector process id unavailable".to_owned())
        })?;
        if let Err(error) = containment.assign(process_id) {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(TracyProtocolError::Transport(format!(
                "cannot contain collector process: {error}"
            )));
        }
        let stdout = child.stdout.take().ok_or_else(|| {
            TracyProtocolError::Transport("collector stdout unavailable".to_owned())
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            TracyProtocolError::Transport("collector stdin unavailable".to_owned())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            TracyProtocolError::Transport("collector stderr unavailable".to_owned())
        })?;
        let stderr_tail = Arc::new(Mutex::new(VecDeque::new()));
        let stderr_lines = Arc::clone(&stderr_tail);
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(mut line)) = lines.next_line().await {
                if line.len() > 4096 {
                    line.truncate(4096);
                    line.push_str("... [truncated]");
                }
                let mut tail = stderr_lines.lock().await;
                if tail.len() == 64 {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
        });
        Ok(Self {
            child: Mutex::new(child),
            transport: CollectorTransport::new(stdout, stdin, spec.request_timeout),
            stderr_tail,
            stderr_task: Mutex::new(Some(stderr_task)),
            _containment: containment,
            exit_code: Mutex::new(None),
        })
    }

    pub async fn session_start(&self, host: &str, port: u16) -> Result<Value, TracyProtocolError> {
        self.transport
            .request(
                "session_start",
                serde_json::json!({
                    "host": host,
                    "port": port,
                    "connect_timeout_ms": 15_000,
                    "progress_timeout_ms": 15_000,
                }),
            )
            .await
    }

    pub async fn capture_window(
        &self,
        duration_ms: u64,
        memory_limit_mb: u64,
        output_path: &Path,
        phase: &str,
        phase_iteration: u32,
    ) -> Result<Value, TracyProtocolError> {
        self.transport
            .request(
                "capture_window",
                serde_json::json!({
                    "duration_ms": duration_ms,
                    "memory_limit_mb": memory_limit_mb,
                    "output_path": output_path,
                    "phase": phase,
                    "phase_iteration": phase_iteration,
                }),
            )
            .await
    }

    pub async fn status(&self) -> Result<Value, TracyProtocolError> {
        self.transport
            .request("session_status", serde_json::json!({}))
            .await
    }

    pub async fn cancel(&self) -> Result<Value, TracyProtocolError> {
        self.transport
            .request("cancel", serde_json::json!({}))
            .await
    }

    pub async fn stop(&self, timeout: Duration) -> Result<Value, TracyProtocolError> {
        let result = self
            .transport
            .request("session_stop", serde_json::json!({}))
            .await;
        let mut child = self.child.lock().await;
        let waited = tokio::time::timeout(timeout, child.wait()).await;
        if waited.is_err() {
            child.kill().await.map_err(|error| {
                TracyProtocolError::Transport(format!("collector kill failed: {error}"))
            })?;
            let _ = child.wait().await;
        }
        if let Some(task) = self.stderr_task.lock().await.take() {
            task.abort();
        }
        result
    }

    pub async fn stderr_tail(&self) -> Vec<String> {
        self.stderr_tail.lock().await.iter().cloned().collect()
    }

    pub async fn is_running(&self) -> bool {
        match self.child.lock().await.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                *self.exit_code.lock().await = status.code();
                false
            }
            Err(_) => false,
        }
    }

    pub async fn process_id(&self) -> Option<u32> {
        self.child.lock().await.id()
    }

    pub async fn exit_code(&self) -> Option<i32> {
        *self.exit_code.lock().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_error_reports_whether_the_measurement_window_started() {
        let started = TracyProtocolError::Helper {
            code: "client_disconnected".to_owned(),
            message: "disconnected".to_owned(),
            details: serde_json::json!({"window_started": true}),
        };
        let not_started = TracyProtocolError::Helper {
            code: "connect_timeout".to_owned(),
            message: "timeout".to_owned(),
            details: serde_json::json!({"window_started": false}),
        };
        assert!(capture_window_started(&started));
        assert!(!capture_window_started(&not_started));
        assert!(!capture_window_started(&TracyProtocolError::Timeout {
            id: 1
        }));
        assert_eq!(
            capture_failure_code(&not_started),
            crate::result::ToolErrorCode::CaptureNotReady
        );
        assert_eq!(
            capture_failure_code(&started),
            crate::result::ToolErrorCode::HelperFailure
        );
    }
}
