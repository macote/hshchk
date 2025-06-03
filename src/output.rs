use std::fs::{File, OpenOptions};
use std::io::{stdout, LineWriter, Write};
use std::path::Path;
use std::sync::Mutex;
use num_format::{Locale, ToFormattedString};
use std::time::Instant;
use unicode_segmentation::UnicodeSegmentation;

use crate::hash_file_process::{
    FileProcessEntry, FileProgress, HashFileProcessResult, ProcessStats,
};
use crate::speed::get_speed;
use crate::tty::terminal_size;

const BYTES_IN_KB: f64 = 1024.0;
const BYTES_IN_MB: f64 = BYTES_IN_KB * 1024.0;
const BYTES_IN_GB: f64 = BYTES_IN_MB * 1024.0;

const OUTPUT_REFRESH_IN_MILLIS: u32 = 233;

enum OutputWriter {
    Console(Box<dyn Write + Send>),
    File(LineWriter<File>),
}

pub struct Output {
    writer: Mutex<Option<OutputWriter>>, // Option to allow for potential failure in opening file
    output_width: usize,
    refresh_rate_in_millis: u32,
    last_output_instant: Option<Instant>,
    last_output_file_progress: FileProgress,
    is_tty: bool,
    silent_console: bool, // If true, console output (even if active writer) is suppressed for progress.
}

impl Output {
    pub fn new(report_file_path: Option<&Path>, silent_console_output: bool) -> Self {
        let (term_width, _) = terminal_size().unwrap_or((80, 0)); // Default width 80 if TTY unavailable
        let mut is_tty_active = atty::is(atty::Stream::Stdout);

        let writer_mutex = Mutex::new(Some(if let Some(path) = report_file_path {
            match OpenOptions::new().create(true).append(true).open(path) {
                Ok(file) => {
                    is_tty_active = false; // Writing to file, so TTY features are off for this output
                    OutputWriter::File(LineWriter::new(file))
                }
                Err(e) => {
                    // Error opening report file, fallback to console but print an error
                    eprintln!("Error opening report file '{}': {}. Falling back to console output.", path.display(), e);
                    // Still respect silent_console_output for the fallback console.
                    OutputWriter::Console(Box::new(stdout()))
                }
            }
        } else {
            OutputWriter::Console(Box::new(stdout()))
        }));

        Output {
            writer: writer_mutex,
            output_width: (term_width.0.saturating_sub(1)) as usize,
            refresh_rate_in_millis: OUTPUT_REFRESH_IN_MILLIS,
            last_output_instant: None,
            last_output_file_progress: FileProgress::default(),
            is_tty: is_tty_active && !silent_console_output, // TTY features only if console, not silent
            silent_console: silent_console_output,
        }
    }

    fn pad_line_if_tty(&self, line: &str) -> String {
        if self.is_tty {
            let mut padded_line = String::from(line);
            let line_len = line.graphemes(true).count();
            if line_len < self.output_width {
                let gap = self.output_width - line_len;
                padded_line.extend(repeat(" ").take(gap));
            }
            padded_line
        } else {
            line.to_string()
        }
    }

    // Internal generic write function
    fn output_line(&self, line: &str, to_stderr: bool) {
        if self.silent_console && !matches!(*self.writer.lock().unwrap(), Some(OutputWriter::File(_))) {
            // If silent_console is true AND we are not writing to a file, suppress output.
            // Errors to stderr might still be desired even in silent mode if no report file.
            if to_stderr && !self.has_report_file() {
                 eprintln!("{}", line); // Use eprintln for direct stderr output without padding/tty checks
            }
            return;
        }

        let mut writer_guard = self.writer.lock().unwrap();
        if let Some(writer_ref) = writer_guard.as_mut() {
            let formatted_line = self.pad_line_if_tty(line);
            let line_with_ending = if self.is_tty && !to_stderr { // TTY progress lines use \r
                format!(" {}\r", formatted_line)
            } else { // File lines or stderr lines use \n
                format!(" {}\n", line.trim_end_matches('\r')) // Ensure no \r for files/stderr
            };

            let write_result = match writer_ref {
                OutputWriter::Console(console_writer) => {
                    if to_stderr {
                        // For console, if it's an error, it should go to stderr.
                        // We need a way to access stderr directly or assume console_writer is stdout.
                        // For simplicity, using eprintln! for direct stderr when `to_stderr` is true for console.
                        eprint!("{}", line_with_ending); // eprintln adds newline
                        Ok(())
                    } else {
                         write!(console_writer, "{}", line_with_ending)
                    }
                }
                OutputWriter::File(file_writer) => {
                    write!(file_writer, "{}", line_with_ending)
                }
            };
            if let Err(e) = write_result {
                eprintln!("Error writing output: {}", e); // Fallback error to stderr
            }
            if let OutputWriter::Console(ref mut c) = writer_ref {
                let _ = c.flush(); // Try to flush console output
            }
        }
    }

    fn has_report_file(&self) -> bool {
        matches!(*self.writer.lock().unwrap(), Some(OutputWriter::File(_)))
    }


    fn write(
        &mut self,
        file_path_str: &str,
        file_size: u64,
        bytes_processed: u64,
        info_override: Option<&str>, // Used for specific messages like errors
        new_line_for_console_progress: bool, // True for final status of a file, or error
        is_error_message: bool,
    ) {
        // Suppress frequent progress updates if silent_console and no report file
        if self.silent_console && !self.has_report_file() && !is_error_message && !new_line_for_console_progress {
            return;
        }

        let now = Instant::now();
        let mut show_output = false;

        if is_error_message || new_line_for_console_progress || self.is_tty { // TTY allows refresh
            let ellapsed_millis = self.last_output_instant.map_or(0, |i| now.duration_since(i).as_millis());
            if is_error_message || new_line_for_console_progress || ellapsed_millis > self.refresh_rate_in_millis.into() {
                show_output = true;
            }
        } else if self.has_report_file() { // Always show for file report (no refresh logic)
            show_output = true;
        }


        if show_output {
            let info_str = if let Some(override_str) = info_override {
                format!(" => {}", override_str)
            } else if self.last_output_file_progress.file_path == file_path_str && file_size > 0 {
                 if bytes_processed != self.last_output_file_progress.bytes_processed {
                    let percent = bytes_processed * 100 / file_size;
                    let speed = get_speed(
                        bytes_processed,
                        self.last_output_file_progress.bytes_processed,
                        self.last_output_instant.map_or(0, |i| now.duration_since(i).as_millis()),
                    );
                    format!(
                        " ({}; {} %; {} {})",
                        file_size.to_formatted_string(&Locale::en),
                        percent.to_formatted_string(&Locale::en),
                        speed.bytes_per_interval.to_formatted_string(&Locale::en),
                        speed.unit
                    )
                } else { "".to_string() } // No change, no new info
            } else if file_size > 0 { // First progress update for this file
                 format!(" ({})", file_size.to_formatted_string(&Locale::en))
            } else { // No size, e.g. "processed" message
                "".to_string()
            };

            let mut line_to_print = file_path_str.to_string();
            if self.is_tty { // Truncate filename if TTY and too long for width
                let available_width = self.output_width.saturating_sub(info_str.graphemes(true).count());
                let mut graphemes = file_path_str.graphemes(true);
                if graphemes.clone().count() > available_width {
                    let mut truncated_path = String::from("..");
                    truncated_path.extend(graphemes.skip(graphemes.clone().count() - (available_width.saturating_sub(2))));
                    line_to_print = truncated_path;
                }
            }
            line_to_print.push_str(&info_str);

            self.output_line(&line_to_print, is_error_message);

            if show_output { // Only update these if we actually wrote something
                self.last_output_instant = Some(now);
                self.last_output_file_progress = FileProgress {
                    file_path: file_path_str.to_string(),
                    file_size,
                    bytes_processed,
                };
            }
        }
    }

    pub fn write_init(&mut self) {
        // Only write "Opening files..." if not silent for console OR if writing to a report file.
        if !self.silent_console || self.has_report_file() {
            self.output_line("Opening files...", false);
            self.last_output_instant = Some(Instant::now());
        }
    }

    pub fn write_error(&mut self, file_process_entry: &FileProcessEntry) {
        self.write(
            file_process_entry.file_path.to_str().unwrap_or("InvalidPath"),
            0,
            0,
            Some(&format!("{:?}", file_process_entry.state)),
            true, // new_line_for_console_progress = true for errors
            true, // is_error_message = true
        );
    }

    pub fn write_progress(&mut self, file_progress: &FileProgress) {
        self.write(
            &file_progress.file_path,
            file_progress.file_size,
            file_progress.bytes_processed,
            None, // No info override, standard progress string will be generated
            false, // Not a final status for the file
            false, // Not an error
        );
    }

    pub fn write_processed(&mut self, file_path: &str) {
        // For "processed" message, treat as a "new line for console progress" to ensure it prints.
        self.write(file_path, 0, 0, None, true, false);
    }

    // format_bytes and format_speed remain the same as before

    fn format_bytes(bytes: u64) -> String {
        if bytes as f64 >= BYTES_IN_GB {
            format!("{:.2} GB", bytes as f64 / BYTES_IN_GB)
        } else if bytes as f64 >= BYTES_IN_MB {
            format!("{:.2} MB", bytes as f64 / BYTES_IN_MB)
        } else if bytes as f64 >= BYTES_IN_KB {
            format!("{:.2} KB", bytes as f64 / BYTES_IN_KB)
        } else {
            format!("{} bytes", bytes)
        }
    }

    fn format_speed(bytes: u64, milliseconds: u128) -> String {
        if milliseconds == 0 {
            return "N/A".to_string();
        }
        let seconds = milliseconds as f64 / 1000.0;
        if seconds == 0.0 {
            return "N/A".to_string();
        }
        let bytes_per_second = bytes as f64 / seconds;
        if bytes_per_second >= BYTES_IN_GB {
            format!("{:.2} GB/s", bytes_per_second / BYTES_IN_GB)
        } else if bytes_per_second >= BYTES_IN_MB {
            format!("{:.2} MB/s", bytes_per_second / BYTES_IN_MB)
        } else if bytes_per_second >= BYTES_IN_KB {
            format!("{:.2} KB/s", bytes_per_second / BYTES_IN_KB)
        } else {
            format!("{:.0} bytes/s", bytes_per_second)
        }
    }


    pub fn write_result(&mut self, process_type_str: &str, result: &HashFileProcessResult) {
        // Result is important, so it should print even if silent_console is true,
        // but it will go to the file if a report file is configured.
        // The internal output_line will respect silent_console for console output.

        let mut main_message = String::new();
        let mut stats_lines: Vec<String> = Vec::new();

        match result {
            HashFileProcessResult::CreateSuccess(stats) | HashFileProcessResult::VerifySuccess(stats) | HashFileProcessResult::UpdateSuccess(stats) => {
                let type_specific_message = match result {
                    HashFileProcessResult::CreateSuccess(_) => "Create Success.".to_string(),
                    HashFileProcessResult::VerifySuccess(_) => {
                        let mut details = vec![];
                        if stats.files_checked_in_verify > 0 { details.push(format!("{} files checked", stats.files_checked_in_verify.to_formatted_string(&Locale::en))); }
                        if stats.errors_in_verify > 0 { details.push(format!("{} errors", stats.errors_in_verify.to_formatted_string(&Locale::en))); }
                        if stats.missing_files_in_verify > 0 { details.push(format!("{} missing", stats.missing_files_in_verify.to_formatted_string(&Locale::en))); }
                        if stats.extra_files_in_verify > 0 { details.push(format!("{} extra", stats.extra_files_in_verify.to_formatted_string(&Locale::en))); }
                        format!("Verify Success. {}", details.join(", "))
                    }
                    HashFileProcessResult::UpdateSuccess(_) => {
                        let mut details = vec![];
                        if stats.files_added_in_update > 0 { details.push(format!("{} added", stats.files_added_in_update.to_formatted_string(&Locale::en))); }
                        if stats.files_updated_in_update > 0 { details.push(format!("{} updated", stats.files_updated_in_update.to_formatted_string(&Locale::en))); }
                        if stats.files_removed_in_update > 0 { details.push(format!("{} removed", stats.files_removed_in_update.to_formatted_string(&Locale::en))); }
                        if stats.files_unchanged_in_update > 0 { details.push(format!("{} unchanged", stats.files_unchanged_in_update.to_formatted_string(&Locale::en))); }
                        if details.is_empty() {
                             "Update Success. No changes.".to_string()
                        } else {
                            format!("Update Success. {}", details.join(", "))
                        }
                    }
                    _ => unreachable!(), // Should not happen due to outer match
                };
                main_message = format!("{} result: {}", process_type_str, type_specific_message);

                stats_lines.push(format!(
                    "  Total files processed (on disk): {}", // Clarified meaning for total_files_processed
                    stats.total_files_processed.to_formatted_string(&Locale::en)
                ));
                stats_lines.push(format!(
                    "  Total bytes processed (hashed/checked): {} ({})", // Clarified meaning
                    Self::format_bytes(stats.total_bytes_processed),
                    stats.total_bytes_processed.to_formatted_string(&Locale::en)
                ));
                stats_lines.push(format!(
                    "  Processing time: {:.2} s",
                    stats.total_processing_time_ms as f64 / 1000.0
                ));
                stats_lines.push(format!(
                    "  Average speed: {}",
                    Self::format_speed(stats.total_bytes_processed, stats.total_processing_time_ms)
                ));
            }
            HashFileProcessResult::Error => main_message = format!("{} result: Error (see messages above)", process_type_str),
            HashFileProcessResult::NoFilesProcessed => main_message = format!("{} result: No files processed", process_type_str),
            HashFileProcessResult::Canceled => main_message = format!("{} result: Canceled", process_type_str),
        }

        self.output_line(&main_message, matches!(result, HashFileProcessResult::Error)); // Errors to stderr
        for line in stats_lines {
            self.output_line(&line, false); // Stats to normal output
        }
    }

    pub fn clear_line(&mut self) {
        // Only clear line if TTY is active (implies not silent for console and not a file)
        if self.is_tty {
            let mut writer_guard = self.writer.lock().unwrap();
            if let Some(OutputWriter::Console(console_writer)) = writer_guard.as_mut() {
                let line_to_clear = self.pad_line_if_tty("");
                print!("{}\r", line_to_clear); // Directly use print! for console manipulation
                let _ = stdout().flush();
            }
        }
    }
}
