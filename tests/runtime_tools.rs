use meridian_mcp::network_audit::{
    tracy_network_evidence, EndpointObservation, EndpointProtocol, NetworkAuditReport,
};
use meridian_mcp::process_metrics::{ProcessIdentity, ProcessRole};

#[cfg(unix)]
#[test]
fn renamed_unix_server_accepts_stdio_eof() {
    use std::io::{BufRead, Write};
    let directory = std::env::temp_dir().join(format!("meridian-renamed-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let executable = directory.join("owned-renamed-server");
    std::fs::copy(env!("CARGO_BIN_EXE_meridian-mcp"), &executable).unwrap();
    let mut child = std::process::Command::new(executable)
        .env_clear()
        .env("MERIDIAN_MCP_MODE", "analysis")
        .env("MERIDIAN_MCP_ROOTS", &directory)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    writeln!(input, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-11-25","capabilities":{{}},"clientInfo":{{"name":"renamed-fixture","version":"1"}}}}}}"#).unwrap();
    let mut response = String::new();
    std::io::BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut response)
        .unwrap();
    if !response.is_empty() {
        writeln!(
            input,
            r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
        )
        .unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(response["id"], 1);
    assert!(response.get("result").is_some());
}

#[cfg(unix)]
#[test]
fn unix_library_embedding_requires_an_explicit_guardian_executable() {
    assert!(meridian_mcp::process::initialize_runtime_owner().is_err());
    meridian_mcp::process::initialize_runtime_owner_with_executable(std::path::Path::new(env!(
        "CARGO_BIN_EXE_meridian-mcp"
    )))
    .unwrap();
}

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
