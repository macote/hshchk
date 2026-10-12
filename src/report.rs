//! Versioned machine-readable reports. Core outcomes remain independent of the JSON schema.
use crate::hash_file_process::{
    DiagnosticLevel, FileProcessState, HashFileProcessType, ProcessOutcome, ProcessStats,
    ProcessStatus,
};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::path::Path;

fn operation_name(operation: HashFileProcessType) -> &'static str {
    match operation {
        HashFileProcessType::Create => "create",
        HashFileProcessType::Verify => "verify",
        HashFileProcessType::Update => "update",
    }
}
fn status_name(status: ProcessStatus) -> &'static str {
    match status {
        ProcessStatus::Success => "success",
        ProcessStatus::VerificationFailed => "verification_failed",
        ProcessStatus::Error => "error",
        ProcessStatus::NoFilesProcessed => "no_files_processed",
        ProcessStatus::Canceled => "canceled",
    }
}
fn statistics(stats: &ProcessStats) -> Value {
    json!({ "files_processed": stats.files_processed, "bytes_hashed": stats.bytes_hashed,
        "bytes_size_checked": stats.bytes_size_checked, "files_added": stats.files_added,
        "files_updated": stats.files_updated, "files_unchanged": stats.files_unchanged,
        "files_removed": stats.files_removed, "missing_files": stats.missing_files,
        "extra_files": stats.extra_files, "errors": stats.errors, "elapsed_ms": stats.elapsed_ms })
}
fn diagnostic(level: &str, path: Option<&Path>, kind: &str, message: &str) -> Value {
    json!({ "level": level, "path": path.map(|path| path.to_string_lossy()),
        "path_is_lossy": path.is_some_and(|path| path.to_str().is_none()), "kind": kind, "message": message })
}
fn envelope(
    operation: Option<HashFileProcessType>,
    status: ProcessStatus,
    dry_run: bool,
    manifest_written: bool,
    stats: &ProcessStats,
    diagnostics: Vec<Value>,
) -> Value {
    json!({ "schema_version": 1, "operation": operation.map(operation_name), "status": status_name(status),
        "exit_code": status.exit_code(), "dry_run": dry_run, "manifest_written": manifest_written,
        "stats": statistics(stats), "diagnostics": diagnostics })
}

/// Convert a complete operation result to schema version 1, including failed and canceled runs.
pub fn json_report(outcome: &ProcessOutcome) -> Value {
    let diagnostics = outcome
        .diagnostics
        .iter()
        .map(|entry| {
            let (kind, message) = match &entry.state {
                FileProcessState::Extra => ("extra", "file is absent from the manifest"),
                FileProcessState::InvalidUnicodeFileName => (
                    "invalid_unicode_file_name",
                    "file name is not valid Unicode",
                ),
                FileProcessState::Missing => ("missing", "file in the manifest is missing"),
                FileProcessState::IncorrectSize => {
                    ("incorrect_size", "file size does not match the manifest")
                }
                FileProcessState::IncorrectHash => {
                    ("incorrect_hash", "checksum does not match the manifest")
                }
                FileProcessState::Error(message) => ("error", message.as_str()),
            };
            diagnostic(
                match entry.level {
                    DiagnosticLevel::Warning => "warning",
                    DiagnosticLevel::Error => "error",
                },
                (!entry.file_path.as_os_str().is_empty()).then_some(entry.file_path.as_path()),
                kind,
                message,
            )
        })
        .collect();
    envelope(
        Some(outcome.process_type),
        outcome.status,
        outcome.dry_run,
        outcome.manifest_written,
        &outcome.stats,
        diagnostics,
    )
}

/// Configuration and usage failures happen before a processor outcome exists.
/// `operation` is None when argument parsing or auto-mode selection did not resolve it.
pub fn error_report(
    operation: Option<HashFileProcessType>,
    dry_run: bool,
    usage_error: bool,
    message: &str,
) -> Value {
    let stats = ProcessStats {
        errors: 1,
        ..Default::default()
    };
    envelope(
        operation,
        ProcessStatus::Error,
        dry_run,
        false,
        &stats,
        vec![diagnostic(
            "error",
            None,
            if usage_error {
                "usage_error"
            } else {
                "configuration_error"
            },
            message,
        )],
    )
}

/// Write one compact JSON object and a newline. No progress or human text is emitted here.
pub fn write_json(report: &Value, mut writer: impl Write) -> io::Result<()> {
    let mut encoded = serde_json::to_vec(report).map_err(io::Error::other)?;
    encoded.push(b'\n');
    writer.write_all(&encoded)?;
    writer.flush()
}
