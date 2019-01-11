use std::error::Error;
use std::path::{MAIN_SEPARATOR, PathBuf};

use cancellation::{CancellationTokenSource};

use crate::HashType;
use crate::file_tree::{FileTree, FileTreeProcessor};
use crate::hash_file::HashFile;

#[derive(PartialEq)]
pub enum HashFileProcessType {
	Create,
	Verify,
}

#[derive(Debug, PartialEq)]
pub enum HashFileProcessResult {
	FilesAreMissing,
	CouldNotOpenHashFile,
	ErrorsOccurredWhileProcessing,
	NoFileToProcess,
	Success,
	Canceled,
	UnsupportedProcessType,
}

pub enum FileState {
	Unknown,
	Missing,
	IncorrectSize,
	IncorrectHash,
	Error(String),
}

pub struct ReportEntry {
	pub file_path: String,
	pub state: FileState,
}

pub struct HashFileProcessorProgressEventArgs {
	pub relative_file_path: String,
	pub file_size: u64,
	pub bytes_processed: usize,
}

pub struct HashFileProcessor<'a> {
	hash_file: HashFile,
	hash_type: HashType,
	hash_file_process_type: HashFileProcessType,
	hash_file_name: String,
	app_file_name: String,
	base_path: PathBuf,
	base_path_len: usize,
	cancellation_token: Option<&'a CancellationTokenSource>,
	progress_event: Option<Box<Fn(HashFileProcessorProgressEventArgs)>>,
    bytes_processed_notification_block_size: usize,
	complete_event: Option<Box<Fn()>>,
	report: Vec<ReportEntry>,
}

const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: usize = 2097152;

impl<'a> HashFileProcessor<'a> {
	pub fn new(
		hash_file_process_type: HashFileProcessType,
		hash_type: HashType,
		hash_file_name: &str,
		app_file_name: &str,
		base_path: &str) -> Self {
		let base_path_normalized: &str;
		if base_path.ends_with(MAIN_SEPARATOR) {
			base_path_normalized = &base_path[..base_path.len() - 1];
		}
		else if base_path.is_empty() {
			base_path_normalized = ".";
		}
		else {
			base_path_normalized = base_path;
		}
		HashFileProcessor {
			hash_file: HashFile::new(),
			hash_type,
			hash_file_process_type,
			hash_file_name: format!("{}{}{}", ".", MAIN_SEPARATOR, hash_file_name),
			app_file_name: format!("{}{}{}", ".", MAIN_SEPARATOR, app_file_name),
			base_path: PathBuf::from(base_path_normalized),
			base_path_len: base_path_normalized.len(),
			cancellation_token: None,
			progress_event: None,
			bytes_processed_notification_block_size: DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
			complete_event: None,
			report: Vec::new(),
		}
	}
	pub fn process(&mut self, cancellation_token: &'a CancellationTokenSource) -> HashFileProcessResult {
		self.cancellation_token = Some(cancellation_token);
		let mut result = HashFileProcessResult::Success;

		if self.hash_file_process_type == HashFileProcessType::Verify {
			self.hash_file.load(&self.hash_file_name);
		}
		else if self.hash_file_process_type != HashFileProcessType::Create {
			result = HashFileProcessResult::UnsupportedProcessType;
		}

		let path = self.base_path.clone();
		let mut file_tree = FileTree::new(self);
		match file_tree.traverse(&path) {
			Err(why) => panic!("couldn't traverse {}: {}", 
				path.display(),
				why.description()),
			Ok(file) => file
		}

		if self.hash_file_process_type == HashFileProcessType::Create {
			if self.hash_file.is_empty() {
				result = HashFileProcessResult::NoFileToProcess;
			}
			else if !self.report.is_empty() {
				result = HashFileProcessResult::ErrorsOccurredWhileProcessing;
			}
			else {
				self.hash_file.save(&self.hash_file_name);
			}
		}
		else if self.hash_file_process_type == HashFileProcessType::Verify {
			if !self.hash_file.is_empty() {
				for file_path in self.hash_file.get_file_paths() {
					self.report.push(ReportEntry {
						file_path, state: FileState::Missing
					});
				}
			}
			else if !self.report.is_empty() {
				result = HashFileProcessResult::ErrorsOccurredWhileProcessing;
			}
		}

		result
	}
	pub fn cancel_process(&self) {
		if let Some(token) = self.cancellation_token {
			token.cancel();
		}
	}
	pub fn save_report(&self) {

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

impl<'a> FileTreeProcessor for HashFileProcessor<'a> {
	fn process_file(&mut self, file_path: &PathBuf) {
		let file_path_str = file_path.to_str().unwrap();
		if file_path_str == self.app_file_name || file_path_str == self.hash_file_name {
			return // skip app file and current hash file
		}

		let relative_file_path = &file_path_str[(self.base_path_len + 1)..];
		let file_size = file_path.metadata().unwrap().len();
		let hash_file_entry = self.hash_file.get_entry(relative_file_path);
		if let Some(file_entry) = hash_file_entry {
			if file_size != file_entry.size {
				self.report.push(ReportEntry {
					file_path: relative_file_path.to_string(), state: FileState::IncorrectSize
				});
			}
		}
		else {
			if self.hash_file_process_type == HashFileProcessType::Verify {
				self.report.push(ReportEntry {
					file_path: relative_file_path.to_string(), state: FileState::Unknown
				});
				return
			}
		}

        let mut file_hasher = crate::get_file_hasher(self.hash_type, file_path_str);

		if let Some(handler) = &self.progress_event {
			let file_path = relative_file_path.to_string();
			handler(HashFileProcessorProgressEventArgs {
				relative_file_path: file_path,
				file_size,
				bytes_processed: 0,
			});
			file_hasher.set_bytes_processed_event_handler(
				Box::new(|_args| {
					// TODO: fix this. use Rc<> (or Arc<>?) instead of Box<>.
					// handler(HashFileProcessorProgressEventArgs {
					// 	relative_file_path: file_path,
					// 	file_size,
					// 	bytes_processed: args.bytes_processed,
					// });
				}));
		}

        file_hasher.compute(&self.cancellation_token.unwrap());

		if self.hash_file_process_type == HashFileProcessType::Create {
			self.hash_file.add_entry(relative_file_path, file_size, &file_hasher.digest());
		} else if self.hash_file_process_type == HashFileProcessType::Verify {
			if let Some(file_entry) = hash_file_entry {
				if file_hasher.digest() != file_entry.digest {
					self.report.push(ReportEntry {
						file_path: relative_file_path.to_string(), state: FileState::IncorrectHash
					});
				}
			}
			
			self.hash_file.remove_entry(relative_file_path);
		}
	}
}