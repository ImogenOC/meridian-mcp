use crate::mcp::ToolDefinition;
use crate::result::{DomainContent, DomainToolResult};
use crate::state::ServerState;
use crate::tools::{self, ToolExecutionContext};
use crate::{PathPolicy, ServerConfig};
use anyhow::Result;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool, ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct MeridianServer {
    config: Arc<ServerConfig>,
    execution: ToolExecutionContext,
    state: Arc<ServerState>,
}

impl MeridianServer {
    pub fn new(config: ServerConfig) -> Result<Self> {
        let debugger = (config.debugger_access() == crate::DebuggerAccess::Auxtools)
            .then(|| crate::spaceman::debugger::validate_installation(config.compiler_allowlist()))
            .transpose()?;
        let policy = PathPolicy::new(
            config.workspace_roots().to_vec(),
            config.compiler_allowlist().to_vec(),
        )?;
        let dmdoc_helper = config
            .helper_manifest()
            .map(crate::spaceman::docs::optional_verified_dmdoc_helper)
            .transpose()?
            .flatten();
        let tracy = (config.tracy_access() == crate::TracyAccess::Byond)
            .then(|| {
                crate::tracy::TracyInstallation::validate(
                    config
                        .helper_manifest()
                        .expect("Tracy config requires a manifest"),
                    "516.1687",
                )
            })
            .transpose()?;
        let execution = ToolExecutionContext::with_features(
            config.mode(),
            policy,
            config.rift_build_access(),
            dmdoc_helper,
            debugger,
            tracy,
        );
        Ok(Self {
            config: Arc::new(config),
            execution,
            state: Arc::new(ServerState::new()),
        })
    }

    pub fn tool_names(&self) -> Vec<String> {
        tools::get_tool_definitions_for_runtime(
            self.config.mode(),
            self.config.rift_build_access(),
            self.execution.dmdoc_helper().is_some(),
            self.execution.debugger().is_some(),
            self.execution.tracy().is_some(),
        )
        .into_iter()
        .map(|definition| definition.name)
        .collect()
    }

    fn tools(&self) -> Vec<Tool> {
        tools::get_tool_definitions_for_runtime(
            self.config.mode(),
            self.config.rift_build_access(),
            self.execution.dmdoc_helper().is_some(),
            self.execution.debugger().is_some(),
            self.execution.tracy().is_some(),
        )
        .into_iter()
        .map(|definition| to_sdk_tool(definition, self.config.rift_build_access()))
        .collect()
    }
}

impl ServerHandler for MeridianServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
			.with_server_info(Implementation::new("meridian-mcp", env!("CARGO_PKG_VERSION")))
			.with_instructions("Call dm_parse_environment before DreamMaker analysis. Use dm_search_context for discovery, verify exact symbols with inspection tools, and reparse after changes. MCP analysis does not replace repository builds.")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.tools()))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools().into_iter().find(|tool| tool.name == name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let result = tools::call_tool(
            &self.execution,
            self.state.as_ref(),
            &request.name,
            arguments,
        )
        .await
        .unwrap_or_else(|error| DomainToolResult::error(error.to_string()));
        Ok(CallToolResponse::Complete(to_sdk_result(result)))
    }
}

fn to_sdk_tool(definition: ToolDefinition, rift_build: crate::RiftBuildAccess) -> Tool {
    let input_schema = definition
        .input_schema
        .as_object()
        .cloned()
        .unwrap_or_default();
    let contract = crate::all_contracts()
        .iter()
        .find(|contract| contract.name == definition.name);
    let annotations = contract.map(|contract| {
        let external_network =
            contract.effects.network_external && rift_build == crate::RiftBuildAccess::Network;
        ToolAnnotations::new()
            .read_only(
                !contract.effects.writes_files
                    && !contract.effects.spawns_process
                    && !contract.effects.network_loopback
                    && !external_network,
            )
            .destructive(false)
            .open_world(external_network)
    });
    let mut tool = Tool::new(definition.name, definition.description, input_schema);
    tool.annotations = annotations;
    tool
}

fn to_sdk_result(result: DomainToolResult) -> CallToolResult {
    let content = result
        .content
        .into_iter()
        .map(|content| match content {
            DomainContent::Text { text } => ContentBlock::text(text),
        })
        .collect();
    if result.is_error == Some(true) {
        CallToolResult::error(content)
    } else {
        CallToolResult::success(content)
    }
}
