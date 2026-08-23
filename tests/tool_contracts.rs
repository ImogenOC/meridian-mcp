use meridian_mcp::{all_contracts, contracts_for, render_tool_reference, CapabilityMode};
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
        }));
}

#[test]
fn checked_in_reference_matches_contract_registry() {
    let expected = render_tool_reference(all_contracts());
    let actual = std::fs::read_to_string("docs/tool-contracts.md").unwrap();
    assert_eq!(actual, expected);
}
