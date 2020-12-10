use crossbeam::channel::unbounded;
use std::fs;
use std::path::PathBuf;

use hshchk::hash_file_process::*;
use hshchk::{HashFileFormat, HashType};

extern crate test_shared;
// #[path = "../src/test/mod.rs"]
// mod test;

static HASHCHECK_SHA1_NAME: &str = "hshchk.sha1";
static HASHCHECK_MD5_NAME: &str = "hshchk.md5";
static HASHSUM_SHA1_NAME: &str = "SHA1SUMS";
static HASHCHECK_SHA1_CONTENT: &str = "file|4|a17c9aaa61e80a1bf71d0d850af4e5baa9800bbd\n";
static HASHCHECK_MD5_CONTENT: &str = "file|4|8d777f385d3dfec8815d20f7496026dc\n";
static HASHSUM_SHA1_CONTENT: &str = "a17c9aaa61e80a1bf71d0d850af4e5baa9800bbd *file\n";

#[test]
fn hash_file_process_create_no_files_processed() {
    let dir = test_shared::create_tmp_dir();
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        hash_type: Some(HashType::SHA1),
        ..Default::default()
    });
    assert_eq!(processor.process(), HashFileProcessResult::NoFilesProcessed);
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "data");
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        hash_type: Some(HashType::SHA1),
        ..Default::default()
    });
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join(HASHCHECK_SHA1_NAME);
    assert_eq!(
        test_shared::get_file_string_content(&checksum_file),
        HASHCHECK_SHA1_CONTENT
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create_md5() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "data");
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        hash_type: Some(HashType::MD5),
        ..Default::default()
    });
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join(HASHCHECK_MD5_NAME);
    assert_eq!(
        test_shared::get_file_string_content(&checksum_file),
        HASHCHECK_MD5_CONTENT
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create_force() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "data");
    let checksum_file = test_shared::create_file_with_content(&dir, HASHCHECK_SHA1_NAME, "test");
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        hash_type: Some(HashType::SHA1),
        force_create: Some(true),
        ..Default::default()
    });

    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert_eq!(
        test_shared::get_file_string_content(&checksum_file),
        HASHCHECK_SHA1_CONTENT
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create_ignore() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "data");
    let _ = test_shared::create_file_with_content(&dir, "ignore", "test");
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        ignore_pattern: Some("ignore"),
        ..Default::default()
    });
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join(HASHCHECK_SHA1_NAME);
    assert_eq!(
        test_shared::get_file_string_content(&checksum_file),
        HASHCHECK_SHA1_CONTENT
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create_match() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "data");
    let _ = test_shared::create_file_with_content(&dir, "unmatched", "test");
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        match_pattern: Some("file"),
        ..Default::default()
    });
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join(HASHCHECK_SHA1_NAME);
    assert_eq!(
        test_shared::get_file_string_content(&checksum_file),
        HASHCHECK_SHA1_CONTENT
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "data");
    let _ =
        test_shared::create_file_with_content(&dir, HASHCHECK_SHA1_NAME, HASHCHECK_SHA1_CONTENT);
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        ..Default::default()
    });
    let (sender, receiver) = unbounded();
    let sender_error = sender.clone();
    processor.set_error_event_sender(sender_error);
    let sender_warning = sender.clone();
    processor.set_warning_event_sender(sender_warning);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert!(receiver.try_recv().is_err());
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify_missing() {
    let dir = test_shared::create_tmp_dir();
    let _ =
        test_shared::create_file_with_content(&dir, HASHCHECK_SHA1_NAME, HASHCHECK_SHA1_CONTENT);
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        ..Default::default()
    });
    let (sender, receiver) = unbounded();
    let sender_error = sender.clone();
    processor.set_error_event_sender(sender_error);
    assert_eq!(processor.process(), HashFileProcessResult::Error);
    assert_eq!(
        FileProcessEntry {
            file_path: PathBuf::from("file"),
            state: FileProcessState::Missing
        },
        receiver.recv().unwrap()
    );
    assert!(receiver.try_recv().is_err());
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify_incorrect_size() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "datadata");
    let _ =
        test_shared::create_file_with_content(&dir, HASHCHECK_SHA1_NAME, HASHCHECK_SHA1_CONTENT);
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        ..Default::default()
    });
    let (sender, receiver) = unbounded();
    let sender_error = sender.clone();
    processor.set_error_event_sender(sender_error);
    assert_eq!(processor.process(), HashFileProcessResult::Error);
    assert_eq!(
        FileProcessEntry {
            file_path: PathBuf::from("file"),
            state: FileProcessState::IncorrectSize
        },
        receiver.recv().unwrap()
    );
    assert!(receiver.try_recv().is_err());
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify_incorrect_hash() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "tada");
    let _ =
        test_shared::create_file_with_content(&dir, HASHCHECK_SHA1_NAME, HASHCHECK_SHA1_CONTENT);
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        ..Default::default()
    });
    let (sender, receiver) = unbounded();
    let sender_error = sender.clone();
    processor.set_error_event_sender(sender_error);
    assert_eq!(processor.process(), HashFileProcessResult::Error);
    assert_eq!(
        FileProcessEntry {
            file_path: PathBuf::from("file"),
            state: FileProcessState::IncorrectHash
        },
        receiver.recv().unwrap()
    );
    assert!(receiver.try_recv().is_err());
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify_report_extra() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "data");
    let _ =
        test_shared::create_file_with_content(&dir, HASHCHECK_SHA1_NAME, HASHCHECK_SHA1_CONTENT);
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        report_extra: Some(true),
        ..Default::default()
    });
    let (error_sender, error_receiver) = unbounded();
    let (warning_sender, warning_receiver) = unbounded();
    let sender_error = error_sender.clone();
    processor.set_error_event_sender(sender_error);
    let sender_warning = warning_sender.clone();
    processor.set_warning_event_sender(sender_warning);
    let _ = test_shared::create_file_with_content(&dir, "extra", "test");
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert_eq!(
        FileProcessEntry {
            file_path: PathBuf::from("extra"),
            state: FileProcessState::Extra
        },
        warning_receiver.recv().unwrap()
    );
    assert!(error_receiver.try_recv().is_err());
    assert!(warning_receiver.try_recv().is_err());
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify_size_only() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "tada");
    let _ =
        test_shared::create_file_with_content(&dir, HASHCHECK_SHA1_NAME, HASHCHECK_SHA1_CONTENT);
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        size_only: Some(true),
        ..Default::default()
    });
    let (error_sender, error_receiver) = unbounded();
    let (warning_sender, warning_receiver) = unbounded();
    let sender_error = error_sender.clone();
    processor.set_error_event_sender(sender_error);
    let sender_warning = warning_sender.clone();
    processor.set_warning_event_sender(sender_warning);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert!(error_receiver.try_recv().is_err());
    assert!(warning_receiver.try_recv().is_err());
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify_ignore() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "data");
    let ignore_file = test_shared::create_file_with_content(&dir, "ignore", "test");
    let mut processor_create = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        ..Default::default()
    });
    assert_eq!(processor_create.process(), HashFileProcessResult::Success);
    let mut processor_verify = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        ignore_pattern: Some("ignore"),
        ..Default::default()
    });
    std::fs::remove_file(ignore_file).expect("Failed to remove ignored file.");
    assert_eq!(processor_verify.process(), HashFileProcessResult::Success);
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify_match() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "data");
    let unmatched_file = test_shared::create_file_with_content(&dir, "unmatched", "test");
    let mut processor_create = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        ..Default::default()
    });
    assert_eq!(processor_create.process(), HashFileProcessResult::Success);
    let mut processor_verify = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        match_pattern: Some("file"),
        ..Default::default()
    });
    std::fs::remove_file(unmatched_file).expect("Failed to remove unmatched file.");
    assert_eq!(processor_verify.process(), HashFileProcessResult::Success);
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_hashsum_create() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "data");
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        hash_type: Some(HashType::SHA1),
        hash_file_format: Some(HashFileFormat::HashSum),
        ..Default::default()
    });
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join(HASHSUM_SHA1_NAME);
    assert_eq!(
        test_shared::get_file_string_content(&checksum_file),
        HASHSUM_SHA1_CONTENT
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_hashsum_verify() {
    let dir = test_shared::create_tmp_dir();
    let _ = test_shared::create_file_with_content(&dir, "file", "data");
    let _ = test_shared::create_file_with_content(&dir, HASHSUM_SHA1_NAME, HASHSUM_SHA1_CONTENT);
    let mut processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: dir.clone(),
        ..Default::default()
    });
    let (sender, receiver) = unbounded();
    let sender_error = sender.clone();
    processor.set_error_event_sender(sender_error);
    let sender_warning = sender.clone();
    processor.set_warning_event_sender(sender_warning);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert!(receiver.try_recv().is_err());
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}
