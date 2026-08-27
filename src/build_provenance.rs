use crate::artifact::FileIdentity;
use crate::build_identity::BuildIdentity;
use crate::{PathPolicy, PrivateStateStore};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectBuildIdentity {
    pub root: PathBuf,
    pub repository_identity: String,
    pub head_revision: Option<String>,
    pub dirty: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BuildInputIdentity {
    pub path: PathBuf,
    pub relative_path: String,
    pub role: String,
    pub size: u64,
    pub sha256: String,
}

impl BuildInputIdentity {
    pub fn capture(root: &Path, path: &Path, role: impl Into<String>) -> Result<Self> {
        let root = root.canonicalize()?;
        let identity = FileIdentity::capture(path)?;
        let relative_path =
            normalize_relative(identity.path.strip_prefix(&root).with_context(|| {
                format!("build input is outside project root: {}", path.display())
            })?);
        Ok(Self {
            path: identity.path,
            relative_path,
            role: role.into(),
            size: identity.size,
            sha256: identity.sha256,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildRecord {
    pub schema: u32,
    pub record_id: String,
    pub artifact_key: String,
    pub mcp_build: BuildIdentity,
    pub compiler: FileIdentity,
    pub project: ProjectBuildIdentity,
    pub inputs: Vec<BuildInputIdentity>,
    pub dmb: FileIdentity,
    pub rsc: Option<FileIdentity>,
    pub fixture_manifest_sha256: Option<String>,
    pub created_at_unix_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BuildAttemptOutcome {
    Succeeded,
    Failed { code: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildAttempt {
    pub schema: u32,
    pub attempt_id: String,
    pub artifact_key: String,
    pub outcome: BuildAttemptOutcome,
    pub observed_inputs: Vec<BuildInputIdentity>,
    pub retained_dmb_sha256: Option<String>,
    pub created_at_unix_ms: u128,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceStatus {
    Verified,
    Unverified,
    Stale,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProvenanceReason {
    pub code: String,
    pub message: String,
    pub role: Option<String>,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LaunchDecision {
    pub status: ProvenanceStatus,
    pub allowed: bool,
    pub record_id: Option<String>,
    pub reasons: Vec<ProvenanceReason>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LaunchProvenance {
    pub status: ProvenanceStatus,
    pub build_record_id: Option<String>,
    pub dmb_sha256: String,
    pub warnings: Vec<ProvenanceReason>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ArtifactLocation {
    schema: u32,
    artifact_key: String,
}

pub struct BuildProvenanceStore {
    state: Arc<PrivateStateStore>,
    policy: PathPolicy,
}

impl BuildProvenanceStore {
    pub fn new(state: Arc<PrivateStateStore>, policy: PathPolicy) -> Self {
        Self { state, policy }
    }

    pub fn artifact_key(&self, dmb_path: &Path) -> Result<String> {
        let (project, relative) = self.project_and_relative(dmb_path)?;
        Ok(format!(
            "{:x}",
            Sha256::digest(format!("{}\n{}", project.repository_identity, relative).as_bytes())
        ))
    }

    pub fn project_identity(&self, artifact_path: &Path) -> Result<ProjectBuildIdentity> {
        self.project_and_relative(artifact_path)
            .map(|(project, _)| project)
    }

    pub fn record_success(&self, record: &BuildRecord) -> Result<()> {
        if record.schema != 1 || record.record_id.is_empty() || record.artifact_key.len() != 64 {
            bail!("build record is invalid");
        }
        let current_key = self.artifact_key(&record.dmb.path)?;
        if current_key != record.artifact_key {
            bail!("build record artifact key does not match the current project identity");
        }
        self.state
            .write_json_atomic(&format!("builds/{}.json", record.artifact_key), record)?;
        self.state.write_json_atomic(
            &format!("locations/{}.json", location_key(&record.dmb.path)?),
            &ArtifactLocation {
                schema: 1,
                artifact_key: record.artifact_key.clone(),
            },
        )?;
        Ok(())
    }

    pub fn record_attempt(&self, attempt: &BuildAttempt) -> Result<()> {
        if attempt.schema != 1 || attempt.attempt_id.is_empty() || attempt.artifact_key.len() != 64
        {
            bail!("build attempt is invalid");
        }
        self.state
            .write_json_atomic(&format!("attempts/{}.json", attempt.artifact_key), attempt)?;
        Ok(())
    }

    pub fn evaluate_launch(
        &self,
        dmb_path: &Path,
        require_verified: bool,
    ) -> Result<LaunchDecision> {
        let dmb_path = if dmb_path.exists() {
            self.policy.read_path(dmb_path)?
        } else {
            let parent = dmb_path
                .parent()
                .ok_or_else(|| anyhow!("artifact path has no parent"))?;
            self.policy.read_path(parent)?.join(
                dmb_path
                    .file_name()
                    .ok_or_else(|| anyhow!("artifact path has no file name"))?,
            )
        };
        let location_path = format!("locations/{}.json", location_key(&dmb_path)?);
        let location_file = self.state.root().join(&location_path);
        if !location_file.is_file() {
            return Ok(unverified(require_verified));
        }
        let location: ArtifactLocation = self.state.read_json(&location_path)?;
        if location.schema != 1 || location.artifact_key.len() != 64 {
            bail!("managed artifact location record is invalid");
        }
        let record: BuildRecord = self
            .state
            .read_json(&format!("builds/{}.json", location.artifact_key))?;
        if record.schema != 1 {
            bail!("managed build record schema is unsupported");
        }

        let mut reasons = Vec::new();
        let current_project = self.project_identity(&dmb_path)?;
        if current_project != record.project {
            reasons.push(reason(
                "repository_identity_changed",
                "the current project identity differs from the recorded successful build",
                None,
                Some(current_project.root.clone()),
            ));
        }
        for input in &record.inputs {
            match FileIdentity::capture(&input.path) {
                Ok(current) if current.size == input.size && current.sha256 == input.sha256 => {}
                Ok(_) => reasons.push(reason(
                    "input_changed",
                    "a recorded build input changed",
                    Some(input.role.clone()),
                    Some(input.path.clone()),
                )),
                Err(_) => reasons.push(reason(
                    "input_missing",
                    "a recorded build input is missing or not a regular file",
                    Some(input.role.clone()),
                    Some(input.path.clone()),
                )),
            }
        }
        compare_output(
            &record.dmb,
            "dmb_changed",
            "the managed DMB changed",
            &mut reasons,
        );
        if let Some(rsc) = &record.rsc {
            compare_output(rsc, "rsc_changed", "the managed RSC changed", &mut reasons);
        }

        let attempt_path = format!("attempts/{}.json", record.artifact_key);
        if self.state.root().join(&attempt_path).is_file() {
            let attempt: BuildAttempt = self.state.read_json(&attempt_path)?;
            if attempt.created_at_unix_ms > record.created_at_unix_ms
                && matches!(attempt.outcome, BuildAttemptOutcome::Failed { .. })
            {
                reasons.push(reason(
                    "later_compile_failed",
                    "a compile attempt after the recorded success failed",
                    None,
                    Some(dmb_path.clone()),
                ));
            }
        }

        if reasons.is_empty() {
            Ok(LaunchDecision {
                status: ProvenanceStatus::Verified,
                allowed: true,
                record_id: Some(record.record_id),
                reasons,
            })
        } else {
            Ok(LaunchDecision {
                status: ProvenanceStatus::Stale,
                allowed: false,
                record_id: Some(record.record_id),
                reasons,
            })
        }
    }

    fn project_and_relative(&self, artifact_path: &Path) -> Result<(ProjectBuildIdentity, String)> {
        let artifact = if artifact_path.exists() {
            self.policy.read_path(artifact_path)?
        } else {
            let parent = artifact_path
                .parent()
                .ok_or_else(|| anyhow!("artifact path has no parent"))?;
            let parent = self.policy.read_path(parent)?;
            parent.join(
                artifact_path
                    .file_name()
                    .ok_or_else(|| anyhow!("artifact path has no file name"))?,
            )
        };
        let root = self
            .policy
            .effective_roots()
            .iter()
            .filter(|root| artifact.starts_with(&root.path))
            .max_by_key(|root| root.path.components().count())
            .ok_or_else(|| anyhow!("artifact is outside effective roots"))?;
        let relative = normalize_relative(artifact.strip_prefix(&root.path)?);
        let repository_identity = root
            .repository_identity
            .as_ref()
            .map(|identity| identity.digest.clone())
            .unwrap_or_else(|| {
                format!(
                    "{:x}",
                    Sha256::digest(root.path.to_string_lossy().as_bytes())
                )
            });
        Ok((
            ProjectBuildIdentity {
                root: root.path.clone(),
                repository_identity,
                head_revision: root.head_revision.clone(),
                dirty: root.dirty,
            },
            relative,
        ))
    }
}

fn compare_output(
    recorded: &FileIdentity,
    code: &str,
    message: &str,
    reasons: &mut Vec<ProvenanceReason>,
) {
    if !matches!(
        FileIdentity::capture(&recorded.path),
        Ok(current) if current.size == recorded.size && current.sha256 == recorded.sha256
    ) {
        reasons.push(reason(code, message, None, Some(recorded.path.clone())));
    }
}

fn unverified(require_verified: bool) -> LaunchDecision {
    LaunchDecision {
        status: ProvenanceStatus::Unverified,
        allowed: !require_verified,
        record_id: None,
        reasons: vec![reason(
            "no_build_record",
            "no managed successful build record exists for this artifact",
            None,
            None,
        )],
    }
}

fn reason(
    code: impl Into<String>,
    message: impl Into<String>,
    role: Option<String>,
    path: Option<PathBuf>,
) -> ProvenanceReason {
    ProvenanceReason {
        code: code.into(),
        message: message.into(),
        role,
        path,
    }
}

fn location_key(path: &Path) -> Result<String> {
    let path = if path.exists() {
        path.canonicalize()?
    } else {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow!("artifact path has no parent"))?
            .canonicalize()?;
        parent.join(
            path.file_name()
                .ok_or_else(|| anyhow!("artifact path has no file name"))?,
        )
    };
    Ok(format!(
        "{:x}",
        Sha256::digest(path.to_string_lossy().to_ascii_lowercase().as_bytes())
    ))
}

fn normalize_relative(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
