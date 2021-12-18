use crossbeam::channel::{select, unbounded};
use tokio_util::sync::CancellationToken;

use crate::hash_file_process::{
    FileProgress, HashFileProcessResult, HashFileProcessType, HashFileProcessor,
};
use crate::output::Output;

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
        cancellation_token: CancellationToken,
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
            let mut error_sender_dropped = false;
            let mut warning_sender_dropped = false;
            let mut progress_sender_dropped = silent;
            let mut senders_dropped = false;
            let mut skip_processed = false;
            let mut output = Output::new();
            let mut file_progress = FileProgress {
                ..Default::default()
            };

            output.write_init();
            while !senders_dropped {
                select! {
                    recv(progress_receiver) -> msg => {
                        if let Ok(args) = msg {
                            if args.bytes_processed == 0 {
                                if !file_progress.file_path.is_empty() && !skip_processed {
                                    output.write_processed(&file_progress.file_path);
                                }

                                skip_processed = false;
                                file_progress = FileProgress { ..args };
                            }
                            else {
                                file_progress.bytes_processed = args.bytes_processed;
                            }

                            output.write_progress(&file_progress);
                        }
                        else {
                            progress_sender_dropped = true;
                        }
                    },
                    recv(error_receiver) -> msg => {
                        if let Ok(error) = msg {
                            skip_processed = true;
                            output.write_error(&error);
                        }
                        else {
                            error_sender_dropped = true;
                        }
                    },
                    recv(warning_receiver) -> msg => {
                        if let Ok(warning) = msg {
                            skip_processed = true;
                            output.write_error(&warning);
                        } else {
                            warning_sender_dropped = true;
                        }
                    }
                }

                senders_dropped =
                    progress_sender_dropped && error_sender_dropped && warning_sender_dropped;
            }

            if !silent && !skip_processed {
                output.write_processed(&file_progress.file_path);
            }
        });

        let process = std::thread::spawn(move || {
            let result = self
                .processor
                .process_with_cancellation_token(cancellation_token);
            drop(error_sender);
            drop(warning_sender);
            drop(progress_sender);
            result
        });

        message_loop.join().unwrap();
        if !silent {
            if let Ok(result) = complete_receiver.recv() {
                let output = Output::new();
                if result == HashFileProcessResult::Canceled {
                    output.clear_line();
                } else {
                    output.write_result(format!("{:?} result: {:?}", process_type, result));
                }
            }
        }

        drop(complete_sender);
        process.join().unwrap()
    }
}
