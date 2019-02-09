use std::collections::HashMap;
use std::error::Error;
use std::io::{BufReader, BufWriter, prelude::*};

use crate::{open_file, create_file};

pub struct FileEntry {
    pub file_path: String,
    pub size: u64,
    pub digest: String,
}

pub struct HashFile {
    files: HashMap<String, FileEntry>,
}

impl HashFile {
    pub fn new () -> Self {
        HashFile {
            files: HashMap::new(),
        }
    }

    pub fn load(&mut self, file_path: &str) {
        let file = open_file(file_path);
        let reader = BufReader::new(file);
        for (_, line) in reader.lines().enumerate() {
            let content = line.unwrap();
            let split = content.split("|");
            let parts: Vec<&str> = split.collect();
            self.add_entry(
                parts[0],
                parts[1].parse::<u64>().unwrap(),
                parts[2]);
        }
    }

    pub fn save(&self, file_path: &str) {
        let file = create_file(&file_path);
        let mut writer = BufWriter::new(&file);
        for file_entry in self.files.values() {
            let mut line = String::new();
            line.push_str(&file_entry.file_path);
            line.push_str("|");
            line.push_str(&file_entry.size.to_string());
            line.push_str("|");
            line.push_str(&file_entry.digest);
            line.push_str("\n");
            match writer.write(line.as_bytes()) {
                Err(why) => panic!("couldn't write to {}: {}",
                    file_path,
                    why.description()),
                Ok(_) => ()
            };
        }
    }

    pub fn add_entry(&mut self, file_path: &str, size: u64, digest: &str) {
        self.files.insert(
            file_path.into(),
            FileEntry { file_path: String::from(file_path), size, digest: digest.into() });
    }

    pub fn remove_entry(&mut self, file_path: &str) {
        self.files.remove(file_path);
    }

    pub fn contains_entry(&self, file_path: &str) -> bool {
        self.files.contains_key(file_path)
    }

    pub fn get_entry(&self, file_path: &str) -> Option<&FileEntry> {
        self.files.get(file_path)
    }

    pub fn get_file_paths(&self) -> Vec<String> {
        self.files.iter().map(|(key, _)| key.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}