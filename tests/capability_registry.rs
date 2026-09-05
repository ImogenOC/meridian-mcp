use meridian_mcp::capabilities::{
    capability_registry, tracy_capability_registry, validate_capability_registry,
    validate_tracy_capability_registry, CapabilityDisposition, BYOND_TRACY_REVISION,
    SPACEMANDMM_REVISION, TRACY_PROTOCOL_VERSION, TRACY_REVISION,
};
use std::collections::HashSet;

#[test]
fn vendor_audit_rejects_changed_missing_and_unrecorded_sources() {
    use std::path::Path;
    use std::process::Command;

    fn copy_tree(source: &Path, destination: &Path) {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    struct OwnedFixture(std::path::PathBuf);
    impl Drop for OwnedFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    let fixture = OwnedFixture(std::env::temp_dir().join(format!(
        "meridian-vendor-audit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    copy_tree(&root.join("vendor"), &fixture.0.join("vendor"));
    std::fs::create_dir_all(fixture.0.join("scripts")).unwrap();
    let script = fixture.0.join("scripts/audit-spacemandmm-capabilities.ps1");
    std::fs::copy(
        root.join("scripts/audit-spacemandmm-capabilities.ps1"),
        &script,
    )
    .unwrap();
    std::fs::copy(
        root.join("spacemandmm-capabilities.json"),
        fixture.0.join("spacemandmm-capabilities.json"),
    )
    .unwrap();
    let audit = || {
        Command::new("pwsh")
            .args(["-NoLogo", "-NoProfile", "-File"])
            .arg(&script)
            .arg("-Check")
            .output()
            .expect("PowerShell must launch")
    };
    let baseline = audit();
    assert!(
        baseline.status.success(),
        "{}",
        String::from_utf8_lossy(&baseline.stderr)
    );

    let source = fixture.0.join("vendor/spacemandmm/dreammaker/src/error.rs");
    let original = std::fs::read_to_string(&source)
        .unwrap()
        .replace("\r\n", "\n");
    std::fs::write(&source, original.replace('\n', "\r\n")).unwrap();
    assert!(
        audit().status.success(),
        "CRLF checkout must retain source identity"
    );

    std::fs::write(&source, format!("{original}\n// unrecorded change\n")).unwrap();
    assert!(
        !audit().status.success(),
        "changed shipped source was accepted"
    );
    std::fs::write(&source, &original).unwrap();
    let extra = source.with_file_name("unrecorded.rs");
    std::fs::write(&extra, "// unrecorded source\n").unwrap();
    assert!(!audit().status.success(), "unrecorded source was accepted");
    std::fs::remove_file(extra).unwrap();
    #[cfg(windows)]
    {
        let hidden = fixture.0.join("vendor/spacemandmm/dmm-tools/build.rs");
        std::fs::write(&hidden, "fn main() {}\n").unwrap();
        let hide_script = fixture.0.join("hide-source.ps1");
        std::fs::write(
            &hide_script,
            "$item = Get-Item -LiteralPath $args[0]\n$item.Attributes = $item.Attributes -bor [IO.FileAttributes]::Hidden\n",
        )
        .unwrap();
        assert!(Command::new("pwsh")
            .args(["-NoLogo", "-NoProfile", "-File"])
            .arg(hide_script)
            .arg(&hidden)
            .status()
            .unwrap()
            .success());
        assert!(
            !audit().status.success(),
            "hidden Cargo build script was accepted"
        );
        std::fs::remove_file(hidden).unwrap();
    }
    std::fs::remove_file(source).unwrap();
    assert!(
        !audit().status.success(),
        "missing shipped source was accepted"
    );
}

#[test]
fn checked_in_registry_is_complete_and_consistent() {
    let registry = capability_registry().expect("checked-in capability registry should parse");

    assert_eq!(registry.spacemandmm_revision, SPACEMANDMM_REVISION);
    assert_eq!(validate_capability_registry(&registry), Ok(()));
    assert!(registry.capabilities.iter().any(|record| {
        record.disposition == CapabilityDisposition::Excluded
            && record
                .rationale
                .as_deref()
                .is_some_and(|value| !value.is_empty())
    }));
}

#[test]
fn tracy_registry_pins_sources_protocol_and_excludes_the_python_mcp_transport() {
    let registry = tracy_capability_registry().expect("checked-in Tracy registry should parse");

    assert_eq!(registry.tracy_revision, TRACY_REVISION);
    assert_eq!(registry.byond_tracy_revision, BYOND_TRACY_REVISION);
    assert_eq!(registry.protocol_version, TRACY_PROTOCOL_VERSION);
    assert_eq!(validate_tracy_capability_registry(&registry), Ok(()));
    assert!(registry.capabilities.iter().any(|record| {
        record.id == "tracy-python-mcp"
            && record.disposition == CapabilityDisposition::Excluded
            && record
                .rationale
                .as_deref()
                .is_some_and(|value| value.contains("arbitrary Python"))
    }));
}

#[test]
fn every_public_tool_has_at_least_one_registry_mapping() {
    let registry = capability_registry().expect("checked-in capability registry should parse");
    let tracy = tracy_capability_registry().expect("checked-in Tracy registry should parse");
    let mapped: HashSet<_> = registry
        .capabilities
        .iter()
        .chain(tracy.capabilities.iter())
        .flat_map(|record| record.targets.iter().map(String::as_str))
        .collect();

    for contract in meridian_mcp::all_contracts() {
        assert!(
            mapped.contains(contract.name),
            "{} has no capability mapping",
            contract.name
        );
    }
}

#[test]
fn every_record_has_a_unique_identity_and_verification_gate() {
    let registry = capability_registry().expect("checked-in capability registry should parse");
    let mut identities = HashSet::new();

    for record in &registry.capabilities {
        assert!(
            identities.insert(record.id.as_str()),
            "duplicate {}",
            record.id
        );
        assert!(
            !record.category.trim().is_empty(),
            "{} has no category",
            record.id
        );
        assert!(
            !record.upstream_component.trim().is_empty(),
            "{} has no upstream component",
            record.id
        );
        assert!(
            !record.verification.trim().is_empty(),
            "{} has no verification gate",
            record.id
        );
        if record.disposition == CapabilityDisposition::Excluded {
            assert!(
                record
                    .rationale
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty()),
                "{} has no exclusion rationale",
                record.id
            );
        }
    }
}
