use meridian_mcp::helper_manifest::{verified_helper, HelperRequest, ManifestError};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    manifest: PathBuf,
    helper: PathBuf,
    hash: String,
}

impl Fixture {
    fn new() -> Self {
        let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "meridian-helper-manifest-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("bin")).expect("create fixture directory");
        let helper = root.join("bin/helper.exe");
        std::fs::write(&helper, b"verified helper bytes").expect("write helper");
        let hash = format!("{:x}", Sha256::digest(b"verified helper bytes"));
        let manifest = root.join("manifest.json");
        Self {
            root,
            manifest,
            helper,
            hash,
        }
    }

    fn write(&self, value: serde_json::Value) {
        std::fs::write(
            &self.manifest,
            serde_json::to_vec_pretty(&value).expect("serialize manifest"),
        )
        .expect("write manifest");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn request<'a>(id: &'a str, revision: &'a str) -> HelperRequest<'a> {
    HelperRequest {
        id,
        platform: "windows",
        target_arch: "x86_64",
        source_revision: revision,
        protocol_version: None,
        byond_version: None,
    }
}

fn v2_entry(fixture: &Fixture) -> serde_json::Value {
    serde_json::json!({
        "id": "tracy-server-helper",
        "platform": "windows",
        "target_arch": "x86_64",
        "path": "bin/helper.exe",
        "sha256": fixture.hash,
        "source_revision": "tracy-revision",
        "protocol_version": 82,
        "byond_min_version": "516.1685",
        "byond_max_version": "516.1687"
    })
}

#[test]
fn schema_v2_selects_and_verifies_the_exact_helper_identity() {
    let fixture = Fixture::new();
    fixture.write(serde_json::json!({
        "schema_version": 2,
        "helpers": [v2_entry(&fixture)]
    }));

    let mut expected = request("tracy-server-helper", "tracy-revision");
    expected.protocol_version = Some(82);
    expected.byond_version = Some("516.1685");
    let verified = verified_helper(&fixture.manifest, expected).expect("verify helper");

    assert_eq!(verified.path, fixture.helper.canonicalize().unwrap());
    assert_eq!(verified.id, "tracy-server-helper");
    assert_eq!(verified.protocol_version, Some(82));
    assert_eq!(verified.sha256, fixture.hash);
}

#[test]
fn schema_v2_rejects_arch_revision_protocol_and_byond_mismatches() {
    let fixture = Fixture::new();
    fixture.write(serde_json::json!({
        "schema_version": 2,
        "helpers": [v2_entry(&fixture)]
    }));

    let mut wrong_arch = request("tracy-server-helper", "tracy-revision");
    wrong_arch.target_arch = "x86";
    assert!(matches!(
        verified_helper(&fixture.manifest, wrong_arch),
        Err(ManifestError::NoMatch { .. })
    ));

    assert!(matches!(
        verified_helper(
            &fixture.manifest,
            request("tracy-server-helper", "wrong-revision")
        ),
        Err(ManifestError::Revision { .. })
    ));

    let mut wrong_protocol = request("tracy-server-helper", "tracy-revision");
    wrong_protocol.protocol_version = Some(81);
    assert!(matches!(
        verified_helper(&fixture.manifest, wrong_protocol),
        Err(ManifestError::Protocol { .. })
    ));

    let mut unsupported_byond = request("tracy-server-helper", "tracy-revision");
    unsupported_byond.byond_version = Some("516.1688");
    assert!(matches!(
        verified_helper(&fixture.manifest, unsupported_byond),
        Err(ManifestError::ByondVersion { .. })
    ));
}

#[test]
fn manifest_rejects_duplicate_id_platform_arch_entries_and_traversal() {
    let fixture = Fixture::new();
    let entry = v2_entry(&fixture);
    fixture.write(serde_json::json!({
        "schema_version": 2,
        "helpers": [entry.clone(), entry]
    }));
    assert!(matches!(
        verified_helper(
            &fixture.manifest,
            request("tracy-server-helper", "tracy-revision")
        ),
        Err(ManifestError::DuplicateIdentity { .. })
    ));

    let outside = fixture.root.parent().unwrap().join("outside-helper.exe");
    std::fs::write(&outside, b"outside").unwrap();
    fixture.write(serde_json::json!({
        "schema_version": 2,
        "helpers": [{
            "id": "tracy-server-helper",
            "platform": "windows",
            "target_arch": "x86_64",
            "path": Path::new("..").join("outside-helper.exe"),
            "sha256": format!("{:x}", Sha256::digest(b"outside")),
            "source_revision": "tracy-revision"
        }]
    }));
    assert!(matches!(
        verified_helper(
            &fixture.manifest,
            request("tracy-server-helper", "tracy-revision")
        ),
        Err(ManifestError::OutsideManifestRoot { .. })
    ));
    let _ = std::fs::remove_file(outside);
}

#[test]
fn legacy_schema_v1_is_read_as_a_dmdoc_entry() {
    let fixture = Fixture::new();
    fixture.write(serde_json::json!({
        "schema_version": 1,
        "helpers": [{
            "platform": "windows-x86_64",
            "path": "bin/helper.exe",
            "sha256": fixture.hash,
            "source_revision": "spaceman-revision"
        }]
    }));

    let verified = verified_helper(&fixture.manifest, request("dmdoc", "spaceman-revision"))
        .expect("legacy manifest remains compatible");
    assert_eq!(verified.id, "dmdoc");
    assert_eq!(verified.path, fixture.helper.canonicalize().unwrap());
}

#[test]
fn helper_bytes_must_match_the_manifest_hash() {
    let fixture = Fixture::new();
    fixture.write(serde_json::json!({
        "schema_version": 2,
        "helpers": [v2_entry(&fixture)]
    }));
    std::fs::write(&fixture.helper, b"tampered").unwrap();

    assert!(matches!(
        verified_helper(
            &fixture.manifest,
            request("tracy-server-helper", "tracy-revision")
        ),
        Err(ManifestError::Checksum { .. })
    ));
}
