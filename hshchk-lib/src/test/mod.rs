use nanoid;
use std::env;
use std::fs;
use std::path::PathBuf;

pub fn create_tmp_dir() -> PathBuf {
    let mut tmp = env::temp_dir();
    tmp.push(nanoid::simple());
    fs::create_dir(tmp.clone()).expect("failed to create dir");
    tmp
}
