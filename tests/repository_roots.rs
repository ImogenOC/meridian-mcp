use meridian_mcp::{expand_effective_roots, RootSource};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct GitWorktreeFixture {
    root: PathBuf,
    primary: PathBuf,
    linked: PathBuf,
    unrelated: PathBuf,
}

impl GitWorktreeFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "meridian-mcp-repository-roots-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let primary = root.join("primary");
        let linked = root.join("linked");
        let unrelated = root.join("unrelated");
        std::fs::create_dir_all(&primary).unwrap();
        std::fs::create_dir_all(&unrelated).unwrap();

        git(&primary, &["init"]);
        std::fs::write(primary.join("tracked.txt"), "tracked\n").unwrap();
        git(&primary, &["add", "tracked.txt"]);
        git(
            &primary,
            &[
                "-c",
                "user.name=Meridian Test",
                "-c",
                "user.email=meridian@example.invalid",
                "commit",
                "-m",
                "fixture",
            ],
        );
        git(
            &primary,
            &[
                "worktree",
                "add",
                linked.to_str().unwrap(),
                "-b",
                "linked-fixture",
            ],
        );

        git(&unrelated, &["init"]);
        std::fs::write(unrelated.join("other.txt"), "other\n").unwrap();

        Self {
            root,
            primary,
            linked,
            unrelated,
        }
    }
}

impl Drop for GitWorktreeFixture {
    fn drop(&mut self) {
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&self.linked)
            .current_dir(&self.primary)
            .status();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn git(directory: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn linked_worktrees_expand_only_from_the_authorized_repository() {
    let fixture = GitWorktreeFixture::new();
    let roots = expand_effective_roots(
        std::slice::from_ref(&fixture.primary),
        std::slice::from_ref(&fixture.primary),
    )
    .unwrap();

    assert!(roots.iter().any(|root| {
        root.path == fixture.primary.canonicalize().unwrap()
            && root.source == RootSource::ExplicitRoot
    }));
    assert!(roots.iter().any(|root| {
        root.path == fixture.linked.canonicalize().unwrap()
            && root.source == RootSource::LinkedGitWorktree
    }));
    assert!(!roots
        .iter()
        .any(|root| root.path == fixture.unrelated.canonicalize().unwrap()));

    let primary_identity = roots
        .iter()
        .find(|root| root.path == fixture.primary.canonicalize().unwrap())
        .and_then(|root| root.repository_identity.as_ref())
        .unwrap();
    let linked_identity = roots
        .iter()
        .find(|root| root.path == fixture.linked.canonicalize().unwrap())
        .and_then(|root| root.repository_identity.as_ref())
        .unwrap();
    assert_eq!(primary_identity, linked_identity);
}
