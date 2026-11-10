"""Synthetic checks for the build-migration tool. No public artifacts or network are needed:
`python3 -I -m unittest discover -s tools/build-migrate/tests`."""

import base64
import contextlib
import copy
import gzip
import hashlib
import io
import json
import re
import shutil
import struct
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parents[1]
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE))

import migrate  # noqa: E402
import watch  # noqa: E402
from buildmigrate import artifacts, derive, fingerprint, repin, unityfs  # noqa: E402
from buildmigrate.wasm import Cursor, Malformed, Transient  # noqa: E402


class Leb128(unittest.TestCase):
    def test_unsigned_and_signed(self):
        c = Cursor(bytes([0xE5, 0x8E, 0x26, 0x7F, 0x80, 0x7F]))
        self.assertEqual(c.uleb(), 624485)
        self.assertEqual(c.sleb(32), -1)
        self.assertEqual(c.sleb(32), -128)

    def test_truncated_fails(self):
        with self.assertRaises(Malformed):
            Cursor(bytes([0x80])).uleb()


class PublicArtifacts(unittest.TestCase):
    def test_gzip_is_decided_by_magic_not_name(self):
        self.assertEqual(artifacts.decoded(gzip.compress(b"payload")), (b"payload", "gzip"))
        self.assertEqual(artifacts.decoded(b"payload"), (b"payload", "identity"))
        with self.assertRaises(Malformed):
            artifacts.decoded(gzip.compress(b"payload")[:-6])

    def directory(self, records, end=None):
        body = b"".join(struct.pack("<III", o, n, len(name)) + name for o, n, name in records)
        end = 20 + len(body) if end is None else end
        return b"UnityWebData1.0\0" + struct.pack("<I", end) + body

    def test_unity_directory_bounds(self):
        head = self.directory([(0, 0, b"a")])
        data = self.directory([(len(head), 3, b"a")]) + b"xyz"
        self.assertEqual(artifacts.unity_directory(data), [{"name": "a", "offset": len(head), "length": 3}])
        with self.assertRaises(Malformed):
            artifacts.unity_directory(self.directory([(0, 3, b"a")]) + b"xyz")
        with self.assertRaises(Malformed):
            artifacts.unity_directory(self.directory([], end=19))
        with self.assertRaises(Malformed):
            artifacts.unity_directory(data[:-1])

    def test_page_must_name_each_asset_role_once(self):
        names = ["%032x.%s" % (i, k) for i, k in enumerate(("loader.js", "framework.js.gz", "wasm.gz", "data.gz"))]
        page = " ".join("Build/" + n for n in names).encode()
        self.assertEqual(artifacts.asset_names(page), names)
        with self.assertRaises(Malformed):
            artifacts.asset_names(page + b" Build/%032x.wasm.gz" % 9)

    def test_page_pin_ignores_the_cdn_analytics_beacon(self):
        page = b"<body>\n    </script>\n  </body>\n</html>\n"
        beacon = (b'<script type="module" src="https://static.cloudflareinsights.com/beacon.min.js/v1"'
                  b' integrity="sha512-x" data-cf-beacon=\'{"token": "t"}\' crossorigin="anonymous"></script>\n')
        injected = page.replace(b"  </body>", b"  " + beacon + b"</body>")
        self.assertEqual(artifacts.public_page(injected), page)
        self.assertEqual(artifacts.public_page(page), page)
        with self.assertRaises(Malformed):
            artifacts.public_page(injected.replace(b"</html>", beacon + b"</html>"))


class OpcodeMap(unittest.TestCase):
    def test_synthetic_seed_fixture(self):
        seed = bytes(range(32))
        wire, inverse = derive.opcode_map(seed, b"c2s")
        self.assertEqual(derive.table_hash(wire),
                         "c87a5dc1ad4dc4657d8ad4ababf6bb39e83daddb12cf61c76e50e89f65e6edab")
        self.assertEqual(wire[0x1234], 0x3945)
        self.assertTrue(all(inverse[w] == i for i, w in enumerate(wire)))
        self.assertEqual(derive.table_hash(derive.opcode_map(seed, b"s2c")[0]),
                         "20a1907c6cadf8a2cf4ac1c36f5dbf36b8a086d08958eec0071ffec8a0663a8a")


class Unity(unittest.TestCase):
    def test_lz4_literals_and_overlapping_match(self):
        block = bytes([0x35]) + b"abc" + bytes([3, 0, 0x10]) + b"x"
        self.assertEqual(unityfs.lz4_block(block, 13), b"abcabcabcabcx")
        with self.assertRaises(Malformed):
            unityfs.lz4_block(bytes([0x35]) + b"abc" + bytes([9, 0, 0x10]) + b"x", 13)

    def test_overlay_tables_and_relevance(self):
        text = b'<INI><TABLE name="SpinelCustomUIParams"><R/></TABLE>\n<TABLE  name=\'MapInfoEx\'></TABLE></INI>'
        tables = unityfs.tables(text)
        self.assertEqual(sorted(tables), ["MapInfoEx", "SpinelCustomUIParams"])
        self.assertEqual(unityfs.RELEVANT_TABLES & set(tables), {"MapInfoEx"})
        with self.assertRaises(Malformed):
            unityfs.tables(b"<TABLE><TABLE name='a'>")

    def test_text_asset_needs_aligned_name_and_length(self):
        script = b"<INI/>"
        blob = struct.pack("<I", 13) + b"libconfig_eng" + b"\0" * 3 + struct.pack("<I", len(script)) + script
        self.assertEqual(unityfs.text_assets(blob, "libconfig_eng"), [script])
        self.assertEqual(unityfs.text_assets(b"\0" + blob, "libconfig_eng"), [])


class Fingerprint(unittest.TestCase):
    def test_initialisation_runs_are_order_independent(self):
        init = "call helper abc"
        tail = [(5, "i32.const 1"), (6, "i32.store8 0 0")]
        a = [(1, "i32.const usage type a"), (2, init), (3, "i32.const usage literal b"), (4, init)] + tail
        b = [(1, "i32.const usage literal b"), (2, init), (3, "i32.const usage type a"), (4, init)] + tail
        out = fingerprint.canonical_initialisation(a, {init})
        self.assertEqual(out, fingerprint.canonical_initialisation(b, {init}))
        self.assertEqual(out, b)
        self.assertEqual(fingerprint.canonical_initialisation(a, set()), a)

    def test_digest_helpers_are_stable(self):
        self.assertEqual(hashlib.sha256(b"c2s").hexdigest(), derive.COMPILED[2][4])


# ---------------------------------------------------------------------------------------------
# Pin sites, repin and the watcher. Everything below runs on invented manifests and sources.
# ---------------------------------------------------------------------------------------------

ROLES = (".loader.js", ".framework.js.gz", ".wasm.gz", ".data.gz")
COMPILED = (("build", 32), ("seed", 44), ("c2s", 3), ("s2c", 3), ("marker", 5), ("client_version", 4),
            ("firstlogin_wait", 12), ("selection_wait", 12), ("md5_rotations", 256), ("catalog_key", 16),
            ("normal_form_effects", 24), ("gun_slots", 16))


def manifest(tag, definitions=True):
    """An invented manifest in the shape of `derive.pins`. Every per-build value depends on `tag`."""
    def h(label):
        return hashlib.sha256((tag + label).encode()).hexdigest()
    wide = 1 + 6 * (tag == "b")
    compiled = {name: {"method": "Owner::" + name, "cell": "0x%x" % (0x300000 + i), "target": "literal %d" % i,
                       "offset": "0x%x" % (0x1000 * (i + 1) * wide + len(tag)), "length": length,
                       "sha256": h(name) if name in derive.VOLATILE else hashlib.sha256(name.encode()).hexdigest()}
                for i, (name, length) in enumerate(COMPILED)}
    build = h("build")[:32]
    compiled["build"]["value"] = build
    compiled["seed"]["value"] = base64.b64encode(hashlib.sha256((tag + "seed").encode()).digest()).decode()
    out = {
        "build": build,
        "deployment": {"spinel-config.json": {"bytes": 120, "sha256": h("config")},
                       "version": {"bytes": 44, "sha256": h("version")},
                       "index.public.html": {"bytes": 999, "sha256": h("page")}},
        "dataVersion": "version-1",
        "assets": [{"name": h("asset%d" % i)[:32] + role, "bytes": 1000 * (i + 1) * wide + 7, "sha256": h("a%d" % i)}
                   for i, role in enumerate(ROLES)],
        "metadata": {"offset": 5000 * wide, "bytes": 9348436 if tag != "b" else 12345678, "sha256": h("metadata"),
                     "magic": "af1bb1fa6c000000"},
        "compiled": compiled,
        "opcode_maps": {k: h(k) for k in ("c2s_wire", "c2s_inverse", "s2c_wire", "s2c_inverse")},
        "catalog": {"overlays": {"libconfig_thai": {"bytes": 1000 * wide, "sha256": h("overlay"), "tables": 127,
                                                    "relevant": []}}, "spineldatasettings": False},
        "tables": {"login_failure": {"address": 4000 * wide, "value": [0, 6, 6, 2, 1]}},
    }
    if definitions:
        out["definitions"] = {"game_definitions.json": h("defs"), "items.json": h("items")}
    return out


def sources(p):
    """Rust and JavaScript text in which every pin of manifest `p` appears once, laid out as rustfmt does."""
    c = p["compiled"]
    def call(name):
        f = c[name]
        return '    let %s = field(\n        metadata,\n        %d,\n        %d,\n        "%s",\n    )?;\n' % (
            name, int(f["offset"], 16), f["length"], f["sha256"])
    profile = ['pub const BUILD: &str = "%s";\n' % p["build"],
               "const MAX_METADATA: usize = %s;\n" % format(p["metadata"]["bytes"], "_"),
               "const ASSETS: [Asset; 4] = [\n"]
    for a in p["assets"]:
        profile.append('    Asset {\n        name: "%s",\n        size: %d,\n        hash: "%s",\n    },\n'
                       % (a["name"], a["bytes"], a["sha256"]))
    profile.append("];\nfn load() {\n    verify_hash(\n        metadata,\n        \"%s\",\n    )?;\n"
                   % p["metadata"]["sha256"])
    profile.extend(call(n) for n in ("marker", "client_version", "build", "seed"))
    profile.append('}\nfn deployment() {\n    verify_hash(\n        config,\n        "%s",\n    )?;\n'
                   '    verify_hash(\n        version,\n        "%s",\n    )?;\n}\n'
                   % (p["deployment"]["spinel-config.json"]["sha256"], p["deployment"]["version"]["sha256"]))
    catalog = []
    for name in ("catalog_key", "normal_form_effects", "gun_slots"):
        f = c[name]
        start = int(f["offset"], 16)
        catalog.append('    let %s = metadata\n        .get(0x%x..0x%x)\n        .ok_or(NetError::BuildChanged)?;\n'
                       '    verify_hash(\n        %s,\n        "%s",\n    )?;\n'
                       % (name, start, start + f["length"], name, f["sha256"]))
    out = {
        "profile.rs": "".join(profile),
        "codec.rs": 'const SEED: &str = "%s";\nconst MAPS: [&str; 4] = [\n%s];\n' % (
            c["seed"]["value"], "".join('    "%s",\n' % d for d in p["opcode_maps"].values())),
        "catalog.rs": "fn load() {\n" + "".join(catalog) + "}\n",
        "decode.rs": '/// Doc.\nconst INVENTORY_CUTOFF_BUILD: Option<&str> = Some("%s");\n' % p["build"],
        "sampler.js": '{\n  const samplerBuild = "%s";\n}\n' % p["build"],
    }
    if "definitions" in p:
        out["definitions.rs"] = "".join('    verify_hash(&x, "%s")?;\n' % d for d in p["definitions"].values())
    return out


def snapshot(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(Path(root).rglob("*")) if p.is_file()}


class Sites(unittest.TestCase):
    def test_a_site_is_one_regex_that_reads_and_groups_its_values(self):
        site = derive.Site("k", "f.rs", r'x = "{name}";{size}', {"name": "a.b", "size": 12})
        self.assertEqual(site.pattern(), r'x = "a\.b";12')
        found = site.grouped().search('let x = "a.b";12 // x = "axb";12')
        self.assertEqual((found.span("name"), found.span("size")), ((9, 12), (14, 16)))
        with self.assertRaises(ValueError):
            derive.Site("k", "f.rs", "{a}", {"b": 1})
        with self.assertRaises(ValueError):
            derive.Site("k", "f.rs", "{a}{a}", {"a": 1})

    def test_every_pin_has_one_site_and_the_catalog_pins_are_optional(self):
        keys = [s.key for s in derive.rust_sites(manifest("a"))]
        self.assertEqual(len(keys), len(set(keys)))
        self.assertEqual({s.key for s in derive.rust_sites(manifest("a")) if s.optional},
                         {"definitions.game_definitions.json", "definitions.items.json"})
        self.assertEqual(len(derive.rust_sites(manifest("a", definitions=False))), len(keys) - 2)

    def test_check_reports_each_site_against_the_source(self):
        a, b = manifest("a"), manifest("b")
        with tempfile.TemporaryDirectory() as tmp:
            for name, text in sources(a).items():
                (Path(tmp) / name).write_text(text)
            held, stale = migrate.Report(), migrate.Report()
            migrate.check_rust(a, tmp, held)
            migrate.check_rust(b, tmp, stale)
        self.assertEqual((held.failures, len(held.lines)), (0, len(derive.rust_sites(a))))
        self.assertTrue(held.lines[0].startswith('RUST      profile.rs: pub const BUILD: &str = "%s";' % a["build"]))
        self.assertEqual((stale.failures, len(stale.lines)), (len(derive.rust_sites(b)),) * 2)
        self.assertTrue(all(line.startswith("STALE     ") for line in stale.lines))

    def test_digit_separators_do_not_hide_a_value(self):
        text = "const MAX_METADATA: usize = 9_348_436;\n"
        site = derive.Site("k", "f.rs", r"const MAX_METADATA: usize = {n};", {"n": 9348436})
        flat, where = repin.flatten(text)
        self.assertEqual(flat, "const MAX_METADATA: usize = 9348436;\n")
        self.assertTrue(re.search(site.pattern(), flat))
        self.assertEqual(text[where[flat.index("9")]:where[flat.index(";") - 1] + 1], "9_348_436")
        self.assertEqual(repin.show(12345678, "9_348_436"), "12_345_678")
        self.assertEqual(repin.show(999, "9_348_436"), "999")
        self.assertEqual(repin.show(12345678, "9348436"), "12345678")
        self.assertEqual(repin.show("ab12", "9_348_436"), "ab12")


class Repin(unittest.TestCase):
    def test_old_to_new_rewrites_every_site_and_back_restores_the_text(self):
        a, b = manifest("a"), manifest("b")
        there = repin.plan(a, b, sources(a))
        self.assertEqual(there.texts, sources(b))
        self.assertEqual(sum(len(keys) for keys in there.moved.values()), len(derive.rust_sites(a)) + 2)
        self.assertEqual(repin.plan(b, a, sources(b)).texts, sources(a))

    def test_old_to_old_changes_nothing(self):
        a = manifest("a")
        same = repin.plan(a, a, sources(a))
        self.assertEqual((same.texts, same.moved, same.notes), ({}, {}, []))

    def test_a_missing_site_fails_and_names_it(self):
        a, b = manifest("a"), manifest("b")
        text = sources(a)
        text["codec.rs"] = text["codec.rs"].replace("const SEED", "const SEED_")
        with self.assertRaisesRegex(Malformed, r"codec\.rs holds 0 matches of the old seed pin"):
            repin.plan(a, b, text)

    def test_a_duplicated_site_fails_and_names_it(self):
        a, b = manifest("a"), manifest("b")
        text = sources(a)
        text["codec.rs"] += 'const SEED: &str = "%s";\n' % a["compiled"]["seed"]["value"]
        with self.assertRaisesRegex(Malformed, r"codec\.rs holds 2 matches of the old seed pin"):
            repin.plan(a, b, text)

    def test_a_pin_missing_from_one_manifest_fails_unless_it_is_optional(self):
        a, b = manifest("a"), manifest("b")
        del b["opcode_maps"]["c2s_wire"]
        with self.assertRaisesRegex(Malformed, "opcode.c2s_wire"):
            repin.plan(a, b, sources(a))
        del a["opcode_maps"]["c2s_wire"]
        a["opcode_maps"]["extra"] = "0" * 64
        with self.assertRaisesRegex(Malformed, "opcode.extra"):
            repin.plan(a, b, {**sources(manifest("a")), "codec.rs": "x"})

    def test_catalog_hashes_stay_when_the_new_cache_has_no_catalog_json(self):
        a, b = manifest("a"), manifest("b", definitions=False)
        done = repin.plan(a, b, {k: v for k, v in sources(a).items()})
        self.assertNotIn("definitions.rs", done.texts)
        self.assertEqual(sum("definitions" in note for note in done.notes), 2)
        self.assertEqual(done.texts["profile.rs"], sources(b)["profile.rs"])
        self.assertEqual(repin.files(a, b), ["catalog.rs", "codec.rs", "decode.rs", "profile.rs", "sampler.js"])
        self.assertEqual(repin.files(a, manifest("b")), sorted(sources(a)))

    def test_a_build_string_that_only_follows_the_build_stays_when_it_holds_another(self):
        a, b = manifest("a"), manifest("b")
        text = sources(a)
        text["decode.rs"] = text["decode.rs"].replace("Some(", "Some(\"stale\", ").replace(', "stale", ', "")
        text["decode.rs"] = 'const INVENTORY_CUTOFF_BUILD: Option<&str> = None;\n'
        done = repin.plan(a, b, text)
        self.assertNotIn("decode.rs", done.texts)
        self.assertIn("KEPT      decode.rs: inventory_cutoff holds another build", done.notes)
        self.assertEqual(done.texts["sampler.js"], sources(b)["sampler.js"])
        text["sampler.js"] += 'const samplerBuild = "%s";\n' % a["build"]
        with self.assertRaisesRegex(Malformed, "sampler.js holds 2 matches"):
            repin.plan(a, b, text)

    def test_layout_is_preserved_around_the_values(self):
        a, b = manifest("a"), manifest("b")
        text = sources(a)
        text["profile.rs"] = text["profile.rs"].replace("        metadata,\n        %d,\n        5,\n" % int(
            a["compiled"]["marker"]["offset"], 16), "        metadata, %d, 5,\n" % int(a["compiled"]["marker"]["offset"], 16))
        self.assertIn("metadata, %d, 5," % int(a["compiled"]["marker"]["offset"], 16), text["profile.rs"])
        done = repin.plan(a, b, text)
        self.assertIn("metadata, %d, 5," % int(b["compiled"]["marker"]["offset"], 16), done.texts["profile.rs"])
        self.assertEqual(len(done.texts["profile.rs"].splitlines()), len(text["profile.rs"].splitlines()))

    def test_edits_that_overlap_are_refused(self):
        self.assertEqual(repin.splice("f", "abcdef", [(0, 2, "X", "one"), (3, 5, "Y", "two")]), "XcYf")
        with self.assertRaisesRegex(Malformed, "one and two pins overlap"):
            repin.splice("f", "abcdef", [(0, 3, "X", "one"), (2, 5, "Y", "two")])

    def test_a_new_value_that_equals_another_old_value_is_not_replaced_twice(self):
        a, b = manifest("a"), manifest("b")
        b["opcode_maps"]["c2s_wire"] = a["opcode_maps"]["c2s_inverse"]
        done = repin.plan(a, b, sources(a))
        codec = done.texts["codec.rs"]
        self.assertEqual(codec.count(a["opcode_maps"]["c2s_inverse"]), 1 + (b["opcode_maps"]["c2s_inverse"] == a[
            "opcode_maps"]["c2s_inverse"]))
        self.assertIn(b["opcode_maps"]["c2s_inverse"], codec)


class RealSources(unittest.TestCase):
    """The committed pins and the committed Rust must stay one description of the same build."""

    @classmethod
    def setUpClass(cls):
        cls.src = REPO / "crates" / "net" / "src"
        cls.build = watch.pinned_build(cls.src)
        cls.pins = json.loads((HERE / "pins" / (cls.build + ".json")).read_text())

    def test_every_committed_pin_appears_once_in_the_rust_sources(self):
        texts = {n: repin.read_source(self.src / n) for n in repin.files(self.pins, self.pins)}
        self.assertEqual(self.pins["build"], self.build)
        for site in derive.rust_sites(self.pins):
            flat = repin.flatten(texts[site.file])[0]
            self.assertEqual(len(re.findall(site.pattern(), flat)), 1, site.key)
        for site in derive.follower_sites(self.build):
            self.assertEqual(len(re.findall(site.pattern(), repin.flatten(texts[site.file])[0])), 1, site.key)

    def test_repin_to_altered_values_and_back_is_the_identity_on_the_real_sources(self):
        altered = copy.deepcopy(self.pins)
        digest = lambda label: hashlib.sha256(("altered" + label).encode()).hexdigest()  # noqa: E731
        altered["build"] = digest("build")[:32]
        altered["compiled"]["build"]["value"] = altered["build"]
        altered["compiled"]["seed"]["value"] = base64.b64encode(bytes.fromhex(digest("seed"))).decode()
        for name, record in altered["compiled"].items():
            record["sha256"] = digest(name)
            record["offset"] = "0x%x" % (int(record["offset"], 16) * 3 + 1)
        for key in ("spinel-config.json", "version"):
            altered["deployment"][key]["sha256"] = digest(key)
        for i, asset in enumerate(altered["assets"]):
            asset.update(name=digest("asset%d" % i)[:32] + ROLES[i], bytes=asset["bytes"] * 11 + 3, sha256=digest(str(i)))
        altered["metadata"].update(bytes=self.pins["metadata"]["bytes"] * 3, sha256=digest("metadata"))
        for key in altered["opcode_maps"]:
            altered["opcode_maps"][key] = digest(key)
        for key in altered.get("definitions", {}):
            altered["definitions"][key] = digest(key)
        original = {n: repin.read_source(self.src / n) for n in repin.files(self.pins, altered)}
        there = repin.plan(self.pins, altered, original)
        self.assertEqual(sorted(there.texts), sorted(original))
        with tempfile.TemporaryDirectory() as tmp:
            for name, text in {**original, **there.texts}.items():
                repin.write_source(Path(tmp) / name, text)
            held, stale = migrate.Report(), migrate.Report()
            migrate.check_rust(altered, tmp, held)
            migrate.check_rust(self.pins, tmp, stale)
        self.assertEqual(held.failures, 0)
        self.assertEqual(stale.failures, len(derive.rust_sites(self.pins)))
        back = repin.plan(altered, self.pins, {**original, **there.texts})
        self.assertEqual({n: back.texts[n] for n in back.texts}, original)


class Invariants(unittest.TestCase):
    def test_what_every_build_derives_again_is_not_compared(self):
        self.assertEqual(derive.invariants(manifest("a")), derive.invariants(manifest("b")))
        self.assertNotIn("definitions", derive.invariants(manifest("a")))
        self.assertEqual(derive.invariants(manifest("a"))["catalog"]["overlays"], {"libconfig_thai": {"relevant": []}})

    def test_whatever_a_client_cannot_change_is_compared(self):
        base = derive.invariants(manifest("a"))
        mutations = (lambda p: p.update(dataVersion="version-2"),
                     lambda p: p["tables"]["login_failure"]["value"].__setitem__(1, 7),
                     lambda p: p["compiled"]["c2s"].update(value="x2c"),
                     lambda p: p["compiled"]["md5_rotations"].update(sha256="0" * 64),
                     lambda p: p["compiled"]["gun_slots"].update(length=12),
                     lambda p: p["compiled"]["marker"].update(method="Other::MoveNext()"),
                     lambda p: p["metadata"].update(magic="af1bb1fa6d000000"),
                     lambda p: p["catalog"]["overlays"]["libconfig_thai"].update(relevant=["MapInfoEx"]),
                     lambda p: p["catalog"].update(spineldatasettings=True))
        for mutate in mutations:
            changed = manifest("a")
            mutate(changed)
            self.assertNotEqual(derive.invariants(changed), base)

    def test_the_committed_pins_are_stable_across_the_per_build_values(self):
        real = json.loads((HERE / "pins" / "ea1a68bd89958fae94179f1e0835ffb5.json").read_text())
        mutated = copy.deepcopy(real)
        mutated["build"] = "0" * 32
        mutated["assets"][0]["sha256"] = "1" * 64
        mutated["compiled"]["seed"]["value"] = "x"
        mutated["compiled"]["client_version"]["offset"] = "0x1"
        mutated["tables"]["login_failure"]["address"] = 1
        self.assertEqual(derive.invariants(mutated), derive.invariants(real))
        self.assertEqual(real["tables"]["login_failure"]["value"], derive.invariants(real)["tables"]["login_failure"]["value"])

    def test_compare_reports_each_difference_as_a_pin_line(self):
        report = migrate.Report()
        migrate.compare("pins", {"a": 1, "b": {"c": [1, 2]}, "gone": 0}, {"a": 2, "b": {"c": [1, 3]}}, report)
        self.assertEqual(report.lines, [
            "PIN       pins.a: expected 1, derived 2",
            "PIN       pins.b.c[1]: expected 2, derived 3",
            "PIN       pins.gone missing from derivation"])
        self.assertEqual(report.failures, 3)


class FakeCache:
    def __init__(self, build, config, version):
        self.build, self.config, self.version = build, config, version


class Descriptors(unittest.TestCase):
    CONFIG = b'{"firstLogin":{"host":"127.0.0.1","port":9},"dataVersion":"v","build":"%s"}'
    VERSION = b'{"build":"%s"}'

    def reviewed(self, build):
        return {"build": build, "deployment": {
            "spinel-config.json": {"sha256": artifacts.sha256(self.CONFIG % build.encode())},
            "version": {"sha256": artifacts.sha256(self.VERSION % build.encode())}}}

    def lines(self, old, new, config=None, version=None):
        report = migrate.Report()
        watch.descriptors(self.reviewed(old), FakeCache(new, config or self.CONFIG % new.encode(),
                                                         version or self.VERSION % new.encode()), report)
        return report.failed

    def test_only_the_build_string_may_differ(self):
        self.assertEqual(self.lines("a" * 32, "b" * 32), [])

    def test_any_other_difference_is_a_pin_failure(self):
        moved = (self.CONFIG % b"b" * 32).replace(b"127.0.0.1", b"127.0.0.2")
        self.assertEqual(self.lines("a" * 32, "b" * 32, config=moved),
                         ["PIN       deployment.spinel-config.json: differs from the reviewed file beyond its build string"])
        self.assertEqual(len(self.lines("a" * 32, "b" * 32, version=b'{"build":"%s","x":1}' % (b"b" * 32))), 1)
        self.assertEqual(len(self.lines("a" * 32, "b" * 32, version=b'{"build":"%s","old":"%s"}' % ((b"b" * 32,) * 2))), 1)


class FakeProver:
    """Stands in for `watch.Artifacts`: the proof is chosen by the test, the files are real."""

    def __init__(self, repo, new, failed=(), sampler=True, after=(), matched=400, signed=400):
        self.repo, self.new, self.failed, self.after = repo, new, list(failed), list(after)
        self.matched, self.signed = matched, signed
        self.sampler = None
        if sampler:
            path = repo / "tools" / "build-migrate" / "sampler" / "pins.json"
            self.sampler = (path, json.dumps({"build": new["build"], "artifacts": {}}, indent=1, sort_keys=True) + "\n")
        self.verified = []

    def prove(self, cache, old):
        return watch.Proof(self.new, self.failed, self.matched, self.sampler, self.signed)

    def verify(self, pins, rust):
        self.verified.append((Path(pins), Path(rust)))
        report = migrate.Report()
        report.add("RUST      profile.rs: written")
        for line in self.after:
            report.add(line, failed=True)
        return report


class Watcher(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.old, self.new = manifest("a"), manifest("b")
        self.repo, self.cache, self.work = self.tmp / "repo", self.tmp / "cache", self.tmp / "work"
        for name, text in sources(self.old).items():
            self.write(self.repo / "crates" / "net" / "src" / name, text)
        tools = self.repo / "tools" / "build-migrate"
        self.write(tools / "pins" / (self.old["build"] + ".json"), json.dumps(self.old, indent=1) + "\n")
        self.write(tools / "sampler" / "pins.json", json.dumps({"build": self.old["build"]}) + "\n")
        self.fetched(self.new["build"])
        self.work.mkdir()

    @staticmethod
    def write(path, text):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def fetched(self, build):
        self.write(self.cache / "spinel-config.json", json.dumps({"build": build, "dataVersion": "v"}))
        self.write(self.cache / "version", json.dumps({"build": build}))

    def go(self, **kwargs):
        prover = FakeProver(self.repo, self.new, **kwargs)
        return watch.run(self.cache, self.repo, self.work, prover), prover

    def test_the_pinned_build_is_current_and_nothing_is_read_or_written(self):
        self.fetched(self.old["build"])
        before = snapshot(self.tmp)
        verdict, _ = self.go()
        self.assertEqual((verdict.outcome, verdict.build, verdict.previous, verdict.body),
                         ("current", self.old["build"], self.old["build"], None))
        self.assertEqual(snapshot(self.tmp), before)

    def test_a_proven_build_moves_every_pin_and_prepares_the_pull_request(self):
        verdict, prover = self.go()
        self.assertEqual((verdict.outcome, verdict.title), ("proven", "fix(net): pin game build %s" % self.new["build"][:8]))
        written = {n: (self.repo / "crates" / "net" / "src" / n).read_text() for n in sources(self.new)}
        self.assertEqual(written, sources(self.new))
        pins = self.repo / "tools" / "build-migrate" / "pins" / (self.new["build"] + ".json")
        self.assertEqual(json.loads(pins.read_text()), self.new)
        self.assertEqual(json.loads((self.repo / "tools/build-migrate/sampler/pins.json").read_text())["build"],
                         self.new["build"])
        self.assertEqual(prover.verified, [(pins, self.repo / "crates" / "net" / "src")])
        self.assertTrue((self.repo / "tools/build-migrate/pins" / (self.old["build"] + ".json")).is_file())
        for fact in (self.new["build"], self.old["build"], "400 signed methods match", "crates/net/src/profile.rs",
                     "crates/net/src/decode.rs", "tools/build-migrate/sampler/pins.json",
                     "`INVENTORY_CUTOFF_BUILD` carries forward"):
            self.assertIn(fact, verdict.body)

    def test_a_cutoff_that_was_off_stays_off(self):
        path = self.repo / "crates" / "net" / "src" / "decode.rs"
        path.write_text("const INVENTORY_CUTOFF_BUILD: Option<&str> = None;\n")
        verdict, _ = self.go()
        self.assertEqual(verdict.outcome, "proven")
        self.assertEqual(path.read_text(), "const INVENTORY_CUTOFF_BUILD: Option<&str> = None;\n")
        self.assertIn("did not name the pinned build", verdict.body)

    def assert_review(self, verdict, before, *facts):
        self.assertEqual(verdict.outcome, "review")
        self.assertEqual(verdict.title, "Game build %s needs review" % self.new["build"][:8])
        self.assertEqual(snapshot(self.tmp), before)
        for fact in (self.new["build"], self.old["build"]) + facts:
            self.assertIn(fact, verdict.body)

    def test_a_failed_proof_leaves_the_tree_alone_and_lists_the_lines(self):
        before = snapshot(self.tmp)
        lines = ["CHANGED   f1  [A.dll]A.B::C() (5 tokens; reviewed 8502ebbf:4)",
                 "UNMATCHED [A.dll]A.B::D(): missing method",
                 "CATALOG   overlay x defines catalog tables MapInfoEx; Net would miss merged rows",
                 "PIN       pins.dataVersion: expected \"1\", derived \"2\"",
                 "SAMPLER   MOVED cells x: 1 -> 2"]
        verdict, prover = self.go(failed=lines, sampler=False)
        self.assert_review(verdict, before, *lines)
        self.assertEqual(prover.verified, [])

    def test_a_site_that_is_not_found_once_is_a_review_not_a_crash(self):
        path = self.repo / "crates" / "net" / "src" / "codec.rs"
        path.write_text(path.read_text() + path.read_text().splitlines()[0] + "\n")
        before = snapshot(self.tmp)
        verdict, _ = self.go()
        self.assert_review(verdict, before, "REPIN     codec.rs holds 2 matches of the old seed pin")

    def test_a_sampler_that_lags_the_pinned_build_is_a_review(self):
        path = self.repo / "crates" / "net" / "src" / "sampler.js"
        path.write_text(path.read_text().replace(self.old["build"], "9" * 32))
        before = snapshot(self.tmp)
        verdict, _ = self.go()
        self.assert_review(verdict, before, "sampler.js does not hold the pinned build %s" % self.old["build"])

    def test_a_missing_sampler_proof_is_a_review_even_if_nothing_else_failed(self):
        before = snapshot(self.tmp)
        verdict, _ = self.go(sampler=False)
        self.assert_review(verdict, before, "REPIN     no sampler pins to carry to the new build")

    def test_a_check_that_fails_after_the_rewrite_undoes_every_write(self):
        before = snapshot(self.tmp)
        verdict, prover = self.go(after=["STALE     profile.rs lacks x"])
        self.assertEqual(len(prover.verified), 1)
        self.assert_review(verdict, before, "REPIN     check failed after the rewrite", "STALE     profile.rs lacks x")

    def test_a_pins_file_that_already_exists_needs_a_person(self):
        self.write(self.repo / "tools/build-migrate/pins" / (self.new["build"] + ".json"), "{}")
        before = snapshot(self.tmp)
        verdict, _ = self.go()
        self.assert_review(verdict, before, "already exists; a return to a reviewed build needs a person")

    def test_an_unreadable_deployment_is_an_error_not_an_outcome(self):
        (self.cache / "version").unlink()
        with self.assertRaisesRegex(Malformed, "missing public artifact version") as caught:
            self.go()
        self.assertNotIsInstance(caught.exception, Transient)
        (self.cache / "version").write_text("[1]")
        with self.assertRaises(Malformed) as caught:
            self.go()
        self.assertNotIsInstance(caught.exception, Transient)
        self.fetched("not a build id")
        with self.assertRaisesRegex(Malformed, "no build id") as caught:
            self.go()
        self.assertNotIsInstance(caught.exception, Transient)

    def test_a_config_and_version_that_disagree_are_a_deploy_in_progress(self):
        (self.cache / "version").write_text(json.dumps({"build": "f" * 32}))
        with self.assertRaisesRegex(Transient, "disagree"):
            self.go()

    def test_a_gateway_build_that_differs_from_config_is_a_deploy_in_progress(self):
        name = artifacts.METADATA.encode()
        head = 20 + 12 + len(name)
        data = b"UnityWebData1.0\0" + struct.pack("<I", head) + struct.pack("<III", head, 4, len(name)) + name + b"meta"
        build = types.SimpleNamespace(cache=types.SimpleNamespace(build="b" * 32, data=data),
                                      compiled={"build": {"value": "a" * 32, "usage": 1}})
        with self.assertRaisesRegex(Transient, "compiled gateway build differs from config"):
            derive.pins(build)

    def test_a_deploy_in_progress_is_not_a_finding_but_any_other_failure_to_read_is(self):
        prover = watch.Artifacts(self.repo, self.work)
        cache = types.SimpleNamespace(build="b" * 32)
        moved = Transient("compiled gateway build differs from config")
        broken = Malformed("metadata entry is missing or duplicated")
        with mock.patch.object(watch.derive, "Build", return_value=cache), \
                mock.patch.object(watch.derive, "pins", side_effect=moved):
            with self.assertRaises(Transient):
                prover.prove(self.cache, self.old)
        with mock.patch.object(watch.derive, "Build", return_value=cache), \
                mock.patch.object(watch.derive, "pins", side_effect=broken):
            proof = prover.prove(self.cache, self.old)
        self.assertEqual(proof.failed, ["ERROR     Malformed: metadata entry is missing or duplicated"])

    def test_an_empty_list_of_signed_methods_is_not_a_proof(self):
        before = snapshot(self.tmp)
        verdict, prover = self.go(matched=0, signed=0)
        self.assert_review(verdict, before, "SIGNED    0 of 0 signed methods match")
        self.assertEqual(prover.verified, [])

    def test_every_signed_method_must_match(self):
        before = snapshot(self.tmp)
        verdict, _ = self.go(matched=399, signed=400)
        self.assert_review(verdict, before, "SIGNED    399 of 400 signed methods match")

    def sampler_stub(self, source):
        self.write(self.repo / "tools" / "build-migrate" / "sampler" / "derive.py", source)

    def sampler_cache(self):
        names = ["a.loader.js", "b.framework.js.gz", "c.wasm.gz", "d.data.gz"]
        return types.SimpleNamespace(build=self.new["build"], root=self.cache, names=names)

    def test_a_sampler_pin_that_moved_is_a_review(self):
        self.sampler_stub("import sys\nprint('MOVED cells x: 1 -> 2', file=sys.stderr)\nsys.exit(1)\n")
        sampler_cache, new = self.sampler_cache(), self.new

        class Prover(watch.Artifacts):
            """The real sampler step and the real decision, on an invented cache."""

            def prove(self, cache, old):
                sampler, failed = self.sampler(sampler_cache, old["build"])
                return watch.Proof(new, failed, 400, sampler, 400)

        before = snapshot(self.tmp)
        verdict = watch.run(self.cache, self.repo, self.work, Prover(self.repo, self.work))
        self.assert_review(verdict, before, "SAMPLER   MOVED cells x: 1 -> 2")

    def test_a_sampler_that_derives_without_a_move_returns_its_pins(self):
        self.sampler_stub(
            "import sys\nout = sys.argv[sys.argv.index('--out') + 1]\n"
            "open(out, 'w').write('{\"build\": \"' + sys.argv[sys.argv.index('--build') + 1] + '\"}\\n')\n")
        found, failed = watch.Artifacts(self.repo, self.work).sampler(self.sampler_cache(), self.old["build"])
        self.assertEqual((failed, found[0].name, json.loads(found[1])),
                         ([], "pins.json", {"build": self.new["build"]}))

    def main(self, *extra, prover=None):
        out = self.tmp / "github-output"
        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(io.StringIO()):
            code = watch.main([str(self.cache), "--repo", str(self.repo), "--work", str(self.work),
                               "--github-output", str(out), *extra], prover=prover)
        return code, stdout.getvalue(), dict(line.split("=", 1) for line in out.read_text().splitlines()) \
            if out.exists() else {}

    def test_main_prints_one_outcome_and_writes_the_names_the_workflow_needs(self):
        code, printed, outputs = self.main(prover=FakeProver(self.repo, self.new))
        b = self.new["build"]
        self.assertEqual((code, printed), (0, "proven\n"))
        self.assertEqual(outputs, {"outcome": "proven", "build": b, "previous": self.old["build"], "short": b[:8],
                                   "title": "fix(net): pin game build %s" % b[:8], "branch": "auto/game-build-%s" % b[:8],
                                   "body_file": str(self.work / "pr-body.md")})
        self.assertIn(b, (self.work / "pr-body.md").read_text())
        self.assertFalse((self.work / "issue-body.md").exists())

    def test_main_names_the_issue_for_a_review(self):
        code, printed, outputs = self.main(prover=FakeProver(self.repo, self.new, failed=["CHANGED   f1 x"]))
        self.assertEqual((code, printed, outputs["outcome"], outputs["body_file"]),
                         (0, "review\n", "review", str(self.work / "issue-body.md")))
        self.assertEqual(outputs["title"], "Game build %s needs review" % self.new["build"][:8])
        self.assertIn("CHANGED   f1 x", (self.work / "issue-body.md").read_text())

    def test_main_exits_3_and_changes_nothing_when_the_files_disagree_on_the_build(self):
        (self.cache / "version").write_text(json.dumps({"build": "f" * 32}))
        before = snapshot(self.tmp)
        code, printed, outputs = self.main(prover=FakeProver(self.repo, self.new))
        self.assertEqual((code, printed, outputs), (3, "", {}))
        self.assertEqual(snapshot(self.tmp), before)

    def test_main_exits_3_when_the_build_moved_since_the_probe(self):
        before = snapshot(self.tmp)
        code, printed, outputs = self.main("--expect-build", "c" * 32, prover=FakeProver(self.repo, self.new))
        self.assertEqual((code, printed, outputs), (3, "", {}))
        self.assertEqual(snapshot(self.tmp), before)
        code, printed, outputs = self.main("--expect-build", self.new["build"], prover=FakeProver(self.repo, self.new))
        self.assertEqual((code, printed, outputs["outcome"]), (0, "proven\n", "proven"))

    def test_main_for_the_pinned_build_needs_no_artifacts_at_all(self):
        self.fetched(self.old["build"])
        code, printed, outputs = self.main()
        self.assertEqual((code, printed, outputs),
                         (0, "current\n", {"outcome": "current", "build": self.old["build"],
                                            "previous": self.old["build"]}))

    def test_main_exits_2_on_a_tool_error_and_refuses_bodies_inside_git(self):
        (self.cache / "version").unlink()
        self.assertEqual(self.main()[0], 2)
        self.fetched(self.new["build"])
        (self.repo / ".git").mkdir()
        self.assertEqual(self.main("--work", str(self.repo / "work"))[0], 2)
        self.assertFalse((self.repo / "work").exists())


class Bodies(unittest.TestCase):
    def test_the_issue_body_fences_its_lines_and_caps_them(self):
        body = watch.issue_body("a" * 32, "b" * 32, ["CHANGED   uses ``` fence", "PIN       ok"])
        self.assertIn("````\nCHANGED   uses ``` fence\nPIN       ok\n````", body)
        many = watch.issue_body("a" * 32, "b" * 32, ["PIN       line %d" % i for i in range(watch.ISSUE_LINES + 5)])
        self.assertIn("... and 5 more line(s)", many)
        self.assertNotIn("line %d" % (watch.ISSUE_LINES + 1), many)

    def test_a_pull_request_body_never_starts_a_line_with_a_release_keyword(self):
        change = watch.Change(["crates/net/src/profile.rs"], [], "PASS: 0 failure(s) for build x")
        body = watch.pull_request_body("a" * 32, "b" * 32, 400, change)
        self.assertFalse(any(line.startswith("BREAKING CHANGE") for line in body.splitlines()))
        self.assertIn("did not name the pinned build", body)

    def test_outputs_refuse_a_line_break(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "out"
            watch.write_outputs(path, {"a": "1", "b": "2"})
            watch.write_outputs(path, {"c": "3"})
            self.assertEqual(path.read_text(), "a=1\nb=2\nc=3\n")
            with self.assertRaises(Malformed):
                watch.write_outputs(path, {"d": "x\ny"})


if __name__ == "__main__":
    unittest.main()
