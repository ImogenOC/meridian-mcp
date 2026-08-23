use meridian_mcp::{PathPolicy, ProjectProfile};

#[test]
fn profile_discovers_checked_in_meridian_configuration_without_executing_it() {
    let root = std::env::temp_dir().join(format!("meridian-mcp-profile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let dme = root.join("tgstation.dme");
    std::fs::write(&dme, "// fixture").unwrap();
    std::fs::write(root.join("SpacemanDMM.toml"), "[dreamchecker]").unwrap();
    std::fs::write(
        root.join("dependencies.sh"),
        "export BYOND_MAJOR=516\nexport BYOND_MINOR=1685\n",
    )
    .unwrap();
    std::fs::write(root.join("BUILD.cmd"), "@echo off\n").unwrap();
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();
    let profile = ProjectProfile::discover(&policy, &dme).unwrap();
    assert_eq!(profile.byond_version(), Some("516.1685"));
    assert!(profile
        .spaceman_config()
        .unwrap()
        .ends_with("SpacemanDMM.toml"));
    assert!(profile
        .full_build_entrypoint()
        .unwrap()
        .ends_with("BUILD.cmd"));
    std::fs::remove_dir_all(root).unwrap();
}
