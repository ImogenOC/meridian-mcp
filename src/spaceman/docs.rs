use crate::capabilities::SPACEMANDMM_REVISION;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum DocsHelperError {
    #[error(transparent)]
    Manifest(#[from] crate::helper_manifest::ManifestError),
}

pub fn verified_dmdoc_helper(manifest_path: &Path) -> Result<PathBuf, DocsHelperError> {
    let helper = crate::helper_manifest::verified_helper(
        manifest_path,
        crate::helper_manifest::HelperRequest {
            id: "dmdoc",
            platform: std::env::consts::OS,
            target_arch: std::env::consts::ARCH,
            source_revision: SPACEMANDMM_REVISION,
            protocol_version: None,
            byond_version: None,
        },
    )?;
    Ok(helper.path)
}

pub fn optional_verified_dmdoc_helper(
    manifest_path: &Path,
) -> Result<Option<PathBuf>, DocsHelperError> {
    match verified_dmdoc_helper(manifest_path) {
        Ok(path) => Ok(Some(path)),
        Err(DocsHelperError::Manifest(crate::helper_manifest::ManifestError::NoMatch {
            ..
        })) => Ok(None),
        Err(error) => Err(error),
    }
}
