use cancellation::{CancellationToken, CancellationTokenSource};

use crate::{HashType};
use crate::hash_file::{HashFile};

enum HashFileProcessType {
	Create,
	Update,
	Verify,
	Single,
	Undefined,
}

enum HashFileProcessResult {
	FilesAreMissing,
	NothingToUpdate,
	CouldNotOpenHashFile,
	ErrorsOccurredWhileProcessing,
	NoFileToProcess,
	Success,
	Canceled,
	UnsupportedProcessType,
}

struct HashFileProcessorProgressEventArgs {
	relative_file_path: String,
	file_size: u64,
	bytes_processed: usize,
}

struct HashFileProcessor {
	hash_file: HashFile,
	hash_type: HashType,
	cancellation_token_source: CancellationTokenSource,
	bytes_processed_notification_block_size: u32,
	new_files_updated: bool,
	progress_event: Box<Fn(HashFileProcessorProgressEventArgs)>,
	complete_event: Box<Fn()>,
}

impl HashFileProcessor {
	fn process() -> HashFileProcessResult {
		HashFileProcessResult::Success
	}
	fn process_file(file_path: &str) {

	}
	fn cancel_process(&self) {
		self.cancellation_token_source.cancel();
	}
}