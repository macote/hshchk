[![crates.io](https://img.shields.io/crates/v/hshchk.svg)](https://crates.io/crates/hshchk)

# hshchk

`hshchk` lets you create and verify checksums on folder files recursively.

# Installation

## dpkg

Download the latest `.deb` package from the [release page](https://github.com/macote/hshchk/releases) and install it via:

``` bash
sudo dpkg -i hshchk-0.1.0-x86_64-unknown-linux-gnu.deb  # adapt version number and architecture
```

## Other

Download an appropriate package from the release page, or build from source with the Rust toolchain:

``` shell
cargo build --release
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

The CLI returns exit code 0 on success and a nonzero code on failure, cancellation, or when no files are processed. Press Ctrl+C to cancel processing.

# Development

``` shell
cargo test --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

The CLI uses HashSum by default. Library callers using `HashFileProcessOptions::default()` retain the legacy format unless they explicitly select `HashFileFormat::HashSum`.

# License

MIT
