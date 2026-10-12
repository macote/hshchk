#!/usr/bin/env python3
"""Build inspectable release archives with stdlib Python; never publish a release."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent
SHELL_FILES = {"bash": "hshchk.bash", "zsh": "_hshchk", "fish": "hshchk.fish", "powershell": "hshchk.ps1", "elvish": "hshchk.elv"}


def version(expected=None):
    value = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", value):
        raise ValueError("package version must be a SemVer release version")
    if expected and expected.removeprefix("v") != value:
        raise ValueError(f"tag/version {expected!r} does not match Cargo.toml version {value!r}")
    return value


def run(binary, args, code=0):
    environment = os.environ.copy()
    if os.name == "nt":
        # Installed compiler DLLs must not mask missing runtime dependencies in a shipped executable.
        environment["PATH"] = os.pathsep.join(part for part in environment.get("PATH", "").split(os.pathsep)
            if not any(marker in part.lower() for marker in ["mingw32", "mingw64", "ucrt64"]))
    result = subprocess.run([str(binary), *map(str, args)], check=False, capture_output=True, text=True, encoding="utf-8", env=environment)
    if result.returncode != code:
        raise RuntimeError(f"{args}: expected exit {code}, got {result.returncode}: {result.stderr}")
    return result.stdout


def smoke(binary):
    """Exercise the optimized or extracted binary, not a debug-build substitute."""
    with tempfile.TemporaryDirectory(prefix="hshchk-smoke-") as directory:
        root = Path(directory)
        file = root / "file"
        file.write_text("data", encoding="utf-8")
        for operation in ["create", "verify"]:
            report = json.loads(run(binary, [operation, root, "--output", "json"]))
            if report["schema_version"] != 1 or report["status"] != "success":
                raise RuntimeError(f"unexpected {operation} result: {report}")
        file.write_text("tada", encoding="utf-8")
        report = json.loads(run(binary, ["verify", root, "--output", "json"], code=1))
        if report["status"] != "verification_failed":
            raise RuntimeError(f"unexpected mismatch result: {report}")
        run(binary, ["update", root, "--output", "json"])
        run(binary, ["verify", root, "--silent"])
    for shell in SHELL_FILES:
        if "hshchk" not in run(binary, ["completions", shell]):
            raise RuntimeError(f"empty completion script for {shell}")


def checksum(path):
    with path.open("rb") as reader:
        digest = hashlib.file_digest(reader, "sha256").hexdigest()
    path.with_name(path.name + ".sha256").write_text(f"{digest}  {path.name}\n", encoding="utf-8")


def deb_package(staging, output, release_version, target):
    if target not in ["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"]:
        raise ValueError("Debian packaging supports the two x86_64 Linux release targets")
    musl = target.endswith("musl")
    package = "hshchk-musl" if musl else "hshchk"
    opposite = "hshchk" if musl else "hshchk-musl"
    with tempfile.TemporaryDirectory(prefix="hshchk-deb-", dir=output) as directory:
        root = Path(directory)
        binary_dir = root / "usr/bin"
        binary_dir.mkdir(parents=True)
        shutil.copy2(staging / "hshchk", binary_dir / "hshchk")
        (binary_dir / "hshchk").chmod(0o755)
        documentation = root / "usr/share/doc" / package
        shutil.copytree(staging / "docs", documentation / "docs")
        for name in ["README.md", "LICENSE", "CHANGELOG.md"]:
            shutil.copy2(staging / name, documentation / name)
        shutil.copy2(staging / "LICENSE", documentation / "copyright")
        completions = {
            "hshchk.bash": "usr/share/bash-completion/completions/hshchk",
            "_hshchk": "usr/share/zsh/vendor-completions/_hshchk",
            "hshchk.fish": "usr/share/fish/vendor_completions.d/hshchk.fish",
            "hshchk.ps1": f"usr/share/{package}/completions/hshchk.ps1",
            "hshchk.elv": f"usr/share/{package}/completions/hshchk.elv",
        }
        for source, destination in completions.items():
            path = root / destination
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(staging / "completions" / source, path)
        control_dir = root / "DEBIAN"
        control_dir.mkdir()
        control = (f"Package: {package}\nVersion: {release_version}\nArchitecture: amd64\n"
                   "Section: utils\nPriority: optional\nMaintainer: Marc-André Côté <mac@live.ca>\n"
                   "Homepage: https://github.com/macote/hshchk\n"
                   f"Provides: hshchk\nConflicts: {opposite}\n")
        if not musl:
            control += "Depends: libc6 (>= 2.35), libgcc-s1\n"
        control += "Description: Create, verify, and update file tree checksums.\n"
        (control_dir / "control").write_text(control, encoding="utf-8")
        asset = output / f"hshchk-{release_version}-{target}.deb"
        subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(root), str(asset)], check=True)
        subprocess.run(["dpkg-deb", "--info", str(asset)], check=True, stdout=subprocess.DEVNULL)
        checksum(asset)
    return asset


def package(binary, generator, target, output, expected_version=None, run_smoke=False, deb=False):
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]+", target):
        raise ValueError("invalid target identifier")
    release_version = version(expected_version)
    binary = binary.resolve(strict=True)
    generator = generator.resolve(strict=True)
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    name = f"hshchk-{release_version}-{target}"
    executable = "hshchk.exe" if "windows" in target else "hshchk"
    with tempfile.TemporaryDirectory(prefix="hshchk-stage-", dir=output) as directory:
        staging = Path(directory) / name
        staging.mkdir()
        shutil.copy2(binary, staging / executable)
        for file in ["README.md", "LICENSE", "CHANGELOG.md"]:
            shutil.copy2(ROOT / file, staging / file)
        shutil.copytree(ROOT / "docs", staging / "docs")
        completion_dir = staging / "completions"
        completion_dir.mkdir()
        for shell, filename in SHELL_FILES.items():
            (completion_dir / filename).write_text(run(generator, ["completions", shell]), encoding="utf-8", newline="\n")
        if "windows" in target:
            asset = output / f"{name}.zip"
            with zipfile.ZipFile(asset, "w", zipfile.ZIP_DEFLATED) as archive:
                for path in sorted(staging.rglob("*")):
                    if path.is_file():
                        archive.write(path, path.relative_to(staging.parent))
            with zipfile.ZipFile(asset) as archive:
                if archive.testzip() is not None:
                    raise RuntimeError("corrupt zip archive")
        else:
            asset = output / f"{name}.tar.gz"
            with tarfile.open(asset, "w:gz") as archive:
                archive.add(staging, arcname=name)
            with tarfile.open(asset) as archive:
                names = archive.getnames()
                if f"{name}/{executable}" not in names:
                    raise RuntimeError("archive is missing its executable")
        checksum(asset)
        if run_smoke:
            with tempfile.TemporaryDirectory(prefix="hshchk-extract-", dir=output) as extract_dir:
                extracted = Path(extract_dir) / executable
                if "windows" in target:
                    with zipfile.ZipFile(asset) as archive:
                        extracted.write_bytes(archive.read(f"{name}/{executable}"))
                else:
                    with tarfile.open(asset) as archive:
                        with archive.extractfile(f"{name}/{executable}") as reader:
                            extracted.write_bytes(reader.read())
                    extracted.chmod(0o755)
                smoke(extracted)
        assets = [asset]
        if deb:
            assets.append(deb_package(staging, output, release_version, target))
        return assets


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata", action="store_true")
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--generator", type=Path, help="Native executable used to generate completions for cross-built packages")
    parser.add_argument("--target")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "dist")
    parser.add_argument("--version")
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--deb", action="store_true")
    args = parser.parse_args()
    if args.metadata:
        print(f"version={version(args.version or os.environ.get('TAG_NAME'))}")
        return
    if args.binary is None or args.target is None:
        parser.error("--binary and --target are required for packaging")
    for asset in package(args.binary, args.generator or args.binary, args.target, args.output_dir, args.version, args.smoke, args.deb):
        print(asset)


if __name__ == "__main__":
    main()
