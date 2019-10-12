use cancellation::CancellationTokenSource;
use clap::{crate_description, crate_name, crate_version, App, AppSettings, Arg};
use hshchk_lib::hash_file_process::{
    HashFileProcessOptions, HashFileProcessResult, HashFileProcessor,
};
use num_format::{Locale, ToFormattedString};
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn (::std::error::Error)>> {
    let app = App::new(crate_name!())
        .setting(AppSettings::ColorAuto)
        .setting(AppSettings::ColoredHelp)
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::UnifiedHelpMessage)
        .version(crate_version!())
        .about(crate_description!())
        .arg(Arg::with_name("directory").help(
            "Target directory. \
             Either create a checksum file or verify files in target directory. \
             If not specified, use current directory. The presence or absence of \
             a checksum file in target directory dictates the operating mode.",
        ))
        .arg(
            Arg::with_name("hashtype")
                .short("t")
                .long("hashtype")
                .takes_value(true)
                .value_name("type")
                .possible_values(&hshchk_lib::get_hash_types())
                .case_insensitive(true)
                .help("Hash function type"),
        )
        .arg(
            Arg::with_name("create")
                .short("c")
                .long("create")
                .help("Force create mode and overwrite checksum file if it exists"),
        )
        .arg(
            Arg::with_name("size")
                .short("f")
                .long("size")
                .help("Check file size only"),
        )
        .arg(
            Arg::with_name("extra")
                .short("r")
                .long("extra")
                .help("Report extra files"),
        )
        .arg(
            Arg::with_name("silent")
                .short("s")
                .long("silent")
                .help("Don't output to stdout"),
        )
        .arg(
            Arg::with_name("match")
                .short("m")
                .long("match")
                .takes_value(true)
                .value_name("pattern")
                .help("Process files that matches pattern"),
        )
        .arg(
            Arg::with_name("ignore")
                .short("i")
                .long("ignore")
                .takes_value(true)
                .value_name("pattern")
                .help("Ignore files that matches pattern"),
        );

    let matches = app.get_matches_safe()?;
    let directory = match matches.value_of("directory") {
        Some(directory) => directory,
        None => ".",
    };
    let target_path = PathBuf::from(&directory);
    if !target_path.is_dir() {
        return Err(Box::new(Error::new(
            ErrorKind::Other,
            "The specified directory doesn't exist.",
        )));
    }

    let hash_type = hshchk_lib::get_hash_type_from_str(
        &matches
            .value_of("hashtype")
            .unwrap_or("SHA1")
            .to_uppercase(),
    );
    let cts = CancellationTokenSource::new();
    let main_cancellation_token = cts.token();
    let processor_cancellation_token = main_cancellation_token.clone();

    ctrlc::set_handler(move || {
        cts.cancel();
    })
    .expect("Failed to set Ctrl-C handler.");

    let mut processor = HashFileProcessor::new_with_options(HashFileProcessOptions {
        base_path: target_path,
        hash_type: Some(hash_type),
        force_create: Some(matches.is_present("create")),
        report_extra_files: Some(matches.is_present("extra")),
        check_file_size_only: Some(matches.is_present("size")),
        match_pattern: matches.value_of("match"),
        ignore_pattern: matches.value_of("ignore"),
    });

    processor.set_error_event_handler(Box::new(|error| {
        eprintln!("{} => {:?}", error.file_path.display(), error.state)
    }));

    processor.set_warning_event_handler(Box::new(|warning| {
        eprintln!("{} => {:?}", warning.file_path.display(), warning.state)
    }));

    if !matches.is_present("silent") {
        processor.set_progress_event_handler(Box::new(|args| {
            if args.bytes_processed == 0 {
                println!(
                    "Processing {} ({})",
                    args.file_path.display(),
                    args.file_size.to_formatted_string(&Locale::en)
                );
            }
        }));
        let process_type = processor.get_process_type();
        processor.set_complete_event_handler(Box::new(move |result| {
            println!("{:?} result: {:?}", process_type, result);
        }));
    }

    match processor.process_with_cancellation_token(&processor_cancellation_token) {
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
    // Enable ANSI support for Windows
    #[cfg(windows)]
    let _ = ansi_term::enable_ansi_support();

    let result = run();

    if let Err(error) = result {
        if let Some(clap_error) = error.downcast_ref::<clap::Error>() {
            eprint!("{}", clap_error); // `clap` errors already have newlines

            match clap_error.kind {
                clap::ErrorKind::HelpDisplayed | clap::ErrorKind::VersionDisplayed => {
                    // The exit code should not indicate an error for --help / --version
                    std::process::exit(0)
                }
                _ => (),
            }
        } else {
            eprintln!("Error: {}", error);
        }

        std::process::exit(1);
    }
}
