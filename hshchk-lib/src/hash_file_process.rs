use crate::block_hasher::HashProgress;
use crate::file_tree::{FileTree, FileTreeProcessor};
use crate::hash_file::{HashFile, HashFileEntry};
use crate::{HashFileFormat, HashType};
use cancellation::{CancellationToken, CancellationTokenSource};
use crossbeam::crossbeam_channel::{select, unbounded, Sender};
use regex::Regex;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use strum::IntoEnumIterator;

static HASHCHECK_BASE_FILE_NAME: &str = "hshchk";
static HASHSUM_SUFFIX: &str = "SUMS";
const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: usize = 2_097_152;

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

#[derive(Debug, Clone, PartialEq)]
pub enum FileProcessState {
    Extra,
    InvalidUnicodeFileName,
    Missing,
    IncorrectSize,
    IncorrectHash,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileProcessEntry {
    pub file_path: PathBuf,
    pub state: FileProcessState,
}

#[derive(Default)]
pub struct FileProgress {
    pub file_path: String,
    pub file_size: u64,
    pub bytes_processed: u64,
}

#[derive(Default)]
pub struct HashFileProcessOptions<'a> {
    pub base_path: PathBuf,
    pub hash_file_format: Option<HashFileFormat>,
    pub hash_type: Option<HashType>,
    pub force_create: Option<bool>,
    pub report_extra: Option<bool>,
    pub size_only: Option<bool>,
    pub match_pattern: Option<&'a str>,
    pub ignore_pattern: Option<&'a str>,
}

pub struct HashFileProcessor {
    hash_file: HashFile,
    hash_type: HashType,
    hash_file_format: Option<HashFileFormat>,
    process_type: HashFileProcessType,
    hash_file_path: PathBuf,
    bin_file_name: PathBuf,
    base_path: PathBuf,
    size_only: bool,
    report_extra: bool,
    match_regex: Option<Regex>,
    ignore_regex: Option<Regex>,
    error_occurred: bool,
    files_processed: bool,
    bytes_processed_notification_block_size: usize,
    cancellation_token: Option<Arc<CancellationToken>>,
    internal_hash_progress_sender: Option<Sender<HashProgress>>,
    internal_progress_sender: Option<Sender<FileProgress>>,
    progress_event: Option<Sender<FileProgress>>,
    warning_event: Option<Sender<FileProcessEntry>>,
    error_event: Option<Sender<FileProcessEntry>>,
    complete_event: Option<Sender<HashFileProcessResult>>,
}

impl HashFileProcessor {
    pub fn new(options: HashFileProcessOptions) -> Self {
        let mut process_type = HashFileProcessType::Create;
        let mut hash_type = options.hash_type.unwrap_or(HashType::SHA1);
        let mut hash_file_format = options.hash_file_format;
        let cano_base_path = fs::canonicalize(options.base_path).unwrap();
        if !options.force_create.unwrap_or_default() {
            if let Some((existing_hash_type, existing_hash_file_format)) =
                get_existing_file_hash_type(&cano_base_path, hash_type)
            {
                hash_type = existing_hash_type;
                hash_file_format = Some(existing_hash_file_format);
                process_type = HashFileProcessType::Verify;
            }
        }

        let hash_file_name = match hash_file_format {
            Some(HashFileFormat::HashSum) => get_hashsum_file_name(hash_type),
            _ => get_hashcheck_file_name(hash_type),
        };

        let hash_file_path = cano_base_path.join(hash_file_name);
        let bin_path = env::current_exe().unwrap();
        let mut bin_file_name = PathBuf::from(bin_path.file_name().unwrap());
        let mut work_path = env::current_dir().unwrap();
        work_path.push(bin_file_name.clone());
        if !work_path.is_file() {
            // The app binary is not in the target root. Ignore skip logic.
            bin_file_name = PathBuf::new();
        }

        HashFileProcessor {
            hash_file: HashFile::new(),
            hash_type,
            hash_file_format,
            process_type,
            hash_file_path,
            bin_file_name,
            base_path: cano_base_path,
            size_only: options.size_only.unwrap_or_default(),
            report_extra: options.report_extra.unwrap_or_default(),
            match_regex: options.match_pattern.map(|s| Regex::new(s).unwrap()),
            ignore_regex: options.ignore_pattern.map(|s| Regex::new(s).unwrap()),
            error_occurred: false,
            files_processed: false,
            bytes_processed_notification_block_size:
                DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
            cancellation_token: None,
            internal_hash_progress_sender: None,
            internal_progress_sender: None,
            progress_event: None,
            error_event: None,
            warning_event: None,
            complete_event: None,
        }
    }
    pub fn handle_error(&mut self, file_path: &Path, error_state: FileProcessState) {
        self.error_occurred = true;
        self.files_processed = true;
        if let Some(sender) = &self.error_event {
            sender
                .send(FileProcessEntry {
                    file_path: file_path.to_path_buf(),
                    state: error_state,
                })
                .unwrap();
        }
    }
    pub fn handle_warning(&mut self, file_path: &Path, warning_state: FileProcessState) {
        if let Some(sender) = &self.warning_event {
            sender
                .send(FileProcessEntry {
                    file_path: file_path.to_path_buf(),
                    state: warning_state,
                })
                .unwrap();
        }
    }
    pub fn process(&mut self) -> HashFileProcessResult {
        let cts = CancellationTokenSource::new();
        let cancellation_token = cts.token();
        self.process_with_cancellation_token(cancellation_token.clone())
    }
    pub fn process_with_cancellation_token(
        &mut self,
        cancellation_token: Arc<CancellationToken>,
    ) -> HashFileProcessResult {
        let result = self.process_internal(cancellation_token);
        if let Some(sender) = &self.complete_event {
            sender.send(result).unwrap();
        }

        result
    }
    pub fn process_internal(
        &mut self,
        cancellation_token: Arc<CancellationToken>,
    ) -> HashFileProcessResult {
        self.cancellation_token = Some(cancellation_token.clone());

        if self.process_type == HashFileProcessType::Verify {
            self.hash_file.load(&self.hash_file_path);
        }

        let mut progress_thread: Option<std::thread::JoinHandle<()>> = None;

        if let Some(progress_sender) = &self.progress_event {
            let (internal_hash_progress_sender, internal_hash_progress_receiver) = unbounded();
            self.internal_hash_progress_sender = Some(internal_hash_progress_sender);
            let (internal_progress_sender, internal_progress_receiver) = unbounded();
            self.internal_progress_sender = Some(internal_progress_sender);
            let proxy_progress_sender = progress_sender.clone();
            progress_thread = Some(std::thread::spawn(move || {
                let mut current_file_path = String::default();
                let mut current_file_size = 0u64;
                loop {
                    select! {
                        recv(internal_progress_receiver) -> msg => {
                            if let Ok(progress) = msg {
                                current_file_path = progress.file_path;
                                current_file_size = progress.file_size;
                                proxy_progress_sender.send(FileProgress {
                                    file_path: current_file_path.clone(),
                                    file_size: current_file_size,
                                    bytes_processed: progress.bytes_processed,
                                }).unwrap()
                            }
                            else {
                                break;
                            }
                        },
                        recv(internal_hash_progress_receiver) -> msg => {
                            if let Ok(progress) = msg {
                                proxy_progress_sender.send(FileProgress {
                                    file_path: current_file_path.clone(),
                                    file_size: current_file_size,
                                    bytes_processed: progress.bytes_processed,
                                }).unwrap()
                            }
                        },
                    }
                }
            }));
        }

        let path = self.base_path.clone();
        let mut file_tree = FileTree::new(self);

        if let Err(why) = file_tree.traverse(&path, &cancellation_token) {
            panic!("Couldn't traverse {}: {}.", path.display(), why);
        }

        if let Some(progress_sender) = &self.internal_progress_sender.take() {
            drop(progress_sender);
        }

        if let Some(thread_handle) = progress_thread {
            thread_handle.join().unwrap();
        }

        if cancellation_token.is_canceled() {
            return HashFileProcessResult::Canceled;
        }

        if self.error_occurred {
            return HashFileProcessResult::Error;
        } else if self.process_type == HashFileProcessType::Create {
            if self.hash_file.is_empty() {
                return HashFileProcessResult::NoFilesProcessed;
            }

            self.hash_file.save(
                &self.hash_file_path,
                self.hash_file_format.unwrap_or(HashFileFormat::HashCheck),
            );
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
    pub fn set_progress_event_sender(&mut self, sender: Sender<FileProgress>) {
        self.set_progress_event_sender_with_bytes_processed_notification_block_size(
            sender,
            DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
        )
    }
    pub fn set_progress_event_sender_with_bytes_processed_notification_block_size(
        &mut self,
        sender: Sender<FileProgress>,
        bytes_processed_notification_block_size: usize,
    ) {
        self.progress_event = Some(sender);
        self.bytes_processed_notification_block_size = bytes_processed_notification_block_size;
    }
    pub fn set_warning_event_sender(&mut self, sender: Sender<FileProcessEntry>) {
        self.warning_event = Some(sender);
    }
    pub fn set_error_event_sender(&mut self, sender: Sender<FileProcessEntry>) {
        self.error_event = Some(sender);
    }
    pub fn set_complete_event_sender(&mut self, sender: Sender<HashFileProcessResult>) {
        self.complete_event = Some(sender);
    }
    pub fn get_process_type(&self) -> HashFileProcessType {
        self.process_type
    }
}

impl FileTreeProcessor for HashFileProcessor {
    fn process_file(&mut self, file_path: &Path) {
        if file_path == self.hash_file_path {
            return; // Skip current hash file
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
            if let Some(file_entry_size) = file_entry.size {
                if file_size != file_entry_size {
                    self.handle_error(relative_file_path, FileProcessState::IncorrectSize);
                    return;
                }
            }
        } else if relative_file_path == self.bin_file_name {
            return; // Skip app binary file
        } else if self.process_type == HashFileProcessType::Verify {
            if self.report_extra {
                self.handle_warning(relative_file_path, FileProcessState::Extra);
            }

            return;
        }

        let mut digest = String::from("");
        if !(self.size_only && self.process_type == HashFileProcessType::Verify) {
            let mut file_hasher = crate::get_file_hasher(self.hash_type, file_path);
            if let Some(progress_sender) = &self.internal_progress_sender {
                progress_sender
                    .send(FileProgress {
                        file_path: relative_file_path.to_string_lossy().into_owned(),
                        file_size,
                        bytes_processed: 0,
                    })
                    .unwrap();
                if let Some(hash_progress_sender) = &self.internal_hash_progress_sender {
                    file_hasher.set_bytes_processed_event_sender(hash_progress_sender.clone());
                }
            }

            let cancellation_token = self.cancellation_token.as_ref().unwrap();
            file_hasher.compute(cancellation_token.clone());
            digest = file_hasher.digest();

            if let Some(progress_sender) = &self.internal_progress_sender {
                progress_sender
                    .send(FileProgress {
                        file_path: relative_file_path.to_string_lossy().into_owned(),
                        file_size,
                        bytes_processed: file_size,
                    })
                    .unwrap();
            }

            if cancellation_token.is_canceled() {
                return;
            }
        }

        if self.process_type == HashFileProcessType::Create {
            self.hash_file.add_entry(HashFileEntry {
                file_path: relative_file_path_str.to_string(),
                size: Some(file_size),
                binary: true,
                digest,
            });
        } else if self.process_type == HashFileProcessType::Verify {
            if let Some(file_entry) = hash_file_entry {
                if !self.size_only && digest != file_entry.digest {
                    self.handle_error(relative_file_path, FileProcessState::IncorrectHash);
                }
            }

            self.hash_file.remove_entry(relative_file_path_str);
        }

        self.files_processed = true;
    }
}

fn get_hashcheck_file_name(hash_type: HashType) -> PathBuf {
    let hash_type_str: &str = hash_type.into();
    let hash_file = Path::new(HASHCHECK_BASE_FILE_NAME);
    hash_file.with_extension(hash_type_str.to_lowercase())
}

fn get_hashsum_file_name(hash_type: HashType) -> PathBuf {
    let hash_type_str: &str = hash_type.into();
    let hash_file_name = hash_type_str.to_uppercase() + HASHSUM_SUFFIX.into();
    let hash_file = Path::new(&hash_file_name);
    hash_file.to_path_buf()
}

fn hash_file_exists(hash_file_path: &mut PathBuf, hash_type: HashType) -> Option<HashFileFormat> {
    hash_file_path.push(get_hashcheck_file_name(hash_type));
    if hash_file_path.is_file() {
        return Some(HashFileFormat::HashCheck);
    }

    hash_file_path.pop();
    hash_file_path.push(get_hashsum_file_name(hash_type));
    if hash_file_path.is_file() {
        return Some(HashFileFormat::HashSum);
    }

    None
}

fn get_existing_file_hash_type(
    base_path: &Path,
    desired_hash_type: HashType,
) -> Option<(HashType, HashFileFormat)> {
    let mut hash_file_path = PathBuf::from(base_path);

    if let Some(hash_file_format) = hash_file_exists(&mut hash_file_path, desired_hash_type) {
        return Some((desired_hash_type, hash_file_format));
    } else {
        hash_file_path.pop();
        for hash_type in HashType::iter() {
            if let Some(hash_file_format) = hash_file_exists(&mut hash_file_path, hash_type) {
                return Some((hash_type, hash_file_format));
            }

            hash_file_path.pop();
        }
    }

    None
}
