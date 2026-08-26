use serde::Serialize;
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

#[derive(Clone, Debug, Serialize)]
pub struct IntegrityCheckpoint {
    pub action: String,
    pub baseline_digest: String,
    pub current_digest: String,
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
    pub owned_paths: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum IntegrityError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("integrity scope exceeds the fixed entry or byte limit")]
    ScopeTooLarge,
    #[error("workspace integrity violation: {0:?}")]
    Violation(Vec<String>),
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
