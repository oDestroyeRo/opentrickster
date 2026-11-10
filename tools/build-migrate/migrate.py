#!/usr/bin/env python3
"""Re-derive and verify OpenTrickster's per-build native pins from public artifacts.

Run with `python3 -I tools/build-migrate/migrate.py <command> ...`. See README.md.
"""

import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from buildmigrate import artifacts, derive, repin  # noqa: E402
from buildmigrate.fingerprint import diff  # noqa: E402
from buildmigrate.wasm import Malformed, check  # noqa: E402

SIGNATURES = HERE / "signatures.json"
PINS = HERE / "pins"
HASH = re.compile(r"\b[0-9a-f]{64}\b")
FUNCTION_AT = re.compile(r"\bf?(\d{3,6})\s*@\s*`?(?:0x)?([0-9a-f]{5,8})\b")


def emit(value):
    print(json.dumps(value, indent=2, ensure_ascii=False))


def cmd_fetch(args):
    emit(artifacts.fetch(args.dir))
    return 0


def cmd_derive(args):
    build = derive.Build(args.cache)
    manifest = derive.pins(build)
    signatures = load_signatures(args.signatures)
    manifest["methods"] = [joined(build, s["name"]) for s in signatures["methods"]]
    emit(manifest)
    return 0


def joined(build, name):
    try:
        return build.join_record(name)
    except Malformed as error:
        return {"name": name, "error": str(error)}


def load_signatures(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


class Report:
    """The lines `check` prints, in order, and how many of them are failures."""

    def __init__(self, echo=False):
        self.lines = []
        self.failed = []
        self.echo = echo

    @property
    def failures(self):
        return len(self.failed)

    def add(self, line, failed=False):
        self.lines.append(line)
        if failed:
            self.failed.append(line)
        if self.echo:
            print(line)


def check_methods(build, signatures, excluded, report):
    fp = build.fingerprinter
    for s in signatures["methods"]:
        if excluded and set(s["sources"]) <= excluded:
            report.add("SKIPPED   %s (cited only by %s)" % (s["name"], ",".join(s["sources"])))
            continue
        try:
            _, (_, function, _, _) = build.function_of(s["name"])
            digest, count = fp.fingerprint(function)
        except Malformed as error:
            report.add("UNMATCHED %s: %s" % (s["name"], error), failed=True)
            continue
        reviewed = [a["build"] for a in s["accepted"] if (a["fingerprint"], a["tokens"]) == (digest, count)]
        if reviewed:
            report.add("MATCH     f%-6d %s (as reviewed for %s)" % (
                function, s["name"], ",".join(b[:8] for b in reviewed)))
        else:
            report.add("CHANGED   f%-6d %s (%d tokens; reviewed %s)" % (
                function, s["name"], count, ",".join("%s:%d" % (a["build"][:8], a["tokens"]) for a in s["accepted"])),
                failed=True)


def check_catalog(manifest, report):
    for name, overlay in manifest["catalog"]["overlays"].items():
        if overlay["relevant"]:
            report.add("CATALOG   overlay %s defines catalog tables %s; Net would miss merged rows"
                       % (name, ",".join(overlay["relevant"])), failed=True)
    if manifest["catalog"]["spineldatasettings"]:
        report.add("CATALOG   the Unity bundle names SpinelDataSettings; the data URL may be overridden",
                   failed=True)


def check_rust(manifest, rust, report):
    sites = derive.rust_sites(manifest)
    flat = {}
    for site in sites:
        if site.file not in flat:
            path = Path(rust) / site.file
            check(path.is_file(), "missing source file %s" % path)
            flat[site.file] = repin.flatten(repin.read_source(path))[0]
        pattern = site.pattern()
        if re.search(pattern, flat[site.file]):
            report.add("RUST      %s: %s" % (site.file, pattern[:90]))
        else:
            report.add("STALE     %s lacks %s" % (site.file, pattern), failed=True)


def run_check(build, signatures, pins=None, rust=None, excluded=(), echo=False):
    """Every check of one cache, as a Report: signatures, reviewed pins, catalog inputs, Rust pins."""
    report = Report(echo)
    check_methods(build, signatures, set(excluded or ()), report)
    manifest = derive.pins(build)
    expected_path = Path(pins) if pins else PINS / (build.cache.build + ".json")
    if not expected_path.is_file():
        report.add("NOPINS    no reviewed pins for build %s at %s" % (build.cache.build, expected_path), failed=True)
    else:
        expected = json.loads(expected_path.read_text(encoding="utf-8"))
        if "definitions" in expected and "definitions" not in manifest:
            report.add("SKIPPED   pins.definitions (the cache carries no catalog JSON; see step 4)")
            del expected["definitions"]
        compare("pins", expected, manifest, report)
    check_catalog(manifest, report)
    if rust:
        check_rust(manifest, rust, report)
    return report


def cmd_check(args):
    build = derive.Build(args.cache)
    report = run_check(build, load_signatures(args.signatures), args.pins, args.rust, args.exclude_source, echo=True)
    print("%s: %d failure(s) for build %s" % ("FAIL" if report.failures else "PASS", report.failures, build.cache.build))
    return 1 if report.failures else 0


def compare(path, expected, actual, report):
    if isinstance(expected, dict) and isinstance(actual, dict):
        for key in expected:
            if key not in actual:
                report.add("PIN       %s.%s missing from derivation" % (path, key), failed=True)
            else:
                compare(path + "." + key, expected[key], actual[key], report)
    elif isinstance(expected, list) and isinstance(actual, list) and len(expected) == len(actual):
        for i, (e, a) in enumerate(zip(expected, actual)):
            compare("%s[%d]" % (path, i), e, a, report)
    elif expected != actual:
        report.add("PIN       %s: expected %s, derived %s" % (path, json.dumps(expected), json.dumps(actual)),
                   failed=True)


def cmd_repin(args):
    """Move the Rust pins of a reviewed build to the values of the build in CACHE."""
    new = derive.pins(derive.Build(args.cache))
    old = json.loads(Path(args.source).read_text(encoding="utf-8"))
    rust = Path(args.rust)
    texts = {}
    for name in repin.files(old, new):
        check((rust / name).is_file(), "missing source file %s" % (rust / name))
        texts[name] = repin.read_source(rust / name)
    result = repin.plan(old, new, texts)
    for name, text in result.texts.items():
        repin.write_source(rust / name, text)
        print("MOVED     %s: %s" % (name, ", ".join(result.moved[name])))
    for note in result.notes:
        print(note)
    print("repinned %d pin(s) in %d file(s): build %s -> %s" % (
        sum(len(keys) for keys in result.moved.values()), len(result.texts), old["build"], new["build"]))
    return 0


def cmd_sign(args):
    """Write signatures for every method body the cited documents pin for this baseline."""
    build = derive.Build(args.cache)
    module = build.module
    imported = len(module.imported_functions)
    by_hash = {}
    for n, (start, size) in enumerate(module.bodies):
        by_hash.setdefault(artifacts.sha256(module.data[start:start + size]), set()).add(n + imported)
    cited = {}
    for path in args.cite:
        text = Path(path).read_text()
        source = Path(path).name
        for h in HASH.findall(text):
            for function in by_hash.get(h, ()):
                cited.setdefault(function, set()).add(source)
        for number, offset in FUNCTION_AT.findall(text):
            function = int(number)
            if imported <= function < imported + len(module.bodies) and \
                    module.body(function)[0] == int(offset, 16):
                cited.setdefault(function, set()).add(source)
    methods = {}
    helpers = []
    for function, sources in sorted(cited.items()):
        names = sorted({build.rt.method_name(i) for i in build.rt.function_methods.get(function, ())})
        if not names:
            helpers.append(function)
            continue
        for name in names:
            if len(build.methods_named(name)) != 1:
                continue
            digest, count = build.fingerprinter.fingerprint(function)
            entry = methods.setdefault(name, {"name": name, "sources": [], "accepted": [
                {"build": build.cache.build, "fingerprint": digest, "tokens": count}]})
            entry["sources"] = sorted(set(entry["sources"]) | sources)
    out = {"baseline": build.cache.build,
           "volatile": list(derive.VOLATILE),
           "methods": sorted(methods.values(), key=lambda m: m["name"])}
    save_signatures(args.out, out)
    print("signed %d methods from %d cited bodies; %d cited runtime helpers have no metadata name: %s"
          % (len(methods), len(cited), len(helpers), ",".join("f%d" % f for f in helpers)))
    return 0


def save_signatures(path, signatures):
    Path(path).write_text(json.dumps(signatures, indent=1, ensure_ascii=False) + "\n")


def cmd_accept(args):
    """Record a reviewed target fingerprint for methods whose change is proven harmless."""
    build = derive.Build(args.cache)
    signatures = load_signatures(args.signatures)
    by_name = {s["name"]: s for s in signatures["methods"]}
    for name in args.names:
        entry = by_name.get(name)
        if entry is None:
            raise Malformed("not a signed method: " + name)
        digest, count = build.fingerprinter.fingerprint(build.function_of(name)[1][1])
        entry["accepted"] = [a for a in entry["accepted"] if a["build"] != build.cache.build]
        entry["accepted"].append({"build": build.cache.build, "fingerprint": digest, "tokens": count,
                                  "review": args.review})
        print("accepted %s for %s" % (build.cache.build, name))
    save_signatures(args.signatures, signatures)
    return 0


def cmd_definitions(args):
    """Carry reviewed catalog JSON to a new build whose catalog inputs are all unchanged.

    The JSON is a pure function of the build string, the server catalog for `dataVersion`, and
    the compiled key, effects and gun slots; overlays only gate it. Anything else stops."""
    old, new = derive.Build(args.baseline), derive.Build(args.target)
    a, b = derive.pins(old), derive.pins(new)
    check("definitions" in a, "the baseline cache has no catalog JSON")
    expected = json.loads((PINS / (old.cache.build + ".json")).read_text()).get("definitions")
    check(a["definitions"] == expected, "baseline catalog JSON differs from its reviewed pins")
    check(a["dataVersion"] == b["dataVersion"], "dataVersion changed: re-resolve the catalog")
    for name in ("catalog_key", "normal_form_effects", "gun_slots"):
        check(a["compiled"][name]["sha256"] == b["compiled"][name]["sha256"], name + " changed")
    check(not b["catalog"]["spineldatasettings"], "SpinelDataSettings needs inspection")
    for name, overlay in b["catalog"]["overlays"].items():
        check(not overlay["relevant"], "overlay %s defines catalog tables" % name)
    out = Path(args.out)
    check(not artifacts.inside_git(out), "refusing to write catalog JSON inside a Git work tree")
    out.mkdir(parents=True, exist_ok=True)
    source = Path(args.baseline)
    defs = (source / "game_definitions.json").read_bytes()
    prefix = ('{"build":"%s",' % old.cache.build).encode()
    check(defs.startswith(prefix) and defs.count(old.cache.build.encode()) == 1,
          "unexpected game_definitions.json layout")
    carried = ('{"build":"%s",' % new.cache.build).encode() + defs[len(prefix):]
    (out / "game_definitions.json").write_bytes(carried)
    (out / "items.json").write_bytes((source / "items.json").read_bytes())
    emit({n: artifacts.sha256((out / n).read_bytes()) for n in derive.DEFINITIONS})
    return 0


# Compiler-generated ordinals that shift when members are added to a type.
ORDINALS = ((re.compile(r"(<>c__DisplayClass)\d+(_\d+)"), r"\1#\2"),
            (re.compile(r">b__\d+_(\d+)"), r">b__#_\1"),
            (re.compile(r">d__\d+"), r">d__#"))


def loose(text):
    for pattern, replacement in ORDINALS:
        text = pattern.sub(replacement, text)
    return text


def image_bodies(build, image):
    """{ordinal-masked name: [normalized texts]} for every joined method of one image."""
    out = {}
    for index, m in enumerate(build.meta.methods):
        joined = build.rt.method_function.get(index)
        if joined is None or build.meta.types[m["declaring"]].get("image") != image:
            continue
        locals_, tokens = build.fingerprinter.normalized(joined[1])
        body = [build.module.function_signature(joined[1]), repr(locals_)] + [loose(t) for _, t in tokens]
        out.setdefault(loose(build.rt.method_name(index)), []).append(body)
    return out


def cmd_compare(args):
    """Compare every method of one image, pairing renumbered compiler-generated names."""
    old, new = derive.Build(args.baseline), derive.Build(args.target)
    a, b = image_bodies(old, args.image), image_bodies(new, args.image)
    counts = {"same": 0, "changed": 0, "baseline-only": 0, "target-only": 0, "ambiguous": 0}
    for name in sorted(set(a) | set(b)):
        if name not in b or name not in a:
            kind = "baseline-only" if name in a else "target-only"
        elif len(a[name]) != 1 or len(b[name]) != 1:
            kind = "ambiguous"
        else:
            kind = "same" if a[name] == b[name] else "changed"
        counts[kind] += 1
        if kind != "same":
            print("%-13s %s" % (kind.upper(), name))
    print("%s: %s" % (args.image, ", ".join("%d %s" % (n, k) for k, n in counts.items())))
    return 0


def cmd_diff(args):
    """Normalized instruction diff of named methods between two caches, for review."""
    old, new = derive.Build(args.baseline), derive.Build(args.target)
    for name in args.names:
        before, _, after = name.partition("=>")
        a = old.fingerprinter.normalized(old.function_of(before)[1][1])[1]
        b = new.fingerprinter.normalized(new.function_of(after or before)[1][1])[1]
        lines = diff(a, b)
        print("%s %s" % ("SAME" if not lines else "DIFF", name))
        for line in lines:
            print("    " + line)
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("fetch", help="download config, version, page and Build assets")
    p.add_argument("dir")
    p.set_defaults(run=cmd_fetch)
    p = sub.add_parser("derive", help="print the sanitized per-build manifest")
    p.add_argument("cache")
    p.add_argument("--signatures", default=SIGNATURES)
    p.set_defaults(run=cmd_derive)
    p = sub.add_parser("check", help="verify fingerprints and reviewed pins; nonzero on drift")
    p.add_argument("cache")
    p.add_argument("--signatures", default=SIGNATURES)
    p.add_argument("--pins", help="reviewed pins file (default: pins/<build>.json)")
    p.add_argument("--rust", help="crates/net/src: also require the Rust pins to match")
    p.add_argument("--exclude-source", action="append",
                   help="skip methods cited only by this document (repeatable)")
    p.set_defaults(run=cmd_check)
    p = sub.add_parser("repin", help="rewrite the Rust pins of a reviewed build to the build in CACHE")
    p.add_argument("cache")
    p.add_argument("--from", dest="source", required=True, help="reviewed pins file of the build Rust holds now")
    p.add_argument("--rust", required=True, help="crates/net/src")
    p.set_defaults(run=cmd_repin)
    p = sub.add_parser("sign", help="write signatures for methods cited by documents")
    p.add_argument("cache")
    p.add_argument("--cite", nargs="+", required=True)
    p.add_argument("--out", default=SIGNATURES)
    p.set_defaults(run=cmd_sign)
    p = sub.add_parser("accept", help="record reviewed fingerprints of changed methods")
    p.add_argument("cache")
    p.add_argument("names", nargs="+")
    p.add_argument("--review", required=True, help="why the change keeps Net's contract")
    p.add_argument("--signatures", default=SIGNATURES)
    p.set_defaults(run=cmd_accept)
    p = sub.add_parser("definitions", help="carry catalog JSON forward when its inputs are unchanged")
    p.add_argument("baseline")
    p.add_argument("target")
    p.add_argument("out")
    p.set_defaults(run=cmd_definitions)
    p = sub.add_parser("compare", help="compare every method of one image between two caches")
    p.add_argument("baseline")
    p.add_argument("target")
    p.add_argument("--image", default="Spinel.Net.dll")
    p.set_defaults(run=cmd_compare)
    p = sub.add_parser("diff", help="normalized body diff between two caches")
    p.add_argument("baseline")
    p.add_argument("target")
    p.add_argument("names", nargs="+", help="canonical name, or OLD=>NEW for a renamed method")
    p.set_defaults(run=cmd_diff)
    args = parser.parse_args(argv)
    try:
        return args.run(args)
    except Malformed as error:
        print("ERROR: %s" % error, file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
