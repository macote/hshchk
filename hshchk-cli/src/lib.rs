use cancellation::CancellationToken;
use crossbeam::crossbeam_channel::{select, unbounded};
use hshchk_lib::hash_file_process::{
    HashFileProcessResult, HashFileProcessType, HashFileProcessor,
};
use num_format::{Locale, ToFormattedString};
use std::sync::Arc;

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

        let message_loop = std::thread::spawn(move || {
            let mut progress_sender_dropped = false;
            let mut error_sender_dropped = false;
            let mut warning_sender_dropped = false;
            while !(progress_sender_dropped && error_sender_dropped && warning_sender_dropped) {
                select! {
                    recv(progress_receiver) -> msg => {
                        if let Ok(args) = msg {
                            if args.bytes_processed == 0 {
                                println!(
                                    "Processing {} ({})",
                                    args.file_path,
                                    args.file_size.to_formatted_string(&Locale::en)
                                );
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
