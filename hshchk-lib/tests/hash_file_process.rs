use crossbeam::crossbeam_channel::unbounded;
use hshchk_lib::hash_file_process::*;
use hshchk_lib::HashType;
use std::fs;
use std::path::PathBuf;

#[path = "../src/test/mod.rs"]
mod test;

static FILE_CHECKSUM_SHA1: &str = "hshchk.sha1";
static FILE_CHECKSUM_MD5: &str = "hshchk.md5";
static FILE_DATA_SHA1: &str = "file|4|a17c9aaa61e80a1bf71d0d850af4e5baa9800bbd\n";
static FILE_DATA_MD5: &str = "file|4|8d777f385d3dfec8815d20f7496026dc\n";

#[test]
fn hash_file_process_create_no_files_processed() {
    let dir = test::create_tmp_dir();
    let mut processor = HashFileProcessor::new(&dir, HashType::SHA1, false);
    assert_eq!(processor.process(), HashFileProcessResult::NoFilesProcessed);
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create() {
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let mut processor = HashFileProcessor::new(&dir, HashType::SHA1, false);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join(FILE_CHECKSUM_SHA1);
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_SHA1
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create_md5() {
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let mut processor = HashFileProcessor::new(&dir, HashType::MD5, false);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join(FILE_CHECKSUM_MD5);
    assert_eq!(test::get_file_string_content(&checksum_file), FILE_DATA_MD5);
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create_force() {
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let checksum_file = test::create_file_with_content(&dir, FILE_CHECKSUM_SHA1, "test");
    let mut processor = HashFileProcessor::new(&dir, HashType::SHA1, true);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_SHA1
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create_ignore() {
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let _ = test::create_file_with_content(&dir, "ignore", "test");
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
        base_path: dir.clone(),
        ignore_pattern: Some("ignore"),
        ..Default::default()
    });
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join(FILE_CHECKSUM_SHA1);
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_SHA1
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create_match() {
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let _ = test::create_file_with_content(&dir, "unmatched", "test");
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
        base_path: dir.clone(),
        match_pattern: Some("file"),
        ..Default::default()
    });
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join(FILE_CHECKSUM_SHA1);
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_SHA1
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify() {
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let _ = test::create_file_with_content(&dir, FILE_CHECKSUM_SHA1, FILE_DATA_SHA1);
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
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
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, FILE_CHECKSUM_SHA1, FILE_DATA_SHA1);
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
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
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "datadata");
    let _ = test::create_file_with_content(&dir, FILE_CHECKSUM_SHA1, FILE_DATA_SHA1);
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
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
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "tada");
    let _ = test::create_file_with_content(&dir, FILE_CHECKSUM_SHA1, FILE_DATA_SHA1);
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
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
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let _ = test::create_file_with_content(&dir, FILE_CHECKSUM_SHA1, FILE_DATA_SHA1);
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
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
    let _ = test::create_file_with_content(&dir, "extra", "test");
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
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "tada");
    let _ = test::create_file_with_content(&dir, FILE_CHECKSUM_SHA1, FILE_DATA_SHA1);
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
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
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let ignore_file = test::create_file_with_content(&dir, "ignore", "test");
    let mut processor_create = HashFileProcessor::new_with_options(HashFileProcessOptions {
        base_path: dir.clone(),
        ..Default::default()
    });
    assert_eq!(processor_create.process(), HashFileProcessResult::Success);
    let mut processor_verify = HashFileProcessor::new_with_options(HashFileProcessOptions {
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
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let unmatched_file = test::create_file_with_content(&dir, "unmatched", "test");
    let mut processor_create = HashFileProcessor::new_with_options(HashFileProcessOptions {
        base_path: dir.clone(),
        ..Default::default()
    });
    assert_eq!(processor_create.process(), HashFileProcessResult::Success);
    let mut processor_verify = HashFileProcessor::new_with_options(HashFileProcessOptions {
        base_path: dir.clone(),
        match_pattern: Some("file"),
        ..Default::default()
    });
    std::fs::remove_file(unmatched_file).expect("Failed to remove unmatched file.");
    assert_eq!(processor_verify.process(), HashFileProcessResult::Success);
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}
