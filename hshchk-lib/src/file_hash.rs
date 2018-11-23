use std::io::{BufReader, Read};
use std::fs;

use sha1::{Sha1};
use sha2::{Sha256, Sha512};
use blake2::{Blake2b, Blake2s};
use digest::Digest;

use cancellation::CancellationToken;

use crate::open_file;

pub struct FileHashBytesProcessedEventArgs {
	pub bytes_processed: u64,
}

impl Clone for FileHashBytesProcessedEventArgs {
    fn clone(&self) -> FileHashBytesProcessedEventArgs {
        FileHashBytesProcessedEventArgs {
            bytes_processed: self.bytes_processed,
        }
    }
}

pub trait FileHasher<T> {
    fn read(&mut self) -> usize;
    fn update(&mut self, byte_count: usize);
    fn digest(self) -> String;
    fn bytes_processed_notification_block_size(&mut self) -> usize;
    fn is_bytes_processed_event_handler_defined(&mut self) -> bool;
    fn handle_bytes_processed_event(&mut self, args: FileHashBytesProcessedEventArgs);
    fn compute(&mut self, ct: &CancellationToken) {
        let mut bytes_read;
        let mut running_notification_block_size = 0usize;
        let bytes_processed_notification_block_size = self.bytes_processed_notification_block_size();
        let mut event_args = FileHashBytesProcessedEventArgs { bytes_processed: 0 };
        loop {
            if ct.is_canceled() {
                break;
            }

            bytes_read = self.read();
            if bytes_read > 0 {
                self.update(bytes_read);
                if self.is_bytes_processed_event_handler_defined() && bytes_processed_notification_block_size > 0 {
                    event_args.bytes_processed += bytes_read as u64;
                    running_notification_block_size += bytes_read;
                    if running_notification_block_size >= bytes_processed_notification_block_size || bytes_read == 0 {
                        if bytes_read > 0 {
                            running_notification_block_size -= bytes_processed_notification_block_size;
                        }

                        self.handle_bytes_processed_event(event_args.clone());
                    }
                }
            } 
            else {
                break;
            }
        }
    }
}

pub struct FileHash<T: Digest> {
    reader: BufReader<std::fs::File>,
    hasher: T,
    buffer: Vec<u8>,
    buffer_size: usize,
    bytes_processed_event: Option<Box<Fn(FileHashBytesProcessedEventArgs)>>,
    bytes_processed_notification_block_size: usize,
}

const DEFAULT_BUFFER_SIZE: usize = 1048576;
const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: usize = 2097152;

impl<T: Digest> FileHash<T> {
    pub fn new_with_buffer_size(file_path: &str, buffer_size: usize) -> FileHash<T> {
        let file = open_file(&file_path);
        let reader = BufReader::new(file);
        FileHash {
            reader,
            hasher: T::new(),
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
            bytes_processed_event: None,
            bytes_processed_notification_block_size: 0
        }
    }
    pub fn new(file_path: &str) -> FileHash<T> {
        FileHash::new_with_buffer_size(file_path, DEFAULT_BUFFER_SIZE)
    }
    pub fn set_bytes_processed_event_handler(&mut self, handler: Box<Fn(FileHashBytesProcessedEventArgs)>) {
        self.set_bytes_processed_event_handler_with_bytes_processed_notification_block_size(
            handler, 
            DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE
        )
    }
    pub fn set_bytes_processed_event_handler_with_bytes_processed_notification_block_size(&mut self, 
        handler: Box<Fn(FileHashBytesProcessedEventArgs)>,
        bytes_processed_notification_block_size: usize) {
        self.bytes_processed_event = Some(handler);
        self.bytes_processed_notification_block_size = bytes_processed_notification_block_size;
    }
}

impl<T: Digest> FileHasher<T> for FileHash<T> {
    fn read(&mut self) -> usize {
        let mut adaptor = (&mut self.reader).take(self.buffer_size as u64);
        adaptor.read_to_end(&mut self.buffer).unwrap()
    }
    fn update(&mut self, byte_count: usize) {
        self.hasher.input(&self.buffer[..byte_count]);
    }
    fn digest(self) -> String {
        hex::encode(self.hasher.result())
    }
    fn is_bytes_processed_event_handler_defined(&mut self) -> bool {
        self.bytes_processed_event.is_some()
    }
    fn bytes_processed_notification_block_size(&mut self) -> usize {
        self.bytes_processed_notification_block_size
    }
    fn handle_bytes_processed_event(&mut self, args: FileHashBytesProcessedEventArgs) {
        match &self.bytes_processed_event {
            Some(handler) => handler(args),
            None => ()
        }
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