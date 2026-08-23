use anyhow::{anyhow, Result};
use dmm_tools::{dmm, minimap, render_passes, IconCache};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::RwLock;
use tracing::info;

use crate::mcp::ToolResult;
use crate::state::ServerState;

/// Render a map to PNG.
pub async fn render_map(state: &mut ServerState, args: Value) -> Result<ToolResult> {
    let dmm_path = args
        .get("dmm_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing dmm_path argument"))?;
    let z_level = args.get("z_level").and_then(Value::as_u64).unwrap_or(1) as usize;
    let output_path = args
        .get("output_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(dmm_path).with_extension("png"));
    let path = PathBuf::from(dmm_path);
    if !path.exists() {
        return Ok(ToolResult::error(format!("File not found: {dmm_path}")));
    }

    let objtree = state
        .objtree
        .as_ref()
        .ok_or_else(|| anyhow!("No environment loaded. Call dm_parse_environment first."))?;
    let context = state
        .context
        .as_ref()
        .ok_or_else(|| anyhow!("No context available"))?;
    let environment_root = state
        .environment_path
        .as_ref()
        .and_then(|environment| environment.parent())
        .ok_or_else(|| anyhow!("Parsed environment has no parent directory"))?;

    info!("Rendering map: {dmm_path} z-level {z_level} to {output_path:?}");
    let map = dmm::Map::from_file(&path)?;
    let (dim_x, dim_y, dim_z) = map.dim_xyz();
    if z_level == 0 || z_level > dim_z {
        return Ok(ToolResult::error(format!(
            "Z-level {z_level} is outside the map range 1..={dim_z}"
        )));
    }

    let mut icon_cache = IconCache::default();
    icon_cache.set_icons_root(environment_root);
    let render_passes = render_passes::configure(&context.config().map_renderer, "", "");
    let errors: RwLock<_> = Default::default();
    let bump = Default::default();
    let image = minimap::generate(
        minimap::Context {
            objtree,
            map: &map,
            level: map.z_level(z_level - 1),
            min: (0, 0),
            max: (dim_x - 1, dim_y - 1),
            render_passes: &render_passes,
            errors: &errors,
            bump: &bump,
            print_errors: false,
        },
        &icon_cache,
    )
    .map_err(|()| anyhow!("SpacemanDMM could not render the requested map"))?;
    let non_transparent_pixels = image.data.iter().filter(|pixel| pixel.a > 0).count();

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.to_file(&output_path)?;

    Ok(ToolResult::text(serde_json::to_string_pretty(&json!({
        "success": true,
        "dmm_path": dmm_path,
        "z_level": z_level,
        "output_path": output_path.display().to_string(),
        "dimensions_pixels": {"width": dim_x * 32, "height": dim_y * 32},
        "non_transparent_pixels": non_transparent_pixels,
        "warning": if non_transparent_pixels == 0 {
            Some("The renderer produced a fully transparent image. This can be expected when render passes hide every atom on the selected z-level.")
        } else {
            None
        }
    }))?))
}

/// Get map dimensions and atom instance statistics.
pub async fn map_info(args: Value) -> Result<ToolResult> {
    let dmm_path = args
        .get("dmm_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing dmm_path argument"))?;
    let path = PathBuf::from(dmm_path);
    if !path.exists() {
        return Ok(ToolResult::error(format!("File not found: {dmm_path}")));
    }

    info!("Getting map info: {dmm_path}");
    let map = dmm::Map::from_file(&path)?;
    let (dim_x, dim_y, dim_z) = map.dim_xyz();
    let mut type_counts: HashMap<String, usize> = HashMap::new();
    let mut area_counts: HashMap<String, usize> = HashMap::new();

    for key in map.grid.iter() {
        let prefabs = map
            .dictionary
            .get(key)
            .ok_or_else(|| anyhow!("Map grid references a missing dictionary key"))?;
        for prefab in prefabs {
            if let Some(base_type) = prefab
                .path
                .split('/')
                .nth(1)
                .filter(|segment| !segment.is_empty())
                .map(|segment| format!("/{segment}"))
            {
                *type_counts.entry(base_type).or_default() += 1;
            }
            if prefab.path == "/area" || prefab.path.starts_with("/area/") {
                *area_counts.entry(prefab.path.clone()).or_default() += 1;
            }
        }
    }

    let mut sorted_types: Vec<_> = type_counts.into_iter().collect();
    sorted_types.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let mut sorted_areas: Vec<_> = area_counts.into_iter().collect();
    sorted_areas.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let content = std::fs::read_to_string(&path)?;

    Ok(ToolResult::text(serde_json::to_string_pretty(&json!({
        "file": dmm_path,
        "format": if content.contains("//MAP CONVERTED BY dmm2tgm.py") { "TGM" } else { "DMM" },
        "dimensions": {"x": dim_x, "y": dim_y, "z": dim_z},
        "unique_tiles": map.dictionary.len(),
        "file_size_bytes": std::fs::metadata(&path)?.len(),
        "top_types": sorted_types.into_iter().take(20).collect::<Vec<_>>(),
        "top_areas": sorted_areas.into_iter().take(20).collect::<Vec<_>>()
    }))?))
}

/// Find exact type and subtype instances on a map.
pub async fn find_on_map(args: Value) -> Result<ToolResult> {
    let dmm_path = args
        .get("dmm_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing dmm_path argument"))?;
    let type_path = args
        .get("type_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing type_path argument"))?;
    let path = PathBuf::from(dmm_path);
    if !path.exists() {
        return Ok(ToolResult::error(format!("File not found: {dmm_path}")));
    }

    info!("Finding {type_path} on map {dmm_path}");
    let map = dmm::Map::from_file(&path)?;
    let descendant_prefix = format!("{}/", type_path.trim_end_matches('/'));
    let mut matching_tiles: BTreeMap<_, Vec<&str>> = BTreeMap::new();
    for (key, prefabs) in &map.dictionary {
        let matches: Vec<_> = prefabs
            .iter()
            .filter(|prefab| {
                prefab.path == type_path || prefab.path.starts_with(&descendant_prefix)
            })
            .map(|prefab| prefab.path.as_str())
            .collect();
        if !matches.is_empty() {
            matching_tiles.insert(*key, matches);
        }
    }

    let keys: Vec<_> = matching_tiles
        .keys()
        .map(|key| map.format_key(*key).to_string())
        .collect();
    let mut coordinates = Vec::new();
    for (z, level) in map.iter_levels() {
        for (coordinate, key) in level.iter_top_down() {
            if let Some(matches) = matching_tiles.get(&key) {
                for matched_type in matches {
                    coordinates.push(json!({
                        "x": coordinate.x,
                        "y": coordinate.y,
                        "z": z,
                        "tile_key": map.format_key(key).to_string(),
                        "matched_type": matched_type
                    }));
                }
            }
        }
    }

    Ok(ToolResult::text(serde_json::to_string_pretty(&json!({
        "type_path": type_path,
        "dmm_path": dmm_path,
        "count": coordinates.len(),
        "matching_tile_keys": keys.len(),
        "keys": keys,
        "coordinates": coordinates
    }))?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::ToolContent;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn fixture_directory() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "meridian-mcp-map-{}-{unique}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn result_json(result: &ToolResult) -> Value {
        let ToolContent::Text { text } = &result.content[0];
        serde_json::from_str(text).expect("tool result should be JSON")
    }

    fn write_map(directory: &Path) -> PathBuf {
        let path = directory.join("fixture.dmm");
        std::fs::write(
            &path,
            r#"//MAP CONVERTED BY dmm2tgm.py THIS HEADER COMMENT MUST NOT COUNT AS A TYPE
"aa" = (/obj/item/test,/turf/open/space,/area/space)
"ab" = (/turf/open/space,/area/space)

(1,1,1) = {"
aa
ab
"}
(2,1,1) = {"
ab
aa
"}
"#,
        )
        .unwrap();
        path
    }

    #[tokio::test]
    async fn map_info_uses_parsed_grid_dimensions_and_instance_counts() {
        let directory = fixture_directory();
        let path = write_map(&directory);
        let result = map_info(json!({"dmm_path": path})).await.unwrap();
        let payload = result_json(&result);

        assert_eq!(payload["dimensions"], json!({"x": 2, "y": 2, "z": 1}));
        assert_eq!(payload["unique_tiles"], 2);
        assert_eq!(payload["top_types"][0], json!(["/area", 4]));
        assert!(payload["top_types"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry[0] != "/"));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn find_on_map_returns_exact_byond_coordinates() {
        let directory = fixture_directory();
        let path = write_map(&directory);
        let result = find_on_map(json!({
            "dmm_path": path,
            "type_path": "/obj/item/test"
        }))
        .await
        .unwrap();
        let payload = result_json(&result);

        assert_eq!(payload["count"], 2);
        assert_eq!(
            payload["coordinates"],
            json!([
                {"x": 1, "y": 2, "z": 1, "tile_key": "aa", "matched_type": "/obj/item/test"},
                {"x": 2, "y": 1, "z": 1, "tile_key": "aa", "matched_type": "/obj/item/test"}
            ])
        );

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn render_map_writes_a_real_png() {
        let directory = fixture_directory();
        let dme_path = directory.join("fixture.dme");
        std::fs::write(&dme_path, "// map render fixture\n").unwrap();
        let map_path = directory.join("render.dmm");
        std::fs::write(
            &map_path,
            r#""a" = (/turf,/area)

(1,1,1) = {"
a
"}
"#,
        )
        .unwrap();
        let output_path = directory.join("render.png");
        let mut state = ServerState::new();
        let parsed =
            crate::tools::parse::parse_environment(&mut state, json!({"dme_path": dme_path}))
                .await
                .unwrap();
        assert_eq!(parsed.is_error, None, "parse result: {parsed:?}");

        let result = render_map(
            &mut state,
            json!({
                "dmm_path": map_path,
                "output_path": output_path,
                "z_level": 1
            }),
        )
        .await
        .unwrap();
        assert_eq!(result.is_error, None, "render result: {result:?}");
        let payload = result_json(&result);
        assert_eq!(payload["non_transparent_pixels"], 0);
        assert!(payload["warning"]
            .as_str()
            .unwrap()
            .contains("fully transparent"));
        let bytes = std::fs::read(&output_path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");

        std::fs::remove_dir_all(directory).unwrap();
    }
}
