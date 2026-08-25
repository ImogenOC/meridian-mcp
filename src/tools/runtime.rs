use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, SeekFrom};
use tokio::process::Command;
use tracing::info;

const DEFAULT_OUTPUT_WAIT_TIMEOUT_MS: u64 = 30_000;
const MAX_OUTPUT_WAIT_TIMEOUT_MS: u64 = 300_000;
const OUTPUT_WAIT_POLL_INTERVAL_MS: u64 = 100;
const OUTPUT_READ_CHUNK_BYTES: usize = 8 * 1024;
const LOG_FILE_POLL_INTERVAL_MS: u64 = 50;

use crate::mcp::ToolResult;
use crate::state::{
    OutputLog, RuntimeState, ServerState, OUTPUT_LINE_MAX_BYTES, OUTPUT_TRUNCATED_SUFFIX,
};

/// Find the DreamDaemon executable
fn find_dreamdaemon() -> Option<PathBuf> {
    let possible_paths = [
        r"C:\Program Files (x86)\BYOND\bin\dreamdaemon.exe",
        r"C:\Program Files\BYOND\bin\dreamdaemon.exe",
        "/usr/local/byond/bin/DreamDaemon",
        "/opt/byond/bin/DreamDaemon",
    ];

    for path in &possible_paths {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    if let Ok(path) = which::which("dreamdaemon") {
        return Some(path);
    }
    if let Ok(path) = which::which("DreamDaemon") {
        return Some(path);
    }

    None
}

fn build_dreamdaemon_args(dmb_path: &Path, port: u16, extra_args: &[String]) -> Vec<String> {
    let mut arguments = vec![
        dmb_path.display().to_string(),
        port.to_string(),
        "-trusted".to_string(),
        "-logself".to_string(),
    ];
    arguments.extend(extra_args.iter().cloned());
    arguments
}

fn normalize_spawn_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let path_text = path.to_string_lossy();
        if let Some(unc_path) = path_text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{unc_path}"));
        }
        if let Some(dos_path) = path_text.strip_prefix(r"\\?\") {
            return PathBuf::from(dos_path);
        }
    }

    path.to_path_buf()
}

fn readiness_succeeded(readiness: &Value) -> bool {
    readiness
        .get("matched")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Start DreamDaemon with a .dmb file
pub async fn run(state: &ServerState, args: Value) -> Result<ToolResult> {
    let mut state = state.runtime().await;
    // Check if already running
    if state.is_game_running() {
        return Ok(ToolResult::error(
            "A game instance is already running. Use dm_stop first.",
        ));
    }

    let dmb_path = args
        .get("dmb_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing dmb_path argument"))?;

    let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(1337) as u16;

    let requested_path = PathBuf::from(dmb_path);
    let requested_working_directory = args
        .get("working_directory")
        .and_then(|value| value.as_str())
        .map(PathBuf::from);
    let path = if requested_path.is_absolute() {
        requested_path
    } else if let Some(working_directory) = &requested_working_directory {
        working_directory.join(requested_path)
    } else {
        requested_path
    };
    if !path.exists() {
        return Ok(ToolResult::error(format!("DMB file not found: {dmb_path}")));
    }
    let path = path.canonicalize()?;
    let spawn_path = normalize_spawn_path(&path);
    let working_directory = spawn_path.parent().ok_or_else(|| {
        anyhow!(
            "DMB file has no working directory: {}",
            spawn_path.display()
        )
    })?;

    let extra_args: Vec<String> = args
        .get("daemon_args")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| anyhow!("daemon_args must contain only strings"))
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();

    let dreamdaemon = find_dreamdaemon()
        .ok_or_else(|| anyhow!("DreamDaemon not found. Please install BYOND."))?;

    info!(
        "Starting DreamDaemon with {} on port {}",
        path.display(),
        port
    );
    state.clear_runtime_diagnostics();

    // Start DreamDaemon. The DMB path is canonicalized and the daemon runs from its parent so
    // relative config, log, and map paths resolve against the game checkout rather than the MCP
    // server's installation directory.
    let daemon_args = build_dreamdaemon_args(&spawn_path, port, &extra_args);
    let log_path = spawn_path.with_extension("log");
    let log_start_offset = std::fs::metadata(&log_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let mut child = Command::new(&dreamdaemon)
        .args(&daemon_args)
        .current_dir(working_directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let pid = child.id();

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    state.set_game_process(child, port);

    if let Some(stdout) = stdout {
        let output_log = Arc::clone(&state.output_log);
        let task = tokio::spawn(async move {
            capture_output_stream(stdout, output_log).await;
        });
        state.add_runtime_output_task(task);
    }

    if let Some(stderr) = stderr {
        let output_log = Arc::clone(&state.output_log);
        let task = tokio::spawn(async move {
            capture_output_stream(stderr, output_log).await;
        });
        state.add_runtime_output_task(task);
    }

    let output_log = Arc::clone(&state.output_log);
    let task = tokio::spawn(capture_output_file(
        log_path.clone(),
        log_start_offset,
        output_log,
    ));
    state.add_runtime_output_task(task);

    // Give the child process a short window to fail before returning control to the caller.
    tokio::time::sleep(Duration::from_millis(250)).await;

    // Check if it's actually running
    if !state.is_game_running() {
        return Ok(ToolResult::error(
            json!({
                "message": "DreamDaemon process exited immediately. Check the DMB file.",
                "last_exit_code": state.last_exit_code,
                "recent_output": state.recent_output(50)
            })
            .to_string(),
        ));
    }

    let mut result = json!({
        "success": true,
        "pid": pid,
        "port": port,
        "dmb_path": dmb_path,
        "working_directory": working_directory.display().to_string(),
        "message": format!("DreamDaemon started on port {}", port)
    });

    if let Some(pattern) = args.get("wait_for").and_then(|value| value.as_str()) {
        let use_regex = args
            .get("wait_regex")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let timeout_ms = args
            .get("startup_timeout_ms")
            .and_then(|value| value.as_u64())
            .unwrap_or(DEFAULT_OUTPUT_WAIT_TIMEOUT_MS);
        let wait_result = wait_for_output_value(&mut state, pattern, use_regex, timeout_ms).await?;
        result["readiness"] = wait_result;
        if !readiness_succeeded(&result["readiness"]) {
            result["success"] = json!(false);
            result["process_stopped"] = json!(state.stop_game_process().await.is_ok());
            return Ok(ToolResult::error(result.to_string()));
        }
    }

    Ok(ToolResult::text(result.to_string()))
}

async fn capture_output_stream<R>(mut stream: R, output_log: OutputLog)
where
    R: AsyncRead + Unpin,
{
    let mut read_buffer = [0_u8; OUTPUT_READ_CHUNK_BYTES];
    let mut line_buffer = Vec::with_capacity(OUTPUT_READ_CHUNK_BYTES);
    let mut line_truncated = false;
    let line_content_limit = OUTPUT_LINE_MAX_BYTES.saturating_sub(OUTPUT_TRUNCATED_SUFFIX.len());

    loop {
        let bytes_read = match stream.read(&mut read_buffer).await {
            Ok(0) => break,
            Ok(bytes_read) => bytes_read,
            Err(_) => break,
        };

        for byte in &read_buffer[..bytes_read] {
            if *byte == b'\n' {
                push_captured_output_line(&output_log, &mut line_buffer, line_truncated);
                line_truncated = false;
                continue;
            }

            if line_buffer.len() < line_content_limit {
                line_buffer.push(*byte);
            } else {
                line_truncated = true;
            }
        }
    }

    if !line_buffer.is_empty() || line_truncated {
        push_captured_output_line(&output_log, &mut line_buffer, line_truncated);
    }
}

async fn capture_output_file(path: PathBuf, start_offset: u64, output_log: OutputLog) {
    let mut file = loop {
        match OpenOptions::new().read(true).open(&path).await {
            Ok(file) => break file,
            Err(_) => tokio::time::sleep(Duration::from_millis(LOG_FILE_POLL_INTERVAL_MS)).await,
        }
    };

    let mut offset = start_offset;
    if file.seek(SeekFrom::Start(offset)).await.is_err() {
        return;
    }

    let mut read_buffer = [0_u8; OUTPUT_READ_CHUNK_BYTES];
    let mut line_buffer = Vec::with_capacity(OUTPUT_READ_CHUNK_BYTES);
    let mut line_truncated = false;
    let line_content_limit = OUTPUT_LINE_MAX_BYTES.saturating_sub(OUTPUT_TRUNCATED_SUFFIX.len());

    loop {
        match file.read(&mut read_buffer).await {
            Ok(0) => {
                if let Ok(metadata) = file.metadata().await {
                    if metadata.len() < offset {
                        offset = 0;
                        line_buffer.clear();
                        line_truncated = false;
                        if file.seek(SeekFrom::Start(0)).await.is_err() {
                            return;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(LOG_FILE_POLL_INTERVAL_MS)).await;
            }
            Ok(bytes_read) => {
                offset += bytes_read as u64;
                for byte in &read_buffer[..bytes_read] {
                    if *byte == b'\n' {
                        push_captured_output_line(&output_log, &mut line_buffer, line_truncated);
                        line_truncated = false;
                        continue;
                    }

                    if line_buffer.len() < line_content_limit {
                        line_buffer.push(*byte);
                    } else {
                        line_truncated = true;
                    }
                }
            }
            Err(_) => return,
        }
    }
}

fn push_captured_output_line(
    output_log: &OutputLog,
    line_buffer: &mut Vec<u8>,
    line_truncated: bool,
) {
    if line_buffer.last() == Some(&b'\r') {
        line_buffer.pop();
    }

    let mut line = String::from_utf8_lossy(line_buffer).to_string();
    if line_truncated {
        line.push_str(OUTPUT_TRUNCATED_SUFFIX);
    }
    crate::state::push_output_line(output_log, line);
    line_buffer.clear();
}

async fn wait_for_output_value(
    state: &mut RuntimeState,
    pattern: &str,
    use_regex: bool,
    timeout_ms: u64,
) -> Result<Value> {
    let timeout_ms = timeout_ms.min(MAX_OUTPUT_WAIT_TIMEOUT_MS);
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        let matched = state
            .matches_output(pattern, use_regex)
            .map_err(|error| anyhow!("Invalid output regex: {error}"))?;
        if matched {
            return Ok(json!({
                "matched": true,
                "pattern": pattern,
                "regex": use_regex,
                "timed_out": false,
                "recent_output": state.recent_output(50)
            }));
        }

        if !state.is_game_running() {
            return Ok(json!({
                "matched": false,
                "pattern": pattern,
                "regex": use_regex,
                "timed_out": false,
                "process_exited": true,
                "last_exit_code": state.last_exit_code,
                "recent_output": state.recent_output(50)
            }));
        }

        if tokio::time::Instant::now() >= deadline {
            return Ok(json!({
                "matched": false,
                "pattern": pattern,
                "regex": use_regex,
                "timed_out": true,
                "process_exited": false,
                "recent_output": state.recent_output(50)
            }));
        }

        tokio::time::sleep(Duration::from_millis(OUTPUT_WAIT_POLL_INTERVAL_MS)).await;
    }
}

/// Wait until DreamDaemon output contains a literal or regular-expression marker.
pub async fn wait_for_output(state: &ServerState, args: Value) -> Result<ToolResult> {
    let mut state = state.runtime().await;
    let running = state.is_game_running();
    let has_runtime_diagnostics =
        state.last_exit_code.is_some() || !state.recent_output(1).is_empty();
    if !running && !has_runtime_diagnostics {
        return Ok(ToolResult::error(
            json!({
                "message": "No game instance is currently running.",
                "last_exit_code": state.last_exit_code,
                "recent_output": state.recent_output(50)
            })
            .to_string(),
        ));
    }

    let pattern = args
        .get("pattern")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("Missing pattern argument"))?;
    let use_regex = args
        .get("regex")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let timeout_ms = args
        .get("timeout_ms")
        .and_then(|value| value.as_u64())
        .unwrap_or(DEFAULT_OUTPUT_WAIT_TIMEOUT_MS);

    let result = wait_for_output_value(&mut state, pattern, use_regex, timeout_ms).await?;
    Ok(ToolResult::text(result.to_string()))
}

#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::ToolContent;
    use crate::state::push_output_line;
    use std::fs::OpenOptions;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(windows)]
    #[test]
    fn dreamdaemon_spawn_paths_drop_windows_verbatim_prefix() {
        let verbatim = Path::new(r"\\?\C:\workspace\tgstation.dmb");
        assert_eq!(
            normalize_spawn_path(verbatim),
            PathBuf::from(r"C:\workspace\tgstation.dmb")
        );

        let ordinary = Path::new(r"C:\workspace\tgstation.dmb");
        assert_eq!(normalize_spawn_path(ordinary), ordinary);

        let unc_verbatim = Path::new(r"\\?\UNC\server\share\tgstation.dmb");
        assert_eq!(
            normalize_spawn_path(unc_verbatim),
            PathBuf::from(r"\\server\share\tgstation.dmb")
        );
    }

    #[test]
    fn readiness_requires_a_matching_marker() {
        assert!(readiness_succeeded(&json!({"matched": true})));
        assert!(!readiness_succeeded(&json!({"matched": false})));
        assert!(!readiness_succeeded(&json!({"timed_out": true})));
    }

    #[test]
    fn daemon_arguments_preserve_extra_runtime_arguments() {
        let arguments = build_dreamdaemon_args(
            PathBuf::from("tgstation.test.dmb").as_path(),
            1337,
            &[
                "-close".to_string(),
                "-params".to_string(),
                "log-directory=ci".to_string(),
            ],
        );

        assert_eq!(
            arguments,
            vec![
                "tgstation.test.dmb",
                "1337",
                "-trusted",
                "-logself",
                "-close",
                "-params",
                "log-directory=ci",
            ]
        );
    }

    #[tokio::test]
    async fn wait_for_output_can_match_retained_output_after_exit() {
        let state = ServerState::new();
        {
            let mut runtime = state.runtime().await;
            runtime.last_exit_code = Some(1);
            push_output_line(
                &runtime.output_log,
                "fatal: initialization failed".to_string(),
            );
        }

        let result = wait_for_output(
            &state,
            json!({"pattern": "initialization failed", "timeout_ms": 10}),
        )
        .await
        .expect("waiting for retained output should succeed");

        assert_eq!(result.is_error, None);
        let ToolContent::Text { text } = &result.content[0];
        let payload: Value = serde_json::from_str(text).expect("wait result should be JSON");
        assert_eq!(payload["matched"], true);
    }

    #[tokio::test]
    async fn output_capture_emits_complete_lines_and_final_unterminated_line() {
        let log = crate::state::OutputLog::default();
        let input = b"ready\npartial without newline";

        capture_output_stream(&input[..], Arc::clone(&log)).await;

        let lines: Vec<String> = log.lock().unwrap().iter().cloned().collect();
        assert_eq!(
            lines,
            vec!["ready".to_string(), "partial without newline".to_string(),]
        );
    }

    #[tokio::test]
    async fn output_capture_truncates_oversized_unterminated_lines() {
        let log = crate::state::OutputLog::default();
        let input = "x".repeat(16_384 + 1_000);

        capture_output_stream(input.as_bytes(), Arc::clone(&log)).await;

        let lines: Vec<String> = log.lock().unwrap().iter().cloned().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() <= 16_384);
        assert!(lines[0].ends_with("... [truncated]"));
    }

    #[test]
    fn topic_packet_matches_the_byond_export_wire_format() {
        assert_eq!(
            build_topic_packet("ping").expect("topic packet should build"),
            vec![
                0x00, 0x83, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, b'p', b'i', b'n', b'g', 0x00,
            ]
        );
    }

    #[test]
    fn topic_response_decodes_strings_and_floats() {
        assert_eq!(
            decode_topic_response(&[0x06, b'p', b'o', b'n', b'g', 0x00])
                .expect("string response should decode"),
            "pong"
        );
        assert_eq!(
            decode_topic_response(&[0x2a, 0x00, 0x00, 0x20, 0x40])
                .expect("float response should decode"),
            "2.5"
        );
    }

    #[tokio::test]
    async fn log_capture_starts_at_the_recorded_offset_and_follows_appends() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "meridian-mcp-runtime-log-{}-{unique}.log",
            std::process::id()
        ));
        std::fs::write(&path, "old run\n").unwrap();
        let start_offset = std::fs::metadata(&path).unwrap().len();
        let log = crate::state::OutputLog::default();
        let task = tokio::spawn(capture_output_file(
            path.clone(),
            start_offset,
            Arc::clone(&log),
        ));

        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(file, "new run ready").unwrap();
        file.flush().unwrap();

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if log
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|line| line == "new run ready")
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("appended log output should be captured");

        task.abort();
        assert!(!log.lock().unwrap().iter().any(|line| line == "old run"));
        std::fs::remove_file(path).unwrap();
    }
}

/// Stop the running DreamDaemon instance
pub async fn stop(state: &ServerState, _args: Value) -> Result<ToolResult> {
    let mut state = state.runtime().await;
    if !state.is_game_running() {
        return Ok(ToolResult::error("No game instance is currently running."));
    }

    match state.stop_game_process().await {
        Ok(()) => Ok(ToolResult::text(
            json!({
                "success": true,
                "message": "DreamDaemon stopped"
            })
            .to_string(),
        )),
        Err(e) => Ok(ToolResult::error(format!(
            "Failed to stop DreamDaemon: {e}"
        ))),
    }
}

/// Get status of the running game
pub async fn status(state: &ServerState, _args: Value) -> Result<ToolResult> {
    let mut state = state.runtime().await;
    if !state.is_game_running() {
        return Ok(ToolResult::text(
            json!({
                "running": false,
                "last_exit_code": state.last_exit_code,
                "recent_output": state.recent_output(50)
            })
            .to_string(),
        ));
    }

    let port = state.game_port.unwrap_or(0);
    let pid = state.game_process.as_ref().map(|p| p.id());

    Ok(ToolResult::text(
        json!({
            "running": true,
            "port": port,
            "pid": pid,
            "recent_output": state.recent_output(50)
        })
        .to_string(),
    ))
}

/// Send a Topic() call to the running game server
pub async fn topic(state: &ServerState, args: Value) -> Result<ToolResult> {
    let mut state = state.runtime().await;
    if !state.is_game_running() {
        return Ok(ToolResult::error("No game instance is currently running."));
    }

    let topic_string = args
        .get("topic")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing topic argument"))?;

    let port = state.game_port.unwrap_or(1337);
    let timeout_ms = args
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(5000);

    info!("Sending Topic to port {}: {}", port, topic_string);

    match send_topic(&format!("127.0.0.1:{port}"), topic_string, timeout_ms).await {
        Ok(response) => Ok(ToolResult::text(
            json!({
                "success": true,
                "response": response
            })
            .to_string(),
        )),
        Err(e) => Ok(ToolResult::error(format!("Topic call failed: {e}"))),
    }
}

/// Send a BYOND Topic packet and get response
async fn send_topic(address: &str, topic: &str, timeout_ms: u64) -> Result<String> {
    let topic_clean = topic.strip_prefix('?').unwrap_or(topic);
    info!(
        "Sending topic packet for: {} (cleaned: {})",
        topic, topic_clean
    );
    let packet = build_topic_packet(topic_clean)?;

    // Clone address for the blocking task
    let address_owned = address.to_string();

    // Connect with timeout
    let stream = tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        tokio::task::spawn_blocking(move || TcpStream::connect(&address_owned)),
    )
    .await??;

    let mut stream = stream?;
    stream.set_read_timeout(Some(Duration::from_millis(timeout_ms)))?;
    stream.set_write_timeout(Some(Duration::from_millis(timeout_ms)))?;

    // Send the packet
    stream.write_all(&packet)?;
    stream.flush()?;

    // Read response
    let mut response_header = [0u8; 4];
    std::io::Read::read_exact(&mut stream, &mut response_header)?;

    if response_header[0] != 0x00 || response_header[1] != 0x83 {
        return Err(anyhow!("Invalid response header"));
    }

    let response_len = ((response_header[2] as u16) << 8) | (response_header[3] as u16);

    if response_len == 0 {
        return Ok(String::new());
    }

    let mut response_data = vec![0u8; response_len as usize];
    std::io::Read::read_exact(&mut stream, &mut response_data)?;

    decode_topic_response(&response_data)
}

fn build_topic_packet(topic: &str) -> Result<Vec<u8>> {
    let topic_bytes = topic.as_bytes();
    let data_len = topic_bytes.len() + 6;
    if data_len > u16::MAX as usize {
        return Err(anyhow!("Topic is too long"));
    }

    let mut packet = Vec::with_capacity(4 + data_len);
    packet.extend_from_slice(&[0x00, 0x83, (data_len >> 8) as u8, (data_len & 0xff) as u8]);
    packet.extend_from_slice(&[0x00; 5]);
    packet.extend_from_slice(topic_bytes);
    packet.push(0x00);
    Ok(packet)
}

fn decode_topic_response(response_data: &[u8]) -> Result<String> {
    let Some(response_type) = response_data.first() else {
        return Err(anyhow!("BYOND returned an empty Topic response"));
    };

    match response_type {
        0x00 if response_data.len() == 1 => Ok(String::new()),
        0x06 => {
            let string_data = &response_data[1..];
            let null_pos = string_data
                .iter()
                .position(|&byte| byte == 0)
                .unwrap_or(string_data.len());
            Ok(String::from_utf8_lossy(&string_data[..null_pos]).to_string())
        }
        0x2a if response_data.len() >= 5 => {
            let float_bytes = response_data[1..5]
                .try_into()
                .expect("length was checked before conversion");
            Ok(f32::from_le_bytes(float_bytes).to_string())
        }
        0x2a => Err(anyhow!("BYOND returned a truncated float Topic response")),
        _ => Err(anyhow!(
            "BYOND returned an unknown Topic response type: 0x{response_type:02x}"
        )),
    }
}
