use serde_json::Value;
use std::fs;
use std::path::Path;

fn workflow_job_block<'a>(workflow: &'a str, job_name: &str) -> &'a str {
    let header = format!("  {job_name}:");
    let start = workflow
        .match_indices(&header)
        .find_map(|(offset, _)| {
            (offset == 0 || workflow.as_bytes()[offset - 1] == b'\n').then_some(offset)
        })
        .unwrap_or_else(|| panic!("workflow is missing job {job_name}"));
    let body_start = start + header.len();
    let mut cursor = body_start;
    for line in workflow[body_start..].split_inclusive('\n') {
        if line.starts_with("  ") && !line.starts_with("    ") && line.trim_end().ends_with(':') {
            return &workflow[start..cursor];
        }
        cursor += line.len();
    }
    &workflow[start..]
}

#[test]
fn workflow_job_block_accepts_crlf() {
    let workflow = "jobs:\r\n  example:\r\n    runs-on: windows-2025\r\n  following:\r\n    runs-on: ubuntu-24.04\r\n";
    let block = workflow_job_block(workflow, "example");

    assert!(block.contains("runs-on: windows-2025"));
    assert!(!block.contains("following:"));
}

#[test]
fn byond_workflow_keeps_product_parser_and_runtime_claims_independent() {
    let workflow = fs::read_to_string(".github/workflows/byond-integration.yml")
        .expect("BYOND integration workflow should be readable");

    let windows = workflow_job_block(&workflow, "windows-meridian-compatibility");
    for required in [
        "scripts/run-byond-integration.ps1",
        "scripts/run-auxtools-integration.ps1",
        "scripts/run-tracy-integration.ps1",
    ] {
        assert!(
            windows.contains(required),
            "Windows product job is missing {required}"
        );
    }
    assert!(
        !windows.contains("needs:"),
        "Windows product evidence must not depend on a synthetic gate"
    );
    assert!(windows.contains("id: auxtools_gate"));
    assert!(windows.contains("id: tracy_gate"));
    assert!(windows.contains("-HostMode headless"));
    assert!(windows.contains("AUXTOOLS_OUTCOME: ${{ steps.auxtools_gate.outcome }}"));
    assert!(windows.contains("TRACY_OUTCOME: ${{ steps.tracy_gate.outcome }}"));
    assert!(windows.contains("$env:AUXTOOLS_OUTCOME -eq 'failure'"));
    assert!(windows.contains("$env:TRACY_OUTCOME -eq 'failure'"));

    let parser = workflow_job_block(&workflow, "prototype-parser-compatibility");
    assert!(parser.contains("windows-2025"));
    assert!(parser.contains("ubuntu-24.04"));
    assert!(parser.contains("scripts/run-large-prototype-parser-integration.ps1"));

    let runtime = workflow_job_block(&workflow, "prototype-runtime-compatibility");
    assert!(runtime.contains("os: ubuntu-24.04"));
    assert!(runtime.contains("required: true"));
    assert!(runtime.contains("os: windows-2025"));
    assert!(runtime.contains("required: false"));
    assert!(runtime.contains("continue-on-error: ${{ !matrix.required }}"));
    assert!(runtime.contains("id: control_runtime"));
    assert!(runtime
        .contains("if: ${{ matrix.required || steps.control_runtime.outcome == 'success' }}"));
    assert!(
        runtime.contains("::warning::Windows prototype runtime diagnostic did not reach readiness")
    );
    assert!(runtime
        .contains("prototype-runtime/${{ matrix.artifact }}/prerequisites/byond-runtime.json"));
    assert!(runtime.contains("-PrerequisiteEvidencePath $prerequisiteEvidence"));
    assert_eq!(
        runtime.matches("-ExpectedByondVersion 516.1687").count(),
        2,
        "both runtime cases must declare the verified BYOND version"
    );
}

#[test]
fn byond_workflow_runs_the_versioned_meridian_compatibility_gate() {
    let workflow = fs::read_to_string(".github/workflows/byond-integration.yml")
        .expect("BYOND integration workflow should be readable");

    for required in [
        "workflow_dispatch:",
        "meridian_ref:",
        "schedule:",
        "runs-on: windows-2025",
        "actions/checkout@v7",
        "AphelionDevelopment/Meridian-Rift",
        "path: integration/Meridian-Rift",
        "scripts/install-byond.ps1",
        "scripts/install-byond-runtime.ps1",
        "cargo build --locked --release",
        "scripts/run-byond-integration.ps1",
        "if: always()",
        "actions/upload-artifact@v6",
    ] {
        assert!(
            workflow.contains(required),
            "workflow is missing {required}"
        );
    }

    let integration_script = fs::read_to_string("scripts/run-byond-integration.ps1")
        .expect("BYOND integration script should be readable");
    assert!(integration_script.contains("run-meridian-compatibility.ps1"));
    let compatibility_script = fs::read_to_string("scripts/run-meridian-compatibility.ps1")
        .expect("Meridian compatibility harness should be readable");
    for required in [
        "$startInfo.Environment['DM_EXE'] = $DreamMakerPath",
        "Invoke-HumanBuild -Root $MeridianRiftRoot -DreamMakerPath $DreamMakerPath",
        "Warm BUILD.cmd stdout",
        "$evidence.builds['human_warm']",
    ] {
        assert!(
            compatibility_script.contains(required),
            "compatibility harness is missing {required}"
        );
    }

    let installer = fs::read_to_string("scripts/install-byond.ps1")
        .expect("BYOND installer should be readable");
    for required in [
        "Invoke-WebRequest",
        "Expand-Archive",
        "MaxAttempts",
        "ExpectedSha256",
        "Get-FileHash",
        "byond-builds.dm-lang.org",
        "tgstation/1.0 CI Script",
        "dm.exe",
    ] {
        assert!(
            installer.contains(required),
            "BYOND installer is missing {required}"
        );
    }
}

#[test]
fn tracy_gates_require_persistent_rotation_and_independent_native_platforms() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let live = std::fs::read_to_string(root.join("scripts/run-tracy-integration.ps1")).unwrap();
    let native = std::fs::read_to_string(root.join("scripts/run-tracy-native-tests.ps1")).unwrap();
    let ci = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    for required in [
        "delayed-first-capture marker",
        "$delay_seconds = 120",
        "Start-Sleep -Seconds $delay_seconds",
        "duration_ms = 30000",
        "delayed_first_capture_seconds = $delay_seconds",
        "capture_duration_ms = $duration_ms",
        "capture_count = 3",
        ".tracy.meridian.json",
        "queue.saturation_count",
        "queue.dropped_events",
        "repository_integrity",
    ] {
        assert!(
            live.contains(required),
            "missing live Tracy gate: {required}"
        );
    }
    assert!(native.contains("platform = if ($IsWindows) { 'windows' } else { 'ubuntu' }"));
    assert!(ci.contains("scripts/run-tracy-native-tests.ps1"));
    assert!(ci.contains("name: Windows"));
    assert!(ci.contains("name: Ubuntu 24.04"));
}

#[test]
fn tracy_experiment_runner_is_bounded_and_raw_traces_are_not_uploaded() {
    let runner = fs::read_to_string("scripts/run-tracy-experiment.ps1").unwrap();
    let validator = fs::read_to_string("scripts/validate-tracy-evidence.ps1").unwrap();
    let workflow = fs::read_to_string(".github/workflows/byond-integration.yml").unwrap();
    for required in [
        "[ValidateRange(3, 20)] [int] $ControlCount = 5",
        "[ValidateRange(5, 300)] [int] $CaptureSeconds = 30",
        "dm_tracy_prepare",
        "dm_tracy_launch",
        "dm_tracy_status",
        "dm_tracy_capture",
        "dm_tracy_frame_stats",
        "dm_tracy_control_stats",
        "dm_tracy_stop",
        "experiment.json",
        "control-stats.json",
        "evidence-index.json",
        "raw_traces_local_only",
    ] {
        assert!(
            runner.contains(required),
            "experiment runner is missing {required}"
        );
    }
    for required in [
        "Get-FileHash",
        "trace_begin_ns",
        "dream_daemon",
        "collector",
        "network_isolation_confirmed",
        "capture_complete",
    ] {
        assert!(
            validator.contains(required),
            "evidence validator is missing {required}"
        );
    }
    assert!(workflow.contains("!**/*.tracy"));
}

#[test]
fn byond_workflow_uses_the_516_1687_runtime_baseline() {
    let workflow = fs::read_to_string(".github/workflows/byond-integration.yml")
        .expect("BYOND integration workflow should be readable");

    for required in [
        "516.1687",
        "6A69818D8216E089D5C16506659A8883D8CCF06A673A2DD9F7C0777E81BCD9A4",
        "8F43564407BB3117827F6727A6192ECAFFA3538AF76742B2FCD083F1CCCF4D8A",
        "scripts/install-auxtools-runtime.ps1",
        "Microsoft.DXSDK.D3DX",
        "9.29.952.8",
        "ead0906ae8a26c18a7525da7490127a2110f7c58f18293738283e30e97c6ea4b",
        "scripts/run-large-prototype-integration.ps1",
        "scripts/run-auxtools-integration.ps1",
        "-DmbPath ./integration/Meridian-Rift/tgstation.dmb",
        "cargo clean",
    ] {
        assert!(
            workflow.contains(required),
            "BYOND 516.1687 workflow contract is missing {required}"
        );
    }
    assert!(
        !workflow.contains("Install BYOND 516.1685"),
        "the workflow still names the obsolete BYOND baseline"
    );
}

#[test]
fn byond_runtime_and_large_prototype_failures_retain_diagnostics() {
    let installer = fs::read_to_string("scripts/install-byond-runtime.ps1")
        .expect("BYOND runtime installer should be readable");
    for required in [
        "Microsoft.DXSDK.D3DX",
        "9.29.952.8",
        "ead0906ae8a26c18a7525da7490127a2110f7c58f18293738283e30e97c6ea4b",
        "D3DX9_43.dll",
        "mfc140u.dll",
        "LICENSE.txt",
        "NOTICE.md",
    ] {
        assert!(
            installer.contains(required),
            "BYOND runtime installer is missing {required}"
        );
    }

    let gate = fs::read_to_string("scripts/run-large-prototype-integration.ps1")
        .expect("large prototype gate should be readable");
    for required in [
        "launcher_exit_code_signed",
        "launcher_exit_code_hex",
        "prerequisites",
        "dreammaker",
        "dreamdaemon",
        "marker_state",
        "owned_processes",
        "retained_fixture_id",
    ] {
        assert!(
            gate.contains(required),
            "large prototype failure evidence is missing {required}"
        );
    }

    let workflow = fs::read_to_string(".github/workflows/byond-integration.yml")
        .expect("BYOND integration workflow should be readable");
    for required in [
        "if: always()",
        "prototype-runtime-${{ matrix.artifact }}-evidence",
        "integration/evidence/prototype-runtime/${{ matrix.artifact }}/**",
    ] {
        assert!(
            workflow.contains(required),
            "BYOND workflow failure evidence is missing {required}"
        );
    }
}

#[test]
fn byond_workflow_defaults_to_a_rift_build_qualified_ref() {
    let workflow = fs::read_to_string(".github/workflows/byond-integration.yml")
        .expect("BYOND integration workflow should be readable");

    for required in [
        "default: aphelion-agents",
        "ref: ${{ inputs.meridian_ref || 'aphelion-agents' }}",
        "Verify Meridian-Rift full-build qualification inputs",
        "'RIFT_BUILD.cmd'",
    ] {
        assert!(
            workflow.contains(required),
            "BYOND workflow can select an unqualified Meridian-Rift ref: missing {required}"
        );
    }
}

#[test]
fn ubuntu_meridian_analysis_is_real_repository_and_byond_free() {
    let workflow = fs::read_to_string(".github/workflows/ci.yml").unwrap();
    for required in [
        "ubuntu-meridian-analysis:",
        "runs-on: ubuntu-24.04",
        "AphelionDevelopment/Meridian-Rift",
        "SpaceManiac/SpacemanDMM",
        "scripts/build-spacemandmm-helpers.ps1",
        "scripts/run-meridian-analysis-compatibility.ps1",
        "ubuntu-meridian-analysis-evidence",
    ] {
        assert!(
            workflow.contains(required),
            "Ubuntu analysis workflow is missing {required}"
        );
    }
    let script = fs::read_to_string("scripts/run-meridian-analysis-compatibility.ps1").unwrap();
    for forbidden in [
        "dm.exe",
        "DreamDaemon",
        "DreamSeeker",
        "BUILD.cmd",
        "RIFT_BUILD.cmd",
        "fetch-auxtools",
    ] {
        assert!(
            !script.contains(forbidden),
            "Ubuntu analysis script contains forbidden BYOND operation {forbidden}"
        );
    }
}

#[test]
fn repository_pins_the_ci_rust_toolchain() {
    let toolchain = fs::read_to_string("rust-toolchain.toml")
        .expect("repository Rust toolchain should be pinned");
    assert!(toolchain.contains("channel = \"1.95.0\""));
    assert!(toolchain.contains("components = [\"rustfmt\", \"clippy\"]"));
}

#[test]
fn portable_ci_audits_the_checked_in_spacemandmm_capability_registry() {
    let workflow = fs::read_to_string(".github/workflows/ci.yml")
        .expect("portable CI workflow should be readable");

    assert!(workflow.contains("scripts/audit-spacemandmm-capabilities.ps1"));
    assert!(workflow.contains("-Check"));
}

#[test]
fn portable_ci_tests_the_unsupported_platform_boundary_without_byond() {
    let workflow = fs::read_to_string(".github/workflows/ci.yml")
        .expect("portable CI workflow should be readable");

    assert!(workflow.contains("MERIDIAN_MCP_RIFT_BUILD: network"));
    assert!(workflow.contains("test_unsupported_rift_compile.ps1"));
    for forbidden in ["Install BYOND", "byond.com/download", "RIFT_BUILD.cmd"] {
        assert!(
            !workflow.contains(forbidden),
            "portable CI must not attempt BYOND or the Windows build wrapper: {forbidden}"
        );
    }
}

#[test]
fn aphelion_workflow_requires_parse_before_map_and_diagnostics() {
    let text = fs::read_to_string("tests/compatibility/aphelion-dmm.json")
        .expect("AphelionDMM compatibility manifest should be readable");
    let manifest: Value = serde_json::from_str(&text)
        .expect("AphelionDMM compatibility manifest should be valid JSON");
    assert_eq!(
        manifest["required_sequence"],
        serde_json::json!(["dm_parse_environment", "dm_map_info", "dm_check_errors"])
    );
    assert_eq!(manifest["parse_required_after_source_change"], true);
}
