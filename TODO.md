# To do

## Completed in the rebased feature batch
- update existing checksum files by rehashing selected files
- preserve excluded entries; remove missing entries only on request
- dry-run update counts without checksum changes
- sorted, transactional manifest saves with unique temporary files
- report files, terminal-aware progress, and plain redirected output
- final counts, actual hashed bytes, inspected sizes, elapsed time, and throughput
- errors for malformed manifests, invalid regexes, inaccessible files, and failed saves
- regression tests for update, reporting, output, and failure preservation

## 1 — CLI and result contract
- explicit create, verify, and update subcommands; keep the directory shortcut
- specify checksum file; reject ambiguous automatic discovery
- structured operation outcome with diagnostics and statistics for library callers
- machine-readable JSON output and documented exit-code categories
- checksum format follow-up
  - decide whether to align the library default with the CLI's HashSum default
  - extend filename detection beyond `<hashType>SUMS`
- overall progress
  - total file and byte counts while creating
  - remaining items and time estimates while verifying

## 2 — Filesystem and format compatibility
- define complete symlink and non-Unicode filename policies
- escaped filenames for checksum format interoperability
- concurrent modification and concurrent manifest writer handling
- preserve crash durability guarantees across supported filesystems
- more fault-injection tests for cancellation during commit and output write failures

## 3 — Performance and release polish
- benchmark before adding bounded multicore hashing
- shell completions and expanded help examples
- decide algorithm defaults in a separately documented release
