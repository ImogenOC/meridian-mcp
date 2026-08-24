mod analysis;
mod compile;
mod map;
mod parse;
pub mod rift;
mod runtime;
mod search;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::mcp::{ToolDefinition, ToolResult};
use crate::state::ServerState;
use crate::{contracts_for_configuration, CapabilityMode, PathPolicy, RiftBuildAccess};

#[derive(Clone)]
pub struct ToolExecutionContext {
    mode: CapabilityMode,
    policy: PathPolicy,
    rift_build: RiftBuildAccess,
}

impl ToolExecutionContext {
    pub fn new(mode: CapabilityMode, policy: PathPolicy) -> Self {
        Self::with_rift_build(mode, policy, RiftBuildAccess::Disabled)
    }

    pub fn with_rift_build(
        mode: CapabilityMode,
        policy: PathPolicy,
        rift_build: RiftBuildAccess,
    ) -> Self {
        Self {
            mode,
            policy,
            rift_build,
        }
    }

    pub fn mode(&self) -> CapabilityMode {
        self.mode
    }

    pub fn rift_build_access(&self) -> RiftBuildAccess {
        self.rift_build
    }

    pub(crate) fn policy(&self) -> &PathPolicy {
        &self.policy
    }
}

/// Get all available tool definitions
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = Vec::new();

    // Parsing tools
    tools.push(ToolDefinition {
        name: "dm_parse_environment".to_string(),
        description: "Parse a DreamMaker environment (.dme file) and cache the object tree. This must be called before using other analysis tools.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "dme_path": {
                    "type": "string",
                    "description": "Path to the .dme environment file"
                }
            },
            "required": ["dme_path"]
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_get_type".to_string(),
        description: "Get information about a type including its variables, procs, parent type, and children.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "type_path": {
                    "type": "string",
                    "description": "The type path (e.g., '/obj/item', '/mob/living')"
                }
            },
            "required": ["type_path"]
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_get_proc".to_string(),
        description: "Get detailed information about a procedure including parameters, body location, and documentation.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "type_path": {
                    "type": "string",
                    "description": "The type path containing the proc"
                },
                "proc_name": {
                    "type": "string",
                    "description": "Name of the procedure"
                }
            },
            "required": ["type_path", "proc_name"]
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_get_var".to_string(),
        description: "Get detailed information about a variable including its type, initial value, and documentation.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "type_path": {
                    "type": "string",
                    "description": "The type path containing the variable"
                },
                "var_name": {
                    "type": "string",
                    "description": "Name of the variable"
                }
            },
            "required": ["type_path", "var_name"]
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_list_types".to_string(),
        description: "List all types in the object tree, optionally filtered by a path prefix."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "prefix": {
                    "type": "string",
                    "description": "Optional path prefix to filter types (e.g., '/obj')"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum depth to traverse (default: unlimited)"
                }
            }
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_search_symbols".to_string(),
        description: "Search for types, procs, or variables by name pattern.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (supports partial matches)"
                },
                "kind": {
                    "type": "string",
                    "enum": ["type", "proc", "var", "all"],
                    "description": "Kind of symbol to search for (default: all)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 50)"
                }
            },
            "required": ["query"]
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_search_context".to_string(),
        description: "Search parsed DreamMaker types, procs, variables, documentation, and source using deterministic ranked retrieval. Call dm_parse_environment first, then verify results with the exact inspection tools.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural-language behavior, exact symbol, or identifier terms to find"
                },
                "kind": {
                    "type": "string",
                    "enum": ["all", "type", "proc", "var"],
                    "description": "Optional symbol kind filter (default: all)"
                },
                "type_prefix": {
                    "type": "string",
                    "description": "Optional canonical type-path prefix, such as /turf/open"
                },
                "file_filter": {
                    "type": "string",
                    "description": "Optional case-insensitive source-path substring"
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 50,
                    "description": "Maximum ranked results (default: 10)"
                },
                "include_source": {
                    "type": "boolean",
                    "description": "Include bounded source excerpts (default: true)"
                },
                "max_source_lines": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 200,
                    "description": "Maximum source lines per result (default: 40)"
                }
            },
            "required": ["query"]
        }),
    });

    // Analysis tools
    tools.push(ToolDefinition {
        name: "dm_check_errors".to_string(),
        description: "Run the type checker and return all diagnostics (errors and warnings)."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Optional: only show errors from this file"
                }
            }
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_get_definition".to_string(),
        description: "Get the source location where a symbol is defined.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "type_path": {
                    "type": "string",
                    "description": "The type path"
                },
                "member_name": {
                    "type": "string",
                    "description": "Optional: name of var or proc to find definition of"
                }
            },
            "required": ["type_path"]
        }),
    });

    // Compile tool
    tools.push(ToolDefinition {
        name: "dm_compile".to_string(),
        description: "Compile the DreamMaker environment using the DM compiler. Returns compiler output and any errors.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "dme_path": {
                    "type": "string",
                    "description": "Path to the .dme file to compile"
                },
                "compiler_path": {
                    "type": "string",
                    "description": "Optional path to the DreamMaker executable"
                },
                "working_directory": {
                    "type": "string",
                    "description": "Optional working directory for the compiler process"
                },
                "defines": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional preprocessor defines, with or without the -D prefix"
                },
                "timeout_ms": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Compiler timeout in milliseconds (default: 600000, capped at 1800000)"
                },
                "idle_timeout_ms": {
                    "type": "integer",
                    "minimum": 1000,
                    "description": "Fail if DreamMaker produces no output and consumes no CPU for this long (default: 45000, capped at 900000)"
                },
                "capture_network": {
                    "type": "boolean",
                    "description": "Request best-effort endpoint observation (default: false)"
                }
            },
            "required": ["dme_path"]
        }),
    });

    tools.push(ToolDefinition {
        name: "rift_compile".to_string(),
        description: "Run Meridian-Rift's contained RIFT_BUILD.cmd full-build gate.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "network_mode": {
                    "type": "string",
                    "enum": ["offline", "allow"],
                    "description": "Dependency network mode (default: offline)"
                },
                "timeout_ms": {
                    "type": "integer",
                    "minimum": 1000,
                    "maximum": 1800000,
                    "description": "Wall timeout in milliseconds (default: 1800000)"
                },
                "idle_timeout_ms": {
                    "type": "integer",
                    "minimum": 1000,
                    "maximum": 900000,
                    "description": "No-output and no-CPU timeout in milliseconds (default: 120000)"
                },
                "capture_network": {
                    "type": "boolean",
                    "description": "Request best-effort endpoint observation (default: false)"
                },
                "force_rebuild": {
                    "type": "boolean",
                    "description": "Remove only canonical root build artifacts before building (default: false)"
                }
            }
        }),
    });

    // Map tools
    tools.push(ToolDefinition {
        name: "dm_render_map".to_string(),
        description: "Render a map file (.dmm) to a PNG image.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "dmm_path": {
                    "type": "string",
                    "description": "Path to the .dmm map file"
                },
                "z_level": {
                    "type": "integer",
                    "description": "Z-level to render (default: 1)"
                },
                "output_path": {
                    "type": "string",
                    "description": "Path to save the PNG (default: same as dmm with .png extension)"
                }
                ,
                "overwrite": {
                    "type": "boolean",
                    "description": "Replace an existing output only when explicitly true (default: false)"
                }
            },
            "required": ["dmm_path"]
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_map_info".to_string(),
        description:
            "Get information about a map file including dimensions, z-levels, and area statistics."
                .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "dmm_path": {
                    "type": "string",
                    "description": "Path to the .dmm map file"
                }
            },
            "required": ["dmm_path"]
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_find_on_map".to_string(),
        description: "Find all instances of a type on a map.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "dmm_path": {
                    "type": "string",
                    "description": "Path to the .dmm map file"
                },
                "type_path": {
                    "type": "string",
                    "description": "Type path to search for (e.g., '/obj/machinery/door')"
                }
            },
            "required": ["dmm_path", "type_path"]
        }),
    });

    // Runtime tools
    tools.push(ToolDefinition {
        name: "dm_run".to_string(),
        description:
            "Start DreamDaemon with a compiled .dmb file. Returns the process ID and port."
                .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "dmb_path": {
                    "type": "string",
                    "description": "Path to the compiled .dmb file"
                },
                "port": {
                    "type": "integer",
                    "description": "Port to run the server on (default: 1337)"
                },
                "working_directory": {
                    "type": "string",
                    "description": "Optional working directory used to resolve a relative DMB path and run DreamDaemon"
                },
                "daemon_args": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional additional DreamDaemon arguments, appended after the standard trusted/logging arguments"
                },
                "wait_for": {
                    "type": "string",
                    "description": "Optional output marker to wait for before returning"
                },
                "wait_regex": {
                    "type": "boolean",
                    "description": "Interpret wait_for as a regular expression (default: false)"
                },
                "startup_timeout_ms": {
                    "type": "integer",
                    "description": "Maximum wait for wait_for in milliseconds (default: 30000)"
                }
            },
            "required": ["dmb_path"]
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_wait_for_output".to_string(),
        description:
            "Wait for a literal or regular-expression marker in captured DreamDaemon output."
                .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Output text or regular expression to wait for"
                },
                "regex": {
                    "type": "boolean",
                    "description": "Interpret pattern as a regular expression (default: false)"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Maximum wait in milliseconds (default: 30000, capped at 300000)"
                }
            },
            "required": ["pattern"]
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_stop".to_string(),
        description: "Stop the currently running DreamDaemon instance.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_status".to_string(),
        description: "Get the status of the running game (port, PID, whether it's running)."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    });

    tools.push(ToolDefinition {
        name: "dm_topic".to_string(),
        description: "Send a Topic() call to the running game server. Use this to communicate with debug handlers in the game.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "topic": {
                    "type": "string",
                    "description": "The topic string to send (e.g., '?debug_screenshot' or '?debug_click=10,20')"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default: 5000)"
                }
            },
            "required": ["topic"]
        }),
    });

    tools
}

pub fn get_tool_definitions_for(
    mode: CapabilityMode,
    rift_build: RiftBuildAccess,
) -> Vec<ToolDefinition> {
    let active: std::collections::HashSet<_> = contracts_for_configuration(mode, rift_build)
        .into_iter()
        .map(|contract| contract.name)
        .collect();
    get_tool_definitions()
        .into_iter()
        .filter(|definition| active.contains(definition.name.as_str()))
        .collect()
}

/// Call a tool by name with the given arguments
pub async fn call_tool(
    context: &ToolExecutionContext,
    state: &mut ServerState,
    name: &str,
    mut args: Value,
) -> Result<ToolResult> {
    if name == "rift_compile" && !cfg!(windows) {
        return Ok(policy_error(
            "unsupported_platform",
            "rift_compile is supported only on Windows".to_string(),
            None,
        ));
    }
    if !contracts_for_configuration(context.mode, context.rift_build)
        .iter()
        .any(|contract| contract.name == name)
    {
        return Ok(policy_error(
            "tool_not_available",
            format!("{name} is not available in {:?} mode", context.mode),
            None,
        ));
    }
    if let Err(error) = contain_arguments(&context.policy, name, &mut args) {
        return Ok(policy_error(
            error.code(),
            error.to_string(),
            Some(error.path()),
        ));
    }
    match name {
        // Parsing tools
        "dm_parse_environment" => parse::parse_environment(state, args).await,
        "dm_get_type" => parse::get_type(state, args).await,
        "dm_get_proc" => parse::get_proc(state, args).await,
        "dm_get_var" => parse::get_var(state, args).await,
        "dm_list_types" => parse::list_types(state, args).await,
        "dm_search_symbols" => parse::search_symbols(state, args).await,
        "dm_search_context" => search::search_context(state, args).await,

        // Analysis tools
        "dm_check_errors" => analysis::check_errors(state, args).await,
        "dm_get_definition" => analysis::get_definition(state, args).await,

        // Compile tool
        "dm_compile" => compile::compile(args).await,
        "rift_compile" => rift::compile(context, state, args).await,

        // Map tools
        "dm_render_map" => map::render_map(state, args).await,
        "dm_map_info" => map::map_info(args).await,
        "dm_find_on_map" => map::find_on_map(args).await,

        // Runtime tools
        "dm_run" => runtime::run(state, args).await,
        "dm_wait_for_output" => runtime::wait_for_output(state, args).await,
        "dm_stop" => runtime::stop(state, args).await,
        "dm_status" => runtime::status(state, args).await,
        "dm_topic" => runtime::topic(state, args).await,
        _ => Err(anyhow!("Unknown tool: {name}")),
    }
}

fn contain_arguments(
    policy: &PathPolicy,
    name: &str,
    args: &mut Value,
) -> std::result::Result<(), crate::PolicyError> {
    match name {
        "dm_parse_environment" | "dm_compile" => {
            canonical_argument(policy, args, "dme_path", false)?
        }
        "dm_render_map" | "dm_map_info" | "dm_find_on_map" => {
            canonical_argument(policy, args, "dmm_path", false)?
        }
        "dm_run" => canonical_argument(policy, args, "dmb_path", true)?,
        _ => {}
    }
    if name == "dm_compile" {
        canonical_optional_argument(policy, args, "working_directory")?;
        if let Some(path) = args.get("compiler_path").and_then(Value::as_str) {
            let path = policy.executable(path)?;
            args["compiler_path"] = Value::String(path.display().to_string());
        }
    }
    if name == "dm_run" {
        canonical_optional_argument(policy, args, "working_directory")?;
    }
    if name == "dm_render_map" {
        let output = args
            .get("output_path")
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(args["dmm_path"].as_str().unwrap()).with_extension("png")
            });
        let overwrite = args
            .get("overwrite")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let output = policy.output_path(output, overwrite)?;
        args["output_path"] = Value::String(output.display().to_string());
    }
    Ok(())
}

fn canonical_argument(
    policy: &PathPolicy,
    args: &mut Value,
    key: &str,
    runtime: bool,
) -> std::result::Result<(), crate::PolicyError> {
    if let Some(path) = args.get(key).and_then(Value::as_str) {
        let path = if runtime {
            policy.runtime_dmb(path)?
        } else {
            policy.read_path(path)?
        };
        args[key] = Value::String(path.display().to_string());
    }
    Ok(())
}

fn canonical_optional_argument(
    policy: &PathPolicy,
    args: &mut Value,
    key: &str,
) -> std::result::Result<(), crate::PolicyError> {
    canonical_argument(policy, args, key, false)
}

fn policy_error(code: &str, message: String, path: Option<&std::path::Path>) -> ToolResult {
    ToolResult::error(
        json!({
            "code": code,
            "message": message,
            "path": path.map(|path| path.display().to_string())
        })
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::get_tool_definitions;

    #[test]
    fn tool_definitions_use_supported_name_prefixes() {
        let definitions = get_tool_definitions();

        assert!(!definitions.is_empty());
        assert!(definitions
            .iter()
            .all(|tool| tool.name.starts_with("dm_") || tool.name == "rift_compile"));
    }

    #[test]
    fn context_search_schema_requires_a_query_and_exposes_filters() {
        let definitions = get_tool_definitions();
        let search = definitions
            .iter()
            .find(|tool| tool.name == "dm_search_context")
            .expect("context search tool should be registered");

        assert_eq!(
            search.input_schema["required"],
            serde_json::json!(["query"])
        );
        assert_eq!(
            search.input_schema["properties"]["kind"]["enum"],
            serde_json::json!(["all", "type", "proc", "var"])
        );
        assert_eq!(search.input_schema["properties"]["limit"]["maximum"], 50);
        assert_eq!(
            search.input_schema["properties"]["max_source_lines"]["maximum"],
            200
        );
    }
}
