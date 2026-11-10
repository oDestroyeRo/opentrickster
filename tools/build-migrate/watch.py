#!/usr/bin/env python3
"""Decide whether the deployed game build can ship without a person, and prepare the change.

    python3 -I tools/build-migrate/watch.py CACHE [--work DIR] [--github-output FILE]

CACHE is a fresh `migrate.py fetch` directory outside Git. The command prints one outcome and
writes it, with the names that the workflow needs, to --github-output. With --expect-build BUILD,
a fetch that names another build counts as a deploy in progress:

    current  The deployed build is the build that Net pins. Nothing changes.
    proven   Every proof below holds. The pins file, the Rust pins and the sampler pins now hold
             the new build, and a pull request body is ready in --work.
    review   A proof is missing. An issue body is ready in --work and the tree is untouched.

The proofs: every signed method body matches a reviewed fingerprint, no catalog input moved
under Net, the pins that a build does not derive again are unchanged, the config and version
differ from the reviewed files only in the build string, no sampler pin moved or changed, and the
Rust pins hold the new values after the rewrite. See README.md.

Exit status: 0 for an outcome, 2 when the tool fails, 3 when the public files disagree on the build,
as they do while a deploy is in progress. Nothing changes after a 3, and the workflow tries again
later.
"""

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import migrate  # noqa: E402
from buildmigrate import artifacts, derive, repin  # noqa: E402
from buildmigrate.wasm import Malformed, Transient  # noqa: E402

PINNED = re.compile(r'pub const BUILD: &str = "([0-9a-f]{32})";')
ISSUE_LINES = 200


def pinned_build(src):
    """The build that crates/net pins, read from its source."""
    found = PINNED.findall(repin.read_source(Path(src) / "profile.rs"))
    if len(found) != 1:
        raise Malformed("profile.rs does not declare exactly one BUILD")
    return found[0]


def deployed_build(cache):
    """The build that a fetched config and version name. They must agree."""
    names = []
    for name in ("spinel-config.json", "version"):
        path = Path(cache) / name
        if not path.is_file():
            raise Malformed("missing public artifact " + name)
        try:
            names.append(json.loads(path.read_bytes()).get("build"))
        except (ValueError, AttributeError):
            raise Malformed("%s is not a JSON object" % name) from None
    if names[0] != names[1]:
        raise Transient("config and version disagree on the build")
    if not isinstance(names[0], str) or not re.fullmatch(r"[0-9a-f]{32}", names[0]):
        raise Malformed("config and version carry no build id")
    return names[0]


class Proof:
    """What a fetched cache shows against the reviewed pins."""

    def __init__(self, manifest=None, failed=(), matched=0, sampler=None, signed=0):
        self.manifest = manifest  # derive.pins of the cache; None when the cache could not be read
        self.failed = list(failed)  # the check lines that stop a migration
        self.matched = matched  # signed methods that match a reviewed fingerprint
        self.signed = signed  # signed methods in signatures.json
        self.sampler = sampler  # (pins file, new text) when no sampler pin moved or changed


class Artifacts:
    """Proofs read from a fetched cache. Nothing here executes target code."""

    def __init__(self, repo, work):
        self.tools = Path(repo) / "tools" / "build-migrate"
        self.work = Path(work)
        self.build = None

    def prove(self, cache, old):
        try:
            self.build = derive.Build(cache)
            manifest = derive.pins(self.build)
            report = migrate.Report()
            signatures = migrate.load_signatures(self.tools / "signatures.json")
            migrate.check_methods(self.build, signatures, set(), report)
            migrate.check_catalog(manifest, report)
            migrate.compare("pins", derive.invariants(old), derive.invariants(manifest), report)
            descriptors(old, self.build.cache, report)
            sampler, failed = self.sampler(self.build.cache, old["build"])
        except Transient:  # a deploy in progress is not a finding
            raise
        except Exception as error:  # a changed format can fail anywhere in the parsers
            return Proof(failed=["ERROR     %s: %s" % (type(error).__name__, error)])
        matched = sum(1 for line in report.lines if line.startswith("MATCH "))
        return Proof(manifest, report.failed + failed, matched, sampler, len(signatures["methods"]))

    def sampler(self, cache, old_build):
        """Derive the sampler pins of the new build against the pins file that names the pinned build.

        Returns ((that file, the derived text), []) when nothing moved or changed, else (None, lines)."""
        folder = self.tools / "sampler"
        named = [path for path in sorted(folder.glob("*.json"))
                 if json.loads(repin.read_source(path)).get("build") == old_build]
        if len(named) != 1:
            return None, ["SAMPLER   %d sampler pin files name build %s; migrate the sampler by hand first"
                          % (len(named), old_build)]
        out = self.work / "sampler.json"
        out.unlink(missing_ok=True)
        wasm, data = (cache.root / name for name in cache.names[2:])
        done = subprocess.run(
            [sys.executable, "-I", str(folder / "derive.py"), "--build", cache.build, str(wasm), str(data),
             "--expect", str(named[0]), "--out", str(out)],
            capture_output=True, text=True, errors="replace", timeout=1800)
        rows = [row for row in done.stderr.splitlines() if row.startswith(("MOVED ", "CHANGED "))]
        if rows:
            return None, ["SAMPLER   " + row for row in rows]
        if done.returncode != 0 or not out.is_file():
            last = (done.stderr.strip().splitlines() or ["no message"])[-1]
            return None, ["SAMPLER   derive.py exited %d: %s" % (done.returncode, last)]
        return (named[0], repin.read_source(out)), []

    def verify(self, pins, rust):
        """`check NEW --rust` against the files as they are written."""
        return migrate.run_check(self.build, migrate.load_signatures(self.tools / "signatures.json"),
                                 pins=pins, rust=rust)


def descriptors(old, cache, report):
    """The config and version may differ from the reviewed files only in the build string.

    A person confirms this when reading a new config: the first-login endpoint and the data version
    did not move. The reviewed hashes make that mechanical."""
    for name, body in (("spinel-config.json", cache.config), ("version", cache.version)):
        before = body.replace(cache.build.encode(), old["build"].encode())
        if body.count(cache.build.encode()) != 1 or artifacts.sha256(before) != old["deployment"][name]["sha256"]:
            report.add("PIN       deployment.%s: differs from the reviewed file beyond its build string" % name,
                       failed=True)


class Writes:
    """File writes that can be undone."""

    def __init__(self):
        self.before = {}

    def put(self, path, text):
        path = Path(path)
        if path not in self.before:
            self.before[path] = path.read_bytes() if path.exists() else None
        path.write_bytes(text.encode("utf-8"))

    def undo(self):
        for path, data in self.before.items():
            if data is None:
                path.unlink(missing_ok=True)
            else:
                path.write_bytes(data)


class Change:
    """A migration that is written and verified."""

    def __init__(self, files, notes, summary):
        self.files, self.notes, self.summary = files, notes, summary


class Verdict:
    def __init__(self, outcome, build, previous, lines=(), title=None, body=None):
        self.outcome, self.build, self.previous = outcome, build, previous
        self.lines, self.title, self.body = list(lines), title, body


def short(build):
    return build[:8]


def migrate_tree(proof, old, repo, target, prover):
    """Write every file of a proven migration, then verify the files as written.

    Returns (failing lines, change). After a failure the tree is as it was."""
    src = repo / "crates" / "net" / "src"
    new = proof.manifest
    writes = Writes()
    done = False
    try:
        texts = {}
        for name in repin.files(old, new):
            if not (src / name).is_file():
                raise Malformed("missing source file %s" % (src / name))
            texts[name] = repin.read_source(src / name)
        plan = repin.plan(old, new, texts)
        sampler_js = next(site.file for site in derive.follower_sites(old["build"]) if site.key == "sampler")
        if sampler_js not in plan.texts:
            raise Malformed("%s does not hold the pinned build %s; migrate the sampler by hand first"
                            % (sampler_js, old["build"]))
        if proof.sampler is None:
            raise Malformed("no sampler pins to carry to the new build")
        for name, text in plan.texts.items():
            writes.put(src / name, text)
        writes.put(target, json.dumps(new, indent=1, ensure_ascii=False) + "\n")
        sampler_pins, sampler_text = proof.sampler
        writes.put(sampler_pins, sampler_text)
        report = prover.verify(target, src)
        if report.failed:
            return ["REPIN     check failed after the rewrite"] + report.failed, None
        done = True
        files = ["crates/net/src/" + name for name in plan.texts]
        files += [path.relative_to(repo).as_posix() for path in (target, sampler_pins)]
        return [], Change(files, plan.notes, "PASS: 0 failure(s) for build %s" % new["build"])
    except Malformed as error:
        return ["REPIN     %s" % error], None
    finally:
        if not done:
            writes.undo()


def run(cache, repo, work, prover, expect=None):
    """Classify the fetched build and, when it is proven, move the repository to it.

    `expect` is the build that an earlier probe read. Another build in the fetch is a deploy in progress."""
    repo = Path(repo)
    previous = pinned_build(repo / "crates" / "net" / "src")
    deployed = deployed_build(cache)
    if expect is not None and deployed != expect:
        raise Transient("the deployed build moved from %s to %s during the run" % (expect, deployed))
    if deployed == previous:
        return Verdict("current", deployed, previous)
    pins = repo / "tools" / "build-migrate" / "pins"
    if not (pins / (previous + ".json")).is_file():
        raise Malformed("no reviewed pins for the pinned build %s" % previous)
    old = json.loads(repin.read_source(pins / (previous + ".json")))
    target = pins / (deployed + ".json")
    proof = prover.prove(cache, old)
    failed = list(proof.failed)
    if not failed and not proof.matched == proof.signed > 0:
        failed.append("SIGNED    %d of %d signed methods match; every signed method must match and one must exist"
                      % (proof.matched, proof.signed))
    if target.exists():
        failed.append("PIN       %s already exists; a return to a reviewed build needs a person" % target.name)
    change = None
    if not failed:
        failed, change = migrate_tree(proof, old, repo, target, prover)
    if failed:
        return Verdict("review", deployed, previous, failed, "Game build %s needs review" % short(deployed),
                       issue_body(previous, deployed, failed))
    return Verdict("proven", deployed, previous, [], "fix(net): pin game build %s" % short(deployed),
                   pull_request_body(previous, deployed, proof.matched, change))


def fence(lines):
    """A markdown fence that no line of `lines` can close."""
    longest = max([len(ticks) for line in lines for ticks in re.findall(r"`+", line)] + [2])
    tick = "`" * (longest + 1)
    return "%s\n%s\n%s" % (tick, "\n".join(lines), tick)


def issue_body(previous, build, lines):
    shown = lines[:ISSUE_LINES]
    if len(lines) > len(shown):
        shown.append("... and %d more line(s)" % (len(lines) - len(shown)))
    return "\n".join([
        "The official game moved from build `%s` to `%s`. The build watcher could not prove that every protocol "
        "method is unchanged, so it shipped nothing." % (previous, build),
        "",
        "## Findings",
        "",
        fence(shown),
        "",
        "## Next step",
        "",
        "Migrate by hand with the steps in `tools/build-migrate/README.md`. Fetch the build into a new directory "
        "outside Git, run `check`, and run `diff` on each changed method. A change to a wire layout, an opcode or "
        "a wait set that Net implements needs code changes first.",
        "",
        "The watcher opens no second issue while this one is open. It checks again every hour, and it opens a "
        "pull request by itself once the build is proven.",
        "",
    ])


def pull_request_body(previous, build, matched, change):
    cutoff = ("`INVENTORY_CUTOFF_BUILD` carries forward to this build."
              if "crates/net/src/decode.rs" in change.files
              else "`INVENTORY_CUTOFF_BUILD` did not name the pinned build, so it stays as it was.")
    return "\n".join([
        "Pins game build `%s`. It replaces build `%s`." % (build, previous),
        "",
        "Every signed method body matches a reviewed fingerprint, so no wire layout, opcode or wait set that "
        "Net implements changed.",
        "",
        "## Proof",
        "",
        "- Signatures: %d signed methods match." % matched,
        "- The config and version differ from the reviewed files only in the build string. The data version and "
        "the first-login endpoint are unchanged.",
        "- Catalog inputs: no overlay defines a table that Net reads, and the bundle names no `SpinelDataSettings`.",
        "- The pins that a build does not derive again are unchanged: metadata format, compiled-field owners and "
        "lengths, pinned contents and the login failure table.",
        "- Sampler pins: none moved and none changed.",
        "- `migrate.py check --rust crates/net/src` on the written files: `%s`" % change.summary,
        "",
        "## Files",
        "",
    ] + ["- `%s`" % name for name in change.files] + [
        "",
        cutoff,
        "",
        "## Not proven",
        "",
        "- Methods that `signatures.json` does not cover.",
        "- The server catalog files for this data version.",
        "- Server acceptance of the new seed and opcode maps.",
        "",
        "Opened by `.github/workflows/game-build.yml`.",
        "",
    ])


def write_outputs(path, values):
    with open(path, "a", encoding="utf-8") as f:
        for key, value in values.items():
            if "\n" in value or "\r" in value:
                raise Malformed("output %s holds a line break" % key)
            f.write("%s=%s\n" % (key, value))


def main(argv=None, prover=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("cache", help="a fresh `migrate.py fetch` directory outside Git")
    parser.add_argument("--repo", default=str(HERE.parents[1]), help="repository root (default: this checkout)")
    parser.add_argument("--work", help="directory outside Git for the body files (default: a new temporary one)")
    parser.add_argument("--github-output", help="file that receives name=value lines for the workflow")
    parser.add_argument("--expect-build", help="the build that a probe read; a fetch that names another build exits 3")
    args = parser.parse_args(argv)
    try:
        work = Path(args.work) if args.work else Path(tempfile.mkdtemp(prefix="game-build-"))
        if artifacts.inside_git(work):
            raise Malformed("refusing to write bodies inside a Git work tree")
        work.mkdir(parents=True, exist_ok=True)
        verdict = run(args.cache, args.repo, work, prover or Artifacts(args.repo, work), args.expect_build)
        outputs = {"outcome": verdict.outcome, "build": verdict.build, "previous": verdict.previous}
        if verdict.body is not None:
            name = "pr-body.md" if verdict.outcome == "proven" else "issue-body.md"
            (work / name).write_text(verdict.body, encoding="utf-8")
            outputs.update(short=short(verdict.build), title=verdict.title, body_file=str(work / name),
                           branch="auto/game-build-%s" % short(verdict.build))
        for line in verdict.lines:
            print(line, file=sys.stderr)
        if args.github_output:
            write_outputs(args.github_output, outputs)
    except Transient as error:
        print("TRANSIENT: %s" % error, file=sys.stderr)
        return 3
    except Malformed as error:
        print("ERROR: %s" % error, file=sys.stderr)
        return 2
    print(verdict.outcome)
    return 0


if __name__ == "__main__":
    sys.exit(main())
