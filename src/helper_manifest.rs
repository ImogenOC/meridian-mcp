use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

pub const HELPER_MANIFEST_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug)]
pub struct HelperRequest<'a> {
    pub id: &'a str,
    pub platform: &'a str,
    pub target_arch: &'a str,
    pub source_revision: &'a str,
    pub protocol_version: Option<u32>,
    pub byond_version: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedHelper {
    pub id: String,
    pub path: PathBuf,
    pub sha256: String,
    pub source_revision: String,
    pub protocol_version: Option<u32>,
    pub byond_min_version: Option<String>,
    pub byond_max_version: Option<String>,
    pub patch_sha256: Option<String>,
    pub patches: Vec<OwnedPatchIdentity>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, serde::Serialize)]
pub struct OwnedPatchIdentity {
    pub name: String,
    pub patch_sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("unsupported helper manifest schema {0}")]
    Schema(u32),
    #[error("helper manifest contains duplicate identity {id}/{platform}/{target_arch}")]
    DuplicateIdentity {
        id: String,
        platform: String,
        target_arch: String,
    },
    #[error("no helper matches {id}/{platform}/{target_arch}")]
    NoMatch {
        id: String,
        platform: String,
        target_arch: String,
    },
    #[error("helper {id} revision mismatch: expected {expected}, found {actual}")]
    Revision {
        id: String,
        expected: String,
        actual: String,
    },
    #[error("helper {id} protocol mismatch: expected {expected}, found {actual:?}")]
    Protocol {
        id: String,
        expected: u32,
        actual: Option<u32>,
    },
    #[error("helper {id} does not support BYOND {version}")]
    ByondVersion { id: String, version: String },
    #[error("helper {id} has an invalid SHA-256 value")]
    InvalidChecksum { id: String },
    #[error("helper {id} checksum mismatch")]
    Checksum { id: String },
    #[error("helper {id} path must be relative to the manifest")]
    AbsolutePath { id: String },
    #[error("helper {id} resolves outside the manifest directory")]
    OutsideManifestRoot { id: String },
}

#[derive(Deserialize)]
struct RawManifest {
    schema_version: u32,
    helpers: Vec<RawEntry>,
}

#[derive(Clone, Deserialize)]
struct RawEntry {
    #[serde(default)]
    id: Option<String>,
    platform: String,
    #[serde(default)]
    target_arch: Option<String>,
    path: PathBuf,
    sha256: String,
    source_revision: String,
    #[serde(default)]
    protocol_version: Option<u32>,
    #[serde(default)]
    byond_min_version: Option<String>,
    #[serde(default)]
    byond_max_version: Option<String>,
    #[serde(default)]
    patch_sha256: Option<String>,
    #[serde(default)]
    patches: Vec<OwnedPatchIdentity>,
}

#[derive(Clone)]
struct Entry {
    id: String,
    platform: String,
    target_arch: String,
    path: PathBuf,
    sha256: String,
    source_revision: String,
    protocol_version: Option<u32>,
    byond_min_version: Option<String>,
    byond_max_version: Option<String>,
    patch_sha256: Option<String>,
    patches: Vec<OwnedPatchIdentity>,
}

pub fn verified_helper(
    manifest_path: &Path,
    request: HelperRequest<'_>,
) -> Result<VerifiedHelper, ManifestError> {
    let bytes = std::fs::read(manifest_path)?;
    let raw: RawManifest = serde_json::from_slice(&bytes)?;
    let entries = normalize_manifest(raw)?;
    validate_unique_identities(&entries)?;

    let entry = entries
        .into_iter()
        .find(|entry| {
            entry.id == request.id
                && entry.platform == request.platform
                && entry.target_arch == request.target_arch
        })
        .ok_or_else(|| ManifestError::NoMatch {
            id: request.id.to_owned(),
            platform: request.platform.to_owned(),
            target_arch: request.target_arch.to_owned(),
        })?;

    if entry.source_revision != request.source_revision {
        return Err(ManifestError::Revision {
            id: entry.id,
            expected: request.source_revision.to_owned(),
            actual: entry.source_revision,
        });
    }
    if let Some(expected) = request.protocol_version {
        if entry.protocol_version != Some(expected) {
            return Err(ManifestError::Protocol {
                id: entry.id,
                expected,
                actual: entry.protocol_version,
            });
        }
    }
    if let Some(version) = request.byond_version {
        if !version_in_range(
            version,
            entry.byond_min_version.as_deref(),
            entry.byond_max_version.as_deref(),
        ) {
            return Err(ManifestError::ByondVersion {
                id: entry.id,
                version: version.to_owned(),
            });
        }
    }
    if entry.sha256.len() != 64 || !entry.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ManifestError::InvalidChecksum { id: entry.id });
    }
    for checksum in entry
        .patch_sha256
        .iter()
        .chain(entry.patches.iter().map(|patch| &patch.patch_sha256))
    {
        if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ManifestError::InvalidChecksum {
                id: entry.id.clone(),
            });
        }
    }
    if entry.path.is_absolute()
        || entry
            .path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(ManifestError::OutsideManifestRoot { id: entry.id });
    }

    let manifest_root = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()?;
    let path = manifest_root.join(&entry.path).canonicalize()?;
    if !path.starts_with(&manifest_root) {
        return Err(ManifestError::OutsideManifestRoot { id: entry.id });
    }
    let actual = format!("{:x}", Sha256::digest(std::fs::read(&path)?));
    if !actual.eq_ignore_ascii_case(&entry.sha256) {
        return Err(ManifestError::Checksum { id: entry.id });
    }

    Ok(VerifiedHelper {
        id: entry.id,
        path,
        sha256: entry.sha256.to_ascii_lowercase(),
        source_revision: entry.source_revision,
        protocol_version: entry.protocol_version,
        byond_min_version: entry.byond_min_version,
        byond_max_version: entry.byond_max_version,
        patch_sha256: entry.patch_sha256,
        patches: entry.patches,
    })
}

fn normalize_manifest(raw: RawManifest) -> Result<Vec<Entry>, ManifestError> {
    match raw.schema_version {
        1 => Ok(raw
            .helpers
            .into_iter()
            .map(|entry| {
                let (platform, target_arch) = entry
                    .platform
                    .rsplit_once('-')
                    .map(|(platform, arch)| (platform.to_owned(), arch.to_owned()))
                    .unwrap_or_else(|| (entry.platform, String::new()));
                Entry {
                    id: "dmdoc".to_owned(),
                    platform,
                    target_arch,
                    path: entry.path,
                    sha256: entry.sha256,
                    source_revision: entry.source_revision,
                    protocol_version: None,
                    byond_min_version: None,
                    byond_max_version: None,
                    patch_sha256: None,
                    patches: Vec::new(),
                }
            })
            .collect()),
        HELPER_MANIFEST_SCHEMA_VERSION => Ok(raw
            .helpers
            .into_iter()
            .map(|entry| Entry {
                id: entry.id.unwrap_or_default(),
                platform: entry.platform,
                target_arch: entry.target_arch.unwrap_or_default(),
                path: entry.path,
                sha256: entry.sha256,
                source_revision: entry.source_revision,
                protocol_version: entry.protocol_version,
                byond_min_version: entry.byond_min_version,
                byond_max_version: entry.byond_max_version,
                patch_sha256: entry.patch_sha256,
                patches: entry.patches,
            })
            .collect()),
        schema => Err(ManifestError::Schema(schema)),
    }
}

fn validate_unique_identities(entries: &[Entry]) -> Result<(), ManifestError> {
    let mut identities = HashSet::new();
    for entry in entries {
        if !identities.insert((&entry.id, &entry.platform, &entry.target_arch)) {
            return Err(ManifestError::DuplicateIdentity {
                id: entry.id.clone(),
                platform: entry.platform.clone(),
                target_arch: entry.target_arch.clone(),
            });
        }
    }
    Ok(())
}

pub(crate) fn version_in_range(
    version: &str,
    minimum: Option<&str>,
    maximum: Option<&str>,
) -> bool {
    let Some(version) = parse_version(version) else {
        return false;
    };
    if minimum
        .and_then(parse_version)
        .is_some_and(|minimum| version < minimum)
    {
        return false;
    }
    if maximum
        .and_then(parse_version)
        .is_some_and(|maximum| version > maximum)
    {
        return false;
    }
    true
}

fn parse_version(value: &str) -> Option<Vec<u32>> {
    value
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()
        .ok()
}
