use crate::{PathPolicy, PolicyError};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const RANDOM_NAME_ATTEMPTS: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OutputArtifact {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AtomicOutputError {
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("output path is not a file: {0}")]
    InvalidOutputType(PathBuf),
    #[error("could not allocate a private temporary output name")]
    TemporaryNameExhausted,
    #[error("could not acquire randomness for a temporary output name: {0}")]
    Entropy(String),
    #[error("{0}")]
    Writer(String),
    #[error("could not install replacement: {install}; restoration failure: {restore}")]
    Replacement { install: String, restore: String },
}

impl AtomicOutputError {
    pub fn writer(message: impl Into<String>) -> Self {
        Self::Writer(message.into())
    }

    pub fn policy_code(&self) -> Option<&'static str> {
        match self {
            Self::Policy(error) => Some(error.code()),
            _ => None,
        }
    }
}

struct TemporaryOutput {
    path: PathBuf,
    armed: bool,
}

pub struct ReservedExternalOutput {
    output: PathBuf,
    temporary: TemporaryOutput,
}

impl ReservedExternalOutput {
    pub fn temporary_path(&self) -> &Path {
        &self.temporary.path
    }

    pub fn commit(self) -> Result<OutputArtifact, AtomicOutputError> {
        let metadata = std::fs::symlink_metadata(&self.temporary.path)?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(AtomicOutputError::InvalidOutputType(
                self.temporary.path.clone(),
            ));
        }
        OpenOptions::new()
            .write(true)
            .open(&self.temporary.path)?
            .sync_all()?;
        install_temporary(self.output, self.temporary)
    }
}

impl TemporaryOutput {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub fn write_atomic<F>(
    policy: &PathPolicy,
    output: &Path,
    overwrite: bool,
    write: F,
) -> Result<OutputArtifact, AtomicOutputError>
where
    F: FnOnce(&mut File) -> Result<(), AtomicOutputError>,
{
    let output = policy.output_path(output, overwrite)?;
    if output.exists() && !output.is_file() {
        return Err(AtomicOutputError::InvalidOutputType(output));
    }
    let parent = output.parent().ok_or_else(|| {
        AtomicOutputError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output path has no parent",
        ))
    })?;
    let (temporary_path, mut temporary_file) = create_private_file(parent, "tmp")?;
    let temporary = TemporaryOutput {
        path: temporary_path,
        armed: true,
    };

    write(&mut temporary_file)?;
    temporary_file.flush()?;
    temporary_file.sync_all()?;
    drop(temporary_file);

    install_temporary(output, temporary)
}

pub fn promote_external_atomic<F>(
    policy: &PathPolicy,
    output: &Path,
    overwrite: bool,
    produce: F,
) -> Result<OutputArtifact, AtomicOutputError>
where
    F: FnOnce(&Path) -> Result<(), AtomicOutputError>,
{
    let output = policy.output_path(output, overwrite)?;
    if output.exists() && !output.is_file() {
        return Err(AtomicOutputError::InvalidOutputType(output));
    }
    let parent = output.parent().ok_or_else(|| {
        AtomicOutputError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output path has no parent",
        ))
    })?;
    let (temporary_path, temporary_file) = create_private_file(parent, "external")?;
    drop(temporary_file);
    let temporary = TemporaryOutput {
        path: temporary_path,
        armed: true,
    };

    produce(&temporary.path)?;
    let metadata = std::fs::symlink_metadata(&temporary.path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(AtomicOutputError::InvalidOutputType(temporary.path.clone()));
    }
    OpenOptions::new()
        .write(true)
        .open(&temporary.path)?
        .sync_all()?;

    install_temporary(output, temporary)
}

pub fn reserve_external_atomic(
    policy: &PathPolicy,
    output: &Path,
    overwrite: bool,
) -> Result<ReservedExternalOutput, AtomicOutputError> {
    let output = policy.output_path(output, overwrite)?;
    if output.exists() && !output.is_file() {
        return Err(AtomicOutputError::InvalidOutputType(output));
    }
    let parent = output.parent().ok_or_else(|| {
        AtomicOutputError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output path has no parent",
        ))
    })?;
    let (temporary_path, temporary_file) = create_private_file(parent, "external")?;
    drop(temporary_file);
    Ok(ReservedExternalOutput {
        output,
        temporary: TemporaryOutput {
            path: temporary_path,
            armed: true,
        },
    })
}

fn install_temporary(
    output: PathBuf,
    mut temporary: TemporaryOutput,
) -> Result<OutputArtifact, AtomicOutputError> {
    let parent = output.parent().ok_or_else(|| {
        AtomicOutputError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output path has no parent",
        ))
    })?;

    let bytes = std::fs::metadata(&temporary.path)?.len();
    let sha256 = hash_file(&temporary.path)?;
    let backup = if output.exists() {
        let backup = private_available_path(parent, "backup")?;
        std::fs::rename(&output, &backup)?;
        Some(backup)
    } else {
        None
    };

    if let Err(install_error) = std::fs::rename(&temporary.path, &output) {
        let restore_error = backup
            .as_ref()
            .and_then(|backup| std::fs::rename(backup, &output).err());
        return match restore_error {
            Some(restore_error) => Err(AtomicOutputError::Replacement {
                install: install_error.to_string(),
                restore: restore_error.to_string(),
            }),
            None => Err(AtomicOutputError::Io(install_error)),
        };
    }
    temporary.disarm();

    if let Some(backup) = backup {
        std::fs::remove_file(backup)?;
    }

    Ok(OutputArtifact {
        path: output.canonicalize()?,
        bytes,
        sha256,
    })
}

fn create_private_file(parent: &Path, purpose: &str) -> Result<(PathBuf, File), AtomicOutputError> {
    for _ in 0..RANDOM_NAME_ATTEMPTS {
        let path = private_path(parent, purpose)?;
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(AtomicOutputError::TemporaryNameExhausted)
}

fn private_available_path(parent: &Path, purpose: &str) -> Result<PathBuf, AtomicOutputError> {
    for _ in 0..RANDOM_NAME_ATTEMPTS {
        let path = private_path(parent, purpose)?;
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(AtomicOutputError::TemporaryNameExhausted)
}

fn private_path(parent: &Path, purpose: &str) -> Result<PathBuf, AtomicOutputError> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|error| AtomicOutputError::Entropy(error.to_string()))?;
    let suffix = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(parent.join(format!(".meridian-mcp-{suffix}.{purpose}")))
}

fn hash_file(path: &Path) -> Result<String, AtomicOutputError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
