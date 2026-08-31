use crate::input::{
    BoundaryError, MAX_INPUT_BYTES, decode_bounded, read_bounded_file, read_bounded_file_bytes,
};
#[cfg(unix)]
use crate::input::{
    current_user_home, local_directory_exists, local_executable_exists, validate_directory_root,
};
#[cfg(unix)]
use crate::process::{MAX_OUTPUT_BYTES, run_program_for_test};
use crate::process::{NativeTool, decode_native_output_for_test};
use proptest::prelude::*;
use std::path::Path;
#[cfg(unix)]
use std::time::Duration;

#[test]
fn rejects_invalid_utf8_and_oversized_inputs() {
    assert!(matches!(
        decode_bounded(&[0xff]),
        Err(BoundaryError::InvalidEncoding)
    ));
    assert!(matches!(
        decode_bounded(&vec![b'a'; MAX_INPUT_BYTES + 1]),
        Err(BoundaryError::InputTooLarge { .. })
    ));
}

#[test]
fn rejects_files_outside_an_allowlisted_root() {
    let allowed = tempfile::tempdir().expect("allowed temp directory");
    let outside = tempfile::NamedTempFile::new().expect("outside temp file");

    assert!(matches!(
        read_bounded_file(outside.path(), &[allowed.path()]),
        Err(BoundaryError::PathNotAllowed)
    ));
}

#[cfg(unix)]
#[test]
fn bounded_binary_reader_does_not_require_text_encoding() {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = directory.path().join("binary.plist");
    std::fs::write(&path, [0xff, 0x00, 0x01]).expect("binary fixture write");

    assert_eq!(
        read_bounded_file_bytes(&path, &[directory.path()]).expect("bounded binary read"),
        [0xff, 0x00, 0x01]
    );
    assert!(matches!(
        read_bounded_file(&path, &[directory.path()]),
        Err(BoundaryError::InvalidEncoding)
    ));
}

#[cfg(not(unix))]
#[test]
fn file_boundary_fails_closed_without_component_relative_no_follow_support() {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = directory.path().join("definition");
    std::fs::write(&path, b"fixture").expect("fixture write");

    assert!(matches!(
        read_bounded_file_bytes(&path, &[directory.path()]),
        Err(BoundaryError::SymlinkRejected)
    ));
}

#[cfg(unix)]
#[test]
fn rejects_symlink_inputs() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("temp directory");
    let target = directory.path().join("target.txt");
    let link = directory.path().join("link.txt");
    std::fs::write(&target, "safe fixture").expect("fixture write");
    symlink(&target, &link).expect("fixture symlink");

    assert!(matches!(
        read_bounded_file(&link, &[directory.path()]),
        Err(BoundaryError::SymlinkRejected)
    ));
}

#[cfg(unix)]
#[test]
fn rejects_a_symlink_in_any_parent_component() {
    use std::os::unix::fs::symlink;

    let allowed = tempfile::tempdir().expect("allowed temp directory");
    let outside = tempfile::tempdir().expect("outside temp directory");
    std::fs::write(outside.path().join("definition"), "secret fixture")
        .expect("outside fixture write");
    let linked_directory = allowed.path().join("linked");
    symlink(outside.path(), &linked_directory).expect("parent symlink");

    assert!(matches!(
        read_bounded_file(&linked_directory.join("definition"), &[allowed.path()]),
        Err(BoundaryError::SymlinkRejected)
    ));
}

#[cfg(unix)]
#[test]
fn current_home_is_derived_from_the_process_identity() {
    let home = current_user_home().expect("current user home directory");
    assert!(home.is_absolute());
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_roots_and_path_probe_type_confusion() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let parent = tempfile::tempdir().expect("parent temp directory");
    let root = parent.path().join("root");
    let root_link = parent.path().join("root-link");
    std::fs::create_dir(&root).expect("fixture root");
    symlink(&root, &root_link).expect("fixture root symlink");
    assert!(matches!(
        validate_directory_root(&root_link),
        Err(BoundaryError::SymlinkRejected)
    ));

    let executable = root.join("tool");
    let directory = root.join("working");
    let executable_link = root.join("tool-link");
    std::fs::write(&executable, "fixture").expect("fixture executable");
    let mut permissions = std::fs::metadata(&executable)
        .expect("fixture metadata")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&executable, permissions).expect("fixture permissions");
    std::fs::create_dir(&directory).expect("fixture working directory");
    symlink(&executable, &executable_link).expect("fixture executable symlink");

    let executable = executable.canonicalize().expect("canonical executable");
    let directory = directory.canonicalize().expect("canonical directory");
    assert!(local_executable_exists(
        executable.to_str().expect("UTF-8 path")
    ));
    assert!(!local_executable_exists(
        directory.to_str().expect("UTF-8 path")
    ));
    assert!(!local_executable_exists(
        executable_link.to_str().expect("UTF-8 path")
    ));
    assert!(local_directory_exists(
        directory.to_str().expect("UTF-8 path")
    ));
    assert!(!local_directory_exists(
        executable.to_str().expect("UTF-8 path")
    ));
    assert!(!local_directory_exists("//remote.example/share"));
}

#[test]
fn native_tools_resolve_to_fixed_absolute_paths() {
    #[cfg(unix)]
    let tools = [NativeTool::Launchctl, NativeTool::Systemctl].as_slice();
    #[cfg(windows)]
    let tools = [NativeTool::Schtasks, NativeTool::PowerShell].as_slice();
    for tool in tools {
        let program = tool.program().expect("fixed native tool path");
        assert!(Path::new(&program).is_absolute(), "{program:?}");
    }
}

#[test]
fn native_output_decoder_accepts_real_utf16le_shape() {
    let mut bytes = vec![0xff, 0xfe];
    for unit in "<Tasks/>".encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    assert_eq!(
        decode_native_output_for_test(bytes).expect("UTF-16LE output"),
        "<Tasks/>"
    );
}

#[cfg(unix)]
#[test]
fn native_process_runner_caps_output_and_times_out() {
    let capped = run_program_for_test("/usr/bin/yes", &[], Duration::from_secs(2));
    assert!(matches!(
        capped,
        Err(BoundaryError::OutputTooLarge {
            limit: MAX_OUTPUT_BYTES
        })
    ));

    let timed_out = run_program_for_test("/bin/sleep", &["2"], Duration::from_millis(20));
    assert!(matches!(timed_out, Err(BoundaryError::ProcessTimeout)));
}

proptest! {
    #[test]
    fn bounded_decoder_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = decode_bounded(&bytes);
    }
}
