use std::error::Error;
use std::fs::File;
use std::path::Path;

use digest::Digest;

use sha1::{Sha1};
use sha2::{Sha256, Sha512};
use blake2::{Blake2b, Blake2s};

use crate::file_hash::{FileHash};

pub mod block_hasher;
pub mod file_hash;
pub mod file_tree;
pub mod hash_file;
pub mod hash_file_process;

pub enum HashType {
	SHA1,
	SHA256,
	SHA512,
    BLAKE2B,
    BLAKE2S,
	Undefined
}

fn open_file(file_path: &str) -> File {
    let path = Path::new(file_path);
    let path_displayable = path.display();
    match File::open(&path) {
        Err(why) => panic!("couldn't open {}: {}", 
            path_displayable,
            why.description()),
        Ok(file) => file,
    }
}

fn create_file(file_path: &str) -> File {
    let path = Path::new(file_path);
    let path_displayable = path.display();
    match File::create(&path) {
        Err(why) => panic!("couldn't create {}: {}", 
            path_displayable,
            why.description()),
        Ok(file) => file,
    }
}

pub fn get_sha1_file_hasher(file_path: &str) -> FileHash<Sha1> {
    FileHash::new(file_path)
}

pub fn get_sha256_file_hasher(file_path: &str) -> FileHash<Sha256> {
    FileHash::new(file_path)
}

pub fn get_sha512_file_hasher(file_path: &str) -> FileHash<Sha512> {
    FileHash::new(file_path)
}

pub fn get_blake2s_file_hasher(file_path: &str) -> FileHash<Blake2s> {
    FileHash::new(file_path)
}

pub fn get_blake2b_file_hasher(file_path: &str) -> FileHash<Blake2b> {
    FileHash::new(file_path)
}

pub fn get_file_hasher<T: Digest>(file_path: &str) -> FileHash<T> {
    FileHash::new(file_path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    // block hasher
    #[test]
    fn block_hasher_compute() {

    }

    // file hash

    // hash file

    // hash file process

}
