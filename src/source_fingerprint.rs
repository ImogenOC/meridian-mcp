//! Metadata fingerprints for the source files that produced an analysis snapshot.
//!
//! `dm_parse_environment` uses these to skip a full reparse when the environment
//! has matching paths, lengths, and settled modification times to the active
//! snapshot. It does not hash file contents.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Files modified within this window of the capture instant are treated as
/// unsettled. Filesystem modification timestamps are coarse (FAT rounds to two
/// seconds, and network filesystems can be worse), so an edit made in the same
/// tick as the capture can be invisible to a later comparison. Refusing to reuse
/// a fingerprint that recent trades a redundant reparse for correctness.
const MTIME_SETTLE_WINDOW: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Eq, PartialEq)]
struct FingerprintEntry {
    path: PathBuf,
    resolved: PathBuf,
    len: u64,
    modified: SystemTime,
}

/// The observed state of every file that contributed to a parse.
///
/// A fingerprint is only usable for reuse decisions when every input could be
/// stat'd and every modification time is comfortably in the past; otherwise
/// `reusable` is false and comparisons always fail closed.
#[derive(Clone, Debug)]
pub struct SourceFingerprint {
    entries: Vec<FingerprintEntry>,
    discovery_paths: Vec<PathBuf>,
    reusable: bool,
}

impl SourceFingerprint {
    /// Stat every input, judging staleness against `observed_at`.
    ///
    /// `observed_at` should be the instant the caller began trusting the files:
    /// for a fresh parse that is the moment the parse started, so a file edited
    /// while the parser was reading it marks the result unreusable.
    pub fn capture(inputs: &[PathBuf], observed_at: SystemTime) -> Self {
        Self::capture_with_discovery(inputs, &[], observed_at)
    }

    /// Optional discovery candidates contribute identity even while absent.
    /// Only NotFound counts as absence; other failures prevent reuse.
    pub fn capture_with_discovery(
        inputs: &[PathBuf],
        discovery_paths: &[PathBuf],
        observed_at: SystemTime,
    ) -> Self {
        let settled_before = observed_at
            .checked_sub(MTIME_SETTLE_WINDOW)
            .unwrap_or(observed_at);
        let mut entries = Vec::with_capacity(inputs.len());
        let mut reusable = true;

        for (path, optional) in inputs
            .iter()
            .map(|path| (path, false))
            .chain(discovery_paths.iter().map(|path| (path, true)))
        {
            let metadata = match std::fs::metadata(path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    if !optional
                        || error.kind() != std::io::ErrorKind::NotFound
                        || std::fs::symlink_metadata(path).is_ok()
                    {
                        reusable = false;
                    }
                    continue;
                }
            };
            let Ok(resolved) = path.canonicalize() else {
                reusable = false;
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                reusable = false;
                continue;
            };
            if modified >= settled_before || !metadata.is_file() {
                reusable = false;
            }
            entries.push(FingerprintEntry {
                path: path.clone(),
                resolved,
                len: metadata.len(),
                modified,
            });
        }

        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Self {
            entries,
            discovery_paths: discovery_paths.to_vec(),
            reusable,
        }
    }

    /// Whether this fingerprint may take part in a reuse decision at all.
    pub fn is_reusable(&self) -> bool {
        self.reusable
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// True when both fingerprints are usable and describe identical files.
    pub fn matches(&self, other: &SourceFingerprint) -> bool {
        self.reusable
            && other.reusable
            && self.entries == other.entries
            && self.discovery_paths == other.discovery_paths
    }

    pub fn discovery_paths(&self) -> &[PathBuf] {
        &self.discovery_paths
    }

    /// The inputs this fingerprint covers, for re-stat'ing on a later call.
    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.entries.iter().map(|entry| entry.path.as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::UNIX_EPOCH;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn scratch_directory() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "meridian-mcp-fingerprint-{}-{unique}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    /// Push a file's modification time far enough back that it counts as settled.
    fn backdate(path: &Path) {
        let file = std::fs::File::options().write(true).open(path).unwrap();
        let settled = SystemTime::now() - Duration::from_secs(60);
        file.set_modified(settled).unwrap();
    }

    #[test]
    fn identical_settled_files_match() {
        let directory = scratch_directory();
        let file = directory.join("a.dm");
        std::fs::write(&file, "contents").unwrap();
        backdate(&file);

        let inputs = vec![file];
        let first = SourceFingerprint::capture(&inputs, SystemTime::now());
        let second = SourceFingerprint::capture(&inputs, SystemTime::now());

        assert!(first.is_reusable());
        assert!(first.matches(&second));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_rewritten_file_breaks_the_match() {
        let directory = scratch_directory();
        let file = directory.join("a.dm");
        std::fs::write(&file, "contents").unwrap();
        backdate(&file);
        let inputs = vec![file.clone()];
        let first = SourceFingerprint::capture(&inputs, SystemTime::now());

        std::fs::write(&file, "different contents").unwrap();
        backdate(&file);
        let second = SourceFingerprint::capture(&inputs, SystemTime::now());

        assert!(!first.matches(&second));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_freshly_written_file_is_never_reusable() {
        let directory = scratch_directory();
        let file = directory.join("a.dm");
        std::fs::write(&file, "contents").unwrap();

        let fingerprint = SourceFingerprint::capture(&[file], SystemTime::now());

        assert!(!fingerprint.is_reusable());
        assert!(!fingerprint.matches(&fingerprint.clone()));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_missing_input_is_never_reusable() {
        let directory = scratch_directory();
        let fingerprint =
            SourceFingerprint::capture(&[directory.join("absent.dm")], SystemTime::now());

        assert!(!fingerprint.is_reusable());
        assert!(fingerprint.is_empty());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn optional_absence_can_be_reused_but_creation_and_deletion_cannot() {
        let directory = scratch_directory();
        let config = directory.join("SpacemanDMM.toml");
        let candidates = vec![config.clone()];
        let absent = SourceFingerprint::capture_with_discovery(&[], &candidates, SystemTime::now());
        assert!(absent.is_reusable());
        assert!(absent.matches(&SourceFingerprint::capture_with_discovery(
            &[],
            &candidates,
            SystemTime::now()
        )));
        std::fs::write(&config, "[diagnostics]\n").unwrap();
        backdate(&config);
        let present =
            SourceFingerprint::capture_with_discovery(&[], &candidates, SystemTime::now());
        assert!(!absent.matches(&present));
        std::fs::remove_file(&config).unwrap();
        assert!(!present.matches(&SourceFingerprint::capture_with_discovery(
            &[],
            &candidates,
            SystemTime::now()
        )));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn retargeted_directory_alias_changes_identity_with_identical_metadata() {
        let directory = scratch_directory();
        let first = directory.join("first");
        let second = directory.join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        let modified = SystemTime::now() - Duration::from_secs(60);
        for target in [&first, &second] {
            let path = target.join("source.dm");
            std::fs::write(&path, "same length").unwrap();
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(modified)
                .unwrap();
        }
        let link = directory.join("alias");
        let create_link = |target: &Path| {
            #[cfg(windows)]
            assert!(std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&link)
                .arg(target)
                .output()
                .unwrap()
                .status
                .success());
            #[cfg(unix)]
            std::os::unix::fs::symlink(target, &link).unwrap();
        };
        let remove_link = || {
            #[cfg(windows)]
            std::fs::remove_dir(&link).unwrap();
            #[cfg(unix)]
            std::fs::remove_file(&link).unwrap();
        };
        create_link(&first);
        let inputs = vec![link.join("source.dm")];
        let before = SourceFingerprint::capture(&inputs, SystemTime::now());
        remove_link();
        create_link(&second);
        let after = SourceFingerprint::capture(&inputs, SystemTime::now());
        assert!(before.is_reusable() && after.is_reusable());
        assert!(!before.matches(&after));
        remove_link();
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_removed_input_breaks_the_match() {
        let directory = scratch_directory();
        let kept = directory.join("a.dm");
        let removed = directory.join("b.dm");
        std::fs::write(&kept, "a").unwrap();
        std::fs::write(&removed, "b").unwrap();
        backdate(&kept);
        backdate(&removed);

        let inputs = vec![kept.clone(), removed.clone()];
        let first = SourceFingerprint::capture(&inputs, SystemTime::now());
        std::fs::remove_file(&removed).unwrap();
        let second = SourceFingerprint::capture(&inputs, SystemTime::now());

        assert!(first.is_reusable());
        assert!(!second.is_reusable());
        assert!(!first.matches(&second));

        std::fs::remove_dir_all(directory).unwrap();
    }
}
