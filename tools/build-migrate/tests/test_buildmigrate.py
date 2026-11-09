"""Synthetic checks for the build-migration tool. No public artifacts or network are needed:
`python3 -I -m unittest discover -s tools/build-migrate/tests`."""

import gzip
import hashlib
import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from buildmigrate import artifacts, derive, fingerprint, unityfs  # noqa: E402
from buildmigrate.wasm import Cursor, Malformed  # noqa: E402


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


if __name__ == "__main__":
    unittest.main()
