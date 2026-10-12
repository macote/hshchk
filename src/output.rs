use crate::hash_file_process::{FileProcessEntry, FileProgress, ProcessOutcome};
use crate::speed::get_speed;
use num_format::{Locale, ToFormattedString};
use std::fs::{File, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

pub struct Output {
    report: Option<File>,
    quiet: bool,
    stdout_summary: bool,
    progress: bool,
    width: usize,
    last_refresh: Option<Instant>,
    file_started: Option<Instant>,
    line_active: bool,
}

impl Output {
    pub fn new(
        report: Option<&Path>,
        quiet: bool,
        progress: Option<bool>,
        stdout_summary: bool,
    ) -> io::Result<Self> {
        let report = report
            .map(|path| OpenOptions::new().create(true).append(true).open(path))
            .transpose()?;
        Ok(Self {
            report,
            quiet,
            stdout_summary,
            progress: !quiet && progress.unwrap_or_else(|| io::stderr().is_terminal()),
            width: terminal_size::terminal_size()
                .map_or(80, |(width, _)| usize::from(width.0))
                .saturating_sub(1),
            last_refresh: None,
            file_started: None,
            line_active: false,
        })
    }
    fn report_line(&mut self, line: &str) -> io::Result<()> {
        if let Some(report) = &mut self.report {
            writeln!(report, "{}", line)?;
        }
        Ok(())
    }
    pub fn flush(&mut self) -> io::Result<()> {
        if let Some(report) = &mut self.report {
            report.flush()?;
        }
        io::stderr().flush()
    }
    fn clear_progress(&mut self) -> io::Result<()> {
        if self.line_active {
            write!(io::stderr(), "\r{}\r", " ".repeat(self.width))?;
            io::stderr().flush()?;
            self.line_active = false;
        }
        Ok(())
    }
    pub fn write_progress(&mut self, progress: &FileProgress) -> io::Result<()> {
        if progress.complete {
            self.report_line(&format!(
                "Hashed {} ({} bytes)",
                progress.file_path, progress.bytes_processed
            ))?;
        }
        if !self.progress {
            return Ok(());
        }
        let now = Instant::now();
        if progress.bytes_processed == 0 && !progress.complete {
            self.file_started = Some(now);
        }
        let started = *self.file_started.get_or_insert(now);
        if !progress.complete
            && self
                .last_refresh
                .is_some_and(|last| last.elapsed() < Duration::from_millis(233))
        {
            return Ok(());
        }
        let truncated = self.progress_line(progress, now.duration_since(started).as_millis());
        let padding = self.width.saturating_sub(truncated.graphemes(true).count());
        write!(io::stderr(), "\r{}{}", truncated, " ".repeat(padding))?;
        io::stderr().flush()?;
        self.last_refresh = Some(Instant::now());
        self.line_active = true;
        Ok(())
    }
    fn progress_line(&self, progress: &FileProgress, elapsed_ms: u128) -> String {
        let percent = if progress.file_size == 0 {
            100.0
        } else {
            progress.bytes_processed as f64 * 100.0 / progress.file_size as f64
        };
        let speed = get_speed(progress.bytes_processed, 0, elapsed_ms);
        let suffix = format!(
            " ({:.0}%; {} bytes; {} {})",
            percent,
            progress.bytes_processed,
            speed.bytes_per_interval.to_formatted_string(&Locale::en),
            speed.unit
        );
        let path_width = self.width.saturating_sub(suffix.graphemes(true).count());
        let path: String = progress
            .file_path
            .graphemes(true)
            .take(path_width)
            .collect();
        let line = format!("{}{}", path, suffix);
        line.graphemes(true).take(self.width).collect()
    }
    pub fn write_diagnostic(&mut self, entry: &FileProcessEntry, warning: bool) -> io::Result<()> {
        self.clear_progress()?;
        let line = format!(
            "{}: {}: {:?}",
            if warning { "warning" } else { "error" },
            entry.file_path.display(),
            entry.state
        );
        writeln!(io::stderr(), "{}", line)?;
        self.report_line(&line)
    }
    pub fn write_result(&mut self, outcome: &ProcessOutcome) -> io::Result<()> {
        let mode = outcome.process_type;
        let result = outcome.status;
        let stats = &outcome.stats;
        let dry_run = outcome.dry_run;
        self.clear_progress()?;
        let speed = if stats.elapsed_ms == 0 {
            "N/A".to_string()
        } else {
            let speed = get_speed(stats.bytes_hashed, 0, stats.elapsed_ms);
            format!(
                "{} {}",
                speed.bytes_per_interval.to_formatted_string(&Locale::en),
                speed.unit
            )
        };
        let lines = [
            format!("{:?}{}: {:?}", mode, if dry_run { " (dry run; manifest unchanged)" } else { "" }, result),
            format!("Files: {}; hashed: {} bytes; size checked: {} bytes", stats.files_processed.to_formatted_string(&Locale::en), stats.bytes_hashed.to_formatted_string(&Locale::en), stats.bytes_size_checked.to_formatted_string(&Locale::en)),
            format!("Added: {}; updated: {}; unchanged: {}; removed: {}; missing: {}; extra: {}; errors: {}", stats.files_added, stats.files_updated, stats.files_unchanged, stats.files_removed, stats.missing_files, stats.extra_files, stats.errors),
            format!("Elapsed: {:.3}s; hashing throughput: {}", stats.elapsed_ms as f64 / 1000.0, speed),
        ];
        for line in lines {
            self.report_line(&line)?;
            if !self.quiet && self.stdout_summary {
                writeln!(io::stdout(), "{}", line)?;
            }
        }
        if let Some(report) = &mut self.report {
            report.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/output.rs"]
mod tests;
