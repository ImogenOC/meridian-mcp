use meridian_mcp::native_evidence::model::*;
use meridian_mcp::native_evidence::{parse_artifact, validate_request, NativeEvidenceContext};
use meridian_mcp::PathPolicy;
use serde_json::json;
use std::fs;

fn fixture() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/evidence")
}
fn context() -> NativeEvidenceContext {
    NativeEvidenceContext {
        policy: PathPolicy::new(vec![fixture()], Vec::new()).unwrap(),
        provenance: None,
    }
}

#[test]
fn descriptors_are_strict_and_limits_are_validated() {
    assert!(serde_json::from_value::<NativeEvidenceRequest>(
        json!({"artifacts":[{"kind":"auto","path":"evidence.json"}],"surprise":true})
    )
    .is_err());
    let request = NativeEvidenceRequest {
        artifacts: (0..33)
            .map(|_| ArtifactDescriptor {
                kind: ArtifactKind::PerformanceCsv,
                path: fixture().join("performance-lf.csv"),
                options: None,
            })
            .collect(),
        dmb_path: None,
        workload: None,
        phases: Vec::new(),
    };
    assert!(validate_request(&request).is_err());
}

#[test]
fn all_five_explicit_formats_are_bounded_and_hashed() {
    let cases = [
        (
            ArtifactKind::ByondProcProfileJson,
            "proc-profile.json",
            EvidenceSemantics::CumulativeSnapshot,
            1,
        ),
        (
            ArtifactKind::ByondSendmapsJson,
            "sendmaps.json",
            EvidenceSemantics::CumulativeSnapshot,
            1,
        ),
        (
            ArtifactKind::PerformanceCsv,
            "performance-lf.csv",
            EvidenceSemantics::IntervalSeries,
            3,
        ),
        (
            ArtifactKind::RuntimeJsonl,
            "runtime-lf.jsonl",
            EvidenceSemantics::EventStream,
            2,
        ),
        (
            ArtifactKind::EventJsonl,
            "events.jsonl",
            EvidenceSemantics::EventStream,
            2,
        ),
    ];
    let context = context();
    let mut total = 0;
    let mut redacted = 0;
    for (kind, name, semantics, count) in cases {
        let parsed = parse_artifact(
            &context,
            &ArtifactDescriptor {
                kind,
                path: fixture().join(name),
                options: Some(ArtifactOptions {
                    selected_metrics: vec![
                        "tick_usage".into(),
                        "duration_ms".into(),
                        "value".into(),
                        "calls".into(),
                        "send_count".into(),
                    ],
                    wall_time_field: Some("timestamp".into()),
                    ..Default::default()
                }),
            },
            &mut total,
            &mut redacted,
        )
        .unwrap();
        assert_eq!(parsed.semantics, semantics);
        assert_eq!(parsed.accepted_records, count);
        assert_eq!(parsed.identity.sha256.len(), 64);
        assert!(!parsed.identity.relative_path.contains(":"));
    }
    assert!(redacted > 0);
}

#[test]
fn csv_and_jsonl_have_lf_crlf_parity() {
    let temporary = std::env::temp_dir().join(format!(
        "meridian-native-evidence-crlf-{}",
        std::process::id()
    ));
    fs::create_dir_all(&temporary).unwrap();
    for name in ["performance-lf.csv", "runtime-lf.jsonl"] {
        let contents = fs::read_to_string(fixture().join(name)).unwrap();
        fs::write(temporary.join(name), contents.replace('\n', "\r\n")).unwrap();
    }
    let context = NativeEvidenceContext {
        policy: PathPolicy::new(vec![temporary.clone()], Vec::new()).unwrap(),
        provenance: None,
    };
    for (kind, name, expected) in [
        (ArtifactKind::PerformanceCsv, "performance-lf.csv", 3),
        (ArtifactKind::RuntimeJsonl, "runtime-lf.jsonl", 2),
    ] {
        let mut total = 0;
        let mut redacted = 0;
        let parsed = parse_artifact(
            &context,
            &ArtifactDescriptor {
                kind,
                path: temporary.join(name),
                options: None,
            },
            &mut total,
            &mut redacted,
        )
        .unwrap();
        assert_eq!(parsed.accepted_records, expected);
    }
    fs::remove_dir_all(temporary).unwrap();
}
