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
    initial_metadata: std::fs::Metadata,
}

impl<T: Digest + digest::FixedOutputReset> FileHash<T> {
    pub fn new_with_buffer_size(path: &Path, buffer_size: usize) -> io::Result<Self> {
        if buffer_size == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "hash buffer must be nonempty",
            ));
        }
        let file = File::open(path)?;
        let initial_metadata = file.metadata()?;
        Ok(Self {
            reader: BufReader::new(file),
            hasher: T::new(),
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
            initial_metadata,
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
impl<T: Digest + digest::FixedOutputReset> BlockHasher for FileHash<T> {
    fn validate_file(&self, path: &Path) -> io::Result<()> {
        let file = self.reader.get_ref();
        let after = file.metadata()?;
        let handle = same_file::Handle::from_file(file.try_clone()?)?;
        let current = same_file::Handle::from_path(path)?;
        if handle != current
            || self.initial_metadata.len() != after.len()
            || self.initial_metadata.modified()? != after.modified()?
        {
            return Err(io::Error::other("file changed during hashing"));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_hasher::BlockHasher;
    use sha1::Sha1;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn same_size_changes_during_hashing_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("file");
        std::fs::write(&path, "datadata").unwrap();
        let mut hasher = FileHash::<Sha1>::new_with_buffer_size(&path, 2).unwrap();
        let mut changed = false;
        hasher
            .compute_with_progress(CancellationToken::new(), &mut |_| {
                if !changed {
                    std::fs::write(&path, "tadatada").unwrap();
                    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
                    file.set_times(std::fs::FileTimes::new().set_modified(
                        std::time::UNIX_EPOCH + std::time::Duration::from_secs(1234),
                    ))
                    .unwrap();
                    changed = true;
                }
            })
            .unwrap();
        assert!(hasher.validate_file(&path).is_err());
    }

    #[test]
    fn replacing_a_file_during_hashing_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("file");
        let replacement = root.path().join("replacement");
        std::fs::write(&path, "datadata").unwrap();
        std::fs::write(&replacement, "datadata").unwrap();
        let mut hasher = FileHash::<Sha1>::new_with_buffer_size(&path, 2).unwrap();
        let mut changed = false;
        hasher
            .compute_with_progress(CancellationToken::new(), &mut |_| {
                if !changed {
                    std::fs::rename(&replacement, &path).unwrap();
                    changed = true;
                }
            })
            .unwrap();
        assert!(hasher.validate_file(&path).is_err());
    }
}
