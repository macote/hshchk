use cancellation::CancellationToken;
use crossbeam::channel::{select, unbounded};
use std::sync::Arc;

use crate::hash_file_process::{
    FileProgress, HashFileProcessResult, HashFileProcessType, HashFileProcessor,
};
use crate::output::Output;
use std::path::PathBuf;

pub struct UI {
    processor: HashFileProcessor,
    silent: bool,
    report_file_path: Option<PathBuf>,
}

impl UI {
    pub fn new(
        processor: HashFileProcessor,
        silent: bool,
        report_file_path: Option<PathBuf>,
    ) -> UI {
        UI {
            processor,
            silent,
            report_file_path,
        }
    }
    pub fn run(
        mut self,
        cancellation_token: Arc<CancellationToken>,
        process_type: HashFileProcessType,
    ) -> HashFileProcessResult {
        let silent_console = self.silent; // Renamed to avoid conflict if self.silent is captured by closures
        let report_path_clone = self.report_file_path.clone(); // Clone for the message loop thread

        let (error_sender, error_receiver) = unbounded();
        let (warning_sender, warning_receiver) = unbounded();
        let (progress_sender, progress_receiver) = unbounded();
        let (complete_sender, complete_receiver) = unbounded();

        self.processor.set_error_event_sender(error_sender.clone());
        self.processor
            .set_warning_event_sender(warning_sender.clone());

        // Progress and complete events are only sent if not in silent mode for console
        // OR if a report file is specified (they will be handled by Output struct).
        if !silent_console || self.report_file_path.is_some() {
            self.processor
                .set_progress_event_sender(progress_sender.clone());
            self.processor
                .set_complete_event_sender(complete_sender.clone());
        } else {
            // If console is silent and no report file, drop senders so processor doesn't wait.
            drop(progress_sender);
            drop(complete_sender);
        }

        let message_loop = std::thread::spawn(move || {
            let mut output = Output::new(report_path_clone.as_deref(), silent_console);
            // If output initialization failed (e.g., can't open report file), it should have panicked or returned error.
            // For simplicity here, assuming Output::new handles critical errors or we'd need error propagation.

            let mut error_sender_dropped = false;
            let mut warning_sender_dropped = false;
            // Progress sender is effectively dropped if not silent_console and no report file
            let mut progress_sender_dropped = silent_console && report_path_clone.is_none();
            let mut senders_dropped = false;
            let mut skip_processed_message_for_console = false;
            let mut file_progress = FileProgress {
                ..Default::default()
            };

            output.write_init(); // Will write to file or console based on Output's internal state
            while !senders_dropped {
                select! {
                    recv(progress_receiver) -> msg => {
                        if let Ok(args) = msg {
                            if args.bytes_processed == 0 { // Start of a new file
                                if file_progress.file_path != "" && !skip_processed_message_for_console {
                                    output.write_processed(&file_progress.file_path);
                                }
                                skip_processed_message_for_console = false;
                                file_progress = FileProgress { ..args };
                            } else { // Continuing progress on the current file
                                file_progress.bytes_processed = args.bytes_processed;
                            }
                            output.write_progress(&file_progress);
                        } else {
                            progress_sender_dropped = true;
                        }
                    },
                    recv(error_receiver) -> msg => {
                        if let Ok(error) = msg {
                            skip_processed_message_for_console = true;
                            output.write_error(&error);
                        } else {
                            error_sender_dropped = true;
                        }
                    },
                    recv(warning_receiver) -> msg => {
                        if let Ok(warning) = msg {
                            skip_processed_message_for_console = true;
                            // write_error is used for warnings too in the original code for Output
                            output.write_error(&warning);
                        } else {
                            warning_sender_dropped = true;
                        }
                    }
                }
                senders_dropped = progress_sender_dropped && error_sender_dropped && warning_sender_dropped;
            }

            // Write final "processed" message for the last file, if applicable
            if !skip_processed_message_for_console && file_progress.file_path != "" {
                 output.write_processed(&file_progress.file_path);
            }
        });

        let processor_report_file_path = self.report_file_path.clone(); // For processor thread's Output instance
        let processor_silent_console = self.silent; // For processor thread's Output instance

        let process = std::thread::spawn(move || {
            let result = self
                .processor
                .process_with_cancellation_token(cancellation_token);
            drop(error_sender);
            drop(warning_sender);
            // progress_sender is potentially dropped earlier if silent and no report file
            if !(!silent_console || self.report_file_path.is_some()) {
                 // Manually drop if it wasn't passed to processor
            } else {
                drop(progress_sender); // Dropped by processor if it was passed
            }
            result
        });

        message_loop.join().unwrap();

        // Final result reporting
        // Uses its own Output instance to ensure it's written even if message_loop's Output is specific.
        // This is important if the message_loop's output.write_result was conditional.
        let mut final_output = Output::new(self.report_file_path.as_deref(), self.silent);

        if let Ok(result) = complete_receiver.recv() {
             if result == HashFileProcessResult::Canceled {
                // Only clear line if not silent for console and no report file (i.e. TTY active)
                if !self.silent && self.report_file_path.is_none() {
                    final_output.clear_line();
                } else {
                    // If there's a report file or silent console, just write the "Canceled" status normally.
                    let process_type_str = match process_type {
                        HashFileProcessType::Create => "Create",
                        HashFileProcessType::Verify => "Verify",
                    };
                    final_output.write_result(process_type_str, &result);
                }
            } else {
                let process_type_str = match process_type {
                    HashFileProcessType::Create => "Create",
                    HashFileProcessType::Verify => "Verify",
                };
                final_output.write_result(process_type_str, &result);
            }
        } else {
            // If complete_receiver failed, it might be because it was dropped (silent+no report file)
            // Or an actual error. If the process itself returned an error, it will be in 'process.join().unwrap()'
        }

        // If complete_sender was dropped early (silent console, no report file),
        // then complete_receiver.recv() would fail. We still need the result from the process thread.
        // However, the current logic in hshchk.rs already handles the result from process.join().unwrap()
        // for final error reporting to stderr. This section is more about ensuring the "result" line
        // from write_result makes it to the correct output.

        // The `complete_sender` is dropped by the processor thread if it was passed.
        // If it was not passed (silent console, no report file), it's dropped when UI::run scope ends for it.

        process.join().unwrap()
    }
}
