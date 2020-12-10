use num_format::{Locale, ToFormattedString};
use std::io::{stdout, Write};
use std::iter::repeat;
use std::time::Instant;
use unicode_segmentation::UnicodeSegmentation;

use hshchk_lib::hash_file_process::{FileProcessEntry, FileProgress};

use crate::speed::get_speed;
use crate::tty::terminal_size;

const OUTPUT_REFRESH_IN_MILLIS: u32 = 233;

pub struct Output {
    output_width: usize,
    refresh_rate_in_millis: u32,
    last_output_instant: Option<Instant>,
    last_output_file_progress: FileProgress,
}

impl Output {
    pub fn new() -> Self {
        let (output_width, _) = terminal_size().unwrap();
        Output {
            output_width: (output_width.0 - 1) as usize,
            refresh_rate_in_millis: OUTPUT_REFRESH_IN_MILLIS,
            last_output_instant: None,
            last_output_file_progress: FileProgress {
                ..Default::default()
            },
        }
    }
    fn pad_line(&self, line: String) -> String {
        let mut padded_line = line.clone();
        let line_len = line.graphemes(true).count();
        if line_len < self.output_width {
            let gap = self.output_width - line_len;
            let pad = &repeat(" ").take(gap).collect::<String>();
            padded_line = line + pad;
        }

        padded_line
    }
    fn write(
        &mut self,
        file_path: &str,
        file_size: u64,
        bytes_processed: u64,
        info: &str,
        new_line: bool,
        error: bool,
    ) {
        let now = Instant::now();
        let ellapsed_millis = match self.last_output_instant {
            Some(instant) => now.duration_since(instant).as_millis(),
            _ => 0,
        };

        if error || new_line || ellapsed_millis > self.refresh_rate_in_millis.into() {
            let mut info_output = String::new();
            if error {
                info_output = format!(" => {}", info);
            } else if self.last_output_file_progress.file_path == file_path {
                if bytes_processed != self.last_output_file_progress.bytes_processed {
                    let percent = match file_size {
                        0 => 100,
                        _ => bytes_processed * 100 / file_size,
                    };
                    let speed = get_speed(
                        bytes_processed,
                        self.last_output_file_progress.bytes_processed,
                        ellapsed_millis,
                    );

                    info_output = format!(
                        " ({}; {} %; {} {})",
                        file_size.to_formatted_string(&Locale::en),
                        percent.to_formatted_string(&Locale::en),
                        speed.bytes_per_interval.to_formatted_string(&Locale::en),
                        speed.unit
                    );
                }
            }

            let printed_file_path: String;
            let file_path_max_size = self.output_width - info_output.len();
            let mut file_path_graphemes = file_path.graphemes(true);
            let file_path_len = file_path_graphemes.clone().count();
            if file_path_max_size < file_path_len {
                let offset = file_path_len - file_path_max_size + "..".len();
                for _ in 0..offset {
                    file_path_graphemes.next();
                }

                printed_file_path = format!("{}{}", "..", file_path_graphemes.as_str());
            } else {
                printed_file_path = file_path.to_owned();
            }

            let line_output = self.pad_line(format!("{}{}", printed_file_path, info_output));
            if error {
                eprintln!(" {}\r", line_output);
            } else if new_line {
                println!(" {}\r", line_output);
            } else {
                print!(" {}\r", line_output);
            }

            stdout().flush().unwrap();
            self.last_output_instant = Some(Instant::now());
            self.last_output_file_progress = FileProgress {
                file_path: file_path.into(),
                file_size,
                bytes_processed,
            };
        }
    }
    pub fn write_init(&mut self) {
        print!(" Opening files...\r");
        stdout().flush().unwrap();
        self.last_output_instant = Some(Instant::now());
    }
    pub fn write_error(&mut self, file_process_entry: &FileProcessEntry) {
        self.write(
            file_process_entry.file_path.to_str().unwrap(),
            0,
            0,
            &format!("{:?}", file_process_entry.state),
            true,
            true,
        );
    }
    pub fn write_progress(&mut self, file_progress: &FileProgress) {
        self.write(
            &file_progress.file_path,
            file_progress.file_size,
            file_progress.bytes_processed,
            &file_progress.file_size.to_formatted_string(&Locale::en),
            false,
            false,
        );
    }
    pub fn write_processed(&mut self, file_path: &str) {
        self.write(file_path, 0, 0, "", false, false);
    }
    pub fn write_result(&self, result: String) {
        println!("{}\r", self.pad_line(result));
    }
    pub fn clear_line(&self) {
        print!("{}\r", self.pad_line("".into()));
        stdout().flush().unwrap();
    }
}
