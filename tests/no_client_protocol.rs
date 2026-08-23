use meridian_mcp::{contracts_for, CapabilityMode};

#[test]
fn supported_product_has_no_client_login_protocol() {
    assert!(!contracts_for(CapabilityMode::Development)
        .iter()
        .any(|contract| contract.name == "dm_connect_test"));
    for inherited_file in [
        "src/client/mod.rs",
        "src/client/crypto.rs",
        "src/client/packets.rs",
        "src/client/protocol.rs",
    ] {
        assert!(
            !std::path::Path::new(inherited_file).exists(),
            "{inherited_file} remains"
        );
    }
}
