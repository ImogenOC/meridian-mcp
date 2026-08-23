use meridian_mcp::{CapabilityMode, MeridianServer, ServerConfig};

fn server(mode: &str) -> MeridianServer {
    let root =
        std::env::temp_dir().join(format!("meridian-mcp-mode-{mode}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    MeridianServer::new(ServerConfig::from_values(Some(mode), vec![root], Vec::new()).unwrap())
        .unwrap()
}

#[test]
fn mode_inventories_are_exact_and_exclude_removed_protocol() {
    let analysis = server("analysis").tool_names();
    let development = server("development").tool_names();
    assert_eq!(analysis.len(), 11);
    assert_eq!(development.len(), 18);
    assert!(analysis.contains(&"dm_parse_environment".to_owned()));
    assert!(!analysis.contains(&"dm_compile".to_owned()));
    assert!(development.contains(&"dm_compile".to_owned()));
    assert!(!development.contains(&"dm_connect_test".to_owned()));
    assert_eq!(CapabilityMode::Analysis, CapabilityMode::Analysis);
}
