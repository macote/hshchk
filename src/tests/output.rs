use super::*;

#[test]
fn live_speed_remains_visible_with_long_unicode_paths() {
    let mut output = Output::new(None, false, Some(true), true).unwrap();
    output.width = 79;
    let progress = FileProgress {
        file_path: "é".repeat(100),
        file_size: 2 * 1_048_576,
        bytes_processed: 1_048_576,
        complete: false,
    };
    let line = output.progress_line(&progress, 1_000);
    assert!(line.ends_with(" (50%; 1048576 bytes; 1.00 MiB/s)"));
    assert_eq!(line.graphemes(true).count(), 79);
    assert!(output.progress_line(&progress, 0).ends_with("0.00 B/s)"));
}

#[test]
fn live_and_final_throughput_share_precision_and_binary_units() {
    use crate::hash_file_process::{HashFileProcessType, ProcessStats, ProcessStatus};

    let root = tempfile::tempdir().unwrap();
    for (status, bytes, elapsed_ms, expected) in [
        (
            ProcessStatus::Success,
            248_729_809_985,
            175_065,
            "1.32 GiB/s",
        ),
        (
            ProcessStatus::Canceled,
            25_373_442_048,
            18_780,
            "1.26 GiB/s",
        ),
    ] {
        let report = root.path().join(format!("{:?}.txt", status));
        let mut output = Output::new(Some(&report), true, Some(false), false).unwrap();
        output.width = 79;
        let progress = FileProgress {
            file_path: "disk.dmg".into(),
            file_size: 248_729_809_985,
            bytes_processed: bytes,
            complete: status == ProcessStatus::Success,
        };
        assert!(output
            .progress_line(&progress, elapsed_ms)
            .ends_with(&format!("{})", expected)));
        output
            .write_result(&ProcessOutcome {
                process_type: HashFileProcessType::Verify,
                status,
                stats: ProcessStats {
                    bytes_hashed: bytes,
                    elapsed_ms,
                    ..Default::default()
                },
                diagnostics: Vec::new(),
                dry_run: false,
                manifest_written: false,
            })
            .unwrap();
        let text = std::fs::read_to_string(&report).unwrap();
        assert!(text.contains(&format!("hashing throughput: {}", expected)));
        assert!(!text.contains(" GB/s"));
    }
}
