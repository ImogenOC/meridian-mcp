use crate::capabilities::{BYOND_TRACY_REVISION, TRACY_PROTOCOL_VERSION, TRACY_REVISION};
use crate::helper_manifest::{
    verified_helper, version_in_range, HelperRequest, ManifestError, VerifiedHelper,
};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct TracyInstallation {
    pub helper: VerifiedHelper,
    pub hook: VerifiedHelper,
}

impl TracyInstallation {
    pub fn validate(manifest_path: &Path) -> Result<Self, ManifestError> {
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
                byond_version: None,
            },
        )?;
        Ok(Self { helper, hook })
    }

    pub fn validate_byond_version(&self, byond_version: &str) -> Result<String, ManifestError> {
        let normalized = if byond_version.contains('.') {
            byond_version.to_owned()
        } else {
            let build = byond_version.parse::<u32>().ok();
            let minimum_major = self
                .hook
                .byond_min_version
                .as_deref()
                .and_then(|version| version.split_once('.'))
                .map(|(major, _)| major);
            let maximum_major = self
                .hook
                .byond_max_version
                .as_deref()
                .and_then(|version| version.split_once('.'))
                .map(|(major, _)| major);
            match (build, minimum_major, maximum_major) {
                (Some(build), Some(minimum_major), Some(maximum_major))
                    if minimum_major == maximum_major =>
                {
                    format!("{minimum_major}.{build}")
                }
                _ => byond_version.to_owned(),
            }
        };
        if version_in_range(
            &normalized,
            self.hook.byond_min_version.as_deref(),
            self.hook.byond_max_version.as_deref(),
        ) {
            Ok(normalized)
        } else {
            Err(ManifestError::ByondVersion {
                id: self.hook.id.clone(),
                version: byond_version.to_owned(),
            })
        }
    }
}
