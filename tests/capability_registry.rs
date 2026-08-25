use meridian_mcp::capabilities::{
    capability_registry, validate_capability_registry, CapabilityDisposition, SPACEMANDMM_REVISION,
};
use std::collections::HashSet;

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
fn every_public_tool_has_at_least_one_registry_mapping() {
    let registry = capability_registry().expect("checked-in capability registry should parse");
    let mapped: HashSet<_> = registry
        .capabilities
        .iter()
        .flat_map(|record| record.targets.iter().map(String::as_str))
        .collect();

    for contract in meridian_mcp::all_contracts() {
        assert!(
            mapped.contains(contract.name),
            "{} has no SpacemanDMM capability mapping",
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
