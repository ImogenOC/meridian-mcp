use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::Path;

use crate::index::{ReferenceHit, SymbolId};
use crate::limits::ServerLimits;
use crate::mcp::ToolResult;
use crate::result::{json_success, ToolMetadata};
use crate::state::ServerState;

pub async fn document_symbols(state: &ServerState, args: Value) -> Result<ToolResult> {
    let snapshot = state.snapshot().await?;
    let file = args
        .get("file_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing file_path argument"))?;
    let maximum = ServerLimits::default().max_document_symbols;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(maximum)
        .min(maximum);
    let symbols = snapshot.language_index.document_symbols(Path::new(file));
    let truncated = symbols.len() > limit;
    let symbols = symbols.iter().take(limit).collect::<Vec<_>>();
    let mut metadata = ToolMetadata::complete(Some(snapshot.generation));
    metadata.truncated = truncated;
    if truncated {
        metadata
            .truncation_reasons
            .push("document_symbol_limit".to_owned());
    }
    Ok(json_success(
        metadata,
        json!({ "count": symbols.len(), "symbols": symbols }),
    ))
}

pub async fn find_implementations(state: &ServerState, args: Value) -> Result<ToolResult> {
    let snapshot = state.snapshot().await?;
    let owner = args
        .get("type_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing type_path argument"))?;
    let member = args.get("member_name").and_then(Value::as_str);
    let maximum = ServerLimits::default().max_reference_results;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(maximum)
        .min(maximum);
    let implementations = snapshot.language_index.implementations(owner, member);
    let truncated = implementations.len() > limit;
    let implementations = implementations.into_iter().take(limit).collect::<Vec<_>>();
    let mut metadata = ToolMetadata::complete(Some(snapshot.generation));
    metadata.truncated = truncated;
    if truncated {
        metadata
            .truncation_reasons
            .push("implementation_limit".to_owned());
    }
    Ok(json_success(
        metadata,
        json!({ "count": implementations.len(), "implementations": implementations }),
    ))
}

pub async fn find_references(state: &ServerState, args: Value) -> Result<ToolResult> {
    let snapshot = state.snapshot().await?;
    let owner = args
        .get("type_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing type_path argument"))?;
    let member = args.get("member_name").and_then(Value::as_str);
    let ty = snapshot
        .objtree
        .find(owner)
        .ok_or_else(|| anyhow!("Type not found: {owner}"))?;
    let (symbol, upstream_symbol) = if let Some(member) = member {
        let variable = ty.get_var_declaration(member);
        let procedure = ty.get_proc_declaration(member);
        match (variable, procedure) {
            (Some(_), Some(_)) => {
                return Err(anyhow!(
                "Ambiguous member {owner}/{member}: both variable and procedure declarations exist"
            ))
            }
            (Some(declaration), None) => {
                let declared_in = snapshot
                    .objtree
                    .iter_types()
                    .find(|candidate| {
                        candidate.vars.values().any(|var| {
                            var.declaration
                                .as_ref()
                                .is_some_and(|item| item.id == declaration.id)
                        })
                    })
                    .map(|candidate| candidate.path.to_string())
                    .unwrap_or_else(|| owner.to_owned());
                (
                    SymbolId::Var {
                        owner: declared_in,
                        name: member.to_owned(),
                    },
                    declaration.id,
                )
            }
            (None, Some(declaration)) => {
                let declared_in = snapshot
                    .objtree
                    .iter_types()
                    .find(|candidate| {
                        candidate.procs.values().any(|proc_value| {
                            proc_value
                                .declaration
                                .as_ref()
                                .is_some_and(|item| item.id == declaration.id)
                        })
                    })
                    .map(|candidate| candidate.path.to_string())
                    .unwrap_or_else(|| owner.to_owned());
                (
                    SymbolId::Proc {
                        owner: declared_in,
                        name: member.to_owned(),
                        override_index: 0,
                    },
                    declaration.id,
                )
            }
            (None, None) => return Err(anyhow!("Member not found: {owner}/{member}")),
        }
    } else {
        (
            SymbolId::Type {
                path: ty.path.to_string(),
            },
            ty.id,
        )
    };
    let maximum = ServerLimits::default().max_reference_results;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(maximum)
        .min(maximum);
    let mut references = snapshot
        .reference_table
        .references(upstream_symbol)
        .iter()
        .map(|reference| ReferenceHit {
            symbol: symbol.clone(),
            kind: reference.kind,
            file: snapshot
                .context
                .file_path(reference.location.file)
                .display()
                .to_string(),
            line: reference.location.line,
            column: reference.location.column,
        })
        .collect::<Vec<_>>();
    let skipped_dynamic = snapshot.reference_table.skipped_dynamic();
    if !args
        .get("include_declaration")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        references.retain(|hit| {
            !snapshot
                .language_index
                .document_symbols(Path::new(&hit.file))
                .iter()
                .any(|declaration| declaration.id == hit.symbol && declaration.line == hit.line)
        });
    }
    if let Some(kind) = args.get("kind").and_then(Value::as_str) {
        references.retain(|hit| {
            serde_json::to_value(hit.kind)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .as_deref()
                == Some(kind)
        });
    }
    let truncated = references.len() > limit;
    let references = references.into_iter().take(limit).collect::<Vec<_>>();
    let mut metadata = ToolMetadata::complete(Some(snapshot.generation));
    metadata.truncated = truncated;
    if truncated {
        metadata
            .truncation_reasons
            .push("reference_limit".to_owned());
    }
    Ok(json_success(
        metadata,
        json!({ "count": references.len(), "skipped_dynamic": skipped_dynamic, "references": references }),
    ))
}
