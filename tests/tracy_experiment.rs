use meridian_mcp::tracy_experiment::*;
use std::collections::BTreeMap;

fn executable() -> ExecutableIdentity {
    finalize_executable(ExecutableIdentity {
        schema: 1,
        executable_id: String::new(),
        repository_revision: Some("0123456789abcdef".into()),
        repository_dirty_digest: "11".repeat(32),
        dmb_sha256: "22".repeat(32),
        rsc_sha256: Some("33".repeat(32)),
        byond_version: "516.1687".into(),
        byond_executable_sha256: "44".repeat(32),
        native_modules: vec![NativeModuleIdentity {
            name: "prof.dll".into(),
            sha256: "55".repeat(32),
        }],
        helper_identity: HelperIdentity {
            source_revision: "tracy-revision".into(),
            sha256: "66".repeat(32),
            patch_sha256: Some("77".repeat(32)),
        },
        hook_identity: HelperIdentity {
            source_revision: "hook-revision".into(),
            sha256: "88".repeat(32),
            patch_sha256: Some("99".repeat(32)),
        },
        startup_mode: "trusted".into(),
        launch_parameters_sha256: "aa".repeat(32),
    })
    .unwrap()
}

#[test]
fn canonical_identity_is_stable_and_separates_executable_from_workload() {
    let first = workload_identity(WorkloadInput {
        annotations: BTreeMap::from([("alpha".into(), "1".into()), ("beta".into(), "2".into())]),
        ..Default::default()
    })
    .unwrap();
    let second = workload_identity(WorkloadInput {
        annotations: BTreeMap::from([("beta".into(), "2".into()), ("alpha".into(), "1".into())]),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(first, second);
    let base = executable();
    let changed_workload = workload_identity(WorkloadInput {
        scenario: Some("alternate".into()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(base.executable_id, executable().executable_id);
    assert_ne!(
        experiment_identity(base.clone(), first)
            .unwrap()
            .experiment_id,
        experiment_identity(base.clone(), changed_workload)
            .unwrap()
            .experiment_id
    );
    let mut changed = base.clone();
    changed.dmb_sha256 = "ff".repeat(32);
    assert_ne!(
        base.executable_id,
        finalize_executable(changed).unwrap().executable_id
    );
}

#[test]
fn workload_inputs_are_bounded_and_reject_host_specific_values() {
    for unsafe_value in [
        "line\nbreak",
        "C:\\private\\file",
        "/private/file",
        "$env:SECRET",
        "%SECRET%",
    ] {
        assert!(validate_workload(WorkloadInput {
            scenario: Some(unsafe_value.into()),
            ..Default::default()
        })
        .is_err());
    }
    assert!(validate_workload(WorkloadInput {
        annotations: BTreeMap::from([("Bad-Key".into(), "value".into())]),
        ..Default::default()
    })
    .is_err());
    let too_many = (0..33)
        .map(|index| (format!("key_{index}"), "value".into()))
        .collect();
    assert_eq!(
        validate_workload(WorkloadInput {
            annotations: too_many,
            ..Default::default()
        })
        .unwrap_err(),
        ExperimentError::TooManyAnnotations
    );
}

#[test]
fn first_capture_binds_only_omitted_workload_and_later_capture_is_immutable() {
    let draft = WorkloadInput {
        map: Some("station".into()),
        ..Default::default()
    };
    let capture = WorkloadInput {
        scenario: Some("steady_state".into()),
        ..Default::default()
    };
    let locked = bind_workload(&draft, &capture).unwrap();
    assert_eq!(locked.map.as_deref(), Some("station"));
    assert_eq!(locked.scenario.as_deref(), Some("steady_state"));
    assert!(verify_locked_workload(&locked, &WorkloadInput::default()).is_ok());
    assert!(verify_locked_workload(
        &locked,
        &WorkloadInput {
            map: Some("station".into()),
            ..Default::default()
        }
    )
    .is_ok());
    assert!(verify_locked_workload(
        &locked,
        &WorkloadInput {
            scenario: Some("different".into()),
            ..Default::default()
        }
    )
    .is_err());
    assert!(bind_workload(
        &draft,
        &WorkloadInput {
            map: Some("other".into()),
            ..Default::default()
        }
    )
    .is_err());
}

#[test]
fn launch_manifest_carries_the_exact_meridian_mcp_build_identity() {
    let manifest = ExperimentLaunchManifest {
        schema: 1,
        experiment_name: Some("fixture".to_owned()),
        meridian_mcp_build: meridian_mcp::build_identity::current().clone(),
        executable: executable(),
        workload_draft: WorkloadInput::default(),
    };
    let value = serde_json::to_value(manifest).unwrap();
    assert_eq!(
        value["meridian_mcp_build"]["build_id"],
        meridian_mcp::build_identity::current().build_id
    );
}
