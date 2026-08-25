use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use tracing::info;

use dreammaker::{Context, Parser, Preprocessor, Severity};

use crate::analysis_snapshot::{
    collect_diagnostics, configured_diagnostic_rules, AnalysisBuild, AnalysisContext,
    AnalysisSnapshot,
};
use crate::mcp::ToolResult;
use crate::result::{structured_error, ToolErrorCode};
use crate::search::SearchIndex;
use crate::source::extract_source;
use crate::state::ServerState;
use crate::{PathPolicy, ProjectProfile};

/// Parse a DreamMaker environment
pub async fn parse_environment(state: &ServerState, args: Value) -> Result<ToolResult> {
    let dme_path = args
        .get("dme_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing dme_path argument"))?;

    let path = PathBuf::from(dme_path);
    if !path.exists() {
        return Ok(ToolResult::error(format!("File not found: {dme_path}")));
    }

    info!("Parsing environment: {}", dme_path);

    let prior = state.active_snapshot().await;
    let parse_path = path.clone();
    let parsed = tokio::task::spawn_blocking(move || -> Result<_> {
        let mut context = Context::default();
        context.autodetect_config(&parse_path);
        let mut preprocessor = Preprocessor::new(&context, parse_path.clone())?;
        let (fatal, objtree) = {
            let mut parser = Parser::new(&context, &mut preprocessor);
            parser.enable_procs();
            parser.parse_object_tree_2()
        };
        let defines = preprocessor.finalize();
        let blocking_errors = context
            .errors()
            .iter()
            .filter(|diagnostic| {
                let description = diagnostic.description();
                diagnostic.severity() == Severity::Error
                    && (description.starts_with("failed to find #include")
                        || description.starts_with("failed to open file: #include")
                        || description == "i/o error opening file"
                        || description == "i/o error reading file")
            })
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if fatal || !blocking_errors.is_empty() {
            let diagnostics = blocking_errors.join("\n");
            return Err(anyhow!("DreamMaker parser reported errors:\n{diagnostics}"));
        }

        dreamchecker::run(&context, &objtree);
        let configured_rules = configured_diagnostic_rules(&parse_path);
        let diagnostics = collect_diagnostics(&context, &configured_rules);
        let type_count = objtree.iter_types().count();
        let search_index = SearchIndex::from_object_tree(&objtree, &context, &parse_path);
        let indexed_document_count = search_index.len();
        let profile = parse_path.parent().and_then(|root| {
            PathPolicy::new(vec![root.to_owned()], Vec::new())
                .ok()
                .and_then(|policy| ProjectProfile::discover(&policy, &parse_path).ok())
        });
        let build = AnalysisBuild::from_parse(
            parse_path,
            &context,
            objtree,
            defines,
            search_index,
            diagnostics,
            profile,
        );
        Ok((build, type_count, indexed_document_count))
    })
    .await;

    match parsed {
        Ok(Ok((build, type_count, indexed_document_count))) => {
            let snapshot = state.install_analysis(build).await;
            Ok(ToolResult::text(serde_json::to_string_pretty(&json!({
                "success": true,
                "environment": dme_path,
                "total_types": type_count,
                "indexed_symbols": indexed_document_count,
                "state_generation": snapshot.generation
            }))?))
        }
        Ok(Err(error)) => parse_failure(state, prior.as_deref(), error.to_string()).await,
        Err(error) => {
            parse_failure(
                state,
                prior.as_deref(),
                format!("parser worker failed: {error}"),
            )
            .await
        }
    }
}

async fn parse_failure(
    state: &ServerState,
    prior: Option<&AnalysisSnapshot>,
    error: String,
) -> Result<ToolResult> {
    Ok(structured_error(
        ToolErrorCode::InvalidInput,
        error,
        Some("Correct the DreamMaker parse errors and run dm_parse_environment again.".to_owned()),
        json!({
        "state_preserved": true,
        "active_environment": prior.map(|snapshot| snapshot.environment_path.display().to_string()),
        "state_generation": state.state_generation().await
        }),
    ))
}

/// Helper to get file path string from a location
fn get_file_path(context: &AnalysisContext, file_id: dreammaker::FileId) -> String {
    context.file_path(file_id).display().to_string()
}

fn resolve_source_path(snapshot: &AnalysisSnapshot, file_path: &str) -> PathBuf {
    let path = PathBuf::from(file_path);
    if path.is_absolute() || path.exists() {
        return path;
    }

    snapshot
        .environment_path
        .parent()
        .map(|root| root.join(&path))
        .unwrap_or(path)
}

/// Get type information
pub async fn get_type(state: &ServerState, args: Value) -> Result<ToolResult> {
    let snapshot = state.snapshot().await?;
    let objtree = &snapshot.objtree;
    let context = &snapshot.context;

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
pub async fn get_proc(state: &ServerState, args: Value) -> Result<ToolResult> {
    let snapshot = state.snapshot().await?;
    let objtree = &snapshot.objtree;
    let context = &snapshot.context;

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
            let mut current_type = Some(ty);
            let mut resolved_type_path = None;
            while let Some(candidate_type) = current_type {
                if let Some(proc_entry) = candidate_type.procs.get(proc_name) {
                    if resolved_type_path.is_none() || proc_entry.declaration.is_some() {
                        resolved_type_path = Some(candidate_type.path.to_string());
                    }
                    if proc_entry.declaration.is_some() {
                        break;
                    }
                }
                current_type = candidate_type.parent_type();
            }

            match resolved_type_path {
                Some(declared_in) => {
                    let declaring_type = objtree
                        .find(&declared_in)
                        .expect("resolved proc owner should remain in the object tree");
                    let proc = declaring_type
                        .procs
                        .get(proc_name)
                        .expect("resolved proc should remain on its owner");
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
                                    resolve_source_path(&snapshot, &file_path).to_string_lossy().as_ref(),
                                    v.location.line,
                                )
                            })
                        })
                        .collect();

                    let result = json!({
                        "name": proc_name,
                        "type_path": type_path,
                        "declared": declared_in == type_path && proc.declaration.is_some(),
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
pub async fn get_var(state: &ServerState, args: Value) -> Result<ToolResult> {
    let snapshot = state.snapshot().await?;
    let objtree = &snapshot.objtree;
    let context = &snapshot.context;

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
pub async fn list_types(state: &ServerState, args: Value) -> Result<ToolResult> {
    let snapshot = state.snapshot().await?;
    let objtree = &snapshot.objtree;

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
pub async fn search_symbols(state: &ServerState, args: Value) -> Result<ToolResult> {
    let snapshot = state.snapshot().await?;
    let objtree = &snapshot.objtree;
    let context = &snapshot.context;

    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing query argument"))?
        .to_lowercase();

    let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("all");

    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let mut results: Vec<Value> = Vec::new();

    if kind == "all" || kind == "macro" {
        for symbol in snapshot.language_index.macros() {
            if results.len() >= limit {
                break;
            }
            if symbol.name.to_lowercase().contains(&query) {
                results.push(json!({"kind":"macro","name":symbol.name,"location":format!("{}:{}", symbol.file, symbol.line),"file":symbol.file,"line":symbol.line,"column":symbol.column}));
            }
        }
    }

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
        let state = ServerState::new();
        let result = parse_environment(&state, json!({"dme_path": dme_path}))
            .await
            .unwrap();
        assert_eq!(result.is_error, None, "parse result: {result:?}");
        (directory, state)
    }

    #[tokio::test]
    async fn type_inspection_returns_docs_and_direct_children() {
        let (directory, state) = parsed_fixture().await;
        let result = get_type(&state, json!({"type_path": "/datum/meridian_fixture"}))
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
        let (directory, state) = parsed_fixture().await;
        let result = get_proc(
            &state,
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
    async fn proc_inspection_resolves_inherited_procs() {
        let (directory, state) = parsed_fixture().await;
        let result = get_proc(
            &state,
            json!({
                "type_path": "/datum/meridian_fixture/child",
                "proc_name": "do_work"
            }),
        )
        .await
        .unwrap();
        let payload = result_json(&result);

        assert_eq!(result.is_error, None);
        assert_eq!(payload["type_path"], "/datum/meridian_fixture/child");
        assert_eq!(payload["declared"], false);
        assert_eq!(payload["overrides"][0]["has_body"], true);

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn variable_inspection_returns_declared_type_and_docs() {
        let (directory, state) = parsed_fixture().await;
        let result = get_var(
            &state,
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

    #[tokio::test]
    async fn failed_reparse_preserves_the_active_project_profile() {
        let (directory, dme_path) = write_environment_fixture();
        let state = ServerState::new();
        let first = parse_environment(&state, json!({"dme_path": dme_path}))
            .await
            .unwrap();
        assert_eq!(first.is_error, None);
        let generation = state.state_generation().await;
        let active_dme = state
            .snapshot()
            .await
            .unwrap()
            .project_profile
            .as_ref()
            .expect("successful parse should discover a profile")
            .dme_path()
            .to_owned();

        let missing_dme = directory.join("missing.dme");
        let failed = parse_environment(&state, json!({"dme_path": missing_dme}))
            .await
            .unwrap();

        assert_eq!(failed.is_error, Some(true));
        assert_eq!(state.state_generation().await, generation);
        assert_eq!(
            state
                .snapshot()
                .await
                .unwrap()
                .project_profile
                .as_ref()
                .expect("failed parse should preserve the prior profile")
                .dme_path(),
            active_dme
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn nonfatal_parser_errors_preserve_the_active_snapshot() {
        let (directory, dme_path) = write_environment_fixture();
        let state = ServerState::new();
        let first = parse_environment(&state, json!({"dme_path": dme_path}))
            .await
            .unwrap();
        assert_eq!(first.is_error, None);
        let generation = state.state_generation().await;

        let invalid_dme = directory.join("invalid.dme");
        std::fs::write(&invalid_dme, "#include \"missing.dm\"\n").unwrap();
        let failed = parse_environment(&state, json!({"dme_path": invalid_dme}))
            .await
            .unwrap();

        assert_eq!(failed.is_error, Some(true), "parse result: {failed:?}");
        assert_eq!(state.state_generation().await, generation);
        let preserved = get_type(&state, json!({"type_path": "/datum/meridian_fixture"}))
            .await
            .unwrap();
        assert_eq!(preserved.is_error, None, "lookup result: {preserved:?}");
        std::fs::remove_dir_all(directory).unwrap();
    }
}
