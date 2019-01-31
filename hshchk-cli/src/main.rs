use std::env;
use std::path::{Path, PathBuf};
use std::io::{Error, ErrorKind};

use clap::{
    App, AppSettings, Arg, arg_enum, crate_name, crate_description, crate_version, _clap_count_exprs
};

use cancellation::{CancellationTokenSource};

use hshchk_lib::{
    HashType, hash_file_process::{HashFileProcessor, HashFileProcessType}
};

arg_enum! {
    #[derive(PartialEq, Debug)]
    enum HashTypeArgument {
        SHA1,
        SHA256,
        SHA512,
        BLAKE2B,
        BLAKE2S,
    } 
}

fn hash_type_from_arg(hash_type_arg: &str) -> HashType {
    match hash_type_arg {
        "sha1" => HashType::SHA1,
        "sha256" => HashType::SHA256,
        "sha512" => HashType::SHA512,
        "blake2b" => HashType::BLAKE2B,
        "blake2s" => HashType::BLAKE2S,
        _ => panic!("Unsupported hash type.")
    }
}

fn run(bin_file_name: &str) -> Result<(), Box<::std::error::Error>> {
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
            .possible_values(&HashTypeArgument::variants())
            .case_insensitive(true)
            .help("Hash function type."))
        .arg(Arg::with_name("create")
            .short("c")
            .long("create")
            .help("Force create mode and overwrite checksum file if it exists."));

    let matches = app.get_matches_safe()?;

    let target_path = match matches.value_of("directory") {
        Some(directory_name) => directory_name,
        None => ".",
    };

    let directory_path = Path::new(&target_path);
    if !directory_path.is_dir() {
        return Err(Box::new(Error::new(ErrorKind::Other, "The specified directory doesn't exist.")));
    }

    let cts = CancellationTokenSource::new();
    let cancellation_token = cts.token();
    let processor_cancellation_token = cancellation_token.clone();

    ctrlc::set_handler(move || { cts.cancel(); }).expect("Error setting Ctrl-C handler");

    let hash_type_arg = matches.value_of("hashtype").unwrap_or("SHA1").to_lowercase();
    let hash_type = hash_type_from_arg(&hash_type_arg);
    let hash_file_name = format!("checksum.{}", hash_type_arg);

    let mut process_type = HashFileProcessType::Create;

    let hash_file_path: PathBuf = [target_path, &hash_file_name].iter().collect();
    if hash_file_path.is_file() {
        process_type = HashFileProcessType::Verify;
    }

    let mut processor = HashFileProcessor::new(
        process_type,
        hash_type,
        &hash_file_name,
        bin_file_name,
        target_path
    );
    processor.set_progress_event_handler(
        Box::new(|args| println!("processing {}", args.relative_file_path)));

    let result = processor.process(&processor_cancellation_token);

    println!("{:?} result: {:?}", process_type, result);

    Ok(())
}

fn main() {
    #[cfg(windows)]
    let _ = ansi_term::enable_ansi_support(); // for Windows

    let args = env::args().collect::<Vec<_>>();

    let bin_file_path = Path::new(&args[0]);
    let bin_file_name = bin_file_path.file_name().unwrap().to_str().unwrap();
    let result = run(bin_file_name);

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