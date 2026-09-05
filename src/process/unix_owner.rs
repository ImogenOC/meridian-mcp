//! Runtime-only group ownership. The sibling guardian holds the group identity
//! and kills inherited group members when the owner's CLOEXEC pipe reaches EOF.
use super::ProcessContainment;
use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MODE: &str = "--internal-runtime-guardian-v1";
const READY: u8 = 0x71;
const SETUP_TIMEOUT: Duration = Duration::from_secs(2);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
static EXECUTABLE: Mutex<Option<PathBuf>> = Mutex::new(None);

pub(super) fn initialize(path: &Path) -> Result<()> {
    anyhow::ensure!(path.is_absolute(), "guardian executable must be absolute");
    let path = path
        .canonicalize()
        .context("cannot resolve guardian executable")?;
    anyhow::ensure!(path.is_file(), "guardian executable must be a file");
    *EXECUTABLE.lock().unwrap_or_else(|error| error.into_inner()) = Some(path);
    Ok(())
}

struct KillGroup(libc::pid_t);

impl Drop for KillGroup {
    fn drop(&mut self) {
        // This process reserves its own PGID until the signal kills it.
        unsafe { libc::kill(-self.0, libc::SIGKILL) };
    }
}

pub(super) fn dispatch() -> bool {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(MODE)) {
        return false;
    }
    let pid = unsafe { libc::getpid() };
    if pid <= 1 || unsafe { libc::getpgrp() } != pid {
        std::process::exit(2);
    }
    let _cleanup = KillGroup(pid);
    if args.next().is_some() {
        return true;
    }
    // A closed readiness reader must take the ordinary group-cleanup path.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };
    if std::io::stdout()
        .write_all(&[READY])
        .and_then(|()| std::io::stdout().flush())
        .is_err()
    {
        return true;
    }
    let mut byte = [0_u8];
    loop {
        match std::io::stdin().read(&mut byte) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            // The owner sends no data: both EOF and protocol errors revoke it.
            _ => return true,
        }
    }
}

struct OwnerState {
    writer: Option<OwnedFd>,
    guardian: Option<Child>,
}

pub(super) struct RuntimeOwner {
    state: Mutex<OwnerState>,
}

impl RuntimeOwner {
    fn start(executable: &Path) -> Result<Self> {
        let mut guardian = Command::new(executable)
            .arg(MODE)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .context("cannot spawn runtime guardian")?;
        let mut ready = guardian
            .stdout
            .take()
            .context("guardian readiness pipe missing")?;
        let owner = Self {
            state: Mutex::new(OwnerState {
                writer: guardian.stdin.take().map(Into::into),
                guardian: Some(guardian),
            }),
        };
        {
            let mut state = owner
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let writer = state
                .writer
                .as_ref()
                .context("guardian owner pipe missing")?;
            // Keep the lease outside stdio even if an embedding host closed 0-2.
            let fd = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
            anyhow::ensure!(
                fd >= 3,
                "cannot reserve CLOEXEC runtime owner pipe: {}",
                std::io::Error::last_os_error()
            );
            state.writer = Some(unsafe { OwnedFd::from_raw_fd(fd) });
        }
        let deadline = Instant::now() + SETUP_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            anyhow::ensure!(!remaining.is_zero(), "guardian readiness timed out");
            let mut fd = libc::pollfd {
                fd: ready.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let result = unsafe {
                libc::poll(
                    &mut fd,
                    1,
                    remaining.as_millis().min(i32::MAX as u128) as i32,
                )
            };
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error).context("guardian readiness poll failed");
            }
            anyhow::ensure!(result != 0, "guardian readiness timed out");
            let mut byte = [0_u8];
            ready
                .read_exact(&mut byte)
                .context("guardian exited before readiness")?;
            anyhow::ensure!(byte[0] == READY, "invalid guardian readiness");
            return Ok(owner);
        }
    }

    fn group(&self) -> libc::pid_t {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .guardian
            .as_ref()
            .unwrap()
            .id() as libc::pid_t
    }

    pub(super) fn terminate(&self) -> Result<()> {
        self.request_termination()?;
        let deadline = Instant::now() + CLEANUP_TIMEOUT;
        while !self.is_terminated()? {
            anyhow::ensure!(Instant::now() < deadline, "runtime group cleanup timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    pub(super) fn request_termination(&self) -> Result<()> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.writer.take();
        let Some(guardian) = state.guardian.as_mut() else {
            return Ok(());
        };
        let group = guardian.id() as libc::pid_t;
        // Never signal after reaping: this unreaped child reserves the identity.
        if unsafe { libc::kill(-group, libc::SIGKILL) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error.into());
            }
        }
        Ok(())
    }

    pub(super) fn is_terminated(&self) -> Result<bool> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let Some(guardian) = state.guardian.as_mut() else {
            return Ok(true);
        };
        let group = guardian.id() as libc::pid_t;
        // WNOWAIT preserves the identity through the final signal/stop check.
        let mut status: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                group as libc::id_t,
                &mut status,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        if unsafe { status.si_pid() } == group && group_stopped(group)? {
            guardian.wait()?;
            state.guardian.take();
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(target_os = "linux")]
fn group_stopped(group: libc::pid_t) -> Result<bool> {
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let stat = match std::fs::read_to_string(entry.path().join("stat")) {
            Ok(stat) => stat,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let Some((_, fields)) = stat.rsplit_once(") ") else {
            anyhow::bail!("invalid process identity record");
        };
        let mut fields = fields.split_whitespace();
        let state = fields.next();
        let _parent = fields.next();
        if fields
            .next()
            .and_then(|value| value.parse::<libc::pid_t>().ok())
            == Some(group)
            && !matches!(state, Some("Z" | "X"))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(not(target_os = "linux"))]
fn group_stopped(_group: libc::pid_t) -> Result<bool> {
    // Other Unix platforms have no qualified process-state oracle yet.
    anyhow::bail!("runtime cleanup completion is not qualified on this Unix platform")
}

impl Drop for RuntimeOwner {
    fn drop(&mut self) {
        let _ = self.terminate();
        let state = self
            .state
            .get_mut()
            .unwrap_or_else(|error| error.into_inner());
        state.writer.take();
        if let Some(mut guardian) = state.guardian.take() {
            // This thread owns only the unreaped child, never an owner lease.
            let _ = std::thread::Builder::new()
                .name("runtime-guardian-reap".into())
                .spawn(move || {
                    let _ = guardian.wait();
                });
        }
    }
}

pub(super) fn spawn(
    command: &mut tokio::process::Command,
) -> Result<(tokio::process::Child, Arc<ProcessContainment>)> {
    let executable = EXECUTABLE
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
        .context(
            "runtime owner is not initialized; configure initialize_runtime_owner_with_executable",
        )?;
    spawn_with_executable(command, &executable)
}

fn spawn_with_executable(
    command: &mut tokio::process::Command,
    executable: &Path,
) -> Result<(tokio::process::Child, Arc<ProcessContainment>)> {
    let owner = RuntimeOwner::start(executable)?;
    let group = owner.group();
    // Rust 1.95 pre_exec forces fork. The fork inherits the CLOEXEC owner writer
    // until exec, keeping the guardian alive even if MCP dies before setpgid.
    // Capture only PGID: a Command-held writer clone would prolong the lease.
    unsafe {
        command.pre_exec(move || {
            if libc::setpgid(0, group) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command
        .kill_on_drop(true)
        .spawn()
        .context("cannot spawn owned runtime")?;
    Ok((
        child,
        Arc::new(ProcessContainment {
            runtime: Some(owner),
            #[cfg(test)]
            cleanup_fault: std::sync::atomic::AtomicU8::new(0),
        }),
    ))
}

#[cfg(all(test, target_os = "linux"))]
#[path = "unix_owner_tests.rs"]
mod tests;
