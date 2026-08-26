use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

fn test_directory(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "meridian-mcp-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn run_pwsh(script: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new("pwsh")
        .args(["-NoLogo", "-NoProfile", "-File"])
        .arg(script)
        .args(arguments)
        .output()
        .expect("PowerShell should launch")
}

#[test]
fn generated_runtime_fixture_exceeds_the_64k_prototype_boundary() {
    let fixture = test_directory("large-prototypes");
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts/new-large-prototype-fixture.ps1");
    let output_directory = fixture.to_str().unwrap();
    let output = run_pwsh(
        &script,
        &[
            "-OutputDirectory",
            output_directory,
            "-PrototypeCount",
            "65537",
        ],
    );
    assert!(
        output.status.success(),
        "fixture generation failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let source = std::fs::read_to_string(fixture.join("large_prototypes.dm")).unwrap();
    let prototypes = source
        .lines()
        .filter(|line| {
            line.starts_with("/datum/meridian_large_prototype/b") && line.contains("/p")
        })
        .collect::<HashSet<_>>();
    assert_eq!(prototypes.len(), 65_537);
    let parent_buckets = prototypes
        .iter()
        .filter_map(|path| path.rsplit_once('/').map(|(parent, _)| parent))
        .collect::<HashSet<_>>();
    assert!(parent_buckets.len() > 1);
    assert!(source.contains("MERIDIAN_LARGE_PROTOTYPE_READY"));
    assert!(source.contains("text2file(\"MERIDIAN_LARGE_PROTOTYPE_READY\", \"startup.marker\")"));
    assert!(fixture.join("large_prototypes.dme").is_file());
    std::fs::remove_dir_all(fixture).unwrap();
}

#[test]
fn auxtools_runtime_check_reports_missing_x86_crt_files() {
    let runtime = test_directory("auxtools-runtime");
    for name in ["MSVCP140.dll", "VCRUNTIME140.dll"] {
        std::fs::write(runtime.join(name), b"technical fixture").unwrap();
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts/install-auxtools-runtime.ps1");
    let runtime_directory = runtime.to_str().unwrap();
    let valid = run_pwsh(&script, &["-RuntimeDirectory", runtime_directory]);
    assert!(
        valid.status.success(),
        "complete runtime was rejected: {}{}",
        String::from_utf8_lossy(&valid.stdout),
        String::from_utf8_lossy(&valid.stderr)
    );

    std::fs::remove_file(runtime.join("MSVCP140.dll")).unwrap();
    let missing_installer = runtime.join("missing-vc_redist.x86.exe");
    let invalid = run_pwsh(
        &script,
        &[
            "-RuntimeDirectory",
            runtime_directory,
            "-InstallerPath",
            missing_installer.to_str().unwrap(),
        ],
    );
    assert!(!invalid.status.success());
    assert!(
        String::from_utf8_lossy(&invalid.stderr).contains("MSVCP140.dll"),
        "missing runtime error did not identify MSVCP140.dll: {}",
        String::from_utf8_lossy(&invalid.stderr)
    );
    std::fs::remove_dir_all(runtime).unwrap();
}
