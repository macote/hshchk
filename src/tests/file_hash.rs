use super::*;
use crate::block_hasher::BlockHasher;
use sha1::Sha1;
use tokio_util::sync::CancellationToken;

#[test]
fn zero_length_buffer_is_rejected_before_opening_input() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("does-not-exist");
    let error = FileHash::<Sha1>::new_with_buffer_size(&missing, 0)
        .err()
        .expect("zero buffer must be rejected");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(!missing.exists());
}

#[test]
fn same_size_changes_during_hashing_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    std::fs::write(&path, "datadata").unwrap();
    let mut hasher = FileHash::<Sha1>::new_with_buffer_size(&path, 2).unwrap();
    let mut changed = false;
    hasher
        .compute_with_progress(CancellationToken::new(), &mut |_| {
            if !changed {
                std::fs::write(&path, "tadatada").unwrap();
                let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
                file.set_times(
                    std::fs::FileTimes::new()
                        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1234)),
                )
                .unwrap();
                changed = true;
            }
        })
        .unwrap();
    assert!(hasher.validate_file(&path).is_err());
}

#[test]
fn replacing_a_file_during_hashing_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file");
    let replacement = root.path().join("replacement");
    std::fs::write(&path, "datadata").unwrap();
    std::fs::write(&replacement, "datadata").unwrap();
    let mut hasher = FileHash::<Sha1>::new_with_buffer_size(&path, 2).unwrap();
    let mut changed = false;
    hasher
        .compute_with_progress(CancellationToken::new(), &mut |_| {
            if !changed {
                std::fs::rename(&replacement, &path).unwrap();
                changed = true;
            }
        })
        .unwrap();
    assert!(hasher.validate_file(&path).is_err());
}
