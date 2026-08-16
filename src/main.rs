mod client;
mod mcp;
mod state;
mod tools;

use anyhow::Result;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn default_rust_log_filter() -> String {
    std::env::var("RUST_LOG").unwrap_or_else(|_| "meridian_mcp=info".into())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr (stdout is for MCP protocol)
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(default_rust_log_filter()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    info!("Starting DreamMaker MCP Server");

    // Run the MCP server
    mcp::run_server().await
}

#[cfg(test)]
mod tests {
    use super::default_rust_log_filter;

    #[test]
    fn default_rust_log_filter_uses_meridian_target() {
        unsafe {
            std::env::remove_var("RUST_LOG");
        }

        assert_eq!(default_rust_log_filter(), "meridian_mcp=info");
    }
}
