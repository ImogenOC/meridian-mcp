use meridian_mcp::{CapabilityMode, MeridianServer, RiftBuildAccess, ServerConfig};
use sha2::{Digest, Sha256};

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

fn tracy_server() -> MeridianServer {
    let root = std::env::temp_dir().join(format!("meridian-mcp-tracy-mode-{}", std::process::id()));
    std::fs::create_dir_all(root.join("helpers")).unwrap();
    let server_helper = root.join("helpers/server-helper.exe");
    let hook = root.join("helpers/prof.dll");
    std::fs::write(&server_helper, b"server helper").unwrap();
    std::fs::write(&hook, b"hook").unwrap();
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let manifest = root.join("manifest.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 2,
            "helpers": [
                {"id":"tracy-server-helper","platform":std::env::consts::OS,"target_arch":std::env::consts::ARCH,"path":"helpers/server-helper.exe","sha256":hash(b"server helper"),"source_revision":"099df3de3dc37eca4712c06b8320fb9c53596edd","protocol_version":82},
                {"id":"byond-tracy","platform":std::env::consts::OS,"target_arch":"x86","path":"helpers/prof.dll","sha256":hash(b"hook"),"source_revision":"d1ec404737b04b1ea73d6df4a1b477deacdb1900","protocol_version":82,"byond_min_version":"516.1687","byond_max_version":"516.1687"}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    MeridianServer::new(
        ServerConfig::from_values_with_features(
            Some("development"),
            vec![root],
            Vec::new(),
            None,
            Some("byond"),
            Some(manifest),
        )
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

#[test]
fn tracy_inventory_is_opt_in_and_exposes_fixed_command_tools_only() {
    let ordinary = server("development", RiftBuildAccess::Disabled).tool_names();
    assert!(!ordinary.iter().any(|name| name.starts_with("dm_tracy_")));

    let tracy = tracy_server().tool_names();
    let tracy_tools = tracy
        .iter()
        .filter(|name| name.starts_with("dm_tracy_"))
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        tracy_tools,
        [
            "dm_tracy_prepare",
            "dm_tracy_launch",
            "dm_tracy_capture",
            "dm_tracy_status",
            "dm_tracy_stop",
            "dm_tracy_hotspots",
            "dm_tracy_zone",
            "dm_tracy_frame_stats",
            "dm_tracy_compare",
        ]
        .into_iter()
        .collect()
    );
    assert!(!tracy.iter().any(|name| name.contains("eval")));
}
