#[cfg(windows)]
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
#[cfg(not(windows))]
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(not(windows))]
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
    #[cfg(windows)]
    {
        run_program_windows(program, arguments, timeout, environment)
    }
    #[cfg(not(windows))]
    {
        run_program_portable(program, arguments, timeout, environment)
    }
}

#[cfg(not(windows))]
fn run_program_portable(
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
    let process_tree = match ProcessTree::attach(&child) {
        Ok(process_tree) => process_tree,
        Err(error) => {
            let _ = child.kill();
            return Err(BoundaryError::ProcessSpawn(error.to_string()));
        }
    };
    let mut child = ManagedChild::new(child, process_tree);

    let stdout = child
        .child
        .stdout
        .take()
        .ok_or_else(|| BoundaryError::ProcessRead("stdout was unavailable".into()))?;
    let stderr = child
        .child
        .stderr
        .take()
        .ok_or_else(|| BoundaryError::ProcessRead("stderr was unavailable".into()))?;
    let (output_sender, output_receiver) = mpsc::channel();
    spawn_reader(OutputKind::Stdout, stdout, output_sender.clone())
        .map_err(|error| BoundaryError::ProcessRead(error.to_string()))?;
    spawn_reader(OutputKind::Stderr, stderr, output_sender)
        .map_err(|error| BoundaryError::ProcessRead(error.to_string()))?;

    let status = match child
        .child
        .wait_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|error| BoundaryError::ProcessRead(error.to_string()))?
    {
        Some(status) => status,
        None => {
            child.terminate_and_reap();
            return Err(BoundaryError::ProcessTimeout);
        }
    };

    let (stdout, stderr) = receive_outputs(&output_receiver, deadline, || {
        child.terminate_and_reap();
    })?;
    if stdout.len().saturating_add(stderr.len()) > MAX_OUTPUT_BYTES {
        return Err(BoundaryError::OutputTooLarge {
            limit: MAX_OUTPUT_BYTES,
        });
    }

    let output = NativeOutput {
        exit_code: status.code(),
        stdout: decode_native_output(stdout)?,
        stderr: decode_native_output(stderr)?,
    };
    child.finish();
    Ok(output)
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
) -> std::io::Result<()> {
    thread::Builder::new()
        .name("jobglass-native-output".into())
        .spawn(move || {
            let _ = sender.send((kind, read_capped(stream)));
        })
        .map(|_| ())
}

fn receive_outputs(
    receiver: &mpsc::Receiver<(OutputKind, std::io::Result<Vec<u8>>)>,
    deadline: Instant,
    mut cleanup: impl FnMut(),
) -> Result<(Vec<u8>, Vec<u8>), BoundaryError> {
    let mut stdout = None;
    let mut stderr = None;
    while stdout.is_none() || stderr.is_none() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            cleanup();
            return Err(BoundaryError::ProcessTimeout);
        }
        let (kind, result) = match receiver.recv_timeout(remaining) {
            Ok(output) => output,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                cleanup();
                return Err(BoundaryError::ProcessTimeout);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                cleanup();
                return Err(BoundaryError::ProcessRead(
                    "output readers disconnected".into(),
                ));
            }
        };
        let bytes = match result {
            Ok(bytes) => bytes,
            Err(error) => {
                cleanup();
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

#[cfg(windows)]
fn run_program_windows(
    program: &Path,
    arguments: &[&str],
    timeout: Duration,
    environment: &[(&str, &str)],
) -> Result<NativeOutput, BoundaryError> {
    if !environment.is_empty() {
        return Err(BoundaryError::ProcessSpawn(
            "custom environments are unsupported for Windows native tools".into(),
        ));
    }
    let deadline = Instant::now() + timeout;
    let (mut process, stdout, stderr) = spawn_windows_process(program, arguments)
        .map_err(|error| BoundaryError::ProcessSpawn(error.to_string()))?;
    let (output_sender, output_receiver) = mpsc::channel();
    spawn_reader(OutputKind::Stdout, stdout, output_sender.clone())
        .map_err(|error| BoundaryError::ProcessRead(error.to_string()))?;
    spawn_reader(OutputKind::Stderr, stderr, output_sender)
        .map_err(|error| BoundaryError::ProcessRead(error.to_string()))?;

    if !process
        .wait(deadline.saturating_duration_since(Instant::now()))
        .map_err(|error| BoundaryError::ProcessRead(error.to_string()))?
    {
        process.terminate_and_reap();
        return Err(BoundaryError::ProcessTimeout);
    }
    let exit_code = process
        .exit_code()
        .map_err(|error| BoundaryError::ProcessRead(error.to_string()))?;
    let (stdout, stderr) = receive_outputs(&output_receiver, deadline, || {
        process.terminate_and_reap();
    })?;
    if stdout.len().saturating_add(stderr.len()) > MAX_OUTPUT_BYTES {
        return Err(BoundaryError::OutputTooLarge {
            limit: MAX_OUTPUT_BYTES,
        });
    }
    let output = NativeOutput {
        exit_code: Some(exit_code as i32),
        stdout: decode_native_output(stdout)?,
        stderr: decode_native_output(stderr)?,
    };
    process.finish();
    Ok(output)
}

#[cfg(windows)]
fn spawn_windows_process(
    program: &Path,
    arguments: &[&str],
) -> std::io::Result<(WindowsProcess, File, File)> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
    use windows_sys::Win32::System::Threading::{
        CREATE_NO_WINDOW, CREATE_SUSPENDED, CreateProcessW, PROCESS_INFORMATION, ResumeThread,
        STARTF_USESTDHANDLES, STARTUPINFOW, TerminateProcess,
    };

    // Assignment happens while the only process thread is suspended, so no descendant can
    // escape the kill-on-close Job Object before containment is established.
    let job = configured_windows_job()?;
    let (stdin_read, stdin_write) = windows_pipe(false)?;
    let (stdout_read, stdout_write) = windows_pipe(true)?;
    let (stderr_read, stderr_write) = windows_pipe(true)?;
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        dwFlags: STARTF_USESTDHANDLES,
        hStdInput: stdin_read.as_raw_handle(),
        hStdOutput: stdout_write.as_raw_handle(),
        hStdError: stderr_write.as_raw_handle(),
        ..Default::default()
    };
    let mut process_information = PROCESS_INFORMATION::default();
    let mut application = program.as_os_str().encode_wide().collect::<Vec<_>>();
    application.push(0);
    let mut command_line = windows_command_line(program, arguments);
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
            CREATE_SUSPENDED | CREATE_NO_WINDOW,
            std::ptr::null(),
            std::ptr::null(),
            &raw const startup,
            &raw mut process_information,
        )
    };
    if created == 0 {
        return Err(std::io::Error::last_os_error());
    }

    let process = unsafe { OwnedHandle::from_raw_handle(process_information.hProcess) };
    let thread = unsafe { OwnedHandle::from_raw_handle(process_information.hThread) };
    let assigned =
        unsafe { AssignProcessToJobObject(job.as_raw_handle(), process.as_raw_handle()) };
    if assigned == 0 {
        let error = std::io::Error::last_os_error();
        unsafe {
            let _ = TerminateProcess(process.as_raw_handle(), 1);
        }
        return Err(error);
    }
    let mut process = WindowsProcess {
        job,
        process,
        armed: true,
    };
    if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
        let error = std::io::Error::last_os_error();
        process.terminate_and_reap();
        return Err(error);
    }

    drop(thread);
    drop(stdin_read);
    drop(stdin_write);
    drop(stdout_write);
    drop(stderr_write);
    Ok((process, File::from(stdout_read), File::from(stderr_read)))
}

#[cfg(windows)]
fn configured_windows_job() -> std::io::Result<std::os::windows::io::OwnedHandle> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

    use windows_sys::Win32::System::JobObjects::{
        CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    };

    let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let job = unsafe { OwnedHandle::from_raw_handle(handle) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let configured = unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&limits).cast(),
            std::mem::size_of_val(&limits) as u32,
        )
    };
    if configured == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(job)
}

#[cfg(windows)]
fn windows_pipe(
    parent_reads: bool,
) -> std::io::Result<(
    std::os::windows::io::OwnedHandle,
    std::os::windows::io::OwnedHandle,
)> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

    use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::System::Pipes::CreatePipe;

    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    let mut read = std::ptr::null_mut();
    let mut write = std::ptr::null_mut();
    if unsafe { CreatePipe(&raw mut read, &raw mut write, &raw mut attributes, 0) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let read = unsafe { OwnedHandle::from_raw_handle(read) };
    let write = unsafe { OwnedHandle::from_raw_handle(write) };
    let parent = if parent_reads { &read } else { &write };
    if unsafe { SetHandleInformation(parent.as_raw_handle(), HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((read, write))
}

#[cfg(windows)]
fn windows_command_line(program: &Path, arguments: &[&str]) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let mut command_line = Vec::new();
    for (index, argument) in std::iter::once(program.as_os_str())
        .chain(arguments.iter().map(OsStr::new))
        .enumerate()
    {
        if index > 0 {
            command_line.push(b' ' as u16);
        }
        append_quoted_windows_argument(&mut command_line, argument.encode_wide());
    }
    command_line.push(0);
    command_line
}

#[cfg(windows)]
fn append_quoted_windows_argument(output: &mut Vec<u16>, argument: impl Iterator<Item = u16>) {
    let quote = b'"' as u16;
    let slash = b'\\' as u16;
    output.push(quote);
    let mut slashes = 0_usize;
    for unit in argument {
        if unit == slash {
            slashes += 1;
        } else {
            let slash_count = if unit == quote {
                slashes * 2 + 1
            } else {
                slashes
            };
            output.extend(std::iter::repeat_n(slash, slash_count));
            slashes = 0;
            output.push(unit);
        }
    }
    output.extend(std::iter::repeat_n(slash, slashes * 2));
    output.push(quote);
}

#[cfg(windows)]
struct WindowsProcess {
    job: std::os::windows::io::OwnedHandle,
    process: std::os::windows::io::OwnedHandle,
    armed: bool,
}

#[cfg(windows)]
impl WindowsProcess {
    fn wait(&self, timeout: Duration) -> std::io::Result<bool> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::{WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT};
        use windows_sys::Win32::System::Threading::WaitForSingleObject;

        let milliseconds = timeout.as_millis().clamp(1, u32::MAX as u128) as u32;
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), milliseconds) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            WAIT_FAILED => Err(std::io::Error::last_os_error()),
            _ => Err(std::io::Error::other(
                "unexpected Windows process wait result",
            )),
        }
    }

    fn exit_code(&self) -> std::io::Result<u32> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::Threading::GetExitCodeProcess;

        let mut exit_code = 0_u32;
        if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &raw mut exit_code) } == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(exit_code)
        }
    }

    fn terminate_and_reap(&mut self) {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;
        use windows_sys::Win32::System::Threading::WaitForSingleObject;

        unsafe {
            let _ = TerminateJobObject(self.job.as_raw_handle(), 1);
            let _ = WaitForSingleObject(self.process.as_raw_handle(), 100);
        }
        self.armed = false;
    }

    fn finish(&mut self) {
        self.terminate_and_reap();
    }
}

#[cfg(windows)]
impl Drop for WindowsProcess {
    fn drop(&mut self) {
        if self.armed {
            self.terminate_and_reap();
        }
    }
}

#[cfg(not(windows))]
struct ManagedChild {
    child: std::process::Child,
    process_tree: ProcessTree,
    armed: bool,
}

#[cfg(not(windows))]
impl ManagedChild {
    fn new(child: std::process::Child, process_tree: ProcessTree) -> Self {
        Self {
            child,
            process_tree,
            armed: true,
        }
    }

    fn terminate_and_reap(&mut self) {
        self.process_tree.terminate();
        let _ = self.child.kill();
        let _ = self.child.wait_timeout(Duration::from_millis(100));
        self.armed = false;
    }

    fn finish(&mut self) {
        self.process_tree.terminate();
        self.armed = false;
    }
}

#[cfg(not(windows))]
impl Drop for ManagedChild {
    fn drop(&mut self) {
        if self.armed {
            self.terminate_and_reap();
        }
    }
}

#[cfg(not(windows))]
struct ProcessTree {
    process_id: u32,
}

#[cfg(not(windows))]
impl ProcessTree {
    fn attach(child: &std::process::Child) -> std::io::Result<Self> {
        Ok(Self {
            process_id: child.id(),
        })
    }

    fn terminate(&self) {
        #[cfg(unix)]
        unsafe {
            let _ = libc::kill(-(self.process_id as i32), libc::SIGKILL);
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
