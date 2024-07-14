use crate::block_hasher::{BlockHasher, HashProgress};
use crate::open_file;
use crossbeam::channel::Sender;
use digest::Digest;
use std::io::{BufReader, Read};
use std::path::Path;

pub struct FileHash<T: Digest + digest::FixedOutputReset> {
    reader: BufReader<std::fs::File>,
    hasher: T,
    buffer: Vec<u8>,
    buffer_size: usize,
    bytes_processed_event: Option<Sender<HashProgress>>,
    bytes_processed_notification_block_size: u64,
}

const DEFAULT_BUFFER_SIZE: usize = 1_048_576;
const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: u64 = 2_097_152;

impl<T: Digest + digest::FixedOutputReset> FileHash<T> {
    pub fn new_with_buffer_size(file_path: &Path, buffer_size: usize) -> Self {
        FileHash {
            reader: BufReader::new(open_file(file_path)),
            hasher: T::new(),
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
            bytes_processed_event: None,
            bytes_processed_notification_block_size: 0,
        }
    }
    pub fn new(file_path: &Path) -> Self {
        FileHash::new_with_buffer_size(file_path, DEFAULT_BUFFER_SIZE)
    }
}

impl<T: Digest + digest::FixedOutputReset> BlockHasher for FileHash<T> {
    fn read(&mut self) -> usize {
        self.buffer.clear();
        let mut adaptor = (&mut self.reader).take(self.buffer_size as u64);
        adaptor.read_to_end(&mut self.buffer).unwrap()
    }
    fn update(&mut self, byte_count: usize) {
        Digest::update(&mut self.hasher, &self.buffer[..byte_count]);
    }
    fn digest(&mut self) -> String {
        hex::encode(self.hasher.finalize_reset())
    }
    fn set_bytes_processed_event_sender(&mut self, sender: Sender<HashProgress>) {
        self.set_bytes_processed_event_sender_with_bytes_processed_notification_block_size(
            sender,
            DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
        )
    }
    fn set_bytes_processed_event_sender_with_bytes_processed_notification_block_size(
        &mut self,
        sender: Sender<HashProgress>,
        bytes_processed_notification_block_size: u64,
    ) {
        self.bytes_processed_event = Some(sender);
        self.bytes_processed_notification_block_size = bytes_processed_notification_block_size;
    }
    fn is_bytes_processed_event_sender_defined(&self) -> bool {
        self.bytes_processed_event.is_some()
    }
    fn bytes_processed_notification_block_size(&self) -> u64 {
        self.bytes_processed_notification_block_size
    }
    fn handle_bytes_processed_event(&self, args: HashProgress) {
        match &self.bytes_processed_event {
            Some(sender) => sender.send(args).unwrap(),
            None => (),
        }
    }
}
