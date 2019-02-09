use std::path::Path;
use std::io::{Error, ErrorKind};

use clap::{
    App, AppSettings, Arg, crate_name, crate_description, crate_version
};

use cancellation::{CancellationTokenSource};

use hshchk_lib::hash_file_process::HashFileProcessor;

fn run() -> Result<(), Box<::std::error::Error>> {
    let app = App::new(crate_name!())
        .setting(AppSettings::ColorAuto)
        .setting(AppSettings::ColoredHelp)
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::UnifiedHelpMessage)
        .version(crate_version!())
        .about(crate_description!())
        .arg(Arg::with_name("directory")
            .help("Target directory. \
            Either create a checksum file or verify files in target directory. \
            If not specified, use current directory. The presence or absence of \
            a checksum file in target directory dictates the operating mode."))
        .arg(Arg::with_name("hashtype")
            .short("t")
            .long("hashtype")
            .takes_value(true)
            .value_name("type")
            .possible_values(&hshchk_lib::get_hash_types())
            .case_insensitive(true)
            .help("Hash function type"))
        .arg(Arg::with_name("create")
            .short("c")
            .long("create")
            .help("Force create mode and overwrite checksum file if it exists"));

    let matches = app.get_matches_safe()?;

    let target_path = match matches.value_of("directory") {
        Some(directory_name) => directory_name,
        None => ".",
    };

    let directory_path = Path::new(&target_path);
    if !directory_path.is_dir() {
        return Err(Box::new(Error::new(ErrorKind::Other, "The specified directory doesn't exist.")));
    }

    let hash_type_arg = matches.value_of("hashtype").unwrap_or("SHA1").to_uppercase();
    let hash_type = hshchk_lib::get_hash_type_from_str(&hash_type_arg);
    let force_create = matches.is_present("create");

    let cts = CancellationTokenSource::new();
    let cancellation_token = cts.token();
    let processor_cancellation_token = cancellation_token.clone();

    ctrlc::set_handler(move || { cts.cancel(); }).expect("Error setting Ctrl-C handler");

    let mut processor = HashFileProcessor::new(hash_type, target_path, force_create);
    let process_type = processor.get_process_type();
    processor.set_progress_event_handler(
        Box::new(|args| println!("Processing {}", args.relative_file_path)));

    let result = processor.process(&processor_cancellation_token);

    println!("{:?} result: {:?}", process_type, result);

    Ok(())
}

fn main() {
    #[cfg(windows)]
    let _ = ansi_term::enable_ansi_support(); // for Windows

    let result = run();

    if let Err(err) = result {
        if let Some(clap_err) = err.downcast_ref::<clap::Error>() {
            eprint!("{}", clap_err); // clap errors already have newlines

            match clap_err.kind {
                clap::ErrorKind::HelpDisplayed | clap::ErrorKind::VersionDisplayed => {
                    // the exit code should not indicate an error for --help / --version
                    std::process::exit(0)
                }
                _ => (),
            }
        } else {
            eprintln!("Error: {}", err);
        }

        std::process::exit(1);
    }
}