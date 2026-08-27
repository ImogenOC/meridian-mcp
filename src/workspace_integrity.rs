use crate::atomic_output::{write_atomic, AtomicOutputError};
use crate::PathPolicy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_INTEGRITY_ENTRIES: usize = 250_000;
const MAX_INTEGRITY_BYTES: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct IntegrityBaseline {
    root: PathBuf,
    records: BTreeMap<String, String>,
    pub digest: String,
    pub preexisting_changes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegrityCheckpoint {
    pub action: String,
    pub baseline_digest: String,
    pub current_digest: String,
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
    pub owned_paths: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityJournalStatus {
    Active,
    Finalized,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct IntegrityJournalDocument {
    schema: u32,
    journal_id: String,
    status: IntegrityJournalStatus,
    baseline_digest: String,
    preexisting_change_count: usize,
    last_action: String,
    checkpoints: Vec<IntegrityCheckpoint>,
}

#[derive(Clone, Debug)]
pub struct IntegrityJournal {
    path: PathBuf,
    document: IntegrityJournalDocument,
}

#[derive(Clone, Debug, Serialize)]
pub struct IntegrityJournalSummary {
    pub journal_id: String,
    pub status: IntegrityJournalStatus,
    pub last_action: String,
    pub checkpoint_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum IntegrityError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("integrity scope exceeds the fixed entry or byte limit")]
    ScopeTooLarge,
    #[error("workspace integrity violation: {0:?}")]
    Violation(Vec<String>),
    #[error("an unfinished Tracy integrity journal requires recovery after {last_action}")]
    RecoveryRequired { last_action: String },
    #[error(transparent)]
    Atomic(#[from] AtomicOutputError),
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
}

impl IntegrityJournal {
    pub fn create(
        policy: &PathPolicy,
        evidence_directory: &Path,
        baseline: &IntegrityBaseline,
    ) -> Result<Self, IntegrityError> {
        let path = evidence_directory.join(".meridian-tracy-session.json");
        let overwrite = if path.is_file() {
            let existing: IntegrityJournalDocument =
                serde_json::from_slice(&std::fs::read(&path)?)?;
            if existing.status != IntegrityJournalStatus::Finalized {
                return Err(IntegrityError::RecoveryRequired {
                    last_action: existing.last_action,
                });
            }
            true
        } else {
            false
        };
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|error| std::io::Error::other(error.to_string()))?;
        let document = IntegrityJournalDocument {
            schema: 1,
            journal_id: random.iter().map(|byte| format!("{byte:02x}")).collect(),
            status: IntegrityJournalStatus::Active,
            baseline_digest: baseline.digest.clone(),
            preexisting_change_count: baseline.preexisting_changes.len(),
            last_action: "baseline_captured".to_owned(),
            checkpoints: Vec::new(),
        };
        let path = persist_journal(policy, &path, overwrite, &document)?;
        Ok(Self { path, document })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn status(&self) -> IntegrityJournalStatus {
        self.document.status
    }

    pub fn summary(&self) -> IntegrityJournalSummary {
        IntegrityJournalSummary {
            journal_id: self.document.journal_id.clone(),
            status: self.document.status,
            last_action: self.document.last_action.clone(),
            checkpoint_count: self.document.checkpoints.len(),
        }
    }

    pub fn record(
        &mut self,
        policy: &PathPolicy,
        checkpoint: IntegrityCheckpoint,
    ) -> Result<(), IntegrityError> {
        self.document.last_action = checkpoint.action.clone();
        self.document.checkpoints.push(checkpoint);
        self.path = persist_journal(policy, &self.path, true, &self.document)?;
        Ok(())
    }

    pub fn finalize(&mut self, policy: &PathPolicy) -> Result<(), IntegrityError> {
        self.document.status = IntegrityJournalStatus::Finalized;
        self.document.last_action = "finalized".to_owned();
        self.path = persist_journal(policy, &self.path, true, &self.document)?;
        Ok(())
    }
}

fn persist_journal(
    policy: &PathPolicy,
    path: &Path,
    overwrite: bool,
    document: &IntegrityJournalDocument,
) -> Result<PathBuf, IntegrityError> {
    let bytes = serde_json::to_vec_pretty(document)?;
    let artifact = write_atomic(policy, path, overwrite, |output| {
        std::io::Write::write_all(output, &bytes)?;
        std::io::Write::write_all(output, b"\n")?;
        Ok(())
    })?;
    Ok(artifact.path)
}

impl IntegrityBaseline {
    pub fn capture(root: &Path) -> Result<Self, IntegrityError> {
        let root = root.canonicalize()?;
        let (records, preexisting_changes) = capture_records(&root)?;
        let digest = digest_records(&records);
        Ok(Self {
            root,
            records,
            digest,
            preexisting_changes,
        })
    }

    pub fn checkpoint(
        &self,
        action: impl Into<String>,
        owned_paths: &[PathBuf],
    ) -> Result<IntegrityCheckpoint, IntegrityError> {
        let (current, _) = capture_records(&self.root)?;
        let owned: BTreeSet<String> = owned_paths
            .iter()
            .filter_map(|path| {
                let normalized = path.canonicalize().unwrap_or_else(|_| path.to_owned());
                normalized
                    .strip_prefix(&self.root)
                    .ok()
                    .map(normalize_relative)
            })
            .collect();
        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut deleted = Vec::new();
        for (path, identity) in &current {
            if owned.contains(path) {
                continue;
            }
            match self.records.get(path) {
                None => added.push(path.clone()),
                Some(previous) if previous != identity => modified.push(path.clone()),
                _ => {}
            }
        }
        for path in self.records.keys() {
            if !current.contains_key(path) && !owned.contains(path) {
                deleted.push(path.clone());
            }
        }
        let checkpoint = IntegrityCheckpoint {
            action: action.into(),
            baseline_digest: self.digest.clone(),
            current_digest: digest_records(&current),
            added,
            modified,
            deleted,
            owned_paths: owned.into_iter().collect(),
        };
        let violations = checkpoint
            .added
            .iter()
            .chain(&checkpoint.modified)
            .chain(&checkpoint.deleted)
            .cloned()
            .collect::<Vec<_>>();
        if violations.is_empty() {
            Ok(checkpoint)
        } else {
            Err(IntegrityError::Violation(violations))
        }
    }
}

fn capture_records(root: &Path) -> Result<(BTreeMap<String, String>, Vec<String>), IntegrityError> {
    let git = Command::new("git")
        .args([
            "-C",
            &root.to_string_lossy(),
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=all",
        ])
        .output();
    if let Ok(output) = git {
        if output.status.success() {
            let changes = output
                .stdout
                .split(|byte| *byte == 0)
                .filter(|entry| !entry.is_empty())
                .map(|entry| String::from_utf8_lossy(entry).into_owned())
                .collect::<Vec<_>>();
            let mut records = BTreeMap::new();
            let tracked = Command::new("git")
                .args(["-C", &root.to_string_lossy(), "ls-files", "-z"])
                .output()?;
            for path in tracked
                .stdout
                .split(|byte| *byte == 0)
                .filter(|entry| !entry.is_empty())
            {
                let relative = String::from_utf8_lossy(path).replace('\\', "/");
                let identity = file_identity(&root.join(&relative))?;
                records.insert(relative, identity);
                if records.len() > MAX_INTEGRITY_ENTRIES {
                    return Err(IntegrityError::ScopeTooLarge);
                }
            }
            for change in &changes {
                if let Some(relative) = change.strip_prefix("? ") {
                    records.insert(
                        relative.replace('\\', "/"),
                        file_identity(&root.join(relative))?,
                    );
                }
            }
            return Ok((records, changes));
        }
    }
    let mut records = BTreeMap::new();
    let mut total_bytes = 0;
    visit_files(root, root, &mut records, &mut total_bytes)?;
    Ok((records, Vec::new()))
}

fn visit_files(
    root: &Path,
    directory: &Path,
    records: &mut BTreeMap<String, String>,
    total_bytes: &mut u64,
) -> Result<(), IntegrityError> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if entry.file_type()?.is_dir() {
            if matches!(
                name.to_str(),
                Some(".git" | "target" | ".meridian-tracy-diagnostics")
            ) {
                continue;
            }
            visit_files(root, &path, records, total_bytes)?;
        } else if entry.file_type()?.is_file() {
            let metadata = entry.metadata()?;
            *total_bytes = total_bytes.saturating_add(metadata.len());
            if records.len() >= MAX_INTEGRITY_ENTRIES || *total_bytes > MAX_INTEGRITY_BYTES {
                return Err(IntegrityError::ScopeTooLarge);
            }
            records.insert(
                normalize_relative(path.strip_prefix(root).unwrap_or(&path)),
                file_identity(&path)?,
            );
        }
    }
    Ok(())
}

fn file_identity(path: &Path) -> Result<String, IntegrityError> {
    if !path.is_file() {
        return Ok("missing".to_owned());
    }
    let bytes = std::fs::read(path)?;
    Ok(format!("{}:{:x}", bytes.len(), Sha256::digest(&bytes)))
}

fn digest_records(records: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    for (path, identity) in records {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(identity.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn normalize_relative(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
