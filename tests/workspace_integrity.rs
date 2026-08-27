use meridian_mcp::workspace_integrity::{
    IntegrityBaseline, IntegrityError, IntegrityJournal, IntegrityJournalStatus,
};
use meridian_mcp::PathPolicy;
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

#[test]
fn unfinished_integrity_journal_requires_recovery_and_contains_no_host_paths() {
    let root = fixture();
    let evidence = root.join("evidence");
    std::fs::create_dir_all(&evidence).unwrap();
    std::fs::write(root.join("source.dm"), "stable").unwrap();
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();
    let baseline = IntegrityBaseline::capture(&root).unwrap();
    let mut journal = IntegrityJournal::create(&policy, &evidence, &baseline).unwrap();
    let checkpoint = baseline
        .checkpoint(
            "post_launch",
            std::slice::from_ref(&journal.path().to_owned()),
        )
        .unwrap();
    journal.record(&policy, checkpoint).unwrap();
    let summary = journal.summary();
    assert_eq!(summary.status, IntegrityJournalStatus::Active);
    assert_eq!(summary.last_action, "post_launch");

    let document = std::fs::read_to_string(journal.path()).unwrap();
    assert!(!document.contains(&root.to_string_lossy().to_string()));
    assert_eq!(journal.status(), IntegrityJournalStatus::Active);
    assert!(matches!(
        IntegrityJournal::create(&policy, &evidence, &baseline),
        Err(IntegrityError::RecoveryRequired { .. })
    ));

    journal.finalize(&policy).unwrap();
    assert_eq!(journal.status(), IntegrityJournalStatus::Finalized);
    assert!(IntegrityJournal::create(&policy, &evidence, &baseline).is_ok());
    std::fs::remove_dir_all(root).unwrap();
}
