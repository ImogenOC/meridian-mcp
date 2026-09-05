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
fn unchanged_content_hits_before_decode_and_preserved_metadata_changes_invalidate() {
    let root = temp_root();
    let path = root.join("cached.dmi");
    write_test_dmi(&path, [1, 2, 3, 255]);
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let mut cache = DmiCache::default();
    let limits = ServerLimits::default();
    let first = cache.load(&path, &limits).unwrap();
    let second = cache.load(&path, &limits).unwrap();
    assert_eq!(
        cache.decode_count(),
        1,
        "unchanged bytes were decoded twice"
    );
    assert!(std::sync::Arc::ptr_eq(&first.icon, &second.icon));
    write_test_dmi(&path, [3, 2, 1, 255]);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    let changed = cache.load(&path, &limits).unwrap();
    assert_eq!(cache.decode_count(), 2);
    assert_ne!(first.identity.sha256, changed.identity.sha256);
    assert!(changed.asset_generation > first.asset_generation);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn dimensions_are_rejected_before_pixel_decode() {
    let root = temp_root();
    let path = root.join("large.dmi");
    write_four_direction_dmi(&path, &[[[1, 2, 3, 255]; 3]; 4]);
    let limits = ServerLimits {
        max_dmi_decoded_pixels: 4,
        ..Default::default()
    };
    let mut cache = DmiCache::default();
    assert!(cache
        .load(&path, &limits)
        .unwrap_err()
        .to_string()
        .contains("max_dmi_decoded_pixels"));
    assert_eq!(
        cache.decode_count(),
        0,
        "oversized image entered pixel decoder"
    );
    std::fs::remove_dir_all(root).unwrap();
}

fn write_metadata_dmi(path: &std::path::Path, description: &str, compressed: bool) {
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    if compressed {
        encoder
            .add_ztxt_chunk("Description".into(), description.into())
            .unwrap();
    } else {
        encoder
            .add_text_chunk("Description".into(), description.into())
            .unwrap();
    }
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[1, 2, 3, 255])
        .unwrap();
}

#[test]
fn file_metadata_decoder_and_frame_limits_precede_pixels() {
    let root = temp_root();
    let path = root.join("bounded.dmi");
    let header = "# BEGIN DMI\nversion = 4.0\n\twidth = 1\n\theight = 1\n";
    let normal = format!("{header}state = \"technical\"\n\tdirs = 1\n\tframes = 1\n# END DMI\n");
    for case in [
        "file",
        "metadata",
        "decoder",
        "frames",
        "states",
        "malformed",
    ] {
        let mut limits = ServerLimits::default();
        let text = match case {
            "metadata" => {
                limits.max_dmi_metadata_bytes = 128;
                format!("{normal}{}", "x".repeat(4096))
            }
            "frames" => {
                limits.max_dmi_frames = 4;
                format!(
                    "{header}state = \"x\"\n\tdirs = 8\n\tframes = {}\n# END DMI\n",
                    usize::MAX
                )
            }
            "states" => {
                limits.max_dmi_states = 1;
                format!("{header}state = \"a\"\nstate = \"b\"\n# END DMI\n")
            }
            "malformed" => format!("{header}not a metadata property\n"),
            _ => normal.clone(),
        };
        write_metadata_dmi(&path, &text, case == "metadata");
        if case == "file" {
            limits.max_dmi_file_bytes = 16;
        }
        if case == "decoder" {
            limits.max_dmi_decoder_bytes = 0;
        }
        let mut cache = DmiCache::default();
        let error = cache.load(&path, &limits).unwrap_err();
        assert_eq!(cache.decode_count(), 0, "{case}: {error}");
        if case != "malformed" {
            assert!(
                error.to_string().contains("resource limit"),
                "{case}: {error}"
            );
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_requests_coalesce_one_identity() {
    let root = temp_root();
    let path = root.join("shared.dmi");
    write_test_dmi(&path, [9, 8, 7, 255]);
    let context = std::sync::Arc::new(ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], vec![]).unwrap(),
    ));
    let state = std::sync::Arc::new(ServerState::new());
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(9));
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let (context, state, barrier, path) = (
            context.clone(),
            state.clone(),
            barrier.clone(),
            path.clone(),
        );
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            payload(
                call_tool(&context, &state, "dm_dmi_info", json!({"dmi_path":path}))
                    .await
                    .unwrap(),
            )
        }));
    }
    barrier.wait().await;
    let mut outputs = Vec::new();
    for task in tasks {
        outputs.push(task.await.unwrap());
    }
    assert!(outputs.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(state.assets().await.decode_count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn scan_metadata_and_state_budgets_cover_zero_frame_assets() {
    let root = temp_root();
    let description = format!("# BEGIN DMI\nversion = 4.0\nwidth = 1\nheight = 1\nstate = \"{}\"\nframes = 0\n# END DMI\n", "long_name".repeat(100));
    for name in ["a", "b", "c"] {
        write_metadata_dmi(&root.join(format!("{name}.dmi")), &description, true);
    }
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], vec![]).unwrap(),
    );
    for (metadata_limit, state_limit, frame_limit, warm, reason) in [
        (
            description.len() * 2,
            100,
            100,
            false,
            "max_dmi_scan_metadata_bytes",
        ),
        (description.len() * 10, 2, 100, false, "max_dmi_scan_states"),
        (
            description.len() * 2,
            100,
            100,
            true,
            "max_dmi_scan_metadata_bytes",
        ),
        (description.len() * 10, 100, 2, false, "max_dmi_scan_frames"),
    ] {
        if frame_limit == 2 {
            for name in ["a", "b", "c"] {
                write_metadata_dmi(
                    &root.join(format!("{name}.dmi")),
                    &description.replace("frames = 0", "frames = 1"),
                    true,
                );
            }
        }
        let state = ServerState::with_limits(ServerLimits {
            max_dmi_scan_metadata_bytes: metadata_limit,
            max_dmi_scan_states: state_limit,
            max_dmi_scan_frames: frame_limit,
            max_dmi_cache_entries: if warm { 3 } else { 1 },
            ..Default::default()
        });
        if warm {
            for name in ["a", "b", "c"] {
                call_tool(
                    &context,
                    &state,
                    "dm_dmi_info",
                    json!({"dmi_path": root.join(format!("{name}.dmi"))}),
                )
                .await
                .unwrap();
            }
        }
        let result = payload(
            call_tool(
                &context,
                &state,
                "dm_find_dmi_duplicates",
                json!({"scope_path": root}),
            )
            .await
            .unwrap(),
        );
        assert!(
            result["truncation_reasons"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == reason),
            "{result:#}"
        );
        assert_eq!(
            state.assets().await.decode_count(),
            if warm { 3 } else { 2 }
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn scan_budget_counts_retained_arcs_after_cache_eviction() {
    let root = temp_root();
    for name in ["a", "b", "c"] {
        write_test_dmi(&root.join(format!("{name}.dmi")), [1, 2, 3, 255]);
    }
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], vec![]).unwrap(),
    );
    let limits = ServerLimits {
        max_dmi_cache_entries: 1,
        max_dmi_cache_bytes: 4,
        max_dmi_scan_decoded_bytes: 8,
        ..ServerLimits::default()
    };
    let state = ServerState::with_limits(limits);
    let result = payload(
        call_tool(
            &context,
            &state,
            "dm_find_dmi_duplicates",
            json!({"scope_path":root}),
        )
        .await
        .unwrap(),
    );
    assert!(
        result["truncation_reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason == "max_dmi_scan_decoded_bytes"),
        "{result:#}"
    );
    assert_eq!(
        state.assets().await.decode_count(),
        2,
        "scan allocated a third asset despite two live retained pixel Arcs"
    );
    eprintln!("scan live decoded peak: 8 bytes; cache ceiling: 4 bytes; decoder calls: 2");
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn audit_reuses_one_asset_across_references_and_phases() {
    let root = temp_root();
    for name in ["a", "b"] {
        write_test_dmi(&root.join(format!("{name}.dmi")), [1, 2, 3, 255]);
    }
    let dme = root.join("fixture.dme");
    std::fs::write(&dme, "/obj/a\n\ticon = 'a.dmi'\n\ticon_state = \"technical\"\n/obj/b\n\ticon = 'a.dmi'\n\ticon_state = \"technical\"\n/obj/c\n\ticon = 'b.dmi'\n\ticon_state = \"technical\"\n").unwrap();
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], vec![]).unwrap(),
    );
    let state = ServerState::with_limits(ServerLimits {
        max_dmi_cache_entries: 1,
        max_dmi_cache_bytes: 4,
        max_dmi_scan_decoded_bytes: 8,
        ..ServerLimits::default()
    });
    assert_ne!(
        call_tool(
            &context,
            &state,
            "dm_parse_environment",
            json!({"dme_path":dme})
        )
        .await
        .unwrap()
        .is_error,
        Some(true)
    );
    let result = call_tool(
        &context,
        &state,
        "dm_audit_icons",
        json!({"include_unused":true}),
    )
    .await
    .unwrap();
    assert_ne!(result.is_error, Some(true), "{result:?}");
    assert_eq!(
        state.assets().await.decode_count(),
        2,
        "audit re-decoded an asset retained by the scan"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn representative_cold_warm_decode_preserves_legacy_pixels_and_metadata() {
    let root = temp_root();
    let path = root.join("representative.dmi");
    let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), 256, 128);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.add_ztxt_chunk("Description".into(), "# BEGIN DMI\nversion = 4.0\n\twidth = 32\n\theight = 32\nstate = \"technical\"\n\tdirs = 8\n\tframes = 4\n\tdelay = 1,2,3,4\n# END DMI\n".into()).unwrap();
    let pixels: Vec<_> = (0..256 * 128)
        .flat_map(|index| {
            [
                (index % 251) as u8,
                ((index / 256) % 251) as u8,
                123,
                if index % 5 == 0 { 0 } else { 255 },
            ]
        })
        .collect();
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&pixels)
        .unwrap();
    let legacy = dmm_tools::dmi::IconFile::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
    let limits = ServerLimits::default();
    let mut cache = DmiCache::default();
    let started = std::time::Instant::now();
    let cold = cache.load(&path, &limits).unwrap();
    let cold_elapsed = started.elapsed();
    let started = std::time::Instant::now();
    let warm = cache.load(&path, &limits).unwrap();
    let warm_elapsed = started.elapsed();
    assert_eq!(cold.icon.image, legacy.image);
    assert_eq!(cold.icon.metadata, legacy.metadata);
    assert_eq!(
        serde_json::to_value(meridian_mcp::spaceman::dmi::profile_dmi(&cold, &limits).unwrap())
            .unwrap(),
        serde_json::to_value(meridian_mcp::spaceman::dmi::profile_dmi(&warm, &limits).unwrap())
            .unwrap()
    );
    assert_eq!(cache.decode_count(), 1);
    assert!(std::sync::Arc::ptr_eq(&cold.icon, &warm.icon));
    eprintln!("representative 256x128/32 sprites: cold={cold_elapsed:?}, warm={warm_elapsed:?}, decoder_calls=1, shared_live_decoded_bytes={}", cold.decoded_bytes());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn zero_frame_and_delay_metadata_keep_legacy_semantics() {
    let root = temp_root();
    let path = root.join("edge.dmi");
    for (frames, delay) in [(0, ""), (1, "\tdelay = 0\n")] {
        write_metadata_dmi(&path, &format!("# BEGIN DMI\nversion = 4.0\n\twidth = 1\n\theight = 1\nstate = \"edge\"\n\tframes = {frames}\n{delay}# END DMI\n"), true);
        let legacy = dmm_tools::dmi::IconFile::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
        let decoded = DmiCache::default()
            .load(&path, &ServerLimits::default())
            .unwrap();
        assert_eq!(decoded.icon.metadata, legacy.metadata);
        assert_eq!(decoded.icon.image, legacy.image);
    }
    std::fs::remove_dir_all(root).unwrap();
}

fn raw_png(
    width: u32,
    height: u32,
    interlaced: bool,
    raw: &[u8],
    extra: Option<(&[u8; 4], &[u8])>,
) -> Vec<u8> {
    fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(kind);
        output.extend_from_slice(data);
        let mut crc = u32::MAX;
        for &byte in kind.iter().chain(data) {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320_u32 & 0_u32.wrapping_sub(crc & 1));
            }
        }
        output.extend_from_slice(&(!crc).to_be_bytes());
    }
    let mut output = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = width.to_be_bytes().to_vec();
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, u8::from(interlaced)]);
    chunk(&mut output, b"IHDR", &header);
    chunk(&mut output, b"IDAT", &fdeflate::compress_to_vec(raw));
    if let Some((kind, data)) = extra {
        chunk(&mut output, kind, data);
    }
    chunk(&mut output, b"IEND", &[]);
    output
}

#[test]
fn oversized_idat_and_unhandled_animation_are_rejected_before_pixels() {
    let root = temp_root();
    let path = root.join("expansion.dmi");
    for data in [
        raw_png(1, 1, false, &[0; 4096], None),
        raw_png(1, 1, false, &[0, 1, 2, 3, 255], Some((b"fdAT", &[0; 8]))),
    ] {
        std::fs::write(&path, data).unwrap();
        let mut cache = DmiCache::default();
        assert!(cache.load(&path, &ServerLimits::default()).is_err());
        assert_eq!(cache.decode_count(), 0);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn interlaced_pixels_keep_legacy_semantics() {
    let root = temp_root();
    let path = root.join("interlaced.dmi");
    let bytes = raw_png(
        2,
        2,
        true,
        &[
            0, 255, 0, 0, 255, 0, 0, 255, 0, 255, 0, 0, 0, 255, 255, 10, 20, 30, 0,
        ],
        None,
    );
    std::fs::write(&path, &bytes).unwrap();
    let legacy = dmm_tools::dmi::IconFile::from_bytes(&bytes).unwrap();
    let decoded = DmiCache::default()
        .load(&path, &ServerLimits::default())
        .unwrap();
    assert_eq!(decoded.icon.image, legacy.image);
    assert_eq!(decoded.icon.metadata, legacy.metadata);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn png_color_modes_keep_legacy_pixels() {
    let root = temp_root();
    let path = root.join("colors.dmi");
    for (color, depth, data) in [
        (
            png::ColorType::Grayscale,
            png::BitDepth::One,
            vec![0b10000000],
        ),
        (
            png::ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            vec![12, 255, 99, 128],
        ),
        (
            png::ColorType::Rgb,
            png::BitDepth::Eight,
            vec![12, 34, 56, 78, 90, 123],
        ),
        (
            png::ColorType::Rgba,
            png::BitDepth::Sixteen,
            vec![
                0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0xff, 0, 0, 0, 0, 0, 0, 0,
            ],
        ),
        (
            png::ColorType::Indexed,
            png::BitDepth::Two,
            vec![0b00010000],
        ),
    ] {
        let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), 2, 1);
        encoder.set_color(color);
        encoder.set_depth(depth);
        if color == png::ColorType::Indexed {
            encoder.set_palette(vec![255, 0, 0, 0, 255, 0]);
            encoder.set_trns(vec![255, 128]);
        }
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&data)
            .unwrap();
        let legacy = dmm_tools::dmi::IconFile::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
        let decoded = DmiCache::default()
            .load(&path, &ServerLimits::default())
            .unwrap();
        assert_eq!(decoded.icon.image, legacy.image, "{color:?} {depth:?}");
    }
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

#[tokio::test]
async fn derived_icon_reads_reject_escape_for_audit_and_render() {
    let root = temp_root();
    let allowed = root.join("allowed");
    std::fs::create_dir(&allowed).unwrap();
    write_test_dmi(&root.join("outside.dmi"), [255, 0, 0, 255]);
    let dme = allowed.join("fixture.dme");
    std::fs::write(
        &dme,
        "/turf/audit\n\ticon = '../outside.dmi'\n\ticon_state = \"technical\"\n/area\n",
    )
    .unwrap();
    let map = allowed.join("fixture.dmm");
    std::fs::write(
        &map,
        "\"a\" = (/turf/audit,/area)\n\n(1,1,1) = {\"\na\n\"}\n",
    )
    .unwrap();
    let context = ToolExecutionContext::new(
        CapabilityMode::Development,
        PathPolicy::new(vec![allowed.clone()], vec![]).unwrap(),
    );
    let state = ServerState::new();
    let parsed = call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({"dme_path":dme}),
    )
    .await
    .unwrap();
    assert_ne!(parsed.is_error, Some(true));
    let audit = call_tool(&context, &state, "dm_audit_icons", json!({})).await;
    let audit_denied = audit.is_err() || audit.unwrap().is_error == Some(true);
    let output = allowed.join("render.png");
    let render = call_tool(
        &context,
        &state,
        "dm_render_map",
        json!({"dmm_path":map,"output_path":output}),
    )
    .await;
    let render_denied = render.is_err() || render.unwrap().is_error == Some(true);
    assert!(
        audit_denied && render_denied,
        "audit_denied={audit_denied}, render_denied={render_denied}"
    );
    assert!(!output.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn authorized_sibling_icon_supports_audit_and_render() {
    let root = temp_root();
    let allowed = root.join("allowed");
    let sibling = root.join("sibling");
    std::fs::create_dir(&allowed).unwrap();
    std::fs::create_dir(&sibling).unwrap();
    write_test_dmi(&sibling.join("technical.dmi"), [255, 0, 0, 255]);
    let dme = allowed.join("fixture.dme");
    std::fs::write(
        &dme,
        "/turf/audit\n\ticon = '../sibling/technical.dmi'\n\ticon_state = \"technical\"\n/area\n",
    )
    .unwrap();
    let map = allowed.join("fixture.dmm");
    std::fs::write(
        &map,
        "\"a\" = (/turf/audit,/area)\n\n(1,1,1) = {\"\na\n\"}\n",
    )
    .unwrap();
    let context = ToolExecutionContext::new(
        CapabilityMode::Development,
        PathPolicy::new(vec![allowed.clone(), sibling], vec![]).unwrap(),
    );
    let state = ServerState::new();
    let parsed = call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({"dme_path":dme}),
    )
    .await
    .unwrap();
    assert_ne!(parsed.is_error, Some(true));
    let audit = call_tool(&context, &state, "dm_audit_icons", json!({}))
        .await
        .unwrap();
    assert_ne!(audit.is_error, Some(true));
    let output = allowed.join("render.png");
    let render = call_tool(
        &context,
        &state,
        "dm_render_map",
        json!({"dmm_path":map,"output_path":output}),
    )
    .await
    .unwrap();
    assert_ne!(render.is_error, Some(true));
    assert!(payload(render)["non_transparent_pixels"].as_u64().unwrap() > 0);
    assert!(output.exists());
    std::fs::remove_dir_all(root).unwrap();
}
