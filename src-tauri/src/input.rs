use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

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
    #[error("scheduler source root was not a directory")]
    NotADirectory,
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
    validate_bounded_bytes(input)?;
    std::str::from_utf8(input).map_err(|_| BoundaryError::InvalidEncoding)
}

pub fn validate_bounded_bytes(input: &[u8]) -> Result<(), BoundaryError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(BoundaryError::InputTooLarge {
            limit: MAX_INPUT_BYTES,
        });
    }
    Ok(())
}

pub fn read_bounded_file(path: &Path, allowed_roots: &[&Path]) -> Result<String, BoundaryError> {
    let bytes = read_bounded_file_bytes(path, allowed_roots)?;
    Ok(decode_bounded(&bytes)?.to_owned())
}

pub fn read_bounded_file_bytes(
    path: &Path,
    allowed_roots: &[&Path],
) -> Result<Vec<u8>, BoundaryError> {
    for root in allowed_roots {
        validate_directory_root(root)?;
    }
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
    validate_bounded_bytes(&bytes)?;
    Ok(bytes)
}

pub fn validate_directory_root(root: &Path) -> Result<(), BoundaryError> {
    let metadata = std::fs::symlink_metadata(root)
        .map_err(|error| BoundaryError::FileRead(error.to_string()))?;
    if metadata.file_type().is_symlink() {
        return Err(BoundaryError::SymlinkRejected);
    }
    if !metadata.is_dir() {
        return Err(BoundaryError::NotADirectory);
    }
    Ok(())
}

pub fn local_executable_exists(value: &str) -> bool {
    let Some(metadata) = safe_local_metadata(value) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub fn local_directory_exists(value: &str) -> bool {
    safe_local_metadata(value).is_some_and(|metadata| metadata.is_dir())
}

fn safe_local_metadata(value: &str) -> Option<std::fs::Metadata> {
    if value.is_empty() || value.len() > 32_768 || value.contains('\0') {
        return None;
    }
    let path = Path::new(value);
    if !path.is_absolute() || is_network_location(path, value) {
        return None;
    }
    let mut current = PathBuf::new();
    let mut final_metadata = None;
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&current).ok()?;
        if metadata.file_type().is_symlink() {
            return None;
        }
        final_metadata = Some(metadata);
    }
    final_metadata
}

fn is_network_location(path: &Path, value: &str) -> bool {
    if value.starts_with("//") || value.starts_with("\\\\") {
        return true;
    }
    #[cfg(unix)]
    {
        ["/Volumes", "/Network", "/net", "/mnt", "/media"]
            .iter()
            .any(|root| path.starts_with(root))
            || path.starts_with("/run/user") && value.contains("/gvfs/")
    }
    #[cfg(not(unix))]
    {
        false
    }
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
