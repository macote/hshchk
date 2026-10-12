#!/usr/bin/env python3
"""Independent CLI workflows; Python 3.9+ stdlib, usable with any candidate binary."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

BINARY = None
VECTORS = json.loads((Path(__file__).parent / "fixtures/digest-vectors.json").read_text(encoding="utf-8"))


class CliWorkflows(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="hshchk-e2e-")
        self.addCleanup(self.temporary.cleanup)
        self.work = Path(self.temporary.name)
        self.root = self.work / "document tree"
        self.root.mkdir()

    def command(self, mode, args=(), code=0, root=None):
        result = subprocess.run([str(BINARY), mode, str(root or self.root),
                                 "--output", "json", "--silent", *map(str, args)],
                                cwd=self.work, capture_output=True, encoding="utf-8", timeout=60)
        self.assertEqual(result.returncode, code, f"{mode} {args}: {result.stderr}")
        self.assertEqual(len(result.stdout.splitlines()), 1, result.stdout)
        self.assertNotIn("\x1b", result.stdout)
        value = json.loads(result.stdout)
        self.assertEqual(value["schema_version"], 1)
        self.assertEqual(value["exit_code"], code)
        self.assertIn(value["status"], ["success", "verification_failed", "error", "no_files_processed", "canceled"])
        return value

    def test_independent_digest_vectors_for_every_algorithm_and_format(self):
        for algorithm in VECTORS["samples"][0]["digests"]:
            for format_name in ["sum", "hashcheck"]:
                with self.subTest(algorithm=algorithm, format=format_name):
                    root = self.work / f"{algorithm}-{format_name}"
                    root.mkdir()
                    expected = {}
                    for sample in VECTORS["samples"]:
                        name = f"nested café/input {sample['length']}.bin"
                        path = root / name
                        path.parent.mkdir(exist_ok=True)
                        path.write_bytes(bytes(i % 251 for i in range(sample["length"])))
                        expected[name] = (sample["length"], sample["digests"][algorithm])
                    options = ["--type", algorithm, "--checksum-file", "manifest", "--format", format_name]
                    created = self.command("create", options, root=root)
                    self.assertEqual(created["stats"]["files_processed"], 3)
                    self.assertEqual(created["stats"]["bytes_hashed"], 2049)
                    entries = {}
                    for line in (root / "manifest").read_text(encoding="utf-8").splitlines():
                        if format_name == "sum":
                            digest, name = line.split(" *", 1)
                            name = name.replace("\\", "/")
                            size = expected[name][0]
                        else:
                            name, size, digest = line.split("|")
                            size = int(size)
                        name = name.replace("\\", "/")
                        entries[name] = (size, digest)
                    self.assertEqual(entries, expected)
                    self.command("verify", ["--type", algorithm, "--checksum-file", "manifest"], root=root)

    def test_foreign_manifests_crlf_text_markers_and_uppercase_digests(self):
        nested = self.root / "sub"
        nested.mkdir()
        (nested / "café file").write_bytes(b"portable")
        digest = hashlib.sha256(b"portable").hexdigest()
        manifest = self.root / "SHA256SUMS"
        # Read an independently authored Windows-style manifest on every platform.
        for content in [f"{digest.upper()}  sub\\café file\r\n",
                        f"sub/café file|8|{digest.upper()}\r\n"]:
            with self.subTest(content=content):
                manifest.write_bytes(content.encode("utf-8"))
                before = manifest.read_bytes()
                self.command("verify")
                self.assertEqual(manifest.read_bytes(), before)
                (nested / "café file").write_bytes(b"modified")
                self.command("update")
                text = manifest.read_text(encoding="utf-8")
                self.assertIn(hashlib.sha256(b"modified").hexdigest(), text)
                self.command("verify")
                (nested / "café file").write_bytes(b"portable")

    def test_edit_session_dry_run_filters_and_append_report(self):
        nested = self.root / "docs"
        nested.mkdir()
        (nested / "keep").write_bytes(b"data")
        (nested / "removed").write_bytes(b"gone")
        (self.root / "excluded").write_bytes(b"outside selection")
        self.command("create", ["--type", "SHA256"])
        manifest = self.root / "SHA256SUMS"
        before = manifest.read_bytes()
        (nested / "keep").write_bytes(b"tada")
        (nested / "removed").unlink()
        (nested / "added").write_bytes(b"new")
        (self.root / "excluded").unlink()
        report = self.work / "history.txt"
        report.write_text("existing history\n", encoding="utf-8")
        options = ["--match", "^docs/", "--remove-missing", "--report-file", report]
        preview = self.command("update", [*options, "--dry-run"])
        self.assertEqual(manifest.read_bytes(), before)
        self.assertFalse(preview["manifest_written"])
        self.assertEqual(preview["stats"]["files_updated"], 1)
        self.assertEqual(preview["stats"]["files_removed"], 1)
        self.command("update", options)
        entries = manifest.read_text(encoding="utf-8")
        self.assertIn("excluded", entries)
        self.assertNotIn("docs/removed", entries.replace("\\", "/"))
        self.assertIn(hashlib.sha256(b"tada").hexdigest(), entries)
        self.assertIn(hashlib.sha256(b"new").hexdigest(), entries)
        self.assertTrue(report.read_text(encoding="utf-8").startswith("existing history\n"))
        self.assertIn("dry run", report.read_text(encoding="utf-8"))
        self.command("verify", ["--match", "^docs/"])
        self.command("verify", code=1)

    def test_manifest_path_escape_and_duplicates_preserve_sentinels(self):
        (self.root / "file").write_bytes(b"data")
        sentinel = self.work / "outside"
        sentinel.write_bytes(b"do not change")
        digest = hashlib.sha256(b"data").hexdigest()
        manifest = self.root / "SHA256SUMS"
        for content in [f"{digest} *../outside\n", f"{digest} *{sentinel.as_posix()}\n",
                        f"{digest} *file\n{digest} *./file\n", "file|invalid size|digest\n"]:
            with self.subTest(content=content):
                manifest.write_bytes(content.encode())
                before = manifest.read_bytes()
                self.command("verify", code=2)
                self.command("update", code=2)
                self.assertEqual(manifest.read_bytes(), before)
                self.assertEqual(sentinel.read_bytes(), b"do not change")

    def test_output_hard_links_are_excluded_and_report_alias_is_refused(self):
        (self.root / "file").write_bytes(b"data")
        self.command("create")
        manifest = self.root / "SHA1SUMS"
        before = manifest.read_bytes()
        alias = self.root / "manifest alias"
        os.link(manifest, alias)
        self.command("update", ["--report-file", alias], 2)
        self.assertEqual(manifest.read_bytes(), before)
        alias.unlink()
        output = self.root / "result.json"
        output.touch()
        output_alias = self.root / "result alias"
        os.link(output, output_alias)
        with output.open("wb") as stream:
            child = subprocess.run([str(BINARY), "verify", str(self.root), "--extra", "--output", "json"],
                                   stdout=stream, stderr=subprocess.PIPE, timeout=60)
        self.assertEqual(child.returncode, 0, child.stderr)
        self.assertEqual(json.loads(output.read_text())["stats"]["extra_files"], 0)
        self.assertEqual(manifest.read_bytes(), before)

    def test_bounded_nested_binary_tree_matches_independent_sha256(self):
        expected = {}
        for index in range(300):
            name = f"group {index % 7}/file {index}.bin"
            path = self.root / name
            path.parent.mkdir(exist_ok=True)
            data = bytes((index + i) % 256 for i in range(index % 512))
            path.write_bytes(data)
            expected[name] = hashlib.sha256(data).hexdigest()
        self.command("create", ["--type", "SHA256"])
        found = {}
        for line in (self.root / "SHA256SUMS").read_text().splitlines():
            digest, name = line.split(" *", 1)
            found[name.replace("\\", "/")] = digest
        self.assertEqual(found, expected)
        verified = self.command("verify")
        self.assertEqual(verified["stats"]["files_processed"], 300)
        self.assertEqual(verified["stats"]["bytes_hashed"], sum(i % 512 for i in range(300)))

    def test_completions_load_in_a_disposable_native_shell(self):
        # No profile is read or changed. Check actual completion, not just script syntax.
        if os.name == "nt":
            shell = shutil.which("pwsh")
            self.assertIsNotNone(shell, "PowerShell 7 is required for Windows E2E")
            script = self.work / "completion.ps1"
            script.write_bytes(subprocess.check_output([str(BINARY), "completions", "powershell"], timeout=30))
            env = os.environ.copy()
            env["HSHCHK_COMPLETION_SCRIPT"] = str(script)
            command = ("$ErrorActionPreference='Stop'; . $env:HSHCHK_COMPLETION_SCRIPT; "
                       "$inputLine='hshchk verify --out'; "
                       "(TabExpansion2 $inputLine $inputLine.Length).CompletionMatches.CompletionText")
            result = subprocess.run([shell, "-NoProfile", "-NonInteractive", "-Command", command],
                                    env=env, capture_output=True, encoding="utf-8", timeout=30)
        else:
            shell = shutil.which("bash")
            self.assertIsNotNone(shell, "Bash is required for Unix E2E")
            script = self.work / "completion.bash"
            script.write_bytes(subprocess.check_output([str(BINARY), "completions", "bash"], timeout=30))
            result = subprocess.run([shell, "--noprofile", "--norc", "-c",
                                    'source "$1"; COMP_WORDS=(hshchk verify --out); COMP_CWORD=2; '
                                    '_hshchk hshchk --out verify; printf "%s\\n" "${COMPREPLY[@]}"', "e2e", str(script)],
                                    capture_output=True, encoding="utf-8", timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--output", result.stdout.splitlines())

    def test_read_only_stdout_reports_errors_without_losing_saved_manifest(self):
        (self.root / "file").write_bytes(b"data")
        output = self.work / "read-only-output"
        output.write_bytes(b"preserve output")
        for output_mode in ["json", "text"]:
            with self.subTest(output_mode=output_mode), output.open("rb") as stream:
                child = subprocess.run([str(BINARY), "create", str(self.root), "--overwrite", "--output", output_mode],
                                       stdout=stream, stderr=subprocess.PIPE, timeout=30)
                self.assertEqual(child.returncode, 2, child.stderr)
                self.assertIn(b"Cannot write", child.stderr)
            self.assertEqual(output.read_bytes(), b"preserve output")
            self.command("verify")
        with output.open("rb") as stream:
            child = subprocess.run([str(BINARY), "completions", "bash"], stdout=stream,
                                   stderr=subprocess.PIPE, timeout=30)
        self.assertEqual(child.returncode, 2, child.stderr)
        self.assertIn(b"Cannot write completions", child.stderr)
        self.assertEqual(output.read_bytes(), b"preserve output")

    def test_unwritable_progress_stream_preserves_manifest_and_returns_json_error(self):
        file = self.root / "file"
        file.write_bytes(b"data")
        self.command("create")
        manifest = self.root / "SHA1SUMS"
        before = manifest.read_bytes()
        file.write_bytes(b"")
        error_stream = self.work / "read-only-error-output"
        error_stream.write_bytes(b"preserve error output")
        with error_stream.open("rb") as stream:
            child = subprocess.run([str(BINARY), "update", str(self.root), "--output", "json", "--progress", "always"],
                                   stdout=subprocess.PIPE, stderr=stream, timeout=30)
        self.assertEqual(child.returncode, 2)
        value = json.loads(child.stdout)
        self.assertEqual(value["status"], "error")
        self.assertFalse(value["manifest_written"])
        self.assertEqual(manifest.read_bytes(), before)
        self.assertEqual(error_stream.read_bytes(), b"preserve error output")

    @unittest.skipIf(os.name == "nt", "Unix link/filename semantics")
    def test_file_symlinks_broken_links_and_directory_cycles(self):
        source = self.work / "source"
        source.write_bytes(b"linked contents")
        link = self.root / "file link"
        link.symlink_to(source)
        self.command("create", ["--type", "SHA256"])
        manifest = self.root / "SHA256SUMS"
        self.assertIn(hashlib.sha256(b"linked contents").hexdigest(), manifest.read_text())
        before = manifest.read_bytes()
        source.unlink()
        self.command("update", code=2)
        self.assertEqual(manifest.read_bytes(), before)
        link.unlink()
        (self.root / "cycle").symlink_to(self.root, target_is_directory=True)
        self.command("update", code=2)
        self.assertEqual(manifest.read_bytes(), before)

    @unittest.skipUnless(os.name == "nt", "Windows NTFS junction semantics")
    def test_directory_junctions_are_rejected_without_replacing_manifest(self):
        (self.root / "file").write_bytes(b"data")
        self.command("create")
        manifest = self.root / "SHA1SUMS"
        before = manifest.read_bytes()
        junction = self.root / "cycle"
        result = subprocess.run(["cmd.exe", "/d", "/c", "mklink", "/J", str(junction), str(self.root)],
                                capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        try:
            failed = self.command("update", code=2)
            self.assertTrue(any("directory symlinks are not followed" in row["message"]
                                for row in failed["diagnostics"]))
            self.assertEqual(manifest.read_bytes(), before)
        finally:
            # Remove only this junction, never recursively remove its target.
            os.rmdir(junction)

    @unittest.skipIf(os.name == "nt", "Unix representability and permission semantics")
    def test_unsupported_names_and_unwritable_destination_preserve_manifest(self):
        file = self.root / "file"
        file.write_bytes(b"data")
        self.command("create")
        manifest = self.root / "SHA1SUMS"
        before = manifest.read_bytes()
        for name in ["new\nline", "back\\slash"]:
            with self.subTest(name=name):
                bad = self.root / name
                bad.write_bytes(b"invalid name")
                self.command("update", code=2)
                self.assertEqual(manifest.read_bytes(), before)
                bad.unlink()
        self.assertNotEqual(os.geteuid(), 0, "Permission checks must run as an unprivileged user")
        mode = self.root.stat().st_mode & 0o777
        try:
            self.root.chmod(0o500)
            self.command("update", code=2)
        finally:
            self.root.chmod(mode)
        self.assertEqual(manifest.read_bytes(), before)
        self.assertEqual(sorted(path.name for path in self.root.iterdir()), ["SHA1SUMS", "file"])


def main():
    global BINARY
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--list", action="store_true")
    args = parser.parse_args()
    names = unittest.defaultTestLoader.getTestCaseNames(CliWorkflows)
    if args.list:
        print("\n".join(names))
        return
    if args.binary is None:
        parser.error("--binary is required")
    BINARY = args.binary.resolve(strict=True)
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(CliWorkflows))
    raise SystemExit(0 if result.wasSuccessful() else 1)


if __name__ == "__main__":
    main()
