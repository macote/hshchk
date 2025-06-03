use crate::block_hasher::BlockHasher;
use crate::file_hash::FileHash;
use blake2::{Blake2b512, Blake2s256};
use md5::Md5;
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use std::io;
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

fn get_md5_file_hasher(file_path: &Path) -> io::Result<FileHash<Md5>> {
    FileHash::new(file_path)
}

fn get_sha1_file_hasher(file_path: &Path) -> io::Result<FileHash<Sha1>> {
    FileHash::new(file_path)
}

fn get_sha256_file_hasher(file_path: &Path) -> io::Result<FileHash<Sha256>> {
    FileHash::new(file_path)
}

fn get_sha512_file_hasher(file_path: &Path) -> io::Result<FileHash<Sha512>> {
    FileHash::new(file_path)
}

fn get_blake2b_file_hasher(file_path: &Path) -> io::Result<FileHash<Blake2b512>> {
    FileHash::new(file_path)
}

fn get_blake2s_file_hasher(file_path: &Path) -> io::Result<FileHash<Blake2s256>> {
    FileHash::new(file_path)
}

fn get_blake3_file_hasher(file_path: &Path) -> io::Result<FileHash<blake3::Hasher>> {
    FileHash::new(file_path)
}

fn get_file_hasher(hash_type: HashType, file_path: &Path) -> io::Result<Box<dyn BlockHasher>> {
    match hash_type {
        HashType::MD5 => Ok(Box::new(get_md5_file_hasher(file_path)?)),
        HashType::SHA1 => Ok(Box::new(get_sha1_file_hasher(file_path)?)),
        HashType::SHA256 => Ok(Box::new(get_sha256_file_hasher(file_path)?)),
        HashType::SHA512 => Ok(Box::new(get_sha512_file_hasher(file_path)?)),
        HashType::Blake2b => Ok(Box::new(get_blake2b_file_hasher(file_path)?)),
        HashType::Blake2s => Ok(Box::new(get_blake2s_file_hasher(file_path)?)),
        HashType::Blake3 => Ok(Box::new(get_blake3_file_hasher(file_path)?)),
    }
}

#[cfg(test)]
extern crate test_shared;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_hasher::Hasher;
    use crate::hash_file::HashFile;
    use hash_file::HashFileEntry;
    use std::fs;
    use tokio_util::sync::CancellationToken;

    // block hasher

    // ...

    // file hash
    #[test]
    fn file_hash_empty_file() {
        let file = test_shared::create_tmp_file("");
        let mut file_hash = get_md5_file_hasher(&file).unwrap();
        let cancellation_token = CancellationToken::new();
        file_hash.compute(cancellation_token.clone()).unwrap();
        let digest = file_hash.digest();
        assert_eq!(digest, "d41d8cd98f00b204e9800998ecf8427e");
        drop(file_hash); // force release of file handle (Windows)
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    #[test]
    fn file_hash_data_file() {
        let file = test_shared::create_tmp_file("data");
        let mut file_hash = get_md5_file_hasher(&file).unwrap();
        let cancellation_token = CancellationToken::new();
        file_hash.compute(cancellation_token.clone()).unwrap();
        let digest = file_hash.digest();
        assert_eq!(digest, "8d777f385d3dfec8815d20f7496026dc");
        drop(file_hash); // force release of file handle (Windows)
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    #[test]
    fn file_hash_data_two_blocks() {
        let file = test_shared::create_tmp_file("datadata");
        let mut file_hash: FileHash<Md5> = FileHash::new_with_buffer_size(&file, 2).unwrap();
        let mut progress = Vec::new();
        let cancellation_token = CancellationToken::new();
        file_hash
            .compute_with_progress(cancellation_token, &mut |bytes| progress.push(bytes))
            .unwrap();
        let digest = file_hash.digest();
        assert_eq!(digest, "511ae0b1c13f95e5f08f1a0dd3da3d93");
        assert_eq!(progress, vec![2, 4, 6, 8]);
        drop(file_hash); // force release of file handle (Windows)
        fs::remove_dir_all(file.parent().unwrap()).expect("Failed to remove test directory.");
    }

    // hash file

    #[test]
    fn hash_file_load_single() {
        let file = test_shared::create_tmp_file("filename|0|hash");
        let mut hash_file = HashFile::new();
        hash_file.load(&file).unwrap();
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
        hash_file.load(&file).unwrap();
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
    fn hash_file_rejects_malformed_entries() {
        for content in [
            "filename|size|hash".to_string(),
            "a".repeat(4096) + "|0|hash",
            "filename|0|".to_string() + &"a".repeat(1025),
            "digest ".to_string(),
            "../outside|0|hash".to_string(),
        ] {
            let file = test_shared::create_tmp_file(&content);
            let mut hash_file = HashFile::new();
            assert!(hash_file.load(&file).is_err());
            assert!(hash_file.is_empty());
            fs::remove_dir_all(file.parent().unwrap()).unwrap();
        }
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
