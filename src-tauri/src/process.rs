use std::io::Read;
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
    fn program(self) -> &'static str {
        match self {
            Self::Launchctl => "/bin/launchctl",
            Self::Crontab => "crontab",
            Self::Systemctl => "systemctl",
            Self::Schtasks => "schtasks.exe",
            Self::PowerShell => "powershell.exe",
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
    run_program(tool.program(), arguments, timeout)
}

fn run_program(
    program: &str,
    arguments: &[&str],
    timeout: Duration,
) -> Result<NativeOutput, BoundaryError> {
    let mut child = Command::new(program)
        .args(arguments)
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
        stdout: String::from_utf8(stdout).map_err(|_| BoundaryError::InvalidEncoding)?,
        stderr: String::from_utf8(stderr).map_err(|_| BoundaryError::InvalidEncoding)?,
    })
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
    run_program(program, arguments, timeout)
}
