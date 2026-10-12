use clap::{command, Arg, ArgAction, ArgMatches, Command};
use clap_complete::aot::{generate, Shell};
use hshchk::hash_file_process::{
    HashFileProcessOptions, HashFileProcessType, HashFileProcessor, ProcessOutcome,
};
use hshchk::report;
use hshchk::{get_hash_types, ui, HashFileFormat};
use std::ffi::OsString;
use std::io::{self, Write};
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
        .arg(Arg::new("output").long("output").value_parser(["text", "json"]).default_value("text")
            .help("Final result format; JSON is one versioned object, including failures"))
        .arg(Arg::new("silent").short('s').long("silent").action(ArgAction::SetTrue).help("Suppress progress and human summaries; retain diagnostics and requested JSON"))
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
        .subcommand(Command::new("completions").about("Print a shell completion script without processing files")
            .arg(Arg::new("shell").required(true).value_parser(clap::value_parser!(Shell)).ignore_case(true)))
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

fn process(
    matches: &ArgMatches,
    process_type: Option<HashFileProcessType>,
    explicit_command: bool,
    json: bool,
) -> Result<ProcessOutcome, Box<dyn std::error::Error>> {
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
        .with_stdout_summary(!json)
        .run_outcome(token))
}

// On parser failure there are no ArgMatches. Only recognize an explicit JSON
// request before `--`, so a directory or positional value cannot opt into JSON.
fn json_requested(args: &[OsString]) -> bool {
    let mut args = args.iter().skip(1).take_while(|arg| *arg != "--");
    while let Some(arg) = args.next() {
        if arg == "--output=json"
            || (arg == "--output" && args.next().is_some_and(|value| value == "json"))
        {
            return true;
        }
    }
    false
}

fn json_exit(value: &serde_json::Value, code: u8) -> ExitCode {
    if let Err(error) = report::write_json(value, io::stdout().lock()) {
        eprintln!("Cannot write JSON output: {}", error);
        return ExitCode::from(2);
    }
    ExitCode::from(code)
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    let matches = match cli().try_get_matches_from(&args) {
        Ok(matches) => matches,
        Err(error) => {
            let _ = error.print();
            let code = error.exit_code() as u8;
            if code != 0 && json_requested(&args) {
                return json_exit(
                    &report::error_report(None, false, true, &error.to_string()),
                    code,
                );
            }
            return ExitCode::from(code);
        }
    };
    if let Some(("completions", args)) = matches.subcommand() {
        let shell = *args.get_one::<Shell>("shell").unwrap();
        let mut script = Vec::new();
        generate(shell, &mut cli(), "hshchk", &mut script);
        if let Err(error) = io::stdout().lock().write_all(&script) {
            eprintln!("Cannot write completions: {}", error);
            return ExitCode::from(2);
        }
        return ExitCode::SUCCESS;
    }
    let (process_type, matches, explicit_command) = match matches.subcommand() {
        Some(("create", args)) => (Some(HashFileProcessType::Create), args, true),
        Some(("verify", args)) => (Some(HashFileProcessType::Verify), args, true),
        Some(("update", args)) => (Some(HashFileProcessType::Update), args, true),
        _ => (None, &matches, false),
    };
    let json = value(matches, "output") == Some("json");
    match process(matches, process_type, explicit_command, json) {
        Ok(outcome) => {
            let code = outcome.status.exit_code();
            if json {
                json_exit(&report::json_report(&outcome), code)
            } else {
                ExitCode::from(code)
            }
        }
        Err(error) => {
            eprintln!("{}", error);
            let mode = process_type.or_else(|| {
                if flag(matches, "create") {
                    Some(HashFileProcessType::Create)
                } else if flag(matches, "update") {
                    Some(HashFileProcessType::Update)
                } else {
                    None
                }
            });
            if json {
                json_exit(
                    &report::error_report(
                        mode,
                        flag(matches, "dry-run"),
                        false,
                        &error.to_string(),
                    ),
                    2,
                )
            } else {
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
