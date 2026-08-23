use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};
use tokio::io::AsyncRead;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::info;

use crate::mcp::ToolResult;

const DEFAULT_IDLE_TIMEOUT_MS: u64 = 45_000;
const MAX_IDLE_TIMEOUT_MS: u64 = 900_000;

#[derive(Debug, PartialEq, Eq)]
enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, PartialEq, Eq)]
struct CompilerDiagnostic {
    file: String,
    line: u32,
    column: Option<u32>,
    severity: DiagnosticSeverity,
    message: String,
}

fn diagnostic_regex() -> &'static Regex {
    static DIAGNOSTIC_REGEX: OnceLock<Regex> = OnceLock::new();
    DIAGNOSTIC_REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)^(?P<file>.*?):(?P<line>\d+)(?::(?P<column>\d+))?:\s*(?P<severity>error|warning)\b\s*:?[\s]*(?P<message>.*)$",
        ).expect("compiler diagnostic regex must be valid")
    })
}

fn parse_diagnostic_line(line: &str) -> Option<CompilerDiagnostic> {
    let captures = diagnostic_regex().captures(line.trim_end())?;
    let severity = match captures
        .name("severity")?
        .as_str()
        .to_ascii_lowercase()
        .as_str()
    {
        "error" => DiagnosticSeverity::Error,
        "warning" => DiagnosticSeverity::Warning,
        _ => return None,
    };

    Some(CompilerDiagnostic {
        file: captures.name("file")?.as_str().to_string(),
        line: captures.name("line")?.as_str().parse().ok()?,
        column: captures
            .name("column")
            .and_then(|value| value.as_str().parse().ok()),
        severity,
        message: captures.name("message")?.as_str().trim().to_string(),
    })
}

fn diagnostic_to_value(diagnostic: &CompilerDiagnostic) -> Value {
    json!({
        "file": diagnostic.file,
        "line": diagnostic.line,
        "column": diagnostic.column,
        "severity": match diagnostic.severity {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
        },
        "message": diagnostic.message,
    })
}

fn compile_succeeded(process_succeeded: bool, error_count: usize) -> bool {
    process_succeeded && error_count == 0
}

#[cfg(windows)]
fn process_cpu_time_100ns(pid: u32) -> Option<u64> {
    use std::mem::MaybeUninit;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }

        let mut creation_time = MaybeUninit::<FILETIME>::uninit();
        let mut exit_time = MaybeUninit::<FILETIME>::uninit();
        let mut kernel_time = MaybeUninit::<FILETIME>::uninit();
        let mut user_time = MaybeUninit::<FILETIME>::uninit();
        let result = GetProcessTimes(
            handle,
            creation_time.as_mut_ptr(),
            exit_time.as_mut_ptr(),
            kernel_time.as_mut_ptr(),
            user_time.as_mut_ptr(),
        );
        CloseHandle(handle);
        if result == 0 {
            return None;
        }

        let kernel_time = kernel_time.assume_init();
        let user_time = user_time.assume_init();
        let to_u64 =
            |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
        Some(to_u64(kernel_time).saturating_add(to_u64(user_time)))
    }
}

#[cfg(not(windows))]
fn process_cpu_time_100ns(_pid: u32) -> Option<u64> {
    None
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

fn compiler_dme_argument(path: &Path, working_directory: &Path) -> String {
    path.strip_prefix(working_directory)
        .ok()
        .filter(|relative_path| !relative_path.as_os_str().is_empty())
        .unwrap_or(path)
        .display()
        .to_string()
}

struct CompilerRun {
    status: Option<ExitStatus>,
    timed_out: bool,
    idle: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[derive(Clone, Copy)]
enum CompilerStream {
    Stdout,
    Stderr,
}

async fn capture_compiler_stream<R>(
    mut stream: R,
    stream_kind: CompilerStream,
    sender: mpsc::Sender<(CompilerStream, Vec<u8>)>,
) where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let bytes_read = match tokio::io::AsyncReadExt::read(&mut stream, &mut buffer).await {
            Ok(0) => break,
            Ok(bytes_read) => bytes_read,
            Err(_) => break,
        };
        if sender
            .send((stream_kind, buffer[..bytes_read].to_vec()))
            .await
            .is_err()
        {
            break;
        }
    }
}

async fn run_compiler(
    compiler: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
    timeout_ms: u64,
    _idle_timeout_ms: u64,
) -> Result<CompilerRun> {
    let mut command = Command::new(compiler);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(working_directory) = working_directory {
        command.current_dir(working_directory);
    }

    let mut child = command.spawn()?;
    let process_id = child.id().unwrap_or_default();
    drop(child.stdin.take());
    let (sender, mut receiver) = mpsc::channel(32);
    let mut reader_tasks = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        reader_tasks.push(tokio::spawn(capture_compiler_stream(
            stdout,
            CompilerStream::Stdout,
            sender.clone(),
        )));
    }
    if let Some(stderr) = child.stderr.take() {
        reader_tasks.push(tokio::spawn(capture_compiler_stream(
            stderr,
            CompilerStream::Stderr,
            sender.clone(),
        )));
    }

    let started_at = tokio::time::Instant::now();
    let mut last_progress = started_at;
    let mut previous_cpu_time = process_cpu_time_100ns(process_id);
    let timeout = Duration::from_millis(timeout_ms);
    let idle_timeout = Duration::from_millis(_idle_timeout_ms);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let (status, timed_out, idle) = loop {
        let now = tokio::time::Instant::now();
        if now.duration_since(started_at) >= timeout {
            let _ = child.kill().await;
            break (child.wait().await.ok(), true, false);
        }

        tokio::select! {
            exit_status = child.wait() => {
                break (exit_status.ok(), false, false);
            }
            output = receiver.recv() => {
                if let Some((stream_kind, bytes)) = output {
                    last_progress = tokio::time::Instant::now();
                    match stream_kind {
                        CompilerStream::Stdout => stdout.extend(bytes),
                        CompilerStream::Stderr => stderr.extend(bytes),
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {}
        }

        if let Some(cpu_time) = process_cpu_time_100ns(process_id) {
            if previous_cpu_time.is_none_or(|previous| cpu_time > previous) {
                last_progress = tokio::time::Instant::now();
            }
            previous_cpu_time = Some(cpu_time);
        }

        if tokio::time::Instant::now().duration_since(last_progress) >= idle_timeout {
            let _ = child.kill().await;
            break (child.wait().await.ok(), false, true);
        }
    };

    drop(sender);
    while let Some((stream_kind, bytes)) = receiver.recv().await {
        match stream_kind {
            CompilerStream::Stdout => stdout.extend(bytes),
            CompilerStream::Stderr => stderr.extend(bytes),
        }
    }
    for reader_task in reader_tasks {
        let _ = reader_task.await;
    }

    Ok(CompilerRun {
        status,
        timed_out,
        idle,
        stdout,
        stderr,
    })
}

#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn relative_dme_paths_resolve_from_requested_working_directory() {
        let resolved = resolve_requested_path(
            Path::new("project/tgstation.dme"),
            Some(Path::new("workspace")),
        );

        assert_eq!(resolved, PathBuf::from("workspace/project/tgstation.dme"));
    }

    #[test]
    fn diagnostic_parser_handles_windows_paths_and_columns() {
        let diagnostic =
            parse_diagnostic_line(r"C:\workspace\code\example.dm:42:7: error: unexpected token")
                .expect("diagnostic should parse");

        assert_eq!(diagnostic.file, r"C:\workspace\code\example.dm");
        assert_eq!(diagnostic.line, 42);
        assert_eq!(diagnostic.column, Some(7));
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.message, "unexpected token");
    }

    #[test]
    fn diagnostic_parser_handles_line_only_warnings() {
        let diagnostic = parse_diagnostic_line("code/example.dm:9: warning: deprecated syntax")
            .expect("diagnostic should parse");

        assert_eq!(diagnostic.file, "code/example.dm");
        assert_eq!(diagnostic.line, 9);
        assert_eq!(diagnostic.column, None);
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostic.message, "deprecated syntax");
    }

    #[test]
    fn diagnostic_parser_ignores_unstructured_output() {
        assert!(parse_diagnostic_line("DreamMaker finished with 0 errors").is_none());
    }

    #[test]
    fn compiler_diagnostics_override_a_zero_process_exit_code() {
        assert!(compile_succeeded(true, 0));
        assert!(!compile_succeeded(true, 1));
        assert!(!compile_succeeded(false, 0));
    }

    #[cfg(windows)]
    #[test]
    fn compiler_spawn_paths_drop_windows_verbatim_prefix() {
        let verbatim = Path::new(r"\\?\C:\workspace\tgstation.dme");
        assert_eq!(
            normalize_spawn_path(verbatim),
            PathBuf::from(r"C:\workspace\tgstation.dme")
        );

        let unc_verbatim = Path::new(r"\\?\UNC\server\share\tgstation.dme");
        assert_eq!(
            normalize_spawn_path(unc_verbatim),
            PathBuf::from(r"\\server\share\tgstation.dme")
        );
    }

    #[test]
    fn compiler_uses_a_dme_path_relative_to_the_spawn_directory() {
        let working_directory = Path::new(r"C:\workspace\project");
        let dme_path = working_directory.join("tgstation.dme");

        assert_eq!(
            compiler_dme_argument(&dme_path, working_directory),
            "tgstation.dme"
        );
    }
}

/// Find the DreamMaker compiler
fn find_dm_compiler() -> Option<PathBuf> {
    // Try common locations
    let possible_paths = [
        // Windows BYOND installation
        r"C:\Program Files (x86)\BYOND\bin\dm.exe",
        r"C:\Program Files\BYOND\bin\dm.exe",
        // Linux/WSL
        "/usr/local/byond/bin/DreamMaker",
        "/opt/byond/bin/DreamMaker",
    ];

    for path in &possible_paths {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    // Try PATH
    if let Ok(path) = which::which("dm") {
        return Some(path);
    }
    if let Ok(path) = which::which("DreamMaker") {
        return Some(path);
    }

    None
}

fn resolve_requested_path(requested_path: &Path, working_directory: Option<&Path>) -> PathBuf {
    if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else if let Some(working_directory) = working_directory {
        working_directory.join(requested_path)
    } else {
        requested_path.to_path_buf()
    }
}

/// Compile a DreamMaker environment
pub async fn compile(args: Value) -> Result<ToolResult> {
    let dme_path = args
        .get("dme_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing dme_path argument"))?;

    let requested_path = PathBuf::from(dme_path);
    let requested_working_directory = args
        .get("working_directory")
        .and_then(|value| value.as_str())
        .map(PathBuf::from);
    let path = resolve_requested_path(&requested_path, requested_working_directory.as_deref());
    if !path.exists() {
        return Ok(ToolResult::error(format!("File not found: {dme_path}")));
    }
    let path = path.canonicalize()?;

    let compiler = args
        .get("compiler_path")
        .and_then(|value| value.as_str())
        .map(PathBuf::from)
        .or_else(find_dm_compiler)
        .ok_or_else(|| anyhow!("DreamMaker compiler not found. Please install BYOND."))?;

    let timeout_ms = args
        .get("timeout_ms")
        .and_then(|value| value.as_u64())
        .unwrap_or(600_000)
        .min(1_800_000);
    let idle_timeout_ms = args
        .get("idle_timeout_ms")
        .and_then(|value| value.as_u64())
        .unwrap_or(DEFAULT_IDLE_TIMEOUT_MS)
        .clamp(1_000, MAX_IDLE_TIMEOUT_MS);

    let working_directory = requested_working_directory
        .map(|directory| resolve_requested_path(&directory, None))
        .map(|directory| directory.canonicalize())
        .transpose()?
        .or_else(|| path.parent().map(PathBuf::from));

    let defines: Vec<String> = args
        .get("defines")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(|define| {
                    if define.starts_with("-D") {
                        define.to_string()
                    } else {
                        format!("-D{define}")
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    info!(
        "Compiling {} with {:?} (timeout {} ms)",
        dme_path, compiler, timeout_ms
    );

    let started_at = SystemTime::now();
    let spawn_path = normalize_spawn_path(&path);
    let spawn_working_directory = working_directory.as_deref().map(normalize_spawn_path);
    let compiler_working_directory = spawn_working_directory
        .as_deref()
        .unwrap_or_else(|| spawn_path.parent().unwrap_or(Path::new(".")));
    let dme_argument = compiler_dme_argument(&spawn_path, compiler_working_directory);
    let arguments: Vec<String> = defines
        .into_iter()
        .chain(std::iter::once(dme_argument.clone()))
        .collect();
    let execution = run_compiler(
        &compiler,
        &arguments,
        spawn_working_directory.as_deref(),
        timeout_ms,
        idle_timeout_ms,
    )
    .await?;

    let stdout = String::from_utf8_lossy(&execution.stdout);
    let stderr = String::from_utf8_lossy(&execution.stderr);

    // Parse output for errors/warnings
    let mut errors: Vec<Value> = Vec::new();
    let mut warnings: Vec<Value> = Vec::new();

    for line in stdout.lines().chain(stderr.lines()) {
        if let Some(diagnostic) = parse_diagnostic_line(line) {
            match &diagnostic.severity {
                DiagnosticSeverity::Error => errors.push(diagnostic_to_value(&diagnostic)),
                DiagnosticSeverity::Warning => warnings.push(diagnostic_to_value(&diagnostic)),
            }
        }
    }

    let process_succeeded = execution
        .status
        .as_ref()
        .map(ExitStatus::success)
        .unwrap_or(false);
    let success = !execution.timed_out
        && !execution.idle
        && compile_succeeded(process_succeeded, errors.len());
    let dmb_path = path.with_extension("dmb");
    let dmb_metadata = std::fs::metadata(&dmb_path).ok();
    let dmb_exists = dmb_metadata.is_some();
    let dmb_updated = dmb_metadata
        .and_then(|metadata| metadata.modified().ok())
        .map(|modified| modified >= started_at)
        .unwrap_or(false);

    let result = json!({
        "success": success,
        "timed_out": execution.timed_out,
        "idle": execution.idle,
        "timeout_ms": timeout_ms,
        "idle_timeout_ms": idle_timeout_ms,
        "exit_code": execution.status.and_then(|status| status.code()),
        "dme_argument": dme_argument,
        "spawn_working_directory": compiler_working_directory.display().to_string(),
        "dmb_exists": dmb_exists,
        "dmb_updated": dmb_updated,
        "dmb_path": if dmb_exists { Some(dmb_path.display().to_string()) } else { None },
        "compiler": compiler.display().to_string(),
        "working_directory": working_directory.map(|directory| directory.display().to_string()),
        "defines": args.get("defines").cloned().unwrap_or_else(|| json!([])),
        "errors": errors,
        "warnings": warnings,
        "stdout": stdout.to_string(),
        "stderr": stderr.to_string()
    });

    if success {
        Ok(ToolResult::text(serde_json::to_string_pretty(&result)?))
    } else {
        Ok(ToolResult::error(serde_json::to_string_pretty(&result)?))
    }
}
