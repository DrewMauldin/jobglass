use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::Path;

use thiserror::Error;

pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
pub const MAX_JOBS: usize = 10_000;

#[derive(Debug, Error)]
pub enum BoundaryError {
    #[error("input exceeded the {limit}-byte limit")]
    InputTooLarge { limit: usize },
    #[error("native process output exceeded the {limit}-byte limit")]
    OutputTooLarge { limit: usize },
    #[error("input was not valid UTF-8")]
    InvalidEncoding,
    #[error("symbolic link input was rejected")]
    SymlinkRejected,
    #[error("path was outside the scheduler source allowlist")]
    PathNotAllowed,
    #[error("scheduler source was not a regular file")]
    NotARegularFile,
    #[error("native process exceeded its time limit")]
    ProcessTimeout,
    #[error("native process could not be started: {0}")]
    ProcessSpawn(String),
    #[error("native process could not be read: {0}")]
    ProcessRead(String),
    #[error("scheduler source could not be read: {0}")]
    FileRead(String),
}

pub fn decode_bounded(input: &[u8]) -> Result<&str, BoundaryError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(BoundaryError::InputTooLarge {
            limit: MAX_INPUT_BYTES,
        });
    }
    std::str::from_utf8(input).map_err(|_| BoundaryError::InvalidEncoding)
}

pub fn read_bounded_file(path: &Path, allowed_roots: &[&Path]) -> Result<String, BoundaryError> {
    let initial_metadata = std::fs::symlink_metadata(path)
        .map_err(|error| BoundaryError::FileRead(error.to_string()))?;
    if initial_metadata.file_type().is_symlink() {
        return Err(BoundaryError::SymlinkRejected);
    }
    if !initial_metadata.is_file() {
        return Err(BoundaryError::NotARegularFile);
    }
    enforce_input_size(initial_metadata.len())?;

    let canonical_path = path
        .canonicalize()
        .map_err(|error| BoundaryError::FileRead(error.to_string()))?;
    let path_is_allowed = allowed_roots.iter().any(|root| {
        root.canonicalize()
            .is_ok_and(|canonical_root| canonical_path.starts_with(canonical_root))
    });
    if !path_is_allowed {
        return Err(BoundaryError::PathNotAllowed);
    }

    let mut file = open_read_only_no_follow(path)?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| BoundaryError::FileRead(error.to_string()))?;
    if !opened_metadata.is_file() {
        return Err(BoundaryError::NotARegularFile);
    }
    enforce_input_size(opened_metadata.len())?;

    let mut bytes = Vec::with_capacity(opened_metadata.len() as usize);
    file.by_ref()
        .take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| BoundaryError::FileRead(error.to_string()))?;
    Ok(decode_bounded(&bytes)?.to_owned())
}

fn enforce_input_size(size: u64) -> Result<(), BoundaryError> {
    if size > MAX_INPUT_BYTES as u64 {
        Err(BoundaryError::InputTooLarge {
            limit: MAX_INPUT_BYTES,
        })
    } else {
        Ok(())
    }
}

fn open_read_only_no_follow(path: &Path) -> Result<File, BoundaryError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    options
        .open(path)
        .map_err(|error| BoundaryError::FileRead(error.to_string()))
}
