# Changelog

## Unreleased

### Added
- Explicit create, verify, and update commands while retaining the directory shortcut.
- Checksum-file selection, format selection, discovery ambiguity errors, and filename variants.
- Correct updates for same-size edits, filtered missing-file removal, and dry-run.
- Transactional manifest saving, pre-save report flushing, and best-effort input-change detection.
- Owned processing outcomes with retained diagnostics, statistics, and manifest-written state.
- Terminal-aware stderr progress, report files, versioned JSON output, and shell completions.
- Reproducible locked builds, stable BLAKE3 integration, cross-platform tests, and refreshed release packaging.

### Migration
- Exit codes: 0 success, 1 verification mismatch/missing file, 2 usage/operational/empty-run failure, 130 cancellation.
- Explicit create requires --overwrite; legacy --create continues to authorize replacement.
- Discovery rejects multiple candidates. --type now filters discovery rather than preferring one algorithm and falling back to others.
- Filters consistently use relative paths with forward slashes.
- Custom manifest names require an algorithm when reading; entries resolve against the target directory.
- CLI creation defaults to HashSum; the library retains its legacy creation default.
- Public HashFileProcessOptions gained fields; struct literals without ..Default::default() must supply them.
- JSON v1 is one result object on stdout; progress and diagnostics remain on stderr. JSON mode retains its result with --silent.

### Limits
- Directory symlinks are rejected; non-Unicode and unrepresentable checksum filenames are not processed.
- File-change detection is best effort. Concurrent writers and filesystem snapshots are not guaranteed.
- A final output failure can occur after a successful manifest replacement.
- ARM release archives are cross-built; runtime validation on target hardware is a separate release check.
