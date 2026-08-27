use meridian_mcp::build_identity::{BuildIdentity, BuildIdentityInput};

#[test]
fn build_identity_changes_when_source_or_binary_identity_changes() {
    let baseline = BuildIdentity::from_input(BuildIdentityInput {
        version: "0.1.0".to_owned(),
        source_revision: Some("a".repeat(40)),
        source_dirty: Some(false),
        target: "x86_64-pc-windows-msvc".to_owned(),
        profile: "release".to_owned(),
        executable_sha256: Some("1".repeat(64)),
    });
    let different_source = BuildIdentity::from_input(BuildIdentityInput {
        source_revision: Some("b".repeat(40)),
        ..baseline.as_input()
    });
    let different_binary = BuildIdentity::from_input(BuildIdentityInput {
        executable_sha256: Some("2".repeat(64)),
        ..baseline.as_input()
    });

    assert!(baseline.complete);
    assert_ne!(baseline.build_id, different_source.build_id);
    assert_ne!(baseline.build_id, different_binary.build_id);
}

#[test]
fn running_build_identity_hashes_the_current_executable() {
    let identity = meridian_mcp::build_identity::current();
    assert_eq!(identity.version, env!("CARGO_PKG_VERSION"));
    assert!(!identity.target.is_empty());
    assert!(!identity.profile.is_empty());
    assert_eq!(
        identity.executable_sha256.as_deref().map(str::len),
        Some(64)
    );
}

#[test]
fn local_dirty_detection_includes_untracked_source_files() {
    let build_script =
        std::fs::read_to_string("build.rs").expect("build script should be readable");
    assert!(build_script.contains("--untracked-files=all"));
    assert!(!build_script.contains("--untracked-files=no"));
}
