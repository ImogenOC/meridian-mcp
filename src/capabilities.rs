use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const SPACEMANDMM_REVISION: &str = "351ddc0ffb2439876d4565ce5130bb6b027ee605";

pub const APPROVED_TOOL_NAMES: &[&str] = &[
    "dm_audit_icons",
    "dm_check_errors",
    "dm_compare_dmi_states",
    "dm_compile",
    "dm_debug_control",
    "dm_debug_evaluate",
    "dm_debug_exception_info",
    "dm_debug_launch",
    "dm_debug_scopes",
    "dm_debug_set_breakpoints",
    "dm_debug_set_exception_breakpoints",
    "dm_debug_set_function_breakpoints",
    "dm_debug_source",
    "dm_debug_stack_trace",
    "dm_debug_stop",
    "dm_debug_threads",
    "dm_debug_variables",
    "dm_debug_wait_for_event",
    "dm_diff_maps",
    "dm_dmi_info",
    "dm_document_symbols",
    "dm_extract_dmi",
    "dm_find_dmi_duplicates",
    "dm_find_implementations",
    "dm_find_on_map",
    "dm_find_references",
    "dm_generate_docs",
    "dm_get_definition",
    "dm_get_proc",
    "dm_get_type",
    "dm_get_var",
    "dm_list_render_passes",
    "dm_list_types",
    "dm_map_info",
    "dm_parse_environment",
    "dm_render_map",
    "dm_render_maps",
    "dm_run",
    "dm_search_context",
    "dm_search_symbols",
    "dm_status",
    "dm_stop",
    "dm_topic",
    "dm_wait_for_output",
    "rift_compile",
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityDisposition {
    Direct,
    McpNative,
    FixedHelper,
    Superseded,
    Excluded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CapabilityRecord {
    pub id: String,
    pub category: String,
    pub upstream_component: String,
    pub disposition: CapabilityDisposition,
    #[serde(default)]
    pub targets: Vec<String>,
    pub platforms: Vec<String>,
    pub verification: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub rationale: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CapabilityRegistry {
    pub schema_version: u32,
    pub spacemandmm_revision: String,
    pub capabilities: Vec<CapabilityRecord>,
}

#[derive(Debug, thiserror::Error)]
pub enum CapabilityRegistryError {
    #[error("checked-in capability registry is invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn capability_registry() -> Result<CapabilityRegistry, CapabilityRegistryError> {
    Ok(serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/spacemandmm-capabilities.json"
    )))?)
}

pub fn validate_capability_registry(registry: &CapabilityRegistry) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let approved_tools: HashSet<_> = APPROVED_TOOL_NAMES.iter().copied().collect();
    let mut identities = HashSet::new();

    if registry.schema_version != 1 {
        errors.push(format!(
            "unsupported capability registry schema {}",
            registry.schema_version
        ));
    }
    if registry.spacemandmm_revision != SPACEMANDMM_REVISION {
        errors.push(format!(
            "capability registry revision {} does not match {}",
            registry.spacemandmm_revision, SPACEMANDMM_REVISION
        ));
    }

    for record in &registry.capabilities {
        if record.id.trim().is_empty() {
            errors.push("capability record has an empty id".to_owned());
        } else if !identities.insert(record.id.as_str()) {
            errors.push(format!("duplicate capability id {}", record.id));
        }
        if record.category.trim().is_empty() {
            errors.push(format!("{} has no category", record.id));
        }
        if record.upstream_component.trim().is_empty() {
            errors.push(format!("{} has no upstream component", record.id));
        }
        if record.verification.trim().is_empty() {
            errors.push(format!("{} has no verification", record.id));
        }
        if record.platforms.is_empty()
            || record
                .platforms
                .iter()
                .any(|platform| !matches!(platform.as_str(), "all" | "windows" | "ubuntu"))
        {
            errors.push(format!("{} has invalid platform coverage", record.id));
        }

        if record.disposition == CapabilityDisposition::Excluded {
            if record
                .rationale
                .as_deref()
                .is_none_or(|rationale| rationale.trim().is_empty())
            {
                errors.push(format!("{} has no exclusion rationale", record.id));
            }
            if !record.targets.is_empty() {
                errors.push(format!("{} is excluded but has tool targets", record.id));
            }
        } else if record.targets.is_empty() {
            errors.push(format!("{} has no implementation target", record.id));
        }

        for target in &record.targets {
            if !approved_tools.contains(target.as_str()) && !target.starts_with("internal:") {
                errors.push(format!("{} has unknown target {}", record.id, target));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
