use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

use dreammaker::Context;

use crate::mcp::ToolResult;
use crate::state::ServerState;

pub(crate) const MAX_SOURCE_LINES: usize = 200;

/// Read a bounded source excerpt beginning at a DreamMaker declaration line.
pub(crate) fn extract_source(file_path: &str, start_line: u32) -> Option<String> {
    if start_line == 0 {
        return None;
    }

    let source = std::fs::read_to_string(file_path).ok()?;
    let start_index = start_line.checked_sub(1)? as usize;
    let lines: Vec<&str> = source.lines().collect();
    if start_index >= lines.len() {
        return None;
    }

    let mut excerpt = Vec::new();
    let mut in_block_comment = false;
    for line in lines.iter().skip(start_index).take(MAX_SOURCE_LINES) {
        let trimmed = line.trim_start();
        let is_column_zero = line
            .chars()
            .next()
            .is_some_and(|character| !character.is_whitespace());
        let is_comment = in_block_comment
            || trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*');

        if !excerpt.is_empty() && is_column_zero && !is_comment {
            break;
        }

        excerpt.push(*line);

        let mut remainder = *line;
        while let Some(start) = remainder.find("/*") {
            let after_start = &remainder[start + 2..];
            if let Some(end) = after_start.find("*/") {
                remainder = &after_start[end + 2..];
            } else {
                in_block_comment = true;
                break;
            }
        }
        if in_block_comment && remainder.contains("*/") {
            in_block_comment = false;
        }
    }

    Some(excerpt.join("\n"))
}

/// Parse a DreamMaker environment
pub async fn parse_environment(state: &mut ServerState, args: Value) -> Result<ToolResult> {
    let dme_path = args
        .get("dme_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing dme_path argument"))?;

    let path = PathBuf::from(dme_path);
    if !path.exists() {
        return Ok(ToolResult::error(format!("File not found: {}", dme_path)));
    }

    info!("Parsing environment: {}", dme_path);

    let context = Context::default();
    match context.parse_environment(&path) {
        Ok(objtree) => {
            let type_count = objtree.iter_types().count();

            state.environment_path = Some(path);
            state.objtree = Some(Arc::new(objtree));
            state.context = Some(context);

            Ok(ToolResult::text(format!(
                "Successfully parsed environment: {}\nTotal types: {}",
                dme_path, type_count
            )))
        }
        Err(e) => Ok(ToolResult::error(format!(
            "Failed to parse environment: {}",
            e
        ))),
    }
}

/// Helper to get file path string from a location
fn get_file_path(context: &Context, file_id: dreammaker::FileId) -> String {
    let path_ref = context.file_path(file_id);
    (*path_ref).display().to_string()
}

fn resolve_source_path(state: &ServerState, file_path: &str) -> PathBuf {
    let path = PathBuf::from(file_path);
    if path.is_absolute() || path.exists() {
        return path;
    }

    state
        .environment_path
        .as_ref()
        .and_then(|environment| environment.parent())
        .map(|root| root.join(&path))
        .unwrap_or(path)
}

/// Get type information
pub async fn get_type(state: &mut ServerState, args: Value) -> Result<ToolResult> {
    let objtree = state
        .objtree
        .as_ref()
        .ok_or_else(|| anyhow!("No environment loaded. Call dm_parse_environment first."))?;
    let context = state
        .context
        .as_ref()
        .ok_or_else(|| anyhow!("No context available"))?;

    let type_path = args
        .get("type_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing type_path argument"))?;

    match objtree.find(type_path) {
        Some(ty) => {
            // Collect variables
            let vars: Vec<Value> = ty
                .vars
                .iter()
                .map(|(name, var)| {
                    json!({
                        "name": name.to_string(),
                        "has_value": var.value.expression.is_some(),
                        "constant": var.value.constant.as_ref().map(|c| format!("{:?}", c)),
                        "declared_here": var.declaration.is_some()
                    })
                })
                .collect();

            // Collect procs
            let procs: Vec<Value> = ty
                .procs
                .iter()
                .map(|(name, proc)| {
                    let param_count = proc.value.first().map(|v| v.parameters.len()).unwrap_or(0);
                    json!({
                        "name": name.to_string(),
                        "parameter_count": param_count,
                        "override_count": proc.value.len(),
                        "declared_here": proc.declaration.is_some()
                    })
                })
                .collect();

            // Get parent
            let parent = ty.parent_type().map(|p| p.path.to_string());

            let file_path = get_file_path(context, ty.location.file);

            let result = json!({
                "path": ty.path,
                "parent": parent,
                "vars": vars,
                "procs": procs,
                "location": format!("{}:{}:{}",
                    file_path,
                    ty.location.line,
                    ty.location.column
                )
            });

            Ok(ToolResult::text(serde_json::to_string_pretty(&result)?))
        }
        None => Ok(ToolResult::error(format!("Type not found: {}", type_path))),
    }
}

/// Get proc information
pub async fn get_proc(state: &mut ServerState, args: Value) -> Result<ToolResult> {
    let objtree = state
        .objtree
        .as_ref()
        .ok_or_else(|| anyhow!("No environment loaded. Call dm_parse_environment first."))?;
    let context = state
        .context
        .as_ref()
        .ok_or_else(|| anyhow!("No context available"))?;

    let type_path = args
        .get("type_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing type_path argument"))?;

    let proc_name = args
        .get("proc_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing proc_name argument"))?;

    match objtree.find(type_path) {
        Some(ty) => {
            match ty.procs.get(proc_name) {
                Some(proc) => {
                    let values: Vec<Value> = proc.value.iter()
                        .map(|v| {
                            let params: Vec<Value> = v.parameters.iter()
                                .map(|p| {
                                    json!({
                                        "name": p.name.to_string(),
                                        "has_default": p.default.is_some(),
                                    })
                                })
                                .collect();

                            let file_path = get_file_path(context, v.location.file);

                            json!({
                                "parameters": params,
                                "location": format!("{}:{}:{}",
                                    file_path,
                                    v.location.line,
                                    v.location.column
                                ),
                                "has_body": v.code.is_some(),
                                "source": extract_source(
                                    resolve_source_path(state, &file_path).to_string_lossy().as_ref(),
                                    v.location.line as u32,
                                )
                            })
                        })
                        .collect();

                    let result = json!({
                        "name": proc_name,
                        "type_path": type_path,
                        "declared": proc.declaration.is_some(),
                        "overrides": values
                    });

                    Ok(ToolResult::text(serde_json::to_string_pretty(&result)?))
                }
                None => Ok(ToolResult::error(format!(
                    "Proc not found: {}/{}",
                    type_path, proc_name
                ))),
            }
        }
        None => Ok(ToolResult::error(format!("Type not found: {}", type_path))),
    }
}

/// Get variable information
pub async fn get_var(state: &mut ServerState, args: Value) -> Result<ToolResult> {
    let objtree = state
        .objtree
        .as_ref()
        .ok_or_else(|| anyhow!("No environment loaded. Call dm_parse_environment first."))?;
    let context = state
        .context
        .as_ref()
        .ok_or_else(|| anyhow!("No context available"))?;

    let type_path = args
        .get("type_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing type_path argument"))?;

    let var_name = args
        .get("var_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing var_name argument"))?;

    match objtree.find(type_path) {
        Some(ty) => match ty.vars.get(var_name) {
            Some(var) => {
                let file_path = get_file_path(context, var.value.location.file);

                let result = json!({
                    "name": var_name,
                    "type_path": type_path,
                    "declared": var.declaration.is_some(),
                    "constant": var.value.constant.as_ref().map(|c| format!("{:?}", c)),
                    "has_expression": var.value.expression.is_some(),
                    "location": format!("{}:{}:{}",
                        file_path,
                        var.value.location.line,
                        var.value.location.column
                    )
                });

                Ok(ToolResult::text(serde_json::to_string_pretty(&result)?))
            }
            None => Ok(ToolResult::error(format!(
                "Variable not found: {}/{}",
                type_path, var_name
            ))),
        },
        None => Ok(ToolResult::error(format!("Type not found: {}", type_path))),
    }
}

/// List types in the object tree
pub async fn list_types(state: &mut ServerState, args: Value) -> Result<ToolResult> {
    let objtree = state
        .objtree
        .as_ref()
        .ok_or_else(|| anyhow!("No environment loaded. Call dm_parse_environment first."))?;

    let prefix = args.get("prefix").and_then(|v| v.as_str()).unwrap_or("");

    let max_depth = args
        .get("max_depth")
        .and_then(|v| v.as_u64())
        .map(|d| d as usize);

    let types: Vec<Value> = objtree
        .iter_types()
        .filter(|ty| ty.path.starts_with(prefix))
        .filter(|ty| {
            if let Some(max) = max_depth {
                ty.path.matches('/').count() <= max
            } else {
                true
            }
        })
        .map(|ty| {
            json!({
                "path": ty.path.to_string(),
                "var_count": ty.vars.len(),
                "proc_count": ty.procs.len()
            })
        })
        .collect();

    let result = json!({
        "count": types.len(),
        "types": types
    });

    Ok(ToolResult::text(serde_json::to_string_pretty(&result)?))
}

/// Search for symbols
pub async fn search_symbols(state: &mut ServerState, args: Value) -> Result<ToolResult> {
    let objtree = state
        .objtree
        .as_ref()
        .ok_or_else(|| anyhow!("No environment loaded. Call dm_parse_environment first."))?;
    let context = state
        .context
        .as_ref()
        .ok_or_else(|| anyhow!("No context available"))?;

    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing query argument"))?
        .to_lowercase();

    let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("all");

    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let mut results: Vec<Value> = Vec::new();

    for ty in objtree.iter_types() {
        if results.len() >= limit {
            break;
        }

        // Search types
        if (kind == "all" || kind == "type") && ty.path.to_lowercase().contains(&query) {
            let file_path = get_file_path(context, ty.location.file);
            results.push(json!({
                "kind": "type",
                "path": ty.path.to_string(),
                "location": format!("{}:{}",
                    file_path,
                    ty.location.line
                )
            }));
        }

        // Search procs
        if kind == "all" || kind == "proc" {
            for (name, proc) in ty.procs.iter() {
                if results.len() >= limit {
                    break;
                }
                if name.to_lowercase().contains(&query) {
                    if let Some(first) = proc.value.first() {
                        let file_path = get_file_path(context, first.location.file);
                        results.push(json!({
                            "kind": "proc",
                            "name": name.to_string(),
                            "type_path": ty.path.to_string(),
                            "location": format!("{}:{}",
                                file_path,
                                first.location.line
                            )
                        }));
                    }
                }
            }
        }

        // Search vars
        if kind == "all" || kind == "var" {
            for (name, var) in ty.vars.iter() {
                if results.len() >= limit {
                    break;
                }
                if name.to_lowercase().contains(&query) {
                    let file_path = get_file_path(context, var.value.location.file);
                    results.push(json!({
                        "kind": "var",
                        "name": name.to_string(),
                        "type_path": ty.path.to_string(),
                        "location": format!("{}:{}",
                            file_path,
                            var.value.location.line
                        )
                    }));
                }
            }
        }
    }

    let result = json!({
        "count": results.len(),
        "results": results
    });

    Ok(ToolResult::text(serde_json::to_string_pretty(&result)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SOURCE_FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_source_file(contents: &str) -> PathBuf {
        let unique_suffix = format!(
            "{}_{}",
            std::process::id(),
            SOURCE_FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(format!("meridian_mcp_source_{unique_suffix}.dm"));
        std::fs::write(&path, contents).expect("source fixture should be writable");
        path
    }

    #[test]
    fn extract_source_reads_an_indented_proc_until_the_next_declaration() {
        let path = write_source_file(
            "/proc/example()\n\tvar/value = 1\n\treturn value\n/proc/next()\n\treturn\n",
        );

        let source = extract_source(path.to_str().unwrap(), 1).expect("source should exist");
        assert_eq!(source, "/proc/example()\n\tvar/value = 1\n\treturn value");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn extract_source_keeps_column_zero_comments_inside_the_declaration() {
        let path = write_source_file(
            "/proc/example()\n\treturn\n// explanation\n/proc/next()\n\treturn\n",
        );

        let source = extract_source(path.to_str().unwrap(), 1).expect("source should exist");
        assert_eq!(source, "/proc/example()\n\treturn\n// explanation");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn extract_source_returns_none_for_a_missing_line() {
        let path = write_source_file("/proc/example()\n\treturn\n");

        assert_eq!(extract_source(path.to_str().unwrap(), 99), None);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn extract_source_is_capped_at_maximum_source_lines() {
        let mut contents = String::from("/proc/example()\n");
        for _ in 0..(MAX_SOURCE_LINES + 20) {
            contents.push_str("\treturn\n");
        }
        let path = write_source_file(&contents);

        let source = extract_source(path.to_str().unwrap(), 1).expect("source should exist");
        assert_eq!(source.lines().count(), MAX_SOURCE_LINES);
        let _ = std::fs::remove_file(path);
    }
}
