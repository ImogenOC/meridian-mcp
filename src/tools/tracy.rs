use super::ToolExecutionContext;
use crate::atomic_output::{reserve_external_atomic, write_atomic, OutputArtifact};
use crate::mcp::ToolResult;
use crate::result::{json_success, ToolMetadata};
use crate::tracy_protocol::{invoke_helper, TracyCommand, TracyInvocationSpec};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
use std::time::Duration;
use tokio::sync::watch;

pub async fn prepare(context: &ToolExecutionContext, args: Value) -> Result<ToolResult> {
    let installation = context
        .tracy()
        .ok_or_else(|| anyhow!("Tracy installation unavailable"))?;
    let dmb_path = Path::new(
        args.get("dmb_path")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("dmb_path is required"))?,
    );
    let parent = dmb_path
        .parent()
        .ok_or_else(|| anyhow!("dmb_path has no parent"))?;
    let hook_name = installation
        .hook
        .path
        .file_name()
        .ok_or_else(|| anyhow!("verified hook has no file name"))?;
    let destination = parent.join(hook_name);
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if destination.exists()
        && hash_file(&destination)?.eq_ignore_ascii_case(&installation.hook.sha256)
    {
        let artifact = OutputArtifact {
            path: destination.canonicalize()?,
            bytes: std::fs::metadata(&destination)?.len(),
            sha256: installation.hook.sha256.clone(),
        };
        return Ok(json_success(
            ToolMetadata::complete(None),
            json!({"state":"already_prepared","artifact":artifact,"source_revision":installation.hook.source_revision,"protocol_version":installation.hook.protocol_version}),
        ));
    }

    let source = installation.hook.path.clone();
    let artifact = write_atomic(context.policy(), &destination, overwrite, |output| {
        let mut input = std::fs::File::open(&source)?;
        std::io::copy(&mut input, output)?;
        Ok(())
    })?;
    Ok(json_success(
        ToolMetadata::complete(None),
        json!({"state":"prepared","artifact":artifact,"source_revision":installation.hook.source_revision,"protocol_version":installation.hook.protocol_version}),
    ))
}

pub async fn launch(
    context: &ToolExecutionContext,
    state: &crate::state::ServerState,
    args: Value,
) -> Result<ToolResult> {
    let installation = context
        .tracy()
        .ok_or_else(|| anyhow!("Tracy installation unavailable"))?;
    let dmb_path = Path::new(required_string(&args, "dmb_path")?);
    let hook_name = installation
        .hook
        .path
        .file_name()
        .ok_or_else(|| anyhow!("verified hook has no file name"))?;
    let hook_path = dmb_path
        .parent()
        .ok_or_else(|| anyhow!("dmb_path has no parent"))?
        .join(hook_name);
    if !hook_path.is_file()
        || !hash_file(&hook_path)?.eq_ignore_ascii_case(&installation.hook.sha256)
    {
        return Err(anyhow!(
            "the verified byond-tracy hook is not prepared beside the DMB; call dm_tracy_prepare"
        ));
    }
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let profiler_port = listener.local_addr()?.port();
    drop(listener);
    let game_port = args
        .get("game_port")
        .and_then(Value::as_u64)
        .unwrap_or(1337) as u16;
    super::runtime::run_profiled(
        state,
        json!({"dmb_path":dmb_path,"port":game_port}),
        profiler_port,
    )
    .await
}

pub async fn capture(
    context: &ToolExecutionContext,
    state: &crate::state::ServerState,
    args: Value,
) -> Result<ToolResult> {
    let installation = context
        .tracy()
        .ok_or_else(|| anyhow!("Tracy installation unavailable"))?;
    let profiler_port = {
        let mut runtime = state.runtime().await;
        if !runtime.is_game_running() || runtime.kind != Some(crate::state::RuntimeKind::Tracy) {
            return Err(anyhow!("no MCP-owned Tracy runtime is active"));
        }
        runtime
            .profiler_port
            .ok_or_else(|| anyhow!("active Tracy runtime has no profiler endpoint"))?
    };
    let duration_ms = args
        .get("duration_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("duration_ms is required"))?;
    let memory_limit_mb = args
        .get("memory_limit_mb")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("memory_limit_mb is required"))?;
    if !(1..=300_000).contains(&duration_ms) || !(16..=4096).contains(&memory_limit_mb) {
        return Err(anyhow!(
            "capture duration or memory limit is outside the permitted range"
        ));
    }
    let output_path = Path::new(required_string(&args, "output_path")?);
    let overwrite = args
        .get("overwrite")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let reserved = reserve_external_atomic(context.policy(), output_path, overwrite)?;
    let temporary_path = reserved.temporary_path().to_owned();
    let (cancel_sender, cancel_receiver) = watch::channel(false);
    {
        let mut capture = state.tracy_capture().await;
        if capture.active {
            return Err(anyhow!("a Tracy capture is already active"));
        }
        capture.active = true;
        capture.cancellation = Some(cancel_sender);
        capture.output_path = Some(output_path.to_owned());
        capture.last_error = None;
    }
    let capture_network = args
        .get("capture_network")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let invocation = invoke_helper(TracyInvocationSpec {
        helper: &installation.helper.path,
        working_directory: output_path
            .parent()
            .ok_or_else(|| anyhow!("output_path has no parent"))?,
        id: 1,
        command: TracyCommand::Capture,
        params: json!({
            "port":profiler_port,
            "duration_ms":duration_ms,
            "memory_limit_mb":memory_limit_mb,
            "output_path":temporary_path,
        }),
        timeout: Duration::from_millis(duration_ms.saturating_add(30_000)),
        capture_network,
        environment: crate::process_environment::minimal_runtime_environment()
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
        cancellation: Some(cancel_receiver),
    })
    .await;
    {
        let mut capture = state.tracy_capture().await;
        capture.active = false;
        capture.cancellation = None;
        if let Err(error) = &invocation {
            capture.last_error = Some(error.to_string());
        }
    }
    let invocation = invocation?;
    let artifact = reserved.commit()?;
    Ok(json_success(
        ToolMetadata::complete(None),
        json!({
            "artifact":artifact,
            "capture":invocation.result,
            "network_audit":invocation.process.network_audit,
            "helper_revision":installation.helper.source_revision,
            "protocol_version":installation.helper.protocol_version,
        }),
    ))
}

pub async fn status(state: &crate::state::ServerState) -> Result<ToolResult> {
    let (running, kind, game_port, profiler_port, pid, last_exit_code, recent_output) = {
        let mut runtime = state.runtime().await;
        let running = runtime.is_game_running();
        (
            running,
            runtime.kind,
            runtime.game_port,
            runtime.profiler_port,
            runtime
                .game_process
                .as_ref()
                .and_then(|process| process.id()),
            runtime.last_exit_code,
            runtime.recent_output(50),
        )
    };
    let capture = state.tracy_capture().await;
    Ok(json_success(
        ToolMetadata::complete(None),
        json!({
            "running":running,
            "runtime_kind":kind,
            "game_port":game_port,
            "profiler_port":profiler_port,
            "pid":pid,
            "last_exit_code":last_exit_code,
            "recent_output":recent_output,
            "capture_active":capture.active,
            "capture_output_path":capture.output_path,
            "last_capture_error":capture.last_error,
        }),
    ))
}

pub async fn stop(state: &crate::state::ServerState) -> Result<ToolResult> {
    if let Some(sender) = state.tracy_capture().await.cancellation.clone() {
        let _ = sender.send(true);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while state.tracy_capture().await.active && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    let mut runtime = state.runtime().await;
    if !runtime.is_game_running() || runtime.kind != Some(crate::state::RuntimeKind::Tracy) {
        return Err(anyhow!("no MCP-owned Tracy runtime is active"));
    }
    runtime.stop_game_process().await?;
    Ok(json_success(
        ToolMetadata::complete(None),
        json!({"lifecycle":"stopped"}),
    ))
}

pub async fn hotspots(
    context: &ToolExecutionContext,
    state: &crate::state::ServerState,
    args: Value,
) -> Result<ToolResult> {
    invoke_analysis(
        context,
        TracyCommand::Hotspots,
        json!({
            "trace_path": required_string(&args, "trace_path")?,
            "limit": args.get("limit").and_then(Value::as_u64).unwrap_or(100),
            "sort": args.get("sort").and_then(Value::as_str).unwrap_or("inclusive"),
        }),
        Duration::from_secs(120),
        state,
    )
    .await
}

pub async fn zone(
    context: &ToolExecutionContext,
    state: &crate::state::ServerState,
    args: Value,
) -> Result<ToolResult> {
    invoke_analysis(
        context,
        TracyCommand::Zone,
        json!({
            "trace_path": required_string(&args, "trace_path")?,
            "name": required_string(&args, "name")?,
            "limit": args.get("limit").and_then(Value::as_u64).unwrap_or(100),
        }),
        Duration::from_secs(120),
        state,
    )
    .await
}

pub async fn frame_stats(
    context: &ToolExecutionContext,
    state: &crate::state::ServerState,
    args: Value,
) -> Result<ToolResult> {
    invoke_analysis(
        context,
        TracyCommand::FrameStats,
        json!({"trace_path":required_string(&args, "trace_path")?}),
        Duration::from_secs(120),
        state,
    )
    .await
}

pub async fn compare(
    context: &ToolExecutionContext,
    state: &crate::state::ServerState,
    args: Value,
) -> Result<ToolResult> {
    invoke_analysis(
        context,
        TracyCommand::Compare,
        json!({
            "baseline_path": required_string(&args, "baseline_path")?,
            "current_path": required_string(&args, "current_path")?,
            "minimum_delta_ns": args.get("minimum_delta_ns").and_then(Value::as_u64).unwrap_or(0),
            "limit": args.get("limit").and_then(Value::as_u64).unwrap_or(100),
        }),
        Duration::from_secs(180),
        state,
    )
    .await
}

async fn invoke_analysis(
    context: &ToolExecutionContext,
    command: TracyCommand,
    params: Value,
    timeout: Duration,
    state: &crate::state::ServerState,
) -> Result<ToolResult> {
    let installation = context
        .tracy()
        .ok_or_else(|| anyhow!("Tracy installation unavailable"))?;
    let working_directory = context
        .policy()
        .workspace_roots()
        .first()
        .ok_or_else(|| anyhow!("workspace root unavailable"))?;
    let invocation = invoke_helper(TracyInvocationSpec {
        helper: &installation.helper.path,
        working_directory,
        id: 1,
        command,
        params,
        timeout,
        capture_network: false,
        environment: Vec::new(),
        cancellation: None,
    })
    .await?;
    let mut result = invocation.result;
    let object = result
        .as_object_mut()
        .ok_or_else(|| anyhow!("Tracy helper result must be an object"))?;
    object.insert(
        "helper_revision".into(),
        Value::String(installation.helper.source_revision.clone()),
    );
    object.insert(
        "protocol_version".into(),
        json!(installation.helper.protocol_version),
    );
    correlate_sources(state, &mut result).await;
    Ok(json_success(ToolMetadata::complete(None), result))
}

async fn correlate_sources(state: &crate::state::ServerState, result: &mut Value) {
    let Some(snapshot) = state.active_snapshot().await else {
        return;
    };
    let Some(items) = result.get_mut("items").and_then(Value::as_array_mut) else {
        return;
    };
    let root = snapshot
        .environment_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    for item in items {
        let Some(file) = item.get("file").and_then(Value::as_str) else {
            continue;
        };
        let Some(line) = item.get("line").and_then(Value::as_u64) else {
            continue;
        };
        let reported = Path::new(file);
        let candidate = if reported.is_absolute() {
            reported.to_owned()
        } else {
            root.join(reported)
        };
        let Ok(source_path) = candidate.canonicalize() else {
            continue;
        };
        if !source_path.starts_with(root) {
            continue;
        }
        if let Some(object) = item.as_object_mut() {
            object.insert(
                "source_correlation".into(),
                json!({
                    "match":"file_line",
                    "path":source_path,
                    "line":line,
                    "state_generation":snapshot.generation,
                }),
            );
        }
    }
}

fn required_string<'a>(args: &'a Value, name: &str) -> Result<&'a str> {
    args.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{name} is required"))
}

fn hash_file(path: &Path) -> Result<String> {
    let mut input = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
