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
        for (_, line) in reader.lines().enumerate() {
            let content = line.unwrap();
            match hash_file_format {
                HashFileFormat::HashCheck => {
                    let parts: Vec<&str> = content.split('|').collect();
                    if parts.len() != 3 {
                        continue;
                    }

                    let file_name = parts[0].replace(file_separator, os_separator);
                    if file_name.len() > MAX_PATH_SIZE {
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
                    self.add_entry(&file_name, Some(size), false, &parts[2].to_lowercase());
                }
                HashFileFormat::HashSum => match content.find(' ') {
                    Some(space_position) => {
                        let digest = &content[..space_position - 1];
                        let file_name = &content[space_position + 2..];
                        let binary = content.as_bytes()[space_position + 1] as char == '*';
                        if file_name.len() > MAX_PATH_SIZE {
                            panic!(
                                "File path length must be less than {} characters.",
                                MAX_PATH_SIZE + 1
                            );
                        }

                        self.add_entry(&file_name, Some(0), binary, &digest.to_lowercase());
                    }
                    _ => continue,
                }
            }
        }
    }

    pub fn save(&self, file_path: &Path, hash_file_format: HashFileFormat) {
        let file = create_file(&file_path);
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

    pub fn add_entry(&mut self, file_path: &str, size: Option<u64>, binary: bool, digest: &str) {
        self.files.insert(
            file_path.into(),
            HashFileEntry {
                file_path: file_path.to_owned(),
                size,
                binary,
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

// fn read_hash_check_entry(line: &str) -> HashFileEntry {
// }

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
