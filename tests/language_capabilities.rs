use meridian_mcp::result::ToolContent;
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::{CapabilityMode, PathPolicy};
use serde_json::{json, Value};

fn payload(result: meridian_mcp::result::ToolResult) -> Value {
    let ToolContent::Text { text } = &result.content[0];
    serde_json::from_str(text).unwrap()
}

async fn parsed_fixture() -> (ToolExecutionContext, ServerState, std::path::PathBuf) {
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
    (context, state, root)
}

#[tokio::test]
async fn document_symbols_include_macros_and_nested_declarations() {
    let (context, state, root) = parsed_fixture().await;
    let result = call_tool(
        &context,
        &state,
        "dm_document_symbols",
        json!({"file_path": root.join("language/fixture.dm")}),
    )
    .await
    .unwrap();
    let body = payload(result);
    let symbols = body["symbols"].as_array().unwrap();

    assert!(
        symbols.iter().any(|symbol| {
            symbol["kind"] == "macro" && symbol["name"] == "MERIDIAN_FIXTURE_VALUE"
        }),
        "{body:#}"
    );
    assert!(
        symbols.iter().any(|symbol| {
            symbol["kind"] == "type" && symbol["id"]["path"] == "/datum/fixture_symbol_parent/child"
        }),
        "{body:#}"
    );
    assert!(
        symbols.windows(2).all(|pair| {
            let left = (
                pair[0]["line"].as_u64().unwrap(),
                pair[0]["column"].as_u64().unwrap(),
            );
            let right = (
                pair[1]["line"].as_u64().unwrap(),
                pair[1]["column"].as_u64().unwrap(),
            );
            left <= right
        }),
        "{body:#}"
    );
}

#[tokio::test]
async fn references_resolve_members_and_ignore_shadowing_locals() {
    let (context, state, _) = parsed_fixture().await;
    let result = call_tool(
        &context,
        &state,
        "dm_find_references",
        json!({
            "type_path": "/datum/fixture_symbol_parent",
            "member_name": "value"
        }),
    )
    .await
    .unwrap();
    let body = payload(result);
    let references = body["references"].as_array().unwrap();

    assert_eq!(body["skipped_dynamic"], 0);
    assert_eq!(references.len(), 2, "{body:#}");
    assert_eq!(references[0]["kind"], "write");
    assert_eq!(references[1]["kind"], "read");
    assert_eq!(references[0]["line"], 23);
    assert_eq!(references[1]["line"], 24);
}

#[tokio::test]
async fn implementations_return_parent_then_child_declarations() {
    let (context, state, _) = parsed_fixture().await;
    let result = call_tool(
        &context,
        &state,
        "dm_find_implementations",
        json!({
            "type_path": "/datum/fixture_symbol_parent",
            "member_name": "compute"
        }),
    )
    .await
    .unwrap();
    let body = payload(result);
    let implementations = body["implementations"].as_array().unwrap();

    assert_eq!(implementations.len(), 2, "{body:#}");
    assert_eq!(
        implementations[0]["declared_in"],
        "/datum/fixture_symbol_parent"
    );
    assert_eq!(
        implementations[1]["declared_in"],
        "/datum/fixture_symbol_parent/child"
    );
}

#[tokio::test]
async fn diagnostics_report_explicit_configuration_provenance() {
    let (context, state, _) = parsed_fixture().await;
    let result = call_tool(&context, &state, "dm_check_errors", json!({}))
        .await
        .unwrap();
    let body = payload(result);
    let diagnostic = body["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| diagnostic["rule"] == "tmp_no_effect")
        .unwrap_or_else(|| panic!("missing tmp_no_effect diagnostic: {body:#}"));

    assert_eq!(diagnostic["configured"], true, "{diagnostic:#}");
    assert_eq!(diagnostic["component"], "parser");
    assert!(diagnostic["file"].as_str().unwrap().ends_with("fixture.dm"));
    assert!(diagnostic["line"].as_u64().unwrap() > 0);
}
