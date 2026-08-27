use meridian_mcp::runtime_integrity::{RuntimeIntegritySession, RuntimeIntegrityStatus};
use meridian_mcp::state::{push_output_line_at, OutputLog};
use meridian_mcp::{
    EffectiveRoot, LaunchProvenance, PrivateStateStore, ProvenanceStatus, RootSource,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn records_first_mutation_with_the_nearest_output_and_never_repairs_it() {
    let base = std::env::temp_dir().join(format!(
        "meridian-runtime-integrity-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = base.join("workspace");
    let state = base.join("state");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    let target = workspace.join("tracked.dm");
    std::fs::write(&target, "before").unwrap();
    let roots = vec![EffectiveRoot {
        path: workspace.canonicalize().unwrap(),
        source: RootSource::ExplicitRoot,
        repository_identity: None,
        head_revision: None,
        dirty: None,
    }];
    let store = Arc::new(PrivateStateStore::open(&state, &roots).unwrap());
    let output = OutputLog::default();
    let mut session = RuntimeIntegritySession::create(
        store,
        &workspace,
        LaunchProvenance {
            status: ProvenanceStatus::Unverified,
            build_record_id: None,
            dmb_sha256: "00".repeat(32),
            warnings: Vec::new(),
        },
        output.clone(),
        Vec::new(),
    )
    .unwrap();
    push_output_line_at(&output, 0, "runtime phase: preview generation".to_owned());
    std::fs::write(&target, "after").unwrap();

    let first = session.observe_now("test").await.unwrap();
    let second = session.finalize("test_finalize").await.unwrap();

    assert_eq!(first.event_count, 1);
    assert_eq!(second.event_count, 1);
    assert_eq!(second.status, RuntimeIntegrityStatus::FinalizedWithChanges);
    assert_eq!(second.warnings[0].relative_path, "tracked.dm");
    assert_eq!(
        second.warnings[0].nearest_output.as_ref().unwrap().text,
        "runtime phase: preview generation"
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "after");
    std::fs::remove_dir_all(base).unwrap();
}
