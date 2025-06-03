use hshchk::hash_file_process::*;
use hshchk::HashFileFormat;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

fn create(root: &Path, format: HashFileFormat) {
    let mut processor = HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.to_path_buf(),
        hash_file_format: Some(format),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(processor.process(), HashFileProcessResult::Success);
}
fn update(root: &Path, remove: bool, dry_run: bool) -> HashFileProcessor {
    HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.to_path_buf(),
        update_mode: true,
        remove_missing_in_update: remove,
        dry_run,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn same_size_edits_are_rehashed_in_both_formats() {
    for format in [HashFileFormat::HashCheck, HashFileFormat::HashSum] {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("file"), "data").unwrap();
        create(root.path(), format);
        fs::write(root.path().join("file"), "tada").unwrap();
        let mut processor = update(root.path(), false, false);
        assert_eq!(processor.process(), HashFileProcessResult::Success);
        assert_eq!(processor.stats().files_updated, 1);
        assert_eq!(processor.stats().bytes_hashed, 4);
        let mut verify = HashFileProcessor::new(HashFileProcessOptions {
            base_path: root.path().to_path_buf(),
            ..Default::default()
        });
        assert_eq!(verify.process(), HashFileProcessResult::Success);
    }
}

#[test]
fn unchanged_files_are_verified_by_hashing() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    create(root.path(), HashFileFormat::HashCheck);
    let mut processor = update(root.path(), false, false);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert_eq!(processor.stats().files_unchanged, 1);
    assert_eq!(processor.stats().bytes_hashed, 4);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert_eq!(processor.stats().files_processed, 1); // Statistics reset between runs.
}

#[test]
fn missing_entries_are_kept_until_removal_is_requested() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    create(root.path(), HashFileFormat::HashCheck);
    fs::remove_file(root.path().join("file")).unwrap();
    let manifest = root.path().join("hshchk.sha1");
    let before = fs::read(&manifest).unwrap();
    let mut keep = update(root.path(), false, false);
    assert_eq!(keep.process(), HashFileProcessResult::Success);
    assert_eq!(keep.stats().missing_files, 1);
    assert_eq!(fs::read(&manifest).unwrap(), before);
    let mut remove = update(root.path(), true, false);
    assert_eq!(remove.process(), HashFileProcessResult::Success);
    assert_eq!(remove.stats().files_removed, 1);
    assert!(fs::read(&manifest).unwrap().is_empty());
}

#[test]
fn filters_preserve_excluded_entries_even_when_missing() {
    for use_ignore in [true, false] {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("sub")).unwrap();
        fs::write(root.path().join("sub/file"), "data").unwrap();
        fs::write(root.path().join("excluded"), "keep").unwrap();
        create(root.path(), HashFileFormat::HashCheck);
        fs::remove_file(root.path().join("excluded")).unwrap();
        let mut processor = HashFileProcessor::try_new(HashFileProcessOptions {
            base_path: root.path().to_path_buf(),
            update_mode: true,
            remove_missing_in_update: true,
            ignore_pattern: use_ignore.then_some("^excluded$"),
            match_pattern: (!use_ignore).then_some("^sub/file$"),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(processor.process(), HashFileProcessResult::Success);
        assert_eq!(processor.stats().files_removed, 0);
        assert_eq!(processor.stats().files_processed, 1);
        assert!(fs::read_to_string(root.path().join("hshchk.sha1"))
            .unwrap()
            .contains("excluded|"));
    }
}

#[test]
fn dry_run_reports_additions_changes_and_removals_without_saving() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("old"), "data").unwrap();
    fs::write(root.path().join("gone"), "gone").unwrap();
    create(root.path(), HashFileFormat::HashSum);
    let manifest = root.path().join("SHA1SUMS");
    let before = fs::read(&manifest).unwrap();
    fs::write(root.path().join("old"), "tada").unwrap();
    fs::write(root.path().join("new"), "new").unwrap();
    fs::remove_file(root.path().join("gone")).unwrap();
    let mut processor = update(root.path(), true, true);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert_eq!(
        (
            processor.stats().files_added,
            processor.stats().files_updated,
            processor.stats().files_removed
        ),
        (1, 1, 1)
    );
    assert_eq!(fs::read(&manifest).unwrap(), before);
}

#[test]
fn canceled_update_preserves_manifest_and_existing_tmp_file() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    create(root.path(), HashFileFormat::HashCheck);
    let manifest = root.path().join("hshchk.sha1");
    let before = fs::read(&manifest).unwrap();
    let token = CancellationToken::new();
    token.cancel();
    let mut processor = update(root.path(), true, false);
    assert_eq!(
        processor.process_with_cancellation_token(token),
        HashFileProcessResult::Canceled
    );
    assert_eq!(fs::read(&manifest).unwrap(), before);
    fs::write(root.path().join("hshchk.sha1.tmp"), "unrelated").unwrap();
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert_eq!(
        fs::read_to_string(root.path().join("hshchk.sha1.tmp")).unwrap(),
        "unrelated"
    );
}

#[test]
fn malformed_manifest_fails_without_rewriting() {
    for content in [
        "hash ",
        "file|size|hash",
        "../escape|4|a17c9aaa61e80a1bf71d0d850af4e5baa9800bbd",
        "file|4|bad",
    ] {
        let root = TempDir::new().unwrap();
        let manifest = root.path().join("hshchk.sha1");
        fs::write(&manifest, content).unwrap();
        let mut processor = update(root.path(), true, false);
        assert_eq!(processor.process(), HashFileProcessResult::Error);
        assert!(processor.stats().errors > 0);
        assert_eq!(fs::read_to_string(&manifest).unwrap(), content);
    }
}

#[test]
fn invalid_options_return_errors() {
    let root = TempDir::new().unwrap();
    assert!(HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.path().to_path_buf(),
        update_mode: true,
        ..Default::default()
    })
    .is_err());
    assert!(HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.path().to_path_buf(),
        match_pattern: Some("["),
        ..Default::default()
    })
    .is_err());
}

#[test]
fn size_only_counts_inspected_bytes_without_claiming_hashing() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    create(root.path(), HashFileFormat::HashCheck);
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: root.path().to_path_buf(),
        size_only: Some(true),
        ..Default::default()
    });
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert_eq!(processor.stats().bytes_size_checked, 4);
    assert_eq!(processor.stats().bytes_hashed, 0);
}

#[test]
fn manifest_entries_are_sorted() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("z"), "z").unwrap();
    fs::write(root.path().join("a"), "a").unwrap();
    create(root.path(), HashFileFormat::HashCheck);
    let text = fs::read_to_string(root.path().join("hshchk.sha1")).unwrap();
    assert!(text.lines().next().unwrap().starts_with("a|"));
}

#[cfg(windows)]
#[test]
fn locked_file_blocks_update_and_preserves_manifest() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = TempDir::new().unwrap();
    let file = root.path().join("file");
    fs::write(&file, "data").unwrap();
    create(root.path(), HashFileFormat::HashCheck);
    let manifest = root.path().join("hshchk.sha1");
    let before = fs::read(&manifest).unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(file)
        .unwrap();
    let mut processor = update(root.path(), true, false);
    assert_eq!(processor.process(), HashFileProcessResult::Error);
    assert_eq!(fs::read(&manifest).unwrap(), before);
    drop(lock);
}

#[cfg(unix)]
#[test]
fn directory_symlink_cycle_blocks_update_instead_of_recursing() {
    use std::os::unix::fs::symlink;
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    create(root.path(), HashFileFormat::HashCheck);
    let manifest = root.path().join("hshchk.sha1");
    let before = fs::read(&manifest).unwrap();
    symlink(root.path(), root.path().join("loop")).unwrap();
    let mut processor = update(root.path(), true, false);
    assert_eq!(processor.process(), HashFileProcessResult::Error);
    assert_eq!(fs::read(&manifest).unwrap(), before);
}
#[test]
fn progress_events_keep_file_identity_and_completion_bytes() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("first"), "abc").unwrap();
    fs::write(root.path().join("second"), "xyzxyz").unwrap();
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: root.path().to_path_buf(),
        ..Default::default()
    });
    let (sender, receiver) = crossbeam::channel::unbounded();
    processor.set_progress_event_sender_with_bytes_processed_notification_block_size(sender, 1);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let events: Vec<_> = receiver.try_iter().collect();
    for (name, size) in [("first", 3), ("second", 6)] {
        let events: Vec<_> = events
            .iter()
            .filter(|event| event.file_path == name)
            .collect();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].bytes_processed, 0);
        assert!(!events[0].complete);
        assert_eq!(events[1].bytes_processed, size);
        assert_eq!(events[2].bytes_processed, size);
        assert!(events[2].complete);
        assert!(events.iter().all(|event| event.file_size == size));
    }
}
