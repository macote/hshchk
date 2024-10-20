use clap::{command, Arg};
use std::io::{Error, ErrorKind};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

use hshchk::hash_file_process::{HashFileProcessOptions, HashFileProcessResult, HashFileProcessor};
use hshchk::{get_hash_types, ui};

fn run() -> Result<(), Box<dyn (::std::error::Error)>> {
    let matches = command!()
        .arg(Arg::new("directory").required(true).help(
            "Target directory. \
         Either create a checksum file or verify files in specified directory. \
         The presence or absence of a checksum file in target directory dictates \
         the operating mode.",
        ))
        .arg(
            Arg::new("type")
                .short('t')
                .long("type")
                .value_parser(get_hash_types())
                .ignore_case(true)
                .default_value("SHA1")
                .help("Hash type"),
        )
        .arg(
            Arg::new("create")
                .short('c')
                .long("create")
                .action(clap::ArgAction::SetTrue)
                .help("Force create mode and overwrite checksum file if it exists"),
        )
        .arg(
            Arg::new("size")
                .short('f')
                .long("size")
                .action(clap::ArgAction::SetTrue)
                .help("Check file size only"),
        )
        .arg(
            Arg::new("extra")
                .short('r')
                .long("extra")
                .action(clap::ArgAction::SetTrue)
                .help("Report extra files"),
        )
        .arg(
            Arg::new("silent")
                .short('s')
                .long("silent")
                .action(clap::ArgAction::SetTrue)
                .help("Don't output to stdout"),
        )
        .arg(
            Arg::new("match")
                .short('m')
                .long("match")
                .value_name("pattern")
                .help("Process files that match regex pattern"),
        )
        .arg(
            Arg::new("ignore")
                .short('i')
                .long("ignore")
                .value_name("pattern")
                .help("Ignore files that match regex pattern"),
        )
        .arg(
            Arg::new("hc")
                .short('u')
                .long("hc")
                .action(clap::ArgAction::SetTrue)
                .help("Use hshchk (e.g. hshchk.sha1) file format"),
        )
        .get_matches();

    let directory = matches.get_one::<String>("directory").unwrap();
    let target_path = PathBuf::from(&directory);
    if !target_path.is_dir() {
        return Err(Box::new(Error::new(
            ErrorKind::Other,
            "The specified directory doesn't exist.",
        )));
    }

    let hash_file_format = hshchk::get_hash_file_format_from_arg(matches.get_flag("hc"));
    let hash_type =
        hshchk::get_hash_type_from_str(&matches.get_one::<String>("type").unwrap().to_uppercase());

    let main_cancellation_token = CancellationToken::new();
    let cancellation_token = main_cancellation_token.clone();

    ctrlc::set_handler(move || {
        cancellation_token.cancel();
    })
    .expect("Failed to set Ctrl-C handler.");

    let processor = HashFileProcessor::new(HashFileProcessOptions {
        base_path: target_path,
        hash_file_format: Some(hash_file_format),
        hash_type: Some(hash_type),
        force_create: Some(matches.get_flag("create")),
        report_extra: Some(matches.get_flag("extra")),
        size_only: Some(matches.get_flag("size")),
        match_pattern: matches.get_one::<String>("match").map(String::as_str),
        ignore_pattern: matches.get_one::<String>("ignore").map(String::as_str),
    });

    let process_type = processor.get_process_type();
    let ui = ui::UI::new(processor, matches.get_flag("silent"));

    match ui.run(main_cancellation_token, process_type) {
        HashFileProcessResult::Error => Err(Box::new(Error::new(
            ErrorKind::Other,
            "The hash check process failed.",
        ))),
        HashFileProcessResult::Canceled => Err(Box::new(Error::new(
            ErrorKind::Interrupted,
            "The hash check process was canceled.",
        ))),
        HashFileProcessResult::NoFilesProcessed => Err(Box::new(Error::new(
            ErrorKind::NotFound,
            "No files were processed.",
        ))),
        HashFileProcessResult::Success => Ok(()),
    }
}

fn main() {
    if let Err(error) = run() {
        if let Some(clap_error) = error.downcast_ref::<clap::Error>() {
            eprint!("{}", clap_error); // `clap` errors already have newlines

            match clap_error.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    // The exit code should not indicate an error for --help / --version
                    std::process::exit(0)
                }
                _ => (),
            }
        } else {
            eprintln!("{}", error);
        }

        std::process::exit(1);
    }
}
