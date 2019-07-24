#![allow(dead_code)]

use nanoid;
use std::env;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

pub fn create_tmp_dir() -> PathBuf {
    let mut dir = env::temp_dir();
    dir.push(nanoid::simple());
    fs::create_dir(dir.clone()).expect("failed to create dir");
    dir
}

pub fn create_tmp_file(data: &str) -> PathBuf {
    let dir = create_tmp_dir();
    create_named_file(&dir, &nanoid::simple(), data)
}

pub fn create_named_file(dir: &PathBuf, name: &str, data: &str) -> PathBuf {
    let mut file = dir.clone();
    file.push(name);
    fs::write(&file, data).expect("failed to write to file");
    file
}

pub fn get_file_string_content(path: &Path) -> String {
    let mut file = File::open(path).expect("failed to open file");
    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("failed to read file content");
    content
}
