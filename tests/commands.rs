use hshchk::hash_file_process::*;
use hshchk::{HashFileFormat, HashType};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn command(mode: &str, root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hshchk"))
        .arg(mode)
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
fn shortcut(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hshchk"))
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
fn input() -> TempDir {
    let root = TempDir::new().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    root
}
fn success(output: Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn explicit_commands_create_verify_update_and_keep_the_shortcut() {
    let root = input();
    success(command("create", root.path(), &["--silent"]));
    success(command("verify", root.path(), &["--silent"]));
    fs::write(root.path().join("file"), "tada").unwrap();
    assert_eq!(
        command("verify", root.path(), &["--silent"]).status.code(),
        Some(1)
    );
    let before = fs::read(root.path().join("SHA1SUMS")).unwrap();
    success(command("update", root.path(), &["--dry-run", "--silent"]));
    assert_eq!(fs::read(root.path().join("SHA1SUMS")).unwrap(), before);
    success(command("update", root.path(), &["--silent"]));
    success(shortcut(root.path(), &["--silent"]));
}

#[test]
fn explicit_create_requires_overwrite_and_legacy_create_still_overwrites() {
    let root = input();
    success(command("create", root.path(), &[]));
    let manifest = root.path().join("SHA1SUMS");
    let before = fs::read(&manifest).unwrap();
    fs::write(root.path().join("file"), "tada").unwrap();
    let output = command("create", root.path(), &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--overwrite"));
    assert_eq!(fs::read(&manifest).unwrap(), before);
    success(command("create", root.path(), &["--overwrite"]));
    fs::write(root.path().join("file"), "data").unwrap();
    success(shortcut(root.path(), &["--create"]));
    success(command("verify", root.path(), &[]));
}

#[test]
fn verify_and_update_do_not_create_missing_manifests() {
    let root = input();
    for mode in ["verify", "update"] {
        assert_eq!(command(mode, root.path(), &[]).status.code(), Some(2));
        assert_eq!(
            command(mode, root.path(), &["--checksum-file", "custom"])
                .status
                .code(),
            Some(2)
        );
        assert!(!root.path().join("custom").exists());
        assert!(!root.path().join("SHA1SUMS").exists());
    }
}

#[test]
fn explicit_relative_manifest_paths_resolve_from_target_not_cwd() {
    let root = input();
    fs::create_dir(root.path().join("checksums")).unwrap();
    let working = TempDir::new().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hshchk"))
        .current_dir(working.path())
        .arg("create")
        .arg(root.path())
        .args(["--checksum-file", "checksums/custom", "--type", "SHA256"])
        .output()
        .unwrap();
    success(output);
    let manifest = root.path().join("checksums/custom");
    assert!(manifest.exists());
    assert!(!working.path().join("checksums/custom").exists());
    let text = fs::read_to_string(&manifest).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(text.ends_with(" *file\n"));
    success(command(
        "verify",
        root.path(),
        &["--checksum-file", "checksums/custom", "--type", "SHA256"],
    ));
    fs::write(root.path().join("file"), "tada").unwrap();
    success(command(
        "update",
        root.path(),
        &["--checksum-file", "checksums/custom", "--type", "SHA256"],
    ));
    success(command(
        "verify",
        root.path(),
        &["--checksum-file", "checksums/custom", "--type", "SHA256"],
    ));
}

#[test]
fn external_manifests_keep_entries_relative_to_target_directory() {
    let root = input();
    let external = TempDir::new().unwrap();
    let manifest = external.path().join("custom");
    success(command(
        "create",
        root.path(),
        &[
            "--checksum-file",
            manifest.to_str().unwrap(),
            "--algorithm",
            "SHA256",
            "--format",
            "hashcheck",
        ],
    ));
    success(command(
        "verify",
        root.path(),
        &[
            "--checksum-file",
            manifest.to_str().unwrap(),
            "--type",
            "SHA256",
            "--size",
        ],
    ));
    fs::write(root.path().join("file"), "tada").unwrap();
    assert_eq!(
        command(
            "verify",
            root.path(),
            &[
                "--checksum-file",
                manifest.to_str().unwrap(),
                "--type",
                "SHA256"
            ]
        )
        .status
        .code(),
        Some(1)
    );
}

#[test]
fn custom_names_require_algorithm_for_reading_and_known_names_infer_it() {
    let root = input();
    success(command(
        "create",
        root.path(),
        &["--checksum-file", "custom", "--type", "Blake3"],
    ));
    let output = command("verify", root.path(), &["--checksum-file", "custom"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--type"));
    success(command(
        "verify",
        root.path(),
        &["--checksum-file", "custom", "--type", "Blake3"],
    ));
    fs::rename(root.path().join("custom"), root.path().join("BLAKE3SUMS")).unwrap();
    success(command(
        "verify",
        root.path(),
        &["--checksum-file", "BLAKE3SUMS"],
    ));
    assert_eq!(
        command(
            "verify",
            root.path(),
            &["--checksum-file", "BLAKE3SUMS", "--type", "SHA256"]
        )
        .status
        .code(),
        Some(2)
    );
}

#[test]
fn discovery_rejects_ambiguity_and_type_restricts_candidates() {
    let root = input();
    success(command("create", root.path(), &["--type", "SHA1"]));
    success(command("create", root.path(), &["--type", "SHA256"]));
    for mode in ["verify", "update"] {
        let output = command(mode, root.path(), &[]);
        assert_eq!(output.status.code(), Some(2));
        let text = String::from_utf8_lossy(&output.stderr);
        assert!(text.contains("SHA1SUMS"));
        assert!(text.contains("SHA256SUMS"));
        assert!(text.contains("--checksum-file"));
    }
    assert_eq!(shortcut(root.path(), &[]).status.code(), Some(2));
    success(command("verify", root.path(), &["--type", "SHA256"]));
    success(command(
        "verify",
        root.path(),
        &["--checksum-file", "SHA256SUMS"],
    ));
    assert_eq!(
        command("verify", root.path(), &["--type", "MD5"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn variants_of_checksum_names_are_detected_and_updated_in_place() {
    for name in [
        "SHA256SUMS",
        "sha256sum",
        "SHA256SUM",
        "sha256sums",
        "sHa256SuMs",
    ] {
        let root = input();
        success(command("create", root.path(), &["--type", "SHA256"]));
        let standard = root.path().join("SHA256SUMS");
        let variant = root.path().join(name);
        if name != "SHA256SUMS" {
            let temporary = root.path().join("renaming");
            fs::rename(&standard, &temporary).unwrap();
            fs::rename(temporary, &variant).unwrap();
        }
        success(command("verify", root.path(), &[]));
        fs::write(root.path().join("file"), "tada").unwrap();
        success(command("update", root.path(), &[]));
        assert!(variant.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
        success(command("verify", root.path(), &[]));
    }
}

#[test]
fn same_algorithm_multiple_formats_require_explicit_selection() {
    let root = input();
    success(command("create", root.path(), &[]));
    success(command("create", root.path(), &["--hc"]));
    assert_eq!(
        command("verify", root.path(), &["--type", "SHA1"])
            .status
            .code(),
        Some(2)
    );
    success(command(
        "verify",
        root.path(),
        &["--checksum-file", "hshchk.sha1"],
    ));
}

#[test]
fn help_describes_each_mode_and_mode_specific_flags_are_rejected() {
    for mode in ["create", "verify", "update"] {
        let output = Command::new(env!("CARGO_BIN_EXE_hshchk"))
            .args([mode, "--help"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert!(String::from_utf8_lossy(&output.stdout).contains("--checksum-file"));
    }
    let root = input();
    for (mode, flags) in [
        ("create", vec!["--size"]),
        ("verify", vec!["--overwrite"]),
        ("verify", vec!["--format", "sum"]),
        ("update", vec!["--size"]),
        ("update", vec!["--format", "sum"]),
        ("create", vec!["--format", "sum", "--hc"]),
    ] {
        assert_eq!(command(mode, root.path(), &flags).status.code(), Some(2));
    }
    assert_eq!(
        shortcut(root.path(), &["--create", "--size"]).status.code(),
        Some(2)
    );
    assert!(!root.path().join("SHA1SUMS").exists());
}

#[test]
fn directories_named_like_commands_can_use_the_compatibility_shortcut() {
    let parent = TempDir::new().unwrap();
    fs::create_dir(parent.path().join("create")).unwrap();
    fs::write(parent.path().join("create/file"), "data").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hshchk"))
        .current_dir(parent.path())
        .args(["--", "create"])
        .output()
        .unwrap();
    success(output);
    assert!(parent.path().join("create/SHA1SUMS").exists());
}

#[test]
fn library_explicit_modes_and_selection_preserve_the_legacy_default() {
    let root = input();
    let mut processor = HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.path().into(),
        process_type: Some(HashFileProcessType::Create),
        checksum_file: Some("custom".into()),
        hash_type: Some(HashType::SHA256),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(processor.hash_file_format(), HashFileFormat::HashCheck);
    assert_eq!(processor.hash_type(), HashType::SHA256);
    assert_eq!(
        processor.checksum_file_path(),
        fs::canonicalize(root.path()).unwrap().join("custom")
    );
    assert_eq!(processor.process_outcome().status, ProcessStatus::Success);
    let mut verifier = HashFileProcessor::try_new(HashFileProcessOptions {
        base_path: root.path().into(),
        process_type: Some(HashFileProcessType::Verify),
        checksum_file: Some("custom".into()),
        hash_type: Some(HashType::SHA256),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(verifier.process_outcome().status, ProcessStatus::Success);
}
#[test]
fn selected_manifest_preserves_actual_format_even_if_filename_suggests_another() {
    let root = input();
    success(command(
        "create",
        root.path(),
        &["--checksum-file", "hshchk.sha256", "--format", "sum"],
    ));
    let manifest = root.path().join("hshchk.sha256");
    assert!(fs::read_to_string(&manifest).unwrap().contains(" *file"));
    success(command("verify", root.path(), &[]));
    assert_eq!(
        command("verify", root.path(), &["--size"]).status.code(),
        Some(2)
    );
    fs::write(root.path().join("file"), "tada").unwrap();
    success(command("update", root.path(), &[]));
    assert!(fs::read_to_string(&manifest).unwrap().contains(" *file"));
}

#[test]
fn library_rejects_conflicting_mode_and_mode_specific_options() {
    let root = input();
    for options in [
        HashFileProcessOptions {
            process_type: Some(HashFileProcessType::Verify),
            force_create: Some(true),
            ..Default::default()
        },
        HashFileProcessOptions {
            process_type: Some(HashFileProcessType::Create),
            update_mode: true,
            ..Default::default()
        },
        HashFileProcessOptions {
            overwrite: true,
            ..Default::default()
        },
        HashFileProcessOptions {
            process_type: Some(HashFileProcessType::Create),
            report_extra: Some(true),
            ..Default::default()
        },
    ] {
        assert!(HashFileProcessor::try_new(HashFileProcessOptions {
            base_path: root.path().into(),
            ..options
        })
        .is_err());
    }
}
#[test]
fn legacy_filenames_starting_with_hex_words_round_trip() {
    let root = input();
    fs::write(root.path().join("dead beef"), "content").unwrap();
    success(command("create", root.path(), &["--format", "hashcheck"]));
    success(command("verify", root.path(), &[]));
    success(command("verify", root.path(), &["--size"]));
    success(command("update", root.path(), &[]));
    success(command("verify", root.path(), &[]));
}
