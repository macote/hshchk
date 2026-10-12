# Independent end-to-end validation for 0.2.0

Status: proposed; these additional tests have not been executed.

## Scope and coverage target

Exercise the packaged CLI through fresh user workflows on four machines: Windows x64, Linux x64, Linux AArch64, and macOS Apple Silicon. Public results use these platform labels only, with no machine names, SSH addresses, usernames, or identifying absolute paths.

Coverage and pass rate are separate requirements. Every required test must pass; no percentage of failed tests is acceptable. Blocked required tests prevent completion and must be resolved rather than counted as passes.

Two independently reported coverage targets apply:

- **Functionality:** map tests to a reviewed inventory of documented CLI/library behaviors and supported platform behavior. Cover at least 80% of applicable behavior requirements on each platform, and 100% of requirements protecting digest correctness, manifest preservation, selection, and exit/output contracts. Forty scenarios below are candidate tests, not the coverage denominator. Freeze the inventory and platform applicability before execution; do not shrink it to improve the score.
- **Code:** measure at least 92% line coverage of first-party production Rust source, including CLI and library code, using instrumented builds. Include existing unit/integration tests and new E2E runs in the combined result; exclude test helpers and third-party dependencies, never uncovered production code merely to meet the threshold. Report regions and uncovered functions as additional diagnostics. Begin with a Linux baseline and explicitly identify cfg-specific Windows/macOS code that requires additional platform reports. Do not describe Linux-only coverage as universal platform coverage.

Plan to execute all 40 scenarios on every machine: 160 machine/scenario results, plus platform extensions. All required scenarios must pass. Existing Rust tests and the 176-check scripted suite remain required and contribute to combined measured coverage, but do not substitute for executing the additional independent scenarios.

All observed correctness or data-loss failures must be resolved before release, regardless of the score. Platform-specific extensions below are reported separately and do not inflate the common score.

## Recommended suite and CI design

1. Measure current coverage first; the repository already has CLI integration tests. Use the uncovered production paths and behavior inventory to select additions instead of duplicating existing assertions indiscriminately.
2. Add deterministic E2E cases to a maintained CLI integration layer with shared fixture/evidence helpers. Run realistic independent workflows through subprocesses. Keep focused unit tests for difficult internal branches that E2E cannot exercise reliably.
3. Make the scenario layer able to exercise both an instrumented development binary and an extracted release binary. Uninstrumented release archives establish packaging/runtime behavior but cannot yield source coverage. Keep independent digest oracles separate from the implementation under test.
4. On every PR, extend the existing Linux/macOS/Windows test jobs with the bounded deterministic scenarios; add one Linux cargo-llvm-cov job collecting unit/integration/E2E execution and enforcing 92% production line coverage. Upload HTML and machine-readable coverage artifacts. Preserve ordinary stable-toolchain tests and doctests.
5. Initially collect Windows coverage as an additional report to identify platform gaps; establish a separate threshold after measuring its denominator. Keep branch coverage informational rather than introducing a nightly dependency for the initial gate.
6. Keep all 11 release-package checks; execute portable E2E scenarios against extracted compatible binaries there. Do not multiply the full stress campaign across all cross-build targets.
7. Run timing-sensitive mutation/concurrency/crash experiments, scale tests, interactive completion/progress, and hardware-specific probes in a manual/pre-release campaign on the four machines. Every required deterministic expectation must pass; exploratory observations must not be silently promoted to guaranteed behavior.

CI impact: one additional instrumented Linux build/test job, coverage artifacts, and modest subprocess/fixture work inside the existing platform jobs. Instrumented compilation is the main likely cost; actual wall time must be measured before promising a duration. Separate coverage caches from ordinary/release builds and pin coverage tooling. The coverage job can run in parallel. No production dependency or additional hosted coverage service is required. Private SSH hosts remain outside unattended public-PR CI.

## Method

- Use archives built from one pinned commit; record archive SHA256, binary version, OS, architecture, shell version, and filesystem type. Repeat affected tests if the binary changes.
- Invoke commands directly from native shells: PowerShell on Windows, Bash on Linux, and Zsh on macOS. Use an interactive terminal or PTY for progress and Ctrl+C cases. Run a second available shell for a smaller compatibility sweep.
- Create new fixtures and command sequences rather than invoking the existing smoke runner. A minimal fixture/evidence helper is acceptable, but expectations must be independently computed and reviewed.
- Build one realistic document tree with nested folders, empty files, binary data, spaces, accented names, and a moderately large file. Use a separate fixture for malformed inputs and failure experiments.
- Validate MD5, SHA1, SHA256, SHA512, Blake2b, and Blake2s against independent Python hashlib calculations or native checksum tools. Use an independently built official b3sum executable for Blake3 if available; otherwise report external Blake3 verification as blocked rather than using hshchk to validate itself.
- Record the actual command, expected result, exit code, separate stdout/stderr, parsed JSON, independent manifest checks, and before/after manifest SHA256. Sanitize private paths before attaching evidence to the PR.
- For each scenario, inspect both the primary result and side effects: selected manifest, report append behavior, untouched inputs, and temporary-file cleanup. A zero exit code alone is insufficient evidence.

## Common scenarios: run on every machine

Critical scenarios are marked **critical**. They protect checksum correctness, manifest preservation, or the public result contract.

| ID | Workflow | Acceptance criteria |
| --- | --- | --- |
| C01 | Create SHA256 checksums for a realistic nested tree | **Critical:** independently computed digests match every selected file; relative paths are correct and the active manifest is excluded. |
| C02 | Generate representative files with every supported algorithm | **Critical:** compare each digest with an independent implementation, including zero-byte input; block external Blake3 coverage if its oracle is unavailable. |
| C03 | Create and independently inspect both formats | Recorded sizes, digest lengths, ordering, and path syntax match the format contract. |
| C04 | Verify an untouched tree from a different working directory | **Critical:** entries resolve against the target tree; verification passes without modifying inputs or manifest. |
| C05 | Round-trip a manifest between two different operating systems | **Critical:** for a shared portable fixture, cross-OS create/verify/update preserves correct digests and relative paths. Test both formats using explicit selection. |
| C06 | Modify file contents while preserving size | **Critical:** verify returns 1; update changes the digest; independent verification then passes. |
| C07 | Rename a directory, add files, and remove files in one edit session | Counts and diagnostics describe actual work; missing entries remain until explicitly removed. |
| C08 | Preview that edit session with dry-run | **Critical:** proposed changes are visible while the manifest remains byte-identical. |
| C09 | Combine match, ignore, and remove-missing on nested paths | **Critical:** only selected missing entries are removed; excluded entries remain byte-identical and separators work across platforms. |
| C10 | Repeat an unchanged update | Manifest content remains deterministic and digests are rechecked; unchanged statistics are correct. |
| C11 | Select a custom nested relative manifest and an external absolute manifest | **Critical:** resolution follows the documented base directory; manifests do not absorb themselves into their input set. |
| C12 | Provide conventional names, case variants, and singular SUM names | Correct algorithm is discovered and update preserves the chosen filename. |
| C13 | Introduce ambiguous manifests, then resolve selection explicitly | Ambiguous verify/update fails clearly; explicit selection affects only the intended manifest. |
| C14 | Supply a conflicting algorithm or omit the algorithm for a custom manifest | Returns 2 before processing; every pre-existing manifest is preserved. |
| C15 | Attempt create over an existing manifest, then use overwrite intentionally | **Critical:** unauthorized replacement fails unchanged; authorized replacement affects only the selected destination. |
| C16 | Run a shell job that branches on actual process exit status | **Critical:** success 0, mismatch/missing 1, usage/operational/empty 2; captured process status matches documented behavior. |
| C17 | Pipe JSON into a separately written consumer | **Critical:** exactly one valid result, matching exit status, schema validation, useful diagnostics, and no progress contamination. |
| C18 | Redirect stdout and stderr into the target tree | **Critical:** identifiable output files are excluded; the result remains parseable; safe output destinations are distinct from inputs and manifest. |
| C19 | Append multiple operations to an existing human report | Previous content is retained; JSON stdout stays separate; the report is not hashed. |
| C20 | Use an interactive terminal and then ordinary redirection | Auto/always/never progress and silent modes behave as documented; output is readable without accidental terminal controls. |
| C21 | Process spaces, accents, emoji, and leading punctuation through native shell quoting | Supported names retain their exact logical relative paths and independently verified digests. |
| C22 | Use target directories named create, verify, update, or completions | Explicit paths and the -- delimiter select the intended directory without invoking a command accidentally. |
| C23 | Pass invalid options, regexes, and incompatible operation flags | Returns 2 with useful diagnostics, including JSON where requested; no manifest writes. |
| C24 | Exercise the documented legacy directory shortcut | Create-without-manifest, verify-with-manifest, legacy forced create, and update follow the documented compatibility rules. |
| C25 | Load generated completions in a temporary shell session and request completion | Command, option, and enum-value completion works interactively in the primary shell; persistent profiles remain unchanged. |
| C26 | Make a selected input inaccessible using only fixture permissions | **Critical:** returns operational error; failed update preserves the original manifest. Restore permissions afterward. |
| C27 | Make the fixture manifest parent unwritable during replacement | **Critical:** failed save preserves the old manifest and leaves no abandoned save file; the error is actionable. |
| C28 | Point the report at the manifest and at a hard-link alias | **Critical:** both aliases are rejected before append/truncation; manifest remains intact. Use hard links only inside the fixture. |
| C29 | Close a downstream consumer while text/JSON output is being written | No panic or hang; output failure returns 2 where observable. A truncated JSON stream is not claimed to be a complete result. Record whether a save had already succeeded. |
| C30 | Interrupt create and update from a real terminal during hashing | **Critical:** Ctrl+C returns 130; the previous manifest is preserved, a new manifest is not committed, and temporary save files are cleaned. |
| C31 | Replace, truncate, or modify a large input while it is being hashed | **Critical:** deliberately timed mutations detectable by the documented identity/size/mtime checks yield error and preserve the manifest. Repeat and report timings; do not claim snapshot guarantees. |
| C32 | Run two updates concurrently on an owned fixture | Exploratory: observe valid manifest structure and final state against independent digests; document lost-update behavior as unsupported concurrency, without claiming serialization. Any corrupt manifest is a failure. |
| C33 | Terminate a child abruptly during hashing, then verify/recover | Original committed manifest remains usable when termination is known to precede saving; inspect leftovers. Crash durability during replacement is exploratory, not a guaranteed contract. |
| C34 | Feed duplicate, malformed, absolute, and parent-escaping manifest entries | **Critical:** rejected with operational errors; no reads/writes outside the fixture and no manifest replacement. Place disposable sentinel files around the target to detect escapes. |
| C35 | Combine mismatch and operational failure in one run | **Critical:** operational status takes precedence; both diagnostics survive in JSON and report output. |
| C36 | Hash approximately 10,000 small files and independently sample results | Correct file counts, digests, stable ordering, responsiveness, and no unexpected resource spike. Use a bounded fixture size. |
| C37 | Hash a moderately large file and compare a native tool result | Correct digest/byte counts; interruptibility and reasonable memory use. Bound size to available free space, especially on ARM. |
| C38 | Include an empty file, an empty subdirectory, and a separate empty tree | Empty files hash correctly; empty directories are handled consistently; empty tree exits 2 without creating a manifest. |
| C39 | Verify recorded sizes in a legacy manifest before and after changing file size | Correct mismatch status and inspected-byte counters with zero bytes hashed; inappropriate HashSum size verification is rejected. |
| C40 | Extract the archive into a path containing spaces and run without installation | Binary executes from its extracted location, version is correct, bundled docs/schema/completions exist, and no user configuration is required. |

For C05, use a hand-built SHA256 fixture shared between all four machines; validate independently before transfer. Do not include case-colliding or platform-invalid names in the portable fixture.

## Additional platform probes

- **Windows:** cmd.exe quoting/redirection alongside PowerShell; NTFS hard links, case-insensitive names, file-sharing locks, and long paths under current host settings. Junction/directory-link rejection when creation is permitted without changing system policy. Test 32-bit and GNU release archives on the same x64 machine as additional compatibility checks.
- **Linux x64:** GNU and musl archives; non-UTF-8 names, Unix backslashes/newlines, file and directory symlinks, and broken links. Use /dev/full for report-output failure. Inspect Debian metadata and extract its binary into a temporary directory without installing the package.
- **Linux AArch64:** execute the actual cross-built archive on hardware; mixed small-file workload and interruptibility under normal host load. Do not infer ARM/ARMv7 32-bit compatibility from AArch64 success.
- **macOS Apple Silicon:** Zsh completions, composed/decomposed Unicode, actual filesystem case behavior, and file/directory symlink handling. Reject unsupported names consistently; do not change filesystem or security settings. Unsigned archive/Gatekeeper behavior can be observed, but signing/notarization is a separate release decision.

Link-policy expectations follow README: directory links rejected; ordinary file links followed. Unsupported filename cases must produce explicit errors, never silently generate misleading checksums.

## Safe setup and cleanup

- Create one private upload directory and one fresh fixture root per platform using the OS temporary-directory API. Record exact resolved paths privately and place an ownership marker before running tests.
- Keep fixtures, sentinels, permission experiments, generated reports, and completion files inside those roots. Do not use existing user data, global package installation, shell profiles, service configuration, or production directories.
- Set explicit limits: generally under 256 MiB per machine, a larger fixture only when free space permits, and a timeout per command. Never fill the real system disk. Use bounded output-failure probes rather than machine-wide exhaustion.
- Restrict ACL/permission changes to test-owned files and restore them before cleanup. Spawn and terminate only child processes created for the campaign; never kill by broad process name.
- Load completion scripts only into disposable shell sessions. Use existing checksum tools; keep any required b3sum executable in the isolated upload directory.
- Remove upload files by their exact recorded paths. Before recursive fixture cleanup, validate ownership marker, resolved root, expected temporary parent, and absence of an escaped cleanup target. Do not follow directory links during cleanup. Preserve failed-case evidence in a sanitized local results directory before removal.
- Verify removal and report leftover paths privately if cleanup fails. Public evidence contains only generic platform labels and sanitized fixture-relative paths.

## Execution order and deliverables

1. Freeze the candidate commit and download matching archives. Prepare the independent digest oracle and portable fixtures.
2. Run the ordinary user workflows and cross-platform round trips (C01-C25, C38-C40).
3. Run permission/output/cancellation/mutation cases (C26-C35) in separate fixtures, then bounded scale checks (C36-C37).
4. Run applicable platform probes and a short free-form exploratory session: unusual option combinations, naming, and shell usage beyond the predefined cases.
5. Produce a sanitized per-platform scorecard with all 40 IDs, result, evidence, and reason for any blocked case. Summarize all defects by release impact; fix blockers and rerun affected workflows on all relevant machines.
6. Attach the report to the release PR. Do not tag or publish until all required tests pass and both coverage targets are met. Keep unsupported concurrency, crash durability, ARM32 hardware, and signing limitations explicit.

Deliverables: the scenario scorecard, sanitized command transcripts, independent digest comparisons, manifest-preservation evidence, package identities/checksums, cleanup confirmation, and a release readiness recommendation.
