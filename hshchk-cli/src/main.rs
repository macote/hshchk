use std::io::{Error, ErrorKind};
use std::path::Path;

use clap::{crate_description, crate_name, crate_version, App, AppSettings, Arg};

use cancellation::CancellationTokenSource;

use hshchk_lib::hash_file_process::{HashFileProcessor, HashFileProcessResult};

fn run() -> Result<(), Box<::std::error::Error>> {
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
            Arg::with_name("silent")
                .short("s")
                .long("silent")
                .help("Don't output to stdout"),
        );

    let matches = app.get_matches_safe()?;

    let target_path = match matches.value_of("directory") {
        Some(directory_name) => directory_name,
        None => ".",
    };

    let directory_path = Path::new(&target_path);
    if !directory_path.is_dir() {
        return Err(Box::new(Error::new(
            ErrorKind::Other,
            "The specified directory doesn't exist.",
        )));
    }

    let hash_type_arg = matches
        .value_of("hashtype")
        .unwrap_or("SHA1")
        .to_uppercase();
    let hash_type = hshchk_lib::get_hash_type_from_str(&hash_type_arg);
    let force_create = matches.is_present("create");
    let silent = matches.is_present("silent");

    let cts = CancellationTokenSource::new();
    let main_cancellation_token = cts.token();
    let processor_cancellation_token = main_cancellation_token.clone();

    ctrlc::set_handler(move || {
        cts.cancel();
    })
    .expect("Failed to set Ctrl-C handler.");

    let result: HashFileProcessResult;
    let mut processor = HashFileProcessor::new(hash_type, target_path, force_create);

    processor.set_error_event_handler(Box::new(|error| {
        eprintln!(
            "{:?}: {:?}",
            error.file_path, error.state
        )
    }));

    if !silent {
        processor.set_progress_event_handler(Box::new(|args| {
            println!(
                "Processing {} ({}; {})",
                args.relative_file_path, args.file_size, args.bytes_processed
            )
        }));
        let process_type = processor.get_process_type();
        processor.set_complete_event_handler(Box::new(move |result| {
            println!(
                "{:?} result: {:?}",
                process_type, result
            )
        }));
    }

    result = processor.process(processor_cancellation_token);

    if result != HashFileProcessResult::Success {
        return Err(Box::new(Error::new(
            ErrorKind::Other,
            "The hash check process failed.",
        )));
    }

    Ok(())
}

fn main() {
    #[cfg(windows)]
    let _ = ansi_term::enable_ansi_support(); // for Windows

    let result = run();

    if let Err(error) = result {
        if let Some(clap_error) = error.downcast_ref::<clap::Error>() {
            eprint!("{}", clap_error); // clap errors already have newlines

            match clap_error.kind {
                clap::ErrorKind::HelpDisplayed | clap::ErrorKind::VersionDisplayed => {
                    // the exit code should not indicate an error for --help / --version
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
