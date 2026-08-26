use meridian_mcp::tracy_artifact::{
    compare_metadata, reserve_trace_set, ComparisonMode, RawRange, TraceMetadata, TraceSetError,
};
use meridian_mcp::tracy_experiment::{
    ExecutableIdentity, ExperimentIdentity, HelperIdentity, WorkloadIdentity,
};
use meridian_mcp::PathPolicy;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn fixture() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "meridian-trace-set-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn promotes_exact_trace_and_schema_sidecar_pair() {
    let root = fixture();
    let trace = root.join("sample.tracy");
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();
    let reserved = reserve_trace_set(&policy, &trace, false).unwrap();
    std::fs::write(reserved.temporary_trace_path(), b"standard-tracy-bytes").unwrap();
    let promoted = reserved.promote(&json!({"schema":2})).unwrap();
    assert_eq!(promoted.trace.path, trace.canonicalize().unwrap());
    let sidecar = root.join("sample.tracy.meridian.json");
    assert_eq!(promoted.sidecar.path, sidecar.canonicalize().unwrap());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(sidecar).unwrap()).unwrap()
            ["schema"],
        2
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn refuses_collisions_and_overwrite_before_capture() {
    let root = fixture();
    let trace = root.join("sample.tracy");
    std::fs::write(&trace, b"human-owned").unwrap();
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();
    assert!(reserve_trace_set(&policy, &trace, false).is_err());
    assert!(matches!(
        reserve_trace_set(&policy, &root.join("new.tracy"), true),
        Err(TraceSetError::OverwriteUnsupported)
    ));
    assert_eq!(std::fs::read(&trace).unwrap(), b"human-owned");
    std::fs::remove_dir_all(root).unwrap();
}

fn metadata(
    experiment_id: &str,
    executable_id: &str,
    workload_id: &str,
    phase: &str,
) -> TraceMetadata {
    TraceMetadata {
        trace_sha256: "00".repeat(32),
        experiment_identity: ExperimentIdentity {
            experiment_id: experiment_id.into(),
            executable: ExecutableIdentity {
                schema: 1,
                executable_id: executable_id.into(),
                repository_revision: None,
                repository_dirty_digest: String::new(),
                dmb_sha256: String::new(),
                rsc_sha256: None,
                byond_version: "516.1687".into(),
                byond_executable_sha256: String::new(),
                native_modules: Vec::new(),
                helper_identity: HelperIdentity {
                    source_revision: String::new(),
                    sha256: String::new(),
                    patch_sha256: None,
                },
                hook_identity: HelperIdentity {
                    source_revision: String::new(),
                    sha256: String::new(),
                    patch_sha256: None,
                },
                startup_mode: "tracy".into(),
                launch_parameters_sha256: String::new(),
            },
            workload: WorkloadIdentity {
                workload_id: workload_id.into(),
                map: None,
                seed: None,
                configuration_profile: None,
                feature_set: Vec::new(),
                scenario: None,
                external_run_id: None,
                annotations: Default::default(),
            },
        },
        phase: phase.into(),
        phase_iteration: 1,
        range: RawRange {
            raw_begin: 1,
            raw_end: 2,
        },
        trace_range_ns: RawRange {
            raw_begin: 1,
            raw_end: 2,
        },
        complete_frames: 10,
        partial_frames: 2,
        zones: 4,
        capture_valid: true,
        queue_saturated: false,
        memory_roles: vec![
            meridian_mcp::process_metrics::ProcessRole::DreamDaemon,
            meridian_mcp::process_metrics::ProcessRole::Collector,
        ],
    }
}

#[test]
fn comparison_requires_identity_compatibility_before_native_analysis() {
    let baseline = metadata("experiment-a", "executable", "workload", "steady_state");
    let mut current = baseline.clone();
    current.phase_iteration = 2;
    assert!(
        compare_metadata(&baseline, &current, ComparisonMode::SameExperimentSamePhase).compatible
    );
    current.phase = "boot".into();
    assert!(
        !compare_metadata(&baseline, &current, ComparisonMode::SameExperimentSamePhase).compatible
    );
    let cross = metadata("experiment-b", "executable", "workload", "steady_state");
    assert!(compare_metadata(&baseline, &cross, ComparisonMode::CrossExperiment).compatible);
    assert!(
        !compare_metadata(&baseline, &cross, ComparisonMode::SameExperimentSamePhase).compatible
    );
}
