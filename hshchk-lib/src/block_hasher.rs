use cancellation::CancellationToken;

pub struct BytesProcessedEventArgs {
	pub bytes_processed: usize,
}

pub trait BlockHasher<'a> {
    fn read(&mut self) -> usize;
    fn update(&mut self, byte_count: usize);
    fn digest(&mut self) -> String;
    fn set_bytes_processed_event_handler(&mut self, handler: Box<Fn(BytesProcessedEventArgs) + Send + Sync + 'a>);
    fn set_bytes_processed_event_handler_with_bytes_processed_notification_block_size(&mut self,
        handler: Box<Fn(BytesProcessedEventArgs) + Send + Sync + 'a>,
        bytes_processed_notification_block_size: usize);
    fn bytes_processed_notification_block_size(&self) -> usize;
    fn is_bytes_processed_event_handler_defined(&self) -> bool;
    fn handle_bytes_processed_event(&self, args: BytesProcessedEventArgs);
    fn compute(&mut self, cancellation_token: &CancellationToken) {
        let mut bytes_read;
        let mut running_notification_block_size = 0usize;
        let mut bytes_processed = 0usize;
        let bytes_processed_notification_block_size = self.bytes_processed_notification_block_size();
        loop {
            if cancellation_token.is_canceled() {
                break;
            }

            bytes_read = self.read();
            if bytes_read > 0 {
                self.update(bytes_read);
                if self.is_bytes_processed_event_handler_defined() && bytes_processed_notification_block_size > 0 {
                    bytes_processed += bytes_read;
                    running_notification_block_size += bytes_read;
                    if running_notification_block_size >= bytes_processed_notification_block_size || bytes_read == 0 {
                        if bytes_read > 0 {
                            running_notification_block_size -= bytes_processed_notification_block_size;
                        }

                        self.handle_bytes_processed_event(BytesProcessedEventArgs { bytes_processed });
                    }
                }
            }
            else {
                break;
            }
        }
    }
}