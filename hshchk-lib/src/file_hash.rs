use std::io::{BufReader, Read};

use digest::Digest;

use crate::block_hasher::{BlockHasher, BytesProcessedEventArgs};
use crate::open_file;

pub struct FileHash<'a, T: Digest> {
    reader: BufReader<std::fs::File>,
    hasher: T,
    buffer: Vec<u8>,
    buffer_size: usize,
    bytes_processed_event: Option<Box<Fn(BytesProcessedEventArgs) + Send + Sync + 'a>>,
    bytes_processed_notification_block_size: usize,
}

const DEFAULT_BUFFER_SIZE: usize = 1_048_576;
const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: usize = 2_097_152;

impl<'a, T: Digest> FileHash<'a, T> {
    pub fn new_with_buffer_size(file_path: &str, buffer_size: usize) -> Self {
        FileHash {
            reader: BufReader::new(open_file(&file_path)),
            hasher: T::new(),
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
            bytes_processed_event: None,
            bytes_processed_notification_block_size: 0,
        }
    }
    pub fn new(file_path: &str) -> Self {
        FileHash::new_with_buffer_size(file_path, DEFAULT_BUFFER_SIZE)
    }
}

impl<'a, T: Digest> BlockHasher<'a> for FileHash<'a, T> {
    fn read(&mut self) -> usize {
        self.buffer.clear();
        let mut adaptor = (&mut self.reader).take(self.buffer_size as u64);
        adaptor.read_to_end(&mut self.buffer).unwrap()
    }
    fn update(&mut self, byte_count: usize) {
        self.hasher.input(&self.buffer[..byte_count]);
    }
    fn digest(&mut self) -> String {
        hex::encode(self.hasher.result_reset())
    }
    fn set_bytes_processed_event_handler(
        &mut self,
        handler: Box<Fn(BytesProcessedEventArgs) + Send + Sync + 'a>,
    ) {
        self.set_bytes_processed_event_handler_with_bytes_processed_notification_block_size(
            handler,
            DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
        )
    }
    fn set_bytes_processed_event_handler_with_bytes_processed_notification_block_size(
        &mut self,
        handler: Box<Fn(BytesProcessedEventArgs) + Send + Sync + 'a>,
        bytes_processed_notification_block_size: usize,
    ) {
        self.bytes_processed_event = Some(handler);
        self.bytes_processed_notification_block_size = bytes_processed_notification_block_size;
    }
    fn is_bytes_processed_event_handler_defined(&self) -> bool {
        self.bytes_processed_event.is_some()
    }
    fn bytes_processed_notification_block_size(&self) -> usize {
        self.bytes_processed_notification_block_size
    }
    fn handle_bytes_processed_event(&self, args: BytesProcessedEventArgs) {
        match &self.bytes_processed_event {
            Some(handler) => handler(args),
            None => (),
        }
    }
}
