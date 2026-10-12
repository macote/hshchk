use super::*;

#[test]
fn invalid_library_targets_and_mode_options_fail_before_processing() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    fs::write(&file, "data").unwrap();
    assert!(HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: file,
        ..Default::default()
    })
    .is_err());
    for options in [
        HashFileProcessOptions {
            base_path: root.path().into(),
            dry_run: true,
            ..Default::default()
        },
        HashFileProcessOptions {
            base_path: root.path().into(),
            remove_missing_in_update: true,
            ..Default::default()
        },
        HashFileProcessOptions {
            base_path: root.path().into(),
            checksum_file: Some(root.path().into()),
            ..Default::default()
        },
    ] {
        assert!(HashFileProcessor::try_new(options).is_err());
    }
}

#[test]
fn legacy_completion_subscription_and_relative_report_exclusion_work() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let mut processor = HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.path().into(),
        ..Default::default()
    })
    .unwrap();
    let (sender, receiver) = crossbeam::channel::unbounded();
    processor.set_complete_event_sender(sender);
    let report = format!(
        "{}-report.txt",
        root.path().file_name().unwrap().to_string_lossy()
    );
    processor.exclude_output_file(Path::new(&report)).unwrap();
    assert!(!Path::new(&report).exists());
    assert_eq!(processor.process_outcome().status, ProcessStatus::Success);
    assert_eq!(receiver.try_recv().unwrap(), HashFileProcessResult::Success);
    assert!(receiver.try_recv().is_err());
}

#[test]
fn disappearing_or_nonregular_worker_inputs_are_retained_as_errors() {
    let root = tempfile::tempdir().unwrap();
    let mut processor = HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.path().into(),
        ..Default::default()
    })
    .unwrap();
    let directory = processor.base_path.join("directory");
    fs::create_dir(&directory).unwrap();
    processor.process_file(&directory);
    let missing = processor.base_path.join("removed-before-open");
    processor.process_file(&missing);
    assert_eq!(processor.stats().errors, 2);
    assert!(processor.has_operational_errors());
    assert!(!processor.manifest_written);
    assert!(processor.diagnostics.iter().any(|entry| matches!(&entry.state, FileProcessState::Error(message) if message == "not a regular file")));
}

#[cfg(windows)]
#[test]
fn unpaired_surrogate_paths_are_rejected_before_filesystem_access() {
    use std::os::windows::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let mut processor = HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.path().into(),
        ..Default::default()
    })
    .unwrap();
    let path = processor
        .base_path
        .join(std::ffi::OsString::from_wide(&[b'f' as u16, 0xd800]));
    processor.process_file(&path);
    assert_eq!(processor.stats().errors, 1);
    assert!(matches!(
        processor.diagnostics[0].state,
        FileProcessState::InvalidUnicodeFileName
    ));
}
#[test]
fn output_flush_failure_prevents_manifest_replacement() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    fs::write(&path, "data").unwrap();
    let mut create = HashFileProcessor::new(HashFileProcessOptions {
        base_path: root.path().to_path_buf(),
        ..Default::default()
    });
    assert_eq!(create.process(), HashFileProcessResult::Success);
    let manifest = root.path().join("hshchk.sha1");
    let original = fs::read(&manifest).unwrap();
    fs::write(&path, "tada").unwrap();
    let mut update = HashFileProcessor::new(HashFileProcessOptions {
        base_path: root.path().to_path_buf(),
        update_mode: true,
        ..Default::default()
    });
    update.set_before_save(|| Err(io::Error::other("report write failed")));
    let outcome = update.process_outcome();
    assert_eq!(outcome.status, ProcessStatus::Error);
    assert!(!outcome.manifest_written);
    assert_eq!(outcome.diagnostics.len(), 1);
    assert_eq!(fs::read(&manifest).unwrap(), original);
    assert_eq!(update.stats().errors, 1);
}

#[test]
fn cancellation_before_save_retains_completed_statistics() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let token = CancellationToken::new();
    let cancel = token.clone();
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: root.path().into(),
        ..Default::default()
    });
    processor.set_before_save(move || {
        cancel.cancel();
        Ok(())
    });
    let outcome = processor.process_outcome_with_cancellation_token(token);
    assert_eq!(outcome.status, ProcessStatus::Canceled);
    assert_eq!(outcome.stats.bytes_hashed, 4);
    assert_eq!(outcome.stats.files_processed, 1);
    assert!(!outcome.manifest_written);
    assert!(!root.path().join("hshchk.sha1").exists());
}

#[test]
fn create_does_not_replace_a_manifest_that_appears_after_setup() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let manifest = root.path().join("hshchk.sha1");
    let late_manifest = manifest.clone();
    let mut processor = HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.path().into(),
        process_type: Some(HashFileProcessType::Create),
        ..Default::default()
    })
    .unwrap();
    processor.set_before_save(move || fs::write(&late_manifest, "created by another writer"));
    let outcome = processor.process_outcome();
    assert_eq!(outcome.status, ProcessStatus::Error);
    assert!(!outcome.manifest_written);
    assert_eq!(
        fs::read_to_string(&manifest).unwrap(),
        "created by another writer"
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
}
