use crossbeam::channel::Sender;
use tokio_util::sync::CancellationToken;

/// Represents progress information during hash computation
pub struct HashProgress {
    /// Number of bytes processed so far
    pub bytes_processed: u64,
}

/// Core trait for hash operations
///
/// This trait defines the fundamental operations needed for hashing:
/// reading data, updating the hash state, and getting the final digest.
/// It separates these core operations from progress reporting and computation control.
pub trait Hasher {
    /// Read data into the internal buffer
    ///
    /// Returns the number of bytes read
    fn read(&mut self) -> usize;

    /// Update the hash with the specified number of bytes from the buffer
    ///
    /// This method should update the internal hash state with the data in the buffer
    fn update(&mut self, byte_count: usize);

    /// Get the hash digest as a hex string
    ///
    /// This method should finalize the hash and return the digest as a hex string
    fn digest(&mut self) -> String;
}

/// Trait for progress reporting during hash operations
///
/// This trait defines methods for reporting progress during hash computation.
/// It's separated from the core hashing operations to allow for more flexible
/// implementations and better separation of concerns.
pub trait ProgressReporter {
    /// Set the sender for progress events with default block size
    fn set_bytes_processed_event_sender(&mut self, sender: Sender<HashProgress>);

    /// Set the sender for progress events with custom block size
    ///
    /// This allows for more fine-grained control over how often progress events are sent
    fn set_bytes_processed_event_sender_with_bytes_processed_notification_block_size(
        &mut self,
        sender: Sender<HashProgress>,
        bytes_processed_notification_block_size: u64,
    );

    /// Get the block size for progress notifications
    ///
    /// This determines how often progress events are sent
    fn bytes_processed_notification_block_size(&self) -> u64;

    /// Check if a progress event sender is defined
    fn is_bytes_processed_event_sender_defined(&self) -> bool;

    /// Handle a progress event
    ///
    /// This method should send the progress information through the configured sender
    fn handle_bytes_processed_event(&self, args: HashProgress);
}

/// Combined trait for hash computation with progress reporting and cancellation
///
/// This trait combines the Hasher and ProgressReporter traits and adds a default
/// implementation for the compute method that handles the main hashing loop,
/// cancellation, and progress reporting. This allows for a clean separation of
/// concerns while still providing a convenient interface for clients.
pub trait BlockHasher: Hasher + ProgressReporter {
    /// Compute the hash with support for cancellation and progress reporting
    ///
    /// This method reads data in chunks, updates the hash state, and reports progress
    /// until all data is processed or cancellation is requested.
    fn compute(&mut self, cancellation_token: CancellationToken) {
        let mut bytes_read;
        let mut running_notification_block_size = 0u64;
        let mut bytes_processed = 0u64;
        let bytes_processed_notification_block_size =
            self.bytes_processed_notification_block_size();
        loop {
            if cancellation_token.is_cancelled() {
                break;
            }

            bytes_read = self.read();
            if bytes_read > 0 {
                self.update(bytes_read);
                if self.is_bytes_processed_event_sender_defined()
                    && bytes_processed_notification_block_size > 0
                {
                    bytes_processed += bytes_read as u64;
                    running_notification_block_size += bytes_read as u64;
                    if running_notification_block_size >= bytes_processed_notification_block_size
                        || bytes_read == 0
                    {
                        if bytes_read > 0 {
                            running_notification_block_size -=
                                bytes_processed_notification_block_size;
                        }

                        self.handle_bytes_processed_event(HashProgress { bytes_processed });
                    }
                }
            } else {
                break;
            }
        }
    }
}
