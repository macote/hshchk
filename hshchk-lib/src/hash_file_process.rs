use std::path::{Path, PathBuf};

use cancellation::{CancellationTokenSource};

use crate::{HashType};
use crate::file_tree::{FileTree, FileTreeProcessor};
use crate::hash_file::{HashFile};

#[derive(PartialEq)]
pub enum HashFileProcessType {
	Create,
	Update,
	Verify,
	Single,
	Undefined,
}

#[derive(PartialEq)]
pub enum HashFileProcessResult {
	FilesAreMissing,
	NothingToUpdate,
	CouldNotOpenHashFile,
	ErrorsOccurredWhileProcessing,
	NoFileToProcess,
	Success,
	Canceled,
	UnsupportedProcessType,
}

pub struct HashFileProcessorProgressEventArgs {
	relative_file_path: String,
	file_size: u64,
	bytes_processed: usize,
}

pub struct HashFileProcessor {
	hash_file: HashFile,
	hash_type: HashType,
	hash_file_process_type: HashFileProcessType,
	hash_file_name: String,
	app_file_name: String,
	base_path: String,
	cancellation_token_source: CancellationTokenSource,
	new_files_updated: bool,
	progress_event: Option<Box<Fn(HashFileProcessorProgressEventArgs)>>,
    bytes_processed_notification_block_size: usize,
	complete_event: Option<Box<Fn()>>,
}

const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: usize = 2097152;

impl HashFileProcessor {
	pub fn new(
		hash_file_process_type: HashFileProcessType,
		hash_type: HashType,
		hash_file_name: &str,
		app_file_name: &str,
		base_path: &str,
		cancellation_token_source: Option<CancellationTokenSource>) -> Self {
		HashFileProcessor {
			hash_file: HashFile::new(),
			hash_type,
			hash_file_process_type,
			hash_file_name: hash_file_name.to_string(),
			app_file_name: app_file_name.to_string(),
			base_path: base_path.to_string(),
			cancellation_token_source: CancellationTokenSource::new(),
			new_files_updated: false,
			progress_event: None,
			bytes_processed_notification_block_size: DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
			complete_event: None,
		}
	}
	pub fn process(mut self) -> HashFileProcessResult {
		self.new_files_updated = false;
		let mut result = HashFileProcessResult::Success;

		if self.hash_file_process_type == HashFileProcessType::Verify
			|| self.hash_file_process_type == HashFileProcessType::Update {
			self.hash_file.load(&self.hash_file_name);
		}
		else if self.hash_file_process_type == HashFileProcessType::Single {
			self.process_file(&PathBuf::from(&self.base_path));
		}
		else if self.hash_file_process_type != HashFileProcessType::Create {
			result = HashFileProcessResult::UnsupportedProcessType;
		}

		let file_tree = FileTree::new(&self);
		file_tree.traverse(&self.base_path);

		if self.hash_file_process_type == HashFileProcessType::Create {
			if self.hash_file.is_empty() {
				result = HashFileProcessResult::NoFileToProcess;
			}
			else {
				self.hash_file.save(&self.hash_file_name);
			}
		}

		result
	}
	fn cancel_process(&self) {
		self.cancellation_token_source.cancel();
	}
	fn save_report(&self) {

	}
    pub fn set_progress_event_handler(&mut self, handler: Box<Fn(HashFileProcessorProgressEventArgs)>) {
        self.set_progress_event_handler_with_bytes_processed_notification_block_size(
            handler, 
            DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE
        )
    }
    pub fn set_progress_event_handler_with_bytes_processed_notification_block_size(&mut self, 
        handler: Box<Fn(HashFileProcessorProgressEventArgs)>,
        bytes_processed_notification_block_size: usize) {
        self.progress_event = Some(handler);
        self.bytes_processed_notification_block_size = bytes_processed_notification_block_size;
    }
    pub fn set_complete_event_handler(&mut self, handler: Box<Fn()>) {
        self.complete_event = Some(handler);
    }
}

impl FileTreeProcessor for HashFileProcessor {
	fn process_file(&self, file_path: &PathBuf) {
		if self.cancellation_token_source.is_canceled() {

		}

		let file_path_str = file_path.to_str().unwrap();
		let relative_file_path: &str;
		if self.base_path.len() == file_path_str.len() {
			//let base_path_path = Path::new(&self.base_path);
			relative_file_path = &self.base_path;
		} else {
			relative_file_path = &file_path_str[self.base_path.len()..];
		}

		let some_file_entry = self.hash_file.get_entry(relative_file_path);
		match some_file_entry {
			None => {
				if self.hash_file_process_type == HashFileProcessType::Verify {
					// report_.AddLine(L"Unknown             : " + relativefilepath);
					// return
				} else if self.hash_file_process_type == HashFileProcessType::Update {
					// newhashfile_.AddFileEntry(fileentry.filepath(), fileentry.size(), fileentry.digest());
					// hashfile_.RemoveFileEntry(relativefilepath);
					// return
				}
			}
			Some(file_entry) =>  {

			}
		}

		println!("entry: {}", file_path.display())
	}
}