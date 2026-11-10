#!/usr/bin/env python3
"""Compute the next release version from Conventional Commits since the newest v* tag.

Run as `python3 -I scripts/release_version.py [--github-output FILE]`. The result is
`version=X.Y.Z` and `releasable=true|false`. When nothing is releasable, `version` is
the version of the newest tag.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from typing import NamedTuple


REPOSITORY = Path(__file__).resolve().parents[1]
NUMBER = "(0|[1-9][0-9]*)"
VERSION = re.compile(f"{NUMBER}[.]{NUMBER}[.]{NUMBER}")
TAG = re.compile(f"v{NUMBER}[.]{NUMBER}[.]{NUMBER}")
SUBJECT = re.compile(r"(?P<type>[A-Za-z]+)(?:\([^()]*\))?(?P<breaking>!)?: \S")
BREAKING_FOOTERS = ("BREAKING CHANGE:", "BREAKING-CHANGE:")

NONE, PATCH, MINOR, MAJOR = range(4)
TYPE_LEVELS = {"feat": MINOR, "fix": PATCH, "perf": PATCH, "revert": PATCH}


class Release(NamedTuple):
    version: str
    releasable: bool


def parse_version(text: str) -> tuple[int, int, int] | None:
    match = VERSION.fullmatch(text)
    return None if match is None else tuple(int(part) for part in match.groups())


def parse_tag(tag: str) -> tuple[int, int, int] | None:
    match = TAG.fullmatch(tag)
    return None if match is None else tuple(int(part) for part in match.groups())


def newest_tag(tags: list[str]) -> str | None:
    """The highest vX.Y.Z tag in semver order. Other tags are ignored."""
    versions = [(parsed, tag) for tag in tags if (parsed := parse_tag(tag)) is not None]
    return max(versions)[1] if versions else None


def bump_level(message: str) -> int:
    subject, _, body = message.strip().partition("\n")
    level = NONE
    match = SUBJECT.match(subject)
    if match is not None:
        if match["breaking"]:
            return MAJOR
        level = TYPE_LEVELS.get(match["type"].lower(), NONE)
    if any(line.startswith(BREAKING_FOOTERS) for line in body.splitlines()):
        return MAJOR
    return level


def next_release(tag: str | None, messages: list[str], workspace_version: str) -> Release:
    if tag is None:
        if parse_version(workspace_version) is None:
            raise ValueError(f"The workspace version is not MAJOR.MINOR.PATCH: {workspace_version!r}")
        return Release(workspace_version, True)
    current = parse_tag(tag)
    if current is None:
        raise ValueError(f"The newest tag is not vMAJOR.MINOR.PATCH: {tag!r}")
    major, minor, patch = current
    level = max(map(bump_level, messages), default=NONE)
    if level == MAJOR:
        major, minor, patch = major + 1, 0, 0
    elif level == MINOR:
        minor, patch = minor + 1, 0
    elif level == PATCH:
        patch += 1
    return Release(f"{major}.{minor}.{patch}", level != NONE)


def git(repository: Path, *arguments: str) -> str:
    return subprocess.run(
        ["git", *arguments], cwd=repository, check=True, capture_output=True, text=True
    ).stdout


def read_release(repository: Path) -> Release:
    # A shallow clone can hide the tag and report a false first release.
    if git(repository, "rev-parse", "--is-shallow-repository").strip() != "false":
        raise ValueError("The release version needs the full history and tags; fetch with fetch-depth: 0.")
    tag = newest_tag(git(repository, "tag", "--list", "v*").split())
    messages = []
    if tag is not None:
        log = git(repository, "log", "--format=%B%x00", f"{tag}..HEAD")
        messages = [message for message in log.split("\0") if message.strip()]
    with (repository / "Cargo.toml").open("rb") as manifest:
        workspace = tomllib.load(manifest).get("workspace", {}).get("package", {})
    return next_release(tag, messages, str(workspace.get("version", "")))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--github-output", type=Path, help="Append the result to this file instead of printing it.")
    parser.add_argument("--repository", type=Path, default=REPOSITORY, help=argparse.SUPPRESS)
    arguments = parser.parse_args(argv)
    release = read_release(arguments.repository)
    result = f"version={release.version}\nreleasable={'true' if release.releasable else 'false'}\n"
    if arguments.github_output is None:
        print(result, end="")
    else:
        with arguments.github_output.open("a", encoding="utf-8") as output:
            output.write(result)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError, tomllib.TOMLDecodeError) as error:
        detail = error.stderr.strip() if isinstance(error, subprocess.CalledProcessError) else error
        print(f"Release version failed: {detail}", file=sys.stderr)
        sys.exit(1)
