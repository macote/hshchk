# Release checklist

The release workflow builds the original target matrix and adds native Apple Silicon:

- Linux: x86_64 GNU and musl, ARM hard-float, ARMv7 hard-float, AArch64 GNU.
- macOS: x86_64 and AArch64.
- Windows: x86_64 and i686, each using MSVC and GNU.

Native/executable-compatible packages are smoke-tested after extracting their binary. ARM cross builds validate archive structure and checksums; this does not establish runtime behavior on ARM hardware. Linux x86_64 GNU packages are built on Ubuntu 22.04 and declare a glibc 2.35 minimum; musl packages are the static alternative.

Archives include the executable, README, license, changelog, JSON schema/documentation, and completion scripts for all five shells. Linux x86_64 builds also produce Debian packages. Each asset has a SHA256 checksum file; the draft-release job combines these into `SHA256SUMS`.

## Before tagging

1. Merge the PR only after normal CI and the release packaging matrix pass.
2. Run `cargo test --locked --all-targets`, `cargo test --locked --doc`, formatting, and Clippy checks.
3. Review the migration notes in README and CHANGELOG, especially exit codes, overwrite behavior, discovery, and library struct additions.
4. Set Cargo.toml to the intended version, refresh Cargo.lock, and move unreleased notes under that version.
5. Optionally run `gh workflow run release.yml --ref master` for an artifact-only rehearsal after this workflow is merged. Manual runs never create a release.
6. Inspect representative archives, Debian metadata, completion scripts, and checksums. Test cross-built binaries on target hardware before claiming hardware runtime support.

## Tagging and draft review

Push a version tag such as `0.2.0` or `v0.2.0` only when it exactly matches Cargo.toml. The workflow rejects a mismatch and waits for every package build before creating a draft release. It never automatically publishes that draft.

Inspect assets and release notes before publishing manually. Re-running the tag workflow may update assets only on an existing draft; it refuses to overwrite an already published release.

No crates.io publication is automated. A package check is validation, not publication; publishing the crate remains a separate release action.

## Local packaging

Python 3.11+ and a built native release binary are required:

```sh
cargo build --locked --release
python scripts/package_release.py --binary target/release/hshchk \
  --target x86_64-unknown-linux-gnu --output-dir dist --smoke --deb
```

On Windows, use `target/release/hshchk.exe`, the appropriate Windows target, and omit `--deb`. Cross-built archives require `--generator` pointing to a native executable from the same revision to generate completion scripts. Generated packages stay under `dist/`; the script does not publish or install them.
