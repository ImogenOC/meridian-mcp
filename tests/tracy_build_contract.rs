use std::path::Path;

#[test]
fn tracy_builder_is_pinned_dual_arch_and_offline_with_owned_native_tests() {
    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/build-tracy-helpers.ps1");
    let script = std::fs::read_to_string(script_path).expect("Tracy build script should exist");

    for required in [
        "099df3de3dc37eca4712c06b8320fb9c53596edd",
        "d1ec404737b04b1ea73d6df4a1b477deacdb1900",
        "protocol_version = 82",
        "target_arch = 'x86_64'",
        "target_arch = 'x86'",
        "meridian-tracy-helper",
        "prof.dll",
        "libprof.so",
        "-R '^meridian_'",
        "byond-tracy-empty-queue.patch",
        "git apply",
    ] {
        assert!(
            script.contains(required),
            "missing build contract: {required}"
        );
    }
    for forbidden in ["git clone", "Invoke-WebRequest", "curl ", "wget "] {
        assert!(
            !script.contains(forbidden),
            "builder must not fetch sources: {forbidden}"
        );
    }
}
