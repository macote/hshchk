use super::*;
use crate::block_hasher::Hasher;
use crate::hash_file::HashFile;
use hash_file::HashFileEntry;
use std::fs;
use tokio_util::sync::CancellationToken;

// block hasher

// ...

// file hash
#[test]
fn file_hash_empty_file() {
    let file = test_shared::create_tmp_file("");
    let mut file_hash = get_md5_file_hasher(&file).unwrap();
    let cancellation_token = CancellationToken::new();
    file_hash
        .compute_with_progress(cancellation_token.clone(), &mut |_| {})
        .unwrap();
    let digest = file_hash.digest();
    assert_eq!(digest, "d41d8cd98f00b204e9800998ecf8427e");
    drop(file_hash); // force release of file handle (Windows)
    fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
}

#[test]
fn file_hash_data_file() {
    let file = test_shared::create_tmp_file("data");
    let mut file_hash = get_md5_file_hasher(&file).unwrap();
    let cancellation_token = CancellationToken::new();
    file_hash
        .compute_with_progress(cancellation_token.clone(), &mut |_| {})
        .unwrap();
    let digest = file_hash.digest();
    assert_eq!(digest, "8d777f385d3dfec8815d20f7496026dc");
    drop(file_hash); // force release of file handle (Windows)
    fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
}

#[test]
fn file_hash_data_two_blocks() {
    let file = test_shared::create_tmp_file("datadata");
    let mut file_hash: FileHash<Md5> = FileHash::new_with_buffer_size(&file, 2).unwrap();
    let mut progress = Vec::new();
    let cancellation_token = CancellationToken::new();
    file_hash
        .compute_with_progress(cancellation_token, &mut |bytes| progress.push(bytes))
        .unwrap();
    let digest = file_hash.digest();
    assert_eq!(digest, "511ae0b1c13f95e5f08f1a0dd3da3d93");
    assert_eq!(progress, vec![2, 4, 6, 8]);
    drop(file_hash); // force release of file handle (Windows)
    fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
}

// hash file

#[test]
fn hash_file_load_single() {
    let file = test_shared::create_tmp_file("filename|0|hash");
    let mut hash_file = HashFile::new();
    hash_file.load(&file).unwrap();
    assert_eq!(1, hash_file.get_file_paths().len());
    let entry = hash_file.get_entry("filename").unwrap();
    assert_eq!(0, entry.size.unwrap());
    assert_eq!("hash", entry.digest);
    fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_load_multiple() {
    let file = test_shared::create_tmp_file("filename1|1|hash1\r\nfilename2|2|hash2");
    let mut hash_file = HashFile::new();
    hash_file.load(&file).unwrap();
    assert_eq!(2, hash_file.get_file_paths().len());
    let entry = hash_file.get_entry("filename1").unwrap();
    assert_eq!(1, entry.size.unwrap());
    assert_eq!("hash1", entry.digest);
    let entry = hash_file.get_entry("filename2").unwrap();
    assert_eq!(2, entry.size.unwrap());
    assert_eq!("hash2", entry.digest);
    fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_rejects_malformed_entries() {
    for content in [
        "filename|size|hash".to_string(),
        "a".repeat(4096) + "|0|hash",
        "filename|0|".to_string() + &"a".repeat(1025),
        "digest ".to_string(),
        "../outside|0|hash".to_string(),
    ] {
        let file = test_shared::create_tmp_file(&content);
        let mut hash_file = HashFile::new();
        assert!(hash_file.load(&file).is_err());
        assert!(hash_file.is_empty());
        fs::remove_dir_all(file.parent().unwrap()).unwrap();
    }
}

#[test]
fn hash_file_is_empty() {
    let hash_file = HashFile::new();
    assert!(hash_file.is_empty());
}

#[test]
fn hash_file_is_not_empty() {
    let mut hash_file = HashFile::new();
    hash_file.add_entry(HashFileEntry {
        file_path: "filename".into(),
        size: None,
        digest: "hash".into(),
    });
    assert!(!hash_file.is_empty());
}

#[test]
fn hash_file_get_file_paths() {
    let mut hash_file = HashFile::new();
    hash_file.add_entry(HashFileEntry {
        file_path: "filename1".into(),
        size: None,
        digest: "hash1".into(),
    });
    hash_file.add_entry(HashFileEntry {
        file_path: "filename2".into(),
        size: None,
        digest: "hash2".into(),
    });
    let mut filenames = hash_file.get_file_paths();
    filenames.sort();
    assert_eq!("filename1filename2", filenames.join(""));
}

#[test]
fn hash_file_remove_entry() {
    let mut hash_file = HashFile::new();
    hash_file.add_entry(HashFileEntry {
        file_path: "filename".into(),
        size: None,
        digest: "hash".into(),
    });
    hash_file.remove_entry("filename");
    assert!(hash_file.is_empty());
}
