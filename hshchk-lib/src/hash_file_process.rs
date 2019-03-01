use std::env;
use std::error::Error;
use std::path::{MAIN_SEPARATOR, PathBuf};
use std::sync::Arc;

use cancellation::{CancellationToken};

use strum::IntoEnumIterator;

use crate::HashType;
use crate::file_tree::{FileTree, FileTreeProcessor};
use crate::hash_file::HashFile;

static HASH_FILE_BASE_NAME: &str = "hshchk";

#[derive(Debug, Copy, Clone, PartialEq)]
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
}

#[derive(Debug, PartialEq)]
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
	process_type: HashFileProcessType,
	hash_file_path: String,
	bin_file_name: String,
	base_path: PathBuf,
	base_path_len: usize,
	cancellation_token: Option<Arc<CancellationToken>>,
	progress_event: Option<Box<Fn(HashFileProcessorProgressEventArgs) + Send + Sync + 'a>>,
    bytes_processed_notification_block_size: usize,
	complete_event: Option<Box<Fn() + Send + Sync + 'a>>,
	report: Vec<ReportEntry>,
}

const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: usize = 2097152;

impl<'a> HashFileProcessor<'a> {
	pub fn new(
		hash_type: HashType,
		base_path_str: &str,
		force_create: bool) -> Self {
		let mut process_type = HashFileProcessType::Create;
		let mut actual_hash_type = hash_type;
		if !force_create {
			if let Some(existing_hash_type) = get_existing_file_hash_type(base_path_str, hash_type) {
				actual_hash_type = existing_hash_type;
				process_type = HashFileProcessType::Verify;
			}
		}

    	let hash_file_name = get_hash_file_name(actual_hash_type);
		let hash_file_path: PathBuf = [base_path_str, &hash_file_name].iter().collect();

		let base_path_normalized: &str;
		if base_path_str.ends_with(MAIN_SEPARATOR) {
			base_path_normalized = &base_path_str[..base_path_str.len() - 1];
		}
		else if base_path_str.is_empty() {
			base_path_normalized = ".";
		}
		else {
			base_path_normalized = base_path_str;
		}

		let bin_path = env::current_exe().unwrap();
		let mut bin_file_name = bin_path.file_name().unwrap().to_str().unwrap();
		let mut work_path = env::current_dir().unwrap();
		work_path.push(bin_file_name);
		if !work_path.is_file() {
			// the app binary is not in the target root. ignore skip logic.
			bin_file_name = "";
		}

		HashFileProcessor {
			hash_file: HashFile::new(),
			hash_type: actual_hash_type,
			process_type,
			hash_file_path: String::from(hash_file_path.to_str().unwrap()),
			bin_file_name: bin_file_name.to_string(),
			base_path: PathBuf::from(base_path_normalized),
			base_path_len: base_path_normalized.len(),
			cancellation_token: None,
			progress_event: None,
			bytes_processed_notification_block_size: DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
			complete_event: None,
			report: Vec::new(),
		}
	}
	pub fn process(&mut self, cancellation_token: Arc<CancellationToken>) -> HashFileProcessResult {
		self.cancellation_token = Some(Arc::clone(&cancellation_token));

		if self.process_type == HashFileProcessType::Verify {
			self.hash_file.load(&self.hash_file_path);
		}

		let path = self.base_path.clone();
		let mut file_tree = FileTree::new(self);
		match file_tree.traverse(&path, &cancellation_token) {
			Err(why) => panic!("couldn't traverse {}: {}",
				path.display(),
				why.description()),
			Ok(_) => ()
		}

		if cancellation_token.is_canceled() {
			return HashFileProcessResult::Canceled;
		}

		if self.process_type == HashFileProcessType::Create {
			if self.hash_file.is_empty() {
				return HashFileProcessResult::NoFileToProcess;
			}

			if !self.report.is_empty() {
				return HashFileProcessResult::ErrorsOccurredWhileProcessing;
			}

			self.hash_file.save(&self.hash_file_path);
		}
		else if self.process_type == HashFileProcessType::Verify {
			if !self.hash_file.is_empty() {
				for file_path in self.hash_file.get_file_paths() {
					self.report.push(ReportEntry {
						file_path, state: FileState::Missing
					});
				}
			}
			else if !self.report.is_empty() {
				return HashFileProcessResult::ErrorsOccurredWhileProcessing;
			}
		}

		HashFileProcessResult::Success
	}
	pub fn save_report(&self) {
	}
    pub fn set_progress_event_handler(&mut self, handler: Box<Fn(HashFileProcessorProgressEventArgs) + Send + Sync + 'a>) {
        self.set_progress_event_handler_with_bytes_processed_notification_block_size(
            handler,
            DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE
        )
    }
    pub fn set_progress_event_handler_with_bytes_processed_notification_block_size(&mut self,
        handler: Box<Fn(HashFileProcessorProgressEventArgs) + Send + Sync + 'a>,
        bytes_processed_notification_block_size: usize) {
        self.progress_event = Some(handler);
        self.bytes_processed_notification_block_size = bytes_processed_notification_block_size;
    }
    pub fn set_complete_event_handler(&mut self, handler: Box<Fn() + Send + Sync + 'a>) {
        self.complete_event = Some(handler);
    }

	pub fn get_process_type(&self) -> HashFileProcessType {
		self.process_type
	}
}

impl<'a> FileTreeProcessor for HashFileProcessor<'a> {
	fn process_file(&mut self, file_path: &PathBuf) {
		let file_path_str = file_path.to_str().unwrap();
		if file_path_str == self.hash_file_path {
			return; // skip current hash file
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
		else if relative_file_path == self.bin_file_name {
			return; // skip app binary file
		}
		else if self.process_type == HashFileProcessType::Verify {
			self.report.push(ReportEntry {
				file_path: relative_file_path.to_string(), state: FileState::Unknown
			});
			return;
		}

        let mut file_hasher = crate::get_file_hasher(self.hash_type, file_path_str);

		if let Some(handler) = &self.progress_event {
			let file_path = relative_file_path.to_string();
			handler(HashFileProcessorProgressEventArgs {
				relative_file_path: file_path.clone(),
				file_size,
				bytes_processed: 0,
			});
			file_hasher.set_bytes_processed_event_handler(
				Box::new(move |args| {
					handler(HashFileProcessorProgressEventArgs {
						relative_file_path: file_path.clone(),
						file_size,
						bytes_processed: args.bytes_processed,
					});
				}));
		}

		let cancellation_token = self.cancellation_token.as_ref().unwrap();
        file_hasher.compute(cancellation_token);

		if cancellation_token.is_canceled() {
			return;
		}

		if self.process_type == HashFileProcessType::Create {
			self.hash_file.add_entry(relative_file_path, file_size, &file_hasher.digest());
		} else if self.process_type == HashFileProcessType::Verify {
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

fn get_hash_file_name(hash_type: HashType) -> String {
	let hash_type_str: &str = hash_type.into();
	String::from(format!("{}.{}", HASH_FILE_BASE_NAME, hash_type_str.to_lowercase()))
}

fn get_existing_file_hash_type(base_path_str: &str, desired_hash_type: HashType) -> Option<HashType> {
	let mut hash_file_path = PathBuf::from(base_path_str);
	let hash_file_exists = |hash_file_path: &mut PathBuf, hash_type: HashType| -> bool {
		hash_file_path.push(get_hash_file_name(hash_type));
		hash_file_path.is_file()
	};

	if hash_file_exists(&mut hash_file_path, desired_hash_type) {
		return Some(desired_hash_type);
	}
	else {
		hash_file_path.pop();
		for hash_type in HashType::iter() {
			if hash_file_exists(&mut hash_file_path, hash_type) {
				return Some(hash_type);
			}

			hash_file_path.pop();
		}
	}

	None
}