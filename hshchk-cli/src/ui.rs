use cancellation::CancellationToken;
use crossbeam::crossbeam_channel::{select, unbounded, tick};
use hshchk_lib::hash_file_process::{
    HashFileProcessResult, HashFileProcessType, HashFileProcessor,
};
use num_format::{Locale, ToFormattedString};
use std::convert::TryInto;
use std::io::Write;
use std::io::stdout;
use std::iter::repeat;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::tty::{terminal_size};

static EMPTY_FILE_PATH: &str = "";
const PROCESS_OUTPUT_REFRESH_IN_MILLISECONDS: u64 = 222;

struct FileProgress {
    file_path: String,
    file_size: u64,
    file_bytes_processed: u64,
}

struct ProgressLine {
    output_width: usize,
    refresh_rate_in_ms: u32,
    last_output_instant: Instant,
}

impl ProgressLine {
    pub fn new() -> Self {
        let (output_width, _) = terminal_size().unwrap();
        ProgressLine {
            output_width: output_width.0 as usize,
            refresh_rate_in_ms: 666,
            last_output_instant: Instant::now()
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
    pub fn output_processed(&self, file_path: &str) {
        println!("{}", self.pad_line(format!("\rProcessed {}", file_path)));
        stdout().flush().unwrap();
    }
    pub fn output_progress(&mut self, file_progress: &FileProgress) {
        let now = Instant::now();
        if file_progress.file_bytes_processed == 0 {
            self.last_output_instant = now;
            print!("{}", self.pad_line(format!("\rProcessing {} ({})",
                file_progress.file_path,
                file_progress.file_size.to_formatted_string(&Locale::en)))
            );
            stdout().flush().unwrap();
        } else if now.duration_since(self.last_output_instant).as_millis() > self.refresh_rate_in_ms.into() {
            self.last_output_instant = now;
            print!("{}", self.pad_line(format!("\rProcessing {} ({}; {})",
                file_progress.file_path,
                file_progress.file_bytes_processed.to_formatted_string(&Locale::en),
                file_progress.file_size.to_formatted_string(&Locale::en)))
            );
            stdout().flush().unwrap();
        }
    }
}

pub struct UI {
    processor: HashFileProcessor,
    silent: bool,
}

impl UI {
    pub fn new(processor: HashFileProcessor, silent: bool) -> UI {
        UI {
            processor,
            silent
         }
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

        let message_loop = std::thread::spawn(move || {
            let mut progress_sender_dropped = false;
            let mut error_sender_dropped = false;
            let mut warning_sender_dropped = false;
            let mut senders_dropped = false;
            let mut progress_line = ProgressLine::new();
            let mut file_progress = FileProgress {
                file_path: String::from(""),
                file_size: 0,
                file_bytes_processed: 0,
            };
            let ticker = tick(Duration::from_millis(PROCESS_OUTPUT_REFRESH_IN_MILLISECONDS));

            while !senders_dropped {
                select! {
                    recv(ticker) -> _ => progress_line.output_progress(&file_progress),
                    recv(progress_receiver) -> msg => {
                        if let Ok(args) = msg {
                            if args.bytes_processed == 0 {
                                if file_progress.file_path != EMPTY_FILE_PATH {
                                    progress_line.output_processed(&file_progress.file_path);
                                }

                                file_progress.file_path = args.file_path;
                                file_progress.file_size = args.file_size;
                                file_progress.file_bytes_processed = 0;
                                progress_line.output_progress(&file_progress);
                            }
                            else {
                                file_progress.file_bytes_processed = args.bytes_processed;
                            }
                        }
                        else {
                            progress_sender_dropped = true;
                        }
                    },
                    recv(error_receiver) -> msg => {
                        if let Ok(error) = msg {
                            eprintln!("{} => {:?}", error.file_path.display(), error.state)
                        }
                        else {
                            error_sender_dropped = true;
                        }
                    },
                    recv(warning_receiver) -> msg => {
                        if let Ok(warning) = msg {
                            eprintln!("{} => {:?}", warning.file_path.display(), warning.state)
                        } else {
                            warning_sender_dropped = true;
                        }
                    }
                }

                senders_dropped = progress_sender_dropped && error_sender_dropped && warning_sender_dropped;
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
                println!("{:?} result: {:?}", process_type, result);
            }
        }

        process.join().unwrap()
    }
}
