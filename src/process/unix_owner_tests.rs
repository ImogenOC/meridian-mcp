use super::*;
use std::os::unix::fs::PermissionsExt;

fn executable() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("meridian-mcp")
}

fn identity(pid: u32) -> (u32, u64) {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    (
        pid,
        stat.rsplit_once(") ")
            .unwrap()
            .1
            .split_whitespace()
            .nth(19)
            .unwrap()
            .parse()
            .unwrap(),
    )
}

fn running((pid, started): (u32, u64)) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    let fields: Vec<_> = stat
        .rsplit_once(") ")
        .unwrap()
        .1
        .split_whitespace()
        .collect();
    fields[19].parse::<u64>().unwrap() == started && !matches!(fields[0], "Z" | "X")
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore]
fn owner_fixture() {
    let Ok(mode) = std::env::var("MERIDIAN_UNIX_OWNER_FIXTURE") else {
        return;
    };
    let owner = RuntimeOwner::start(&executable()).unwrap();
    let mut identities = vec![identity(owner.group() as u32)];
    if mode == "prejoin" {
        // Test-only deterministic fork pause before group assignment. All values
        // are prepared before fork and the child uses only signal-safe syscalls.
        let program = std::ffi::CString::new("/bin/sleep").unwrap();
        let argument = std::ffi::CString::new("30").unwrap();
        let arguments = [program.as_ptr(), argument.as_ptr(), std::ptr::null()];
        let environment = [std::ptr::null::<libc::c_char>()];
        let group = owner.group();
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            unsafe {
                libc::raise(libc::SIGSTOP);
                if libc::setpgid(0, group) != 0 {
                    libc::_exit(2);
                }
                libc::execve(program.as_ptr(), arguments.as_ptr(), environment.as_ptr());
                libc::_exit(3);
            }
        }
        identities.push(identity(pid as u32));
    } else {
        let sentinel = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        identities.push(identity(sentinel.id()));
        // Deliberately leave this unrelated execed process alive after owner loss.
        drop(sentinel);
    }
    std::fs::write(
        std::env::var_os("MERIDIAN_UNIX_OWNER_MARKER").unwrap(),
        serde_json::to_vec(&identities).unwrap(),
    )
    .unwrap();
    std::thread::sleep(Duration::from_secs(30));
    drop(owner);
}

#[test]
fn inherited_lease_covers_prejoin_owner_loss_and_exec_does_not_retain_it() {
    for mode in ["prejoin", "execed_sentinel"] {
        let marker =
            std::env::temp_dir().join(format!("meridian-unix-{}-{mode}.json", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let mut fixture = ChildGuard(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "process::unix_owner::tests::owner_fixture",
                    "--nocapture",
                ])
                .env("MERIDIAN_UNIX_OWNER_FIXTURE", mode)
                .env("MERIDIAN_UNIX_OWNER_MARKER", &marker)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(8);
        while !marker.exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        let identities: Vec<(u32, u64)> =
            serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
        if mode == "prejoin" {
            loop {
                let stat =
                    std::fs::read_to_string(format!("/proc/{}/stat", identities[1].0)).unwrap();
                if stat.rsplit_once(") ").unwrap().1.starts_with("T ") {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        fixture.0.kill().unwrap();
        fixture.0.wait().unwrap();
        if mode == "prejoin" {
            std::thread::sleep(Duration::from_millis(150));
            assert!(
                running(identities[0]),
                "guardian saw EOF before target joined"
            );
            unsafe {
                libc::kill(identities[1].0 as libc::pid_t, libc::SIGCONT);
            }
        }
        while running(identities[0]) || (mode == "prejoin" && running(identities[1])) {
            assert!(
                Instant::now() < deadline,
                "owned identity survived owner loss: {identities:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        if mode == "execed_sentinel" {
            let alive = running(identities[1]);
            unsafe {
                libc::kill(identities[1].0 as libc::pid_t, libc::SIGKILL);
            }
            assert!(alive, "unrelated sentinel was terminated");
        }
        eprintln!("{mode}: owner SIGKILL cleanup confirmed {identities:?}");
        std::fs::remove_file(marker).unwrap();
    }
}

#[tokio::test]
async fn guardian_setup_and_target_exec_fail_closed() {
    let root = std::env::temp_dir().join(format!("meridian-unix-failure-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let target_marker = root.join("target");
    let mut target = tokio::process::Command::new("/usr/bin/touch");
    target.arg(&target_marker);
    assert!(spawn_with_executable(&mut target, &root.join("missing")).is_err());
    for (name, body) in [
        ("bad", "printf x; sleep 30"),
        ("eof", "exit 1"),
        ("timeout", "sleep 30"),
    ] {
        let script = root.join(name);
        let pidfile = root.join(format!("{name}.pid"));
        std::fs::write(
            &script,
            format!("#!/bin/sh\necho $$ > '{}'\n{body}\n", pidfile.display()),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(spawn_with_executable(&mut target, &script).is_err());
        let pid: u32 = std::fs::read_to_string(pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "failed guardian was not reaped"
        );
    }
    assert!(!target_marker.exists(), "failed setup executed target code");
    let mut missing = tokio::process::Command::new(root.join("missing-target"));
    assert!(spawn_with_executable(&mut missing, &executable()).is_err());
    let mut wrong_session = tokio::process::Command::new("/usr/bin/touch");
    wrong_session.arg(&target_marker);
    unsafe {
        wrong_session.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    assert!(spawn_with_executable(&mut wrong_session, &executable()).is_err());
    assert!(
        !target_marker.exists(),
        "failed group join executed target code"
    );
    // A valid runtime after failure still reports the actual target PID/output/code.
    let mut target = tokio::process::Command::new("/bin/sh");
    target
        .args(["-c", "echo $$; exit 37"])
        .stdout(Stdio::piped());
    let (child, containment) = spawn_with_executable(&mut target, &executable()).unwrap();
    let pid = child.id().unwrap();
    let group = containment.runtime.as_ref().unwrap().group();
    assert_ne!(pid, group as u32);
    let output = child.wait_with_output().await.unwrap();
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap(),
        pid
    );
    assert_eq!(output.status.code(), Some(37));
    containment.terminate(1).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
