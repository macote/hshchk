use hshchk::hash_file_process::*;
use hshchk::ui::UI;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

#[test]
fn legacy_ui_run_returns_compatible_success_and_error_results() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let create = processor(root.path());
    let manifest = create.checksum_file_path().to_owned();
    assert_eq!(
        hshchk::ui::UI::new(create, true)
            .with_stdout_summary(false)
            .run(CancellationToken::new(), HashFileProcessType::Create),
        HashFileProcessResult::Success
    );
    assert!(manifest.exists());
    fs::write(root.path().join("file"), "tada").unwrap();
    let verify = processor(root.path());
    assert_eq!(
        hshchk::ui::UI::new(verify, true)
            .with_stdout_summary(false)
            .run(CancellationToken::new(), HashFileProcessType::Verify),
        HashFileProcessResult::Error
    );
}

fn processor(root: &Path) -> HashFileProcessor {
    HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.into(),
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn creation_and_verification_return_owned_statistics_and_write_state() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let created = processor(root.path()).process_outcome();
    assert_eq!(created.process_type, HashFileProcessType::Create);
    assert_eq!(created.status, ProcessStatus::Success);
    assert!(created.manifest_written);
    assert!(!created.dry_run);
    assert_eq!(created.stats.bytes_hashed, 4);
    assert!(created.diagnostics.is_empty());
    let verified = processor(root.path()).process_outcome();
    assert_eq!(verified.process_type, HashFileProcessType::Verify);
    assert_eq!(verified.status, ProcessStatus::Success);
    assert_eq!(verified.stats.bytes_hashed, 4);
    assert!(!verified.manifest_written);
}

#[test]
fn mismatches_and_missing_files_are_retained_without_channel_subscriptions() {
    let root = TempDir::new().unwrap();
    for name in ["edited", "missing", "size"] {
        fs::write(root.path().join(name), "data").unwrap();
    }
    processor(root.path()).process_outcome();
    fs::write(root.path().join("edited"), "tada").unwrap();
    fs::write(root.path().join("size"), "different size").unwrap();
    fs::remove_file(root.path().join("missing")).unwrap();
    let outcome = processor(root.path()).process_outcome();
    assert_eq!(outcome.status, ProcessStatus::VerificationFailed);
    assert_eq!(outcome.status.exit_code(), 1);
    assert_eq!(outcome.stats.errors, 3);
    assert_eq!(outcome.stats.missing_files, 1);
    assert_eq!(outcome.diagnostics.len(), 3);
    for (diagnostic, (name, state)) in outcome.diagnostics.iter().zip([
        ("edited", FileProcessState::IncorrectHash),
        ("missing", FileProcessState::Missing),
        ("size", FileProcessState::IncorrectSize),
    ]) {
        assert_eq!(diagnostic.file_path, Path::new(name));
        assert_eq!(diagnostic.state, state);
        assert_eq!(diagnostic.level, DiagnosticLevel::Error);
    }
    assert!(!outcome.manifest_written);
}

#[test]
fn malformed_manifests_are_operational_errors_with_retained_messages() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("hshchk.sha1"), "file|bad|digest").unwrap();
    let outcome = processor(root.path()).process_outcome();
    assert_eq!(outcome.status, ProcessStatus::Error);
    assert_eq!(outcome.status.exit_code(), 2);
    assert_eq!(outcome.stats.errors, 1);
    assert_eq!(outcome.diagnostics.len(), 1);
    assert!(
        matches!(&outcome.diagnostics[0].state, FileProcessState::Error(message) if message.contains(":1:"))
    );
    assert!(!outcome.manifest_written);
}

#[test]
fn warnings_do_not_fail_verification_and_outcomes_survive_processor_reuse() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    processor(root.path()).process_outcome();
    fs::write(root.path().join("extra"), "extra").unwrap();
    let mut verifier = HashFileProcessor::new(HashFileProcessOptions {
        base_path: root.path().into(),
        report_extra: Some(true),
        ..Default::default()
    });
    let first = verifier.process_outcome();
    assert_eq!(first.status, ProcessStatus::Success);
    assert_eq!(first.stats.extra_files, 1);
    assert_eq!(first.diagnostics[0].level, DiagnosticLevel::Warning);
    fs::remove_file(root.path().join("extra")).unwrap();
    let next = verifier.process_outcome();
    assert!(next.diagnostics.is_empty());
    assert_eq!(next.stats.extra_files, 0);
    assert_eq!(first.diagnostics.len(), 1);
    assert_eq!(first.stats.extra_files, 1);
}

#[test]
fn update_dry_run_and_retained_missing_entries_are_exposed() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("gone"), "data").unwrap();
    processor(root.path()).process_outcome();
    fs::remove_file(root.path().join("gone")).unwrap();
    let mut updater = HashFileProcessor::new(HashFileProcessOptions {
        base_path: root.path().into(),
        update_mode: true,
        dry_run: true,
        ..Default::default()
    });
    let outcome = updater.process_outcome();
    assert_eq!(outcome.status, ProcessStatus::Success);
    assert_eq!(outcome.process_type, HashFileProcessType::Update);
    assert!(outcome.dry_run);
    assert!(!outcome.manifest_written);
    assert_eq!(outcome.stats.missing_files, 1);
    assert_eq!(outcome.diagnostics[0].level, DiagnosticLevel::Warning);
    assert_eq!(outcome.diagnostics[0].state, FileProcessState::Missing);
}

#[test]
fn canceled_and_empty_runs_have_explicit_status_and_no_manifest_write() {
    let root = TempDir::new().unwrap();
    let mut processor = processor(root.path());
    let token = CancellationToken::new();
    token.cancel();
    let outcome = processor.process_outcome_with_cancellation_token(token);
    assert_eq!(outcome.status, ProcessStatus::Canceled);
    assert_eq!(outcome.status.exit_code(), 130);
    assert_eq!(outcome.stats.bytes_hashed, 0);
    assert!(!outcome.manifest_written);
    assert_eq!(
        processor.process_outcome().status,
        ProcessStatus::NoFilesProcessed
    );
    assert_eq!(processor.process(), HashFileProcessResult::NoFilesProcessed);
}

#[test]
fn ui_preflight_errors_are_returned_as_diagnostics() {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let report = root.path().join("hshchk.sha1");
    let outcome = UI::new(processor(root.path()), true)
        .with_report_file(Some(report.clone()))
        .run_outcome(CancellationToken::new());
    assert_eq!(outcome.status, ProcessStatus::Error);
    assert_eq!(outcome.stats.errors, 1);
    assert_eq!(outcome.stats.files_processed, 0);
    assert!(!outcome.manifest_written);
    assert_eq!(outcome.diagnostics[0].file_path, report);
    assert!(!root.path().join("hshchk.sha1").exists());
}

#[cfg(windows)]
#[test]
fn operational_errors_take_precedence_over_checksum_mismatches() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = TempDir::new().unwrap();
    for name in ["edited", "locked"] {
        fs::write(root.path().join(name), "data").unwrap();
    }
    processor(root.path()).process_outcome();
    fs::write(root.path().join("edited"), "tada").unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(root.path().join("locked"))
        .unwrap();
    let outcome = processor(root.path()).process_outcome();
    assert_eq!(outcome.status, ProcessStatus::Error);
    assert_eq!(outcome.status.exit_code(), 2);
    assert!(outcome
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.state == FileProcessState::IncorrectHash));
    assert!(outcome
        .diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic.state, FileProcessState::Error(_))));
    drop(lock);
}

#[cfg(unix)]
#[test]
fn operational_errors_take_precedence_over_checksum_mismatches() {
    use std::os::unix::fs::symlink;
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("edited"), "data").unwrap();
    processor(root.path()).process_outcome();
    fs::write(root.path().join("edited"), "tada").unwrap();
    symlink("nonexistent-target", root.path().join("broken-link")).unwrap();
    let outcome = processor(root.path()).process_outcome();
    assert_eq!(outcome.status, ProcessStatus::Error);
    assert_eq!(outcome.status.exit_code(), 2);
    assert!(outcome
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.state == FileProcessState::IncorrectHash));
    assert!(outcome
        .diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic.state, FileProcessState::Error(_))));
}
