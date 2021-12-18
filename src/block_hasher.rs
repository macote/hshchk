use crossbeam::channel::Sender;
use tokio_util::sync::CancellationToken;

pub struct HashProgress {
    pub bytes_processed: u64,
}

pub trait BlockHasher {
    fn read(&mut self) -> usize;
    fn update(&mut self, byte_count: usize);
    fn digest(&mut self) -> String;
    fn set_bytes_processed_event_sender(&mut self, sender: Sender<HashProgress>);
    fn set_bytes_processed_event_sender_with_bytes_processed_notification_block_size(
        &mut self,
        sender: Sender<HashProgress>,
        bytes_processed_notification_block_size: u64,
    );
    fn bytes_processed_notification_block_size(&self) -> u64;
    fn is_bytes_processed_event_sender_defined(&self) -> bool;
    fn handle_bytes_processed_event(&self, args: HashProgress);
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
