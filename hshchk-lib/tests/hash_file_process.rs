use std::fs;
use hshchk_lib::HashType;
use hshchk_lib::hash_file_process::{
    HashFileProcessResult, HashFileProcessor,
};

#[path="../src/test/mod.rs"]
mod test;

#[test]
fn create_in_empty_dir() {
    let d = test::create_tmp_dir();
    let mut p = HashFileProcessor::new(d.to_str().unwrap(), HashType::SHA1, false);
    let result = p.process();
    fs::remove_dir_all(d).expect("failed to remove dir");
    assert_eq!(result, HashFileProcessResult::Error);
}

#[test]
fn create_in_dir() {
    let d = test::create_tmp_dir();
    let mut f = d.clone();
    f.push("file");
    fs::write(f, "data").expect("failed to write");
    let mut p = HashFileProcessor::new(d.to_str().unwrap(), HashType::SHA1, false);
    let result = p.process();
    fs::remove_dir_all(d).expect("failed to remove dir");
    assert_eq!(result, HashFileProcessResult::Success);
}

#[test]
fn create_in_dir_with_ignore() {
    let d = test::create_tmp_dir();
    let mut f = d.clone();
    f.push("file");
    fs::write(f, "data").expect("failed to write");
    let mut p = HashFileProcessor::new(d.to_str().unwrap(), HashType::SHA1, false);
    //let mut
    let result = p.process();
    fs::remove_dir_all(d).expect("failed to remove dir");
    assert_eq!(result, HashFileProcessResult::Success);
}