# To do

## 1
- checksum format follow-up
  - CLI already defaults to HashSum without file sizes; decide whether to align the library default
  - extend filename detection beyond `<hashType>SUMS` to `<hashType>sum`, `<hashType>SUM` and `<hashType>sums`
- overall progress
  - file and byte count when creating
  - completion info when verifying (items left, time left, etc.)
  - stats (how many files, total bytes, avg speed, etc.)

## 2
- performance
  - use multiple CPU cores to calculate (one file per core)
- features
  - report mode (output to file or no ui progress)
  - update checksum file

## 3
- features
  - specify hash file
- error handling
  - inaccessible files and folders (lock, permission, etc.)
  - terminal output when stdout is redirected or no terminal size is available
  - malformed checksum entries and invalid regular expressions
- additional tests
