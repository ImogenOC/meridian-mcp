use meridian_mcp::tracy_protocol::{
    build_request, parse_response, TracyCommand, TracyProtocolError,
};
use serde_json::json;
use tokio::io::{duplex, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[test]
fn request_is_one_line_fixed_command_json() {
    let request = build_request(
        7,
        TracyCommand::FrameStats,
        json!({"trace_path":"trace.tracy"}),
    )
    .expect("build request");
    assert!(request.ends_with(b"\n"));
    assert_eq!(request.iter().filter(|byte| **byte == b'\n').count(), 1);
    let value: serde_json::Value = serde_json::from_slice(&request).unwrap();
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["id"], 7);
    assert_eq!(value["command"], "frame_stats");
    assert_eq!(value["params"]["trace_path"], "trace.tracy");
}

#[test]
fn response_requires_one_matching_bounded_envelope() {
    let parsed = parse_response(
        9,
        br#"{"schema_version":2,"id":9,"ok":true,"result":{"frame_count":4}}
"#,
    )
    .unwrap();
    assert_eq!(parsed["frame_count"], 4);

    assert!(matches!(
        parse_response(9, br#"{"schema_version":2,"id":8,"ok":true,"result":{}}"#),
        Err(TracyProtocolError::ResponseId { .. })
    ));
    assert!(matches!(
        parse_response(9, b"{}\n{}\n"),
        Err(TracyProtocolError::MultipleResponses)
    ));
    assert!(matches!(
        parse_response(9, br#"{"schema_version":2,"id":9,"ok":false,"error":{"code":"bad_trace","message":"bad"}}"#),
        Err(TracyProtocolError::Helper { .. })
    ));

    let error = parse_response(
        9,
        br#"{"schema_version":2,"id":9,"ok":false,"error":{"code":"invalid_capture","message":"bad","details":{"error_codes":["no_complete_frames"]}}}"#,
    )
    .unwrap_err();
    match error {
        TracyProtocolError::Helper { details, .. } => {
            assert_eq!(details["error_codes"][0], "no_complete_frames");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn persistent_transport_multiplexes_out_of_order_responses() {
    let (client_read, mut server_write) = duplex(4096);
    let (server_read, client_write) = duplex(4096);
    let transport = meridian_mcp::tracy_collector::CollectorTransport::new(
        client_read,
        client_write,
        std::time::Duration::from_secs(1),
    );
    let first = {
        let transport = transport.clone();
        tokio::spawn(async move { transport.request("session_status", json!({})).await })
    };
    let second = {
        let transport = transport.clone();
        tokio::spawn(async move { transport.request("cancel", json!({})).await })
    };
    let mut requests = BufReader::new(server_read).lines();
    let mut ids = Vec::new();
    for _ in 0..2 {
        let request: serde_json::Value =
            serde_json::from_str(&requests.next_line().await.unwrap().unwrap()).unwrap();
        ids.push(request["id"].as_u64().unwrap());
    }
    server_write
        .write_all(
            format!(
                "{{\"schema_version\":2,\"id\":{},\"ok\":true,\"result\":{{\"order\":2}}}}\n",
                ids[1]
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    server_write
        .write_all(
            format!(
                "{{\"schema_version\":2,\"id\":{},\"ok\":true,\"result\":{{\"order\":1}}}}\n",
                ids[0]
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    assert_eq!(first.await.unwrap().unwrap()["order"], 1);
    assert_eq!(second.await.unwrap().unwrap()["order"], 2);
}

#[tokio::test]
async fn persistent_transport_rejects_unknown_response_ids() {
    let (client_read, mut server_write) = duplex(4096);
    let (_server_read, client_write) = duplex(4096);
    let transport = meridian_mcp::tracy_collector::CollectorTransport::new(
        client_read,
        client_write,
        std::time::Duration::from_secs(1),
    );
    server_write
        .write_all(b"{\"schema_version\":2,\"id\":999,\"ok\":true,\"result\":{}}\n")
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert!(matches!(
        transport.request("session_status", json!({})).await,
        Err(TracyProtocolError::Transport(_))
    ));
}

#[test]
fn arbitrary_commands_cannot_be_constructed() {
    let names = [
        TracyCommand::Capture,
        TracyCommand::Hotspots,
        TracyCommand::Zone,
        TracyCommand::FrameStats,
        TracyCommand::Compare,
    ]
    .map(TracyCommand::as_str);
    assert_eq!(
        names,
        ["capture", "hotspots", "zone", "frame_stats", "compare"]
    );
}
