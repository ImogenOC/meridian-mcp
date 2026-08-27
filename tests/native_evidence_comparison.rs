use meridian_mcp::native_evidence::model::*;
use meridian_mcp::native_evidence::{compare_runs, NativeEvidenceContext};
use meridian_mcp::PathPolicy;

#[test]
fn comparison_refuses_unverified_runs_before_metrics() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/evidence");
    let context = NativeEvidenceContext {
        policy: PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
        provenance: None,
    };
    let request = || NativeEvidenceRequest {
        artifacts: vec![ArtifactDescriptor {
            kind: ArtifactKind::PerformanceCsv,
            path: root.join("performance-lf.csv"),
            options: Some(ArtifactOptions {
                selected_metrics: vec!["tick_usage".into()],
                ..Default::default()
            }),
        }],
        dmb_path: None,
        workload: None,
        phases: Vec::new(),
    };
    let error = compare_runs(&context, vec![request(), request()]).unwrap_err();
    assert!(error.to_string().contains("evidence_identity_mismatch"));
}
