use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::input::BoundaryError;

pub const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub enum NativeTool {
    Launchctl,
    Crontab,
    Systemctl,
    Schtasks,
    PowerShell,
}

impl NativeTool {
    pub(crate) fn program(self) -> Result<PathBuf, BoundaryError> {
        match self {
            Self::Launchctl => Ok(PathBuf::from("/bin/launchctl")),
            Self::Crontab => Ok(PathBuf::from("/usr/bin/crontab")),
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
        NativeTool::Crontab => &[("LC_ALL", "C")][..],
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
    let mut child = Command::new(program)
        .args(arguments)
        .envs(environment.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| BoundaryError::ProcessSpawn(error.to_string()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| BoundaryError::ProcessRead("stdout was unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| BoundaryError::ProcessRead("stderr was unavailable".into()))?;
    let stdout_reader = thread::spawn(move || read_capped(stdout));
    let stderr_reader = thread::spawn(move || read_capped(stderr));

    let status = match child
        .wait_timeout(timeout)
        .map_err(|error| BoundaryError::ProcessRead(error.to_string()))?
    {
        Some(status) => status,
        None => {
            child
                .kill()
                .map_err(|error| BoundaryError::ProcessRead(error.to_string()))?;
            let _ = child.wait();
            return Err(BoundaryError::ProcessTimeout);
        }
    };

    let stdout = join_reader(stdout_reader)?;
    let stderr = join_reader(stderr_reader)?;
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

fn join_reader(
    reader: thread::JoinHandle<std::io::Result<Vec<u8>>>,
) -> Result<Vec<u8>, BoundaryError> {
    reader
        .join()
        .map_err(|_| BoundaryError::ProcessRead("output reader panicked".into()))?
        .map_err(|error| BoundaryError::ProcessRead(error.to_string()))
}

#[cfg(test)]
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
