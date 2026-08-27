use meridian_mcp::result::{ToolContent, ToolResult};
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::{CapabilityMode, PathPolicy};
use serde_json::json;

fn payload(result: &ToolResult) -> serde_json::Value {
    let ToolContent::Text { text } = &result.content[0];
    serde_json::from_str(text).expect("server status should be structured JSON")
}

#[tokio::test]
async fn server_status_reports_immutable_startup_and_session_state() {
    let root =
        std::env::temp_dir().join(format!("meridian-mcp-server-status-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );

    let result = call_tool(&context, &ServerState::new(), "dm_server_status", json!({}))
        .await
        .unwrap();
    let value = payload(&result);

    assert_eq!(result.is_error, None);
    assert_eq!(value["mcp_build"]["schema"], 1);
    assert_eq!(value["mode"], "analysis");
    assert_eq!(value["containment"]["mode"], "immutable_startup_roots");
    assert_eq!(
        value["containment"]["policy_source"],
        "server_startup_configuration"
    );
    assert_eq!(
        value["containment"]["effective_roots"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(value["analysis"]["state_generation"], 0);
    assert_eq!(
        value["analysis"]["environment_path"],
        serde_json::Value::Null
    );
    assert_eq!(value["runtime"]["running"], false);

    std::fs::remove_dir_all(root).unwrap();
}
