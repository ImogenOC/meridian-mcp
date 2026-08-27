use meridian_mcp::result::ToolContent;
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::{CapabilityMode, PathPolicy, ProcResolutionKind};
use serde_json::json;

async fn parsed_fixture() -> (ToolExecutionContext, ServerState, std::path::PathBuf) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let state = ServerState::new();
    let result = call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({"dme_path": root.join("language/fixture.dme")}),
    )
    .await
    .unwrap();
    let ToolContent::Text { text } = &result.content[0];
    let payload: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["success"], true, "{payload:#}");
    (context, state, root)
}

#[tokio::test]
async fn resolver_separates_child_implementation_from_parent_declaration() {
    let (_, state, _) = parsed_fixture().await;
    let snapshot = state.snapshot().await.unwrap();
    let resolution = snapshot
        .proc_resolver()
        .resolve(
            "/datum/meridian_resolution_child",
            "meridian_resolution_fixture",
        )
        .unwrap();

    assert_eq!(
        resolution.implementation_owner,
        "/datum/meridian_resolution_child"
    );
    assert_eq!(resolution.declaration_owner, "/datum");
    assert_eq!(
        resolution.resolution_kind,
        ProcResolutionKind::LocalImplementation
    );
    assert_eq!(resolution.implementations.len(), 2);
    assert_eq!(
        resolution.implementations[0].owner,
        "/datum/meridian_resolution_child"
    );
    assert_eq!(resolution.implementations[1].owner, "/datum");
}

#[tokio::test]
async fn resolver_reports_inherited_and_missing_procs_deterministically() {
    let (_, state, _) = parsed_fixture().await;
    let snapshot = state.snapshot().await.unwrap();
    let inherited = snapshot
        .proc_resolver()
        .resolve(
            "/datum/meridian_resolution_child/grandchild",
            "meridian_resolution_fixture",
        )
        .unwrap();

    assert_eq!(
        inherited.resolution_kind,
        ProcResolutionKind::InheritedImplementation
    );
    assert_ne!(
        inherited.implementation_owner,
        "/datum/meridian_resolution_child/grandchild"
    );

    let first_error = snapshot
        .proc_resolver()
        .resolve(
            "/datum/meridian_resolution_child",
            "meridian_missing_fixture",
        )
        .unwrap_err();
    let second_error = snapshot
        .proc_resolver()
        .resolve(
            "/datum/meridian_resolution_child",
            "meridian_missing_fixture",
        )
        .unwrap_err();
    assert_eq!(first_error, second_error);
}

#[tokio::test]
async fn analysis_tools_agree_on_child_implementation_and_parent_declaration() {
    let (context, state, root) = parsed_fixture().await;
    let requested_type = "/datum/meridian_resolution_child";
    let proc_name = "meridian_resolution_fixture";

    let exact = call_tool(
        &context,
        &state,
        "dm_get_proc",
        json!({"type_path": requested_type, "proc_name": proc_name}),
    )
    .await
    .unwrap();
    let ToolContent::Text { text } = &exact.content[0];
    let exact: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(exact["requested_type_path"], requested_type, "{exact:#}");
    assert_eq!(exact["implementation_owner"], requested_type, "{exact:#}");
    assert_eq!(exact["declaration_owner"], "/datum", "{exact:#}");
    assert_eq!(
        exact["resolution_kind"], "local_implementation",
        "{exact:#}"
    );
    assert_eq!(exact["overrides"].as_array().unwrap().len(), 2, "{exact:#}");

    let definition = call_tool(
        &context,
        &state,
        "dm_get_definition",
        json!({"type_path": requested_type, "member_name": proc_name}),
    )
    .await
    .unwrap();
    let ToolContent::Text { text } = &definition.content[0];
    let definition: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(
        definition["implementation_owner"], requested_type,
        "{definition:#}"
    );
    assert_eq!(definition["declaration_owner"], "/datum", "{definition:#}");

    let symbols = call_tool(
        &context,
        &state,
        "dm_search_symbols",
        json!({"query": proc_name, "kind": "proc"}),
    )
    .await
    .unwrap();
    let ToolContent::Text { text } = &symbols.content[0];
    let symbols: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        symbols["results"].as_array().unwrap().iter().any(|item| {
            item["implementation_owner"] == requested_type && item["declaration_owner"] == "/datum"
        }),
        "{symbols:#}"
    );

    let context_search = call_tool(
        &context,
        &state,
        "dm_search_context",
        json!({"query": proc_name, "kind": "proc"}),
    )
    .await
    .unwrap();
    let ToolContent::Text { text } = &context_search.content[0];
    let context_search: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        context_search["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["implementation_owner"] == requested_type
                    && item["declaration_owner"] == "/datum"
            }),
        "{context_search:#}"
    );

    let document = call_tool(
        &context,
        &state,
        "dm_document_symbols",
        json!({"file_path": root.join("language/fixture.dm")}),
    )
    .await
    .unwrap();
    let ToolContent::Text { text } = &document.content[0];
    let document: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        document["symbols"].as_array().unwrap().iter().any(|item| {
            item["name"] == proc_name
                && item["implementation_owner"] == requested_type
                && item["declaration_owner"] == "/datum"
        }),
        "{document:#}"
    );

    let implementations = call_tool(
        &context,
        &state,
        "dm_find_implementations",
        json!({"type_path": "/datum", "member_name": proc_name}),
    )
    .await
    .unwrap();
    let ToolContent::Text { text } = &implementations.content[0];
    let implementations: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(
        implementations["implementations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["implementation_owner"] == requested_type
                    && item["declaration_owner"] == "/datum"
            }),
        "{implementations:#}"
    );
}
