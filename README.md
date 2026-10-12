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

Use an explicit operation:

``` shell
hshchk create path/to/folder --type SHA256
hshchk verify path/to/folder
hshchk update path/to/folder --dry-run --remove-missing
hshchk update path/to/folder --remove-missing
```

`create` writes a new checksum manifest and requires `--overwrite` before replacing an existing destination. `verify` and `update` require an existing manifest; neither silently creates one.

The CLI defaults to SHA1 for creation and HashSum format (`<hash> *<relative-path>`), saved as `SHA1SUMS`. Use `--format hashcheck` (or `--hc`) to create the legacy `hshchk.<algorithm>` format with file sizes. Reading detects format from the manifest contents, and update preserves that format and the selected filename.

``` shell
hshchk create path/to/folder --format hashcheck
hshchk create path/to/folder --overwrite
hshchk verify path/to/folder --checksum-file SHA256SUMS
hshchk create path/to/folder --checksum-file checksums/custom --type SHA256
hshchk verify path/to/folder --checksum-file checksums/custom --type SHA256
hshchk --help
hshchk update --help
```

## Manifest selection

Without `--checksum-file`, verification/update searches the target root for `hshchk.<algorithm>` and `<algorithm>SUM`/`<algorithm>SUMS`, ignoring ASCII case. Supported algorithms are MD5, SHA1, SHA256, SHA512, Blake2b, Blake2s, and Blake3. Variants such as `sha256sum`, `SHA256SUM`, and `sha256sums` are accepted and updated in place.

Discovery must find exactly one candidate. If several exist, select one with `--checksum-file`; `--type` can narrow discovery to one algorithm, but cannot choose between two manifests for that algorithm. `--algorithm` is an alias for `--type`.

Relative `--checksum-file` paths resolve against the target directory, while absolute paths may point outside it. Manifest entries always resolve against the target directory, regardless of the manifest's location. Creation requires the destination's parent directory to exist.

Conventional filenames identify the algorithm. Custom filenames require `--type` when verifying or updating; digest length cannot distinguish algorithms such as SHA256 and Blake3. A supplied algorithm must agree with a conventional filename. Custom creation defaults to SHA1 unless `--type` is supplied; a conventional destination name supplies its algorithm when `--type` is omitted.

## Compatibility shortcut

``` shell
hshchk path/to/folder
hshchk path/to/folder --type SHA256 --create
hshchk path/to/folder --update
```

The directory shortcut verifies a discovered manifest or creates one when none is found. Legacy `--create` / `-c` still authorizes overwriting; legacy `--update` / `-x` still requires an existing manifest. Discovery now rejects ambiguity, and `--type` filters candidates rather than preferring one algorithm and falling back to others. With the shortcut, no candidate for the selected algorithm triggers creation; use explicit `verify` when creation is not intended.

For directories named `create`, `verify`, `update`, or `completions`, use an explicit path such as `./create`, or `hshchk -- create`. Place options after the command and target directory; shortcut arguments cannot be mixed with subcommands.

| Option | Purpose |
| --- | --- |
| `--type <algorithm>` / `--algorithm` | Select the creation algorithm or restrict discovery; specify the algorithm for custom manifests. |
| `--checksum-file <path>` | Select the manifest; relative paths resolve against the target directory. |
| `--format sum\|hashcheck` | Creation format; `--hc` / `-u` aliases `hashcheck`. |
| `--overwrite` | Explicit `create` only: authorize replacing the selected destination. |
| `--match <pattern>` | Select relative file paths matching a regular expression. |
| `--ignore <pattern>` | Exclude relative file paths matching a regular expression. |
| `--extra` | Verification only: warn about files absent from the manifest. |
| `--size` | Verification only: inspect recorded sizes without hashing; requires a legacy manifest. |
| `--remove-missing` | Update only: remove selected missing entries. |
| `--dry-run` | Update only: compute proposed counts without replacing the manifest. |
| `--report-file <path>` | Append hashing records, diagnostics, and a summary; relative to the working directory. |
| `--output text\|json` | Select a human summary or one versioned JSON result. |
| `--progress auto\|always\|never` | Control live progress on stderr; defaults to terminal detection. |
| `--silent` / `-s` | Suppress progress and human summaries; retain diagnostics, reports, and explicitly requested JSON. |

Filters match paths relative to the target directory, with `/` as the separator on every platform. The same rule applies to traversal and missing-entry detection. Excluded entries are preserved during update, including with `--remove-missing`.

``` shell
hshchk update path/to/folder --dry-run --remove-missing
hshchk update path/to/folder --remove-missing
hshchk verify path/to/folder --report-file report.txt --progress never
```

Update requires an existing manifest; it does not silently create one. Files are rehashed even when their sizes match. Missing entries are retained with a warning unless removal is requested. Dry-run writes no checksum changes, although an explicitly requested report is still written.

Manifest saves use sorted entries and a unique temporary file in the destination directory. Creation without overwrite authorization also refuses a destination that appears after setup. Processing errors and cancellation leave the original manifest intact. Queued report events are drained and flushed before saving; report-write failures at that stage prevent replacement. A failure writing the final summary can still occur after a successful manifest save. File identity, size, and modification-time changes detected during hashing abort processing; this is a best-effort check, not a filesystem snapshot. Malformed entries, duplicate paths, invalid digests, and paths escaping the target directory are rejected. Directory symlinks are rejected to avoid cycles and traversal outside the target; ordinary file symlinks are followed. Selected files must have Unicode names representable in the chosen checksum format. Until escaped filenames are supported, newline-containing names are rejected, backslashes in Unix filenames are rejected, and the legacy format also rejects names containing `|`.

The report and active manifest are excluded from hashing. A report cannot alias the manifest, including through a hard link. Reports append to existing files and an unavailable report path fails before processing begins.

Live progress and diagnostics go to stderr. Final summaries go to stdout and use complete lines. Redirected output defaults to no live progress. Statistics distinguish bytes actually hashed from sizes inspected without hashing; counts on failed or canceled runs describe work performed, not committed manifest changes.

The CLI excludes identifiable redirected stdout/stderr files from hashing, including output files inside the target directory. Use result paths distinct from input files and the checksum manifest: shell redirection opens/truncates its destination before the CLI starts.

## Machine-readable output

```sh
hshchk verify files --output json > result.json
hshchk update files --dry-run --output json --silent
```

JSON mode emits one compact result object with schema version, operation, status, exit code, statistics, retained diagnostics, dry-run flag, and manifest-written state. It also reports usage/configuration failures as JSON. Progress stays on stderr, and report files remain human-readable. Help, version, and completion commands produce text.

See the [JSON v1 contract](docs/json-output-v1.md) and [JSON Schema](docs/json-output-v1.schema.json) for field definitions, failure behavior, and examples.

## Shell completions

Generate scripts for Bash, Zsh, Fish, PowerShell, or Elvish without processing files:

```sh
hshchk completions bash > hshchk.bash
source ./hshchk.bash
hshchk completions zsh > _hshchk
hshchk completions fish > hshchk.fish
hshchk completions elvish > hshchk.elv
```

For Zsh, place `_hshchk` in a directory on `$fpath` and initialize completion with `compinit`. For Fish, place the script in `~/.config/fish/completions/hshchk.fish`. For Elvish, load the generated module according to your shell configuration. PowerShell can load the script for the current session:

```powershell
hshchk completions powershell | Out-File -Encoding utf8 hshchk.ps1
. ./hshchk.ps1
```

Release archives include the generated scripts. See [release validation](docs/releasing.md) and [migration notes](CHANGELOG.md).

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

`HashFileProcessOptions` also accepts `process_type`, `checksum_file`, and `overwrite` for explicit operations and selection. Leave `process_type` unset for the compatibility auto mode. `checksum_file_path()`, `hash_type()`, and `hash_file_format()` expose the resolved configuration.

`hshchk::report::json_report(&outcome)` converts a library result to the documented JSON v1 shape; `write_json(&value, writer)` writes a single JSON record without terminal output.

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
