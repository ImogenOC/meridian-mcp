use meridian_mcp::network_audit::{
    tracy_network_evidence, EndpointObservation, EndpointProtocol, NetworkAuditReport,
};
use meridian_mcp::process_metrics::{ProcessIdentity, ProcessRole};

#[test]
fn tracy_network_evidence_is_honest_and_owned_loopback_only() {
    let identity = ProcessIdentity {
        pid: 42,
        started_at_identity: 7,
        role: ProcessRole::DreamDaemon,
    };
    let report = NetworkAuditReport {
        requested: true,
        available: true,
        capture_complete: false,
        truncated: false,
        observations: vec![
            EndpointObservation {
                protocol: EndpointProtocol::Tcp,
                process_id: 42,
                local_endpoint: "127.0.0.1:8086".into(),
                remote_endpoint: None,
                first_seen_ms: 0,
                last_seen_ms: 1,
            },
            EndpointObservation {
                protocol: EndpointProtocol::Tcp,
                process_id: 42,
                local_endpoint: "0.0.0.0:1337".into(),
                remote_endpoint: None,
                first_seen_ms: 0,
                last_seen_ms: 1,
            },
            EndpointObservation {
                protocol: EndpointProtocol::Tcp,
                process_id: 99,
                local_endpoint: "127.0.0.1:8086".into(),
                remote_endpoint: None,
                first_seen_ms: 0,
                last_seen_ms: 1,
            },
        ],
        warning: None,
    };
    let evidence = tracy_network_evidence(report, 8086, &[identity], true, true);
    assert!(!evidence.network_isolation_confirmed);
    assert!(!evidence.capture_complete);
    assert_eq!(evidence.owned_loopback_endpoints.len(), 1);
    assert!(evidence.listener_verified);
    assert!(evidence.collector_connection_verified);
}
