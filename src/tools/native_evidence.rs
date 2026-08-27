use super::ToolExecutionContext;
use crate::mcp::ToolResult;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

pub async fn summary(context: &ToolExecutionContext, args: Value) -> Result<ToolResult> {
    let request: crate::native_evidence::model::NativeEvidenceRequest =
        serde_json::from_value(args)?;
    let evidence = evidence_context(context);
    let result = tokio::task::spawn_blocking(move || {
        crate::native_evidence::summarize_run(&evidence, request)
    })
    .await??;
    Ok(ToolResult::text(serde_json::to_string_pretty(&result)?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompareRequest {
    runs: Vec<crate::native_evidence::model::NativeEvidenceRequest>,
}

pub async fn compare(context: &ToolExecutionContext, args: Value) -> Result<ToolResult> {
    let request: CompareRequest = serde_json::from_value(args)?;
    let evidence = evidence_context(context);
    let result = tokio::task::spawn_blocking(move || {
        crate::native_evidence::compare_runs(&evidence, request.runs)
    })
    .await?;
    match result {
        Ok(result) => Ok(ToolResult::text(serde_json::to_string_pretty(&result)?)),
        Err(error) if error.to_string().contains("evidence_identity_mismatch") => {
            Ok(ToolResult::structured_error(
                "evidence_identity_mismatch",
                error.to_string(),
                "Use runs with the same verified managed build and workload identity.",
            ))
        }
        Err(error) => Err(anyhow!(error)),
    }
}

fn evidence_context(
    context: &ToolExecutionContext,
) -> crate::native_evidence::NativeEvidenceContext {
    crate::native_evidence::NativeEvidenceContext {
        policy: context.policy().clone(),
        provenance: context.build_provenance_arc(),
    }
}
