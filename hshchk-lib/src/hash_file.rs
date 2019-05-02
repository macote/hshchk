use std::collections::HashMap;
use std::error::Error;
use std::io::{
    prelude::{BufRead, Write},
    BufReader, BufWriter,
};
use std::path::MAIN_SEPARATOR;

use crate::{create_file, open_file};

pub struct FileEntry {
    pub file_path: String,
    pub size: u64,
    pub digest: String,
}

pub struct HashFile {
    files: HashMap<String, FileEntry>,
}

impl HashFile {
    pub fn new() -> Self {
        HashFile {
            files: HashMap::new(),
        }
    }

    pub fn load(&mut self, file_path: &str) {
        let file = open_file(file_path);
        let reader = BufReader::new(file);
        for (_, line) in reader.lines().enumerate() {
            let content = line.unwrap();
            let split = content.split('|');
            let parts: Vec<&str> = split.collect();
            let file_name = parts[0].replace(&replaceable_separator(), &MAIN_SEPARATOR.to_string());
            self.add_entry(
                &file_name,
                parts[1].parse::<u64>().unwrap(),
                &parts[2].to_lowercase(),
            );
        }
    }

    pub fn save(&self, file_path: &str) {
        let file = create_file(&file_path);
        let mut writer = BufWriter::new(&file);
        for file_entry in self.files.values() {
            let mut line = String::new();
            line.push_str(&format!(
                "{}|{}|{}\n",
                &file_entry.file_path,
                &file_entry.size.to_string(),
                &file_entry.digest
            ));
            if let Err(why) = writer.write(line.as_bytes()) {
                panic!("couldn't write to {}: {}", file_path, why.description())
            };
        }
    }

    pub fn add_entry(&mut self, file_path: &str, size: u64, digest: &str) {
        self.files.insert(
            file_path.into(),
            FileEntry {
                file_path: String::from(file_path),
                size,
                digest: digest.into(),
            },
        );
    }

    pub fn remove_entry(&mut self, file_path: &str) {
        self.files.remove(file_path);
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

fn replaceable_separator() -> String {
    match MAIN_SEPARATOR {
        '/' => String::from("\\"),
        _ => String::from("/"),
    }
}
