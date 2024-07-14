# To do

## 1
- switch to sha1sum file format by default
  - drop file size
  - support `<hashType>sum`, `<hashType>SUM`, `<hashType>sums` and `<hashType>SUMS`
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
- additional tests
