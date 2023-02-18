use crate::{create_file, open_file, replaceable_separator, HashFileFormat};
use std::collections::HashMap;
use std::io::{
    prelude::{BufRead, Write},
    BufReader, BufWriter,
};
use std::path::{Path, MAIN_SEPARATOR};

const MAX_PATH_SIZE: usize = 4_096 - 1;
const MAX_HASH_SIZE: usize = 1024;

// `hshchk-lib` supports well-formed Unicode file names only.
// This is why paths are stored using `String` instead of `Path`.
// Files having ill-formed Unicode file names are not processed.
pub struct HashFileEntry {
    pub file_path: String,
    pub size: Option<u64>,
    pub binary: bool,
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
        let hash_file_format = get_hash_file_format(file_path);
        let file = open_file(file_path);
        let reader = BufReader::new(&file);
        let file_separator = replaceable_separator();
        let os_separator = &MAIN_SEPARATOR.to_string();
        let entry_parse = match hash_file_format {
            HashFileFormat::HashCheck => parse_hash_check_entry,
            _ => parse_hash_sum_entry,
        };

        for (_, line) in reader.lines().enumerate() {
            let content = line.unwrap().replace(file_separator, os_separator);
            if let Some(file_entry) = entry_parse(&content) {
                self.add_entry(file_entry);
            }
        }
    }

    pub fn save(&self, file_path: &Path, hash_file_format: HashFileFormat) {
        let file = create_file(file_path);
        let mut writer = BufWriter::new(&file);
        let entry_format = match hash_file_format {
            HashFileFormat::HashCheck => format_hash_check_entry,
            HashFileFormat::HashSum => format_hash_sum_entry,
        };
        for file_entry in self.files.values() {
            let line = &entry_format(file_entry);
            if let Err(why) = writer.write(line.as_bytes()) {
                panic!("Couldn't write to {}: {}.", file_path.display(), why)
            };
        }
    }

    pub fn add_entry(&mut self, file_entry: HashFileEntry) {
        self.files.insert(file_entry.file_path.clone(), file_entry);
    }

    pub fn remove_entry(&mut self, file_path: &str) {
        self.files.remove(file_path);
    }

    pub fn get_entry(&self, file_path: &str) -> Option<&HashFileEntry> {
        self.files.get(file_path)
    }

    pub fn get_file_paths(&self) -> Vec<String> {
        self.files.keys().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

fn get_hash_file_format(file_path: &Path) -> HashFileFormat {
    let file = open_file(file_path);
    let mut reader = BufReader::new(&file);
    let mut first_line = String::new();
    reader.read_line(&mut first_line).unwrap();
    match first_line.find('|') {
        Some(_) => HashFileFormat::HashCheck,
        _ => HashFileFormat::HashSum,
    }
}

fn parse_hash_check_entry(line: &str) -> Option<HashFileEntry> {
    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() == 3 {
        if parts[0].len() > MAX_PATH_SIZE {
            panic!(
                "File path length must be less than {} characters.",
                MAX_PATH_SIZE + 1
            );
        }

        if parts[2].len() > MAX_HASH_SIZE {
            panic!(
                "Hash length must be less than {} characters.",
                MAX_HASH_SIZE + 1
            );
        }

        let size = parts[1].parse::<u64>().expect("Failed to parse file size");
        Some(HashFileEntry {
            file_path: parts[0].to_string(),
            size: Some(size),
            binary: true,
            digest: parts[2].to_lowercase(),
        })
    } else {
        None
    }
}

fn parse_hash_sum_entry(line: &str) -> Option<HashFileEntry> {
    match line.find(' ') {
        Some(space_position) => {
            let digest = &line[..space_position];
            let file_path = &line[space_position + 2..];
            let binary = line.as_bytes()[space_position + 1] as char == '*';
            if file_path.len() > MAX_PATH_SIZE {
                panic!(
                    "File path length must be less than {} characters.",
                    MAX_PATH_SIZE + 1
                );
            }

            Some(HashFileEntry {
                file_path: file_path.to_string(),
                size: None,
                binary,
                digest: digest.to_lowercase(),
            })
        }
        _ => None,
    }
}

fn format_hash_check_entry(entry: &HashFileEntry) -> String {
    format!(
        "{}|{}|{}\n",
        &entry.file_path,
        &entry.size.unwrap().to_string(),
        &entry.digest
    )
}

fn format_hash_sum_entry(entry: &HashFileEntry) -> String {
    format!("{} *{}\n", &entry.digest, &entry.file_path)
}
