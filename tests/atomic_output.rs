use meridian_mcp::atomic_output::{
    promote_external_atomic, reserve_external_atomic, write_atomic, AtomicOutputError,
};
use meridian_mcp::PathPolicy;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn fixture() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "meridian-mcp-atomic-output-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn rejects_outputs_outside_the_workspace() {
    let root = fixture();
    let outside = fixture();
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();

    let error = write_atomic(&policy, &outside.join("result.txt"), false, |file| {
        file.write_all(b"forbidden")?;
        Ok(())
    })
    .unwrap_err();

    assert_eq!(error.policy_code(), Some("path_outside_workspace"));
    assert!(!outside.join("result.txt").exists());
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(outside).unwrap();
}

#[test]
fn refuses_an_existing_output_without_explicit_overwrite() {
    let root = fixture();
    let output = root.join("result.txt");
    std::fs::write(&output, "original").unwrap();
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();

    let error = write_atomic(&policy, &output, false, |_| Ok(())).unwrap_err();

    assert_eq!(error.policy_code(), Some("output_exists"));
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "original");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn writes_a_new_output_and_reports_its_identity() {
    let root = fixture();
    let output = root.join("result.txt");
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();

    let artifact = write_atomic(&policy, &output, false, |file| {
        file.write_all(b"meridian")?;
        Ok(())
    })
    .unwrap();

    assert_eq!(artifact.path, output.canonicalize().unwrap());
    assert_eq!(artifact.bytes, 8);
    assert_eq!(
        artifact.sha256,
        "b6c4ac412ac8822355239dd717c11ca5b07373e4db550d0423c1b6aeceef8493"
    );
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "meridian");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn replaces_an_existing_output_only_after_the_writer_succeeds() {
    let root = fixture();
    let output = root.join("result.txt");
    std::fs::write(&output, "original").unwrap();
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();

    write_atomic(&policy, &output, true, |file| {
        file.write_all(b"replacement")?;
        Ok(())
    })
    .unwrap();

    assert_eq!(std::fs::read_to_string(&output).unwrap(), "replacement");
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn writer_failure_preserves_the_original_and_removes_temporary_files() {
    let root = fixture();
    let output = root.join("result.txt");
    std::fs::write(&output, "original").unwrap();
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();

    let error = write_atomic(&policy, &output, true, |file| {
        file.write_all(b"partial")?;
        Err(AtomicOutputError::writer("deliberate writer failure"))
    })
    .unwrap_err();

    assert_eq!(error.to_string(), "deliberate writer failure");
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "original");
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_output_is_promoted_only_after_successful_validation() {
    let root = fixture();
    let output = root.join("capture.tracy");
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();

    let artifact = promote_external_atomic(&policy, &output, false, |temporary| {
        std::fs::write(temporary, b"trace bytes")?;
        Ok(())
    })
    .unwrap();

    assert_eq!(artifact.path, output.canonicalize().unwrap());
    assert_eq!(artifact.bytes, 11);
    assert_eq!(std::fs::read(&output).unwrap(), b"trace bytes");
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_output_failure_preserves_existing_output_and_cleans_temporary_file() {
    let root = fixture();
    let output = root.join("capture.tracy");
    std::fs::write(&output, b"original").unwrap();
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();

    let error = promote_external_atomic(&policy, &output, true, |temporary| {
        std::fs::write(temporary, b"partial")?;
        Err(AtomicOutputError::writer("capture failed"))
    })
    .unwrap_err();

    assert_eq!(error.to_string(), "capture failed");
    assert_eq!(std::fs::read(&output).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reserved_external_output_is_private_until_commit_and_cleans_on_drop() {
    let root = fixture();
    let output = root.join("capture.tracy");
    let policy = PathPolicy::new(vec![root.clone()], Vec::new()).unwrap();

    let reserved = reserve_external_atomic(&policy, &output, false).unwrap();
    assert!(!output.exists());
    std::fs::write(reserved.temporary_path(), b"complete trace").unwrap();
    let artifact = reserved.commit().unwrap();

    assert_eq!(artifact.bytes, 14);
    assert_eq!(std::fs::read(&output).unwrap(), b"complete trace");

    let abandoned = root.join("abandoned.tracy");
    let reserved = reserve_external_atomic(&policy, &abandoned, false).unwrap();
    let temporary = reserved.temporary_path().to_owned();
    drop(reserved);
    assert!(!temporary.exists());
    assert!(!abandoned.exists());
    std::fs::remove_dir_all(root).unwrap();
}
