use meridian_mcp::result::{ToolContent, ToolResult};
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::{CapabilityMode, PathPolicy};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn payload(result: &ToolResult) -> Value {
    let ToolContent::Text { text } = &result.content[0];
    serde_json::from_str(text).expect("compiler result should be JSON")
}

#[tokio::test]
async fn direct_compile_reports_bounded_output_artifacts_and_optional_audit() {
    let root = std::env::temp_dir().join(format!(
        "meridian-mcp-compiler-runner-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let dme = root.join("fixture.dme");
    let dmb = root.join("fixture.dmb");
    std::fs::write(&dme, "// fixture").unwrap();
    std::fs::write(&dmb, "pre-existing artifact").unwrap();
    let compiler = std::env::current_exe().unwrap();
    let policy = PathPolicy::new(vec![root.clone()], vec![compiler.clone()]).unwrap();
    let context = ToolExecutionContext::new(CapabilityMode::Development, policy);

    let result = call_tool(
        &context,
        &mut ServerState::new(),
        "dm_compile",
        json!({
            "dme_path": dme,
            "compiler_path": compiler,
            "working_directory": root,
            "capture_network": true,
            "timeout_ms": 10_000,
            "idle_timeout_ms": 5_000
        }),
    )
    .await
    .unwrap();
    let payload = payload(&result);

    assert_eq!(payload["termination"], "exited");
    assert_eq!(payload["network_audit"]["requested"], true);
    assert_eq!(payload["network_audit"]["capture_complete"], false);
    assert!(payload["stdout_truncated_bytes"].as_u64().is_some());
    assert!(payload["stderr_truncated_bytes"].as_u64().is_some());
    assert!(payload["artifact_before"]["sha256"].is_string());
    assert!(payload["artifact_after"]["sha256"].is_string());
    assert_eq!(payload["dmb_exists"], true);
    assert_eq!(payload["dme_argument"], "fixture.dme");
    std::fs::remove_dir_all(root).unwrap();
}
