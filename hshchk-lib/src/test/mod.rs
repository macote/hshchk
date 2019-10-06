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
    fs::create_dir(dir.clone()).expect("Failed to create temp directory.");
    dir
}

pub fn create_tmp_file(data: &str) -> PathBuf {
    let dir = create_tmp_dir();
    create_file_with_content(&dir, &nanoid::simple(), data)
}

pub fn create_file_with_content(dir: &PathBuf, name: &str, content: &str) -> PathBuf {
    let mut file = dir.clone();
    file.push(name);
    fs::write(&file, content)
        .unwrap_or_else(|_| panic!("Failed to write to file {}.", file.display()));
    file
}

pub fn get_file_string_content(path: &Path) -> String {
    let mut file =
        File::open(path).unwrap_or_else(|_| panic!("Failed to open file {}.", path.display()));
    let mut content = String::new();
    file.read_to_string(&mut content)
        .unwrap_or_else(|_| panic!("Failed to read the file content of {}.", path.display()));
    content
}
