use crate::capabilities::SPACEMANDMM_REVISION;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum DocsHelperError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("no dmdoc helper for this platform in manifest")]
    Platform,
    #[error("dmdoc helper revision mismatch")]
    Revision,
    #[error("dmdoc helper checksum mismatch")]
    Checksum,
}

#[derive(Deserialize)]
struct Manifest {
    helpers: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    platform: String,
    path: PathBuf,
    sha256: String,
    source_revision: String,
}

pub fn verified_dmdoc_helper(manifest_path: &Path) -> Result<PathBuf, DocsHelperError> {
    let bytes = std::fs::read(manifest_path)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let entry = manifest
        .helpers
        .into_iter()
        .find(|entry| entry.platform == platform)
        .ok_or(DocsHelperError::Platform)?;
    if entry.source_revision != SPACEMANDMM_REVISION {
        return Err(DocsHelperError::Revision);
    }
    let path = manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(entry.path)
        .canonicalize()?;
    let actual = format!("{:x}", Sha256::digest(std::fs::read(&path)?));
    if !actual.eq_ignore_ascii_case(&entry.sha256) {
        return Err(DocsHelperError::Checksum);
    }
    Ok(path)
}
