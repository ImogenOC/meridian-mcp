use meridian_mcp::{EffectiveRoot, PrivateStateStore, RootSource};
use serde_json::{json, Value};
use std::path::PathBuf;

struct StateFixture {
    base: PathBuf,
    workspace: PathBuf,
    state: PathBuf,
}

impl StateFixture {
    fn new(name: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "meridian-mcp-private-state-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let workspace = base.join("workspace");
        let state = base.join("state");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        Self {
            base,
            workspace,
            state,
        }
    }

    fn roots(&self) -> Vec<EffectiveRoot> {
        vec![EffectiveRoot {
            path: self.workspace.canonicalize().unwrap(),
            source: RootSource::ExplicitRoot,
            repository_identity: None,
            head_revision: None,
            dirty: None,
        }]
    }
}

impl Drop for StateFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.base).unwrap();
    }
}

#[test]
fn development_state_must_be_outside_workspace_and_writable() {
    let fixture = StateFixture::new("boundary");
    assert!(PrivateStateStore::open(&fixture.state, &fixture.roots()).is_ok());
    let nested_state = fixture.workspace.join("state");
    std::fs::create_dir_all(&nested_state).unwrap();
    assert!(PrivateStateStore::open(&nested_state, &fixture.roots()).is_err());
}

#[test]
fn atomic_records_survive_reopen_and_reject_traversal() {
    let fixture = StateFixture::new("reopen");
    let store = PrivateStateStore::open(&fixture.state, &fixture.roots()).unwrap();
    store
        .write_json_atomic("builds/example.json", &json!({"schema": 1}))
        .unwrap();
    assert!(store
        .write_json_atomic("../escape.json", &json!({"schema": 1}))
        .is_err());
    drop(store);

    let reopened = PrivateStateStore::open(&fixture.state, &fixture.roots()).unwrap();
    assert_eq!(
        reopened.read_json::<Value>("builds/example.json").unwrap()["schema"],
        1
    );
    assert_eq!(reopened.list_records("builds", 10).unwrap().len(), 1);
}
