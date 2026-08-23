use crate::{MeridianServer, ServerConfig};
use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use serde::Serialize;
use serde_json::Value;

pub use crate::result::{ToolContent, ToolResult};

#[derive(Debug, Serialize, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

pub async fn run_server(config: ServerConfig) -> Result<()> {
    let service = MeridianServer::new(config)?.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
