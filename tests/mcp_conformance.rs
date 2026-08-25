use meridian_mcp::{CapabilityMode, MeridianServer, RiftBuildAccess, ServerConfig};

fn server(mode: &str, access: RiftBuildAccess) -> MeridianServer {
    let root =
        std::env::temp_dir().join(format!("meridian-mcp-mode-{mode}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let access = match access {
        RiftBuildAccess::Disabled => None,
        RiftBuildAccess::Offline => Some("offline"),
        RiftBuildAccess::Network => Some("network"),
    };
    MeridianServer::new(
        ServerConfig::from_values_with_rift_build(Some(mode), vec![root], Vec::new(), access)
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn mode_inventories_are_exact_and_exclude_removed_protocol() {
    let analysis = server("analysis", RiftBuildAccess::Network).tool_names();
    let development = server("development", RiftBuildAccess::Disabled).tool_names();
    let development_offline = server("development", RiftBuildAccess::Offline).tool_names();
    let development_network = server("development", RiftBuildAccess::Network).tool_names();
    assert_eq!(analysis.len(), 20);
    assert_eq!(development.len(), 29);
    for tool in [
        "dm_document_symbols",
        "dm_find_references",
        "dm_find_implementations",
        "dm_dmi_info",
        "dm_compare_dmi_states",
        "dm_find_dmi_duplicates",
        "dm_audit_icons",
        "dm_diff_maps",
        "dm_list_render_passes",
    ] {
        assert!(
            analysis.contains(&tool.to_owned()),
            "missing analysis tool {tool}"
        );
    }
    for tool in ["dm_extract_dmi", "dm_render_maps"] {
        assert!(!analysis.contains(&tool.to_owned()));
        assert!(
            development.contains(&tool.to_owned()),
            "missing development tool {tool}"
        );
    }
    assert!(analysis.contains(&"dm_parse_environment".to_owned()));
    assert!(!analysis.contains(&"dm_compile".to_owned()));
    assert!(development.contains(&"dm_compile".to_owned()));
    assert!(!development.contains(&"dm_connect_test".to_owned()));
    assert!(!analysis.contains(&"rift_compile".to_owned()));
    assert!(!development.contains(&"rift_compile".to_owned()));
    #[cfg(windows)]
    {
        assert!(development_offline.contains(&"rift_compile".to_owned()));
        assert!(development_network.contains(&"rift_compile".to_owned()));
    }
    #[cfg(not(windows))]
    {
        assert!(!development_offline.contains(&"rift_compile".to_owned()));
        assert!(!development_network.contains(&"rift_compile".to_owned()));
    }
    assert_eq!(CapabilityMode::Analysis, CapabilityMode::Analysis);
}

#[test]
fn rift_compile_schema_has_no_caller_controlled_paths_or_commands() {
    let definitions = meridian_mcp::tools::get_tool_definitions();
    let tool = definitions
        .iter()
        .find(|tool| tool.name == "rift_compile")
        .expect("rift_compile schema should be registered");
    let properties = tool.input_schema["properties"]
        .as_object()
        .expect("properties must be an object");
    let names = properties
        .keys()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        names,
        [
            "network_mode",
            "timeout_ms",
            "idle_timeout_ms",
            "capture_network",
            "force_rebuild",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        properties["network_mode"]["enum"],
        serde_json::json!(["offline", "allow"])
    );
    assert_eq!(properties["timeout_ms"]["minimum"], 1_000);
    assert_eq!(properties["timeout_ms"]["maximum"], 1_800_000);
    assert_eq!(properties["idle_timeout_ms"]["minimum"], 1_000);
    assert_eq!(properties["idle_timeout_ms"]["maximum"], 900_000);
}
