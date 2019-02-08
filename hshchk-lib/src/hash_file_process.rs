use std::error::Error;
use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use cancellation::{CancellationToken};

use crate::HashType;
use crate::file_tree::{FileTree, FileTreeProcessor};
use crate::hash_file::HashFile;

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
	cancellation_token: Option<&'a CancellationToken>,
	progress_event: Option<Box<Fn(HashFileProcessorProgressEventArgs)>>,
    bytes_processed_notification_block_size: usize,
	complete_event: Option<Box<Fn()>>,
	report: Vec<ReportEntry>,
}

const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: usize = 2097152;

impl<'a> HashFileProcessor<'a> {
	pub fn new(
		hash_type: HashType,
		bin_path_str: &str,
		base_path_str: &str,
		force_create: bool) -> Self {
		let hash_type_str: &str = hash_type.into();
    	let hash_file_name = format!("checksum.{}", hash_type_str.to_lowercase());
		let hash_file_path: PathBuf = [base_path_str, &hash_file_name].iter().collect();
    	let mut process_type = HashFileProcessType::Create;
		if hash_file_path.is_file() && !force_create {
			process_type = HashFileProcessType::Verify;
		}

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

		let bin_path = Path::new(bin_path_str);
		let mut bin_file_name = bin_path.file_name().unwrap().to_str().unwrap();
		let tmp = bin_path.canonicalize().unwrap();
		let bin_cano_path = tmp.to_str().unwrap();
		let base_path = PathBuf::from(base_path_str);
		let tmp = base_path.canonicalize().unwrap();
		let base_cano_path = tmp.to_str().unwrap();
		if bin_cano_path != format!("{}{}{}", base_cano_path, MAIN_SEPARATOR, bin_file_name) {
			// the app binary is not in the target root. ignore skip logic.
			bin_file_name = "";
		}

		HashFileProcessor {
			hash_file: HashFile::new(),
			hash_type,
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
	pub fn process(&mut self, cancellation_token: &'a CancellationToken) -> HashFileProcessResult {
		self.cancellation_token = Some(cancellation_token);

		if self.process_type == HashFileProcessType::Verify {
			self.hash_file.load(&self.hash_file_path);
		}

		let path = self.base_path.clone();
		let mut file_tree = FileTree::new(self);
		match file_tree.traverse(&path, cancellation_token) {
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
		if relative_file_path == self.bin_file_name {
			return; // skip app binary file
		}

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
			if self.process_type == HashFileProcessType::Verify {
				self.report.push(ReportEntry {
					file_path: relative_file_path.to_string(), state: FileState::Unknown
				});
				return;
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

		let cancellation_token = &self.cancellation_token.unwrap();
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