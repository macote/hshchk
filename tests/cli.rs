use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hshchk"))
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn redirected_output_and_summary_use_plain_complete_lines() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let output = cli(root.path(), &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(!output.stdout.contains(&b'\r'));
    assert!(!output.stdout.contains(&0x1b));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Create: Success\n"));
    assert!(text.contains("hashed: 4 bytes"));
    assert!(text.contains("Elapsed:"));
}

#[test]
fn silent_mode_finishes_and_still_reports_errors() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let output = cli(root.path(), &["--silent"]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    fs::write(root.path().join("file"), "tada").unwrap();
    let output = cli(root.path(), &["--silent"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("IncorrectHash"));
}

#[test]
fn report_inside_root_is_excluded_and_has_one_summary() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let report = root.path().join("report.txt");
    let output = cli(
        root.path(),
        &["--report-file", report.to_str().unwrap(), "--silent"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let manifest = fs::read_to_string(root.path().join("SHA1SUMS")).unwrap();
    assert_eq!(manifest.lines().count(), 1);
    assert!(!manifest.contains("report.txt"));
    let text = fs::read_to_string(&report).unwrap();
    assert_eq!(text.matches("Create: Success").count(), 1);
    assert!(text.contains("Hashed file (4 bytes)"));
    assert!(!text.contains('\r'));
    let output = cli(
        root.path(),
        &["--report-file", report.to_str().unwrap(), "--extra"],
    );
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn report_cannot_alias_manifest_even_through_a_hard_link() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    assert!(cli(root.path(), &[]).status.success());
    let manifest = root.path().join("SHA1SUMS");
    let alias = root.path().join("alias.txt");
    fs::hard_link(&manifest, &alias).unwrap();
    let before = fs::read(&manifest).unwrap();
    for report in [&manifest, &alias] {
        let output = cli(root.path(), &["--report-file", report.to_str().unwrap()]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("must differ"));
        assert_eq!(fs::read(&manifest).unwrap(), before);
    }
}

#[test]
fn report_open_failure_prevents_manifest_creation() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let output = cli(
        root.path(),
        &["--report-file", root.path().to_str().unwrap()],
    );
    assert!(!output.status.success());
    assert!(!root.path().join("SHA1SUMS").exists());
}

#[test]
fn invalid_regex_and_conflicting_modes_are_usage_errors() {
    let root = TempDir::new().unwrap();
    for args in [
        &["--match", "["][..],
        &["--create", "--update"],
        &["--remove-missing"],
        &["--dry-run"],
    ] {
        let output = cli(root.path(), args);
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
        assert!(!root.path().join("SHA1SUMS").exists());
    }
}

#[test]
fn update_without_manifest_fails_and_dry_run_preserves_it() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    assert!(!cli(root.path(), &["--update"]).status.success());
    assert!(!root.path().join("SHA1SUMS").exists());
    assert!(cli(root.path(), &[]).status.success());
    let manifest = root.path().join("SHA1SUMS");
    let before = fs::read(&manifest).unwrap();
    fs::write(root.path().join("file"), "tada").unwrap();
    let output = cli(root.path(), &["--update", "--dry-run"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("dry run; manifest unchanged"));
    assert_eq!(fs::read(&manifest).unwrap(), before);
    assert!(cli(root.path(), &["--update"]).status.success());
    assert!(cli(root.path(), &[]).status.success());
}

#[test]
fn size_only_rejects_manifests_without_sizes() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    assert!(cli(root.path(), &[]).status.success());
    let output = cli(root.path(), &["--size"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("recorded sizes"));
}

#[test]
fn forced_progress_is_on_stderr_and_can_be_disabled() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let forced = cli(root.path(), &["--progress", "always"]);
    assert!(forced.status.success());
    assert!(forced.stderr.contains(&b'\r'));
    assert!(String::from_utf8_lossy(&forced.stderr).contains("B/s)"));
    assert!(!forced.stdout.contains(&b'\r'));
    let disabled = cli(root.path(), &["--progress", "never"]);
    assert!(disabled.status.success());
    assert!(disabled.stderr.is_empty());
}
#[cfg(target_os = "linux")]
#[test]
fn report_write_failure_blocks_manifest_replacement() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    assert!(cli(root.path(), &[]).status.success());
    let manifest = root.path().join("SHA1SUMS");
    let original = fs::read(&manifest).unwrap();
    fs::write(root.path().join("file"), "tada").unwrap();
    let output = cli(
        root.path(),
        &["--update", "--report-file", "/dev/full", "--silent"],
    );
    assert!(!output.status.success());
    assert_eq!(fs::read(&manifest).unwrap(), original);
}
#[test]
fn every_supported_algorithm_creates_verifies_and_detects_edits() {
    for algorithm in hshchk::get_hash_types() {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("file"), "data").unwrap();
        let output = cli(root.path(), &["--type", algorithm, "--silent"]);
        assert!(
            output.status.success(),
            "{}: {}",
            algorithm,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            cli(root.path(), &["--silent"]).status.success(),
            "{}",
            algorithm
        );
        fs::write(root.path().join("file"), "tada").unwrap();
        assert!(
            !cli(root.path(), &["--silent"]).status.success(),
            "{}",
            algorithm
        );
    }
}
#[test]
fn cli_exit_codes_distinguish_mismatches_and_operational_errors() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    assert_eq!(cli(root.path(), &["--silent"]).status.code(), Some(0));
    fs::write(root.path().join("file"), "tada").unwrap();
    assert_eq!(cli(root.path(), &["--silent"]).status.code(), Some(1));
    fs::remove_file(root.path().join("file")).unwrap();
    assert_eq!(cli(root.path(), &["--silent"]).status.code(), Some(1));
    fs::write(root.path().join("SHA1SUMS"), "malformed ").unwrap();
    assert_eq!(cli(root.path(), &["--silent"]).status.code(), Some(2));
}

#[test]
fn usage_errors_empty_runs_and_invalid_paths_exit_two() {
    let root = TempDir::new().unwrap();
    assert_eq!(cli(root.path(), &["--silent"]).status.code(), Some(2));
    assert_eq!(cli(root.path(), &["--match", "["]).status.code(), Some(2));
    assert_eq!(
        cli(root.path(), &["--create", "--update"]).status.code(),
        Some(2)
    );
    assert_eq!(
        cli(&root.path().join("nonexistent"), &[]).status.code(),
        Some(2)
    );
    assert_eq!(
        cli(
            root.path(),
            &["--report-file", root.path().to_str().unwrap()]
        )
        .status
        .code(),
        Some(2)
    );
}

#[test]
fn help_and_version_exit_zero_without_a_directory() {
    for arg in ["--help", "--version"] {
        let output = Command::new(env!("CARGO_BIN_EXE_hshchk"))
            .arg(arg)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        assert!(!output.stdout.is_empty());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn stdout_failure_is_operational_even_after_manifest_was_saved() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let full = fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hshchk"))
        .arg(root.path())
        .stdout(full)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(root.path().join("SHA1SUMS").exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Cannot write output"));
}

#[cfg(unix)]
#[test]
fn ctrl_c_exits_130_and_does_not_save_a_partial_manifest() {
    use std::io::Read;
    use std::process::Stdio;
    let root = TempDir::new().unwrap();
    let file = fs::File::create(root.path().join("file")).unwrap();
    file.set_len(512 * 1024 * 1024).unwrap();
    drop(file);
    let mut child = Command::new(env!("CARGO_BIN_EXE_hshchk"))
        .arg(root.path())
        .args(["--progress", "always"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let mut observed = Vec::new();
    let mut byte = [0];
    while !observed.ends_with(b"file") {
        if stderr.read(&mut byte).unwrap() == 0 {
            panic!("CLI stopped before hashing began");
        }
        observed.push(byte[0]);
    }
    // The first progress message is sent only after the signal handler is installed.
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGINT) }, 0);
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(130));
    assert!(!root.path().join("SHA1SUMS").exists());
}
