use meridian_mcp::result::ToolContent;
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::{CapabilityMode, PathPolicy};
use serde_json::{json, Value};

fn payload(result: meridian_mcp::result::ToolResult) -> Value {
    let ToolContent::Text { text } = &result.content[0];
    serde_json::from_str(text).unwrap()
}

#[tokio::test]
async fn owned_language_and_map_fixtures_exercise_real_adapters() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let state = ServerState::new();
    let parsed = call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({"dme_path": root.join("language/fixture.dme")}),
    )
    .await
    .unwrap();
    assert_eq!(payload(parsed)["success"], true);
    let found = call_tool(&context, &state, "dm_find_on_map", json!({"dmm_path": root.join("maps/fixture.dmm"), "type_path": "/obj/item/meridian_fixture"})).await.unwrap();
    assert_eq!(payload(found)["count"], 2);
}
