#!/usr/bin/env python3
"""Integration checks for locally built release packages."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
import package_release as release

BINARY = None
TARGET = None


class PackagingTests(unittest.TestCase):
    def test_archive_contains_binary_docs_completions_and_matching_checksum(self):
        with tempfile.TemporaryDirectory(prefix="hshchk-package-test-") as directory:
            output = Path(directory)
            assets = release.package(BINARY, BINARY, TARGET, output, run_smoke=True)
            self.assertEqual(len(assets), 1)
            archive = assets[0]
            if archive.suffix == ".zip":
                with zipfile.ZipFile(archive) as reader:
                    names = reader.namelist()
                    get = reader.read
                    self.check_contents(names, get)
            else:
                with tarfile.open(archive) as reader:
                    names = reader.getnames()
                    self.check_contents(names, lambda name: reader.extractfile(name).read())
            with archive.open("rb") as reader:
                digest = hashlib.file_digest(reader, "sha256").hexdigest()
            self.assertEqual(archive.with_name(archive.name + ".sha256").read_text(), f"{digest}  {archive.name}\n")
            self.assertEqual(len(list(output.iterdir())), 2)  # staging/extraction directories are cleaned

    def check_contents(self, names, get):
        root = f"hshchk-{release.version()}-{TARGET}"
        executable = "hshchk.exe" if "windows" in TARGET else "hshchk"
        for name in [executable, "README.md", "LICENSE", "CHANGELOG.md", "docs/json-output-v1.schema.json"]:
            self.assertIn(f"{root}/{name}", names)
        for filename in release.SHELL_FILES.values():
            self.assertIn(f"{root}/completions/{filename}", names)
            self.assertIn(b"hshchk", get(f"{root}/completions/{filename}"))
        self.assertTrue(all(not name.startswith("/") and ".." not in Path(name).parts for name in names))

    def test_invalid_versions_and_targets_fail_before_writing_assets(self):
        with tempfile.TemporaryDirectory(prefix="hshchk-package-test-") as directory:
            output = Path(directory)
            with self.assertRaises(ValueError):
                release.package(BINARY, BINARY, "../escape", output)
            with self.assertRaises(ValueError):
                release.package(BINARY, BINARY, TARGET, output, expected_version="9.9.9")
            self.assertEqual(list(output.iterdir()), [])

    def test_completion_scripts_parse_in_available_native_shells(self):
        import shutil
        with tempfile.TemporaryDirectory(prefix="hshchk-completion-test-") as directory:
            root = Path(directory)
            bash = shutil.which("bash")
            if bash:
                script = root / "hshchk.bash"
                script.write_text(release.run(BINARY, ["completions", "bash"]), encoding="utf-8")
                subprocess.run([bash, "-n", str(script)], check=True)
            powershell = shutil.which("pwsh")
            if powershell:
                script = root / "hshchk.ps1"
                script.write_text(release.run(BINARY, ["completions", "powershell"]), encoding="utf-8")
                environment = os.environ.copy()
                environment["HSHCHK_COMPLETION_SCRIPT"] = str(script)
                subprocess.run([powershell, "-NoProfile", "-NonInteractive", "-Command",
                    "$ErrorActionPreference='Stop'; $tokens=$null; $parseErrors=$null; [System.Management.Automation.Language.Parser]::ParseFile($env:HSHCHK_COMPLETION_SCRIPT,[ref]$tokens,[ref]$parseErrors) | Out-Null; if ($parseErrors.Count) { $parseErrors | Write-Error; exit 1 }"], check=True, env=environment)


def main():
    global BINARY, TARGET
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", required=True)
    args, remaining = parser.parse_known_args()
    BINARY = args.binary.resolve(strict=True)
    TARGET = args.target
    unittest.main(argv=[sys.argv[0], *remaining])


if __name__ == "__main__":
    main()
