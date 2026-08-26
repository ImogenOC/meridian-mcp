use std::path::Path;
use std::process::Command;

#[test]
fn readiness_marker_wins_while_the_launched_process_is_still_running() {
    let root = std::env::temp_dir().join(format!(
        "meridian-process-readiness-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let child = root.join("delayed-marker.ps1");
    let marker = root.join("startup.marker");
    std::fs::write(
        &child,
        "param([string]$Marker)\nStart-Sleep -Milliseconds 300\n[IO.File]::WriteAllText($Marker, 'READY')\nStart-Sleep -Seconds 5\n",
    )
    .unwrap();

    let module = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/process-readiness.psm1");
    let harness = root.join("harness.ps1");
    std::fs::write(
        &harness,
        format!(
            r#"$ErrorActionPreference = 'Stop'
Import-Module -Force '{}'
$child = Start-Process -FilePath (Get-Process -Id $PID).Path -ArgumentList @('-NoLogo', '-NoProfile', '-File', '{}', '{}') -PassThru
try {{
	$result = Wait-ProcessReadiness -Process $child -MarkerPath '{}' -ExpectedMarker 'READY' -TimeoutSeconds 3
	$child.Refresh()
	if ($result.status -ne 'ready') {{ throw "Unexpected status: $($result.status)" }}
	if ($child.HasExited) {{ throw 'The helper waited for process exit instead of readiness.' }}
}} finally {{
	Stop-Process -Id $child.Id -Force -ErrorAction SilentlyContinue
}}
"#,
            module.display(),
            child.display(),
            marker.display(),
            marker.display()
        ),
    )
    .unwrap();

    let output = Command::new("pwsh")
        .args(["-NoLogo", "-NoProfile", "-File"])
        .arg(&harness)
        .output()
        .expect("PowerShell should launch");
    assert!(
        output.status.success(),
        "readiness helper failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn readiness_marker_can_arrive_after_the_launcher_exits() {
    let root = std::env::temp_dir().join(format!(
        "meridian-launcher-readiness-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let marker_writer = root.join("delayed-marker.ps1");
    let marker = root.join("startup.marker");
    std::fs::write(
        &marker_writer,
        "param([string]$Marker)\nStart-Sleep -Milliseconds 500\n[IO.File]::WriteAllText($Marker, 'READY')\nStart-Sleep -Seconds 5\n",
    )
    .unwrap();

    let module = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/process-readiness.psm1");
    let harness = root.join("harness.ps1");
    std::fs::write(
        &harness,
        format!(
            r#"$ErrorActionPreference = 'Stop'
Import-Module -Force '{}'
$markerWriter = Start-Process -FilePath (Get-Process -Id $PID).Path -ArgumentList @('-NoLogo', '-NoProfile', '-File', '{}', '{}') -PassThru
$launcher = Start-Process -FilePath (Get-Process -Id $PID).Path -ArgumentList @('-NoLogo', '-NoProfile', '-Command', 'Start-Sleep -Milliseconds 100') -PassThru
try {{
	$result = Wait-ProcessReadiness -Process $launcher -MarkerPath '{}' -ExpectedMarker 'READY' -TimeoutSeconds 3
	if ($result.status -ne 'ready') {{ throw "Unexpected status: $($result.status)" }}
}} finally {{
	Stop-Process -Id $launcher.Id -Force -ErrorAction SilentlyContinue
	Stop-Process -Id $markerWriter.Id -Force -ErrorAction SilentlyContinue
}}
"#,
            module.display(),
            marker_writer.display(),
            marker.display(),
            marker.display()
        ),
    )
    .unwrap();

    let output = Command::new("pwsh")
        .args(["-NoLogo", "-NoProfile", "-File"])
        .arg(&harness)
        .output()
        .expect("PowerShell should launch");
    assert!(
        output.status.success(),
        "readiness helper failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}
