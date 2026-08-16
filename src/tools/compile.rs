use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};
use tokio::process::Command;
use tracing::info;

use crate::mcp::ToolResult;

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

#[cfg(test)]
mod tests {
    use super::*;

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

/// Compile a DreamMaker environment
pub async fn compile(args: Value) -> Result<ToolResult> {
    let dme_path = args
        .get("dme_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing dme_path argument"))?;

    let path = PathBuf::from(dme_path);
    if !path.exists() {
        return Ok(ToolResult::error(format!("File not found: {}", dme_path)));
    }

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

    let working_directory = args
        .get("working_directory")
        .and_then(|value| value.as_str())
        .map(PathBuf::from)
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
                        format!("-D{}", define)
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
    let mut command = Command::new(&compiler);
    command
        .args(defines)
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(working_directory) = &working_directory {
        command.current_dir(working_directory);
    }

    let output =
        match tokio::time::timeout(Duration::from_millis(timeout_ms), command.output()).await {
            Ok(output) => output?,
            Err(_) => {
                let result = json!({
                    "success": false,
                    "timed_out": true,
                    "timeout_ms": timeout_ms,
                    "compiler": compiler.display().to_string(),
                    "dme_path": dme_path,
                    "errors": [],
                    "warnings": [],
                });
                return Ok(ToolResult::error(serde_json::to_string_pretty(&result)?));
            }
        };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

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

    let success = output.status.success();
    let dmb_path = path.with_extension("dmb");
    let dmb_metadata = std::fs::metadata(&dmb_path).ok();
    let dmb_exists = dmb_metadata.is_some();
    let dmb_updated = dmb_metadata
        .and_then(|metadata| metadata.modified().ok())
        .map(|modified| modified >= started_at)
        .unwrap_or(false);

    let result = json!({
        "success": success,
        "timed_out": false,
        "exit_code": output.status.code(),
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
