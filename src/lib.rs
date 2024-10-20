use crate::block_hasher::BlockHasher;
use crate::file_hash::FileHash;
use blake2::{Blake2b512, Blake2s256};
use md5::Md5;
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use std::fs::File;
use std::path::{Path, MAIN_SEPARATOR};
use strum::IntoEnumIterator;
use strum_macros::{EnumIter, EnumString, IntoStaticStr};
mod block_hasher;
mod file_hash;
mod file_tree;
mod hash_file;
pub mod hash_file_process;
mod output;
mod speed;
pub mod ui;

#[derive(Clone, Copy, Debug, EnumIter, EnumString, IntoStaticStr, PartialEq)]
#[strum(ascii_case_insensitive)]
pub enum HashType {
    MD5,
    SHA1,
    SHA256,
    SHA512,
    Blake2b,
    Blake2s,
    Blake3,
}

#[derive(Clone, Copy, Debug, EnumIter, EnumString, IntoStaticStr, PartialEq)]
pub enum HashFileFormat {
    HashCheck, // filepath|size|hash
    HashSum,   // hash<space><space/asterisk>filepath
}

pub fn replaceable_separator() -> &'static str {
    match MAIN_SEPARATOR {
        '/' => "\\",
        _ => "/",
    }
}

pub fn get_hash_types() -> Vec<&'static str> {
    HashType::iter().map(|ht| ht.into()).collect()
}

pub fn get_hash_type_from_str(type_str: &str) -> HashType {
    type_str.parse().unwrap()
}

pub fn get_hash_file_format_from_arg(hc_format_present: bool) -> HashFileFormat {
    if hc_format_present {
        HashFileFormat::HashCheck
    } else {
        HashFileFormat::HashSum
    }
}

fn open_file(file_path: &Path) -> File {
    match File::open(file_path) {
        Err(why) => panic!("Couldn't open {}: {}.", file_path.display(), why),
        Ok(file) => file,
    }
}

fn create_file(file_path: &Path) -> File {
    match File::create(file_path) {
        Err(why) => panic!("Couldn't create {}: {}.", file_path.display(), why),
        Ok(file) => file,
    }
}

fn get_md5_file_hasher(file_path: &Path) -> FileHash<Md5> {
    FileHash::new(file_path)
}

fn get_sha1_file_hasher(file_path: &Path) -> FileHash<Sha1> {
    FileHash::new(file_path)
}

fn get_sha256_file_hasher(file_path: &Path) -> FileHash<Sha256> {
    FileHash::new(file_path)
}

fn get_sha512_file_hasher(file_path: &Path) -> FileHash<Sha512> {
    FileHash::new(file_path)
}

fn get_blake2b_file_hasher(file_path: &Path) -> FileHash<Blake2b512> {
    FileHash::new(file_path)
}

fn get_blake2s_file_hasher(file_path: &Path) -> FileHash<Blake2s256> {
    FileHash::new(file_path)
}

fn get_blake3_file_hasher(file_path: &Path) -> FileHash<blake3::Hasher> {
    FileHash::new(file_path)
}

fn get_file_hasher(hash_type: HashType, file_path: &Path) -> Box<dyn BlockHasher> {
    match hash_type {
        HashType::MD5 => Box::new(get_md5_file_hasher(file_path)),
        HashType::SHA1 => Box::new(get_sha1_file_hasher(file_path)),
        HashType::SHA256 => Box::new(get_sha256_file_hasher(file_path)),
        HashType::SHA512 => Box::new(get_sha512_file_hasher(file_path)),
        HashType::Blake2b => Box::new(get_blake2b_file_hasher(file_path)),
        HashType::Blake2s => Box::new(get_blake2s_file_hasher(file_path)),
        HashType::Blake3 => Box::new(get_blake3_file_hasher(file_path)),
    }
}

#[cfg(test)]
extern crate test_shared;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash_file::HashFile;
    use crossbeam::channel::unbounded;
    use hash_file::HashFileEntry;
    use std::fs;
    use tokio_util::sync::CancellationToken;

    // block hasher

    // ...

    // file hash
    #[test]
    fn file_hash_bytes_processed_event_sender_undefined() {
        let file = test_shared::create_tmp_file("");
        let file_hash: FileHash<Md5> = FileHash::new(&file);
        assert_eq!(file_hash.is_bytes_processed_event_sender_defined(), false);
        drop(file_hash); // force release of file handle (Windows)
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    #[test]
    fn file_hash_bytes_processed_event_sender_defined() {
        let file = test_shared::create_tmp_file("");
        let mut file_hash: FileHash<Md5> = FileHash::new(&file);
        let (sender, _) = unbounded();
        file_hash.set_bytes_processed_event_sender(sender);
        assert_eq!(file_hash.is_bytes_processed_event_sender_defined(), true);
        drop(file_hash); // force release of file handle (Windows)
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    #[test]
    fn file_hash_empty_file() {
        let file = test_shared::create_tmp_file("");
        let mut file_hash = get_md5_file_hasher(&file);
        let cancellation_token = CancellationToken::new();
        file_hash.compute(cancellation_token.clone());
        let digest = file_hash.digest();
        assert_eq!(digest, "d41d8cd98f00b204e9800998ecf8427e");
        drop(file_hash); // force release of file handle (Windows)
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    #[test]
    fn file_hash_data_file() {
        let file = test_shared::create_tmp_file("data");
        let mut file_hash = get_md5_file_hasher(&file);
        let cancellation_token = CancellationToken::new();
        file_hash.compute(cancellation_token.clone());
        let digest = file_hash.digest();
        assert_eq!(digest, "8d777f385d3dfec8815d20f7496026dc");
        drop(file_hash); // force release of file handle (Windows)
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    #[test]
    fn file_hash_data_two_blocks() {
        let file = test_shared::create_tmp_file("datadata");
        let mut file_hash: FileHash<Md5> = FileHash::new_with_buffer_size(&file, 2);
        let (sender, receiver) = unbounded();
        file_hash.set_bytes_processed_event_sender_with_bytes_processed_notification_block_size(
            sender, 4,
        );
        let cancellation_token = CancellationToken::new();
        file_hash.compute(cancellation_token);
        let digest = file_hash.digest();
        assert_eq!(digest, "511ae0b1c13f95e5f08f1a0dd3da3d93");
        assert_eq!(4, receiver.recv().unwrap().bytes_processed);
        assert_eq!(8, receiver.recv().unwrap().bytes_processed);
        assert!(receiver.try_recv().is_err());
        drop(file_hash); // force release of file handle (Windows)
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    // hash file

    #[test]
    fn hash_file_load_single() {
        let file = test_shared::create_tmp_file("filename|0|hash");
        let mut hash_file = HashFile::new();
        hash_file.load(&file);
        assert_eq!(1, hash_file.get_file_paths().len());
        let entry = hash_file.get_entry("filename").unwrap();
        assert_eq!(0, entry.size.unwrap());
        assert_eq!("hash", entry.digest);
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    #[test]
    fn hash_file_load_multiple() {
        let file = test_shared::create_tmp_file("filename1|1|hash1\r\nfilename2|2|hash2");
        let mut hash_file = HashFile::new();
        hash_file.load(&file);
        assert_eq!(2, hash_file.get_file_paths().len());
        let entry = hash_file.get_entry("filename1").unwrap();
        assert_eq!(1, entry.size.unwrap());
        assert_eq!("hash1", entry.digest);
        let entry = hash_file.get_entry("filename2").unwrap();
        assert_eq!(2, entry.size.unwrap());
        assert_eq!("hash2", entry.digest);
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    #[test]
    fn hash_file_load_failed_size() {
        let file = test_shared::create_tmp_file("filename|size|hash");
        let file_clone = file.clone();
        let mut hash_file = HashFile::new();
        assert_eq!(
            std::panic::catch_unwind(move || {
                hash_file.load(&file_clone);
            })
            .err()
            .and_then(|a| a
                .downcast_ref::<String>()
                .map(|s| { &s[..25] == "Failed to parse file size" })),
            Some(true)
        );
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    #[test]
    fn hash_file_load_failed_filename() {
        let file = test_shared::create_tmp_file(&("a".repeat(4096) + "|0|hash"));
        let file_clone = file.clone();
        let mut hash_file = HashFile::new();
        assert_eq!(
            std::panic::catch_unwind(move || {
                hash_file.load(&file_clone);
            })
            .err()
            .and_then(|a| a
                .downcast_ref::<String>()
                .map(|s| { s == "File path length must be less than 4096 characters." })),
            Some(true)
        );
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    #[test]
    fn hash_file_load_failed_hash() {
        let file = test_shared::create_tmp_file(&(String::from("filename|0|") + &"a".repeat(1025)));
        let file_clone = file.clone();
        let mut hash_file = HashFile::new();
        assert_eq!(
            std::panic::catch_unwind(move || {
                hash_file.load(&file_clone);
            })
            .err()
            .and_then(|a| a
                .downcast_ref::<String>()
                .map(|s| { s == "Hash length must be less than 1025 characters." })),
            Some(true)
        );
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    #[test]
    fn hash_file_is_empty() {
        let hash_file = HashFile::new();
        assert!(hash_file.is_empty());
    }

    #[test]
    fn hash_file_is_not_empty() {
        let mut hash_file = HashFile::new();
        hash_file.add_entry(HashFileEntry {
            file_path: "filename".into(),
            size: None,
            digest: "hash".into(),
        });
        assert!(!hash_file.is_empty());
    }

    #[test]
    fn hash_file_get_file_paths() {
        let mut hash_file = HashFile::new();
        hash_file.add_entry(HashFileEntry {
            file_path: "filename1".into(),
            size: None,
            digest: "hash1".into(),
        });
        hash_file.add_entry(HashFileEntry {
            file_path: "filename2".into(),
            size: None,
            digest: "hash2".into(),
        });
        let mut filenames = hash_file.get_file_paths();
        filenames.sort();
        assert_eq!("filename1filename2", filenames.join(""));
    }

    #[test]
    fn hash_file_remove_entry() {
        let mut hash_file = HashFile::new();
        hash_file.add_entry(HashFileEntry {
            file_path: "filename".into(),
            size: None,
            digest: "hash".into(),
        });
        hash_file.remove_entry("filename");
        assert!(hash_file.is_empty());
    }
}
