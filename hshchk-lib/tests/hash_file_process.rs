use hshchk_lib::hash_file_process::{
    HashFileProcessOptions, HashFileProcessResult, HashFileProcessor,
};
use hshchk_lib::HashType;
use std::fs;

#[path="../src/test/mod.rs"]
mod test;

static FILE_DATA_CHECKSUM: &str = "file|4|a17c9aaa61e80a1bf71d0d850af4e5baa9800bbd\n";

#[test]
fn hash_file_process_create_empty() {
    let dir = test::create_tmp_dir();
    let mut processor = HashFileProcessor::new(&dir, HashType::SHA1, false);
    assert_eq!(processor.process(), HashFileProcessResult::Error);
    fs::remove_dir_all(dir).expect("failed to remove dir");
}

#[test]
fn hash_file_process_create() {
    let dir = test::create_tmp_dir();
    let _ = test::create_named_file(&dir, "file", "data");
    let mut processor = HashFileProcessor::new(&dir, HashType::SHA1, false);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join("hshchk.sha1");
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_CHECKSUM
    );
    fs::remove_dir_all(dir).expect("failed to remove dir");
}

#[test]
fn hash_file_process_create_force() {
    let dir = test::create_tmp_dir();
    let _ = test::create_named_file(&dir, "file", "data");
    let checksum_file = test::create_named_file(&dir, "hshchk.sha1", "test");
    let mut processor = HashFileProcessor::new(&dir, HashType::SHA1, true);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_CHECKSUM
    );
    fs::remove_dir_all(dir).expect("failed to remove dir");
}

#[test]
fn hash_file_process_create_ignore() {
    let dir = test::create_tmp_dir();
    let _ = test::create_named_file(&dir, "file", "data");
    let _ = test::create_named_file(&dir, "ignore", "test");
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
        base_path: dir.clone(),
        ignore_pattern: Some("ignore"),
        ..Default::default()
    });
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join("hshchk.sha1");
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_CHECKSUM
    );
    fs::remove_dir_all(dir).expect("failed to remove dir");
}

#[test]
fn hash_file_process_create_match() {
    let dir = test::create_tmp_dir();
    let _ = test::create_named_file(&dir, "file", "data");
    let _ = test::create_named_file(&dir, "unmatched", "test");
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
        base_path: dir.clone(),
        match_pattern: Some("file"),
        ..Default::default()
    });
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join("hshchk.sha1");
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_CHECKSUM
    );
    fs::remove_dir_all(dir).expect("failed to remove dir");
}
