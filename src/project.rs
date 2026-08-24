use crate::{PathPolicy, PolicyError};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ProjectProfile {
    root: PathBuf,
    dme_path: PathBuf,
    spaceman_config: Option<PathBuf>,
    human_build_entrypoint: Option<PathBuf>,
    rift_build_entrypoint: Option<PathBuf>,
    byond_version: Option<String>,
}

impl ProjectProfile {
    pub fn discover(policy: &PathPolicy, dme_path: &Path) -> Result<Self, PolicyError> {
        let dme_path = policy.read_path(dme_path)?;
        let root = dme_path
            .parent()
            .expect("a canonical file has a parent")
            .to_owned();
        let spaceman_config = contained_optional(policy, root.join("SpacemanDMM.toml"))?;
        let human_build_entrypoint = contained_optional(policy, root.join("BUILD.cmd"))?;
        let rift_build_entrypoint = contained_optional(policy, root.join("RIFT_BUILD.cmd"))?;
        let dependencies = root.join("dependencies.sh");
        let byond_version = if dependencies.is_file() {
            let dependencies = policy.read_path(dependencies)?;
            parse_byond_version(&std::fs::read_to_string(dependencies).unwrap_or_default())
        } else {
            None
        };
        Ok(Self {
            root,
            dme_path,
            spaceman_config,
            human_build_entrypoint,
            rift_build_entrypoint,
            byond_version,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn dme_path(&self) -> &Path {
        &self.dme_path
    }
    pub fn spaceman_config(&self) -> Option<&Path> {
        self.spaceman_config.as_deref()
    }
    pub fn human_build_entrypoint(&self) -> Option<&Path> {
        self.human_build_entrypoint.as_deref()
    }
    pub fn rift_build_entrypoint(&self) -> Option<&Path> {
        self.rift_build_entrypoint.as_deref()
    }
    pub fn full_build_entrypoint(&self) -> Option<&Path> {
        self.human_build_entrypoint()
    }
    pub fn byond_version(&self) -> Option<&str> {
        self.byond_version.as_deref()
    }

    pub fn is_rift_build_qualified(&self) -> bool {
        self.dme_path.file_name().and_then(|name| name.to_str()) == Some("tgstation.dme")
            && is_direct_root_file(
                &self.root,
                self.human_build_entrypoint.as_deref(),
                "BUILD.cmd",
            )
            && is_direct_root_file(
                &self.root,
                self.rift_build_entrypoint.as_deref(),
                "RIFT_BUILD.cmd",
            )
            && self.byond_version.is_some()
    }
}

fn is_direct_root_file(root: &Path, path: Option<&Path>, name: &str) -> bool {
    path.is_some_and(|path| {
        path.parent() == Some(root) && path.file_name().and_then(|file| file.to_str()) == Some(name)
    })
}

fn contained_optional(policy: &PathPolicy, path: PathBuf) -> Result<Option<PathBuf>, PolicyError> {
    if path.is_file() {
        policy.read_path(path).map(Some)
    } else {
        Ok(None)
    }
}

fn parse_byond_version(contents: &str) -> Option<String> {
    let value = |name: &str| -> Option<String> {
        let values = contents
            .lines()
            .filter_map(|line| {
                let line = line.trim().strip_prefix("export ").unwrap_or(line.trim());
                line.strip_prefix(&format!("{name}=")).and_then(|value| {
                    let value =
                        value.trim_matches(|character| character == '\'' || character == '"');
                    (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
                        .then(|| value.to_owned())
                })
            })
            .collect::<Vec<_>>();
        (values.len() == 1).then(|| values[0].clone())
    };
    Some(format!(
        "{}.{}",
        value("BYOND_MAJOR")?,
        value("BYOND_MINOR")?
    ))
}

#[cfg(test)]
mod tests {
    use super::parse_byond_version;

    #[test]
    fn version_parser_accepts_exported_literals_only() {
        assert_eq!(
            parse_byond_version("export BYOND_MAJOR=516\nBYOND_MINOR='1685'"),
            Some("516.1685".into())
        );
        assert_eq!(parse_byond_version("BYOND_MAJOR=$(command)"), None);
        assert_eq!(
            parse_byond_version("BYOND_MAJOR=$(command)\nBYOND_MINOR=1685"),
            None
        );
        assert_eq!(
            parse_byond_version("BYOND_MAJOR=516\nBYOND_MINOR=$BYOND_MINOR"),
            None
        );
    }
}
