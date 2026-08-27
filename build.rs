use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=MERIDIAN_BUILD_REVISION");
    println!("cargo:rerun-if-env-changed=MERIDIAN_BUILD_DIRTY");
    for git_path in ["HEAD", "index"] {
        if let Some(path) = git_output(&["rev-parse", "--git-path", git_path]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    let revision = std::env::var("MERIDIAN_BUILD_REVISION")
        .ok()
        .or_else(|| git_output(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_owned());
    let dirty = std::env::var("MERIDIAN_BUILD_DIRTY")
        .ok()
        .unwrap_or_else(|| {
            git_output(&["status", "--porcelain", "--untracked-files=all"])
                .map(|status| if status.is_empty() { "false" } else { "true" }.to_owned())
                .unwrap_or_else(|| "unknown".to_owned())
        });
    println!("cargo:rustc-env=MERIDIAN_BUILD_REVISION={revision}");
    println!("cargo:rustc-env=MERIDIAN_BUILD_DIRTY={dirty}");
    println!(
        "cargo:rustc-env=MERIDIAN_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned())
    );
    println!(
        "cargo:rustc-env=MERIDIAN_BUILD_PROFILE={}",
        std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_owned())
    );
}

fn git_output(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git").args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
