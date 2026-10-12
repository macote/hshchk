# Test and coverage policy

All required tests must pass. Coverage is a separate release/CI requirement, not an acceptable test failure rate.

## Test layers

- Rust unit and integration tests cover library contracts, error propagation, cancellation, atomic-save behavior, and CLI parsing/results.
- `tests/e2e_cli.py` runs independent CLI workflows against a supplied binary. It uses Python 3.9+ standard library, fresh temporary fixtures, subprocess timeouts, and separate stdout/stderr. Seven algorithms in both formats are checked against independent digest fixtures. Other workflows cover report append history, filtered edit sessions, foreign manifests, hard-link output identities, path escapes, a bounded nested binary tree, and actual Bash/PowerShell completion suggestions.
- Unix-only E2E cases cover file links, broken links, directory cycles, unsupported filenames, and unwritable manifest directories. They are inapplicable on Windows, not silently treated as passes. Permission cases require an unprivileged account and restore permissions before temporary cleanup.
- Release packaging and hardware validation remain separate from instrumented coverage. Stress, interactive terminal sizing, additional interactive shells, cross-machine transfer, and unsupported concurrency/crash observations are explicit remaining exploratory work in `release-e2e-plan.md`.

Run the ordinary checks:

```sh
cargo test --locked --all-targets
cargo test --locked --doc
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
python3 tests/e2e_cli.py --binary target/debug/hshchk
```

On Windows, use `python` and `target/debug/hshchk.exe`; PowerShell 7 must be on PATH for the completion-session test. Bash must be available for the corresponding Unix case. The E2E suite never reads or changes persistent shell profiles. To test a packaged binary on another machine, copy the runner and its `fixtures/digest-vectors.json` file together, then pass the extracted executable via `--binary`. Native fixture rules apply; do not run the permission suite as root.

## Code coverage

Linux and Windows MSVC CI enforce at least **80% first-party production Rust line coverage**. Existing Rust tests and the external E2E process executions contribute to the same LLVM report. Function and region coverage are reported as diagnostics; branch coverage is not a claimed metric or gate.

Unit-test modules live in `src/tests/` so cargo-llvm-cov's default test-directory filter excludes their lines. Test helpers and dependencies are excluded; production source, including CLI, output, progress, adapters, and error paths, stays in the denominator. Platform reports are separate because cfg-selected code differs. Neither report establishes coverage on every supported architecture. Ordinary release binaries are not instrumented and cannot supply source coverage.

CI pins cargo-llvm-cov to 0.9.1. With that tool installed and the rustup llvm-tools-preview component available, a Bash reproduction is:

```sh
mkdir -p coverage
export CARGO_TARGET_DIR="$PWD/target/coverage"
source <(cargo llvm-cov show-env --sh)
cargo llvm-cov clean --workspace
cargo test --locked --all-targets
cargo build --locked
python3 tests/e2e_cli.py --binary "$CARGO_TARGET_DIR/debug/hshchk"
cargo llvm-cov report --json --output-path coverage/coverage.json --fail-under-lines 80
cargo llvm-cov report --html --output-dir coverage/html
```

Use a disposable shell for the instrumentation environment. On PowerShell, the environment setup is `Invoke-Expression (cargo llvm-cov show-env --pwsh | Out-String)` and the binary has an `.exe` suffix. Doctests remain an independent stable-toolchain check; they are not claimed as instrumented coverage.

## Functionality traceability

`functionality-inventory.json` records a reviewed inventory of documented behaviors, platform applicability, criticality, and linked acceptance tests. At least **80% of applicable requirements must have available test mappings**, with **100% of critical requirements mapped**. The checker runs only after the Rust and E2E suites pass; all those tests must pass, including tests beyond the minimum mapping percentage.

This is an acceptance-test traceability metric. Name discovery alone cannot prove an assertion is adequate: reviewers must verify the test checks the stated behavior. It is distinct from measured Rust line coverage. Explicit empty mappings preserve visibility of manual/exploratory gaps; do not remove requirements, classify applicable cases away, or exclude uncovered production lines merely to improve a percentage.

After passing both suites, reproduce the mapping report:

```sh
cargo test --locked --all-targets -- --list > rust-tests.txt
python3 tests/e2e_cli.py --list > e2e-tests.txt
python3 scripts/check_functionality.py --rust-tests rust-tests.txt \
  --e2e-tests e2e-tests.txt --output functionality.json
```

CI uploads functionality JSON, per-platform coverage JSON/HTML, and the tested native CLI binaries for hardware reruns. Reports use generic platform labels. Public reports must not include private machine identifiers or SSH addresses.

## CI cost

The existing four platform jobs run the bounded E2E suite and mapping checks. Two additional parallel jobs compile instrumented Linux and Windows MSVC builds and publish coverage reports. No production dependency or external coverage service is introduced. Linux uses Ubuntu 22.04 to keep the uploaded validation binary compatible with the release's glibc 2.35 baseline. Cross-built release packages keep their existing validation; private hardware tests remain outside public-PR CI.

The existing release-workflow path filters still trigger the full packaging matrix when scripts/workflow inputs change. Once landed, editing the E2E runner under `tests/` alone does not trigger that release matrix. CI timings should be compared against the recorded first run before setting a performance budget.
