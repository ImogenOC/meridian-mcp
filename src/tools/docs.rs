use crate::limits::ServerLimits;
use crate::mcp::ToolResult;
use crate::process::{run_contained_process, ProcessSpec, TerminationReason};
use crate::result::{json_success, ToolMetadata};
use crate::state::ServerState;
use crate::tools::ToolExecutionContext;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub async fn generate(
    context: &ToolExecutionContext,
    state: &ServerState,
    args: Value,
) -> Result<ToolResult> {
    let helper = context
        .dmdoc_helper()
        .ok_or_else(|| anyhow!("dmdoc helper unavailable"))?;
    let snapshot = state.snapshot().await?;
    let output = PathBuf::from(
        args.get("output_directory")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Missing output_directory"))?,
    );
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let parent = output
        .parent()
        .ok_or_else(|| anyhow!("output directory has no parent"))?;
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|error| anyhow!(error.to_string()))?;
    let temporary = parent.join(format!(
        ".meridian-mcp-dmdoc-{}",
        random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ));
    std::fs::create_dir(&temporary)?;
    let limits = ServerLimits::default();
    let outcome = run_contained_process(ProcessSpec {
        program: helper.to_owned(),
        arguments: vec![
            "-e".into(),
            snapshot.environment_path.as_os_str().to_owned(),
            "--output".into(),
            temporary.as_os_str().to_owned(),
        ],
        working_directory: environment_root(&snapshot.environment_path)?.to_owned(),
        environment: Vec::new(),
        timeout: Duration::from_millis(limits.max_docs_duration_ms),
        idle_timeout: Duration::from_millis(limits.max_docs_duration_ms),
        capture_network: false,
    })
    .await?;
    if outcome.termination != TerminationReason::Exited || outcome.exit_code != Some(0) {
        let _ = std::fs::remove_dir_all(&temporary);
        return Err(anyhow!(
            "dmdoc failed ({:?}, exit {:?}): {}",
            outcome.termination,
            outcome.exit_code,
            outcome.stderr.text
        ));
    }
    if !temporary.join("index.html").is_file() {
        let _ = std::fs::remove_dir_all(&temporary);
        return Err(anyhow!("dmdoc did not produce index.html"));
    }
    let (files, bytes) = match directory_stats(&temporary, &limits) {
        Ok(stats) => stats,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&temporary);
            return Err(error);
        }
    };
    install_directory(&temporary, &output, overwrite)?;
    Ok(json_success(
        ToolMetadata::complete(Some(snapshot.generation)),
        json!({
            "output_directory":output,
            "files":files,
            "bytes":bytes,
            "index":output.join("index.html"),
            "helper":helper,
            "source_revision":snapshot.spacemandmm_revision,
            "duration_ms":outcome.duration_ms,
            "stdout":outcome.stdout.text,
            "stderr":outcome.stderr.text,
            "truncated":outcome.stdout.truncated_bytes != 0 || outcome.stderr.truncated_bytes != 0
        }),
    ))
}

fn environment_root(environment: &Path) -> Result<&Path> {
    environment
        .parent()
        .ok_or_else(|| anyhow!("parsed environment has no parent directory"))
}

fn install_directory(temporary: &Path, output: &Path, overwrite: bool) -> Result<()> {
    if output.exists() && !overwrite {
        return Err(anyhow!("output exists; set overwrite=true"));
    }
    let parent = output
        .parent()
        .ok_or_else(|| anyhow!("output directory has no parent"))?;
    let backup = if output.exists() {
        let backup = private_directory_path(parent, "backup")?;
        std::fs::rename(output, &backup)?;
        Some(backup)
    } else {
        None
    };
    if let Err(error) = std::fs::rename(temporary, output) {
        if let Some(backup) = &backup {
            let _ = std::fs::rename(backup, output);
        }
        return Err(error.into());
    }
    if let Some(backup) = backup {
        std::fs::remove_dir_all(backup)?
    }
    Ok(())
}
fn private_directory_path(parent: &Path, purpose: &str) -> Result<PathBuf> {
    for _ in 0..32 {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|error| anyhow!(error.to_string()))?;
        let suffix = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = parent.join(format!(".meridian-mcp-dmdoc-{suffix}.{purpose}"));
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(anyhow!("could not allocate a private backup directory"))
}

fn directory_stats(root: &Path, limits: &ServerLimits) -> Result<(usize, u64)> {
    let mut stack = vec![root.to_owned()];
    let mut files = 0;
    let mut bytes = 0;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                return Err(anyhow!("dmdoc output contains a symbolic link"));
            }
            if file_type.is_dir() {
                stack.push(entry.path())
            } else if file_type.is_file() {
                files += 1;
                bytes += entry.metadata()?.len();
                if files > limits.max_docs_files {
                    return Err(anyhow!("dmdoc output exceeds max_docs_files"));
                }
                if bytes > limits.max_docs_output_bytes {
                    return Err(anyhow!("dmdoc output exceeds max_docs_output_bytes"));
                }
            } else {
                return Err(anyhow!("dmdoc output contains an unsupported file type"));
            }
        }
    }
    Ok((files, bytes))
}
