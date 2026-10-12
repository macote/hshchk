use super::*;
use crate::hash_file_process::ProcessDiagnostic;

#[test]
fn every_diagnostic_variant_has_a_stable_json_kind() {
    for (state, expected) in [
        (FileProcessState::Extra, "extra"),
        (
            FileProcessState::InvalidUnicodeFileName,
            "invalid_unicode_file_name",
        ),
        (FileProcessState::Missing, "missing"),
        (FileProcessState::IncorrectSize, "incorrect_size"),
        (FileProcessState::IncorrectHash, "incorrect_hash"),
        (FileProcessState::Error("reason".into()), "error"),
    ] {
        let outcome = ProcessOutcome {
            process_type: HashFileProcessType::Verify,
            status: ProcessStatus::Error,
            stats: ProcessStats::default(),
            diagnostics: vec![ProcessDiagnostic {
                level: DiagnosticLevel::Error,
                file_path: "file".into(),
                state,
            }],
            dry_run: false,
            manifest_written: false,
        };
        let value = json_report(&outcome);
        assert_eq!(value["diagnostics"][0]["kind"], expected);
        assert_eq!(value["diagnostics"][0]["path"], "file");
    }
}

#[test]
fn json_writer_propagates_write_and_flush_failures() {
    struct FailingWriter {
        data: Vec<u8>,
        fail_write: bool,
    }
    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_write {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"));
            }
            self.data.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "flush failed",
            ))
        }
    }
    let value = error_report(None, false, true, "invalid arguments");
    let mut writer = FailingWriter {
        data: Vec::new(),
        fail_write: true,
    };
    assert_eq!(
        write_json(&value, &mut writer).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert!(writer.data.is_empty());
    writer.fail_write = false;
    assert_eq!(
        write_json(&value, &mut writer).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(writer.data.last(), Some(&b'\n'));
    assert_eq!(
        serde_json::from_slice::<Value>(&writer.data).unwrap(),
        value
    );
}

#[cfg(windows)]
#[test]
fn unpaired_surrogate_diagnostic_paths_are_explicitly_lossy() {
    use std::os::windows::ffi::OsStringExt;
    let path = std::path::PathBuf::from(std::ffi::OsString::from_wide(&[b'f' as u16, 0xd800]));
    let value = diagnostic("error", Some(&path), "error", "reason");
    assert_eq!(value["path"], "f\u{fffd}");
    assert_eq!(value["path_is_lossy"], true);
}
