use meridian_mcp::artifact::ArtifactSnapshot;
use meridian_mcp::process::{
    run_contained_process, ProcessSpec, TerminationReason, MAX_PROCESS_OUTPUT_BYTES,
};
#[cfg(windows)]
use std::ffi::OsString;
use std::io::Write;
#[cfg(windows)]
use std::path::{Path, PathBuf};
use std::time::Duration;

fn fixture_spec(mode: &str, timeout: Duration, idle_timeout: Duration) -> ProcessSpec {
    ProcessSpec {
        program: std::env::current_exe().unwrap(),
        arguments: vec![
            "--ignored".into(),
            "--exact".into(),
            "process_fixture_helper".into(),
            "--nocapture".into(),
        ],
        working_directory: std::env::current_dir().unwrap(),
        environment: vec![("MERIDIAN_PROCESS_FIXTURE_MODE".into(), mode.into())],
        timeout,
        idle_timeout,
        capture_network: false,
    }
}

#[test]
#[ignore]
fn process_fixture_helper() {
    let Ok(mode) = std::env::var("MERIDIAN_PROCESS_FIXTURE_MODE") else {
        return;
    };
    match mode.as_str() {
        "exit7" => {
            println!("fixture exact exit");
            std::process::exit(7);
        }
        "huge" => {
            let payload = vec![b'x'; MAX_PROCESS_OUTPUT_BYTES * 2];
            std::io::stdout().write_all(&payload).unwrap();
            std::io::stdout().write_all(b"STDOUT_END").unwrap();
            std::io::stdout().flush().unwrap();
            std::io::stderr().write_all(&payload).unwrap();
            std::io::stderr().write_all(b"STDERR_END").unwrap();
            std::io::stderr().flush().unwrap();
        }
        "wall" => loop {
            print!("progress");
            std::io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_millis(50));
        },
        "idle" => std::thread::sleep(Duration::from_secs(30)),
        "success" => println!("fixture success"),
        other => panic!("unknown helper mode: {other}"),
    }
}

#[tokio::test]
async fn artifact_snapshots_stream_hashes_and_detect_changes() {
    let root = std::env::temp_dir().join(format!("meridian-mcp-artifact-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("tgstation.dmb");
    std::fs::write(&artifact, b"first").unwrap();
    let first = ArtifactSnapshot::capture(&root, &artifact).unwrap();
    std::fs::write(&artifact, b"second").unwrap();
    let second = ArtifactSnapshot::capture(&root, &artifact).unwrap();

    assert!(first.exists);
    assert_eq!(first.size, Some(5));
    assert_eq!(second.size, Some(6));
    assert_ne!(first.sha256, second.sha256);
    assert_eq!(first.sha256.as_deref().unwrap().len(), 64);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn output_is_tail_bounded_and_reports_truncation() {
    let outcome = run_contained_process(fixture_spec(
        "huge",
        Duration::from_secs(10),
        Duration::from_secs(5),
    ))
    .await
    .unwrap();

    assert_eq!(outcome.termination, TerminationReason::Exited);
    assert_eq!(outcome.exit_code, Some(0));
    assert!(outcome.stdout.text.len() <= MAX_PROCESS_OUTPUT_BYTES);
    assert!(outcome.stderr.text.len() <= MAX_PROCESS_OUTPUT_BYTES);
    assert!(outcome.stdout.truncated_bytes > 0);
    assert!(outcome.stderr.truncated_bytes > 0);
    assert!(outcome.stdout.text.contains("STDOUT_END"));
    assert!(outcome.stderr.text.contains("STDERR_END"));
}

#[tokio::test]
async fn wall_and_idle_timeouts_are_distinct() {
    let wall = run_contained_process(fixture_spec(
        "wall",
        Duration::from_millis(500),
        Duration::from_secs(2),
    ))
    .await
    .unwrap();
    assert_eq!(wall.termination, TerminationReason::WallTimeout);

    let idle = run_contained_process(fixture_spec(
        "idle",
        Duration::from_secs(5),
        Duration::from_millis(500),
    ))
    .await
    .unwrap();
    assert_eq!(idle.termination, TerminationReason::IdleTimeout);
}

#[tokio::test]
async fn successful_and_nonzero_exits_preserve_exact_codes() {
    let success = run_contained_process(fixture_spec(
        "success",
        Duration::from_secs(5),
        Duration::from_secs(2),
    ))
    .await
    .unwrap();
    assert_eq!(success.termination, TerminationReason::Exited);
    assert_eq!(success.exit_code, Some(0));

    let nonzero = run_contained_process(fixture_spec(
        "exit7",
        Duration::from_secs(5),
        Duration::from_secs(2),
    ))
    .await
    .unwrap();
    assert_eq!(nonzero.termination, TerminationReason::Exited);
    assert_eq!(nonzero.exit_code, Some(7));
}

#[cfg(windows)]
fn powershell_environment() -> (PathBuf, Vec<(OsString, OsString)>) {
    let system_root = std::env::var_os("SystemRoot").expect("SystemRoot is required on Windows");
    let powershell = Path::new(&system_root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    let mut environment = Vec::new();
    for name in [
        "SystemRoot",
        "SystemDrive",
        "WINDIR",
        "ComSpec",
        "PATH",
        "PATHEXT",
        "TEMP",
        "TMP",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "ProgramData",
        "LOCALAPPDATA",
        "APPDATA",
        "USERPROFILE",
    ] {
        if let Some(value) = std::env::var_os(name) {
            environment.push((name.into(), value));
        }
    }
    (powershell, environment)
}

#[cfg(windows)]
#[tokio::test]
async fn windows_job_timeout_terminates_descendants_and_audit_is_bounded() {
    let (powershell, environment) = powershell_environment();
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/process");
    let parent = fixture_root.join("parent.ps1");
    let child = fixture_root.join("child.ps1");
    let marker = std::env::temp_dir().join(format!(
        "meridian-mcp-child-marker-{}.txt",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&marker);
    let outcome = run_contained_process(ProcessSpec {
        program: powershell,
        arguments: vec![
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-File".into(),
            parent.into_os_string(),
            "-Child".into(),
            child.into_os_string(),
            "-Marker".into(),
            marker.clone().into_os_string(),
        ],
        working_directory: fixture_root,
        environment,
        timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(5),
        capture_network: true,
    })
    .await
    .unwrap();

    assert_eq!(
        outcome.termination,
        TerminationReason::WallTimeout,
        "outcome: {outcome:#?}"
    );
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(
        !marker.exists(),
        "a descendant escaped the Windows Job Object"
    );
    assert!(outcome.network_audit.requested);
    assert!(!outcome.network_audit.capture_complete);
    assert!(
        outcome.network_audit.observations.len()
            <= meridian_mcp::network_audit::MAX_NETWORK_OBSERVATIONS
    );
}
