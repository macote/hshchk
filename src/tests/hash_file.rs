use super::*;

#[test]
fn blank_manifest_detection_and_loading_are_consistent() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("manifest");
    std::fs::write(&path, "\n  \n\r\n").unwrap();
    assert_eq!(HashFile::detect_format(&path).unwrap(), None);
    let mut manifest = HashFile::new();
    manifest.load(&path).unwrap();
    assert!(manifest.is_empty());
}

#[test]
fn malformed_path_and_size_boundaries_are_rejected() {
    for line in ["file|4", "file|4|abc|extra", ".|4|abc", "|4|abc"] {
        assert_eq!(
            parse_entry(line, HashFileFormat::HashCheck)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
    assert!(parse_entry(
        &format!("{}|4|abc", "x".repeat(MAX_PATH_SIZE + 1)),
        HashFileFormat::HashCheck
    )
    .is_err());
    assert!(parse_entry(
        &format!("file|4|{}", "a".repeat(MAX_HASH_SIZE + 1)),
        HashFileFormat::HashCheck
    )
    .is_err());
}

#[test]
fn missing_recorded_size_cannot_replace_an_existing_manifest() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("manifest");
    std::fs::write(&path, "original").unwrap();
    let mut manifest = HashFile::new();
    manifest.add_entry(HashFileEntry {
        file_path: "file".into(),
        size: None,
        digest: "abc".into(),
    });
    assert!(manifest
        .save(
            &path,
            HashFileFormat::HashCheck,
            &CancellationToken::new(),
            true
        )
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn empty_save_honors_cancellation_before_replacement() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("manifest");
    std::fs::write(&path, "original").unwrap();
    let empty = HashFile::new();
    let canceled = CancellationToken::new();
    canceled.cancel();
    assert_eq!(
        empty
            .save(&path, HashFileFormat::HashSum, &canceled, true)
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    let new_path = root.path().join("new-manifest");
    empty
        .save(
            &new_path,
            HashFileFormat::HashSum,
            &CancellationToken::new(),
            true,
        )
        .unwrap();
    assert_eq!(std::fs::read(&new_path).unwrap(), b"");
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
}
#[test]
fn canceled_save_preserves_original_and_cleans_temporary_file() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("manifest");
    std::fs::write(&path, "original").unwrap();
    let mut manifest = HashFile::new();
    manifest.add_entry(HashFileEntry {
        file_path: "file".into(),
        size: Some(4),
        digest: "digest".into(),
    });
    let token = CancellationToken::new();
    token.cancel();
    let error = manifest
        .save(&path, HashFileFormat::HashCheck, &token, true)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}
#[test]
fn checksum_paths_with_pipes_are_not_misdetected_as_legacy_format() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("SHA1SUMS");
    std::fs::write(
        &path,
        "a17c9aaa61e80a1bf71d0d850af4e5baa9800bbd *with|pipe\n",
    )
    .unwrap();
    let mut manifest = HashFile::new();
    manifest.load(&path).unwrap();
    assert!(manifest.get_entry("with|pipe").unwrap().size.is_none());
}
#[cfg(windows)]
#[test]
fn failed_replacement_preserves_original_and_cleans_temporary_file() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("manifest");
    std::fs::write(&path, "original").unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let mut manifest = HashFile::new();
    manifest.add_entry(HashFileEntry {
        file_path: "file".into(),
        size: Some(4),
        digest: "digest".into(),
    });
    assert!(manifest
        .save(
            &path,
            HashFileFormat::HashCheck,
            &CancellationToken::new(),
            true
        )
        .is_err());
    drop(lock);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}
