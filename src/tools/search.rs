use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};

use crate::mcp::ToolResult;
use crate::search::{SearchIndex, SearchRequest, SymbolKind};
use crate::state::ServerState;

const DEFAULT_RESULT_LIMIT: usize = 10;
const MAX_RESULT_LIMIT: usize = 50;
const DEFAULT_SOURCE_LINES: usize = 40;
const MAX_SOURCE_LINES: usize = 200;

pub(crate) async fn search_context(state: &ServerState, args: Value) -> Result<ToolResult> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .ok_or_else(|| anyhow!("Missing or empty query argument"))?;

    let snapshot = match state.snapshot().await {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return Ok(ToolResult::error(
                "No search index loaded. Call dm_parse_environment first.",
            ));
        }
    };
    let index = &snapshot.search_index;

    let kind = parse_kind(args.get("kind").and_then(Value::as_str).unwrap_or("all"))?;
    let type_prefix = optional_nonempty_string(&args, "type_prefix");
    let file_filter = optional_nonempty_string(&args, "file_filter");
    let limit = bounded_usize(&args, "limit", DEFAULT_RESULT_LIMIT, 1, MAX_RESULT_LIMIT)?;
    let include_source = args
        .get("include_source")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let max_source_lines = bounded_usize(
        &args,
        "max_source_lines",
        DEFAULT_SOURCE_LINES,
        1,
        MAX_SOURCE_LINES,
    )?;

    let request = SearchRequest {
        query,
        kind,
        type_prefix,
        file_filter,
        limit,
    };
    let hits = index.search(&request);
    let results: Vec<Value> = hits
        .iter()
        .map(|hit| {
            let document = hit.document;
            let mut result = Map::from_iter([
                (
                    "score".to_string(),
                    json!((hit.score * 1_000.0).round() / 1_000.0),
                ),
                ("kind".to_string(), json!(document.kind.as_str())),
                ("symbol".to_string(), json!(document.symbol)),
                ("name".to_string(), json!(document.name)),
                ("type_path".to_string(), json!(document.type_path)),
                ("parent".to_string(), json!(document.parent)),
                ("file".to_string(), json!(document.file)),
                ("line".to_string(), json!(document.line)),
                ("column".to_string(), json!(document.column)),
                ("docs".to_string(), json!(document.docs)),
                ("parameters".to_string(), json!(document.parameters)),
                ("override_index".to_string(), json!(document.override_index)),
                ("override_count".to_string(), json!(document.override_count)),
            ]);

            if include_source {
                result.insert(
                    "source".to_string(),
                    json!(document
                        .source
                        .as_deref()
                        .map(|source| truncate_lines(source, max_source_lines))),
                );
            }

            Value::Object(result)
        })
        .collect();

    let response = json!({
        "query": query,
        "query_terms": SearchIndex::query_terms(query),
        "indexed_documents": index.len(),
        "count": results.len(),
        "results": results,
    });
    Ok(ToolResult::text(serde_json::to_string_pretty(&response)?))
}

fn parse_kind(kind: &str) -> Result<Option<SymbolKind>> {
    match kind {
        "all" => Ok(None),
        "type" => Ok(Some(SymbolKind::Type)),
        "proc" => Ok(Some(SymbolKind::Proc)),
        "var" => Ok(Some(SymbolKind::Var)),
        _ => Err(anyhow!(
            "Invalid kind '{kind}'. Expected all, type, proc, or var."
        )),
    }
}

fn optional_nonempty_string<'a>(args: &'a Value, name: &str) -> Option<&'a str> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn bounded_usize(
    args: &Value,
    name: &str,
    default: usize,
    minimum: usize,
    maximum: usize,
) -> Result<usize> {
    let Some(value) = args.get(name) else {
        return Ok(default);
    };
    let value = value
        .as_u64()
        .ok_or_else(|| anyhow!("{name} must be a positive integer"))?;
    usize::try_from(value)
        .map(|value| value.clamp(minimum, maximum))
        .map_err(|_| anyhow!("{name} is too large"))
}

fn truncate_lines(source: &str, maximum: usize) -> String {
    source.lines().take(maximum).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn context_search_before_parsing_returns_actionable_tool_error() {
        let state = crate::state::ServerState::new();

        let result = search_context(&state, serde_json::json!({"query": "air"}))
            .await
            .expect("tool call should serialize an expected state error");
        let value = serde_json::to_value(result).expect("tool result should serialize");

        assert_eq!(value["isError"], true);
        assert!(value["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("dm_parse_environment")));
    }
}
