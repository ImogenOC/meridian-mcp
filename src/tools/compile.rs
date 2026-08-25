use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tracing::info;

use crate::artifact::ArtifactSnapshot;
use crate::mcp::ToolResult;
use crate::process::{run_contained_process, ProcessSpec, TerminationReason};

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

fn compiler_environment() -> Vec<(OsString, OsString)> {
    #[cfg(windows)]
    let names = [
        "SystemRoot",
        "SystemDrive",
        "WINDIR",
        "ComSpec",
        "PATH",
        "PATHEXT",
        "TEMP",
        "TMP",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "ProgramData",
        "LOCALAPPDATA",
        "APPDATA",
        "USERPROFILE",
        "NUMBER_OF_PROCESSORS",
        "PROCESSOR_ARCHITECTURE",
    ];
    #[cfg(not(windows))]
    let names = ["PATH", "HOME", "TMPDIR", "LD_LIBRARY_PATH"];

    names
        .into_iter()
        .filter_map(|name| std::env::var_os(name).map(|value| (name.into(), value)))
        .collect()
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
        .ok_or_else(|| anyhow!("DreamMaker compiler not found. Please install BYOND."))?
        .canonicalize()?;

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

    let spawn_path = normalize_spawn_path(&path);
    let spawn_working_directory = working_directory.as_deref().map(normalize_spawn_path);
    let compiler_working_directory = spawn_working_directory
        .as_deref()
        .unwrap_or_else(|| spawn_path.parent().unwrap_or(Path::new(".")));
    let dme_argument = compiler_dme_argument(&spawn_path, compiler_working_directory);
    let arguments: Vec<OsString> = defines
        .into_iter()
        .map(OsString::from)
        .chain(std::iter::once(OsString::from(&dme_argument)))
        .collect();
    let dmb_path = path.with_extension("dmb");
    let project_root = path
        .parent()
        .ok_or_else(|| anyhow!("DreamMaker environment has no project root"))?;
    let artifact_before = ArtifactSnapshot::capture(project_root, &dmb_path)?;
    let capture_network = args
        .get("capture_network")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let execution = run_contained_process(ProcessSpec {
        program: normalize_spawn_path(&compiler),
        arguments,
        working_directory: compiler_working_directory.to_owned(),
        environment: compiler_environment(),
        stdin: None,
        timeout: Duration::from_millis(timeout_ms),
        idle_timeout: Duration::from_millis(idle_timeout_ms),
        capture_network,
        cancellation: None,
    })
    .await?;

    let stdout = &execution.stdout.text;
    let stderr = &execution.stderr.text;

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

    let process_succeeded =
        execution.termination == TerminationReason::Exited && execution.exit_code == Some(0);
    let success = compile_succeeded(process_succeeded, errors.len());
    let artifact_after = ArtifactSnapshot::capture(project_root, &dmb_path)?;
    let dmb_exists = artifact_after.exists;
    let dmb_updated = artifact_after.exists
        && (!artifact_before.exists
            || artifact_before.sha256 != artifact_after.sha256
            || artifact_before.modified_unix_ms != artifact_after.modified_unix_ms);
    let timed_out = execution.termination == TerminationReason::WallTimeout;
    let idle = execution.termination == TerminationReason::IdleTimeout;

    let result = json!({
        "success": success,
        "timed_out": timed_out,
        "idle": idle,
        "termination": execution.termination,
        "duration_ms": execution.duration_ms,
        "timeout_ms": timeout_ms,
        "idle_timeout_ms": idle_timeout_ms,
        "exit_code": execution.exit_code,
        "dme_argument": dme_argument,
        "spawn_working_directory": compiler_working_directory.display().to_string(),
        "dmb_exists": dmb_exists,
        "dmb_updated": dmb_updated,
        "dmb_path": if dmb_exists { Some(artifact_after.path.display().to_string()) } else { None },
        "artifact_before": artifact_before,
        "artifact_after": artifact_after,
        "compiler": compiler.display().to_string(),
        "working_directory": working_directory.map(|directory| directory.display().to_string()),
        "defines": args.get("defines").cloned().unwrap_or_else(|| json!([])),
        "errors": errors,
        "warnings": warnings,
        "stdout": stdout,
        "stderr": stderr,
        "stdout_truncated_bytes": execution.stdout.truncated_bytes,
        "stderr_truncated_bytes": execution.stderr.truncated_bytes,
        "network_audit": execution.network_audit
    });

    if success {
        Ok(ToolResult::text(serde_json::to_string_pretty(&result)?))
    } else {
        Ok(ToolResult::error(serde_json::to_string_pretty(&result)?))
    }
}
