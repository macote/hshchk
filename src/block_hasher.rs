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
    #[cfg(test)]
    fn compute(&mut self, token: CancellationToken) -> std::io::Result<u64> {
        self.compute_with_progress(token, &mut |_| {})
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
mod tests {
    use super::*;
    struct Reader {
        reads: usize,
        fail_second_read: bool,
    }
    impl Hasher for Reader {
        fn read(&mut self) -> std::io::Result<usize> {
            self.reads += 1;
            if self.fail_second_read && self.reads == 2 {
                return Err(std::io::Error::other("read failed"));
            }
            Ok(if self.reads <= 2 { 2 } else { 0 })
        }
        fn update(&mut self, _: usize) {}
        fn digest(&mut self) -> String {
            String::new()
        }
    }
    impl BlockHasher for Reader {}

    #[test]
    fn cancellation_stops_before_reading_the_next_block() {
        let token = CancellationToken::new();
        let mut reader = Reader {
            reads: 0,
            fail_second_read: false,
        };
        let bytes = reader
            .compute_with_progress(token.clone(), &mut |_| token.cancel())
            .unwrap();
        assert_eq!(bytes, 2);
        assert_eq!(reader.reads, 1);
    }
    #[test]
    fn read_errors_propagate_after_reporting_completed_bytes() {
        let mut reader = Reader {
            reads: 0,
            fail_second_read: true,
        };
        let mut completed = 0;
        assert!(reader
            .compute_with_progress(CancellationToken::new(), &mut |bytes| completed = bytes)
            .is_err());
        assert_eq!(completed, 2);
    }
}
