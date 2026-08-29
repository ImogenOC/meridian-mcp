use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
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
use crate::source_fingerprint::SourceFingerprint;
use crate::state::ServerState;
use crate::{PathPolicy, ProjectProfile};

/// Diagnostics that mean the environment was never fully read, so the resulting
/// object tree is silently incomplete rather than merely flawed.
///
/// SpacemanDMM has no structured discriminant for these, so they are matched on
/// description text. The strings below are verified against the pinned revision
/// (`preprocessor.rs` and `lexer.rs`); if an upstream bump reworded any of them
/// the failure is silent and severe — a truncated tree installed as a success —
/// so `blocking_error_descriptions_match_upstream_wording` guards the list.
const BLOCKING_ERROR_PREFIXES: &[&str] =
    &["failed to find #include", "failed to open file: #include"];
const BLOCKING_ERROR_MESSAGES: &[&str] = &["i/o error opening file", "i/o error reading file"];

/// Default ceiling on a single parse. Generous enough for a station-sized
/// environment on a cold cache; present so a pathological input cannot wedge a
/// client forever with no reply.
const DEFAULT_PARSE_TIMEOUT_MS: u64 = 600_000;

fn is_blocking_error(description: &str) -> bool {
    BLOCKING_ERROR_PREFIXES
        .iter()
        .any(|prefix| description.starts_with(prefix))
        || BLOCKING_ERROR_MESSAGES.contains(&description)
}

/// Parse a DreamMaker environment
pub async fn parse_environment(state: &ServerState, args: Value) -> Result<ToolResult> {
    let dme_path = args
        .get("dme_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing dme_path argument"))?;

    let path = PathBuf::from(dme_path);
    if !path.is_file() {
        let reason = if path.is_dir() {
            format!("Not a file (expected a .dme environment): {dme_path}")
        } else {
            format!("File not found: {dme_path}")
        };
        let prior = state
            .active_snapshot()
            .await
            .map(|snapshot| snapshot.environment_path.clone());
        return parse_failure(state, prior.as_deref(), reason).await;
    }

    let timeout = Duration::from_millis(
        args.get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_PARSE_TIMEOUT_MS),
    );
    let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);

    // Serialize parses before touching the active snapshot, so a concurrent
    // caller cannot observe a half-replaced state or double peak memory.
    let permit = state.parse_permit().await;

    if !force {
        if let Some(reused) = reusable_snapshot(state, &path).await {
            info!("Reusing analysis snapshot for {}", path.display());
            let (errors, warnings) = reused.diagnostic_counts();
            return Ok(ToolResult::text(serde_json::to_string_pretty(&json!({
                "success": true,
                "reused": true,
                "environment": reused.environment_path.display().to_string(),
                "total_types": reused.total_types,
                "indexed_symbols": reused.indexed_symbol_count(),
                "error_count": errors,
                "warning_count": warnings,
                "state_generation": reused.generation,
                "spacemandmm_revision": reused.spacemandmm_revision,
            }))?));
        }
    }

    info!("Parsing environment: {}", dme_path);

    // Only the prior environment path is needed on the failure path. Holding the
    // whole prior snapshot across the parse would pin the old object tree in
    // memory while the new one is built.
    let prior_environment = state
        .active_snapshot()
        .await
        .map(|snapshot| snapshot.environment_path.clone());

    let started = Instant::now();
    let parse_started_at = SystemTime::now();
    let parse_path = path.clone();
    let mut handle = tokio::task::spawn_blocking(move || -> Result<_> {
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
                diagnostic.severity() == Severity::Error
                    && is_blocking_error(diagnostic.description())
            })
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if fatal || !blocking_errors.is_empty() {
            let diagnostics = blocking_errors.join("\n");
            return Err(anyhow!("DreamMaker parser reported errors:\n{diagnostics}"));
        }

        // Always run. Measured on a ~10k-file, 65k-type environment, skipping
        // dreamchecker saved under 1% of parse time (41.4s vs 41.7s, inside the
        // noise) while dropping every semantic diagnostic — 139 errors and a
        // warning on that environment. There is no useful trade here; the cost
        // is dominated by preprocessing and index construction, not by this.
        dreamchecker::run(&context, &objtree);
        let configured_rules = configured_diagnostic_rules(&parse_path);
        let diagnostics = collect_diagnostics(&context, &configured_rules);
        let search_index = SearchIndex::from_object_tree(&objtree, &context, &parse_path);
        let profile = parse_path.parent().and_then(|root| {
            PathPolicy::new(vec![root.to_owned()], Vec::new())
                .ok()
                .and_then(|policy| ProjectProfile::discover(&policy, &parse_path).ok())
        });
        Ok(AnalysisBuild::from_parse(
            parse_path,
            &context,
            objtree,
            defines,
            search_index,
            diagnostics,
            profile,
            parse_started_at,
        ))
    });

    // A blocking task cannot be aborted, so on timeout the worker keeps running.
    // Move the parse permit into a detached waiter rather than dropping it here:
    // that keeps the next parse queued behind the orphan instead of letting the
    // two coexist, which is exactly the memory blowup this lock exists to stop.
    let parsed = match tokio::time::timeout(timeout, &mut handle).await {
        Ok(joined) => joined,
        Err(_elapsed) => {
            tokio::spawn(async move {
                let _ = handle.await;
                drop(permit);
            });
            return parse_failure(
                state,
                prior_environment.as_deref(),
                format!(
                    "parse exceeded {} ms and was abandoned; the worker is still running and the \
                     next parse will queue behind it",
                    timeout.as_millis()
                ),
            )
            .await;
        }
    };

    let result = match parsed {
        Ok(Ok(build)) => {
            let snapshot = state.install_analysis(build).await;
            let (errors, warnings) = snapshot.diagnostic_counts();
            Ok(ToolResult::text(serde_json::to_string_pretty(&json!({
                "success": true,
                "reused": false,
                "environment": snapshot.environment_path.display().to_string(),
                "total_types": snapshot.total_types,
                "indexed_symbols": snapshot.indexed_symbol_count(),
                "error_count": errors,
                "warning_count": warnings,
                "duration_ms": started.elapsed().as_millis() as u64,
                "state_generation": snapshot.generation,
                "spacemandmm_revision": snapshot.spacemandmm_revision,
            }))?))
        }
        Ok(Err(error)) => {
            parse_failure(state, prior_environment.as_deref(), error.to_string()).await
        }
        Err(error) => {
            parse_failure(
                state,
                prior_environment.as_deref(),
                format!("parser worker failed: {error}"),
            )
            .await
        }
    };
    drop(permit);
    result
}

/// The active snapshot, when it already describes exactly this environment.
///
/// Returns `None` whenever reuse cannot be proven safe: a different environment,
/// or source files whose on-disk state does not match the snapshot's fingerprint.
async fn reusable_snapshot(
    state: &ServerState,
    path: &std::path::Path,
) -> Option<Arc<AnalysisSnapshot>> {
    let snapshot = state.active_snapshot().await?;
    if snapshot.environment_path.as_path() != path {
        return None;
    }
    let current = SourceFingerprint::capture(snapshot.source_inputs(), SystemTime::now());
    snapshot
        .source_fingerprint
        .matches(&current)
        .then_some(snapshot)
}

async fn parse_failure(
    state: &ServerState,
    prior_environment: Option<&std::path::Path>,
    error: String,
) -> Result<ToolResult> {
    Ok(structured_error(
        ToolErrorCode::InvalidInput,
        error,
        Some("Correct the DreamMaker parse errors and run dm_parse_environment again.".to_owned()),
        json!({
        "state_preserved": true,
        "active_environment": prior_environment.map(|path| path.display().to_string()),
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

    let resolution = match snapshot.proc_resolver().resolve(type_path, proc_name) {
        Ok(resolution) => resolution,
        Err(error) => {
            return Ok(structured_error(
                ToolErrorCode::NotFound,
                error.to_string(),
                Some("Check the exact type and proc names in the active analysis snapshot.".into()),
                serde_json::to_value(error)?,
            ));
        }
    };
    let mut values = Vec::with_capacity(resolution.implementations.len());
    for implementation in &resolution.implementations {
        let owner = objtree
            .find(&implementation.owner)
            .expect("resolved proc owner should remain in the object tree");
        let proc_ref = owner
            .iter_self_procs()
            .find(|proc_ref| {
                proc_ref.name() == proc_name && proc_ref.index() == implementation.override_index
            })
            .expect("resolved proc implementation should remain in the object tree");
        let value = proc_ref.get();
        let parameters = value
            .parameters
            .iter()
            .map(|parameter| {
                json!({
                    "name": parameter.name.to_string(),
                    "has_default": parameter.default.is_some(),
                })
            })
            .collect::<Vec<_>>();
        let file_path = get_file_path(context, value.location.file);
        values.push(json!({
            "owner": implementation.owner,
            "override_index": implementation.override_index,
            "parameters": parameters,
            "documentation": value.docs.text(),
            "location": format!("{}:{}:{}", file_path, value.location.line, value.location.column),
            "has_body": implementation.has_body,
            "source": extract_source(
                resolve_source_path(&snapshot, &file_path).to_string_lossy().as_ref(),
                value.location.line,
            ),
        }));
    }

    let result = json!({
        "name": proc_name,
        "type_path": type_path,
        "requested_type_path": resolution.requested_type_path,
        "implementation_owner": resolution.implementation_owner,
        "declaration_owner": resolution.declaration_owner,
        "resolution_kind": resolution.resolution_kind,
        "declared": resolution.implementation_owner == type_path,
        "overrides": values,
        "resolution_diagnostics": resolution.diagnostics(),
    });
    Ok(ToolResult::text(serde_json::to_string_pretty(&result)?))
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

    if kind == "all" || kind == "proc" {
        for resolution in snapshot.proc_resolver().resolutions().filter(|resolution| {
            resolution.requested_type_path == resolution.implementation_owner
                && resolution.proc_name.to_lowercase().contains(&query)
        }) {
            if results.len() >= limit {
                break;
            }
            let first = resolution
                .implementations
                .first()
                .expect("a resolved procedure has an implementation");
            results.push(json!({
                "kind": "proc",
                "name": resolution.proc_name,
                "type_path": resolution.implementation_owner,
                "implementation_owner": resolution.implementation_owner,
                "declaration_owner": resolution.declaration_owner,
                "resolution_kind": resolution.resolution_kind,
                "location": format!("{}:{}", first.location.file, first.location.line),
            }));
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

    /// Push every fixture file's mtime far enough into the past that the
    /// fingerprint's settle window accepts it. Without this, files written
    /// moments ago are deliberately treated as unreusable.
    fn settle(directory: &std::path::Path) {
        let settled = SystemTime::now() - Duration::from_secs(60);
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                std::fs::File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_modified(settled)
                    .unwrap();
            }
        }
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

    #[test]
    fn blocking_error_descriptions_match_upstream_wording() {
        // Verified against the pinned SpacemanDMM revision: the first two are
        // formatted in preprocessor.rs, the last two raised in lexer.rs.
        assert!(is_blocking_error(
            r#"failed to find #include "code/absent.dm""#
        ));
        assert!(is_blocking_error(
            r#"failed to open file: #include "code/absent.dm""#
        ));
        assert!(is_blocking_error("i/o error opening file"));
        assert!(is_blocking_error("i/o error reading file"));

        assert!(!is_blocking_error("expected expression, found ')'"));
        assert!(!is_blocking_error("undefined proc: do_work"));
    }

    #[tokio::test]
    async fn an_unchanged_environment_is_reused_without_reparsing() {
        let (directory, dme_path) = write_environment_fixture();
        settle(&directory);
        let state = ServerState::new();

        let first = parse_environment(&state, json!({"dme_path": dme_path.clone()}))
            .await
            .unwrap();
        assert_eq!(first.is_error, None, "first parse: {first:?}");
        assert_eq!(result_json(&first)["reused"], false);
        let generation = state.state_generation().await;

        let second = parse_environment(&state, json!({"dme_path": dme_path}))
            .await
            .unwrap();
        let payload = result_json(&second);

        assert_eq!(second.is_error, None, "second parse: {second:?}");
        assert_eq!(payload["reused"], true);
        assert_eq!(payload["state_generation"], generation);
        assert_eq!(payload["total_types"], result_json(&first)["total_types"]);
        assert_eq!(state.state_generation().await, generation);

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn an_edited_source_file_forces_a_reparse() {
        let (directory, dme_path) = write_environment_fixture();
        settle(&directory);
        let state = ServerState::new();
        parse_environment(&state, json!({"dme_path": dme_path.clone()}))
            .await
            .unwrap();
        let generation = state.state_generation().await;

        std::fs::write(
            directory.join("fixture.dm"),
            "/datum/meridian_fixture\n\n/datum/meridian_fixture/successor\n",
        )
        .unwrap();
        settle(&directory);

        let reparsed = parse_environment(&state, json!({"dme_path": dme_path}))
            .await
            .unwrap();
        let payload = result_json(&reparsed);

        assert_eq!(reparsed.is_error, None, "reparse: {reparsed:?}");
        assert_eq!(payload["reused"], false);
        assert_eq!(payload["state_generation"], generation + 1);
        assert!(state
            .snapshot()
            .await
            .unwrap()
            .objtree
            .find("/datum/meridian_fixture/successor")
            .is_some());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn force_reparses_an_unchanged_environment() {
        let (directory, dme_path) = write_environment_fixture();
        settle(&directory);
        let state = ServerState::new();
        parse_environment(&state, json!({"dme_path": dme_path.clone()}))
            .await
            .unwrap();
        let generation = state.state_generation().await;

        let forced = parse_environment(&state, json!({"dme_path": dme_path, "force": true}))
            .await
            .unwrap();
        let payload = result_json(&forced);

        assert_eq!(payload["reused"], false);
        assert_eq!(payload["state_generation"], generation + 1);

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn a_successful_parse_reports_diagnostic_counts_and_duration() {
        let (directory, dme_path) = write_environment_fixture();
        settle(&directory);
        let state = ServerState::new();

        let result = parse_environment(&state, json!({"dme_path": dme_path.clone()}))
            .await
            .unwrap();
        let payload = result_json(&result);

        assert!(payload["error_count"].is_u64());
        assert!(payload["warning_count"].is_u64());
        assert!(payload["duration_ms"].is_u64());
        assert!(!payload["spacemandmm_revision"].as_str().unwrap().is_empty());
        assert_eq!(
            payload["environment"].as_str().unwrap(),
            dme_path.display().to_string()
        );

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn a_directory_is_rejected_before_parsing() {
        let (directory, _) = write_environment_fixture();
        let result = parse_environment(
            &ServerState::new(),
            json!({"dme_path": directory.display().to_string()}),
        )
        .await
        .unwrap();

        assert_eq!(result.is_error, Some(true));
        let payload = result_json(&result);
        assert!(
            payload["message"]
                .as_str()
                .is_some_and(|message| message.contains("Not a file")),
            "payload: {payload}"
        );
        assert_eq!(payload["details"]["state_preserved"], true);

        std::fs::remove_dir_all(directory).unwrap();
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
