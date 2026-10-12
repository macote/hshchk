use tokio_util::sync::CancellationToken;

pub trait Hasher {
    fn read(&mut self) -> std::io::Result<usize>;
    fn update(&mut self, byte_count: usize);
    fn digest(&mut self) -> String;
}

pub trait BlockHasher: Hasher {
    fn validate_file(&self, _path: &std::path::Path) -> std::io::Result<()> {
        Ok(())
    }
    /// Progress is emitted by the hashing thread, retaining its file identity.
    fn compute_with_progress(
        &mut self,
        token: CancellationToken,
        progress: &mut dyn FnMut(u64),
    ) -> std::io::Result<u64> {
        let mut bytes_processed = 0;
        while !token.is_cancelled() {
            let count = self.read()?;
            if count == 0 {
                break;
            }
            self.update(count);
            bytes_processed += count as u64;
            progress(bytes_processed);
        }
        Ok(bytes_processed)
    }
}
#[cfg(test)]
#[path = "tests/block_hasher.rs"]
mod tests;
