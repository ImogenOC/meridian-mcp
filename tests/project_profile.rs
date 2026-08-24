use meridian_mcp::{PathPolicy, ProjectProfile};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn fixture(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "meridian-mcp-profile-{name}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn write_qualified_fixture(root: &Path, dme_name: &str) -> PathBuf {
    let dme = root.join(dme_name);
    std::fs::write(&dme, "// fixture").unwrap();
    std::fs::write(root.join("SpacemanDMM.toml"), "[dreamchecker]").unwrap();
    std::fs::write(
        root.join("dependencies.sh"),
        "export BYOND_MAJOR=516\nexport BYOND_MINOR=1685\n",
    )
    .unwrap();
    std::fs::write(root.join("BUILD.cmd"), "@echo off\n").unwrap();
    std::fs::write(root.join("RIFT_BUILD.cmd"), "@echo off\n").unwrap();
    dme
}

#[test]
fn profile_discovers_separate_human_and_agent_build_entrypoints() {
    let root = fixture("qualified");
    let dme = write_qualified_fixture(&root, "tgstation.dme");
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();
    let profile = ProjectProfile::discover(&policy, &dme).unwrap();
    assert_eq!(profile.root(), root.canonicalize().unwrap().as_path());
    assert_eq!(profile.dme_path(), dme.canonicalize().unwrap().as_path());
    assert_eq!(profile.byond_version(), Some("516.1685"));
    assert!(profile
        .spaceman_config()
        .unwrap()
        .ends_with("SpacemanDMM.toml"));
    assert!(profile
        .human_build_entrypoint()
        .unwrap()
        .ends_with("BUILD.cmd"));
    assert!(profile
        .rift_build_entrypoint()
        .unwrap()
        .ends_with("RIFT_BUILD.cmd"));
    assert!(profile.is_rift_build_qualified());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn qualification_requires_canonical_meridian_files_and_literal_version() {
    let cases = [
        ("wrong-dme", "other.dme", None, None),
        ("missing-human", "tgstation.dme", Some("BUILD.cmd"), None),
        (
            "missing-wrapper",
            "tgstation.dme",
            Some("RIFT_BUILD.cmd"),
            None,
        ),
        (
            "computed-version",
            "tgstation.dme",
            None,
            Some("export BYOND_MAJOR=${BYOND_CHANNEL}\nexport BYOND_MINOR=1685\n"),
        ),
    ];

    for (name, dme_name, remove, dependencies) in cases {
        let root = fixture(name);
        let dme = write_qualified_fixture(&root, dme_name);
        if let Some(remove) = remove {
            std::fs::remove_file(root.join(remove)).unwrap();
        }
        if let Some(dependencies) = dependencies {
            std::fs::write(root.join("dependencies.sh"), dependencies).unwrap();
        }
        let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();
        let profile = ProjectProfile::discover(&policy, &dme).unwrap();
        assert!(!profile.is_rift_build_qualified(), "case {name} qualified");
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn build_entrypoint_cannot_resolve_outside_the_workspace() {
    let root = fixture("link-root");
    let outside = fixture("link-outside");
    let dme = write_qualified_fixture(&root, "tgstation.dme");
    std::fs::remove_file(root.join("RIFT_BUILD.cmd")).unwrap();
    let outside_script = outside.join("RIFT_BUILD.cmd");
    std::fs::write(&outside_script, "@echo off\n").unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_script, root.join("RIFT_BUILD.cmd")).unwrap();
    #[cfg(windows)]
    if let Err(error) =
        std::os::windows::fs::symlink_file(&outside_script, root.join("RIFT_BUILD.cmd"))
    {
        eprintln!("skipping symlink containment assertion: {error}");
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(outside).unwrap();
        return;
    }

    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();
    let error = ProjectProfile::discover(&policy, &dme).unwrap_err();
    assert_eq!(error.code(), "path_outside_workspace");
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(outside).unwrap();
}
