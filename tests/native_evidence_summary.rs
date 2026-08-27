use meridian_mcp::native_evidence::model::*;
use meridian_mcp::native_evidence::statistics::NumericSummary;
use meridian_mcp::native_evidence::{summarize_run, NativeEvidenceContext};
use meridian_mcp::PathPolicy;

#[test]
fn type7_statistics_and_unverified_summary_are_deterministic() {
    let stats = NumericSummary::from_samples(&[1.0, 2.0, 3.0, 4.0]).unwrap();
    assert_eq!(stats.count, 4);
    assert_eq!(stats.missing_count, 0);
    assert_eq!(stats.mean, 2.5);
    assert_eq!(stats.p50, 2.5);
    assert!((stats.p95 - 3.85).abs() < 1e-12);
    assert!(
        meridian_mcp::native_evidence::statistics::coefficient_of_variation(&stats, 4).is_some()
    );
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/evidence");
    let context = NativeEvidenceContext {
        policy: PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
        provenance: None,
    };
    let summary = summarize_run(
        &context,
        NativeEvidenceRequest {
            artifacts: vec![ArtifactDescriptor {
                kind: ArtifactKind::PerformanceCsv,
                path: root.join("performance-lf.csv"),
                options: Some(ArtifactOptions {
                    selected_metrics: vec!["tick_usage".into()],
                    wall_time_field: Some("timestamp".into()),
                    ..Default::default()
                }),
            }],
            dmb_path: None,
            workload: None,
            phases: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(summary.schema, 1);
    assert_eq!(summary.identity.identity_verification, "unavailable");
    assert_eq!(summary.datasets[0].metrics["tick_usage"].mean, 20.0);
    assert_eq!(summary.datasets[0].raw_records, 3);
    assert_eq!(summary.datasets[0].unassigned_records, 3);
}

#[test]
fn interval_summary_includes_full_series_and_each_assigned_phase() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/evidence");
    let context = NativeEvidenceContext {
        policy: PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
        provenance: None,
    };
    let summary = summarize_run(
        &context,
        NativeEvidenceRequest {
            artifacts: vec![ArtifactDescriptor {
                kind: ArtifactKind::PerformanceCsv,
                path: root.join("performance-lf.csv"),
                options: Some(ArtifactOptions {
                    selected_metrics: vec!["tick_usage".into()],
                    wall_time_field: Some("timestamp".into()),
                    ..Default::default()
                }),
            }],
            dmb_path: None,
            workload: None,
            phases: vec![
                PhaseInput {
                    id: "startup".into(),
                    wall_start: Some("2026-01-01T00:00:00Z".into()),
                    wall_end: Some("2026-01-01T00:00:02Z".into()),
                    world_start_ds: None,
                    world_end_ds: None,
                },
                PhaseInput {
                    id: "running".into(),
                    wall_start: Some("2026-01-01T00:00:02Z".into()),
                    wall_end: Some("2026-01-01T00:00:03Z".into()),
                    world_start_ds: None,
                    world_end_ds: None,
                },
            ],
        },
    )
    .unwrap();
    assert_eq!(summary.datasets.len(), 3);
    assert_eq!(summary.datasets[0].classification, "full_interval");
    assert_eq!(summary.datasets[0].accepted_records, 3);
    assert_eq!(
        summary.datasets[1].assigned_phase.as_deref(),
        Some("startup")
    );
    assert_eq!(summary.datasets[1].accepted_records, 2);
    assert_eq!(
        summary.datasets[2].assigned_phase.as_deref(),
        Some("running")
    );
    assert_eq!(summary.datasets[2].accepted_records, 1);
}
