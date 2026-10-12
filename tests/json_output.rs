use hshchk::hash_file_process::*;
use hshchk::report;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

fn cli(mode: &str, root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hshchk"))
        .arg(mode)
        .arg(root)
        .args(["--output", "json"])
        .args(args)
        .output()
        .unwrap()
}
fn document(output: Output, code: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.stdout.contains(&b'\r'));
    assert!(!output.stdout.contains(&0x1b));
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 1, "{}", text);
    let value: Value = serde_json::from_str(&text).unwrap();
    validate(&value);
    assert_eq!(value["exit_code"], code);
    value
}
fn validate(value: &Value) {
    let schema: Value =
        serde_json::from_str(include_str!("../docs/json-output-v1.schema.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(
        validator.is_valid(value),
        "report does not match schema: {}",
        value
    );
}
fn input() -> TempDir {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    root
}

#[test]
fn successful_create_and_verify_are_one_schema_valid_record() {
    let root = input();
    let created = document(cli("create", root.path(), &[]), 0);
    assert_eq!(created["operation"], "create");
    assert_eq!(created["manifest_written"], true);
    assert_eq!(created["stats"]["bytes_hashed"], 4);
    assert!(created["diagnostics"].as_array().unwrap().is_empty());
    let verified = document(cli("verify", root.path(), &["--silent"]), 0);
    assert_eq!(verified["operation"], "verify");
    assert_eq!(verified["manifest_written"], false);
}

#[test]
fn mismatches_missing_files_and_warnings_keep_distinct_statuses() {
    let root = input();
    document(cli("create", root.path(), &[]), 0);
    fs::write(root.path().join("file"), "tada").unwrap();
    let mismatch = document(cli("verify", root.path(), &[]), 1);
    assert_eq!(mismatch["status"], "verification_failed");
    assert_eq!(mismatch["diagnostics"][0]["kind"], "incorrect_hash");
    fs::remove_file(root.path().join("file")).unwrap();
    let missing = document(cli("verify", root.path(), &[]), 1);
    assert_eq!(missing["diagnostics"][0]["kind"], "missing");
    fs::write(root.path().join("file"), "data").unwrap();
    fs::write(root.path().join("extra"), "extra").unwrap();
    let warning = document(cli("verify", root.path(), &["--extra"]), 0);
    assert_eq!(warning["diagnostics"][0]["level"], "warning");
    assert_eq!(warning["stats"]["extra_files"], 1);
}

#[test]
fn update_dry_run_and_report_output_do_not_contaminate_json() {
    let root = input();
    document(cli("create", root.path(), &[]), 0);
    let original = fs::read(root.path().join("SHA1SUMS")).unwrap();
    fs::write(root.path().join("file"), "tada").unwrap();
    let report_file = root.path().join("report.txt");
    let preview = document(
        cli(
            "update",
            root.path(),
            &[
                "--dry-run",
                "--report-file",
                report_file.to_str().unwrap(),
                "--progress",
                "always",
            ],
        ),
        0,
    );
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["manifest_written"], false);
    assert_eq!(preview["stats"]["files_updated"], 1);
    assert_eq!(fs::read(root.path().join("SHA1SUMS")).unwrap(), original);
    assert!(fs::read_to_string(&report_file)
        .unwrap()
        .contains("dry run"));
    let updated = document(
        cli(
            "update",
            root.path(),
            &["--report-file", report_file.to_str().unwrap()],
        ),
        0,
    );
    assert_eq!(updated["manifest_written"], true);
    assert_eq!(updated["stats"]["files_processed"], 1);
}

#[test]
fn empty_and_malformed_manifest_failures_match_the_schema() {
    let root = TempDir::new().unwrap();
    let empty = document(cli("create", root.path(), &[]), 2);
    assert_eq!(empty["status"], "no_files_processed");
    fs::write(root.path().join("SHA1SUMS"), "malformed ").unwrap();
    let malformed = document(cli("verify", root.path(), &[]), 2);
    assert_eq!(malformed["status"], "error");
    assert_eq!(malformed["diagnostics"][0]["kind"], "error");
}

#[test]
fn setup_errors_and_usage_errors_are_still_json() {
    let root = input();
    let setup = document(cli("verify", root.path(), &[]), 2);
    assert_eq!(setup["operation"], "verify");
    assert_eq!(setup["diagnostics"][0]["kind"], "configuration_error");
    let usage = document(cli("create", root.path(), &["--unknown-option"]), 2);
    assert!(usage["operation"].is_null());
    assert_eq!(usage["diagnostics"][0]["kind"], "usage_error");
    let bad_regex = document(cli("create", root.path(), &["--ignore", "["]), 2);
    assert_eq!(bad_regex["status"], "error");
    let bad_report = document(
        cli(
            "create",
            root.path(),
            &["--report-file", root.path().to_str().unwrap()],
        ),
        2,
    );
    assert_eq!(bad_report["status"], "error");
    assert!(!root.path().join("SHA1SUMS").exists());
    let invalid_directory = document(cli("create", &root.path().join("missing"), &[]), 2);
    assert_eq!(invalid_directory["status"], "error");
}

#[test]
fn quoted_unicode_paths_and_messages_are_escaped_and_parseable() {
    let root = input();
    fs::rename(root.path().join("file"), root.path().join("café file")).unwrap();
    document(cli("create", root.path(), &[]), 0);
    fs::write(root.path().join("café file"), "tada").unwrap();
    let mismatch = document(cli("verify", root.path(), &[]), 1);
    assert_eq!(mismatch["diagnostics"][0]["path"], "café file");
    let message = "quote \" and backslash \\ and newline\n";
    let value = report::error_report(None, false, true, message);
    let mut bytes = Vec::new();
    report::write_json(&value, &mut bytes).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    validate(&value);
    assert_eq!(value["diagnostics"][0]["message"], message);
}

#[test]
fn canceled_library_outcome_has_a_schema_valid_result() {
    let root = input();
    let mut processor = HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.path().into(),
        ..Default::default()
    })
    .unwrap();
    let token = CancellationToken::new();
    token.cancel();
    let value = report::json_report(&processor.process_outcome_with_cancellation_token(token));
    validate(&value);
    assert_eq!(value["status"], "canceled");
    assert_eq!(value["exit_code"], 130);
    assert_eq!(value["manifest_written"], false);
}

#[test]
fn help_version_and_completion_scripts_remain_text_auxiliary_output() {
    for flags in [["--help", "--output=json"], ["--version", "--output=json"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_hshchk"))
            .args(flags)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(serde_json::from_slice::<Value>(&output.stdout).is_err());
    }
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let output = Command::new(env!("CARGO_BIN_EXE_hshchk"))
            .args(["completions", shell])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            shell,
            String::from_utf8_lossy(&output.stderr)
        );
        let script = String::from_utf8(output.stdout).unwrap();
        assert!(script.contains("hshchk"));
        assert!(script.contains("checksum-file"));
        assert!(script.contains("output"));
    }
}

#[test]
fn output_options_after_double_dash_are_positional_not_json_requests() {
    let root = input();
    let output = Command::new(env!("CARGO_BIN_EXE_hshchk"))
        .arg("--")
        .arg(root.path())
        .args(["--output", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn non_unicode_diagnostic_paths_are_explicitly_marked_lossy() {
    use std::os::unix::ffi::OsStringExt;
    let root = TempDir::new().unwrap();
    let name = std::ffi::OsString::from_vec(vec![b'f', 0xff]);
    fs::write(root.path().join(name), "data").unwrap();
    let value = document(cli("create", root.path(), &[]), 2);
    assert_eq!(value["diagnostics"][0]["path_is_lossy"], true);
    assert_eq!(value["diagnostics"][0]["kind"], "invalid_unicode_file_name");
}

#[cfg(unix)]
#[test]
fn non_unicode_paths_in_owned_outcomes_are_marked_lossy() {
    use std::os::unix::ffi::OsStringExt;
    // macOS rejects these names on disk, but owned diagnostics can still contain them.
    let outcome = ProcessOutcome {
        process_type: HashFileProcessType::Create,
        status: ProcessStatus::Error,
        stats: ProcessStats {
            errors: 1,
            ..ProcessStats::default()
        },
        diagnostics: vec![ProcessDiagnostic {
            level: DiagnosticLevel::Error,
            file_path: std::ffi::OsString::from_vec(vec![b'f', 0xff]).into(),
            state: FileProcessState::InvalidUnicodeFileName,
        }],
        dry_run: false,
        manifest_written: false,
    };
    let value = report::json_report(&outcome);
    validate(&value);
    assert_eq!(value["diagnostics"][0]["path"], "f\u{fffd}");
    assert_eq!(value["diagnostics"][0]["path_is_lossy"], true);
}

#[cfg(target_os = "linux")]
#[test]
fn report_write_failure_is_captured_before_the_json_result_is_emitted() {
    let root = input();
    document(cli("create", root.path(), &[]), 0);
    let manifest = root.path().join("SHA1SUMS");
    let original = fs::read(&manifest).unwrap();
    fs::write(root.path().join("file"), "tada").unwrap();
    let value = document(
        cli("update", root.path(), &["--report-file", "/dev/full"]),
        2,
    );
    assert_eq!(value["status"], "error");
    assert_eq!(value["manifest_written"], false);
    assert_eq!(fs::read(manifest).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn canceled_cli_emits_json_and_exit_130() {
    use std::io::Read;
    use std::process::Stdio;
    let root = TempDir::new().unwrap();
    let file = fs::File::create(root.path().join("file")).unwrap();
    file.set_len(512 * 1024 * 1024).unwrap();
    drop(file);
    let mut child = Command::new(env!("CARGO_BIN_EXE_hshchk"))
        .arg("create")
        .arg(root.path())
        .args(["--output", "json", "--progress", "always"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let mut observed = Vec::new();
    let mut byte = [0];
    while !observed.ends_with(b"file") {
        assert_ne!(stderr.read(&mut byte).unwrap(), 0);
        observed.push(byte[0]);
    }
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGINT) }, 0);
    let value = document(child.wait_with_output().unwrap(), 130);
    assert_eq!(value["status"], "canceled");
    assert_eq!(value["manifest_written"], false);
}
#[test]
fn redirected_results_and_progress_inside_target_are_excluded_from_hashing() {
    let root = input();
    let result_file = root.path().join("result.json");
    let progress_file = root.path().join("progress.txt");
    let output = Command::new(env!("CARGO_BIN_EXE_hshchk"))
        .arg("create")
        .arg(root.path())
        .args(["--output", "json", "--progress", "always"])
        .stdout(fs::File::create(&result_file).unwrap())
        .stderr(fs::File::create(&progress_file).unwrap())
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&fs::read(&result_file).unwrap()).unwrap();
    validate(&value);
    assert_eq!(value["stats"]["files_processed"], 1);
    assert_eq!(value["stats"]["bytes_hashed"], 4);
    let manifest = fs::read_to_string(root.path().join("SHA1SUMS")).unwrap();
    assert_eq!(manifest.lines().count(), 1);
    assert!(!manifest.contains("result.json"));
    assert!(!manifest.contains("progress.txt"));
}

#[cfg(target_os = "linux")]
#[test]
fn final_report_write_errors_are_reflected_in_json() {
    let root = input();
    document(cli("create", root.path(), &["--format", "hashcheck"]), 0);
    let value = document(
        cli(
            "verify",
            root.path(),
            &["--size", "--report-file", "/dev/full"],
        ),
        2,
    );
    assert_eq!(value["status"], "error");
    assert_eq!(value["stats"]["bytes_size_checked"], 4);
    assert_eq!(value["manifest_written"], false);
    assert!(value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["message"].as_str().unwrap().contains("final output")));
}
