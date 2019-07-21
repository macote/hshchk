use crate::block_hasher::BlockHasher;
use crate::file_hash::FileHash;
use blake2::{Blake2b, Blake2s};
use md5::Md5;
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use std::error::Error;
use std::fs::File;
use std::path::Path;
use strum::IntoEnumIterator;
use strum_macros::{EnumIter, EnumString, IntoStaticStr};
pub mod hash_file_process;
mod block_hasher;
mod file_hash;
mod file_tree;
mod hash_file;

#[derive(Copy, Clone, PartialEq, Debug, EnumString, EnumIter, IntoStaticStr)]
pub enum HashType {
    MD5,
    SHA1,
    SHA256,
    SHA512,
    BLAKE2B,
    BLAKE2S,
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

fn get_file_hasher<'a>(hash_type: HashType, file_path: &'a str) -> Box<BlockHasher + 'a> {
    match hash_type {
        HashType::MD5 => Box::new(get_md5_file_hasher(file_path)),
        HashType::SHA1 => Box::new(get_sha1_file_hasher(file_path)),
        HashType::SHA256 => Box::new(get_sha256_file_hasher(file_path)),
        HashType::SHA512 => Box::new(get_sha512_file_hasher(file_path)),
        HashType::BLAKE2B => Box::new(get_blake2b_file_hasher(file_path)),
        HashType::BLAKE2S => Box::new(get_blake2s_file_hasher(file_path)),
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod tests {
    use super::*;
    use cancellation::CancellationToken;
    use std::fs;
    use std::fs::File;

    // block hasher

    // file hash
    #[test]
    fn empty_file() {
        let d = test::create_tmp_dir();
        let mut f = d.clone();
        f.push("file");
        File::create(&f).expect("failed to create");
        let mut fh = get_md5_file_hasher(f.to_str().unwrap());
        fh.compute(CancellationToken::none());
        let digest = fh.digest();
        assert_eq!(digest, "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn data_file() {
        let d = test::create_tmp_dir();
        let mut f = d.clone();
        f.push("file");
        fs::write(&f, "data").expect("failed to write");
        let mut fh = get_md5_file_hasher(f.to_str().unwrap());
        fh.compute(CancellationToken::none());
        let digest = fh.digest();
        assert_eq!(digest, "8d777f385d3dfec8815d20f7496026dc");
    }

    // hash file

    // hash file process

}
