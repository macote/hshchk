use hshchk_lib::hash_file_process::*;
use hshchk_lib::HashType;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::channel;

#[path = "../src/test/mod.rs"]
mod test;

static FILE_DATA_CHECKSUM: &str = "file|4|a17c9aaa61e80a1bf71d0d850af4e5baa9800bbd\n";

#[test]
fn hash_file_process_create_empty() {
    let dir = test::create_tmp_dir();
    let mut processor = HashFileProcessor::new(&dir, HashType::SHA1, false);
    assert_eq!(processor.process(), HashFileProcessResult::Error);
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create() {
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let mut processor = HashFileProcessor::new(&dir, HashType::SHA1, false);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    let checksum_file = dir.join("hshchk.sha1");
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_CHECKSUM
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_create_force() {
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let checksum_file = test::create_file_with_content(&dir, "hshchk.sha1", "test");
    let mut processor = HashFileProcessor::new(&dir, HashType::SHA1, true);
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_CHECKSUM
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
    let checksum_file = dir.join("hshchk.sha1");
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_CHECKSUM
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
    let checksum_file = dir.join("hshchk.sha1");
    assert_eq!(
        test::get_file_string_content(&checksum_file),
        FILE_DATA_CHECKSUM
    );
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify() {
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let _ = test::create_file_with_content(&dir, "hshchk.sha1", FILE_DATA_CHECKSUM);
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
        base_path: dir.clone(),
        ..Default::default()
    });
    let (sender, receiver) = channel();
    let sender_error = sender.clone();
    processor.set_error_event_handler(Box::new(move |_| {
        sender_error.send(1).unwrap();
    }));
    let sender_warning = sender.clone();
    processor.set_warning_event_handler(Box::new(move |_| {
        sender_warning.send(1).unwrap();
    }));
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert!(receiver.try_recv().is_err());
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify_extra_files() {
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "data");
    let _ = test::create_file_with_content(&dir, "hshchk.sha1", FILE_DATA_CHECKSUM);
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
        base_path: dir.clone(),
        report_extra_files: Some(true),
        ..Default::default()
    });
    let (error_sender, error_receiver) = channel();
    let (warning_sender, warning_receiver) = channel();
    let sender_error = error_sender.clone();
    processor.set_error_event_handler(Box::new(move |_| {
        sender_error.send("error").unwrap();
    }));
    let sender_warning = warning_sender.clone();
    processor.set_warning_event_handler(Box::new(move |file_process_entry| {
        sender_warning.send(file_process_entry.clone()).unwrap();
    }));
    let _ = test::create_file_with_content(&dir, "extra", "test");
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert_eq!(FileProcessEntry { file_path: PathBuf::from("extra"), state: FileProcessState::Extra}, warning_receiver.recv().unwrap());
    assert!(error_receiver.try_recv().is_err());
    assert!(warning_receiver.try_recv().is_err());
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}

#[test]
fn hash_file_process_verify_file_size_only() {
    let dir = test::create_tmp_dir();
    let _ = test::create_file_with_content(&dir, "file", "tada");
    let _ = test::create_file_with_content(&dir, "hshchk.sha1", FILE_DATA_CHECKSUM);
    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
        base_path: dir.clone(),
        check_file_size_only: Some(true),
        ..Default::default()
    });
    let (error_sender, error_receiver) = channel();
    let (warning_sender, warning_receiver) = channel();
    let sender_error = error_sender.clone();
    processor.set_error_event_handler(Box::new(move |_| {
        sender_error.send("error").unwrap();
    }));
    let sender_warning = warning_sender.clone();
    processor.set_warning_event_handler(Box::new(move |file_process_entry| {
        sender_warning.send(file_process_entry.clone()).unwrap();
    }));
    assert_eq!(processor.process(), HashFileProcessResult::Success);
    assert!(error_receiver.try_recv().is_err());
    assert!(warning_receiver.try_recv().is_err());
    fs::remove_dir_all(dir).expect("Failed to remove test directory.");
}
