# To do

- determine if hshchk should use (md5|sha1|etc.)sum file format by default
  - if so, it needs to stay backward compatible with `filename|size|hash` format
- report/stats (file counts)
- features
  - -o option: output file
  - support (md5|sha1|etc.)sum files
- verbosity
  - progress loop
    - filetree updates
    - block updates
    - overall
  - defaults to progress output?
- error handling
  - inaccessible files and folders
- tests
- readme
- packaging
- release
