use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityMode {
    Analysis,
    Development,
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    mode: CapabilityMode,
    workspace_roots: Vec<PathBuf>,
    compiler_allowlist: Vec<PathBuf>,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        let mode = std::env::var("MERIDIAN_MCP_MODE").ok();
        let roots = std::env::var_os("MERIDIAN_MCP_ROOTS").ok_or_else(|| {
            anyhow!("MERIDIAN_MCP_ROOTS must contain at least one workspace root")
        })?;
        let compilers = std::env::var_os("MERIDIAN_MCP_COMPILERS");
        Self::from_values(
            mode.as_deref(),
            std::env::split_paths(&roots).collect(),
            compilers
                .as_deref()
                .map(std::env::split_paths)
                .map(Iterator::collect)
                .unwrap_or_default(),
        )
    }

    pub fn from_values(
        mode: Option<&str>,
        workspace_roots: Vec<PathBuf>,
        compiler_allowlist: Vec<PathBuf>,
    ) -> Result<Self> {
        let mode = match mode.unwrap_or("analysis") {
            "analysis" => CapabilityMode::Analysis,
            "development" => CapabilityMode::Development,
            other => return Err(anyhow!("unknown MERIDIAN_MCP_MODE value: {other}")),
        };
        if workspace_roots.is_empty() {
            return Err(anyhow!("at least one workspace root is required"));
        }
        Ok(Self {
            mode,
            workspace_roots: canonicalize_all(workspace_roots, "workspace root")?,
            compiler_allowlist: canonicalize_all(compiler_allowlist, "compiler")?,
        })
    }

    pub fn mode(&self) -> CapabilityMode {
        self.mode
    }
    pub fn workspace_roots(&self) -> &[PathBuf] {
        &self.workspace_roots
    }
    pub fn compiler_allowlist(&self) -> &[PathBuf] {
        &self.compiler_allowlist
    }
}

fn canonicalize_all(paths: Vec<PathBuf>, label: &str) -> Result<Vec<PathBuf>> {
    paths
        .into_iter()
        .map(|path| {
            path.canonicalize()
                .with_context(|| format!("cannot canonicalize {label}: {}", path.display()))
        })
        .collect()
}
