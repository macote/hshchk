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

/// Stable operation categories shared by library callers and CLI exit codes.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ProcessStatus {
    Success,
    VerificationFailed,
    Error,
    NoFilesProcessed,
    Canceled,
}

impl ProcessStatus {
    /// 0: success, 1: checksum mismatch/missing file, 2: operational failure, 130: cancellation.
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::VerificationFailed => 1,
            Self::Error | Self::NoFilesProcessed => 2,
            Self::Canceled => 130,
        }
    }

    /// Preserve the original status-only API, which combines both failure categories.
    pub fn legacy_result(self) -> HashFileProcessResult {
        match self {
            Self::Success => HashFileProcessResult::Success,
            Self::VerificationFailed | Self::Error => HashFileProcessResult::Error,
            Self::NoFilesProcessed => HashFileProcessResult::NoFilesProcessed,
            Self::Canceled => HashFileProcessResult::Canceled,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessDiagnostic {
    pub level: DiagnosticLevel,
    pub file_path: PathBuf,
    pub state: FileProcessState,
}

/// An owned snapshot of one run; later runs do not change its statistics or diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessOutcome {
    pub process_type: HashFileProcessType,
    pub status: ProcessStatus,
    pub stats: ProcessStats,
    pub diagnostics: Vec<ProcessDiagnostic>,
    pub dry_run: bool,
    /// True once replacement succeeded, even if writing the final summary then failed.
    pub manifest_written: bool,
}

impl ProcessOutcome {
    pub(crate) fn setup_error(
        process_type: HashFileProcessType,
        dry_run: bool,
        path: PathBuf,
        message: String,
    ) -> Self {
        let mut outcome = Self {
            process_type,
            status: ProcessStatus::Error,
            stats: ProcessStats::default(),
            diagnostics: Vec::new(),
            dry_run,
            manifest_written: false,
        };
        outcome.record_output_error(path, message);
        outcome
    }

    pub(crate) fn record_output_error(&mut self, path: PathBuf, message: String) {
        self.status = ProcessStatus::Error;
        self.stats.errors += 1;
        self.diagnostics.push(ProcessDiagnostic {
            level: DiagnosticLevel::Error,
            file_path: path,
            state: FileProcessState::Error(message),
        });
    }
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
    /// Explicit operation; None keeps directory-based auto mode.
    pub process_type: Option<HashFileProcessType>,
    /// Relative paths resolve against base_path, not the working directory.
    pub checksum_file: Option<PathBuf>,
    /// Authorize replacement in explicit Create mode. Legacy force_create also authorizes it.
    pub overwrite: bool,
}

pub struct HashFileProcessor {
    hash_file: HashFile,
    hash_type: HashType,
    hash_file_format: HashFileFormat,
    process_type: HashFileProcessType,
    hash_file_path: PathBuf,
    overwrite_manifest: bool,
    base_path: PathBuf,
    size_only: bool,
    report_extra: bool,
    match_regex: Option<Regex>,
    ignore_regex: Option<Regex>,
    error_occurred: bool,
    diagnostics: Vec<ProcessDiagnostic>,
    manifest_written: bool,
    process_stats: ProcessStats,
    remove_missing_in_update: bool,
    dry_run: bool,
    seen_paths: HashSet<String>,
    excluded_paths: HashSet<PathBuf>,
    console_outputs: Vec<same_file::Handle>,
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
        let legacy_mode = if options.force_create.unwrap_or_default() {
            Some(HashFileProcessType::Create)
        } else if options.update_mode {
            Some(HashFileProcessType::Update)
        } else {
            None
        };
        if (options.update_mode && options.force_create.unwrap_or_default())
            || options
                .process_type
                .zip(legacy_mode)
                .is_some_and(|(explicit, legacy)| explicit != legacy)
        {
            return Err(invalid_options("conflicting operation modes"));
        }
        let requested_mode = options.process_type.or(legacy_mode);
        if (options.remove_missing_in_update || options.dry_run)
            && requested_mode != Some(HashFileProcessType::Update)
        {
            return Err(invalid_options("removal and dry-run require update mode"));
        }
        if options.overwrite && requested_mode != Some(HashFileProcessType::Create) {
            return Err(invalid_options("overwrite requires create mode"));
        }
        let base_path = fs::canonicalize(&options.base_path)?;
        if !base_path.is_dir() {
            return Err(invalid_options("target is not a directory"));
        }
        let overwrite = options.overwrite || options.force_create.unwrap_or_default();
        let mut hash_type = options.hash_type.unwrap_or(HashType::SHA1);
        let mut hash_file_format = options
            .hash_file_format
            .unwrap_or(HashFileFormat::HashCheck);
        let mut selected_path = None;
        let mut exists = false;

        if let Some(path) = &options.checksum_file {
            let known = checksum_name_hint(path);
            let path = resolve_checksum_path(&base_path, path)?;
            let metadata = match fs::metadata(&path) {
                Ok(metadata) => Some(metadata),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => return Err(error),
            };
            if metadata
                .as_ref()
                .is_some_and(|metadata| !metadata.is_file())
            {
                return Err(invalid_options("checksum path is not a regular file"));
            }
            exists = metadata.is_some();
            if let Some((known_type, _)) = known {
                if options
                    .hash_type
                    .is_some_and(|requested| requested != known_type)
                {
                    return Err(invalid_options(
                        "selected algorithm conflicts with checksum filename",
                    ));
                }
                hash_type = known_type;
            } else if exists
                && requested_mode != Some(HashFileProcessType::Create)
                && options.hash_type.is_none()
            {
                return Err(invalid_options(
                    "custom checksum filename requires an explicit algorithm (--type)",
                ));
            }
            if exists && requested_mode != Some(HashFileProcessType::Create) {
                hash_file_format = HashFile::detect_format(&path)?
                    .or(known.map(|(_, format)| format))
                    .unwrap_or(HashFileFormat::HashSum);
            }
            selected_path = Some(path);
        } else if requested_mode != Some(HashFileProcessType::Create) {
            let mut candidates = discover_checksum_files(&base_path, options.hash_type)?;
            if candidates.len() > 1 {
                candidates.sort_by(|left, right| left.0.cmp(&right.0));
                let names = candidates
                    .iter()
                    .map(|(path, _, _)| path.file_name().unwrap().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(invalid_options(&format!("multiple checksum files found: {}. Select one with --checksum-file (or restrict discovery with --type)", names)));
            }
            if let Some((path, algorithm, format)) = candidates.pop() {
                exists = true;
                hash_type = algorithm;
                hash_file_format = HashFile::detect_format(&path)?.unwrap_or(format);
                selected_path = Some(path);
            }
        }
        let process_type = requested_mode.unwrap_or(if exists {
            HashFileProcessType::Verify
        } else {
            HashFileProcessType::Create
        });
        if process_type != HashFileProcessType::Create && !exists {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no checksum file found for the requested operation",
            ));
        }
        let hash_file_path = selected_path.unwrap_or_else(|| {
            base_path.join(match hash_file_format {
                HashFileFormat::HashCheck => get_hashcheck_file_name(hash_type),
                HashFileFormat::HashSum => get_hashsum_file_name(hash_type),
            })
        });
        if process_type == HashFileProcessType::Create && !overwrite && hash_file_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "checksum file already exists; use --overwrite to replace it",
            ));
        }
        if options.size_only.unwrap_or_default()
            && (process_type != HashFileProcessType::Verify
                || hash_file_format != HashFileFormat::HashCheck)
        {
            return Err(invalid_options(
                "size-only verification requires a manifest with recorded sizes",
            ));
        }
        if options.report_extra.unwrap_or_default() && process_type != HashFileProcessType::Verify {
            return Err(invalid_options(
                "reporting extra files requires verify mode",
            ));
        }
        let compile = |pattern: Option<&str>| -> io::Result<Option<Regex>> {
            pattern
                .map(Regex::new)
                .transpose()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
        };
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
            overwrite_manifest: process_type != HashFileProcessType::Create || overwrite,
            base_path,
            size_only: options.size_only.unwrap_or_default(),
            report_extra: options.report_extra.unwrap_or_default(),
            match_regex: compile(options.match_pattern)?,
            ignore_regex: compile(options.ignore_pattern)?,
            error_occurred: false,
            diagnostics: Vec::new(),
            manifest_written: false,
            process_stats: ProcessStats::default(),
            remove_missing_in_update: options.remove_missing_in_update,
            dry_run: options.dry_run,
            seen_paths: HashSet::new(),
            excluded_paths,
            console_outputs: Vec::new(),
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
    pub fn checksum_file_path(&self) -> &Path {
        &self.hash_file_path
    }
    pub fn hash_type(&self) -> HashType {
        self.hash_type
    }
    pub fn hash_file_format(&self) -> HashFileFormat {
        self.hash_file_format
    }
    pub fn stats(&self) -> &ProcessStats {
        &self.process_stats
    }
    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }
    pub(crate) fn exclude_console_destinations(&mut self) {
        self.console_outputs =
            IntoIterator::into_iter([same_file::Handle::stdout(), same_file::Handle::stderr()])
                .filter_map(Result::ok)
                .collect();
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
        self.process_outcome().status.legacy_result()
    }
    pub fn process_with_cancellation_token(
        &mut self,
        token: CancellationToken,
    ) -> HashFileProcessResult {
        self.process_outcome_with_cancellation_token(token)
            .status
            .legacy_result()
    }

    /// Process files and return an owned result, including diagnostics without event subscriptions.
    ///
    /// ```no_run
    /// use hshchk::hash_file_process::{HashFileProcessOptions, HashFileProcessor, ProcessStatus};
    /// let mut processor = HashFileProcessor::try_new(HashFileProcessOptions {
    ///     base_path: "files".into(),
    ///     ..Default::default()
    /// })?;
    /// let outcome = processor.process_outcome();
    /// if outcome.status == ProcessStatus::VerificationFailed {
    ///     for diagnostic in &outcome.diagnostics {
    ///         eprintln!("{}: {:?}", diagnostic.file_path.display(), diagnostic.state);
    ///     }
    /// }
    /// println!("Hashed {} bytes", outcome.stats.bytes_hashed);
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn process_outcome(&mut self) -> ProcessOutcome {
        self.process_outcome_with_cancellation_token(CancellationToken::new())
    }

    pub fn process_outcome_with_cancellation_token(
        &mut self,
        token: CancellationToken,
    ) -> ProcessOutcome {
        self.process_stats = ProcessStats::default();
        self.error_occurred = false;
        self.diagnostics.clear();
        self.manifest_written = false;
        self.seen_paths.clear();
        self.hash_file = HashFile::new();
        let started = Instant::now();
        let result = self.process_internal(token);
        self.process_stats.elapsed_ms = started.elapsed().as_millis();
        if let Some(sender) = &self.complete_event {
            let _ = sender.send(result);
        }
        let status = match result {
            HashFileProcessResult::Success => ProcessStatus::Success,
            HashFileProcessResult::NoFilesProcessed => ProcessStatus::NoFilesProcessed,
            HashFileProcessResult::Canceled => ProcessStatus::Canceled,
            HashFileProcessResult::Error
                if self.process_type == HashFileProcessType::Verify
                    && !self.has_operational_errors() =>
            {
                ProcessStatus::VerificationFailed
            }
            HashFileProcessResult::Error => ProcessStatus::Error,
        };
        self.diagnostics
            .sort_by(|left, right| left.file_path.cmp(&right.file_path));
        ProcessOutcome {
            process_type: self.process_type,
            status,
            stats: self.process_stats.clone(),
            diagnostics: self.diagnostics.clone(),
            dry_run: self.dry_run,
            manifest_written: self.manifest_written,
        }
    }
    fn has_operational_errors(&self) -> bool {
        self.diagnostics.iter().any(|diagnostic| {
            diagnostic.level == DiagnosticLevel::Error
                && !matches!(
                    diagnostic.state,
                    FileProcessState::Missing
                        | FileProcessState::IncorrectSize
                        | FileProcessState::IncorrectHash
                )
        })
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
            || (!self.console_outputs.is_empty()
                && same_file::Handle::from_path(path).is_ok_and(|handle| {
                    self.console_outputs.iter().any(|output| output == &handle)
                }))
    }
    fn handle_error(&mut self, path: &Path, state: FileProcessState) {
        self.error_occurred = true;
        self.process_stats.errors += 1;
        if state == FileProcessState::Missing {
            self.process_stats.missing_files += 1;
        }
        self.diagnostics.push(ProcessDiagnostic {
            level: DiagnosticLevel::Error,
            file_path: path.to_path_buf(),
            state: state.clone(),
        });
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
        self.diagnostics.push(ProcessDiagnostic {
            level: DiagnosticLevel::Warning,
            file_path: path.to_path_buf(),
            state: state.clone(),
        });
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
        if self.has_operational_errors() {
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
                self.overwrite_manifest,
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
            self.manifest_written = true;
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
fn invalid_options(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn checksum_name_hint(path: &Path) -> Option<(HashType, HashFileFormat)> {
    let name = path.file_name()?.to_str()?;
    HashType::iter().find_map(|algorithm| {
        let algorithm_name: &str = algorithm.into();
        if name.eq_ignore_ascii_case(&format!("hshchk.{}", algorithm_name)) {
            Some((algorithm, HashFileFormat::HashCheck))
        } else if name.eq_ignore_ascii_case(&format!("{}SUM", algorithm_name))
            || name.eq_ignore_ascii_case(&format!("{}SUMS", algorithm_name))
        {
            Some((algorithm, HashFileFormat::HashSum))
        } else {
            None
        }
    })
}

fn discover_checksum_files(
    base: &Path,
    selected_type: Option<HashType>,
) -> io::Result<Vec<(PathBuf, HashType, HashFileFormat)>> {
    let mut candidates = Vec::new();
    for entry in fs::read_dir(base)? {
        let path = entry?.path();
        if let Some((algorithm, format)) = checksum_name_hint(&path) {
            if selected_type.is_none_or(|selected| selected == algorithm)
                && fs::metadata(&path)?.is_file()
            {
                candidates.push((fs::canonicalize(path)?, algorithm, format));
            }
        }
    }
    Ok(candidates)
}

fn resolve_checksum_path(base: &Path, path: &Path) -> io::Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    match fs::canonicalize(&path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = fs::canonicalize(
                path.parent()
                    .ok_or_else(|| invalid_options("checksum path has no parent"))?,
            )?;
            Ok(parent.join(
                path.file_name()
                    .ok_or_else(|| invalid_options("checksum path has no filename"))?,
            ))
        }
        Err(error) => Err(error),
    }
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
        let outcome = update.process_outcome();
        assert_eq!(outcome.status, ProcessStatus::Error);
        assert!(!outcome.manifest_written);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(fs::read(&manifest).unwrap(), original);
        assert_eq!(update.stats().errors, 1);
    }

    #[test]
    fn cancellation_before_save_retains_completed_statistics() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("file"), "data").unwrap();
        let token = CancellationToken::new();
        let cancel = token.clone();
        let mut processor = HashFileProcessor::new(HashFileProcessOptions {
            base_path: root.path().into(),
            ..Default::default()
        });
        processor.set_before_save(move || {
            cancel.cancel();
            Ok(())
        });
        let outcome = processor.process_outcome_with_cancellation_token(token);
        assert_eq!(outcome.status, ProcessStatus::Canceled);
        assert_eq!(outcome.stats.bytes_hashed, 4);
        assert_eq!(outcome.stats.files_processed, 1);
        assert!(!outcome.manifest_written);
        assert!(!root.path().join("hshchk.sha1").exists());
    }

    #[test]
    fn create_does_not_replace_a_manifest_that_appears_after_setup() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("file"), "data").unwrap();
        let manifest = root.path().join("hshchk.sha1");
        let late_manifest = manifest.clone();
        let mut processor = HashFileProcessor::try_new(HashFileProcessOptions {
            base_path: root.path().into(),
            process_type: Some(HashFileProcessType::Create),
            ..Default::default()
        })
        .unwrap();
        processor.set_before_save(move || fs::write(&late_manifest, "created by another writer"));
        let outcome = processor.process_outcome();
        assert_eq!(outcome.status, ProcessStatus::Error);
        assert!(!outcome.manifest_written);
        assert_eq!(
            fs::read_to_string(&manifest).unwrap(),
            "created by another writer"
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    }
}
