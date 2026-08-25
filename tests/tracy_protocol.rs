use meridian_mcp::tracy_protocol::{
    build_request, parse_response, TracyCommand, TracyProtocolError,
};
use serde_json::json;

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
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["id"], 7);
    assert_eq!(value["command"], "frame_stats");
    assert_eq!(value["params"]["trace_path"], "trace.tracy");
}

#[test]
fn response_requires_one_matching_bounded_envelope() {
    let parsed = parse_response(
        9,
        br#"{"schema_version":1,"id":9,"ok":true,"result":{"frame_count":4}}
"#,
    )
    .unwrap();
    assert_eq!(parsed["frame_count"], 4);

    assert!(matches!(
        parse_response(9, br#"{"schema_version":1,"id":8,"ok":true,"result":{}}"#),
        Err(TracyProtocolError::ResponseId { .. })
    ));
    assert!(matches!(
        parse_response(9, b"{}\n{}\n"),
        Err(TracyProtocolError::MultipleResponses)
    ));
    assert!(matches!(
        parse_response(9, br#"{"schema_version":1,"id":9,"ok":false,"error":{"code":"bad_trace","message":"bad"}}"#),
        Err(TracyProtocolError::Helper { .. })
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
