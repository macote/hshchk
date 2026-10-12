# Repository Guidelines

## Project Structure & Module Organization

`hshchk` is a Rust 2018 CLI and library for creating, verifying, and updating directory-tree checksums. `src/hshchk.rs` is the CLI entry point; `src/lib.rs` exposes library modules. Hashing, traversal, manifest processing, UI, and reporting live in separate `src/` modules. Unit tests reside in `src/tests/`; integration tests and independent CLI workflows live in `tests/`, with digest fixtures in `tests/fixtures/`. `test_shared/` provides shared Rust test helpers. `docs/` contains JSON contracts, testing policy, and release guidance; `scripts/` contains packaging and validation tools. Build outputs belong in `target/` and release packages in `dist/`.

## Build, Test, and Development Commands

Run commands from the repository root:

- `cargo build --locked`: build the debug CLI for local and E2E testing.
- `cargo build --locked --release`: build the optimized executable.
- `cargo run --locked -- verify path/to/folder`: verify an existing checksum manifest.
- `cargo test --locked --all-targets`: run Rust unit and integration tests.
- `cargo test --locked --doc`: run documentation tests.
- `cargo fmt --all -- --check`: check Rust formatting; use `cargo fmt --all` to apply it.
- `cargo clippy --locked --all-targets -- -D warnings`: reject lint warnings.
- `python tests/e2e_cli.py --binary target/debug/hshchk.exe`: run Windows E2E workflows; requires Python 3.9+ and PowerShell 7. On Unix, use `python3` and omit `.exe`.

## Coding Style & Naming Conventions

Follow rustfmt and existing module boundaries. Use four-space indentation for Rust and TOML, LF endings, and no trailing whitespace, following `.editorconfig`. Use `snake_case` for modules, functions, and descriptive test names; `PascalCase` for types. Keep `Cargo.lock` tracked and validate dependency changes with locked builds.

## Testing Guidelines

Use Rust's built-in test framework and Python `unittest`. Add behavioral regression tests beside the relevant unit or integration tests; use temporary directories for filesystem scenarios. All required tests must pass. CI enforces at least 92% production Rust line coverage on Linux and Windows MSVC. Update functionality mappings when documented behavior changes; see `docs/testing.md`.

## Commit & Pull Request Guidelines

Recent commits use concise, imperative descriptions such as “Add CLI E2E tests and coverage gates”; no strict prefix convention is evident. Keep commits focused. PRs should describe behavior changes, link relevant issues, and report validation commands and results. Update README, CHANGELOG, and JSON contract documentation when affected. Include platform limitations and exact builds for hardware validation.
