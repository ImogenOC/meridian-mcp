use crate::network_audit::{NetworkAuditCollector, NetworkAuditReport};
use anyhow::{Context, Result};
use serde::Serialize;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncRead;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, watch};

pub const MAX_PROCESS_OUTPUT_BYTES: usize = 512 * 1024;
pub const MAX_PROCESS_INPUT_BYTES: usize = 1024 * 1024;
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);
const OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BoundedOutput {
    pub text: String,
    pub captured_bytes: usize,
    pub truncated_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationReason {
    Exited,
    WallTimeout,
    IdleTimeout,
    Cancelled,
    SpawnFailed,
}

#[derive(Debug)]
pub struct ProcessSpec {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
    pub working_directory: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
    pub stdin: Option<Vec<u8>>,
    pub timeout: Duration,
    pub idle_timeout: Duration,
    pub capture_network: bool,
    pub cancellation: Option<watch::Receiver<bool>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProcessOutcome {
    pub exit_code: Option<i32>,
    pub termination: TerminationReason,
    pub duration_ms: u128,
    pub stdout: BoundedOutput,
    pub stderr: BoundedOutput,
    pub network_audit: NetworkAuditReport,
}

#[derive(Clone, Copy)]
enum OutputStream {
    Stdout,
    Stderr,
}

struct TailBuffer {
    bytes: Vec<u8>,
    truncated_bytes: u64,
}

impl TailBuffer {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            truncated_bytes: 0,
        }
    }

    fn push(&mut self, chunk: &[u8]) {
        if chunk.len() >= MAX_PROCESS_OUTPUT_BYTES {
            self.truncated_bytes = self
                .truncated_bytes
                .saturating_add(self.bytes.len() as u64)
                .saturating_add((chunk.len() - MAX_PROCESS_OUTPUT_BYTES) as u64);
            self.bytes.clear();
            self.bytes
                .extend_from_slice(&chunk[chunk.len() - MAX_PROCESS_OUTPUT_BYTES..]);
            return;
        }
        let overflow = self
            .bytes
            .len()
            .saturating_add(chunk.len())
            .saturating_sub(MAX_PROCESS_OUTPUT_BYTES);
        if overflow != 0 {
            self.bytes.drain(..overflow);
            self.truncated_bytes = self.truncated_bytes.saturating_add(overflow as u64);
        }
        self.bytes.extend_from_slice(chunk);
    }

    fn finish(self) -> BoundedOutput {
        BoundedOutput {
            captured_bytes: self.bytes.len(),
            text: String::from_utf8_lossy(&self.bytes).into_owned(),
            truncated_bytes: self.truncated_bytes,
        }
    }
}

async fn capture_stream<R>(
    mut reader: R,
    stream: OutputStream,
    sender: mpsc::Sender<(OutputStream, Vec<u8>)>,
) where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = match tokio::io::AsyncReadExt::read(&mut reader, &mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(count) => count,
        };
        if sender
            .send((stream, buffer[..count].to_vec()))
            .await
            .is_err()
        {
            break;
        }
    }
}

pub async fn run_contained_process(spec: ProcessSpec) -> Result<ProcessOutcome> {
    if spec
        .stdin
        .as_ref()
        .is_some_and(|input| input.len() > MAX_PROCESS_INPUT_BYTES)
    {
        anyhow::bail!("process stdin exceeds the {MAX_PROCESS_INPUT_BYTES}-byte limit");
    }
    let started_at = tokio::time::Instant::now();
    let mut audit = NetworkAuditCollector::new(spec.capture_network);
    let containment = ProcessContainment::new().context("cannot create process containment")?;
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.arguments)
        .current_dir(&spec.working_directory)
        .env_clear()
        .envs(spec.environment.iter().cloned())
        .stdin(if spec.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let message = format!("failed to spawn {}: {error}", spec.program.display());
            return Ok(ProcessOutcome {
                exit_code: None,
                termination: TerminationReason::SpawnFailed,
                duration_ms: started_at.elapsed().as_millis(),
                stdout: TailBuffer::new().finish(),
                stderr: BoundedOutput {
                    captured_bytes: message.len(),
                    text: message,
                    truncated_bytes: 0,
                },
                network_audit: audit.finish(),
            });
        }
    };
    let process_id = child.id().unwrap_or_default();
    if let Err(error) = containment.assign(process_id) {
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Err(error).context("refusing to run a process outside containment");
    }

    let stdin_task = match (spec.stdin, child.stdin.take()) {
        (Some(input), Some(mut stdin)) => Some(tokio::spawn(async move {
            stdin.write_all(&input).await?;
            stdin.shutdown().await
        })),
        _ => None,
    };

    let (sender, mut receiver) = mpsc::channel(64);
    let mut reader_tasks = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        reader_tasks.push(tokio::spawn(capture_stream(
            stdout,
            OutputStream::Stdout,
            sender.clone(),
        )));
    }
    if let Some(stderr) = child.stderr.take() {
        reader_tasks.push(tokio::spawn(capture_stream(
            stderr,
            OutputStream::Stderr,
            sender.clone(),
        )));
    }

    let mut stdout = TailBuffer::new();
    let mut stderr = TailBuffer::new();
    let mut last_progress = started_at;
    let mut previous_cpu_time = containment.cpu_time_100ns();
    let mut cancellation = spec.cancellation;
    let (termination, exit_code) = loop {
        let now = tokio::time::Instant::now();
        if now.duration_since(started_at) >= spec.timeout {
            terminate_child(&containment, &mut child).await?;
            break (
                TerminationReason::WallTimeout,
                child.wait().await.ok().and_then(|s| s.code()),
            );
        }
        if now.duration_since(last_progress) >= spec.idle_timeout {
            terminate_child(&containment, &mut child).await?;
            break (
                TerminationReason::IdleTimeout,
                child.wait().await.ok().and_then(|s| s.code()),
            );
        }

        tokio::select! {
            status = child.wait() => {
                let status = status.context("failed waiting for contained process")?;
                break (TerminationReason::Exited, status.code());
            }
            output = receiver.recv() => {
                if let Some((stream, bytes)) = output {
                    last_progress = tokio::time::Instant::now();
                    append_output(&mut stdout, &mut stderr, stream, &bytes);
                }
            }
            changed = async {
                match cancellation.as_mut() {
                    Some(receiver) => receiver.changed().await,
                    None => std::future::pending().await,
                }
            } => {
                if changed.is_ok() && cancellation.as_ref().is_some_and(|receiver| *receiver.borrow()) {
                    terminate_child(&containment, &mut child).await?;
                    break (
                        TerminationReason::Cancelled,
                        child.wait().await.ok().and_then(|status| status.code()),
                    );
                }
            }
            _ = tokio::time::sleep(PROCESS_POLL_INTERVAL) => {}
        }

        if let Some(cpu_time) = containment.cpu_time_100ns() {
            if previous_cpu_time.is_none_or(|previous| cpu_time > previous) {
                last_progress = tokio::time::Instant::now();
            }
            previous_cpu_time = Some(cpu_time);
        }
        let process_ids = containment.process_ids(process_id);
        audit.sample(&process_ids, started_at.elapsed().as_millis());
    };

    let process_ids = containment.process_ids(process_id);
    audit.sample(&process_ids, started_at.elapsed().as_millis());
    let _ = containment.terminate(1);
    drop(sender);
    drain_output(&mut receiver, &mut stdout, &mut stderr, &mut reader_tasks).await;
    if let Some(task) = stdin_task {
        match task.await {
            Ok(Ok(())) => {}
            Ok(Err(error)) if termination == TerminationReason::Exited => {
                return Err(error).context("failed writing bounded process stdin");
            }
            Err(error) if termination == TerminationReason::Exited => {
                return Err(error).context("bounded process stdin task failed");
            }
            Ok(Err(_)) | Err(_) => {}
        }
    }

    Ok(ProcessOutcome {
        exit_code,
        termination,
        duration_ms: started_at.elapsed().as_millis(),
        stdout: stdout.finish(),
        stderr: stderr.finish(),
        network_audit: audit.finish(),
    })
}

fn append_output(
    stdout: &mut TailBuffer,
    stderr: &mut TailBuffer,
    stream: OutputStream,
    bytes: &[u8],
) {
    match stream {
        OutputStream::Stdout => stdout.push(bytes),
        OutputStream::Stderr => stderr.push(bytes),
    }
}

async fn terminate_child(containment: &ProcessContainment, _child: &mut Child) -> Result<()> {
    containment.terminate(1)?;
    #[cfg(not(windows))]
    {
        let _ = _child.kill().await;
    }
    Ok(())
}

async fn drain_output(
    receiver: &mut mpsc::Receiver<(OutputStream, Vec<u8>)>,
    stdout: &mut TailBuffer,
    stderr: &mut TailBuffer,
    reader_tasks: &mut Vec<tokio::task::JoinHandle<()>>,
) {
    let deadline = tokio::time::Instant::now() + OUTPUT_DRAIN_TIMEOUT;
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }
        match tokio::time::timeout(deadline - now, receiver.recv()).await {
            Ok(Some((stream, bytes))) => append_output(stdout, stderr, stream, &bytes),
            Ok(None) | Err(_) => break,
        }
    }
    for task in reader_tasks.drain(..) {
        if !task.is_finished() {
            task.abort();
        }
        let _ = task.await;
    }
}

#[cfg(not(windows))]
pub(crate) struct ProcessContainment;

#[cfg(not(windows))]
impl ProcessContainment {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self)
    }
    pub(crate) fn assign(&self, _process_id: u32) -> Result<()> {
        Ok(())
    }
    pub(crate) fn terminate(&self, _exit_code: u32) -> Result<()> {
        Ok(())
    }
    fn cpu_time_100ns(&self) -> Option<u64> {
        None
    }
    fn process_ids(&self, root_process_id: u32) -> Vec<u32> {
        vec![root_process_id]
    }
}

#[cfg(windows)]
pub(crate) struct ProcessContainment {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
unsafe impl Send for ProcessContainment {}
#[cfg(windows)]
unsafe impl Sync for ProcessContainment {}

#[cfg(windows)]
impl ProcessContainment {
    pub(crate) fn new() -> Result<Self> {
        use std::mem::size_of;
        use std::ptr;
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        unsafe {
            let handle = CreateJobObjectW(ptr::null(), ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if configured == 0 {
                let error = std::io::Error::last_os_error();
                windows_sys::Win32::Foundation::CloseHandle(handle);
                return Err(error.into());
            }
            Ok(Self { handle })
        }
    }

    pub(crate) fn assign(&self, process_id: u32) -> Result<()> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };

        unsafe {
            let process = OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA | PROCESS_TERMINATE,
                0,
                process_id,
            );
            if process.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let assigned = AssignProcessToJobObject(self.handle, process);
            let assign_error = (assigned == 0).then(std::io::Error::last_os_error);
            let closed = CloseHandle(process);
            if let Some(error) = assign_error {
                return Err(error.into());
            }
            if closed == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(())
        }
    }

    pub(crate) fn terminate(&self, exit_code: u32) -> Result<()> {
        let terminated = unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.handle, exit_code)
        };
        if terminated == 0 {
            Err(std::io::Error::last_os_error().into())
        } else {
            Ok(())
        }
    }

    fn cpu_time_100ns(&self) -> Option<u64> {
        use std::mem::size_of;
        use std::ptr;
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicAndIoAccountingInformation, QueryInformationJobObject,
            JOBOBJECT_BASIC_AND_IO_ACCOUNTING_INFORMATION,
        };

        unsafe {
            let mut accounting = JOBOBJECT_BASIC_AND_IO_ACCOUNTING_INFORMATION::default();
            let queried = QueryInformationJobObject(
                self.handle,
                JobObjectBasicAndIoAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_AND_IO_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_AND_IO_ACCOUNTING_INFORMATION>() as u32,
                ptr::null_mut(),
            );
            (queried != 0).then(|| {
                accounting
                    .BasicInfo
                    .TotalUserTime
                    .saturating_add(accounting.BasicInfo.TotalKernelTime)
                    .max(0) as u64
            })
        }
    }

    fn process_ids(&self, root_process_id: u32) -> Vec<u32> {
        use std::mem::size_of_val;
        use std::ptr;
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicProcessIdList, QueryInformationJobObject, JOBOBJECT_BASIC_PROCESS_ID_LIST,
        };

        let mut storage = vec![0_usize; 1024];
        let queried = unsafe {
            QueryInformationJobObject(
                self.handle,
                JobObjectBasicProcessIdList,
                storage.as_mut_ptr().cast(),
                size_of_val(storage.as_slice()) as u32,
                ptr::null_mut(),
            )
        };
        if queried == 0 {
            return vec![root_process_id];
        }
        let list = unsafe { &*(storage.as_ptr().cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>()) };
        let count = (list.NumberOfProcessIdsInList as usize).min(storage.len().saturating_sub(1));
        let ids = unsafe { std::slice::from_raw_parts(list.ProcessIdList.as_ptr(), count) };
        let mut output = ids.iter().map(|id| *id as u32).collect::<Vec<_>>();
        if output.is_empty() {
            output.push(root_process_id);
        }
        output
    }
}

#[cfg(windows)]
impl Drop for ProcessContainment {
    fn drop(&mut self) {
        let closed = unsafe { windows_sys::Win32::Foundation::CloseHandle(self.handle) };
        debug_assert_ne!(closed, 0, "failed to close Windows Job Object handle");
    }
}
