use crate::file_tree::{FileTree, FileTreeProcessor};
use crate::hash_file::{HashFile, HashFileEntry};
use crate::{HashFileFormat, HashType};
use crossbeam::channel::Sender;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;
use strum::IntoEnumIterator;
use tokio_util::sync::CancellationToken;

const DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE: usize = 2_097_152;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum HashFileProcessType {
    Create,
    Verify,
    Update,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum HashFileProcessResult {
    Success,
    Error,
    NoFilesProcessed,
    Canceled,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct ProcessStats {
    pub files_processed: u64,
    pub bytes_hashed: u64,
    pub bytes_size_checked: u64,
    pub files_added: u64,
    pub files_updated: u64,
    pub files_unchanged: u64,
    pub files_removed: u64,
    pub missing_files: u64,
    pub extra_files: u64,
    pub errors: u64,
    pub elapsed_ms: u128,
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
    pub complete: bool,
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
    pub update_mode: bool,
    pub remove_missing_in_update: bool,
    pub dry_run: bool,
}

pub struct HashFileProcessor {
    hash_file: HashFile,
    hash_type: HashType,
    hash_file_format: HashFileFormat,
    process_type: HashFileProcessType,
    hash_file_path: PathBuf,
    base_path: PathBuf,
    size_only: bool,
    report_extra: bool,
    match_regex: Option<Regex>,
    ignore_regex: Option<Regex>,
    error_occurred: bool,
    process_stats: ProcessStats,
    remove_missing_in_update: bool,
    dry_run: bool,
    seen_paths: HashSet<String>,
    excluded_paths: HashSet<PathBuf>,
    bytes_processed_notification_block_size: usize,
    cancellation_token: Option<CancellationToken>,
    progress_event: Option<Sender<FileProgress>>,
    warning_event: Option<Sender<FileProcessEntry>>,
    error_event: Option<Sender<FileProcessEntry>>,
    complete_event: Option<Sender<HashFileProcessResult>>,
    before_save: Option<Box<dyn FnMut() -> io::Result<()> + Send>>,
}

impl HashFileProcessor {
    /// Compatibility constructor. CLI callers use try_new for actionable errors.
    pub fn new(options: HashFileProcessOptions) -> Self {
        Self::try_new(options).expect("invalid processing options")
    }
    pub fn try_new(options: HashFileProcessOptions) -> io::Result<Self> {
        if options.update_mode && options.force_create.unwrap_or_default() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "create and update are mutually exclusive",
            ));
        }
        if (options.remove_missing_in_update || options.dry_run) && !options.update_mode {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "removal and dry-run require update mode",
            ));
        }
        let base_path = fs::canonicalize(&options.base_path)?;
        if !base_path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "target is not a directory",
            ));
        }
        let mut hash_type = options.hash_type.unwrap_or(HashType::SHA1);
        let mut hash_file_format = options
            .hash_file_format
            .unwrap_or(HashFileFormat::HashCheck);
        let mut process_type = HashFileProcessType::Create;
        if !options.force_create.unwrap_or_default() {
            if let Some((existing_type, format)) =
                get_existing_file_hash_type(&base_path, hash_type)
            {
                hash_type = existing_type;
                hash_file_format = format;
                process_type = if options.update_mode {
                    HashFileProcessType::Update
                } else {
                    HashFileProcessType::Verify
                };
            } else if options.update_mode {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "no checksum file found to update",
                ));
            }
        }
        if options.size_only.unwrap_or_default()
            && (process_type != HashFileProcessType::Verify
                || hash_file_format != HashFileFormat::HashCheck)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "size-only verification requires a manifest with recorded sizes",
            ));
        }
        let compile = |pattern: Option<&str>| -> io::Result<Option<Regex>> {
            pattern
                .map(Regex::new)
                .transpose()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
        };
        let hash_file_path = base_path.join(match hash_file_format {
            HashFileFormat::HashCheck => get_hashcheck_file_name(hash_type),
            HashFileFormat::HashSum => get_hashsum_file_name(hash_type),
        });
        let mut excluded_paths = HashSet::new();
        excluded_paths.insert(hash_file_path.clone());
        if let Ok(binary) = std::env::current_exe().and_then(fs::canonicalize) {
            excluded_paths.insert(binary);
        }
        Ok(Self {
            hash_file: HashFile::new(),
            hash_type,
            hash_file_format,
            process_type,
            hash_file_path,
            base_path,
            size_only: options.size_only.unwrap_or_default(),
            report_extra: options.report_extra.unwrap_or_default(),
            match_regex: compile(options.match_pattern)?,
            ignore_regex: compile(options.ignore_pattern)?,
            error_occurred: false,
            process_stats: ProcessStats::default(),
            remove_missing_in_update: options.remove_missing_in_update,
            dry_run: options.dry_run,
            seen_paths: HashSet::new(),
            excluded_paths,
            bytes_processed_notification_block_size:
                DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
            cancellation_token: None,
            progress_event: None,
            warning_event: None,
            error_event: None,
            complete_event: None,
            before_save: None,
        })
    }
    pub fn get_process_type(&self) -> HashFileProcessType {
        self.process_type
    }
    pub fn stats(&self) -> &ProcessStats {
        &self.process_stats
    }
    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }
    pub(crate) fn set_before_save(
        &mut self,
        callback: impl FnMut() -> io::Result<()> + Send + 'static,
    ) {
        self.before_save = Some(Box::new(callback));
    }
    pub fn exclude_output_file(&mut self, path: &Path) -> io::Result<()> {
        let path = if path.exists() {
            fs::canonicalize(path)?
        } else {
            let absolute = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()?.join(path)
            };
            let parent = fs::canonicalize(absolute.parent().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "report has no parent directory",
                )
            })?)?;
            parent.join(absolute.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "report has no file name")
            })?)
        };
        if path == self.hash_file_path
            || same_file::is_same_file(&path, &self.hash_file_path).unwrap_or(false)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "report file must differ from checksum file",
            ));
        }
        self.excluded_paths.insert(path);
        Ok(())
    }
    pub fn set_progress_event_sender(&mut self, sender: Sender<FileProgress>) {
        self.set_progress_event_sender_with_bytes_processed_notification_block_size(
            sender,
            DEFAULT_BYTES_PROCESSED_NOTIFICATION_BLOCK_SIZE,
        );
    }
    pub fn set_progress_event_sender_with_bytes_processed_notification_block_size(
        &mut self,
        sender: Sender<FileProgress>,
        block_size: usize,
    ) {
        self.progress_event = Some(sender);
        self.bytes_processed_notification_block_size = block_size;
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
    pub fn process(&mut self) -> HashFileProcessResult {
        self.process_with_cancellation_token(CancellationToken::new())
    }
    pub fn process_with_cancellation_token(
        &mut self,
        token: CancellationToken,
    ) -> HashFileProcessResult {
        self.process_stats = ProcessStats::default();
        self.error_occurred = false;
        self.seen_paths.clear();
        self.hash_file = HashFile::new();
        let started = Instant::now();
        let result = self.process_internal(token);
        self.process_stats.elapsed_ms = started.elapsed().as_millis();
        if let Some(sender) = &self.complete_event {
            let _ = sender.send(result);
        }
        result
    }
    fn selected(&self, path: &str) -> bool {
        let path = path.replace('\\', "/");
        self.match_regex
            .as_ref()
            .is_none_or(|regex| regex.is_match(&path))
            && self
                .ignore_regex
                .as_ref()
                .is_none_or(|regex| !regex.is_match(&path))
    }
    fn excluded(&self, path: &Path) -> bool {
        self.excluded_paths.contains(path)
            || fs::canonicalize(path).is_ok_and(|p| self.excluded_paths.contains(&p))
            || self
                .excluded_paths
                .iter()
                .any(|excluded| same_file::is_same_file(path, excluded).unwrap_or(false))
    }
    fn handle_error(&mut self, path: &Path, state: FileProcessState) {
        self.error_occurred = true;
        self.process_stats.errors += 1;
        if state == FileProcessState::Missing {
            self.process_stats.missing_files += 1;
        }
        if let Some(sender) = &self.error_event {
            let _ = sender.send(FileProcessEntry {
                file_path: path.to_path_buf(),
                state,
            });
        }
    }
    fn handle_warning(&mut self, path: &Path, state: FileProcessState) {
        if state == FileProcessState::Extra {
            self.process_stats.extra_files += 1;
        }
        if state == FileProcessState::Missing {
            self.process_stats.missing_files += 1;
        }
        if let Some(sender) = &self.warning_event {
            let _ = sender.send(FileProcessEntry {
                file_path: path.to_path_buf(),
                state,
            });
        }
    }
    fn process_internal(&mut self, cancellation_token: CancellationToken) -> HashFileProcessResult {
        self.cancellation_token = Some(cancellation_token.clone());
        if cancellation_token.is_cancelled() {
            return HashFileProcessResult::Canceled;
        }
        if self.process_type != HashFileProcessType::Create {
            if let Err(error) = self.hash_file.load(&self.hash_file_path) {
                self.handle_error(
                    &self.hash_file_path.clone(),
                    FileProcessState::Error(error.to_string()),
                );
                return HashFileProcessResult::Error;
            }
            for path in self.hash_file.get_file_paths() {
                let entry = self.hash_file.get_entry(&path).unwrap();
                let expected_length = match self.hash_type {
                    HashType::MD5 => 32,
                    HashType::SHA1 => 40,
                    HashType::SHA256 | HashType::Blake2s | HashType::Blake3 => 64,
                    HashType::SHA512 | HashType::Blake2b => 128,
                };
                if entry.digest.len() != expected_length
                    || !entry.digest.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    self.handle_error(
                        Path::new(&path),
                        FileProcessState::Error("invalid digest for selected algorithm".into()),
                    );
                }
            }
            if self.error_occurred {
                return HashFileProcessResult::Error;
            }
        }
        let path = self.base_path.clone();
        if let Err(error) = FileTree::new(self).traverse(&path, &cancellation_token) {
            self.handle_error(&path, FileProcessState::Error(error.to_string()));
        }
        if cancellation_token.is_cancelled() {
            return HashFileProcessResult::Canceled;
        }
        // An incomplete scan cannot authorize removing entries or replacing a manifest.
        if self.error_occurred {
            return HashFileProcessResult::Error;
        }
        if self.process_type != HashFileProcessType::Create {
            for path in self.hash_file.get_file_paths() {
                if self.seen_paths.contains(&path)
                    || !self.selected(&path)
                    || self.excluded(&self.base_path.join(&path))
                {
                    continue;
                }
                if self.process_type == HashFileProcessType::Verify {
                    self.handle_error(Path::new(&path), FileProcessState::Missing);
                } else if self.remove_missing_in_update {
                    self.hash_file.remove_entry(&path);
                    self.process_stats.files_removed += 1;
                } else {
                    self.handle_warning(Path::new(&path), FileProcessState::Missing);
                }
            }
        }
        if self.error_occurred {
            return HashFileProcessResult::Error;
        }
        if (self.process_type == HashFileProcessType::Create && self.hash_file.is_empty())
            || (self.process_stats.files_processed == 0
                && self.process_stats.files_removed == 0
                && self.process_stats.missing_files == 0
                && self.process_stats.extra_files == 0)
        {
            return HashFileProcessResult::NoFilesProcessed;
        }
        if self.process_type != HashFileProcessType::Verify && !self.dry_run {
            if let Some(callback) = &mut self.before_save {
                if let Err(error) = callback() {
                    self.handle_error(
                        &self.hash_file_path.clone(),
                        FileProcessState::Error(format!(
                            "cannot flush output before saving: {}",
                            error
                        )),
                    );
                    return HashFileProcessResult::Error;
                }
            }
            if let Err(error) = self.hash_file.save(
                &self.hash_file_path,
                self.hash_file_format,
                &cancellation_token,
            ) {
                if cancellation_token.is_cancelled() {
                    return HashFileProcessResult::Canceled;
                }
                self.handle_error(
                    &self.hash_file_path.clone(),
                    FileProcessState::Error(error.to_string()),
                );
                return HashFileProcessResult::Error;
            }
        }
        HashFileProcessResult::Success
    }
}

impl FileTreeProcessor for HashFileProcessor {
    fn process_file(&mut self, file_path: &Path) {
        if self.excluded(file_path) {
            return;
        }
        let relative = file_path
            .strip_prefix(&self.base_path)
            .expect("traversal outside target");
        let path = match relative.to_str() {
            Some(path) => path.to_string(),
            None => {
                self.handle_error(relative, FileProcessState::InvalidUnicodeFileName);
                return;
            }
        };
        if !self.selected(&path) {
            return;
        }
        if self.process_type != HashFileProcessType::Verify
            && (path.contains(['\n', '\r'])
                || (cfg!(unix) && path.contains('\\'))
                || (self.hash_file_format == HashFileFormat::HashCheck && path.contains('|')))
        {
            self.handle_error(
                relative,
                FileProcessState::Error(
                    "file name cannot be represented in selected checksum format".into(),
                ),
            );
            return;
        }
        let file_size = match fs::metadata(file_path) {
            Ok(metadata) if metadata.is_file() => metadata.len(),
            Ok(_) => {
                self.handle_error(
                    relative,
                    FileProcessState::Error("not a regular file".into()),
                );
                return;
            }
            Err(error) => {
                self.handle_error(relative, FileProcessState::Error(error.to_string()));
                return;
            }
        };
        self.seen_paths.insert(path.clone());
        let existing = self.hash_file.get_entry(&path).cloned();
        if self.process_type == HashFileProcessType::Verify {
            let entry = match &existing {
                Some(entry) => entry,
                None => {
                    if self.report_extra {
                        self.handle_warning(relative, FileProcessState::Extra);
                    }
                    return;
                }
            };
            self.process_stats.files_processed += 1;
            if entry.size.is_some_and(|size| size != file_size) {
                self.handle_error(relative, FileProcessState::IncorrectSize);
                return;
            }
            if self.size_only {
                self.process_stats.bytes_size_checked += file_size;
                return;
            }
        } else {
            self.process_stats.files_processed += 1;
        }
        let mut hasher = match crate::get_file_hasher(self.hash_type, file_path) {
            Ok(hasher) => hasher,
            Err(error) => {
                self.handle_error(relative, FileProcessState::Error(error.to_string()));
                return;
            }
        };
        if let Some(sender) = &self.progress_event {
            let _ = sender.send(FileProgress {
                file_path: path.clone(),
                file_size,
                bytes_processed: 0,
                complete: false,
            });
        }
        let token = self.cancellation_token.as_ref().unwrap();
        let sender = self.progress_event.clone();
        let block_size = self.bytes_processed_notification_block_size as u64;
        let mut last_notification = 0;
        let mut bytes_read = 0;
        let hashed = match hasher.compute_with_progress(token.clone(), &mut |bytes| {
            bytes_read = bytes;
            if bytes.saturating_sub(last_notification) >= block_size {
                if let Some(sender) = &sender {
                    let _ = sender.send(FileProgress {
                        file_path: path.clone(),
                        file_size,
                        bytes_processed: bytes,
                        complete: false,
                    });
                }
                last_notification = bytes;
            }
        }) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.process_stats.bytes_hashed += bytes_read;
                self.handle_error(relative, FileProcessState::Error(error.to_string()));
                return;
            }
        };
        self.process_stats.bytes_hashed += hashed;
        if token.is_cancelled() {
            return;
        }
        if hashed != file_size {
            self.handle_error(
                relative,
                FileProcessState::Error("file size changed during hashing".into()),
            );
            return;
        }
        if let Err(error) = hasher.validate_file(file_path) {
            self.handle_error(relative, FileProcessState::Error(error.to_string()));
            return;
        }
        let digest = hasher.digest();
        if let Some(sender) = &self.progress_event {
            let _ = sender.send(FileProgress {
                file_path: path.clone(),
                file_size,
                bytes_processed: hashed,
                complete: true,
            });
        }
        match self.process_type {
            HashFileProcessType::Verify => {
                if existing.unwrap().digest != digest {
                    self.handle_error(relative, FileProcessState::IncorrectHash);
                }
            }
            HashFileProcessType::Create | HashFileProcessType::Update => {
                match existing {
                    Some(entry)
                        if entry.digest == digest
                            && entry.size.is_none_or(|size| size == file_size) =>
                    {
                        self.process_stats.files_unchanged += 1
                    }
                    Some(_) => self.process_stats.files_updated += 1,
                    None => self.process_stats.files_added += 1,
                }
                self.hash_file.add_entry(HashFileEntry {
                    file_path: path,
                    size: Some(file_size),
                    digest,
                });
            }
        }
    }
}

fn get_hashcheck_file_name(hash_type: HashType) -> PathBuf {
    let name: &str = hash_type.into();
    Path::new("hshchk").with_extension(name.to_lowercase())
}
fn get_hashsum_file_name(hash_type: HashType) -> PathBuf {
    let name: &str = hash_type.into();
    PathBuf::from(format!("{}SUMS", name.to_uppercase()))
}
fn get_existing_file_hash_type(
    base: &Path,
    desired: HashType,
) -> Option<(HashType, HashFileFormat)> {
    std::iter::once(desired)
        .chain(HashType::iter().filter(|hash| *hash != desired))
        .find_map(|hash| {
            if base.join(get_hashcheck_file_name(hash)).is_file() {
                Some((hash, HashFileFormat::HashCheck))
            } else if base.join(get_hashsum_file_name(hash)).is_file() {
                Some((hash, HashFileFormat::HashSum))
            } else {
                None
            }
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_flush_failure_prevents_manifest_replacement() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("file");
        fs::write(&path, "data").unwrap();
        let mut create = HashFileProcessor::new(HashFileProcessOptions {
            base_path: root.path().to_path_buf(),
            ..Default::default()
        });
        assert_eq!(create.process(), HashFileProcessResult::Success);
        let manifest = root.path().join("hshchk.sha1");
        let original = fs::read(&manifest).unwrap();
        fs::write(&path, "tada").unwrap();
        let mut update = HashFileProcessor::new(HashFileProcessOptions {
            base_path: root.path().to_path_buf(),
            update_mode: true,
            ..Default::default()
        });
        update.set_before_save(|| Err(io::Error::other("report write failed")));
        assert_eq!(update.process(), HashFileProcessResult::Error);
        assert_eq!(fs::read(&manifest).unwrap(), original);
        assert_eq!(update.stats().errors, 1);
    }
}
