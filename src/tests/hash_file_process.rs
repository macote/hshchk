use super::*;
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
