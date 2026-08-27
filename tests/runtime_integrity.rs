use meridian_mcp::runtime_integrity::{
    recover_unfinished, RuntimeIntegritySession, RuntimeIntegrityStatus,
};
use meridian_mcp::state::{push_output_line_at, OutputLog};
use meridian_mcp::{
    EffectiveRoot, LaunchProvenance, PrivateStateStore, ProvenanceStatus, RootSource,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn launch_provenance() -> LaunchProvenance {
    LaunchProvenance {
        status: ProvenanceStatus::Unverified,
        build_record_id: None,
        dmb_sha256: "00".repeat(32),
        warnings: Vec::new(),
    }
}

fn roots_for(workspace: &std::path::Path) -> Vec<EffectiveRoot> {
    vec![EffectiveRoot {
        path: workspace.canonicalize().unwrap(),
        source: RootSource::ExplicitRoot,
        repository_identity: None,
        head_revision: None,
        dirty: None,
    }]
}

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
    let roots = roots_for(&workspace);
    let store = Arc::new(PrivateStateStore::open(&state, &roots).unwrap());
    let output = OutputLog::default();
    let mut session = RuntimeIntegritySession::create(
        store,
        &workspace,
        launch_provenance(),
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

#[test]
fn recovery_skips_a_live_session_and_recovers_it_after_exit() {
    let base = std::env::temp_dir().join(format!(
        "meridian-runtime-integrity-recovery-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = base.join("workspace");
    let state = base.join("state");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(workspace.join("tracked.dm"), "before").unwrap();
    let roots = roots_for(&workspace);
    let store = Arc::new(PrivateStateStore::open(&state, &roots).unwrap());
    let session = RuntimeIntegritySession::create(
        store.clone(),
        &workspace,
        launch_provenance(),
        OutputLog::default(),
        Vec::new(),
    )
    .unwrap();

    assert!(recover_unfinished(&store, &roots).unwrap().is_empty());

    let active: meridian_mcp::runtime_integrity::RuntimeIntegrityJournal = store
        .read_json(&format!(
            "runtime-integrity/{}.json",
            session.summary().session_id
        ))
        .unwrap();
    assert_eq!(active.status, RuntimeIntegrityStatus::Active);

    drop(session);
    let recovered = recover_unfinished(&store, &roots).unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(
        recovered[0].status,
        RuntimeIntegrityStatus::ObservedDuringRecovery
    );
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn recovery_respects_liveness_across_processes_and_forced_exit() {
    let base = std::env::temp_dir().join(format!(
        "meridian-runtime-integrity-subprocess-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = base.join("workspace");
    let state = base.join("state");
    let ready = base.join("ready");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(workspace.join("tracked.dm"), "before").unwrap();
    let roots = roots_for(&workspace);
    let store = PrivateStateStore::open(&state, &roots).unwrap();

    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--ignored")
        .arg("--exact")
        .arg("runtime_integrity_lock_holder_helper")
        .arg("--nocapture")
        .env("MERIDIAN_RUNTIME_LOCK_HELPER", "1")
        .env("MERIDIAN_RUNTIME_LOCK_WORKSPACE", &workspace)
        .env("MERIDIAN_RUNTIME_LOCK_STATE", &state)
        .env("MERIDIAN_RUNTIME_LOCK_READY", &ready)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.is_file() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("runtime integrity lock holder exited before readiness: {status}");
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("runtime integrity lock holder did not become ready");
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    let live_recovery = recover_unfinished(&store, &roots);
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(live_recovery.unwrap().is_empty());

    let recovered = recover_unfinished(&store, &roots).unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(
        recovered[0].status,
        RuntimeIntegrityStatus::ObservedDuringRecovery
    );
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
#[ignore]
fn runtime_integrity_lock_holder_helper() {
    if std::env::var_os("MERIDIAN_RUNTIME_LOCK_HELPER").is_none() {
        return;
    }
    let workspace =
        std::path::PathBuf::from(std::env::var_os("MERIDIAN_RUNTIME_LOCK_WORKSPACE").unwrap());
    let state = std::path::PathBuf::from(std::env::var_os("MERIDIAN_RUNTIME_LOCK_STATE").unwrap());
    let ready = std::path::PathBuf::from(std::env::var_os("MERIDIAN_RUNTIME_LOCK_READY").unwrap());
    let roots = roots_for(&workspace);
    let store = Arc::new(PrivateStateStore::open(&state, &roots).unwrap());
    let session = RuntimeIntegritySession::create(
        store,
        &workspace,
        launch_provenance(),
        OutputLog::default(),
        Vec::new(),
    )
    .unwrap();
    std::fs::write(ready, session.summary().session_id).unwrap();
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
