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
