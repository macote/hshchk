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
mod blake3_digest;
mod block_hasher;
mod file_hash;
mod file_tree;
mod hash_file;
pub mod hash_file_process;
mod output;
pub mod report;
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

fn get_blake3_file_hasher(file_path: &Path) -> io::Result<FileHash<blake3_digest::Blake3Digest>> {
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
#[path = "tests/lib.rs"]
mod tests;
