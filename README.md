[![Build status](https://github.com/macote/hshchk/workflows/ci/badge.svg)](https://github.com/macote/hshchk/actions)
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
| `--size` | Verify recorded sizes without hashing; use with legacy files containing sizes. |

Filters match absolute paths while traversing files, and relative paths when checking for missing checksum entries. Use patterns that work with both forms.

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
