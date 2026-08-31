use std::fs::File;
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
    let mut file = open_beneath_allowed_root(path, allowed_roots)?;
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

pub fn valid_environment_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .next()
            .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && value
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[cfg(unix)]
pub fn current_user_home() -> Option<PathBuf> {
    use std::ffi::CStr;
    use std::ffi::OsString;
    use std::mem::MaybeUninit;
    use std::os::unix::ffi::OsStringExt;

    let suggested = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let buffer_size = if suggested > 0 {
        usize::try_from(suggested).ok()?.min(MAX_INPUT_BYTES)
    } else {
        16 * 1024
    };
    let mut buffer = vec![0_u8; buffer_size];
    let mut password_record = MaybeUninit::<libc::passwd>::uninit();
    let mut result = std::ptr::null_mut();
    let status = unsafe {
        libc::getpwuid_r(
            libc::getuid(),
            password_record.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if status != 0 || result.is_null() {
        return None;
    }
    let password_record = unsafe { password_record.assume_init() };
    if password_record.pw_dir.is_null() {
        return None;
    }
    let bytes = unsafe { CStr::from_ptr(password_record.pw_dir) }.to_bytes();
    Some(PathBuf::from(OsString::from_vec(bytes.to_vec())))
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
        let _ = path;
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

fn open_beneath_allowed_root(path: &Path, allowed_roots: &[&Path]) -> Result<File, BoundaryError> {
    for root in allowed_roots {
        if let Ok(relative) = path.strip_prefix(root) {
            return open_beneath_root(root, relative);
        }
    }
    Err(BoundaryError::PathNotAllowed)
}

#[cfg(unix)]
fn open_beneath_root(root: &Path, relative: &Path) -> Result<File, BoundaryError> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::OpenOptionsExt;

    let mut root_options = std::fs::OpenOptions::new();
    root_options
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let root_directory = root_options.open(root).map_err(map_no_follow_error)?;
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(BoundaryError::PathNotAllowed);
    }

    let mut directory: Option<OwnedFd> = None;
    for (index, component) in components.iter().enumerate() {
        let std::path::Component::Normal(name) = component else {
            return Err(BoundaryError::PathNotAllowed);
        };
        let name = CString::new(name.as_bytes()).map_err(|_| BoundaryError::PathNotAllowed)?;
        let parent_fd = directory
            .as_ref()
            .map_or(root_directory.as_raw_fd(), AsRawFd::as_raw_fd);
        let is_final = index + 1 == components.len();
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | if is_final { 0 } else { libc::O_DIRECTORY };
        let file_descriptor = unsafe { libc::openat(parent_fd, name.as_ptr(), flags) };
        if file_descriptor < 0 {
            return Err(map_no_follow_error(std::io::Error::last_os_error()));
        }
        let opened = unsafe { OwnedFd::from_raw_fd(file_descriptor) };
        if is_final {
            return Ok(File::from(opened));
        }
        directory = Some(opened);
    }
    Err(BoundaryError::PathNotAllowed)
}

#[cfg(unix)]
fn map_no_follow_error(error: std::io::Error) -> BoundaryError {
    if matches!(error.raw_os_error(), Some(code) if code == libc::ELOOP || code == libc::ENOTDIR) {
        BoundaryError::SymlinkRejected
    } else {
        BoundaryError::FileRead(error.to_string())
    }
}

#[cfg(not(unix))]
fn open_beneath_root(_root: &Path, _relative: &Path) -> Result<File, BoundaryError> {
    // The Windows scanners consume bounded native command output rather than scheduler files.
    // Fail closed if this file boundary is ever reached on a platform where component-relative
    // no-follow opens are not implemented.
    Err(BoundaryError::SymlinkRejected)
}
