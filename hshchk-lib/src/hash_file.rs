use crate::{create_file, open_file};
use std::collections::HashMap;
use std::error::Error;
use std::io::{
    prelude::{BufRead, Write},
    BufReader, BufWriter,
};
use std::path::{Path, MAIN_SEPARATOR};

// `hshchk-lib` supports well-formed Unicode file names only.
// this is why paths are stored using `String` instead of `Path`.
// files having ill-formed Unicode file names are not processed.
pub struct HashFileEntry {
    pub file_path: String,
    pub size: u64,
    pub digest: String,
}

pub struct HashFile {
    files: HashMap<String, HashFileEntry>,
}

impl HashFile {
    pub fn new() -> Self {
        HashFile {
            files: HashMap::new(),
        }
    }

    pub fn load(&mut self, file_path: &Path) {
        let file = open_file(file_path);
        let reader = BufReader::new(file);
        let file_separator = replaceable_separator();
        let os_separator = &MAIN_SEPARATOR.to_string();
        for (_, line) in reader.lines().enumerate() {
            let content = line.unwrap();
            let split = content.split('|');
            let parts: Vec<&str> = split.collect();
            let file_name = parts[0].replace(file_separator, os_separator);
            self.add_entry(
                &file_name,
                parts[1].parse::<u64>().unwrap(),
                &parts[2].to_lowercase(),
            );
        }
    }

    pub fn save(&self, file_path: &Path) {
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
                panic!(
                    "couldn't write to {}: {}",
                    file_path.display(),
                    why.description()
                )
            };
        }
    }

    pub fn add_entry(&mut self, file_path: &str, size: u64, digest: &str) {
        self.files.insert(
            file_path.into(),
            HashFileEntry {
                file_path: file_path.to_owned(),
                size,
                digest: digest.into(),
            },
        );
    }

    pub fn remove_entry(&mut self, file_path: &str) {
        self.files.remove(file_path);
    }

    pub fn get_entry(&self, file_path: &str) -> Option<&HashFileEntry> {
        self.files.get(file_path)
    }

    pub fn get_file_paths(&self) -> Vec<String> {
        self.files.iter().map(|(key, _)| key.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

fn replaceable_separator() -> &'static str {
    match MAIN_SEPARATOR {
        '/' => "\\",
        _ => "/",
    }
}
