use meridian_mcp::{
    BuildAttempt, BuildAttemptOutcome, BuildInputIdentity, BuildProvenanceStore, BuildRecord,
    EffectiveRoot, FileIdentity, PathPolicy, PrivateStateStore, ProvenanceStatus,
    RepositoryIdentity, RootSource,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct ProvenanceFixture {
    base: PathBuf,
    workspace: PathBuf,
    state: PathBuf,
    input: PathBuf,
    dmb: PathBuf,
    rsc: PathBuf,
}

impl ProvenanceFixture {
    fn new(name: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "meridian-mcp-provenance-{name}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let workspace = base.join("workspace");
        let state = base.join("state");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        let input = workspace.join("fixture.dm");
        let dmb = workspace.join("fixture.dmb");
        let rsc = workspace.join("fixture.rsc");
        std::fs::write(&input, "source-v1").unwrap();
        std::fs::write(&dmb, "dmb-v1").unwrap();
        std::fs::write(&rsc, "rsc-v1").unwrap();
        Self {
            base,
            workspace,
            state,
            input,
            dmb,
            rsc,
        }
    }

    fn root(&self, digest: &str) -> EffectiveRoot {
        EffectiveRoot {
            path: self.workspace.canonicalize().unwrap(),
            source: RootSource::ExplicitRoot,
            repository_identity: Some(RepositoryIdentity {
                kind: "git_common_directory_sha256",
                digest: digest.to_owned(),
            }),
            head_revision: Some("revision-a".to_owned()),
            dirty: Some(false),
        }
    }

    fn store(&self, digest: &str) -> BuildProvenanceStore {
        let roots = vec![self.root(digest)];
        let private = Arc::new(PrivateStateStore::open(&self.state, &roots).unwrap());
        let policy = PathPolicy::from_effective_roots(roots, Vec::new()).unwrap();
        BuildProvenanceStore::new(private, policy)
    }

    fn success(&self, store: &BuildProvenanceStore) -> BuildRecord {
        BuildRecord {
            schema: 1,
            record_id: "record-success".to_owned(),
            artifact_key: store.artifact_key(&self.dmb).unwrap(),
            mcp_build: meridian_mcp::build_identity::current().clone(),
            compiler: FileIdentity::capture(&self.input).unwrap(),
            project: store.project_identity(&self.dmb).unwrap(),
            inputs: vec![
                BuildInputIdentity::capture(&self.workspace, &self.input, "source").unwrap(),
            ],
            dmb: FileIdentity::capture(&self.dmb).unwrap(),
            rsc: Some(FileIdentity::capture(&self.rsc).unwrap()),
            fixture_manifest_sha256: None,
            created_at_unix_ms: 1,
        }
    }
}

impl Drop for ProvenanceFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.base).unwrap();
    }
}

#[test]
fn failed_attempt_makes_last_success_stale_across_reopen() {
    let fixture = ProvenanceFixture::new("failed");
    let store = fixture.store("repo-a");
    let success = fixture.success(&store);
    store.record_success(&success).unwrap();
    store
        .record_attempt(&BuildAttempt {
            schema: 1,
            attempt_id: "attempt-failed".to_owned(),
            artifact_key: success.artifact_key.clone(),
            outcome: BuildAttemptOutcome::Failed {
                code: "compiler_failed".to_owned(),
            },
            observed_inputs: success.inputs.clone(),
            retained_dmb_sha256: Some(success.dmb.sha256.clone()),
            created_at_unix_ms: 2,
        })
        .unwrap();
    drop(store);

    let reopened = fixture.store("repo-a");
    let decision = reopened.evaluate_launch(&fixture.dmb, false).unwrap();
    assert_eq!(decision.status, ProvenanceStatus::Stale);
    assert!(!decision.allowed);
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.code == "later_compile_failed"));
}

#[test]
fn changed_inputs_outputs_and_repository_identity_are_stale() {
    for (name, change, reason) in [
        ("input", "input", "input_changed"),
        ("dmb", "dmb", "dmb_changed"),
        ("rsc", "rsc", "rsc_changed"),
    ] {
        let fixture = ProvenanceFixture::new(name);
        let store = fixture.store("repo-a");
        store.record_success(&fixture.success(&store)).unwrap();
        match change {
            "input" => std::fs::write(&fixture.input, "source-v2").unwrap(),
            "dmb" => std::fs::write(&fixture.dmb, "dmb-v2").unwrap(),
            "rsc" => std::fs::write(&fixture.rsc, "rsc-v2").unwrap(),
            _ => unreachable!(),
        }
        let decision = store.evaluate_launch(&fixture.dmb, false).unwrap();
        assert_eq!(decision.status, ProvenanceStatus::Stale, "{name}");
        assert!(
            decision.reasons.iter().any(|item| item.code == reason),
            "{name}: {decision:#?}"
        );
    }

    let fixture = ProvenanceFixture::new("repository");
    let store = fixture.store("repo-a");
    store.record_success(&fixture.success(&store)).unwrap();
    drop(store);
    let changed_repository = fixture.store("repo-b");
    let decision = changed_repository
        .evaluate_launch(&fixture.dmb, false)
        .unwrap();
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.code == "repository_identity_changed"));
}

#[test]
fn unmanaged_artifacts_are_unverified_and_can_require_verification() {
    let fixture = ProvenanceFixture::new("unmanaged");
    let store = fixture.store("repo-a");
    let permissive = store.evaluate_launch(&fixture.dmb, false).unwrap();
    assert_eq!(permissive.status, ProvenanceStatus::Unverified);
    assert!(permissive.allowed);
    let strict = store.evaluate_launch(&fixture.dmb, true).unwrap();
    assert_eq!(strict.status, ProvenanceStatus::Unverified);
    assert!(!strict.allowed);
}

#[test]
fn not_yet_compiled_artifact_is_unverified_instead_of_an_io_error() {
    let fixture = ProvenanceFixture::new("missing-artifact");
    std::fs::remove_file(&fixture.dmb).unwrap();
    let decision = fixture
        .store("repo-a")
        .evaluate_launch(&fixture.dmb, false)
        .unwrap();
    assert_eq!(decision.status, ProvenanceStatus::Unverified);
    assert!(decision.allowed);
}
