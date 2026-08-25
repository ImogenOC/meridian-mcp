use meridian_mcp::result::ToolContent;
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::{CapabilityMode, PathPolicy};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn fixture() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "meridian-mcp-analysis-snapshot-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let one = root.join("one.dme");
    let two = root.join("two.dme");
    std::fs::write(&one, "/datum/snapshot_one\n").unwrap();
    std::fs::write(&two, "/datum/snapshot_two\n").unwrap();
    (root, one, two)
}

fn payload(result: meridian_mcp::result::ToolResult) -> Value {
    let ToolContent::Text { text } = &result.content[0];
    serde_json::from_str(text).unwrap()
}

#[tokio::test]
async fn held_snapshot_survives_a_new_parse_generation() {
    let (root, one, two) = fixture();
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let state = ServerState::new();

    let first = call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({ "dme_path": one }),
    )
    .await
    .unwrap();
    assert_eq!(payload(first)["state_generation"], 1);
    let held = state.snapshot().await.unwrap();

    let second = call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({ "dme_path": two }),
    )
    .await
    .unwrap();
    assert_eq!(payload(second)["state_generation"], 2);
    let active = state.snapshot().await.unwrap();

    assert_eq!(held.generation, 1);
    assert_eq!(active.generation, 2);
    assert!(held.environment_path.ends_with("one.dme"));
    assert!(active.environment_path.ends_with("two.dme"));
    assert!(held.objtree.find("/datum/snapshot_one").is_some());
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn failed_parse_preserves_the_complete_active_generation() {
    let (root, one, _) = fixture();
    let broken = root.join("broken.dme");
    std::fs::create_dir(&broken).unwrap();
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let state = ServerState::new();
    call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({ "dme_path": one }),
    )
    .await
    .unwrap();
    let before = state.snapshot().await.unwrap();

    let failed = call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({ "dme_path": broken }),
    )
    .await
    .unwrap();
    let failed = payload(failed);
    let after = state.snapshot().await.unwrap();

    assert_eq!(failed["details"]["state_preserved"], true);
    assert_eq!(failed["details"]["state_generation"], 1);
    assert_eq!(before.generation, after.generation);
    assert_eq!(before.environment_path, after.environment_path);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cloned_snapshot_read_does_not_block_installing_the_next_generation() {
    let (root, one, two) = fixture();
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let state = Arc::new(ServerState::new());
    call_tool(
        &context,
        state.as_ref(),
        "dm_parse_environment",
        json!({ "dme_path": one }),
    )
    .await
    .unwrap();
    let held = state.snapshot().await.unwrap();
    let reader = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        held.objtree.iter_types().count()
    });

    call_tool(
        &context,
        state.as_ref(),
        "dm_parse_environment",
        json!({ "dme_path": two }),
    )
    .await
    .unwrap();

    assert!(reader.await.unwrap() > 0);
    assert_eq!(state.snapshot().await.unwrap().generation, 2);
    std::fs::remove_dir_all(root).unwrap();
}
