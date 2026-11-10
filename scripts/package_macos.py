#!/usr/bin/env python3
"""Build the desktop binary and create a signed local macOS application bundle."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile


REPOSITORY = Path(__file__).resolve().parents[1]
PACKAGE = "opentrickster-desktop"
EXECUTABLE = "opentrickster"
RELEASE_ENVIRONMENT = "OPENTRICKSTER_RELEASE"
RELEASE_VERSION = re.compile(r"(0|[1-9][0-9]*)[.](0|[1-9][0-9]*)[.](0|[1-9][0-9]*)")
FONT_LICENSES = (
    "IBMPlexSans-OFL.txt",
    "IBMPlexSansThai-OFL.txt",
    "IBMPlexMono-OFL.txt",
)


class PackageError(Exception):
    pass


def capture(arguments: list[str], repository: Path) -> str:
    return subprocess.check_output(arguments, cwd=repository, text=True).strip()


def check_destination(destination: Path) -> None:
    if destination.suffix != ".app":
        raise PackageError("The output must have an .app extension.")
    if os.path.lexists(destination):
        raise PackageError(f"Output already exists; choose a new path: {destination}")


def desktop_package(repository: Path) -> dict:
    metadata = json.loads(
        capture(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
            repository,
        )
    )
    for package in metadata["packages"]:
        if package["name"] == PACKAGE and package["id"] in metadata["workspace_members"]:
            return package
    raise PackageError("The desktop workspace member has not been integrated.")


def build_binary(
    repository: Path, package: dict, target: str | None, debug: bool, release_version: str | None
) -> Path:
    # Only a release build carries the version; an ambient value must not leak into other builds.
    environment = {k: v for k, v in os.environ.items() if k != RELEASE_ENVIRONMENT}
    if release_version:
        environment[RELEASE_ENVIRONMENT] = release_version
    arguments = [
        "cargo", "build", "--locked", "--package", PACKAGE, "--bin", EXECUTABLE,
        "--message-format=json-render-diagnostics",
    ]
    if not debug:
        arguments.append("--release")
    if target:
        arguments.extend(["--target", target])
    executable = None
    with subprocess.Popen(
        arguments, cwd=repository, env=environment, stdout=subprocess.PIPE, text=True
    ) as build:
        assert build.stdout is not None
        for line in build.stdout:
            message = json.loads(line)
            if message.get("reason") == "compiler-message":
                rendered = message.get("message", {}).get("rendered")
                if rendered:
                    print(rendered, file=sys.stderr, end="")
            if (
                message.get("reason") == "compiler-artifact"
                and message.get("package_id") == package["id"]
                and message.get("target", {}).get("name") == EXECUTABLE
                and "bin" in message.get("target", {}).get("kind", [])
                and message.get("executable")
            ):
                executable = Path(message["executable"])
        if build.wait() != 0:
            raise PackageError("The desktop build failed; no application was published.")
    if executable is None or not executable.is_file():
        raise PackageError("Cargo did not report a desktop executable.")
    return executable


def inspect_binary(binary: Path, repository: Path) -> dict:
    architectures = capture(["lipo", "-archs", str(binary)], repository).split()
    if not architectures or not set(architectures) <= {"arm64", "x86_64"}:
        raise PackageError("The desktop executable must be a macOS arm64 or x86_64 binary.")
    commands = capture(["xcrun", "vtool", "-show-build", str(binary)], repository)
    minima = re.findall(r"^\s+minos\s+(\d+(?:\.\d+){0,2})\s*$", commands, re.MULTILINE)
    minima.extend(re.findall(
        r"cmd LC_VERSION_MIN_MACOSX\s+cmdsize \d+\s+version (\d+(?:\.\d+){0,2})",
        commands,
    ))
    if not minima or not ("platform MACOS" in commands or "LC_VERSION_MIN_MACOSX" in commands):
        raise PackageError("Cannot determine the executable's macOS deployment target.")
    minimum = max(minima, key=lambda value: tuple(int(part) for part in value.split(".")))
    libraries = capture(["otool", "-L", str(binary)], repository)
    for line in libraries.splitlines():
        if not line.startswith("\t"):
            continue
        library = line.strip().split(" (compatibility version", 1)[0]
        if not library.startswith(("/System/Library/", "/usr/lib/")):
            raise PackageError(f"The executable requires an unbundled library: {library}")
    return {"architectures": architectures, "minimum_macos": minimum}


def assemble_bundle(
    repository: Path, binary: Path, version: str, destination: Path, provenance: dict
) -> dict:
    check_destination(destination)
    numeric_version = version.split("-", 1)[0].split("+", 1)[0]
    if not re.fullmatch(r"\d+\.\d+\.\d+", numeric_version):
        raise PackageError("The package needs a three-component numeric release version.")
    details = inspect_binary(binary, repository)
    licenses = [(repository / "LICENSE", "OpenTrickster-Apache-2.0.txt")]
    licenses.extend(
        (repository / "crates/desktop/resources/fonts" / name, name)
        for name in FONT_LICENSES
    )
    for source, _ in licenses:
        if not source.is_file():
            raise PackageError(f"Missing bundled license: {source.relative_to(repository)}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".opentrickster-package-", dir=destination.parent) as staging:
        bundle = Path(staging) / "OpenTrickster.app"
        contents = bundle / "Contents"
        macos = contents / "MacOS"
        resources = contents / "Resources"
        legal = resources / "Legal"
        macos.mkdir(parents=True)
        legal.mkdir(parents=True)
        shutil.copy2(binary, macos / EXECUTABLE)
        (macos / EXECUTABLE).chmod(0o755)
        info = {
            "CFBundleName": "OpenTrickster",
            "CFBundleDisplayName": "OpenTrickster",
            "CFBundleIdentifier": "org.opentrickster.desktop",
            "CFBundleExecutable": EXECUTABLE,
            "CFBundlePackageType": "APPL",
            "CFBundleInfoDictionaryVersion": "6.0",
            "CFBundleShortVersionString": numeric_version,
            "CFBundleVersion": numeric_version,
            "LSMinimumSystemVersion": details["minimum_macos"],
            "NSPrincipalClass": "NSApplication",
            "NSHighResolutionCapable": True,
            "NSSupportsAutomaticGraphicsSwitching": True,
        }
        with (contents / "Info.plist").open("wb") as stream:
            plistlib.dump(info, stream)
        for source, name in licenses:
            shutil.copy2(source, legal / name)
        build = {"version": version, **provenance, **details}
        (resources / "build.json").write_text(json.dumps(build, indent=2) + "\n", encoding="utf-8")
        subprocess.run(["plutil", "-lint", str(contents / "Info.plist")], check=True)
        subprocess.run(["codesign", "--force", "--sign", "-", str(bundle)], check=True)
        subprocess.run(["codesign", "--verify", "--strict", str(bundle)], check=True)
        # Reserve the final directory exclusively; rename must never replace an existing app.
        destination.mkdir()
        try:
            (bundle / "Contents").rename(destination / "Contents")
        except BaseException:
            destination.rmdir()
            raise
    return build


def release_version_argument(text: str) -> str:
    if not RELEASE_VERSION.fullmatch(text):
        raise argparse.ArgumentTypeError("expected MAJOR.MINOR.PATCH, for example 1.2.3")
    return text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("dist/OpenTrickster.app"))
    parser.add_argument("--debug", action="store_true", help="Package the debug build for local verification.")
    parser.add_argument("--target", choices=["aarch64-apple-darwin", "x86_64-apple-darwin"])
    parser.add_argument(
        "--release-version",
        type=release_version_argument,
        metavar="X.Y.Z",
        help="Package a release with this version. Needs a clean work tree and the release profile.",
    )
    arguments = parser.parse_args()
    if sys.platform != "darwin":
        raise PackageError("Build and package OpenTrickster on macOS.")
    if arguments.release_version and arguments.debug:
        raise PackageError("A release version needs the release profile; remove --debug.")
    destination = arguments.output.expanduser().absolute()
    check_destination(destination)
    package = desktop_package(REPOSITORY)
    revision = capture(["git", "rev-parse", "--verify", "HEAD"], REPOSITORY)
    dirty = bool(capture(["git", "status", "--porcelain"], REPOSITORY))
    if arguments.release_version and dirty:
        raise PackageError("A release version needs a clean work tree; commit or remove local changes.")
    binary = build_binary(REPOSITORY, package, arguments.target, arguments.debug, arguments.release_version)
    version = arguments.release_version or package["version"]
    details = assemble_bundle(REPOSITORY, binary, version, destination, {
        "source_revision": revision,
        "source_dirty": dirty,
        "profile": "debug" if arguments.debug else "release",
    })
    print(f"Packaged {destination} ({', '.join(details['architectures'])}); local ad-hoc signature verified.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (PackageError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Packaging failed: {error}", file=sys.stderr)
        sys.exit(1)
