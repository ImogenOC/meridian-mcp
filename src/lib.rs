pub mod analysis_snapshot;
pub mod artifact;
pub mod atomic_output;
pub mod build_identity;
pub mod capabilities;
pub mod config;
pub mod contracts;
pub mod helper_manifest;
pub mod index;
pub mod limits;
pub mod mcp;
pub mod network_audit;
pub mod parameters;
pub mod path_policy;
pub mod process;
pub mod process_environment;
pub mod process_metrics;
pub mod project;
pub mod result;
mod search;
pub mod server;
mod source;
pub mod spaceman;
pub mod state;
pub mod tools;
pub mod tracy;
pub mod tracy_artifact;
pub mod tracy_collector;
pub mod tracy_experiment;
pub mod tracy_protocol;
pub mod tracy_statistics;
pub mod workspace_integrity;

pub use config::{CapabilityMode, DebuggerAccess, RiftBuildAccess, ServerConfig, TracyAccess};
pub use contracts::{
    all_contracts, contracts_for, contracts_for_configuration, render_tool_reference, SupportLevel,
    ToolContract, ToolEffects,
};
pub use parameters::RiftNetworkMode;
pub use path_policy::{PathPolicy, PolicyError};
pub use project::ProjectProfile;
pub use server::MeridianServer;
pub use tools::rift::BuildEvidence;

pub async fn run(config: ServerConfig) -> anyhow::Result<()> {
    mcp::run_server(config).await
}
