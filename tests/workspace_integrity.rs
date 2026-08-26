use meridian_mcp::workspace_integrity::{IntegrityBaseline, IntegrityError};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn fixture() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "meridian-integrity-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn detects_non_owned_changes_and_ignores_exact_owned_outputs() {
    let root = fixture();
    let source = root.join("source.dm");
    let owned = root.join("capture.tracy");
    std::fs::write(&source, "before").unwrap();
    let baseline = IntegrityBaseline::capture(&root).unwrap();
    assert!(baseline.checkpoint("unchanged", &[]).is_ok());
    std::fs::write(&owned, "trace").unwrap();
    let owned_checkpoint = baseline.checkpoint("capture", std::slice::from_ref(&owned));
    assert!(owned_checkpoint.is_ok(), "{owned_checkpoint:?}");
    std::fs::write(&source, "after").unwrap();
    assert!(matches!(
        baseline.checkpoint("modified", &[owned]),
        Err(IntegrityError::Violation(_))
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpoint_serialization_contains_only_relative_paths() {
    let root = fixture();
    std::fs::write(root.join("source.dm"), "stable").unwrap();
    let baseline = IntegrityBaseline::capture(&root).unwrap();
    let checkpoint = baseline.checkpoint("status", &[]).unwrap();
    let json = serde_json::to_string(&checkpoint).unwrap();
    assert!(!json.contains(&root.to_string_lossy().to_string()));
    std::fs::remove_dir_all(root).unwrap();
}
