use super::*;
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
