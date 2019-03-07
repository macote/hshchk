use std::error::Error;
use std::fs::File;
use std::path::Path;

use blake2::{Blake2b, Blake2s};
use md5::Md5;
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use strum::IntoEnumIterator;
use strum_macros::{EnumIter, EnumString, IntoStaticStr};

use crate::block_hasher::BlockHasher;
use crate::file_hash::FileHash;

mod block_hasher;
mod file_hash;
mod file_tree;
mod hash_file;
pub mod hash_file_process;

#[derive(Copy, Clone, PartialEq, Debug, EnumString, EnumIter, IntoStaticStr)]
pub enum HashType {
    MD5,
    SHA1,
    SHA256,
    SHA512,
    BLAKE2B,
    BLAKE2S,
}

fn open_file(file_path: &str) -> File {
    let path = Path::new(file_path);
    let path_displayable = path.display();
    match File::open(&path) {
        Err(why) => panic!("couldn't open {}: {}", path_displayable, why.description()),
        Ok(file) => file,
    }
}

fn create_file(file_path: &str) -> File {
    let path = Path::new(file_path);
    let path_displayable = path.display();
    match File::create(&path) {
        Err(why) => panic!(
            "couldn't create {}: {}",
            path_displayable,
            why.description()
        ),
        Ok(file) => file,
    }
}

fn get_md5_file_hasher(file_path: &str) -> FileHash<Md5> {
    FileHash::new(file_path)
}

fn get_sha1_file_hasher(file_path: &str) -> FileHash<Sha1> {
    FileHash::new(file_path)
}

fn get_sha256_file_hasher(file_path: &str) -> FileHash<Sha256> {
    FileHash::new(file_path)
}

fn get_sha512_file_hasher(file_path: &str) -> FileHash<Sha512> {
    FileHash::new(file_path)
}

fn get_blake2b_file_hasher(file_path: &str) -> FileHash<Blake2b> {
    FileHash::new(file_path)
}

fn get_blake2s_file_hasher(file_path: &str) -> FileHash<Blake2s> {
    FileHash::new(file_path)
}

pub fn get_file_hasher<'a>(hash_type: HashType, file_path: &'a str) -> Box<BlockHasher + 'a> {
    match hash_type {
        HashType::MD5 => Box::new(get_md5_file_hasher(file_path)),
        HashType::SHA1 => Box::new(get_sha1_file_hasher(file_path)),
        HashType::SHA256 => Box::new(get_sha256_file_hasher(file_path)),
        HashType::SHA512 => Box::new(get_sha512_file_hasher(file_path)),
        HashType::BLAKE2B => Box::new(get_blake2b_file_hasher(file_path)),
        HashType::BLAKE2S => Box::new(get_blake2s_file_hasher(file_path)),
    }
}

pub fn get_hash_types() -> Vec<&'static str> {
    let mut types: Vec<&'static str> = Vec::new();
    for hash_type in HashType::iter() {
        types.push(hash_type.into());
    }

    types
}

pub fn get_hash_type_from_str(type_str: &str) -> HashType {
    type_str.parse().unwrap()
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    // block hasher

    // file hash

    // hash file

    // hash file process

}
