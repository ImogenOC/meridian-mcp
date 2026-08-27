use meridian_mcp::tracy_artifact::{
    compare_metadata, reserve_trace_set, validate_capture_result, ComparisonMode, RawRange,
    TraceMetadata, TraceSetError,
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
fn invalid_capture_is_retained_only_as_a_non_authoritative_diagnostic() {
    let root = fixture();
    let trace = root.join("authoritative.tracy");
    let diagnostics = root.join("diagnostics");
    std::fs::create_dir_all(&diagnostics).unwrap();
    let diagnostic_trace = diagnostics.join("steady-state-1.invalid.tracy");
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();
    let reserved = reserve_trace_set(&policy, &trace, false).unwrap();
    std::fs::write(reserved.temporary_trace_path(), b"invalid-tracy-bytes").unwrap();

    let retained = reserved
        .promote_diagnostic(
            &policy,
            &diagnostic_trace,
            &json!({
                "schema": 2,
                "authoritative": false,
                "validation": {"valid": false, "error_codes": ["zero_zones"]},
            }),
        )
        .unwrap();

    assert!(!trace.exists());
    assert!(!root.join("authoritative.tracy.meridian.json").exists());
    assert!(!retained.authoritative);
    assert_eq!(
        retained.trace.path,
        diagnostic_trace.canonicalize().unwrap()
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(&retained.sidecar.path).unwrap()
        )
        .unwrap()["validation"]["error_codes"],
        json!(["zero_zones"])
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn capture_publication_rejects_invalid_or_malformed_helper_results() {
    let invalid = json!({
        "validation": {
            "valid": false,
            "raw_begin": 10,
            "raw_end": 20,
            "trace_begin_ns": 100,
            "trace_end_ns": 200,
            "complete_frames": 3,
            "zones": 0,
            "error_codes": ["zero_zones"],
        }
    });
    assert_eq!(
        validate_capture_result(&invalid).unwrap_err(),
        vec!["zero_zones"]
    );

    let malformed = json!({"validation": {"valid": true}});
    assert_eq!(
        validate_capture_result(&malformed).unwrap_err(),
        vec![
            "missing_raw_range",
            "missing_trace_range",
            "no_complete_frames",
            "zero_zones"
        ]
    );

    let valid = json!({
        "validation": {
            "valid": true,
            "raw_begin": 10,
            "raw_end": 20,
            "trace_begin_ns": 100,
            "trace_end_ns": 200,
            "complete_frames": 3,
            "zones": 4,
            "error_codes": [],
        }
    });
    assert_eq!(validate_capture_result(&valid), Ok(()));
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
        meridian_mcp_build_id: "mcp-build-a".into(),
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
                build_record_id: None,
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
    let mut different_mcp = baseline.clone();
    different_mcp.meridian_mcp_build_id = "mcp-build-b".into();
    assert!(
        !compare_metadata(&baseline, &different_mcp, ComparisonMode::CrossExperiment).compatible
    );
}
