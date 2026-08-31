use meridian_mcp::capabilities::{BYOND_TRACY_REVISION, TRACY_PROTOCOL_VERSION, TRACY_REVISION};
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::tracy::TracyInstallation;
use meridian_mcp::{CapabilityMode, PathPolicy, RiftBuildAccess};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn fixture() -> (std::path::PathBuf, ToolExecutionContext) {
    let root = std::env::temp_dir().join(format!(
        "meridian-tracy-tools-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("helpers")).unwrap();
    let helper = root.join("helpers/meridian-tracy-helper.exe");
    let hook = root.join("helpers/prof.dll");
    std::fs::write(&helper, b"helper").unwrap();
    std::fs::write(&hook, b"verified hook").unwrap();
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let manifest = root.join("manifest.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&serde_json::json!({"schema_version":2,"helpers":[
            {"id":"tracy-server-helper","platform":std::env::consts::OS,"target_arch":std::env::consts::ARCH,"path":"helpers/meridian-tracy-helper.exe","sha256":hash(b"helper"),"source_revision":TRACY_REVISION,"protocol_version":TRACY_PROTOCOL_VERSION},
            {"id":"byond-tracy","platform":std::env::consts::OS,"target_arch":"x86","path":"helpers/prof.dll","sha256":hash(b"verified hook"),"source_revision":BYOND_TRACY_REVISION,"protocol_version":TRACY_PROTOCOL_VERSION,"byond_min_version":"516.1685","byond_max_version":"516.1687"}
        ]})).unwrap(),
    ).unwrap();
    let installation = TracyInstallation::validate(&manifest).unwrap();
    let context = ToolExecutionContext::with_features(
        CapabilityMode::Development,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
        RiftBuildAccess::Disabled,
        None,
        None,
        Some(installation),
    );
    (root, context)
}

#[test]
fn actual_byond_version_is_checked_against_the_verified_hook_range() {
    let (root, _) = fixture();
    let installation = TracyInstallation::validate(&root.join("manifest.json")).unwrap();

    assert_eq!(
        installation.validate_byond_version("516.1685").unwrap(),
        "516.1685"
    );
    assert_eq!(
        installation.validate_byond_version("1685").unwrap(),
        "516.1685"
    );
    assert_eq!(
        installation.validate_byond_version("516.1687").unwrap(),
        "516.1687"
    );
    assert!(installation.validate_byond_version("516.1684").is_err());
    assert!(installation.validate_byond_version("516.1688").is_err());
    assert!(installation.validate_byond_version("unknown").is_err());
}

#[tokio::test]
async fn prepare_is_hash_verified_atomic_and_idempotent() {
    let (root, context) = fixture();
    let dmb = root.join("game.dmb");
    std::fs::write(&dmb, b"dmb").unwrap();

    let first = call_tool(
        &context,
        &ServerState::new(),
        "dm_tracy_prepare",
        serde_json::json!({"dmb_path":dmb}),
    )
    .await
    .unwrap();
    assert_ne!(first.is_error, Some(true));
    assert_eq!(
        std::fs::read(root.join("prof.dll")).unwrap(),
        b"verified hook"
    );

    let second = call_tool(
        &context,
        &ServerState::new(),
        "dm_tracy_prepare",
        serde_json::json!({"dmb_path":dmb}),
    )
    .await
    .unwrap();
    assert_ne!(second.is_error, Some(true));

    std::fs::write(root.join("prof.dll"), b"different").unwrap();
    assert!(call_tool(
        &context,
        &ServerState::new(),
        "dm_tracy_prepare",
        serde_json::json!({"dmb_path":dmb}),
    )
    .await
    .is_err());

    call_tool(
        &context,
        &ServerState::new(),
        "dm_tracy_prepare",
        serde_json::json!({"dmb_path":dmb,"overwrite":true}),
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read(root.join("prof.dll")).unwrap(),
        b"verified hook"
    );
    std::fs::remove_dir_all(root).unwrap();
}
