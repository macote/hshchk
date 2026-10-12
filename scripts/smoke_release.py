#!/usr/bin/env python3
"""Test a release archive without installing it. Requires only Python 3.9+ stdlib."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile
import zipfile


def require(condition, message):
    if not condition:
        raise AssertionError(message)


class SmokeTests:
    def __init__(self, binary, root):
        self.binary = binary
        self.root = root
        self.commands = 0
        self.environment = os.environ.copy()
        if os.name == "nt":
            self.environment["PATH"] = os.pathsep.join(
                part for part in self.environment.get("PATH", "").split(os.pathsep)
                if not any(marker in part.lower() for marker in ["mingw32", "mingw64", "ucrt64"]))

    def invoke(self, args, code=0, structured=True):
        result = subprocess.run([str(self.binary), *map(str, args)], capture_output=True,
                                encoding="utf-8", timeout=30, env=self.environment)
        self.commands += 1
        require(result.returncode == code,
                f"{args}: expected {code}, got {result.returncode}: {result.stderr}")
        if not structured:
            return result.stdout
        require(len(result.stdout.splitlines()) == 1, "JSON stdout must contain one line")
        require("\x1b" not in result.stdout, "JSON stdout contains terminal escapes")
        value = json.loads(result.stdout)
        require(value["schema_version"] == 1 and value["exit_code"] == code, "invalid JSON envelope")
        require(isinstance(value["diagnostics"], list), "missing diagnostics")
        return value

    def operation(self, mode, root, args=(), code=0):
        return self.invoke([mode, root, "--output", "json", "--silent", *args], code)

    def lifecycle(self, algorithm, manifest_format):
        root = self.root / f"{algorithm}-{manifest_format}"
        root.mkdir()
        file = root / "café file.txt"
        file.write_bytes(b"data")
        selected = ["--type", algorithm, "--checksum-file", "checksums.txt"]
        created = self.operation("create", root, [*selected, "--format", manifest_format])
        require(created["manifest_written"] and created["stats"]["bytes_hashed"] == 4, "create failed")
        manifest = root / "checksums.txt"
        original = manifest.read_bytes()
        self.operation("verify", root, selected)
        self.operation("create", root, selected, 2)
        require(manifest.read_bytes() == original, "refused creation changed manifest")
        file.write_bytes(b"tada")
        failed = self.operation("verify", root, selected, 1)
        require(failed["status"] == "verification_failed", "same-size edit was missed")
        preview = self.operation("update", root, [*selected, "--dry-run"])
        require(preview["dry_run"] and not preview["manifest_written"], "dry-run saved manifest")
        require(manifest.read_bytes() == original, "dry-run changed manifest")
        updated = self.operation("update", root, selected)
        require(updated["manifest_written"] and updated["stats"]["files_updated"] == 1, "update missed edit")
        self.operation("verify", root, selected)
        if manifest_format == "hashcheck":
            sized = self.operation("verify", root, [*selected, "--size"])
            require(sized["stats"]["bytes_hashed"] == 0, "size verification hashed data")
        (root / "extra.txt").write_bytes(b"extra")
        extra = self.operation("verify", root, [*selected, "--extra"])
        require(extra["stats"]["extra_files"] == 1, "extra-file warning absent")
        file.unlink()
        missing = self.operation("verify", root, selected, 1)
        require(missing["stats"]["missing_files"] == 1, "missing file absent from result")
        removed = self.operation("update", root, [*selected, "--remove-missing"])
        require(removed["stats"]["files_removed"] == 1, "missing entry was not removed")
        self.operation("verify", root, selected)

    def edges(self):
        root = self.root / "edge-cases"
        root.mkdir()
        (root / "keep").write_bytes(b"keep")
        (root / "gone").write_bytes(b"gone")
        self.operation("create", root)
        manifest = root / "SHA1SUMS"
        original = manifest.read_bytes()
        (root / "gone").unlink()
        filtered = self.operation("update", root, ["--remove-missing", "--match", "^keep$"])
        require(filtered["stats"]["files_removed"] == 0 and manifest.read_bytes() == original,
                "filtered update removed excluded entry")
        self.operation("verify", root, code=1)
        report = self.root / "report.txt"
        self.operation("update", root, ["--remove-missing", "--report-file", report])
        require(report.stat().st_size > 0, "human report missing")
        (root / "SHA256SUMS").write_bytes(b"")
        self.operation("verify", root, code=2)
        (root / "SHA256SUMS").unlink()
        self.operation("verify", root, ["--match", "["], 2)
        empty = self.root / "empty"
        empty.mkdir()
        self.operation("create", empty, code=2)
        require(not list(empty.iterdir()), "empty run created manifest")
        malformed = self.root / "malformed"
        malformed.mkdir()
        (malformed / "SHA1SUMS").write_text("not a manifest\n", encoding="utf-8")
        self.operation("verify", malformed, code=2)
        # Open output before starting the child, as a shell redirect does.
        redirected = root / "result.json"
        with redirected.open("wb") as stream:
            result = subprocess.run([str(self.binary), "verify", str(root), "--extra", "--output", "json"],
                                    stdout=stream, stderr=subprocess.PIPE, timeout=30, env=self.environment)
        self.commands += 1
        require(result.returncode == 0, "redirected verification failed")
        require(json.loads(redirected.read_text())["stats"]["extra_files"] == 0,
                "redirected stdout was included as an extra file")
        for shell in ["bash", "zsh", "fish", "powershell", "elvish"]:
            script = self.invoke(["completions", shell], structured=False)
            require("hshchk" in script and "checksum-file" in script, f"incomplete {shell} completion")
            executable = shutil.which(shell)
            if shell in ["bash", "zsh", "fish"] and executable:
                script_path = self.root / f"completion.{shell}"
                script_path.write_text(script, encoding="utf-8")
                subprocess.run([executable, "-n", str(script_path)], check=True, timeout=30)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--version", default="0.2.0")
    args = parser.parse_args()
    archive = args.archive.resolve(strict=True)
    sidecar = archive.with_name(archive.name + ".sha256")
    expected = sidecar.read_text(encoding="utf-8").strip().split("  ", 1)
    require(len(expected) == 2 and expected[1] == archive.name, "invalid checksum sidecar")
    digest = hashlib.sha256()
    with archive.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    require(digest.hexdigest() == expected[0], "archive checksum mismatch")
    with tempfile.TemporaryDirectory(prefix="hshchk-release-smoke-") as directory:
        root = Path(directory)
        executable = "hshchk.exe" if os.name == "nt" else "hshchk"
        with (zipfile.ZipFile(archive) if archive.suffix == ".zip" else tarfile.open(archive)) as reader:
            names = reader.namelist() if archive.suffix == ".zip" else reader.getnames()
            binaries = [name for name in names if name.endswith("/" + executable)]
            require(len(binaries) == 1, "archive must contain exactly one executable")
            prefix = binaries[0].rsplit("/", 1)[0]
            for name in ["README.md", "LICENSE", "CHANGELOG.md", "docs/json-output-v1.schema.json"]:
                require(prefix + "/" + name in names, f"missing packaged {name}")
            binary = root / executable
            if archive.suffix == ".zip":
                binary.write_bytes(reader.read(binaries[0]))
            else:
                with reader.extractfile(binaries[0]) as stream:
                    binary.write_bytes(stream.read())
                binary.chmod(0o755)
        tests = SmokeTests(binary, root)
        version = tests.invoke(["--version"], structured=False).strip()
        require(version == f"hshchk {args.version}", f"wrong version: {version}")
        for algorithm in ["MD5", "SHA1", "SHA256", "SHA512", "Blake2b", "Blake2s", "Blake3"]:
            for manifest_format in ["sum", "hashcheck"]:
                tests.lifecycle(algorithm, manifest_format)
        tests.edges()
        summary = {"platform": platform.system(), "architecture": platform.machine(),
                   "version": args.version, "archive": archive.name, "sha256": expected[0],
                   "cli_checks": tests.commands, "status": "passed"}
    # TemporaryDirectory has removed only the isolated test directory before success is reported.
    print(json.dumps(summary))


if __name__ == "__main__":
    main()
