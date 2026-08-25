use crate::capabilities::{BYOND_TRACY_REVISION, TRACY_PROTOCOL_VERSION, TRACY_REVISION};
use crate::helper_manifest::{verified_helper, HelperRequest, ManifestError, VerifiedHelper};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct TracyInstallation {
    pub helper: VerifiedHelper,
    pub hook: VerifiedHelper,
}

impl TracyInstallation {
    pub fn validate(manifest_path: &Path, byond_version: &str) -> Result<Self, ManifestError> {
        let helper = verified_helper(
            manifest_path,
            HelperRequest {
                id: "tracy-server-helper",
                platform: std::env::consts::OS,
                target_arch: std::env::consts::ARCH,
                source_revision: TRACY_REVISION,
                protocol_version: Some(TRACY_PROTOCOL_VERSION),
                byond_version: None,
            },
        )?;
        let hook = verified_helper(
            manifest_path,
            HelperRequest {
                id: "byond-tracy",
                platform: std::env::consts::OS,
                target_arch: "x86",
                source_revision: BYOND_TRACY_REVISION,
                protocol_version: Some(TRACY_PROTOCOL_VERSION),
                byond_version: Some(byond_version),
            },
        )?;
        Ok(Self { helper, hook })
    }
}
