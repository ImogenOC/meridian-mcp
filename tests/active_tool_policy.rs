use meridian_mcp::result::{ToolContent, ToolResult};
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::{CapabilityMode, PathPolicy, RiftBuildAccess};
use serde_json::json;

fn message(result: &ToolResult) -> &str {
    match &result.content[0] {
        ToolContent::Text { text } => text,
    }
}

#[tokio::test]
async fn rift_compile_cannot_broaden_the_startup_network_ceiling() {
    let root =
        std::env::temp_dir().join(format!("meridian-mcp-rift-ceiling-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let context = ToolExecutionContext::with_rift_build(
        CapabilityMode::Development,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
        RiftBuildAccess::Offline,
    );
    let result = call_tool(
        &context,
        &mut ServerState::new(),
        "rift_compile",
        json!({"network_mode": "allow"}),
    )
    .await
    .unwrap();
    assert_eq!(result.is_error, Some(true));
    #[cfg(windows)]
    assert!(message(&result).contains("network_mode_denied"));
    #[cfg(not(windows))]
    assert!(message(&result).contains("unsupported_platform"));
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn rift_compile_rejects_zero_duration_limits() {
    let root =
        std::env::temp_dir().join(format!("meridian-mcp-rift-timeout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let context = ToolExecutionContext::with_rift_build(
        CapabilityMode::Development,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
        RiftBuildAccess::Offline,
    );
    let result = call_tool(
        &context,
        &mut ServerState::new(),
        "rift_compile",
        json!({"timeout_ms": 0}),
    )
    .await
    .unwrap();
    assert_eq!(result.is_error, Some(true));
    #[cfg(windows)]
    assert!(message(&result).contains("invalid_arguments"));
    #[cfg(not(windows))]
    assert!(message(&result).contains("unsupported_platform"));
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn analysis_mode_rejects_active_tools() {
    let root = std::env::temp_dir().join(format!("meridian-mcp-active-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let result = call_tool(&context, &mut ServerState::new(), "dm_compile", json!({}))
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(true));
    assert!(message(&result).contains("tool_not_available"));
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn development_mode_rejects_unlisted_compilers_and_implicit_overwrite() {
    let root = std::env::temp_dir().join(format!("meridian-mcp-active-dev-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let dme = root.join("fixture.dme");
    let compiler = root.join("unlisted.exe");
    let dmm = root.join("fixture.dmm");
    let png = root.join("fixture.png");
    for path in [&dme, &compiler, &dmm, &png] {
        std::fs::write(path, "fixture").unwrap();
    }
    let context = ToolExecutionContext::new(
        CapabilityMode::Development,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let compiler_result = call_tool(
        &context,
        &mut ServerState::new(),
        "dm_compile",
        json!({"dme_path": dme, "compiler_path": compiler}),
    )
    .await
    .unwrap();
    assert!(message(&compiler_result).contains("executable_not_allowed"));
    let render_result = call_tool(
        &context,
        &mut ServerState::new(),
        "dm_render_map",
        json!({"dmm_path": dmm, "output_path": png}),
    )
    .await
    .unwrap();
    assert!(message(&render_result).contains("output_exists"));
    std::fs::remove_dir_all(root).unwrap();
}
