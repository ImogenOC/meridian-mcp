use crate::process::{run_contained_process, ProcessOutcome, ProcessSpec, TerminationReason};
use serde::Serialize;
use serde_json::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::watch;

pub const TRACY_HELPER_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TracyCommand {
    Capture,
    Hotspots,
    Zone,
    FrameStats,
    Compare,
}

impl TracyCommand {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Hotspots => "hotspots",
            Self::Zone => "zone",
            Self::FrameStats => "frame_stats",
            Self::Compare => "compare",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TracyProtocolError {
    #[error("helper params must be a JSON object")]
    InvalidParams,
    #[error("helper request serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("helper produced multiple response lines")]
    MultipleResponses,
    #[error("helper produced an empty response")]
    EmptyResponse,
    #[error("helper response schema mismatch")]
    ResponseSchema,
    #[error("helper response id mismatch: expected {expected}, found {actual}")]
    ResponseId { expected: u64, actual: u64 },
    #[error("helper response is missing a result")]
    MissingResult,
    #[error("helper failed with {code}: {message}")]
    Helper { code: String, message: String },
    #[error("helper process did not exit successfully: {0:?}")]
    Process(TerminationReason),
    #[error(transparent)]
    Runner(#[from] anyhow::Error),
}

#[derive(Debug)]
pub struct TracyInvocation {
    pub result: Value,
    pub process: ProcessOutcome,
}

pub struct TracyInvocationSpec<'a> {
    pub helper: &'a Path,
    pub working_directory: &'a Path,
    pub id: u64,
    pub command: TracyCommand,
    pub params: Value,
    pub timeout: Duration,
    pub capture_network: bool,
    pub environment: Vec<(OsString, OsString)>,
    pub cancellation: Option<watch::Receiver<bool>>,
}

pub fn build_request(
    id: u64,
    command: TracyCommand,
    params: Value,
) -> Result<Vec<u8>, TracyProtocolError> {
    if !params.is_object() {
        return Err(TracyProtocolError::InvalidParams);
    }
    let mut request = serde_json::to_vec(&serde_json::json!({
        "schema_version": TRACY_HELPER_SCHEMA_VERSION,
        "id": id,
        "command": command,
        "params": params,
    }))?;
    request.push(b'\n');
    Ok(request)
}

pub fn parse_response(id: u64, bytes: &[u8]) -> Result<Value, TracyProtocolError> {
    let mut response = bytes;
    if response.ends_with(b"\n") {
        response = &response[..response.len() - 1];
    }
    if response.ends_with(b"\r") {
        response = &response[..response.len() - 1];
    }
    if response.is_empty() {
        return Err(TracyProtocolError::EmptyResponse);
    }
    if response.contains(&b'\n') || response.contains(&b'\r') {
        return Err(TracyProtocolError::MultipleResponses);
    }
    let document: Value = serde_json::from_slice(response)?;
    if document.get("schema_version").and_then(Value::as_u64)
        != Some(TRACY_HELPER_SCHEMA_VERSION.into())
    {
        return Err(TracyProtocolError::ResponseSchema);
    }
    let actual = document
        .get("id")
        .and_then(Value::as_u64)
        .ok_or(TracyProtocolError::ResponseSchema)?;
    if actual != id {
        return Err(TracyProtocolError::ResponseId {
            expected: id,
            actual,
        });
    }
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
        });
    }
    document
        .get("result")
        .cloned()
        .ok_or(TracyProtocolError::MissingResult)
}

pub async fn invoke_helper(
    spec: TracyInvocationSpec<'_>,
) -> Result<TracyInvocation, TracyProtocolError> {
    let request = build_request(spec.id, spec.command, spec.params)?;
    let process = run_contained_process(ProcessSpec {
        program: PathBuf::from(spec.helper),
        arguments: Vec::new(),
        working_directory: spec.working_directory.to_owned(),
        environment: spec.environment,
        stdin: Some(request),
        timeout: spec.timeout,
        idle_timeout: spec.timeout,
        capture_network: spec.capture_network,
        cancellation: spec.cancellation,
    })
    .await?;
    if process.termination != TerminationReason::Exited {
        return Err(TracyProtocolError::Process(process.termination));
    }
    let result = parse_response(spec.id, process.stdout.text.as_bytes())?;
    if process.exit_code != Some(0) {
        return Err(TracyProtocolError::Process(process.termination));
    }
    Ok(TracyInvocation { result, process })
}
