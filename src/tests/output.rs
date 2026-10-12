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
    assert!(line.ends_with(" (50%; 1048576 bytes; 1 MiB/s)"));
    assert_eq!(line.graphemes(true).count(), 79);
    assert!(output.progress_line(&progress, 0).ends_with("0 B/s)"));
}
