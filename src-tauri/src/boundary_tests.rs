use crate::input::{BoundaryError, MAX_INPUT_BYTES, decode_bounded, read_bounded_file};
use crate::process::{MAX_OUTPUT_BYTES, run_program_for_test};
use proptest::prelude::*;
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
