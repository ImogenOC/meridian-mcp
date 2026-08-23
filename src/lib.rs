pub mod config;
pub mod contracts;
pub mod mcp;
pub mod parameters;
pub mod path_policy;
pub mod project;
pub mod result;
mod search;
pub mod server;
mod source;
pub mod state;
pub mod tools;

pub use config::{CapabilityMode, ServerConfig};
pub use contracts::{
    all_contracts, contracts_for, render_tool_reference, SupportLevel, ToolContract, ToolEffects,
};
pub use path_policy::{PathPolicy, PolicyError};
pub use project::ProjectProfile;
pub use server::MeridianServer;

pub async fn run(config: ServerConfig) -> anyhow::Result<()> {
    mcp::run_server(config).await
}
