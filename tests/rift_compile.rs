use meridian_mcp::result::{ToolContent, ToolResult};
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::{CapabilityMode, PathPolicy, RiftBuildAccess};
use serde_json::json;
#[cfg(windows)]
use serde_json::Value;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(windows)]
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn text(result: &ToolResult) -> &str {
    let ToolContent::Text { text } = &result.content[0];
    text
}

#[cfg(windows)]
fn payload(result: &ToolResult) -> Value {
    serde_json::from_str(text(result)).expect("rift_compile result should be JSON")
}

#[cfg(windows)]
fn fixture(name: &str, dme_name: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "meridian-mcp-rift-{name}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let dme = root.join(dme_name);
    std::fs::write(&dme, "// fixture\n").unwrap();
    std::fs::write(root.join("BUILD.cmd"), "@echo off\n").unwrap();
    std::fs::write(
        root.join("dependencies.sh"),
        "export BYOND_MAJOR=516\nexport BYOND_MINOR=1685\n",
    )
    .unwrap();
    std::fs::write(
        root.join("RIFT_BUILD.cmd"),
        r#"@echo off
setlocal
if /I "%MERIDIAN_RIFT_BUILD_NETWORK%"=="offline" if not exist "%~dp0offline.ready" (
  >&2 echo [offline_preflight_failed] fixture cache is cold
  exit /b 2
)
>"%~dp0wrapper.marker" echo ran
if exist "%~dp0no-write.ready" exit /b 0
if exist "%~dp0cache-hit.ready" (
  echo Skipping 'dm' ^(up to date^)
  exit /b 0
)
>"%~dp0tgstation.dmb" echo compiled dmb
>"%~dp0tgstation.rsc" echo compiled rsc
exit /b 0
"#,
    )
    .unwrap();
    (root, dme)
}

fn context(root: &Path, compilers: Vec<PathBuf>, access: RiftBuildAccess) -> ToolExecutionContext {
    ToolExecutionContext::with_rift_build(
        CapabilityMode::Development,
        PathPolicy::new(vec![root.to_owned()], compilers).unwrap(),
        access,
    )
}

#[cfg(windows)]
async fn parse_project(context: &ToolExecutionContext, state: &mut ServerState, dme: &Path) {
    let result = call_tool(
        context,
        state,
        "dm_parse_environment",
        json!({"dme_path": dme}),
    )
    .await
    .unwrap();
    assert_eq!(result.is_error, None, "parse failed: {}", text(&result));
}

#[cfg(windows)]
#[tokio::test]
async fn rift_compile_requires_a_parsed_qualified_project() {
    let (root, dme) = fixture("qualification", "other.dme");
    let compiler = std::env::current_exe().unwrap();
    let context = context(&root, vec![compiler], RiftBuildAccess::Offline);
    let mut state = ServerState::new();

    let before_parse = call_tool(&context, &mut state, "rift_compile", json!({}))
        .await
        .unwrap();
    assert!(text(&before_parse).contains("project_not_parsed"));

    parse_project(&context, &mut state, &dme).await;
    let unqualified = call_tool(&context, &mut state, "rift_compile", json!({}))
        .await
        .unwrap();
    assert!(text(&unqualified).contains("project_not_qualified"));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[tokio::test]
async fn rift_compile_requires_exactly_one_startup_compiler() {
    let (root, dme) = fixture("compilers", "tgstation.dme");
    std::fs::write(root.join("offline.ready"), "ready").unwrap();
    let mut state = ServerState::new();
    let no_compiler = context(&root, Vec::new(), RiftBuildAccess::Offline);
    parse_project(&no_compiler, &mut state, &dme).await;
    let missing = call_tool(&no_compiler, &mut state, "rift_compile", json!({}))
        .await
        .unwrap();
    assert!(text(&missing).contains("compiler_not_configured"));

    let first = std::env::current_exe().unwrap();
    let second = root.join("second-compiler.exe");
    std::fs::copy(&first, &second).unwrap();
    let ambiguous = context(&root, vec![first, second], RiftBuildAccess::Offline);
    let result = call_tool(&ambiguous, &mut state, "rift_compile", json!({}))
        .await
        .unwrap();
    assert!(text(&result).contains("compiler_ambiguous"));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[tokio::test]
async fn offline_and_argument_policy_fail_before_unapproved_execution() {
    let (root, dme) = fixture("policy", "tgstation.dme");
    let compiler = std::env::current_exe().unwrap();
    let context = context(&root, vec![compiler], RiftBuildAccess::Offline);
    let mut state = ServerState::new();
    parse_project(&context, &mut state, &dme).await;

    let denied = call_tool(
        &context,
        &mut state,
        "rift_compile",
        json!({"network_mode": "allow"}),
    )
    .await
    .unwrap();
    assert!(text(&denied).contains("network_mode_denied"));
    assert!(!root.join("wrapper.marker").exists());

    let unknown = call_tool(
        &context,
        &mut state,
        "rift_compile",
        json!({"script": "OTHER.cmd", "arguments": ["lint"]}),
    )
    .await
    .unwrap();
    assert!(text(&unknown).contains("invalid_arguments"));
    assert!(!root.join("wrapper.marker").exists());

    let cold = call_tool(&context, &mut state, "rift_compile", json!({}))
        .await
        .unwrap();
    assert!(text(&cold).contains("offline_preflight_failed"));
    assert!(!root.join("wrapper.marker").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[tokio::test]
async fn fixed_wrapper_produces_fresh_artifact_evidence() {
    let (root, dme) = fixture("fresh space", "tgstation.dme");
    std::fs::write(root.join("offline.ready"), "ready").unwrap();
    let compiler = std::env::current_exe().unwrap();
    let context = context(&root, vec![compiler], RiftBuildAccess::Offline);
    let mut state = ServerState::new();
    parse_project(&context, &mut state, &dme).await;

    let result = call_tool(
        &context,
        &mut state,
        "rift_compile",
        json!({"capture_network": true}),
    )
    .await
    .unwrap();
    let payload = payload(&result);
    assert_eq!(result.is_error, None, "result: {payload:#}");
    assert_eq!(payload["evidence"], "fresh_artifacts");
    assert_eq!(payload["network_audit"]["requested"], true);
    assert_eq!(payload["network_audit"]["capture_complete"], false);
    assert_eq!(payload["artifact_after"]["dmb"]["exists"], true);
    assert_eq!(payload["artifact_after"]["rsc"]["exists"], true);
    assert!(payload["artifact_after"]["dmb"]["sha256"].is_string());
    assert!(root.join("wrapper.marker").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[tokio::test]
async fn force_rebuild_rejects_unchanged_artifacts() {
    let (root, dme) = fixture("unchanged", "tgstation.dme");
    for name in ["offline.ready", "no-write.ready"] {
        std::fs::write(root.join(name), "ready").unwrap();
    }
    std::fs::write(root.join("tgstation.dmb"), "old dmb").unwrap();
    std::fs::write(root.join("tgstation.rsc"), "old rsc").unwrap();
    let compiler = std::env::current_exe().unwrap();
    let context = context(&root, vec![compiler], RiftBuildAccess::Offline);
    let mut state = ServerState::new();
    parse_project(&context, &mut state, &dme).await;

    let result = call_tool(
        &context,
        &mut state,
        "rift_compile",
        json!({"force_rebuild": true}),
    )
    .await
    .unwrap();
    assert_eq!(result.is_error, Some(true));
    assert_eq!(payload(&result)["code"], "insufficient_evidence");
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[tokio::test]
async fn exact_dm_cache_marker_supports_a_non_forced_cache_hit() {
    let (root, dme) = fixture("cache-hit", "tgstation.dme");
    for name in ["offline.ready", "cache-hit.ready"] {
        std::fs::write(root.join(name), "ready").unwrap();
    }
    std::fs::write(root.join("tgstation.dmb"), "cached dmb").unwrap();
    std::fs::write(root.join("tgstation.rsc"), "cached rsc").unwrap();
    let compiler = std::env::current_exe().unwrap();
    let context = context(&root, vec![compiler], RiftBuildAccess::Offline);
    let mut state = ServerState::new();
    parse_project(&context, &mut state, &dme).await;

    let result = call_tool(&context, &mut state, "rift_compile", json!({}))
        .await
        .unwrap();
    let payload = payload(&result);
    assert_eq!(result.is_error, None, "result: {payload:#}");
    assert_eq!(payload["evidence"], "valid_cache_hit");
    assert!(payload["cache_evidence"]
        .as_str()
        .unwrap()
        .contains("Skipping 'dm'"));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(not(windows))]
#[tokio::test]
async fn non_windows_direct_call_returns_stable_unsupported_platform() {
    let root =
        std::env::temp_dir().join(format!("meridian-mcp-rift-platform-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let context = context(&root, Vec::new(), RiftBuildAccess::Offline);
    let result = call_tool(&context, &mut ServerState::new(), "rift_compile", json!({}))
        .await
        .unwrap();
    assert!(text(&result).contains("unsupported_platform"));
    std::fs::remove_dir_all(root).unwrap();
}
