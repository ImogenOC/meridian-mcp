use anyhow::{anyhow, bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use crate::EffectiveRoot;

const MAX_RECORD_BYTES: usize = 8 * 1024 * 1024;
const MAX_RECORDS: usize = 100_000;
const TEMPORARY_NAME_ATTEMPTS: usize = 32;

pub struct PrivateStateStore {
    root: PathBuf,
    _lock: File,
}

impl PrivateStateStore {
    pub fn open(path: &Path, workspace_roots: &[EffectiveRoot]) -> Result<Self> {
        let metadata = std::fs::symlink_metadata(path).with_context(|| {
            format!("private state directory does not exist: {}", path.display())
        })?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            bail!("private state path must be an existing non-symlink directory");
        }
        let root = path.canonicalize()?;
        if workspace_roots
            .iter()
            .any(|workspace| root.starts_with(&workspace.path) || workspace.path.starts_with(&root))
        {
            bail!("private state directory must be outside every workspace root");
        }

        let lock_path = root.join(".meridian-mcp.lock");
        let lock = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .with_context(|| format!("private state directory is locked: {}", root.display()))?;
        Ok(Self { root, _lock: lock })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write_json_atomic<T: Serialize>(&self, relative: &str, value: &T) -> Result<PathBuf> {
        let output = self.resolve_record(relative, true)?;
        let bytes = serde_json::to_vec_pretty(value)?;
        if bytes.len() > MAX_RECORD_BYTES {
            bail!("private state record exceeds the 8 MiB limit");
        }
        let parent = output.parent().expect("validated record path has a parent");
        let (temporary, mut file) = create_private_file(parent)?;
        let result = (|| -> Result<()> {
            file.write_all(&bytes)?;
            file.write_all(b"\n")?;
            file.flush()?;
            file.sync_all()?;
            drop(file);
            install_temporary(&temporary, &output)?;
            let installed: serde_json::Value = self.read_json(relative)?;
            drop(installed);
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result?;
        Ok(output)
    }

    pub fn read_json<T: DeserializeOwned>(&self, relative: &str) -> Result<T> {
        let path = self.resolve_record(relative, false)?;
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!("private state record is not a regular file");
        }
        if metadata.len() > MAX_RECORD_BYTES as u64 {
            bail!("private state record exceeds the 8 MiB limit");
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        File::open(path)?
            .take((MAX_RECORD_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_RECORD_BYTES {
            bail!("private state record exceeds the 8 MiB limit");
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn list_records(&self, namespace: &str, max_entries: usize) -> Result<Vec<PathBuf>> {
        let maximum = max_entries.min(MAX_RECORDS);
        let directory = self.resolve_record(namespace, false)?;
        if !directory.is_dir() {
            return Ok(Vec::new());
        }
        let mut pending = vec![directory];
        let mut records = Vec::new();
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(directory)? {
                let entry = entry?;
                let metadata = entry.file_type()?;
                if metadata.is_symlink() {
                    bail!("private state namespaces cannot contain symlinks");
                }
                if metadata.is_dir() {
                    pending.push(entry.path());
                } else if metadata.is_file() {
                    records.push(entry.path());
                    if records.len() > maximum {
                        bail!("private state record enumeration exceeds its limit");
                    }
                }
            }
        }
        records.sort();
        Ok(records)
    }

    fn resolve_record(&self, relative: &str, create_parent: bool) -> Result<PathBuf> {
        let relative = Path::new(relative);
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            bail!("private state record name must be a non-empty relative path");
        }
        let path = self.root.join(relative);
        let parent = path.parent().expect("validated record path has a parent");
        if create_parent {
            std::fs::create_dir_all(parent)?;
        }
        if parent.exists() {
            let canonical_parent = parent.canonicalize()?;
            if !canonical_parent.starts_with(&self.root) {
                bail!("private state record escapes through a symlink or reparse point");
            }
        }
        if path.exists() {
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                bail!("private state records cannot be symlinks");
            }
        }
        Ok(path)
    }
}

impl Drop for PrivateStateStore {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.root.join(".meridian-mcp.lock"));
    }
}

fn create_private_file(parent: &Path) -> Result<(PathBuf, File)> {
    for _ in 0..TEMPORARY_NAME_ATTEMPTS {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|error| anyhow!(error.to_string()))?;
        let path = parent.join(format!(".meridian-tmp-{}", hex(&random)));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!("could not allocate a private state temporary file")
}

fn install_temporary(temporary: &Path, output: &Path) -> Result<()> {
    let parent = output.parent().expect("validated record path has a parent");
    let backup = if output.exists() {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|error| anyhow!(error.to_string()))?;
        let backup = parent.join(format!(".meridian-backup-{}", hex(&random)));
        std::fs::rename(output, &backup)?;
        Some(backup)
    } else {
        None
    };
    if let Err(install_error) = std::fs::rename(temporary, output) {
        if let Some(backup) = &backup {
            let _ = std::fs::rename(backup, output);
        }
        return Err(install_error.into());
    }
    if let Some(backup) = backup {
        std::fs::remove_file(backup)?;
    }
    OpenOptions::new().write(true).open(output)?.sync_all()?;
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
