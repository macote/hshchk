# Roadmap

## PR 1 - Reliable processing and updates (reuse GitHub PR #3)
Implemented:
- rehash all selected files when updating, including same-size edits
- preserve excluded entries and remove missing entries only on request
- dry-run update counts without replacing the manifest
- sorted, transactional manifest saves using unique temporary files
- report files, terminal-aware progress, and plain redirected output
- statistics for files, actual hashed bytes, inspected sizes, time, and throughput
- actionable errors for malformed manifests, invalid regexes, inaccessible files, and failed saves
- flush report events before saving, preserving manifests on report-write failure
- detect file identity, size, and modification-time changes during hashing
- regression tests and stable Linux, macOS, Windows MSVC, and Windows GNU CI

## PR 2 - Structured results and exit codes
- one operation outcome containing status, statistics, and retained diagnostics
- library callers can inspect failed and canceled operations without reading channels
- distinguish verification mismatches from operational errors
- document success, mismatch, usage/operational error, and cancellation exit codes
- retain compatibility wrappers where useful
- test the public library result and CLI exit-code contracts

## PR 3 - Explicit CLI operations and manifest selection
- explicit create, verify, and update subcommands; retain the directory shortcut
- select a checksum file explicitly; reject ambiguous automatic discovery
- define relative manifest path resolution and format selection
- validate incompatible option combinations in the argument parser
- extend checksum filename detection beyond `<hashType>SUMS`
- decide whether to align the library default with the CLI's HashSum default
- expand help, migration examples, and compatibility tests

## PR 4 - Machine-readable output and release polish
- JSON output built on the structured operation outcome
- validate the schema for success, mismatch, failure, and cancellation
- keep progress and diagnostics separate from structured stdout
- shell completions, help examples, and release/migration documentation
- release validation on all supported platforms

## Later work
- total file/byte progress and remaining-time estimates
- complete symlink and non-Unicode filename policies
- escaped filenames for checksum interoperability
- concurrent manifest writers and stronger file snapshot guarantees
- crash durability guarantees across supported filesystems
- benchmark before adding bounded multicore hashing
- decide algorithm defaults in a separately documented release
