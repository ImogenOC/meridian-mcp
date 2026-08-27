use crate::atomic_output::{
    reserve_external_atomic, AtomicOutputError, OutputArtifact, ReservedExternalOutput,
};
use crate::process_metrics::ProcessRole;
use crate::tracy_experiment::ExperimentIdentity;
use crate::PathPolicy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub struct ReservedTraceSet {
    trace: ReservedExternalOutput,
    sidecar: ReservedExternalOutput,
}

#[derive(Debug, thiserror::Error)]
pub enum TraceSetError {
    #[error(transparent)]
    Atomic(#[from] AtomicOutputError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("paired Tracy outputs do not support overwrite; choose a new capture name")]
    OverwriteUnsupported,
    #[error("sidecar serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("paired output rollback failed; the newly-created trace remains at {0}")]
    Rollback(PathBuf),
}

#[derive(Debug, Serialize)]
pub struct PromotedTraceSet {
    pub trace: OutputArtifact,
    pub sidecar: OutputArtifact,
}

#[derive(Debug, Serialize)]
pub struct DiagnosticTraceSet {
    pub authoritative: bool,
    pub trace: OutputArtifact,
    pub sidecar: OutputArtifact,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawRange {
    pub raw_begin: u64,
    pub raw_end: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceMetadata {
    pub trace_sha256: String,
    pub meridian_mcp_build_id: String,
    pub experiment_identity: ExperimentIdentity,
    pub phase: String,
    pub phase_iteration: u32,
    pub range: RawRange,
    pub trace_range_ns: RawRange,
    pub complete_frames: u64,
    pub partial_frames: u64,
    pub zones: u64,
    pub capture_valid: bool,
    pub queue_saturated: bool,
    pub memory_roles: Vec<ProcessRole>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonMode {
    SameExperimentSamePhase,
    CrossExperiment,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityMismatch {
    pub field: String,
    pub baseline: String,
    pub current: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComparisonCompatibility {
    pub compatible: bool,
    pub mode: ComparisonMode,
    pub checked_fields: Vec<String>,
    pub mismatches: Vec<IdentityMismatch>,
    pub warnings: Vec<String>,
}

pub fn reserve_trace_set(
    policy: &PathPolicy,
    output: &Path,
    overwrite: bool,
) -> Result<ReservedTraceSet, TraceSetError> {
    if overwrite {
        return Err(TraceSetError::OverwriteUnsupported);
    }
    let sidecar = PathBuf::from(format!(
        "{}.meridian.json",
        output.as_os_str().to_string_lossy()
    ));
    Ok(ReservedTraceSet {
        trace: reserve_external_atomic(policy, output, false)?,
        sidecar: reserve_external_atomic(policy, &sidecar, false)?,
    })
}

pub fn read_trace_metadata(trace: &Path) -> Result<Option<TraceMetadata>, TraceSetError> {
    let sidecar = PathBuf::from(format!(
        "{}.meridian.json",
        trace.as_os_str().to_string_lossy()
    ));
    if !sidecar.is_file() {
        return Ok(None);
    }
    let document: serde_json::Value = serde_json::from_slice(&std::fs::read(sidecar)?)?;
    let expected = document
        .get("trace_sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "sidecar is missing trace_sha256",
            ))
        })?;
    let actual = hash_file(trace)?;
    if !expected.eq_ignore_ascii_case(&actual) {
        return Err(TraceSetError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "trace/sidecar hash mismatch",
        )));
    }
    let validation = &document["capture"]["validation"];
    let metadata = TraceMetadata {
        trace_sha256: actual,
        meridian_mcp_build_id: document["meridian_mcp_build"]["build_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        experiment_identity: serde_json::from_value(document["experiment_identity"].clone())?,
        phase: document["phase"].as_str().unwrap_or_default().to_owned(),
        phase_iteration: document["phase_iteration"].as_u64().unwrap_or(0) as u32,
        range: RawRange {
            raw_begin: validation["raw_begin"].as_u64().unwrap_or(0),
            raw_end: validation["raw_end"].as_u64().unwrap_or(0),
        },
        trace_range_ns: RawRange {
            raw_begin: validation["trace_begin_ns"].as_u64().unwrap_or(0),
            raw_end: validation["trace_end_ns"].as_u64().unwrap_or(0),
        },
        complete_frames: validation["complete_frames"].as_u64().unwrap_or(0),
        partial_frames: validation["partial_frames"].as_u64().unwrap_or(0),
        zones: validation["zones"].as_u64().unwrap_or(0),
        capture_valid: validation["valid"].as_bool().unwrap_or(false),
        queue_saturated: validation["queue"]["saturation_count"]
            .as_u64()
            .unwrap_or(0)
            > 0
            || validation["queue"]["dropped_events"].as_u64().unwrap_or(0) > 0,
        memory_roles: document["memory_series"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|series| serde_json::from_value(series["identity"]["role"].clone()).ok())
            .collect(),
    };
    if metadata.meridian_mcp_build_id.is_empty()
        || metadata.phase.is_empty()
        || metadata.phase_iteration == 0
        || metadata.range.raw_end <= metadata.range.raw_begin
        || metadata.trace_range_ns.raw_end <= metadata.trace_range_ns.raw_begin
        || metadata.memory_roles.len() != 2
    {
        return Err(TraceSetError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "sidecar identity or range is invalid",
        )));
    }
    Ok(Some(metadata))
}

pub fn compare_metadata(
    baseline: &TraceMetadata,
    current: &TraceMetadata,
    mode: ComparisonMode,
) -> ComparisonCompatibility {
    let mut checked_fields = Vec::new();
    let mut mismatches = Vec::new();
    let mut check = |field: &str, left: String, right: String| {
        checked_fields.push(field.to_owned());
        if left != right {
            mismatches.push(IdentityMismatch {
                field: field.to_owned(),
                baseline: left,
                current: right,
            });
        }
    };
    check(
        "meridian_mcp_build_id",
        baseline.meridian_mcp_build_id.clone(),
        current.meridian_mcp_build_id.clone(),
    );
    check(
        "executable_identity",
        baseline
            .experiment_identity
            .executable
            .executable_id
            .clone(),
        current.experiment_identity.executable.executable_id.clone(),
    );
    check(
        "workload_identity",
        baseline.experiment_identity.workload.workload_id.clone(),
        current.experiment_identity.workload.workload_id.clone(),
    );
    check("phase", baseline.phase.clone(), current.phase.clone());
    check(
        "memory_roles",
        serde_json::to_string(&baseline.memory_roles).unwrap_or_default(),
        serde_json::to_string(&current.memory_roles).unwrap_or_default(),
    );
    if mode == ComparisonMode::SameExperimentSamePhase {
        check(
            "experiment_id",
            baseline.experiment_identity.experiment_id.clone(),
            current.experiment_identity.experiment_id.clone(),
        );
    }
    ComparisonCompatibility {
        compatible: mismatches.is_empty(),
        mode,
        checked_fields,
        mismatches,
        warnings: Vec::new(),
    }
}

impl ReservedTraceSet {
    pub fn temporary_trace_path(&self) -> &Path {
        self.trace.temporary_path()
    }

    pub fn promote<T: Serialize>(self, sidecar: &T) -> Result<PromotedTraceSet, TraceSetError> {
        let bytes = serde_json::to_vec_pretty(sidecar)?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(self.sidecar.temporary_path())?;
        output.write_all(&bytes)?;
        output.write_all(b"\n")?;
        output.flush()?;
        output.sync_all()?;
        drop(output);

        let trace = self.trace.commit()?;
        match self.sidecar.commit() {
            Ok(sidecar) => Ok(PromotedTraceSet { trace, sidecar }),
            Err(error) => {
                if hash_file(&trace.path).is_ok_and(|hash| hash == trace.sha256)
                    && std::fs::remove_file(&trace.path).is_err()
                {
                    return Err(TraceSetError::Rollback(trace.path));
                }
                Err(error.into())
            }
        }
    }

    pub fn promote_diagnostic<T: Serialize>(
        self,
        policy: &PathPolicy,
        diagnostic_trace: &Path,
        sidecar: &T,
    ) -> Result<DiagnosticTraceSet, TraceSetError> {
        let diagnostic = reserve_trace_set(policy, diagnostic_trace, false)?;
        std::fs::copy(
            self.temporary_trace_path(),
            diagnostic.temporary_trace_path(),
        )?;
        let promoted = diagnostic.promote(sidecar)?;
        Ok(DiagnosticTraceSet {
            authoritative: false,
            trace: promoted.trace,
            sidecar: promoted.sidecar,
        })
    }
}

pub fn validate_capture_result(capture: &serde_json::Value) -> Result<(), Vec<String>> {
    let validation = &capture["validation"];
    let mut errors = validation["error_codes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if validation["valid"].as_bool() != Some(true) && errors.is_empty() {
        push_unique(&mut errors, "invalid_capture");
    }
    match (
        validation["raw_begin"].as_u64(),
        validation["raw_end"].as_u64(),
    ) {
        (Some(begin), Some(end)) if end > begin => {}
        (Some(_), Some(_)) => push_unique(&mut errors, "non_monotonic_raw_clock"),
        _ => push_unique(&mut errors, "missing_raw_range"),
    }
    match (
        validation["trace_begin_ns"].as_u64(),
        validation["trace_end_ns"].as_u64(),
    ) {
        (Some(begin), Some(end)) if end > begin => {}
        (Some(_), Some(_)) => push_unique(&mut errors, "nonpositive_trace_span"),
        _ => push_unique(&mut errors, "missing_trace_range"),
    }
    if validation["complete_frames"].as_u64().unwrap_or(0) == 0 {
        push_unique(&mut errors, "no_complete_frames");
    }
    if validation["zones"].as_u64().unwrap_or(0) == 0 {
        push_unique(&mut errors, "zero_zones");
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn push_unique(errors: &mut Vec<String>, code: &str) {
    if !errors.iter().any(|existing| existing == code) {
        errors.push(code.to_owned());
    }
}

fn hash_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
