use meridian_mcp::limits::ServerLimits;
use meridian_mcp::result::ToolContent;
use meridian_mcp::spaceman::dmi::{compare_states, prepare_dmi, DmiCache, MatchKind};
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::{CapabilityMode, PathPolicy};
use serde_json::{json, Value};
use sha2::Digest;
use std::io::BufWriter;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "meridian-mcp-dmi-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn write_test_dmi(path: &std::path::Path, pixel: [u8; 4]) {
    let output = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(BufWriter::new(output), 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
		.add_text_chunk(
			"Description".to_owned(),
			"# BEGIN DMI\nversion = 4.0\n\twidth = 1\n\theight = 1\nstate = \"technical\"\n\tdirs = 1\n\tframes = 1\n# END DMI\n".to_owned(),
		)
		.unwrap();
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&pixel).unwrap();
}

fn write_four_direction_dmi(path: &std::path::Path, frames: &[[[u8; 4]; 3]; 4]) {
    let output = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(BufWriter::new(output), 12, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
		.add_text_chunk(
			"Description".to_owned(),
			"# BEGIN DMI\nversion = 4.0\n\twidth = 3\n\theight = 1\nstate = \"technical\"\n\tdirs = 4\n\tframes = 1\n# END DMI\n".to_owned(),
		)
		.unwrap();
    let pixels = frames
        .iter()
        .flatten()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&pixels).unwrap();
}

fn mirror(frame: [[u8; 4]; 3]) -> [[u8; 4]; 3] {
    [frame[2], frame[1], frame[0]]
}

fn payload(result: meridian_mcp::result::ToolResult) -> Value {
    let ToolContent::Text { text } = &result.content[0];
    serde_json::from_str(text).unwrap()
}

#[test]
fn prepared_assets_revalidate_same_path_content_before_cache_locking() {
    let root = temp_root();
    let path = root.join("technical.dmi");
    let limits = ServerLimits::default();
    let mut cache = DmiCache::default();

    write_test_dmi(&path, [255, 0, 0, 255]);
    let first = cache.install(prepare_dmi(&path, &limits).unwrap(), &limits);
    write_test_dmi(&path, [0, 0, 255, 255]);
    let second = cache.install(prepare_dmi(&path, &limits).unwrap(), &limits);

    assert_ne!(first.identity.sha256, second.identity.sha256);
    assert!(second.asset_generation > first.asset_generation);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn whole_state_comparison_requires_one_transform_and_direction_mapping() {
    let root = temp_root();
    let left_path = root.join("left.dmi");
    let consistent_path = root.join("consistent.dmi");
    let inconsistent_path = root.join("inconsistent.dmi");
    let transparent = [0, 0, 0, 0];
    let red = [255, 0, 0, 255];
    let green = [0, 255, 0, 255];
    let blue = [0, 0, 255, 255];
    let yellow = [255, 255, 0, 255];
    let left_frames = [
        [red, transparent, transparent],
        [green, green, red],
        [blue, transparent, blue],
        [yellow, yellow, yellow],
    ];
    let consistent_frames = [
        mirror(left_frames[0]),
        mirror(left_frames[1]),
        mirror(left_frames[3]),
        mirror(left_frames[2]),
    ];
    let mut inconsistent_frames = consistent_frames;
    inconsistent_frames[1] = left_frames[1];
    write_four_direction_dmi(&left_path, &left_frames);
    write_four_direction_dmi(&consistent_path, &consistent_frames);
    write_four_direction_dmi(&inconsistent_path, &inconsistent_frames);

    let limits = ServerLimits::default();
    let mut cache = DmiCache::default();
    let left = cache.install(prepare_dmi(&left_path, &limits).unwrap(), &limits);
    let consistent = cache.install(prepare_dmi(&consistent_path, &limits).unwrap(), &limits);
    let inconsistent = cache.install(prepare_dmi(&inconsistent_path, &limits).unwrap(), &limits);

    let global = compare_states(&left, "technical", 0, &consistent, "technical", 0, 0.985).unwrap();
    let mixed =
        compare_states(&left, "technical", 0, &inconsistent, "technical", 0, 0.985).unwrap();
    assert_eq!(global.image_match, MatchKind::Transformed, "{global:#?}");
    assert_eq!(mixed.image_match, MatchKind::Different, "{mixed:#?}");
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn duplicate_scan_buckets_unrelated_states_before_detailed_comparison() {
    let root = temp_root();
    write_test_dmi(&root.join("alpha.dmi"), [255, 0, 0, 255]);
    write_test_dmi(&root.join("beta.dmi"), [255, 0, 0, 255]);
    write_test_dmi(&root.join("unrelated.dmi"), [0, 0, 0, 0]);
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let result = call_tool(
        &context,
        &ServerState::new(),
        "dm_find_dmi_duplicates",
        json!({"scope_path": root}),
    )
    .await
    .unwrap();
    let body = payload(result);

    assert_eq!(body["cluster_count"], 1, "{body:#}");
    assert_eq!(body["clusters"][0]["members"].as_array().unwrap().len(), 2);
    assert!(
        body["candidate_comparisons"].as_u64().unwrap() < 3,
        "{body:#}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn icon_audit_reports_missing_static_states_and_dynamic_uncertainty() {
    let root = temp_root();
    write_test_dmi(&root.join("technical.dmi"), [255, 0, 0, 255]);
    std::fs::write(root.join("fixture.dme"), "#include \"fixture.dm\"\n").unwrap();
    std::fs::write(
        root.join("fixture.dm"),
        r#"/obj/technical_valid
	icon = 'technical.dmi'
	icon_state = "technical"

/obj/technical_missing_state
	icon = 'technical.dmi'
	icon_state = "missing"

/obj/technical_dynamic_state
	icon = 'technical.dmi'
	icon_state = pick("technical", "missing")
"#,
    )
    .unwrap();
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let state = ServerState::new();
    let parsed = call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({"dme_path": root.join("fixture.dme")}),
    )
    .await
    .unwrap();
    let parsed = payload(parsed);
    assert_eq!(parsed["success"], true, "{parsed:#}");
    let result = call_tool(
        &context,
        &state,
        "dm_audit_icons",
        json!({"scope_path": root, "include_unused": true}),
    )
    .await
    .unwrap();
    let body = payload(result);

    assert_eq!(body["complete"], false, "{body:#}");
    assert!(
        body["missing_states"]
            .as_array()
            .unwrap()
            .iter()
            .any(|missing| {
                missing["type_path"] == "/obj/technical_missing_state"
                    && missing["state"] == "missing"
            }),
        "{body:#}"
    );
    assert!(
        body["dynamic_references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reference| { reference["type_path"] == "/obj/technical_dynamic_state" }),
        "{body:#}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn extraction_supports_exact_frames_and_contact_sheets_without_source_changes() {
    let root = temp_root();
    let source = root.join("technical.dmi");
    let frame_output = root.join("frame.png");
    let sheet_output = root.join("sheet.png");
    write_test_dmi(&source, [255, 0, 0, 255]);
    let source_hash = sha2::Sha256::digest(std::fs::read(&source).unwrap());
    let context = ToolExecutionContext::new(
        CapabilityMode::Development,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let state = ServerState::new();
    for (kind, output, extra) in [
        (
            "frame",
            &frame_output,
            json!({"direction": "south", "frame": 0}),
        ),
        ("contact_sheet", &sheet_output, json!({})),
    ] {
        let mut args = json!({
            "dmi_path": source,
            "state": "technical",
            "kind": kind,
            "output_path": output,
            "overwrite": false,
        });
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let result = call_tool(&context, &state, "dm_extract_dmi", args)
            .await
            .unwrap();
        let is_error = result.is_error;
        let body = payload(result);
        assert_eq!(is_error, None, "{body}");
        assert_eq!(&std::fs::read(output).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
    }
    assert_eq!(
        source_hash.as_slice(),
        sha2::Sha256::digest(std::fs::read(&source).unwrap()).as_slice()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn dmi_profile_reports_pixels_identity_and_hotspot_limitation() {
    let root = temp_root();
    let source = root.join("technical.dmi");
    write_test_dmi(&source, [10, 20, 30, 128]);
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let result = call_tool(
        &context,
        &ServerState::new(),
        "dm_dmi_info",
        json!({"dmi_path": source}),
    )
    .await
    .unwrap();
    let body = payload(result);
    let profile = &body["profile"];

    assert_eq!(profile["sheet_width"], 1);
    assert_eq!(
        profile["states"][0]["frames"][0]["pixel_counts"]["translucent"],
        1
    );
    assert_eq!(
        profile["states"][0]["frames"][0]["alpha_bounds"]["max_x"],
        0
    );
    assert!(profile["identity"]["sha256"].as_str().unwrap().len() == 64);
    assert!(
        profile["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| { warning["code"] == "hotspot_unsupported" }),
        "{body:#}"
    );
    std::fs::remove_dir_all(root).unwrap();
}
