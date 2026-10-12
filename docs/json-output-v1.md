# hshchk JSON output, schema version 1

Use `--output json` with the directory shortcut or an explicit operation:

```sh
hshchk verify files --output json
hshchk update files --dry-run --output json --silent
```

Processing stdout contains exactly one compact JSON object followed by a newline. Progress and human diagnostics use stderr. `--silent` suppresses progress and human summaries but retains the explicitly requested JSON result. Report files remain human-readable. Help, version, and successful completion generation are text-only auxiliary commands.

The schema is [json-output-v1.schema.json](json-output-v1.schema.json), using JSON Schema draft 2020-12. Tests validate real CLI results against this file. Field names and enum values are stable within version 1; changes to the record shape require a new schema version. Consumers should select the schema using `schema_version` and must not depend on object key order or diagnostic message wording.

| Field | Meaning |
| --- | --- |
| `schema_version` | Integer `1`. |
| `operation` | `create`, `verify`, `update`, or `null` if parsing/auto selection did not resolve an operation. |
| `status` | `success`, `verification_failed`, `error`, `no_files_processed`, or `canceled`. |
| `exit_code` | `0`, `1`, `2`, or `130`, matching the process exit code when output delivery succeeds. |
| `dry_run` | Whether this was a preview update. |
| `manifest_written` | Whether manifest replacement succeeded. Remains true if a later output failure occurred. |
| `stats` | Integer counters and elapsed milliseconds, including partial work on failure/cancellation. |
| `diagnostics` | Retained warnings and errors; can be empty. |

Statistics are `files_processed`, `bytes_hashed`, `bytes_size_checked`, `files_added`, `files_updated`, `files_unchanged`, `files_removed`, `missing_files`, `extra_files`, `errors`, and `elapsed_ms`. Byte counters represent bytes, not formatted units. Consumers with limited integer precision should use an appropriate JSON integer parser for very large counts.

Every diagnostic contains:

- `level`: `warning` or `error`.
- `kind`: `extra`, `invalid_unicode_file_name`, `missing`, `incorrect_size`, `incorrect_hash`, `error`, `usage_error`, or `configuration_error`.
- `path`: a display path or `null` when no path applies. File-entry paths are relative to the target directory; initialization/manifest/output paths may be absolute or user-supplied.
- `path_is_lossy`: true when the original OS path was not valid Unicode and replacement characters were used. The display string then cannot reconstruct the original path.
- `message`: human explanation; use `kind` for machine decisions.

Argument errors and configuration failures use the same envelope, zero work counters, one error diagnostic, exit code `2`, and `manifest_written: false`. Argument failures have `operation: null`; a configuration failure includes the explicit operation when known. A valid `--output json` or `--output=json` request is recognized on parser failure before the `--` argument delimiter.

Example verification failure (expanded for readability):

```json
{
  "schema_version": 1,
  "operation": "verify",
  "status": "verification_failed",
  "exit_code": 1,
  "dry_run": false,
  "manifest_written": false,
  "stats": {
    "files_processed": 1, "bytes_hashed": 4, "bytes_size_checked": 0,
    "files_added": 0, "files_updated": 0, "files_unchanged": 0,
    "files_removed": 0, "missing_files": 0, "extra_files": 0,
    "errors": 1, "elapsed_ms": 3
  },
  "diagnostics": [{
    "level": "error", "kind": "incorrect_hash", "path": "file",
    "path_is_lossy": false, "message": "checksum does not match the manifest"
  }]
}
```

Cancellation uses `status: canceled`, exit code `130`, partial statistics, and `manifest_written: false`. Operational/output errors use exit code `2` even when verification also finds mismatches.

JSON is emitted after report rendering, so report-write errors can be included in the result. If stdout itself fails, a complete JSON record cannot be guaranteed: the CLI reports the output error on stderr and exits `2`. No second JSON object is attempted after a partial write. A successful save is not rolled back for a later output failure.

Library callers can use `hshchk::report::json_report(&outcome)` and `write_json(&value, writer)` without terminal integration.

The CLI excludes identifiable redirected stdout/stderr files from hashing, including output files inside the target directory. Use result paths distinct from input files and the checksum manifest: shell redirection opens/truncates its destination before the CLI starts.
