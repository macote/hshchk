use crate::block_hasher::{BlockHasher, Hasher};
use digest::Digest;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

const DEFAULT_BUFFER_SIZE: usize = 1_048_576;

pub struct FileHash<T: Digest + digest::FixedOutputReset> {
    reader: BufReader<File>,
    hasher: T,
    buffer: Vec<u8>,
    buffer_size: usize,
}

impl<T: Digest + digest::FixedOutputReset> FileHash<T> {
    pub fn new_with_buffer_size(path: &Path, buffer_size: usize) -> io::Result<Self> {
        if buffer_size == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "hash buffer must be nonempty",
            ));
        }
        Ok(Self {
            reader: BufReader::new(File::open(path)?),
            hasher: T::new(),
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
        })
    }
    pub fn new(path: &Path) -> io::Result<Self> {
        Self::new_with_buffer_size(path, DEFAULT_BUFFER_SIZE)
    }
}

impl<T: Digest + digest::FixedOutputReset> Hasher for FileHash<T> {
    fn read(&mut self) -> io::Result<usize> {
        self.buffer.clear();
        (&mut self.reader)
            .take(self.buffer_size as u64)
            .read_to_end(&mut self.buffer)
    }
    fn update(&mut self, count: usize) {
        Digest::update(&mut self.hasher, &self.buffer[..count]);
    }
    fn digest(&mut self) -> String {
        hex::encode(self.hasher.finalize_reset())
    }
}
impl<T: Digest + digest::FixedOutputReset> BlockHasher for FileHash<T> {}
