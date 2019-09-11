use crate::file_tree::{FileTree, FileTreeProcessor};
use crate::hash_file::HashFile;
use crate::HashType;
use cancellation::CancellationToken;
use regex::Regex;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use strum::IntoEnumIterator;

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
    NoFilesProcessed,
    Canceled,
}

#[derive(Debug, PartialEq)]
pub enum FileProcessState {
    Extra,
    InvalidUnicodeFileName,
    Missing,
    IncorrectSize,
    IncorrectHash,
    Error(String),
}

pub struct FileProcessEntry {
    pub file_path: PathBuf,
    pub state: FileProcessState,
}

pub struct ProcessProgress {
    pub file_path: PathBuf,
    pub file_size: u64,
    pub bytes_processed: usize,
}

#[derive(Default)]
pub struct HashFileProcessOptions<'a> {
    pub base_path: PathBuf,
    pub hash_type: Option<HashType>,
    pub force_create: Option<bool>,
    pub report_extra_files: Option<bool>,
    pub check_file_size_only: Option<bool>,
    pub match_pattern: Option<&'a str>,
    pub ignore_pattern: Option<&'a str>,
}

pub struct HashFileProcessor<'a> {
    hash_file: HashFile,
    hash_type: HashType,
    process_type: HashFileProcessType,
    hash_file_path: PathBuf,
    bin_file_name: PathBuf,
    base_path: PathBuf,
    check_file_size_only: bool,
    report_extra_files: bool,
    match_regex: Option<Regex>,
    ignore_regex: Option<Regex>,
    error_occurred: bool,
    files_processed: bool,
    bytes_processed_notification_block_size: usize,
    cancellation_token: Option<&'a CancellationToken>,
    progress_event: Option<Box<dyn Fn(ProcessProgress) + Send + Sync + 'a>>,
    warning_event: Option<Box<dyn Fn(FileProcessEntry) + Send + Sync + 'a>>,
    error_event: Option<Box<dyn Fn(FileProcessEntry) + Send + Sync + 'a>>,
    complete_event: Option<Box<dyn Fn(HashFileProcessResult) + Send + Sync + 'a>>,
}

const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: usize = 2_097_152;

impl<'a> HashFileProcessor<'a> {
    pub fn new_with_options(options: HashFileProcessOptions) -> Self {
        let mut process_type = HashFileProcessType::Create;
        let mut actual_hash_type = options.hash_type.unwrap_or(HashType::SHA1);
        let cano_base_path = fs::canonicalize(options.base_path).unwrap();
        if !options.force_create.unwrap_or_default() {
            if let Some(existing_hash_type) =
                get_existing_file_hash_type(&cano_base_path, actual_hash_type)
            {
                actual_hash_type = existing_hash_type;
                process_type = HashFileProcessType::Verify;
            }
        }

        let hash_file_name = get_hash_file_name(actual_hash_type);
        let hash_file_path = cano_base_path.join(hash_file_name);
        let bin_path = env::current_exe().unwrap();
        let mut bin_file_name = PathBuf::from(bin_path.file_name().unwrap());
        let mut work_path = env::current_dir().unwrap();
        work_path.push(bin_file_name.clone());
        if !work_path.is_file() {
            // the app binary is not in the target root. ignore skip logic.
            bin_file_name = PathBuf::new();
        }

        HashFileProcessor {
            hash_file: HashFile::new(),
            hash_type: actual_hash_type,
            process_type,
            hash_file_path,
            bin_file_name,
            base_path: cano_base_path,
            check_file_size_only: options.check_file_size_only.unwrap_or_default(),
            report_extra_files: options.report_extra_files.unwrap_or_default(),
            match_regex: options.match_pattern.map(|s| Regex::new(s).unwrap()),
            ignore_regex: options.ignore_pattern.map(|s| Regex::new(s).unwrap()),
            error_occurred: false,
            files_processed: false,
            bytes_processed_notification_block_size:
                DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
            cancellation_token: None,
            progress_event: None,
            error_event: None,
            warning_event: None,
            complete_event: None,
        }
    }
    pub fn new(base_path: &Path, hash_type: HashType, force_create: bool) -> Self {
        HashFileProcessor::new_with_options(HashFileProcessOptions {
            base_path: base_path.to_path_buf(),
            hash_type: Some(hash_type),
            force_create: Some(force_create),
            ..Default::default()
        })
    }
    pub fn handle_error(&mut self, file_path: &Path, error_state: FileProcessState) {
        self.error_occurred = true;
        self.files_processed = true;
        if let Some(handler) = &self.error_event {
            handler(FileProcessEntry {
                file_path: file_path.to_path_buf(),
                state: error_state,
            });
        }
    }
    pub fn handle_warning(&mut self, file_path: &Path, error_state: FileProcessState) {
        if let Some(handler) = &self.error_event {
            handler(FileProcessEntry {
                file_path: file_path.to_path_buf(),
                state: error_state,
            });
        }
    }
    pub fn process(&mut self) -> HashFileProcessResult {
        self.process_with_cancellation_token(CancellationToken::none())
    }
    pub fn process_with_cancellation_token(
        &mut self,
        cancellation_token: &'a CancellationToken,
    ) -> HashFileProcessResult {
        let result = self.process_internal(cancellation_token);
        if let Some(handler) = &self.complete_event {
            handler(result);
        }

        result
    }
    pub fn process_internal(
        &mut self,
        cancellation_token: &'a CancellationToken,
    ) -> HashFileProcessResult {
        self.cancellation_token = Some(cancellation_token);

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

        if self.error_occurred {
            return HashFileProcessResult::Error;
        } else if self.process_type == HashFileProcessType::Create {
            if self.hash_file.is_empty() {
                return HashFileProcessResult::Error;
            }

            self.hash_file.save(&self.hash_file_path);
        } else if self.process_type == HashFileProcessType::Verify && !self.hash_file.is_empty() {
            for file_path in self.hash_file.get_file_paths() {
                if let Some(regex) = &self.match_regex {
                    if !regex.is_match(&file_path) {
                        continue;
                    }
                }

                if let Some(regex) = &self.ignore_regex {
                    if regex.is_match(&file_path) {
                        continue;
                    }
                }

                self.handle_error(Path::new(&file_path), FileProcessState::Missing);
            }

            if self.error_occurred {
                return HashFileProcessResult::Error;
            }
        }

        if self.files_processed {
            HashFileProcessResult::Success
        } else {
            HashFileProcessResult::NoFilesProcessed
        }
    }
    pub fn set_progress_event_handler(
        &mut self,
        handler: Box<dyn Fn(ProcessProgress) + Send + Sync + 'a>,
    ) {
        self.set_progress_event_handler_with_bytes_processed_notification_block_size(
            handler,
            DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
        )
    }
    pub fn set_progress_event_handler_with_bytes_processed_notification_block_size(
        &mut self,
        handler: Box<dyn Fn(ProcessProgress) + Send + Sync + 'a>,
        bytes_processed_notification_block_size: usize,
    ) {
        self.progress_event = Some(handler);
        self.bytes_processed_notification_block_size = bytes_processed_notification_block_size;
    }
    pub fn set_warning_event_handler(
        &mut self,
        handler: Box<dyn Fn(FileProcessEntry) + Send + Sync + 'a>,
    ) {
        self.warning_event = Some(handler);
    }
    pub fn set_error_event_handler(
        &mut self,
        handler: Box<dyn Fn(FileProcessEntry) + Send + Sync + 'a>,
    ) {
        self.error_event = Some(handler);
    }
    pub fn set_complete_event_handler(
        &mut self,
        handler: Box<dyn Fn(HashFileProcessResult) + Send + Sync + 'a>,
    ) {
        self.complete_event = Some(handler);
    }
    pub fn get_process_type(&self) -> HashFileProcessType {
        self.process_type
    }
}

impl<'a> FileTreeProcessor for HashFileProcessor<'a> {
    fn process_file(&mut self, file_path: &Path) {
        if file_path == self.hash_file_path {
            return; // skip current hash file
        }

        let file_path_str = match file_path.to_str() {
            Some(file_path_str) => file_path_str,
            None => {
                self.handle_warning(file_path, FileProcessState::InvalidUnicodeFileName);
                return;
            }
        };

        if let Some(regex) = &self.match_regex {
            if !regex.is_match(file_path_str) {
                return;
            }
        }

        if let Some(regex) = &self.ignore_regex {
            if regex.is_match(file_path_str) {
                return;
            }
        }

        let relative_file_path = file_path.strip_prefix(&self.base_path).unwrap();
        let relative_file_path_str = relative_file_path.to_str().unwrap();
        let file_size: u64;
        match file_path.metadata() {
            Ok(metadata) => file_size = metadata.len(),
            Err(error) => {
                self.handle_error(
                    relative_file_path,
                    FileProcessState::Error(error.to_string()),
                );
                return;
            }
        }

        let hash_file_entry = self.hash_file.get_entry(relative_file_path_str);
        if let Some(file_entry) = hash_file_entry {
            if file_size != file_entry.size {
                self.handle_error(relative_file_path, FileProcessState::IncorrectSize);
                return;
            }
        } else if relative_file_path == self.bin_file_name {
            return; // skip app binary file
        } else if self.process_type == HashFileProcessType::Verify {
            if self.report_extra_files {
                self.handle_warning(relative_file_path, FileProcessState::Extra);
            }
            return;
        }

        let mut digest = String::from("");
        if !(self.check_file_size_only && self.process_type == HashFileProcessType::Verify) {
            let mut file_hasher = crate::get_file_hasher(self.hash_type, file_path);
            if let Some(handler) = &self.progress_event {
                handler(ProcessProgress {
                    file_path: relative_file_path.to_path_buf(),
                    file_size,
                    bytes_processed: 0,
                });
                file_hasher.set_bytes_processed_event_handler(Box::new(move |args| {
                    handler(ProcessProgress {
                        file_path: relative_file_path.to_path_buf(),
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

        if self.process_type == HashFileProcessType::Create {
            self.hash_file
                .add_entry(relative_file_path_str, file_size, &digest);
        } else if self.process_type == HashFileProcessType::Verify {
            if let Some(file_entry) = hash_file_entry {
                if !self.check_file_size_only && digest != file_entry.digest {
                    self.handle_error(relative_file_path, FileProcessState::IncorrectHash);
                }
            }

            self.hash_file.remove_entry(relative_file_path_str);
        }

        self.files_processed = true;
    }
}

fn get_hash_file_name(hash_type: HashType) -> PathBuf {
    let hash_type_str: &str = hash_type.into();
    let hash_file = Path::new(HASH_FILE_BASE_NAME);
    hash_file.with_extension(hash_type_str.to_lowercase())
}

fn get_existing_file_hash_type(base_path: &Path, desired_hash_type: HashType) -> Option<HashType> {
    let mut hash_file_path = PathBuf::from(base_path);
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
