[![crates.io](https://img.shields.io/crates/v/hshchk.svg)](https://crates.io/crates/hshchk)

# hshchk

`hshchk` creates, verifies, and updates checksums for files in a directory tree.

# Installation

## dpkg

Download a `.deb` package for your architecture from the [release page](https://github.com/macote/hshchk/releases). In the directory containing that package, install it with:

``` bash
sudo dpkg -i ./hshchk-*.deb
```

## Other

Download an appropriate package from the release page, or build from source with the Rust toolchain:

``` shell
cargo build --locked --release
```

The executable is `target/release/hshchk` (`target/release/hshchk.exe` on Windows).

# Usage

``` shell
hshchk path/to/folder
```

If a recognized checksum file exists in the target folder, `hshchk` verifies its entries. Otherwise, it recursively hashes the files and creates a checksum file. The CLI defaults to SHA1 and the HashSum format (`<hash> *<relative-path>`), saved as `SHA1SUMS`.

``` shell
hshchk path/to/folder --type SHA256
hshchk path/to/folder --type SHA256 --create
hshchk path/to/folder --hc
hshchk --help
```

`--type` selects the algorithm for creation and the preferred algorithm when detecting an existing checksum file. Supported algorithms are MD5, SHA1, SHA256, SHA512, Blake2b, Blake2s, and Blake3. Detection also checks the other supported algorithms.

`--create` forces creation and overwrites the checksum file for the selected algorithm and format. `--hc` creates the legacy `hshchk.<algorithm>` format, which includes file sizes. Existing legacy files are also detected automatically.

| Option | Purpose |
| --- | --- |
| `--match <pattern>` | Process file paths matching a regular expression. |
| `--ignore <pattern>` | Skip file paths matching a regular expression. |
| `--extra` | Report files absent from the checksum file during verification. |
| `--size` | Verify recorded sizes without hashing; requires a legacy manifest with sizes. |
| `--update` / `-x` | Update an existing manifest, rehashing every selected file. |
| `--remove-missing` | Remove selected missing entries during update. |
| `--dry-run` | Compute proposed update counts without replacing the manifest. |
| `--report-file <path>` | Append per-file hashing records, diagnostics, and a summary to a report. |
| `--progress auto\|always\|never` | Control live progress on stderr; defaults to terminal detection. |
| `--silent` / `-s` | Suppress live progress and stdout summaries; retain errors and report output. |

Filters match paths relative to the target directory, with `/` as the separator on every platform. The same rule applies to traversal and missing-entry detection. Excluded entries are preserved during update, including with `--remove-missing`.

``` shell
hshchk path/to/folder --update --dry-run --remove-missing
hshchk path/to/folder --update --remove-missing
hshchk path/to/folder --report-file report.txt --progress never
```

Update requires an existing manifest; it does not silently create one. Files are rehashed even when their sizes match. Missing entries are retained with a warning unless removal is requested. Dry-run writes no checksum changes, although an explicitly requested report is still written.

Manifest saves use sorted entries and a unique temporary file in the destination directory before replacing the original. Processing errors and cancellation leave the original manifest intact. Queued report events are drained and flushed before saving; report-write failures at that stage prevent replacement. A failure writing the final summary can still occur after a successful manifest save. File identity, size, and modification-time changes detected during hashing abort processing; this is a best-effort check, not a filesystem snapshot. Malformed entries, duplicate paths, invalid digests, and paths escaping the target directory are rejected. Directory symlinks are rejected to avoid cycles and traversal outside the target; ordinary file symlinks are followed. Selected files must have Unicode names representable in the chosen checksum format. Until escaped filenames are supported, newline-containing names are rejected, backslashes in Unix filenames are rejected, and the legacy format also rejects names containing `|`.

The report and active manifest are excluded from hashing. A report cannot alias the manifest, including through a hard link. Reports append to existing files and an unavailable report path fails before processing begins.

Live progress and diagnostics go to stderr. Final summaries go to stdout and use complete lines. Redirected output defaults to no live progress. Statistics distinguish bytes actually hashed from sizes inspected without hashing; counts on failed or canceled runs describe work performed, not committed manifest changes.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Successful create, verify, update, or dry-run; also help and version output. |
| `1` | Verification found an incorrect hash, incorrect recorded size, or missing file. |
| `2` | Invalid arguments or configuration, filesystem/manifest/output errors, or no files processed. |
| `130` | Processing was canceled with Ctrl+C. |

Warnings about extra files or retained missing entries during update do not fail the run. Operational errors take precedence over verification mismatches. An output failure returns `2`, including when it occurs while writing the final summary after the manifest was saved.

Press Ctrl+C to cancel processing. Cancellation detected before manifest replacement leaves the existing manifest intact.

**Migration:** earlier versions generally returned `1` for any failure. Scripts should now distinguish verification failures (`1`) from usage or operational failures (`2`) and cancellation (`130`). Verification summaries use `VerificationFailed` for mismatches. Existing `--silent`, format options, and the directory shortcut remain available.

# Library use

Use `HashFileProcessor::try_new` to validate configuration, then call `process_outcome()` or `process_outcome_with_cancellation_token(token)` to get an owned result:

```rust
use hshchk::hash_file_process::{
    HashFileProcessOptions, HashFileProcessor, ProcessStatus,
};

fn main() -> std::io::Result<()> {
    let mut processor = HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: "files".into(),
        ..Default::default()
    })?;
    let outcome = processor.process_outcome();

    println!("Status: {:?}; hashed: {} bytes", outcome.status, outcome.stats.bytes_hashed);
    if outcome.status == ProcessStatus::VerificationFailed {
        for diagnostic in &outcome.diagnostics {
            eprintln!("{}: {:?}", diagnostic.file_path.display(), diagnostic.state);
        }
    }
    Ok(())
}
```

`ProcessOutcome` contains the operation type, status, statistics, warning/error diagnostics, dry-run flag, and `manifest_written`. Diagnostics are retained even without event subscriptions, and results remain unchanged when the processor is reused. `ProcessStatus::exit_code()` exposes the CLI mapping above.

Configuration errors are returned by `try_new`; errors encountered during processing appear in the outcome. For terminal/report integration, `UI::run_outcome(token)` also retains setup and output errors. Its `manifest_written` flag stays true if saving succeeded before the final output failed.

The existing `process()`, `process_with_cancellation_token()`, `stats()`, event senders, and `UI::run()` remain available. Their `HashFileProcessResult::Error` combines verification and operational failures; use the new outcome API to distinguish them. Cancellation and failures retain statistics for work already completed.

# Development

``` shell
cargo test --locked --all-targets
cargo test --locked --doc
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
```

Cargo.lock is tracked for reproducible CLI builds, and CI tests the locked dependency set. BLAKE3 uses its stable native API through a local digest adapter rather than its experimental trait integration.

The CLI uses HashSum by default. Library callers using `HashFileProcessOptions::default()` retain the legacy format unless they explicitly select `HashFileFormat::HashSum`.

# License

MIT
