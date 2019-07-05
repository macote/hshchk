use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use cancellation::CancellationToken;
use strum::IntoEnumIterator;

use crate::file_tree::{FileTree, FileTreeProcessor};
use crate::hash_file::HashFile;
use crate::HashType;

static HASH_FILE_BASE_NAME: &str = "hshchk";

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum HashFileProcessType {
    Create,
    Verify,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum HashFileProcessResult {
    Success,
    Error,
    Canceled,
}

#[derive(Debug, PartialEq)]
pub enum FileErrorState {
    Extra,
    Missing,
    IncorrectSize,
    IncorrectHash,
    Error(String),
}

pub struct FileErrorEntry {
    pub file_path: String,
    pub state: FileErrorState,
}

pub struct HashFileProcessProgressEventArgs {
    pub relative_file_path: String,
    pub file_size: u64,
    pub bytes_processed: usize,
}

#[derive(Default)]
pub struct HashFileProcessOptions {
    pub base_path: String,
    pub hash_type: Option<HashType>,
    pub force_create: Option<bool>,
    pub report_extra_files: Option<bool>,
    pub check_file_size_only: Option<bool>,
}

pub struct HashFileProcessor<'a> {
    hash_file: HashFile,
    hash_type: HashType,
    process_type: HashFileProcessType,
    hash_file_path: String,
    bin_file_name: String,
    base_path: PathBuf,
    base_path_len: usize,
    check_file_size_only: bool,
    report_extra_files: bool,
    error_count: usize,
    bytes_processed_notification_block_size: usize,
    cancellation_token: Option<Arc<CancellationToken>>,
    progress_event: Option<Box<Fn(HashFileProcessProgressEventArgs) + Send + Sync + 'a>>,
    error_event: Option<Box<Fn(FileErrorEntry) + Send + Sync + 'a>>,
    complete_event: Option<Box<Fn(HashFileProcessResult) + Send + Sync + 'a>>,
}

const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: usize = 2_097_152;

impl<'a> HashFileProcessor<'a> {
    pub fn new_with_options(options: HashFileProcessOptions) -> Self {
        let mut process_type = HashFileProcessType::Create;
        let mut actual_hash_type = options.hash_type.unwrap_or(HashType::SHA1);
        let cano_base_path = fs::canonicalize(PathBuf::from(options.base_path)).unwrap();
        let cano_base_path_str = cano_base_path.to_str().unwrap();
        if !options.force_create.unwrap_or_default() {
            if let Some(existing_hash_type) =
                get_existing_file_hash_type(cano_base_path_str, actual_hash_type)
            {
                actual_hash_type = existing_hash_type;
                process_type = HashFileProcessType::Verify;
            }
        }

        let hash_file_name = get_hash_file_name(actual_hash_type);
        let hash_file_path: PathBuf = [cano_base_path_str, &hash_file_name].iter().collect();

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
            base_path: PathBuf::from(cano_base_path_str),
            base_path_len: cano_base_path_str.len(),
            check_file_size_only: options.check_file_size_only.unwrap_or_default(),
            report_extra_files: options.report_extra_files.unwrap_or_default(),
            error_count: 0,
            bytes_processed_notification_block_size:
                DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
            cancellation_token: None,
            progress_event: None,
            error_event: None,
            complete_event: None,
        }
    }
    pub fn new(hash_type: HashType, base_path_str: &str, force_create: bool) -> Self {
        HashFileProcessor::new_with_options(HashFileProcessOptions {
            base_path: String::from(base_path_str),
            hash_type: Some(hash_type),
            force_create: Some(force_create),
            ..Default::default()
        })
    }
    pub fn handle_error(&mut self, file_path: &str, error_state: FileErrorState) {
        self.error_count += 1;
        if let Some(handler) = &self.error_event {
            handler(FileErrorEntry {
                file_path: file_path.to_string(),
                state: error_state,
            });
        }
    }
    pub fn process(&mut self, cancellation_token: Arc<CancellationToken>) -> HashFileProcessResult {
        let result = self.process_internal(cancellation_token);
        if let Some(handler) = &self.complete_event {
            handler(result);
        }

        result
    }
    pub fn process_internal(
        &mut self,
        cancellation_token: Arc<CancellationToken>,
    ) -> HashFileProcessResult {
        self.cancellation_token = Some(Arc::clone(&cancellation_token));

        if self.process_type == HashFileProcessType::Verify {
            self.hash_file.load(&self.hash_file_path);
        }

        let path = self.base_path.clone();
        let mut file_tree = FileTree::new(self);

        if let Err(why) = file_tree.traverse(&path, &cancellation_token) {
            panic!(
                "couldn't traverse {}: {}",
                path.display(),
                why.description()
            );
        }

        if cancellation_token.is_canceled() {
            return HashFileProcessResult::Canceled;
        }

        if self.error_count > 0 {
            return HashFileProcessResult::Error;
        } else if self.process_type == HashFileProcessType::Create {
            if self.hash_file.is_empty() {
                return HashFileProcessResult::Error;
            }

            self.hash_file.save(&self.hash_file_path);
        } else if self.process_type == HashFileProcessType::Verify
            && self.report_extra_files
            && !self.hash_file.is_empty() {
            for file_path in self.hash_file.get_file_paths() {
                self.handle_error(&file_path, FileErrorState::Missing);
            }

            return HashFileProcessResult::Error;
        }

        HashFileProcessResult::Success
    }
    pub fn set_progress_event_handler(
        &mut self,
        handler: Box<Fn(HashFileProcessProgressEventArgs) + Send + Sync + 'a>,
    ) {
        self.set_progress_event_handler_with_bytes_processed_notification_block_size(
            handler,
            DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
        )
    }
    pub fn set_progress_event_handler_with_bytes_processed_notification_block_size(
        &mut self,
        handler: Box<Fn(HashFileProcessProgressEventArgs) + Send + Sync + 'a>,
        bytes_processed_notification_block_size: usize,
    ) {
        self.progress_event = Some(handler);
        self.bytes_processed_notification_block_size = bytes_processed_notification_block_size;
    }
    pub fn set_error_event_handler(&mut self, handler: Box<Fn(FileErrorEntry) + Send + Sync + 'a>) {
        self.error_event = Some(handler);
    }
    pub fn set_complete_event_handler(
        &mut self,
        handler: Box<Fn(HashFileProcessResult) + Send + Sync + 'a>,
    ) {
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
        let file_size: u64;
        match file_path.metadata() {
            Ok(metadata) => file_size = metadata.len(),
            Err(error) => {
                self.handle_error(relative_file_path, FileErrorState::Error(error.to_string()));
                return;
            }
        }

        let hash_file_entry = self.hash_file.get_entry(relative_file_path);
        if let Some(file_entry) = hash_file_entry {
            if file_size != file_entry.size {
                self.handle_error(relative_file_path, FileErrorState::IncorrectSize);
                return;
            }
        } else if relative_file_path == self.bin_file_name {
            return; // skip app binary file
        } else if self.process_type == HashFileProcessType::Verify {
            self.handle_error(relative_file_path, FileErrorState::Extra);
            return;
        }

        let mut digest = String::from("");
        if !(self.check_file_size_only && self.process_type == HashFileProcessType::Verify) {
            {
                let mut file_hasher = crate::get_file_hasher(self.hash_type, file_path_str);
                if let Some(handler) = &self.progress_event {
                    let file_path = relative_file_path.to_string();
                    handler(HashFileProcessProgressEventArgs {
                        relative_file_path: file_path.clone(),
                        file_size,
                        bytes_processed: 0,
                    });
                    file_hasher.set_bytes_processed_event_handler(Box::new(move |args| {
                        handler(HashFileProcessProgressEventArgs {
                            relative_file_path: file_path.clone(),
                            file_size,
                            bytes_processed: args.bytes_processed,
                        });
                    }));
                }

                let cancellation_token = self.cancellation_token.as_ref().unwrap();
                file_hasher.compute(cancellation_token);
                digest = file_hasher.digest();

                if cancellation_token.is_canceled() {
                    return;
                }
            }
        }

        if self.process_type == HashFileProcessType::Create {
            self.hash_file
                .add_entry(relative_file_path, file_size, &digest);
        } else if self.process_type == HashFileProcessType::Verify {
            if let Some(file_entry) = hash_file_entry {
                if !self.check_file_size_only && digest != file_entry.digest {
                    self.handle_error(relative_file_path, FileErrorState::IncorrectHash);
                }
            }

            self.hash_file.remove_entry(relative_file_path);
        }
    }
}

fn get_hash_file_name(hash_type: HashType) -> String {
    let hash_type_str: &str = hash_type.into();
    format!("{}.{}", HASH_FILE_BASE_NAME, hash_type_str.to_lowercase())
}

fn get_existing_file_hash_type(
    base_path_str: &str,
    desired_hash_type: HashType,
) -> Option<HashType> {
    let mut hash_file_path = PathBuf::from(base_path_str);
    let hash_file_exists = |hash_file_path: &mut PathBuf, hash_type: HashType| -> bool {
        hash_file_path.push(get_hash_file_name(hash_type));
        hash_file_path.is_file()
    };

    if hash_file_exists(&mut hash_file_path, desired_hash_type) {
        return Some(desired_hash_type);
    } else {
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
