use crate::hash_file_process::{HashFileProcessResult, HashFileProcessType, HashFileProcessor};
use crate::output::Output;
use crossbeam::channel::{never, select, unbounded};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

pub struct UI {
    processor: HashFileProcessor,
    silent: bool,
    report_file: Option<PathBuf>,
    progress: Option<bool>,
}

impl UI {
    pub fn new(processor: HashFileProcessor, silent: bool) -> Self {
        Self {
            processor,
            silent,
            report_file: None,
            progress: None,
        }
    }
    pub fn with_report_file(mut self, path: Option<PathBuf>) -> Self {
        self.report_file = path;
        self
    }
    pub fn with_progress(mut self, progress: Option<bool>) -> Self {
        self.progress = progress;
        self
    }

    pub fn run(
        mut self,
        token: CancellationToken,
        mode: HashFileProcessType,
    ) -> HashFileProcessResult {
        // Validate before opening: the report must never append to the manifest.
        if let Some(path) = &self.report_file {
            if let Err(error) = self.processor.exclude_output_file(path) {
                eprintln!("Cannot use report '{}': {}", path.display(), error);
                return HashFileProcessResult::Error;
            }
        }
        let output = match Output::new(self.report_file.as_deref(), self.silent, self.progress) {
            Ok(output) => output,
            Err(error) => {
                eprintln!("Cannot open report: {}", error);
                return HashFileProcessResult::Error;
            }
        };
        let (errors, mut error_receiver) = unbounded();
        let (warnings, mut warning_receiver) = unbounded();
        let (progress, mut progress_receiver) = unbounded();
        self.processor.set_error_event_sender(errors);
        self.processor.set_warning_event_sender(warnings);
        if !self.silent || self.report_file.is_some() {
            self.processor.set_progress_event_sender(progress.clone());
        }
        drop(progress);
        let (flush_sender, mut flush_receiver) =
            unbounded::<crossbeam::channel::Sender<std::io::Result<()>>>();
        self.processor.set_before_save(move || {
            let (reply, response) = unbounded();
            flush_sender
                .send(reply)
                .map_err(|_| std::io::Error::other("output worker stopped"))?;
            response
                .recv()
                .map_err(|_| std::io::Error::other("output worker stopped"))?
        });
        let renderer = std::thread::spawn(move || -> std::io::Result<Output> {
            let mut output = output;
            let mut active = 4;
            while active > 0 {
                select! {
                    recv(flush_receiver) -> message => match message {
                        Ok(reply) => {
                            // Hashing has stopped producing events. Drain all queues
                            // before acknowledging that report output is writable.
                            for entry in error_receiver.try_iter() { output.write_diagnostic(&entry, false)?; }
                            for entry in warning_receiver.try_iter() { output.write_diagnostic(&entry, true)?; }
                            for progress in progress_receiver.try_iter() { output.write_progress(&progress)?; }
                            let _ = reply.send(output.flush());
                        },
                        Err(_) => { flush_receiver = never(); active -= 1; }
                    },
                    recv(error_receiver) -> message => match message {
                        Ok(entry) => output.write_diagnostic(&entry, false)?,
                        Err(_) => { error_receiver = never(); active -= 1; }
                    },
                    recv(warning_receiver) -> message => match message {
                        Ok(entry) => output.write_diagnostic(&entry, true)?,
                        Err(_) => { warning_receiver = never(); active -= 1; }
                    },
                    recv(progress_receiver) -> message => match message {
                        Ok(progress) => output.write_progress(&progress)?,
                        Err(_) => { progress_receiver = never(); active -= 1; }
                    },
                }
            }
            Ok(output)
        });
        let result = self.processor.process_with_cancellation_token(token);
        let stats = self.processor.stats().clone();
        let dry_run = self.processor.is_dry_run();
        drop(self.processor);
        match renderer.join() {
            Ok(Ok(mut output)) => {
                if let Err(error) = output.write_result(mode, result, &stats, dry_run) {
                    eprintln!("Cannot write output: {}", error);
                    return HashFileProcessResult::Error;
                }
            }
            Ok(Err(error)) => {
                eprintln!("Cannot write output: {}", error);
                return HashFileProcessResult::Error;
            }
            Err(_) => {
                eprintln!("Output worker failed");
                return HashFileProcessResult::Error;
            }
        }
        result
    }
}
