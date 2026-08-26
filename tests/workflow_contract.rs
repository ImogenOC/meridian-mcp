use serde_json::Value;
use std::fs;

#[test]
fn byond_workflow_runs_the_versioned_meridian_compatibility_gate() {
    let workflow = fs::read_to_string(".github/workflows/byond-integration.yml")
        .expect("BYOND integration workflow should be readable");

    for required in [
        "workflow_dispatch:",
        "meridian_ref:",
        "schedule:",
        "runs-on: windows-latest",
        "actions/checkout@v7",
        "AphelionDevelopment/Meridian-Rift",
        "path: integration/Meridian-Rift",
        "scripts/install-byond.ps1",
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
fn byond_workflow_uses_the_516_1687_runtime_baseline() {
    let workflow = fs::read_to_string(".github/workflows/byond-integration.yml")
        .expect("BYOND integration workflow should be readable");

    for required in [
        "516.1687",
        "6A69818D8216E089D5C16506659A8883D8CCF06A673A2DD9F7C0777E81BCD9A4",
        "8F43564407BB3117827F6727A6192ECAFFA3538AF76742B2FCD083F1CCCF4D8A",
        "scripts/install-auxtools-runtime.ps1",
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
