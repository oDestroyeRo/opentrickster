#!/usr/bin/env python3
"""Build Desktop against an immutable Net snapshot and run the offline native fixture."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--net-revision", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    source = Path(__file__).resolve().parents[3]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    revision = subprocess.check_output(
        ["git", "rev-parse", "--verify", args.net_revision + "^{commit}"],
        cwd=source, text=True,
    ).strip()
    workspace = output / "workspace"
    workspace.mkdir()
    archive = subprocess.check_output(
        ["git", "archive", revision, "Cargo.toml", "Cargo.lock", "crates/engine", "crates/net"], cwd=source,
    )
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        for member in tar.getmembers():
            path = PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or member.issym() or member.islnk():
                raise ValueError("Unexpected archive member")
        tar.extractall(workspace)
    shutil.copytree(source / "crates/desktop", workspace / "crates/desktop")
    manifest = workspace / "Cargo.toml"
    text = manifest.read_text()
    if '"crates/desktop"' not in text:
        text = text.replace("members = [", 'members = ["crates/desktop",', 1)
    manifest.write_text(text)
    sources = {
        "desktop_base": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source, text=True).strip(),
        "net_revision": revision,
        "engine_source": subprocess.check_output(["git", "rev-parse", revision + ":crates/engine"], cwd=source, text=True).strip(),
        "desktop_files": {
            str(path.relative_to(source)): digest(path)
            for path in sorted((source / "crates/desktop").rglob("*")) if path.is_file()
        },
        "toolchain": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "account_used": False,
        "target_code_executed": False,
    }
    (output / "sources.json").write_text(json.dumps(sources, indent=2) + "\n")
    env = dict(os.environ, CARGO_TARGET_DIR=str(output / "target"))
    cargo = ["cargo"]
    package = ["--manifest-path", str(manifest), "-p", "opentrickster-desktop"]
    commands = [
        ("fmt", cargo + ["fmt"] + package + ["--", "--check"]),
        ("resolve", cargo + ["generate-lockfile", "--manifest-path", str(manifest), "--offline"]),
        ("fetch", cargo + ["fetch", "--manifest-path", str(manifest), "--locked", "--target", subprocess.check_output(["rustc", "-vV"], text=True).split("host: ")[1].splitlines()[0]]),
        ("test", cargo + ["test"] + package + ["--offline", "--lib"]),
        ("clippy", cargo + ["clippy"] + package + ["--offline", "--all-targets", "--no-deps", "--", "-Dwarnings"]),
        ("build", cargo + ["build"] + package + ["--offline"]),
        ("native", [str(output / "target/debug/opentrickster"), "--verify-input", "--verification-dir", str(output / "native")]),
    ]
    results = []
    try:
        for name, command in commands:
            print(name + ": " + " ".join(command), flush=True)
            with (output / (name + ".log")).open("w") as log:
                result = subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=600 if name != "native" else 30)
            results.append({"check": name, "command": command, "exit_code": result.returncode})
            if result.returncode:
                raise RuntimeError(name + " failed; read " + str(output / (name + ".log")))
        native = json.loads((output / "native/report.json").read_text())
        if native["passed"] is not True:
            raise RuntimeError("Native input fixture failed")
    finally:
        (output / "checks.json").write_text(json.dumps(results, indent=2) + "\n")
        if (workspace / "Cargo.lock").exists():
            sources["resolved_lock_sha256"] = digest(workspace / "Cargo.lock")
            (output / "sources.json").write_text(json.dumps(sources, indent=2) + "\n")
    print("Passed. Proof: " + str(output), flush=True)


if __name__ == "__main__":
    main()
