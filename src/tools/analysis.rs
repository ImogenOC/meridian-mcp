use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tracing::info;

use crate::analysis_snapshot::AnalysisContext;
use crate::mcp::ToolResult;
use crate::result::{json_success, ToolMetadata};
use crate::state::ServerState;

/// Helper to get file path string from a location
fn get_file_path(context: &AnalysisContext, file_id: dreammaker::FileId) -> String {
    context.file_path(file_id).display().to_string()
}

/// Run type checker and return errors
pub async fn check_errors(state: &ServerState, args: Value) -> Result<ToolResult> {
    let snapshot = state.snapshot().await?;

    let file_filter = args.get("file_path").and_then(|v| v.as_str());

    info!("Running type checker...");

    let diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| file_filter.is_none_or(|filter| diagnostic.file.contains(filter)))
        .collect::<Vec<_>>();

    let result = json!({
        "count": diagnostics.len(),
        "diagnostics": diagnostics
    });

    Ok(json_success(
        ToolMetadata::complete(Some(snapshot.generation)),
        result,
    ))
}

/// Get definition location for a symbol
pub async fn get_definition(state: &ServerState, args: Value) -> Result<ToolResult> {
    let snapshot = state.snapshot().await?;
    let objtree = &snapshot.objtree;
    let context = &snapshot.context;

    let type_path = args
        .get("type_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing type_path argument"))?;

    let member_name = args.get("member_name").and_then(|v| v.as_str());

    match objtree.find(type_path) {
        Some(ty) => {
            if let Some(member) = member_name {
                let mut current = Some(ty);
                while let Some(t) = current {
                    if let Some(var) = t.vars.get(member) {
                        if var.declaration.is_some() {
                            let file_path = get_file_path(context, var.value.location.file);
                            let result = json!({
                                "kind": "var",
                                "name": member,
                                "type_path": type_path,
                                "defined_in": t.path,
                                "file": file_path,
                                "line": var.value.location.line,
                                "column": var.value.location.column
                                ,"declaration_kind": "var",
                                "resolved_type_owner": t.path,
                                "state_generation": snapshot.generation,
                                "spacemandmm_revision": snapshot.spacemandmm_revision
                            });
                            return Ok(ToolResult::text(serde_json::to_string_pretty(&result)?));
                        }
                    }

                    if let Some(proc) = t.procs.get(member) {
                        if proc.declaration.is_some() {
                            if let Some(first) = proc.value.first() {
                                let file_path = get_file_path(context, first.location.file);
                                let result = json!({
                                    "kind": "proc",
                                    "name": member,
                                    "type_path": type_path,
                                    "defined_in": t.path,
                                    "file": file_path,
                                    "line": first.location.line,
                                    "column": first.location.column
                                    ,"declaration_kind": "proc",
                                    "resolved_type_owner": t.path,
                                    "state_generation": snapshot.generation,
                                    "spacemandmm_revision": snapshot.spacemandmm_revision
                                });
                                return Ok(ToolResult::text(serde_json::to_string_pretty(
                                    &result,
                                )?));
                            }
                        }
                    }

                    current = t.parent_type();
                }

                Ok(ToolResult::error(format!(
                    "Member not found: {type_path}/{member}"
                )))
            } else {
                // Just get the type definition
                let file_path = get_file_path(context, ty.location.file);
                let result = json!({
                    "kind": "type",
                    "path": ty.path,
                    "file": file_path,
                    "line": ty.location.line,
                    "column": ty.location.column
                    ,"declaration_kind": "type",
                    "resolved_type_owner": ty.path,
                    "state_generation": snapshot.generation,
                    "spacemandmm_revision": snapshot.spacemandmm_revision
                });
                Ok(ToolResult::text(serde_json::to_string_pretty(&result)?))
            }
        }
        None => Ok(ToolResult::error(format!("Type not found: {type_path}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::ToolContent;
    use crate::tools::parse::parse_environment;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn result_json(result: &ToolResult) -> Value {
        let ToolContent::Text { text } = &result.content[0];
        serde_json::from_str(text).expect("tool result should be JSON")
    }

    async fn inherited_definition_fixture() -> (std::path::PathBuf, ServerState) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "meridian-mcp-definition-{}-{unique}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let dme_path = directory.join("fixture.dme");
        std::fs::write(&dme_path, "#include \"fixture.dm\"\n").unwrap();
        std::fs::write(
            directory.join("fixture.dm"),
            r#"/datum/definition_parent
	var/inherited_value = 1

/datum/definition_parent/proc/inherited_proc()
	return inherited_value

/datum/definition_parent/child
	inherited_value = 2
"#,
        )
        .unwrap();

        let state = ServerState::new();
        let parse_result = parse_environment(&state, json!({"dme_path": dme_path}))
            .await
            .unwrap();
        assert_eq!(parse_result.is_error, None);
        (directory, state)
    }

    #[tokio::test]
    async fn inherited_member_definitions_report_the_declaring_type() {
        let (directory, state) = inherited_definition_fixture().await;

        for (member, kind) in [("inherited_value", "var"), ("inherited_proc", "proc")] {
            let result = get_definition(
                &state,
                json!({
                    "type_path": "/datum/definition_parent/child",
                    "member_name": member,
                }),
            )
            .await
            .unwrap();
            let payload = result_json(&result);
            assert_eq!(payload["kind"], kind);
            assert_eq!(payload["defined_in"], "/datum/definition_parent");
            assert_eq!(payload["type_path"], "/datum/definition_parent/child");
        }

        std::fs::remove_dir_all(directory).unwrap();
    }
}
