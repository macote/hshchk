use crate::block_hasher::HashProgress;
use crate::file_tree::{FileTree, FileTreeProcessor};
use crate::hash_file::{HashFile, HashFileEntry};
use crate::{HashFileFormat, HashType};
use cancellation::{CancellationToken, CancellationTokenSource};
use crossbeam::channel::{select, unbounded, Sender};
use regex::Regex;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use strum::IntoEnumIterator;

static HASHCHECK_BASE_FILE_NAME: &str = "hshchk";
static HASHSUM_SUFFIX: &str = "SUMS";
const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: usize = 2_097_152;

use std::collections::HashSet;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum HashFileProcessType {
    Create,
    Verify,
    Update,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct ProcessStats {
    pub total_files_processed: u64, // Files touched/considered on disk
    pub total_bytes_processed: u64, // Bytes of files that were actually hashed or size-checked
    pub total_processing_time_ms: u128,
    // Create specific
    // (total_files_processed can be # files added to manifest, total_bytes_processed their size)
    // Verify specific
    pub files_checked_in_verify: u64, // Files in manifest that were found on disk and checked
    pub errors_in_verify: u64,        // Hash/size mismatches
    pub missing_files_in_verify: u64,  // Files in manifest not found on disk
    pub extra_files_in_verify: u64,    // Files on disk not in manifest (if reported)
    // Update specific
    pub files_added_in_update: u64,    // New files added to manifest
    pub files_updated_in_update: u64,  // Existing manifest entries re-hashed and updated
    pub files_removed_in_update: u64,  // Files removed from manifest (due to --remove-missing)
    pub files_unchanged_in_update: u64,// Files found on disk, matched manifest, and not re-hashed
}

#[derive(Debug, Clone, PartialEq)]
pub enum HashFileProcessResult {
    CreateSuccess(ProcessStats),
    VerifySuccess(ProcessStats),
    UpdateSuccess(ProcessStats),
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
    // New fields for update mode, from CLI
    pub update_mode: bool,
    pub remove_missing_in_update: bool,
    pub force_rehash_in_update: bool,
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
    files_processed: bool, // General flag, might be redundant with ProcessStats
    process_stats: ProcessStats,
    start_time: Option<std::time::Instant>,
    bytes_processed_notification_block_size: usize,
    cancellation_token: Option<Arc<CancellationToken>>,
    // Store update-specific options from HashFileProcessOptions
    update_mode: bool,
    remove_missing_in_update: bool,
    force_rehash_in_update: bool,
    // Keep track of paths from the loaded hash file for update mode's missing file detection
    paths_in_loaded_hash_file: HashSet<String>,
    internal_hash_progress_sender: Option<Sender<HashProgress>>,
    internal_progress_sender: Option<Sender<FileProgress>>,
    progress_event: Option<Sender<FileProgress>>,
    warning_event: Option<Sender<FileProcessEntry>>,
    error_event: Option<Sender<FileProcessEntry>>,
    complete_event: Option<Sender<HashFileProcessResult>>,
}

impl HashFileProcessor {
    pub fn new(options: HashFileProcessOptions) -> Self {
        let mut determined_process_type = HashFileProcessType::Create;
        let mut hash_type = options.hash_type.unwrap_or(HashType::SHA1);
        let mut current_hash_file_format = options.hash_file_format; // Can be None initially
        let cano_base_path = fs::canonicalize(options.base_path.clone())
            .unwrap_or_else(|_| options.base_path.clone()); // Use original path if canonicalization fails

        let existing_hash_info = get_existing_file_hash_type(&cano_base_path, hash_type);

        if options.update_mode {
            if let Some((existing_type, existing_format)) = existing_hash_info {
                determined_process_type = HashFileProcessType::Update;
                hash_type = existing_type; // Override with type from existing file for update
                current_hash_file_format = Some(existing_format); // Override with format from existing file
            } else {
                // No checksum file found to update. Switch to Create mode.
                // This could also be an error condition if strict update behavior is desired.
                determined_process_type = HashFileProcessType::Create;
                // A warning could be logged here: "Update mode specified, but no checksum file found. Creating a new one."
            }
        } else if !options.force_create.unwrap_or_default() {
            if let Some((existing_type, existing_format)) = existing_hash_info {
                determined_process_type = HashFileProcessType::Verify;
                hash_type = existing_type;
                current_hash_file_format = Some(existing_format);
            }
        }
        // If options.force_create is true (and not update_mode), determined_process_type remains Create.

        // Determine the final hash file format to be used for saving or naming,
        // especially if one wasn't loaded or if creating new.
        let final_hash_file_format = current_hash_file_format.unwrap_or(
            if options.hash_file_format == Some(HashFileFormat::HashSum) { // User explicitly requested HashSum
                HashFileFormat::HashSum
            } else { // Default to HashCheck for new files
                HashFileFormat::HashCheck
            }
        );

        let hash_file_name = match final_hash_file_format {
            HashFileFormat::HashSum => get_hashsum_file_name(hash_type),
            HashFileFormat::HashCheck => get_hashcheck_file_name(hash_type),
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
            hash_type, // Now correctly reflects existing file's type in Verify/Update modes
            hash_file_format: Some(final_hash_file_format), // Ensure this is Some for the processor
            process_type: determined_process_type,
            hash_file_path,
            bin_file_name,
            base_path: cano_base_path,
            size_only: options.size_only.unwrap_or_default(),
            report_extra: options.report_extra.unwrap_or_default(),
            match_regex: options.match_pattern.map(|s| Regex::new(s).unwrap()),
            ignore_regex: options.ignore_pattern.map(|s| Regex::new(s).unwrap()),
            error_occurred: false,
            files_processed: false, // This flag might become redundant due to ProcessStats
            process_stats: ProcessStats::default(),
            start_time: None,
            bytes_processed_notification_block_size:
                DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
            cancellation_token: None,
            update_mode: options.update_mode, // Store the CLI option
            remove_missing_in_update: options.remove_missing_in_update,
            force_rehash_in_update: options.force_rehash_in_update,
            paths_in_loaded_hash_file: HashSet::new(), // For update mode
            internal_hash_progress_sender: None,
            internal_progress_sender: None,
            progress_event: None,
            error_event: None,
            warning_event: None,
            complete_event: None,
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
    fn handle_error(&mut self, file_path: &Path, error_state: FileProcessState) {
        self.error_occurred = true;
        // self.files_processed = true; // files_processed is less critical with ProcessStats
        match self.process_type {
            HashFileProcessType::Verify | HashFileProcessType::Update => {
                match error_state {
                    FileProcessState::Missing => { self.process_stats.missing_files_in_verify += 1; }
                    FileProcessState::IncorrectSize | FileProcessState::IncorrectHash => {
                        self.process_stats.errors_in_verify += 1;
                    }
                    _ => {} // Other errors are just general errors
                }
            }
            HashFileProcessType::Create => { /* General error */ }
        }
        if let Some(sender) = &self.error_event {
            sender
                .send(FileProcessEntry {
                    file_path: file_path.to_path_buf(),
                    state: error_state,
                })
                .unwrap();
        }
    }
    fn handle_warning(&mut self, file_path: &Path, warning_state: FileProcessState) {
        match self.process_type {
            HashFileProcessType::Verify | HashFileProcessType::Update => {
                if warning_state == FileProcessState::Extra {
                    self.process_stats.extra_files_in_verify += 1;
                }
                // If warning_state is Missing (used in Update for missing but kept files)
                // it's handled in process_internal or if it needs specific stat for ProcessStats.
            }
            HashFileProcessType::Create => {}
        }
        if let Some(sender) = &self.warning_event {
            sender
                .send(FileProcessEntry {
                    file_path: file_path.to_path_buf(),
                    state: warning_state,
                })
                .unwrap();
        }
    }
    fn process_internal(
        &mut self,
        cancellation_token: Arc<CancellationToken>,
    ) -> HashFileProcessResult {
        self.cancellation_token = Some(cancellation_token.clone());
        self.start_time = Some(std::time::Instant::now());
        self.process_stats = ProcessStats::default(); // Reset for current operation

        if self.process_type == HashFileProcessType::Verify || self.process_type == HashFileProcessType::Update {
            // Load existing hash file. If it fails (e.g. not found, though new() checks), it will panic.
            // This is acceptable for now, as new() logic tries to determine mode based on existence.
            self.hash_file.load(&self.hash_file_path);
            if self.process_type == HashFileProcessType::Update {
                // Populate paths_in_loaded_hash_file for tracking
                for path_str in self.hash_file.get_file_paths() {
                    self.paths_in_loaded_hash_file.insert(path_str);
                }
            }
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

        if let Some(progress_sender) = self.internal_progress_sender.take() {
            drop(progress_sender);
        }

        if let Some(thread_handle) = progress_thread {
            thread_handle.join().unwrap();
        }

        if cancellation_token.is_canceled() { return HashFileProcessResult::Canceled; }

        if let Some(start_time) = self.start_time {
            self.process_stats.total_processing_time_ms = start_time.elapsed().as_millis();
        }

        if self.process_type == HashFileProcessType::Update {
            let mut missing_paths_to_remove_from_manifest: Vec<String> = Vec::new();
            for path_in_manifest in &self.paths_in_loaded_hash_file {
                // If it's still in paths_in_loaded_hash_file, it means process_file didn't see it on disk.
                if self.remove_missing_in_update {
                    missing_paths_to_remove_from_manifest.push(path_in_manifest.clone());
                } else {
                    self.process_stats.missing_files_in_verify += 1; // Count as conventionally missing
                    self.handle_warning(Path::new(path_in_manifest), FileProcessState::Missing); // Log as warning
                }
            }
            for path_str in &missing_paths_to_remove_from_manifest {
                self.hash_file.remove_entry(path_str); // Actually remove from in-memory hash_file
                self.process_stats.files_removed_in_update += 1;
            }
        } else if self.process_type == HashFileProcessType::Verify {
            // For Verify, any file path string still in hash_file at this point is a missing file.
            // (process_file removes found files from hash_file in Verify mode).
            for file_path_str in self.hash_file.get_file_paths() {
                if let Some(regex) = &self.match_regex { if !regex.is_match(&file_path_str) { continue; } }
                if let Some(regex) = &self.ignore_regex { if regex.is_match(&file_path_str) { continue; } }
                self.handle_error(Path::new(&file_path_str), FileProcessState::Missing);
            }
        }

        if self.error_occurred { return HashFileProcessResult::Error; }

        match self.process_type {
            HashFileProcessType::Create => {
                if self.process_stats.total_files_processed == 0 { return HashFileProcessResult::NoFilesProcessed; }
                self.hash_file.save(&self.hash_file_path, self.hash_file_format.unwrap(), false);
                return HashFileProcessResult::CreateSuccess(self.process_stats.clone());
            }
            HashFileProcessType::Verify => {
                 if self.process_stats.files_checked_in_verify == 0 &&
                    self.process_stats.missing_files_in_verify == 0 &&
                    self.process_stats.extra_files_in_verify == 0 {
                     return HashFileProcessResult::NoFilesProcessed;
                 }
                return HashFileProcessResult::VerifySuccess(self.process_stats.clone());
            }
            HashFileProcessType::Update => {
                let stats = &self.process_stats;
                if stats.files_added_in_update == 0 && stats.files_updated_in_update == 0 &&
                   stats.files_removed_in_update == 0 && stats.files_unchanged_in_update == 0 &&
                   self.paths_in_loaded_hash_file.is_empty() { // Check if original manifest was also empty
                     // This condition means no files on disk and original manifest was empty, or all files filtered out.
                    if self.hash_file.is_empty() { return HashFileProcessResult::NoFilesProcessed; }
                }
                self.hash_file.save(&self.hash_file_path, self.hash_file_format.unwrap(), true); // Atomic save
                return HashFileProcessResult::UpdateSuccess(self.process_stats.clone());
            }
        }
    }
}

impl FileTreeProcessor for HashFileProcessor {
    fn process_file(&mut self, file_on_disk_path: &Path) {
        if file_on_disk_path == self.hash_file_path { return; }

        let file_on_disk_path_str = match file_on_disk_path.to_str() {
            Some(s) => s,
            None => { self.handle_warning(file_on_disk_path, FileProcessState::InvalidUnicodeFileName); return; }
        };

        if let Some(regex) = &self.match_regex { if !regex.is_match(file_on_disk_path_str) { return; } }
        if let Some(regex) = &self.ignore_regex { if regex.is_match(file_on_disk_path_str) { return; } }

        let relative_file_path = file_on_disk_path.strip_prefix(&self.base_path).unwrap();
        let relative_file_path_str = relative_file_path.to_str().unwrap().to_string(); // Owned for HashSet ops

        let disk_file_metadata = match fs::metadata(file_on_disk_path) {
            Ok(md) => md,
            Err(e) => { self.handle_error(relative_file_path, FileProcessState::Error(e.to_string())); return; }
        };
        let disk_file_size = disk_file_metadata.len();

        // Regardless of mode, if it's the app binary itself, skip.
        if relative_file_path == self.bin_file_name { return; }

        self.process_stats.total_files_processed += 1; // Count any file encountered on disk (not filtered out)

        let existing_manifest_entry_opt = self.hash_file.get_entry(&relative_file_path_str).cloned();


        match self.process_type {
            HashFileProcessType::Create => {
                // Hashing will occur below, stats updated there.
            }
            HashFileProcessType::Verify => {
                if let Some(entry) = &existing_manifest_entry_opt {
                    self.process_stats.files_checked_in_verify += 1;
                    self.process_stats.total_bytes_processed += disk_file_size; // Bytes "checked"
                    if let Some(entry_size) = entry.size {
                        if disk_file_size != entry_size {
                            self.handle_error(relative_file_path, FileProcessState::IncorrectSize);
                            self.hash_file.remove_entry(&relative_file_path_str); return;
                        }
                    }
                    if self.size_only { self.hash_file.remove_entry(&relative_file_path_str); return; }
                    // If not size_only, proceed to hashing block
                } else { // File on disk, not in manifest
                    if self.report_extra { self.handle_warning(relative_file_path, FileProcessState::Extra); }
                    return; // Do not hash extra files in verify mode
                }
            }
            HashFileProcessType::Update => {
                self.paths_in_loaded_hash_file.remove(&relative_file_path_str); // Mark as seen
                if let Some(entry) = &existing_manifest_entry_opt { // File is in current manifest
                    self.process_stats.files_checked_in_verify += 1; // "checked" against old manifest
                    let mut needs_rehash = self.force_rehash_in_update;
                    if let Some(entry_size) = entry.size {
                        if disk_file_size != entry_size { needs_rehash = true; }
                    } else { needs_rehash = true; } // No size in manifest, rehash

                    if !needs_rehash {
                        self.process_stats.files_unchanged_in_update += 1;
                        self.process_stats.total_bytes_processed += disk_file_size; // "processed" by checking
                        return; // Unchanged, no hashing needed
                    }
                    // If needs_rehash, fall through to hashing block
                } else { // File on disk, not in manifest => new file
                    // Fall through to hashing block, will be added
                }
            }
        }

        // Hashing logic (Create, Verify not size_only, Update new/changed)
        let should_hash = match self.process_type {
            HashFileProcessType::Create => true,
            HashFileProcessType::Verify => !self.size_only && existing_manifest_entry_opt.is_some(),
            HashFileProcessType::Update => true, // Decision to hash or not was made above for Update
        };

        if !should_hash { return; }

        let mut file_hasher = crate::get_file_hasher(self.hash_type, file_on_disk_path);
        if let Some(ref progress) = self.internal_progress_sender {
            progress.send(FileProgress { file_path: relative_file_path_str.clone(), file_size: disk_file_size, bytes_processed: 0 }).unwrap();
            if let Some(ref hash_progress) = self.internal_hash_progress_sender {
                file_hasher.set_bytes_processed_event_sender(hash_progress.clone());
            }
        }
        file_hasher.compute(self.cancellation_token.as_ref().unwrap().clone());
        let digest = file_hasher.digest();
        if let Some(ref progress) = self.internal_progress_sender {
            progress.send(FileProgress { file_path: relative_file_path_str.clone(), file_size: disk_file_size, bytes_processed: disk_file_size }).unwrap();
        }
        if self.cancellation_token.as_ref().unwrap().is_canceled() { return; }

        // If hashing was performed, update total_bytes_processed for Create/Update.
        // For Verify, total_bytes_processed was already updated for checked files.
        if self.process_type == HashFileProcessType::Create || self.process_type == HashFileProcessType::Update {
            self.process_stats.total_bytes_processed += disk_file_size;
        }

        match self.process_type {
            HashFileProcessType::Create => {
                self.hash_file.add_entry(HashFileEntry { file_path: relative_file_path_str, size: Some(disk_file_size), binary: true, digest });
            }
            HashFileProcessType::Verify => { // Implies entry existed and hashing was done
                if let Some(entry) = existing_manifest_entry_opt {
                    if digest != entry.digest {
                        self.handle_error(relative_file_path, FileProcessState::IncorrectHash);
                    }
                }
                self.hash_file.remove_entry(&relative_file_path_str); // Remove from check-list
            }
            HashFileProcessType::Update => {
                if let Some(mut entry) = existing_manifest_entry_opt { // File was in manifest and re-hashed
                    entry.digest = digest;
                    entry.size = Some(disk_file_size);
                    self.hash_file.update_entry(entry); // Assumes this replaces the old entry
                    self.process_stats.files_updated_in_update += 1;
                } else { // New file for manifest
                    self.hash_file.add_entry(HashFileEntry { file_path: relative_file_path_str, size: Some(disk_file_size), binary: true, digest });
                    self.process_stats.files_added_in_update += 1;
                }
            }
        }
    }
}

fn get_hashcheck_file_name(hash_type: HashType) -> PathBuf {
    let hash_type_str: &str = hash_type.into();
    let hash_file = Path::new(HASHCHECK_BASE_FILE_NAME);
    hash_file.with_extension(hash_type_str.to_lowercase())
}

fn get_hashsum_file_name(hash_type: HashType) -> PathBuf {
    let hash_type_str: &str = hash_type.into();
    let hash_file_name = hash_type_str.to_uppercase() + HASHSUM_SUFFIX;
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
