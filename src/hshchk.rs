use clap::{command, Arg, ArgAction, ArgMatches, Command};
use hshchk::hash_file_process::{
    HashFileProcessOptions, HashFileProcessType, HashFileProcessor, ProcessOutcome,
};
use hshchk::{get_hash_types, ui, HashFileFormat};
use std::path::PathBuf;
use std::process::ExitCode;
use tokio_util::sync::CancellationToken;

fn common_args(command: Command) -> Command {
    command
        .arg(Arg::new("directory").required(true).value_parser(clap::value_parser!(PathBuf)).help("Target directory; manifest entries are relative to this directory"))
        .arg(Arg::new("type").short('t').long("type").visible_alias("algorithm").value_parser(get_hash_types()).ignore_case(true)
            .help("Creation algorithm or discovery filter; required to read a custom manifest name"))
        .arg(Arg::new("checksum-file").long("checksum-file").value_name("PATH").value_parser(clap::value_parser!(PathBuf))
            .help("Select a manifest; relative paths resolve against the target directory"))
        .arg(Arg::new("silent").short('s').long("silent").action(ArgAction::SetTrue).help("Suppress progress and stdout summaries; retain diagnostics"))
        .arg(Arg::new("match").short('m').long("match").value_name("PATTERN").help("Select relative paths matching this regex"))
        .arg(Arg::new("ignore").short('i').long("ignore").value_name("PATTERN").help("Exclude relative paths matching this regex"))
        .arg(Arg::new("report-file").long("report-file").value_parser(clap::value_parser!(PathBuf)).value_name("PATH")
            .help("Append diagnostics and a summary to a report; path is relative to the working directory"))
        .arg(Arg::new("progress").long("progress").value_parser(["auto", "always", "never"]).default_value("auto")
            .help("Live stderr progress; auto enables it only in a terminal"))
}

fn creation_args(command: Command) -> Command {
    command
        .arg(Arg::new("hc").short('u').long("hc").action(ArgAction::SetTrue).conflicts_with("format")
            .help("Create the legacy format with recorded file sizes (alias for --format hashcheck)"))
        .arg(Arg::new("format").long("format").value_parser(["sum", "hashcheck"])
            .help("Creation format [default: sum]; reading detects format from contents"))
}
fn verification_args(command: Command) -> Command {
    command
        .arg(
            Arg::new("size")
                .short('f')
                .long("size")
                .action(ArgAction::SetTrue)
                .help("Verify recorded sizes without hashing; requires a legacy manifest"),
        )
        .arg(
            Arg::new("extra")
                .short('r')
                .long("extra")
                .action(ArgAction::SetTrue)
                .help("Warn about files absent from the manifest"),
        )
}
fn update_args(command: Command, legacy: bool) -> Command {
    let remove = Arg::new("remove-missing")
        .long("remove-missing")
        .action(ArgAction::SetTrue)
        .help("Remove selected missing entries; preserve excluded entries");
    let dry_run = Arg::new("dry-run")
        .long("dry-run")
        .action(ArgAction::SetTrue)
        .help("Preview update counts without replacing the manifest");
    command
        .arg(if legacy {
            remove.requires("update")
        } else {
            remove
        })
        .arg(if legacy {
            dry_run.requires("update")
        } else {
            dry_run
        })
}
fn cli() -> Command {
    let root = creation_args(verification_args(update_args(
        common_args(command!()),
        true,
    )))
    .arg(
        Arg::new("create")
            .short('c')
            .long("create")
            .action(ArgAction::SetTrue)
            .conflicts_with_all(["update", "size", "extra"])
            .help("Legacy shortcut: force creation and authorize overwriting the manifest"),
    )
    .arg(
        Arg::new("update")
            .short('x')
            .long("update")
            .action(ArgAction::SetTrue)
            .conflicts_with_all(["size", "extra", "format", "hc"])
            .help("Legacy shortcut: update an existing manifest by rehashing selected files"),
    );
    root.subcommand_negates_reqs(true).args_conflicts_with_subcommands(true).propagate_version(true)
        .after_help("Examples:\n  hshchk create files --type SHA256\n  hshchk verify files --checksum-file SHA256SUMS\n  hshchk update files --dry-run --remove-missing\n  hshchk files  # compatibility shortcut: verify if a manifest exists, otherwise create")
        .subcommand(creation_args(common_args(Command::new("create").about("Create a checksum manifest; refuse replacement unless --overwrite is supplied")))
            .arg(Arg::new("overwrite").long("overwrite").action(ArgAction::SetTrue).help("Authorize replacing the selected manifest")))
        .subcommand(verification_args(common_args(Command::new("verify").about("Verify an existing checksum manifest"))))
        .subcommand(update_args(common_args(Command::new("update").about("Update an existing checksum manifest by rehashing selected files")), false))
}

fn flag(matches: &ArgMatches, name: &str) -> bool {
    matches
        .try_get_one::<bool>(name)
        .ok()
        .flatten()
        .copied()
        .unwrap_or(false)
}
fn value<'a>(matches: &'a ArgMatches, name: &str) -> Option<&'a str> {
    matches
        .try_get_one::<String>(name)
        .ok()
        .flatten()
        .map(String::as_str)
}

fn run() -> Result<ProcessOutcome, Box<dyn std::error::Error>> {
    let matches = cli().try_get_matches()?;
    let (process_type, matches, explicit_command) = match matches.subcommand() {
        Some(("create", args)) => (Some(HashFileProcessType::Create), args, true),
        Some(("verify", args)) => (Some(HashFileProcessType::Verify), args, true),
        Some(("update", args)) => (Some(HashFileProcessType::Update), args, true),
        _ => (None, &matches, false),
    };
    let hash_file_format = if flag(matches, "hc") || value(matches, "format") == Some("hashcheck") {
        HashFileFormat::HashCheck
    } else {
        HashFileFormat::HashSum
    };
    let token = CancellationToken::new();
    let cancellation = token.clone();
    ctrlc::set_handler(move || cancellation.cancel())?;
    let processor = HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: matches.get_one::<PathBuf>("directory").unwrap().clone(),
        hash_file_format: Some(hash_file_format),
        hash_type: value(matches, "type").map(hshchk::get_hash_type_from_str),
        force_create: Some(!explicit_command && flag(matches, "create")),
        report_extra: Some(flag(matches, "extra")),
        size_only: Some(flag(matches, "size")),
        match_pattern: value(matches, "match"),
        ignore_pattern: value(matches, "ignore"),
        update_mode: !explicit_command && flag(matches, "update"),
        remove_missing_in_update: flag(matches, "remove-missing"),
        dry_run: flag(matches, "dry-run"),
        process_type,
        checksum_file: matches.get_one::<PathBuf>("checksum-file").cloned(),
        overwrite: flag(matches, "overwrite"),
    })?;
    let progress = match value(matches, "progress") {
        Some("always") => Some(true),
        Some("never") => Some(false),
        _ => None,
    };
    Ok(ui::UI::new(processor, flag(matches, "silent"))
        .with_report_file(matches.get_one::<PathBuf>("report-file").cloned())
        .with_progress(progress)
        .run_outcome(token))
}

fn main() -> ExitCode {
    match run() {
        Ok(outcome) => ExitCode::from(outcome.status.exit_code()),
        Err(error) => {
            if let Some(error) = error.downcast_ref::<clap::Error>() {
                let _ = error.print();
                ExitCode::from(error.exit_code() as u8)
            } else {
                eprintln!("{}", error);
                ExitCode::from(2)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_argument_relationships_are_consistent() {
        cli().debug_assert();
    }
}
