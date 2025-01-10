use crate::block_hasher::{BlockHasher, HashProgress, Hasher, ProgressReporter};
use crate::open_file;
use crossbeam::channel::Sender;
use digest::Digest;
use std::io::{BufReader, Read};
use std::path::Path;

/// A file hasher that computes cryptographic hashes of file contents
///
/// This struct implements the Hasher, ProgressReporter, and BlockHasher traits
/// to provide a complete solution for hashing files with progress reporting.
/// It's generic over any hash algorithm that implements the Digest and FixedOutputReset traits.
pub struct FileHash<T: Digest + digest::FixedOutputReset> {
    /// Buffered reader for the file
    reader: BufReader<std::fs::File>,
    /// The hash algorithm implementation
    hasher: T,
    /// Buffer for reading file data
    buffer: Vec<u8>,
    /// Size of the buffer in bytes
    buffer_size: usize,
    /// Optional sender for progress events
    bytes_processed_event: Option<Sender<HashProgress>>,
    /// Block size for progress notifications
    bytes_processed_notification_block_size: u64,
}

/// Default buffer size for reading files (1 MB)
const DEFAULT_BUFFER_SIZE: usize = 1_048_576;
/// Default block size for progress notifications (2 MB)
const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: u64 = 2_097_152;

impl<T: Digest + digest::FixedOutputReset> FileHash<T> {
    /// Creates a new FileHash with a custom buffer size
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file to hash
    /// * `buffer_size` - Size of the buffer in bytes
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

    /// Creates a new FileHash with the default buffer size
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file to hash
    pub fn new(file_path: &Path) -> Self {
        FileHash::new_with_buffer_size(file_path, DEFAULT_BUFFER_SIZE)
    }
}

// Implement core hashing operations
impl<T: Digest + digest::FixedOutputReset> Hasher for FileHash<T> {
    /// Reads data from the file into the buffer
    ///
    /// Returns the number of bytes read
    fn read(&mut self) -> usize {
        self.buffer.clear();
        let mut adaptor = (&mut self.reader).take(self.buffer_size as u64);
        adaptor.read_to_end(&mut self.buffer).unwrap()
    }

    /// Updates the hash with the specified number of bytes from the buffer
    fn update(&mut self, byte_count: usize) {
        Digest::update(&mut self.hasher, &self.buffer[..byte_count]);
    }

    /// Gets the hash digest as a hex string
    fn digest(&mut self) -> String {
        hex::encode(self.hasher.finalize_reset())
    }
}

// Implement progress reporting
impl<T: Digest + digest::FixedOutputReset> ProgressReporter for FileHash<T> {
    /// Sets the sender for progress events with the default block size
    fn set_bytes_processed_event_sender(&mut self, sender: Sender<HashProgress>) {
        self.set_bytes_processed_event_sender_with_bytes_processed_notification_block_size(
            sender,
            DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
        )
    }

    /// Sets the sender for progress events with a custom block size
    fn set_bytes_processed_event_sender_with_bytes_processed_notification_block_size(
        &mut self,
        sender: Sender<HashProgress>,
        bytes_processed_notification_block_size: u64,
    ) {
        self.bytes_processed_event = Some(sender);
        self.bytes_processed_notification_block_size = bytes_processed_notification_block_size;
    }

    /// Gets the block size for progress notifications
    fn bytes_processed_notification_block_size(&self) -> u64 {
        self.bytes_processed_notification_block_size
    }

    /// Checks if a progress event sender is defined
    fn is_bytes_processed_event_sender_defined(&self) -> bool {
        self.bytes_processed_event.is_some()
    }

    /// Handles a progress event by sending it through the configured sender
    fn handle_bytes_processed_event(&self, args: HashProgress) {
        if let Some(sender) = &self.bytes_processed_event {
            sender.send(args).unwrap()
        }
    }
}

// Implement the combined BlockHasher trait
// This is an empty implementation because all required methods
// are already provided by the Hasher and ProgressReporter implementations
impl<T: Digest + digest::FixedOutputReset> BlockHasher for FileHash<T> {}
