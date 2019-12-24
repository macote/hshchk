use cancellation::CancellationToken;
use crossbeam::crossbeam_channel::{select, tick, unbounded};
use hshchk_lib::hash_file_process::{
    FileProcessEntry, FileProgress, HashFileProcessResult, HashFileProcessType, HashFileProcessor,
};
use num_format::{Locale, ToFormattedString};
use std::convert::TryInto;
use std::io::stdout;
use std::io::Write;
use std::iter::repeat;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::tty::terminal_size;

static EMPTY_FILE_PATH: &str = "";
static BPS: &str = "B/s";
static KBPS: &str = "KB/s";
static MBPS: &str = "MB/s";
static GBPS: &str = "GB/s";
const TICKER_REFRESH_IN_MILLIS: u32 = 222;
const PROGRESS_REFRESH_IN_MILLIS: u32 = 666;

struct Speed {
    bytes_per_interval: u64,
    unit: &'static str
}

fn get_speed(current_bytes: u64, previous_bytes: u64, elapsed_millis: u128) -> Speed {
    let speed = (current_bytes - previous_bytes) as u128 * 1_000 / elapsed_millis;
    if speed < 1_024 {
        return Speed { bytes_per_interval: speed.try_into().unwrap(), unit: BPS }
    } else if speed < 1_048_576 {
        return Speed { bytes_per_interval: (speed / 1_024).try_into().unwrap(), unit: KBPS };
    } else if speed < 1_073_741_824 {
        return Speed { bytes_per_interval: (speed / 1_048_576).try_into().unwrap(), unit: MBPS };
    }

    Speed { bytes_per_interval: (speed / 1_073_741_824).try_into().unwrap(), unit: GBPS }
}

struct ProgressLine {
    output_width: usize,
    refresh_rate_in_millis: u32,
    last_output_instant: Instant,
    last_file_progress: FileProgress,
}

impl ProgressLine {
    pub fn new() -> Self {
        let (output_width, _) = terminal_size().unwrap();
        ProgressLine {
            output_width: output_width.0 as usize,
            refresh_rate_in_millis: PROGRESS_REFRESH_IN_MILLIS,
            last_output_instant: Instant::now(),
            last_file_progress: FileProgress {
                ..Default::default()
            },
        }
    }
    fn pad_line(&self, line: String) -> String {
        let mut padded_line = line.clone();
        if line.len() < self.output_width.try_into().unwrap() {
            let gap = self.output_width - line.len();
            let pad = &repeat(" ").take(gap).collect::<String>();
            padded_line = line + pad;
        }

        padded_line
    }
    pub fn output_error(&self, file_process_entry: &FileProcessEntry) {
        eprintln!(
            "{}\r",
            self.pad_line(format!(
                " {} => {:?}",
                file_process_entry.file_path.display(),
                file_process_entry.state
            ))
        );
    }
    pub fn output_processed(&self, file_path: &str) {
        println!("{}\r", self.pad_line(format!(" {}", file_path)));
        stdout().flush().unwrap();
    }
    pub fn output_progress(&mut self, file_progress: &FileProgress) {
        let now = Instant::now();
        let mut speed = Speed { bytes_per_interval: 0, unit: BPS };
        let mut percent = 0u64;
        if self.last_file_progress.file_path == file_progress.file_path {
            percent = file_progress.bytes_processed * 100 / file_progress.file_size;
            let elapsed_millis = now.duration_since(self.last_output_instant).as_millis();
            if elapsed_millis > 0 {
                if file_progress.bytes_processed != self.last_file_progress.bytes_processed {
                    speed = get_speed(
                        file_progress.bytes_processed,
                        self.last_file_progress.bytes_processed,
                        elapsed_millis);
                }
            }
        }

        if file_progress.bytes_processed == 0 {
            self.last_output_instant = now;
            print!(
                "{}\r",
                self.pad_line(format!(
                    " {} ({})",
                    file_progress.file_path,
                    file_progress.file_size.to_formatted_string(&Locale::en)
                ))
            );
            stdout().flush().unwrap();
        } else if now.duration_since(self.last_output_instant).as_millis()
            > self.refresh_rate_in_millis.into()
        {
            self.last_output_instant = now;
            print!(
                "{}\r",
                self.pad_line(format!(
                    " {} ({} - {} % - {} {})",
                    file_progress.file_path,
                    file_progress.file_size.to_formatted_string(&Locale::en),
                    percent.to_formatted_string(&Locale::en),
                    speed.bytes_per_interval.to_formatted_string(&Locale::en),
                    speed.unit
                ))
            );
            stdout().flush().unwrap();
        }

        self.last_file_progress = FileProgress {
            file_path: file_progress.file_path.clone(),
            file_size: file_progress.file_size,
            bytes_processed: file_progress.bytes_processed,
        };
    }
}

pub struct UI {
    processor: HashFileProcessor,
    silent: bool,
}

impl UI {
    pub fn new(processor: HashFileProcessor, silent: bool) -> UI {
        UI { processor, silent }
    }
    pub fn run(
        mut self,
        cancellation_token: Arc<CancellationToken>,
        process_type: HashFileProcessType,
    ) -> HashFileProcessResult {
        let silent = self.silent;
        let (error_sender, error_receiver) = unbounded();
        let (warning_sender, warning_receiver) = unbounded();
        let (progress_sender, progress_receiver) = unbounded();
        let (complete_sender, complete_receiver) = unbounded();

        self.processor.set_error_event_sender(error_sender.clone());
        self.processor
            .set_warning_event_sender(warning_sender.clone());
        if !silent {
            self.processor
                .set_progress_event_sender(progress_sender.clone());
            self.processor
                .set_complete_event_sender(complete_sender.clone());
        }

        let silent_progress = silent;

        let message_loop = std::thread::spawn(move || {
            let mut progress_sender_dropped = false;
            let mut error_sender_dropped = false;
            let mut warning_sender_dropped = false;
            let mut senders_dropped = false;
            let mut skip_processed = false;
            let mut progress_line = ProgressLine::new();
            let mut file_progress = FileProgress {
                file_path: String::from(""),
                file_size: 0,
                bytes_processed: 0,
            };
            let ticker = tick(Duration::from_millis(TICKER_REFRESH_IN_MILLIS as u64));

            while !senders_dropped {
                select! {
                    recv(ticker) -> _ => {
                        if !silent_progress {
                            progress_line.output_progress(&file_progress);
                        }
                    },
                    recv(progress_receiver) -> msg => {
                        if let Ok(args) = msg {
                            if args.bytes_processed == 0 {
                                if file_progress.file_path != EMPTY_FILE_PATH && !skip_processed {
                                    progress_line.output_processed(&file_progress.file_path);
                                }

                                skip_processed = false;

                                file_progress.file_path = args.file_path;
                                file_progress.file_size = args.file_size;
                                file_progress.bytes_processed = 0;
                                progress_line.output_progress(&file_progress);
                            }
                            else {
                                file_progress.bytes_processed = args.bytes_processed;
                            }
                        }
                        else {
                            progress_sender_dropped = true;
                        }
                    },
                    recv(error_receiver) -> msg => {
                        if let Ok(error) = msg {
                            skip_processed = true;
                            progress_line.output_error(&error);
                        }
                        else {
                            error_sender_dropped = true;
                        }
                    },
                    recv(warning_receiver) -> msg => {
                        if let Ok(warning) = msg {
                            skip_processed = true;
                            progress_line.output_error(&warning);
                        } else {
                            warning_sender_dropped = true;
                        }
                    }
                }

                senders_dropped =
                    progress_sender_dropped && error_sender_dropped && warning_sender_dropped;
            }

            if !silent {
                progress_line.output_processed(&file_progress.file_path);
            }
        });

        let process = std::thread::spawn(move || {
            let result = self
                .processor
                .process_with_cancellation_token(cancellation_token);
            drop(progress_sender);
            drop(error_sender);
            drop(warning_sender);
            result
        });

        message_loop.join().unwrap();
        if !silent {
            if let Ok(result) = complete_receiver.recv() {
                println!(" {:?} result: {:?}", process_type, result);
            }
        }

        process.join().unwrap()
    }
}