use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use wait_timeout::ChildExt;

use crate::input::BoundaryError;

pub const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub enum NativeTool {
    Launchctl,
    Systemctl,
    Schtasks,
    PowerShell,
}

impl NativeTool {
    pub(crate) fn program(self) -> Result<PathBuf, BoundaryError> {
        match self {
            Self::Launchctl => Ok(PathBuf::from("/bin/launchctl")),
            Self::Systemctl => Ok(PathBuf::from("/usr/bin/systemctl")),
            Self::Schtasks => windows_system_tool("schtasks.exe"),
            Self::PowerShell => windows_system_tool("WindowsPowerShell/v1.0/powershell.exe"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_native_tool(
    tool: NativeTool,
    arguments: &[&str],
    timeout: Duration,
) -> Result<NativeOutput, BoundaryError> {
    let environment = match tool {
        NativeTool::Systemctl => &[("LC_ALL", "C"), ("TZ", "UTC")][..],
        _ => &[][..],
    };
    run_program(&tool.program()?, arguments, timeout, environment)
}

fn run_program(
    program: &Path,
    arguments: &[&str],
    timeout: Duration,
    environment: &[(&str, &str)],
) -> Result<NativeOutput, BoundaryError> {
    let deadline = Instant::now() + timeout;
    let mut command = Command::new(program);
    command
        .args(arguments)
        .envs(environment.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        command.pre_exec(|| {
            if libc::setpgid(0, 0) == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
    let mut child = command
        .spawn()
        .map_err(|error| BoundaryError::ProcessSpawn(error.to_string()))?;
    let process_tree = ProcessTree::attach(&child).map_err(|error| {
        let _ = child.kill();
        BoundaryError::ProcessSpawn(error.to_string())
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| BoundaryError::ProcessRead("stdout was unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| BoundaryError::ProcessRead("stderr was unavailable".into()))?;
    let (output_sender, output_receiver) = mpsc::channel();
    spawn_reader(OutputKind::Stdout, stdout, output_sender.clone());
    spawn_reader(OutputKind::Stderr, stderr, output_sender);

    let status = match child
        .wait_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|error| BoundaryError::ProcessRead(error.to_string()))?
    {
        Some(status) => status,
        None => {
            terminate_and_reap(&process_tree, &mut child);
            return Err(BoundaryError::ProcessTimeout);
        }
    };

    let (stdout, stderr) = receive_outputs(&output_receiver, deadline, &process_tree, &mut child)?;
    if stdout.len().saturating_add(stderr.len()) > MAX_OUTPUT_BYTES {
        return Err(BoundaryError::OutputTooLarge {
            limit: MAX_OUTPUT_BYTES,
        });
    }

    Ok(NativeOutput {
        exit_code: status.code(),
        stdout: decode_native_output(stdout)?,
        stderr: decode_native_output(stderr)?,
    })
}

#[derive(Clone, Copy)]
enum OutputKind {
    Stdout,
    Stderr,
}

fn spawn_reader(
    kind: OutputKind,
    stream: impl Read + Send + 'static,
    sender: mpsc::Sender<(OutputKind, std::io::Result<Vec<u8>>)>,
) {
    thread::spawn(move || {
        let _ = sender.send((kind, read_capped(stream)));
    });
}

fn receive_outputs(
    receiver: &mpsc::Receiver<(OutputKind, std::io::Result<Vec<u8>>)>,
    deadline: Instant,
    process_tree: &ProcessTree,
    child: &mut std::process::Child,
) -> Result<(Vec<u8>, Vec<u8>), BoundaryError> {
    let mut stdout = None;
    let mut stderr = None;
    while stdout.is_none() || stderr.is_none() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            terminate_and_reap(process_tree, child);
            return Err(BoundaryError::ProcessTimeout);
        }
        let (kind, result) = match receiver.recv_timeout(remaining) {
            Ok(output) => output,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                terminate_and_reap(process_tree, child);
                return Err(BoundaryError::ProcessTimeout);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                terminate_and_reap(process_tree, child);
                return Err(BoundaryError::ProcessRead(
                    "output readers disconnected".into(),
                ));
            }
        };
        let bytes = match result {
            Ok(bytes) => bytes,
            Err(error) => {
                terminate_and_reap(process_tree, child);
                return Err(BoundaryError::ProcessRead(error.to_string()));
            }
        };
        match kind {
            OutputKind::Stdout => stdout = Some(bytes),
            OutputKind::Stderr => stderr = Some(bytes),
        }
    }
    Ok((stdout.unwrap_or_default(), stderr.unwrap_or_default()))
}

fn terminate_and_reap(process_tree: &ProcessTree, child: &mut std::process::Child) {
    process_tree.terminate();
    let _ = child.kill();
    let _ = child.wait_timeout(Duration::from_millis(100));
}

fn decode_native_output(bytes: Vec<u8>) -> Result<String, BoundaryError> {
    if bytes.starts_with(&[0xff, 0xfe]) {
        return decode_utf16(&bytes[2..], u16::from_le_bytes);
    }
    if bytes.starts_with(&[0xfe, 0xff]) {
        return decode_utf16(&bytes[2..], u16::from_be_bytes);
    }
    let looks_utf16le = bytes.len() >= 4
        && bytes.len().is_multiple_of(2)
        && bytes
            .iter()
            .skip(1)
            .step_by(2)
            .take(32)
            .any(|byte| *byte == 0);
    if looks_utf16le {
        return decode_utf16(&bytes, u16::from_le_bytes);
    }
    String::from_utf8(bytes).map_err(|_| BoundaryError::InvalidEncoding)
}

fn decode_utf16(bytes: &[u8], decode: impl Fn([u8; 2]) -> u16) -> Result<String, BoundaryError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(BoundaryError::InvalidEncoding);
    }
    let units = bytes.as_chunks::<2>().0.iter().map(|pair| decode(*pair));
    char::decode_utf16(units)
        .collect::<Result<String, _>>()
        .map_err(|_| BoundaryError::InvalidEncoding)
}

#[cfg(windows)]
fn windows_system_tool(relative: &str) -> Result<PathBuf, BoundaryError> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

    let mut buffer = vec![0_u16; 32_768];
    let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(BoundaryError::ProcessSpawn(
            "Windows system directory was unavailable".into(),
        ));
    }
    Ok(PathBuf::from(OsString::from_wide(&buffer[..length])).join(relative))
}

#[cfg(not(windows))]
fn windows_system_tool(relative: &str) -> Result<PathBuf, BoundaryError> {
    Ok(PathBuf::from("/Windows/System32").join(relative))
}

fn read_capped(mut stream: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream
        .by_ref()
        .take((MAX_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

struct ProcessTree {
    #[cfg(not(windows))]
    process_id: u32,
    #[cfg(windows)]
    job: windows_sys::Win32::Foundation::HANDLE,
}

impl ProcessTree {
    #[cfg(not(windows))]
    fn attach(child: &std::process::Child) -> std::io::Result<Self> {
        Ok(Self {
            process_id: child.id(),
        })
    }

    #[cfg(windows)]
    fn attach(child: &std::process::Child) -> std::io::Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };

        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        };
        let assigned =
            configured != 0 && unsafe { AssignProcessToJobObject(job, child.as_raw_handle()) } != 0;
        if !assigned {
            let error = std::io::Error::last_os_error();
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(job);
            }
            return Err(error);
        }
        Ok(Self { job })
    }

    fn terminate(&self) {
        #[cfg(unix)]
        unsafe {
            let _ = libc::kill(-(self.process_id as i32), libc::SIGKILL);
        }
        #[cfg(windows)]
        unsafe {
            let _ = windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

#[cfg(all(test, unix))]
pub(crate) fn run_program_for_test(
    program: &str,
    arguments: &[&str],
    timeout: Duration,
) -> Result<NativeOutput, BoundaryError> {
    run_program(Path::new(program), arguments, timeout, &[])
}

#[cfg(test)]
pub(crate) fn decode_native_output_for_test(bytes: Vec<u8>) -> Result<String, BoundaryError> {
    decode_native_output(bytes)
}
