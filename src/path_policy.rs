use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct PolicyError {
    code: &'static str,
    path: PathBuf,
    message: String,
}

impl PolicyError {
    pub fn code(&self) -> &'static str {
        self.code
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for PolicyError {}

#[derive(Clone, Debug)]
pub struct PathPolicy {
    roots: Vec<PathBuf>,
    executables: Vec<PathBuf>,
}

impl PathPolicy {
    pub fn new(roots: Vec<PathBuf>, executables: Vec<PathBuf>) -> Result<Self, PolicyError> {
        if roots.is_empty() {
            return Err(error(
                "no_workspace_roots",
                PathBuf::new(),
                "no workspace roots configured",
            ));
        }
        Ok(Self {
            roots: canonicalize(roots, "invalid_workspace_root")?,
            executables: canonicalize(executables, "invalid_executable")?,
        })
    }

    pub fn read_path(&self, path: impl AsRef<Path>) -> Result<PathBuf, PolicyError> {
        let input = path.as_ref();
        let canonical = input.canonicalize().map_err(|source| {
            error(
                "path_not_found",
                input.to_owned(),
                format!("cannot resolve {}: {source}", input.display()),
            )
        })?;
        self.require_contained(canonical)
    }

    pub fn output_path(
        &self,
        path: impl AsRef<Path>,
        overwrite: bool,
    ) -> Result<PathBuf, PolicyError> {
        let input = path.as_ref();
        if input.exists() {
            let canonical = self.read_path(input)?;
            if !overwrite {
                return Err(error(
                    "output_exists",
                    canonical,
                    "output already exists; set overwrite=true",
                ));
            }
            return Ok(canonical);
        }
        let parent = input.parent().ok_or_else(|| {
            error(
                "invalid_output_path",
                input.to_owned(),
                "output path has no parent",
            )
        })?;
        let parent = parent.canonicalize().map_err(|source| {
            error(
                "output_parent_not_found",
                parent.to_owned(),
                format!("cannot resolve output parent: {source}"),
            )
        })?;
        let candidate = parent.join(input.file_name().ok_or_else(|| {
            error(
                "invalid_output_path",
                input.to_owned(),
                "output path has no file name",
            )
        })?);
        self.require_contained(candidate)
    }

    pub fn executable(&self, path: impl AsRef<Path>) -> Result<PathBuf, PolicyError> {
        let input = path.as_ref();
        let canonical = input.canonicalize().map_err(|_| {
            error(
                "executable_not_allowed",
                input.to_owned(),
                "executable is not allowlisted",
            )
        })?;
        if self.executables.contains(&canonical) {
            Ok(canonical)
        } else {
            Err(error(
                "executable_not_allowed",
                canonical,
                "executable is not allowlisted",
            ))
        }
    }

    pub fn runtime_dmb(&self, path: impl AsRef<Path>) -> Result<PathBuf, PolicyError> {
        self.read_path(path)
    }

    pub fn compiler_allowlist(&self) -> &[PathBuf] {
        &self.executables
    }

    fn require_contained(&self, path: PathBuf) -> Result<PathBuf, PolicyError> {
        if self.roots.iter().any(|root| path.starts_with(root)) {
            Ok(path)
        } else {
            Err(error(
                "path_outside_workspace",
                path,
                "path is outside configured workspace roots",
            ))
        }
    }
}

fn canonicalize(paths: Vec<PathBuf>, code: &'static str) -> Result<Vec<PathBuf>, PolicyError> {
    paths
        .into_iter()
        .map(|path| {
            path.canonicalize().map_err(|source| {
                error(
                    code,
                    path.clone(),
                    format!("cannot resolve {}: {source}", path.display()),
                )
            })
        })
        .collect()
}

fn error(code: &'static str, path: PathBuf, message: impl Into<String>) -> PolicyError {
    PolicyError {
        code,
        path,
        message: message.into(),
    }
}
