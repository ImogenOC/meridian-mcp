use meridian_mcp::{CapabilityMode, PathPolicy, RiftBuildAccess, ServerConfig, TracyAccess};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn fixture() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "meridian-mcp-policy-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn rift_build_access_is_disabled_by_default_and_strict_when_configured() {
    let root = fixture();
    let compatible =
        ServerConfig::from_values(Some("development"), vec![root.clone()], Vec::new()).unwrap();
    assert_eq!(compatible.rift_build_access(), RiftBuildAccess::Disabled);

    let offline = ServerConfig::from_values_with_rift_build(
        Some("development"),
        vec![root.clone()],
        Vec::new(),
        Some("offline"),
    )
    .unwrap();
    assert_eq!(offline.rift_build_access(), RiftBuildAccess::Offline);

    let network = ServerConfig::from_values_with_rift_build(
        Some("development"),
        vec![root.clone()],
        Vec::new(),
        Some("network"),
    )
    .unwrap();
    assert_eq!(network.rift_build_access(), RiftBuildAccess::Network);

    assert!(ServerConfig::from_values_with_rift_build(
        Some("development"),
        vec![root.clone()],
        Vec::new(),
        Some("internet"),
    )
    .is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn analysis_is_the_default_and_development_is_explicit() {
    let root = fixture();
    let analysis = ServerConfig::from_values(None, vec![root.clone()], Vec::new()).unwrap();
    assert_eq!(analysis.mode(), CapabilityMode::Analysis);
    let development =
        ServerConfig::from_values(Some("development"), vec![root.clone()], Vec::new()).unwrap();
    assert_eq!(development.mode(), CapabilityMode::Development);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn tracy_is_disabled_by_default_and_requires_development_with_a_manifest() {
    let root = fixture();
    let default =
        ServerConfig::from_values(Some("development"), vec![root.clone()], Vec::new()).unwrap();
    assert_eq!(default.tracy_access(), TracyAccess::Disabled);

    assert!(ServerConfig::from_values_with_features(
        Some("analysis"),
        vec![root.clone()],
        Vec::new(),
        None,
        Some("byond"),
        Some(root.join("manifest.json")),
    )
    .is_err());
    assert!(ServerConfig::from_values_with_features(
        Some("development"),
        vec![root.clone()],
        Vec::new(),
        None,
        Some("byond"),
        None,
    )
    .is_err());
    assert!(ServerConfig::from_values_with_features(
        Some("development"),
        vec![root.clone()],
        Vec::new(),
        None,
        Some("remote"),
        Some(root.join("manifest.json")),
    )
    .is_err());
    std::fs::write(root.join("manifest.json"), "{}").unwrap();
    let enabled = ServerConfig::from_values_with_features(
        Some("development"),
        vec![root.clone()],
        Vec::new(),
        None,
        Some("byond"),
        Some(root.join("manifest.json")),
    )
    .unwrap();
    assert_eq!(enabled.tracy_access(), TracyAccess::Byond);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn traversal_and_unlisted_executables_are_rejected() {
    let root = fixture();
    let outside = root.parent().unwrap().join("outside.dm");
    std::fs::write(&outside, "outside").unwrap();
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();
    assert_eq!(
        policy.read_path(&outside).unwrap_err().code(),
        "path_outside_workspace"
    );
    assert_eq!(
        policy.executable(&outside).unwrap_err().code(),
        "executable_not_allowed"
    );
    std::fs::remove_file(outside).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn outputs_require_containment_and_explicit_overwrite() {
    let root = fixture();
    let output = root.join("map.png");
    std::fs::write(&output, "existing").unwrap();
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();
    assert_eq!(
        policy.output_path(&output, false).unwrap_err().code(),
        "output_exists"
    );
    assert_eq!(
        policy.output_path(&output, true).unwrap(),
        output.canonicalize().unwrap()
    );
    std::fs::remove_dir_all(root).unwrap();
}
