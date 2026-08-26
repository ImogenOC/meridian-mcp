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
	if ($result.elapsed_milliseconds -lt 200) {{ throw 'Readiness elapsed time was not recorded.' }}
	if (@($result.samples).Count -lt 1) {{ throw 'Readiness samples were not recorded.' }}
	if ($null -eq $result.last_progress_milliseconds) {{ throw 'Last progress time was not recorded.' }}
	if ($null -ne $result.process_exit_code) {{ throw 'A running helper reported an exit code.' }}
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
	if (@($result.samples).Count -lt 1) {{ throw 'Launcher readiness samples were not recorded.' }}
	if ($null -eq $result.process_exit_code) {{ throw 'Exited launcher did not report an exit code.' }}
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

#[test]
fn readiness_timeout_retains_progress_samples_for_a_busy_process() {
    let root = std::env::temp_dir().join(format!(
        "meridian-progress-readiness-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let busy_child = root.join("busy-child.ps1");
    let marker = root.join("startup.marker");
    std::fs::write(
        &busy_child,
        "$value = 0\nwhile ($true) { $value = ($value + 1) % 1000000 }\n",
    )
    .unwrap();

    let module = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/process-readiness.psm1");
    let harness = root.join("harness.ps1");
    std::fs::write(
        &harness,
        format!(
            r#"$ErrorActionPreference = 'Stop'
Import-Module -Force '{}'
$child = Start-Process -FilePath (Get-Process -Id $PID).Path -ArgumentList @('-NoLogo', '-NoProfile', '-File', '{}') -PassThru
try {{
	$result = Wait-ProcessReadiness -Process $child -MarkerPath '{}' -ExpectedMarker 'READY' -TimeoutSeconds 2
	if ($result.status -ne 'timed_out') {{ throw "Unexpected status: $($result.status)" }}
	if ($result.elapsed_milliseconds -lt 1900) {{ throw 'Timeout elapsed time was not recorded.' }}
	if (@($result.samples).Count -lt 2) {{ throw 'Timeout did not retain bounded progress samples.' }}
	if ($result.last_progress_milliseconds -le 0) {{ throw 'Busy process progress was not detected.' }}
	if ($null -ne $result.process_exit_code) {{ throw 'Busy process reported an exit code.' }}
}} finally {{
	Stop-Process -Id $child.Id -Force -ErrorAction SilentlyContinue
}}
"#,
            module.display(),
            busy_child.display(),
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
fn prototype_runtime_classification_distinguishes_control_boundary_and_progress() {
    let root = std::env::temp_dir().join(format!(
        "meridian-runtime-classification-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let module = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/process-readiness.psm1");
    let harness = root.join("harness.ps1");
    std::fs::write(
        &harness,
        format!(
            r#"$ErrorActionPreference = 'Stop'
Import-Module -Force '{}'
$cases = @(
	@{{ expected = 'passed'; arguments = @{{ RuntimeCase = 'boundary'; CompileSucceeded = $true; MarkerReady = $true; ReadinessStatus = 'ready'; HasProcessProgress = $true; ControlPassed = $true }} }},
	@{{ expected = 'compile_failure'; arguments = @{{ RuntimeCase = 'control'; CompileSucceeded = $false; MarkerReady = $false; ReadinessStatus = 'not_started'; HasProcessProgress = $false; ControlPassed = $false }} }},
	@{{ expected = 'environment_failure'; arguments = @{{ RuntimeCase = 'control'; CompileSucceeded = $true; MarkerReady = $false; ReadinessStatus = 'timed_out'; HasProcessProgress = $true; ControlPassed = $false }} }},
	@{{ expected = 'boundary_regression'; arguments = @{{ RuntimeCase = 'boundary'; CompileSucceeded = $true; MarkerReady = $false; ReadinessStatus = 'timed_out'; HasProcessProgress = $false; ControlPassed = $true }} }},
	@{{ expected = 'inconclusive_timeout'; arguments = @{{ RuntimeCase = 'boundary'; CompileSucceeded = $true; MarkerReady = $false; ReadinessStatus = 'timed_out'; HasProcessProgress = $true; ControlPassed = $true }} }},
	@{{ expected = 'boundary_regression'; arguments = @{{ RuntimeCase = 'boundary'; CompileSucceeded = $true; MarkerReady = $false; ReadinessStatus = 'process_exited'; HasProcessProgress = $false; ControlPassed = $true }} }}
)
foreach ($case in $cases) {{
	$arguments = $case.arguments
	$actual = Get-PrototypeRuntimeClassification @arguments
	if ($actual -ne $case.expected) {{ throw "Expected $($case.expected), got $actual." }}
}}
"#,
            module.display()
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
        "classification helper failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn public_fixture_evidence_excludes_generated_host_paths() {
    let root = std::env::temp_dir().join(format!(
        "meridian-fixture-evidence-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let module = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/process-readiness.psm1");
    let harness = root.join("harness.ps1");
    std::fs::write(
        &harness,
        format!(
            r#"$ErrorActionPreference = 'Stop'
Import-Module -Force '{}'
$privateRoot = 'C:\Users\Private\fixture'
$inputEvidence = @{{
	layout = 'bucketed'
	declared_leaf_count = 65537
	declared_parent_count = 258
	declared_type_count = 65538
	first_path = '/datum/mlp/p00000'
	boundary_path = '/datum/mlp/p65535'
	last_path = '/datum/mlp/p65536'
	source = "$privateRoot\large_prototypes.dm"
	environment = "$privateRoot\large_prototypes.dme"
}}
$public = ConvertTo-PublicPrototypeFixtureEvidence $inputEvidence
$json = $public | ConvertTo-Json -Compress
if ($json.Contains($privateRoot)) {{ throw 'Public fixture evidence retained a host path.' }}
$keys = @($public.Keys | Sort-Object)
$expected = @('boundary_path', 'declared_leaf_count', 'declared_parent_count', 'declared_type_count', 'first_path', 'last_path', 'layout')
if ([string]::Join(',', $keys) -ne [string]::Join(',', $expected)) {{ throw "Unexpected keys: $($keys -join ',')" }}
"#,
            module.display()
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
        "fixture evidence helper failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}
