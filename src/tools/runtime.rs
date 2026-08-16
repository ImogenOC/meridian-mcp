use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tracing::info;

const DEFAULT_OUTPUT_WAIT_TIMEOUT_MS: u64 = 30_000;
const MAX_OUTPUT_WAIT_TIMEOUT_MS: u64 = 300_000;
const OUTPUT_WAIT_POLL_INTERVAL_MS: u64 = 100;
const OUTPUT_READ_CHUNK_BYTES: usize = 8 * 1024;

use crate::client::BYONDClient;
use crate::mcp::ToolResult;
use crate::state::{OutputLog, ServerState, OUTPUT_LINE_MAX_BYTES, OUTPUT_TRUNCATED_SUFFIX};

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

/// Start DreamDaemon with a .dmb file
pub async fn run(state: &mut ServerState, args: Value) -> Result<ToolResult> {
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

    let path = PathBuf::from(dmb_path);
    if !path.exists() {
        return Ok(ToolResult::error(format!(
            "DMB file not found: {}",
            dmb_path
        )));
    }

    let dreamdaemon = find_dreamdaemon()
        .ok_or_else(|| anyhow!("DreamDaemon not found. Please install BYOND."))?;

    info!("Starting DreamDaemon with {} on port {}", dmb_path, port);
    state.clear_runtime_diagnostics();

    // Start DreamDaemon
    // Arguments: dmb_path port -trusted -logself
    // Note: Removed -invisible so the server auto-starts properly
    let mut child = Command::new(&dreamdaemon)
        .arg(&path)
        .arg(port.to_string())
        .arg("-trusted")
        .arg("-logself")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let pid = child.id();

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    state.set_game_process(child, port);

    if let Some(stdout) = stdout {
        let output_log = Arc::clone(&state.output_log);
        tokio::spawn(async move {
            capture_output_stream(stdout, output_log).await;
        });
    }

    if let Some(stderr) = stderr {
        let output_log = Arc::clone(&state.output_log);
        tokio::spawn(async move {
            capture_output_stream(stderr, output_log).await;
        });
    }

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
        let wait_result = wait_for_output_value(state, pattern, use_regex, timeout_ms).await?;
        result["readiness"] = wait_result;
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
    state: &mut ServerState,
    pattern: &str,
    use_regex: bool,
    timeout_ms: u64,
) -> Result<Value> {
    let timeout_ms = timeout_ms.min(MAX_OUTPUT_WAIT_TIMEOUT_MS);
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        let matched = state
            .matches_output(pattern, use_regex)
            .map_err(|error| anyhow!("Invalid output regex: {}", error))?;
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
pub async fn wait_for_output(state: &mut ServerState, args: Value) -> Result<ToolResult> {
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

    let result = wait_for_output_value(state, pattern, use_regex, timeout_ms).await?;
    Ok(ToolResult::text(result.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::ToolContent;
    use crate::state::push_output_line;

    #[tokio::test]
    async fn wait_for_output_can_match_retained_output_after_exit() {
        let mut state = ServerState::new();
        state.last_exit_code = Some(1);
        push_output_line(
            &state.output_log,
            "fatal: initialization failed".to_string(),
        );

        let result = wait_for_output(
            &mut state,
            json!({"pattern": "initialization failed", "timeout_ms": 10}),
        )
        .await
        .expect("waiting for retained output should succeed");

        assert_eq!(result.is_error, None);
        let ToolContent::Text { text } = &result.content[0] else {
            panic!("expected a text tool result");
        };
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
}

/// Stop the running DreamDaemon instance
pub async fn stop(state: &mut ServerState, _args: Value) -> Result<ToolResult> {
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
            "Failed to stop DreamDaemon: {}",
            e
        ))),
    }
}

/// Get status of the running game
pub async fn status(state: &mut ServerState, _args: Value) -> Result<ToolResult> {
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
pub async fn topic(state: &mut ServerState, args: Value) -> Result<ToolResult> {
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

    match send_topic(&format!("127.0.0.1:{}", port), topic_string, timeout_ms).await {
        Ok(response) => Ok(ToolResult::text(
            json!({
                "success": true,
                "response": response
            })
            .to_string(),
        )),
        Err(e) => Ok(ToolResult::error(format!("Topic call failed: {}", e))),
    }
}

/// Send a BYOND Topic packet and get response
async fn send_topic(address: &str, topic: &str, timeout_ms: u64) -> Result<String> {
    // BYOND Topic packet format:
    // Bytes 0-1: 0x00 0x83 (magic header)
    // Bytes 2-3: Length of data (big-endian u16)
    // Bytes 4+: The topic string as null-terminated ASCII

    // Strip leading "?" if present - it's a URL delimiter, not part of the topic
    let topic_clean = topic.strip_prefix('?').unwrap_or(topic);
    let topic_bytes = topic_clean.as_bytes();

    info!(
        "Sending topic packet for: {} (cleaned: {})",
        topic, topic_clean
    );
    let packet_len = topic_bytes.len() + 6; // +1 for null terminator, +5 for header overhead

    let mut packet = Vec::with_capacity(packet_len);
    packet.push(0x00); // Magic byte 1
    packet.push(0x83); // Magic byte 2

    // Length (big-endian u16) - includes 5 padding bytes + 1 type byte + string + null terminator
    let data_len = (topic_bytes.len() + 7) as u16;
    packet.push((data_len >> 8) as u8);
    packet.push((data_len & 0xFF) as u8);

    // Type: 0x06 for string topic
    packet.push(0x00);
    packet.push(0x00);
    packet.push(0x00);
    packet.push(0x00);
    packet.push(0x00);
    packet.push(0x06); // String type

    // The topic string
    packet.extend_from_slice(topic_bytes);
    packet.push(0x00); // Null terminator

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

    // Response format depends on type byte
    if response_data.is_empty() {
        return Ok(String::new());
    }

    // Skip type indicator and parse as string
    let response_str = if response_data.len() > 1 {
        // Check response type
        match response_data[0] {
            0x06 => {
                // ASCII string - skip type byte, read until null
                let string_data = &response_data[1..];
                let null_pos = string_data
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(string_data.len());
                String::from_utf8_lossy(&string_data[..null_pos]).to_string()
            }
            0x2a => {
                // Float response
                if response_data.len() >= 5 {
                    let float_bytes: [u8; 4] = [
                        response_data[1],
                        response_data[2],
                        response_data[3],
                        response_data[4],
                    ];
                    let value = f32::from_le_bytes(float_bytes);
                    value.to_string()
                } else {
                    String::new()
                }
            }
            _ => {
                // Unknown type, return hex representation
                format!("raw:{:02x?}", response_data)
            }
        }
    } else {
        String::new()
    };

    Ok(response_str)
}

/// Test connecting to the BYOND server as a client and log received packets
pub async fn connect_test(state: &mut ServerState, args: Value) -> Result<ToolResult> {
    if !state.is_game_running() {
        return Ok(ToolResult::error("No game instance is currently running."));
    }

    let port = state.game_port.unwrap_or(1337);
    let timeout_secs = args
        .get("timeout_secs")
        .and_then(|v| v.as_u64())
        .unwrap_or(5);

    info!("Testing BYOND client connection to port {}", port);

    // Run the connection in a blocking task since it uses sync I/O
    let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value> {
        // Connect to the server
        let mut client = BYONDClient::connect("127.0.0.1", port)?;

        // Receive initial packets
        let packets = client.receive_initial_packets(timeout_secs)?;

        // Build summary of received packets
        let packet_summary: Vec<serde_json::Value> = packets
            .iter()
            .map(|p| {
                json!({
                    "type": format!("0x{:04X}", u16::from(p.packet_type)),
                    "type_name": format!("{:?}", p.packet_type),
                    "seq": p.seq,
                    "data_len": p.data.len(),
                    "data_preview": if p.data.len() <= 64 {
                        format!("{:02X?}", p.data)
                    } else {
                        format!("{:02X?}... ({} more bytes)", &p.data[..64], p.data.len() - 64)
                    }
                })
            })
            .collect();

        client.disconnect()?;

        Ok(json!({
            "success": true,
            "packets_received": packets.len(),
            "packets": packet_summary
        }))
    })
    .await??;

    Ok(ToolResult::text(result.to_string()))
}
