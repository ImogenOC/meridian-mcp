use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use tracing::info;

use dreammaker::Context;

use crate::mcp::ToolResult;
use crate::search::SearchIndex;
use crate::source::extract_source;
use crate::state::ServerState;
use crate::{PathPolicy, ProjectProfile};

/// Parse a DreamMaker environment
pub async fn parse_environment(state: &mut ServerState, args: Value) -> Result<ToolResult> {
    let dme_path = args
        .get("dme_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing dme_path argument"))?;

    let path = PathBuf::from(dme_path);
    if !path.exists() {
        return Ok(ToolResult::error(format!("File not found: {dme_path}")));
    }

    info!("Parsing environment: {}", dme_path);

    let context = Context::default();
    match context.parse_environment(&path) {
        Ok(objtree) => {
            let type_count = objtree.iter_types().count();
            let search_index = SearchIndex::from_object_tree(&objtree, &context, &path);
            let indexed_document_count = search_index.len();

            let profile = path.parent().and_then(|root| {
                PathPolicy::new(vec![root.to_owned()], Vec::new())
                    .ok()
                    .and_then(|policy| ProjectProfile::discover(&policy, &path).ok())
            });
            state.replace_environment(path, context, objtree, search_index, profile);

            Ok(ToolResult::text(serde_json::to_string_pretty(&json!({
                "success": true,
                "environment": dme_path,
                "total_types": type_count,
                "indexed_symbols": indexed_document_count,
                "state_generation": state.state_generation()
            }))?))
        }
        Err(error) => Ok(ToolResult::error(serde_json::to_string_pretty(&json!({
            "success": false,
            "error": error.to_string(),
            "state_preserved": true,
            "active_environment": state.environment_path.as_ref().map(|path| path.display().to_string()),
            "state_generation": state.state_generation()
        }))?)),
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
                        "constant": var.value.constant.as_ref().map(|c| format!("{c:?}")),
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
            let children: Vec<_> = ty.children().map(|child| child.path.to_string()).collect();

            let file_path = get_file_path(context, ty.location.file);

            let result = json!({
                "path": ty.path,
                "parent": parent,
                "children": children,
                "documentation": ty.docs.text(),
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
        None => Ok(ToolResult::error(format!("Type not found: {type_path}"))),
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
                                "documentation": v.docs.text(),
                                "location": format!("{}:{}:{}",
                                    file_path,
                                    v.location.line,
                                    v.location.column
                                ),
                                "has_body": v.code.is_some() || v.body_range.is_some(),
                                "source": extract_source(
                                    resolve_source_path(state, &file_path).to_string_lossy().as_ref(),
                                    v.location.line,
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
                    "Proc not found: {type_path}/{proc_name}"
                ))),
            }
        }
        None => Ok(ToolResult::error(format!("Type not found: {type_path}"))),
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
                    "declared_type": var.declaration.as_ref().map(|declaration| declaration.var_type.to_string()),
                    "documentation": var.value.docs.text(),
                    "constant": var.value.constant.as_ref().map(|c| format!("{c:?}")),
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
                "Variable not found: {type_path}/{var_name}"
            ))),
        },
        None => Ok(ToolResult::error(format!("Type not found: {type_path}"))),
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
    use crate::mcp::{ToolContent, ToolResult};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn result_json(result: &ToolResult) -> Value {
        let ToolContent::Text { text } = &result.content[0];
        serde_json::from_str(text).expect("tool result should be JSON")
    }

    fn write_environment_fixture() -> (PathBuf, PathBuf) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "meridian-mcp-inspection-{}-{unique}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let dme_path = directory.join("fixture.dme");
        std::fs::write(&dme_path, "#include \"fixture.dm\"\n").unwrap();
        std::fs::write(
            directory.join("fixture.dm"),
            r#"/** Fixture parent documentation. */
/datum/meridian_fixture
	/// Stored fixture values.
	var/list/items = list()

/** Return the supplied value.
 * Arguments:
 * * value - value to return
 */
/datum/meridian_fixture/proc/do_work(value)
	return value

/datum/meridian_fixture/child
"#,
        )
        .unwrap();
        (directory, dme_path)
    }

    async fn parsed_fixture() -> (PathBuf, ServerState) {
        let (directory, dme_path) = write_environment_fixture();
        let mut state = ServerState::new();
        let result = parse_environment(&mut state, json!({"dme_path": dme_path}))
            .await
            .unwrap();
        assert_eq!(result.is_error, None, "parse result: {result:?}");
        (directory, state)
    }

    #[tokio::test]
    async fn type_inspection_returns_docs_and_direct_children() {
        let (directory, mut state) = parsed_fixture().await;
        let result = get_type(&mut state, json!({"type_path": "/datum/meridian_fixture"}))
            .await
            .unwrap();
        let payload = result_json(&result);

        assert!(payload["documentation"]
            .as_str()
            .unwrap()
            .contains("Fixture parent documentation"));
        assert_eq!(
            payload["children"],
            json!(["/datum/meridian_fixture/child"])
        );

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn proc_inspection_reports_docs_and_a_parsed_body() {
        let (directory, mut state) = parsed_fixture().await;
        let result = get_proc(
            &mut state,
            json!({
                "type_path": "/datum/meridian_fixture",
                "proc_name": "do_work"
            }),
        )
        .await
        .unwrap();
        let payload = result_json(&result);

        assert_eq!(payload["overrides"][0]["has_body"], true);
        assert!(payload["overrides"][0]["documentation"]
            .as_str()
            .unwrap()
            .contains("Return the supplied value"));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn variable_inspection_returns_declared_type_and_docs() {
        let (directory, mut state) = parsed_fixture().await;
        let result = get_var(
            &mut state,
            json!({
                "type_path": "/datum/meridian_fixture",
                "var_name": "items"
            }),
        )
        .await
        .unwrap();
        let payload = result_json(&result);

        assert!(payload["declared_type"].as_str().unwrap().contains("list"));
        assert!(payload["documentation"]
            .as_str()
            .unwrap()
            .contains("Stored fixture values"));

        std::fs::remove_dir_all(directory).unwrap();
    }
}
