use meridian_mcp::{
    all_contracts, contracts_for, contracts_for_configuration, render_tool_reference,
    CapabilityMode, RiftBuildAccess,
};
use std::collections::HashSet;

#[test]
fn contracts_are_unique_bounded_and_analysis_is_read_only() {
    let contracts = all_contracts();
    let names: HashSet<_> = contracts.iter().map(|contract| contract.name).collect();
    assert_eq!(names.len(), contracts.len());
    assert!(contracts
        .iter()
        .all(|contract| !contract.summary.is_empty()));
    assert!(contracts
        .iter()
        .all(|contract| contract.max_output_bytes > 0));
    assert!(contracts_for(CapabilityMode::Analysis)
        .iter()
        .all(|contract| {
            !contract.effects.writes_files
                && !contract.effects.spawns_process
                && !contract.effects.network_loopback
                && !contract.effects.network_external
        }));
    let parse = all_contracts()
        .iter()
        .find(|contract| contract.name == "dm_parse_environment")
        .expect("dm_parse_environment must have a maximum contract");
    assert_eq!(parse.timeout_ms, Some(1_800_000));
}

#[test]
fn rift_compile_contract_respects_mode_access_and_platform() {
    let names = |mode, access| {
        contracts_for_configuration(mode, access)
            .into_iter()
            .map(|contract| contract.name)
            .collect::<HashSet<_>>()
    };

    assert!(!names(CapabilityMode::Analysis, RiftBuildAccess::Network).contains("rift_compile"));
    assert!(
        !names(CapabilityMode::Development, RiftBuildAccess::Disabled).contains("rift_compile")
    );

    #[cfg(windows)]
    {
        assert!(
            names(CapabilityMode::Development, RiftBuildAccess::Offline).contains("rift_compile")
        );
        assert!(
            names(CapabilityMode::Development, RiftBuildAccess::Network).contains("rift_compile")
        );
    }
    #[cfg(not(windows))]
    {
        assert!(
            !names(CapabilityMode::Development, RiftBuildAccess::Offline).contains("rift_compile")
        );
        assert!(
            !names(CapabilityMode::Development, RiftBuildAccess::Network).contains("rift_compile")
        );
    }

    let contract = all_contracts()
        .iter()
        .find(|contract| contract.name == "rift_compile")
        .expect("rift_compile must have a maximum contract");
    assert!(contract.effects.network_external);
    assert_eq!(contract.timeout_ms, Some(1_800_000));
}

#[test]
fn checked_in_reference_matches_contract_registry() {
    let expected = render_tool_reference(all_contracts());
    let actual = std::fs::read_to_string("docs/tool-contracts.md").unwrap();
    assert_eq!(actual, expected);
}
