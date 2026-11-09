"""Read-only UnityFS bundle decoding (LZ4/LZ4HC or stored blocks) and TextAsset lookup.

Used to re-prove that the public catalog overlays (`libconfig_thai`, `libconfig_eng`) inside
`data.unity3d` are byte-identical to the reviewed ones, so the catalog resolution is unchanged.
"""

import hashlib
import re
import struct

from .wasm import check

# Reviewed public TextAsset overlays (docs/research/target-game-definitions.md).
OVERLAYS = {
    "libconfig_thai": "ce9c8f53be329c6097212a451dc241bffe8e2a2230abbc8154bb1eed2e0d6758",
    "libconfig_eng": "8fc4019b037f01107035b65c410800606a7954004daf1272a2b387f3d6378ac2",
}


def lz4_block(src, size):
    """Decode one raw LZ4 block of known decoded size."""
    out = bytearray()
    i, n = 0, len(src)
    while True:
        check(i < n, "truncated LZ4 block")
        token = src[i]
        i += 1
        literal = token >> 4
        if literal == 15:
            while True:
                check(i < n, "truncated LZ4 literal length")
                b = src[i]
                i += 1
                literal += b
                if b != 255:
                    break
        check(i + literal <= n, "LZ4 literal overruns its block")
        out += src[i:i + literal]
        i += literal
        if i == n:
            break
        check(i + 2 <= n, "truncated LZ4 offset")
        offset = src[i] | src[i + 1] << 8
        i += 2
        check(0 < offset <= len(out), "LZ4 offset outside the window")
        match = token & 15
        if match == 15:
            while True:
                check(i < n, "truncated LZ4 match length")
                b = src[i]
                i += 1
                match += b
                if b != 255:
                    break
        match += 4
        start = len(out) - offset
        if offset >= match:
            out += out[start:start + match]
        else:
            pattern = bytes(out[start:])
            out += (pattern * (match // offset + 1))[:match]
        check(len(out) <= size, "LZ4 block exceeds its declared size")
    check(len(out) == size, "LZ4 block size mismatch")
    return bytes(out)


def _decompress(kind, raw, size):
    if kind == 0:
        check(len(raw) == size, "stored block size mismatch")
        return raw
    check(kind in (2, 3), "unsupported UnityFS compression %d" % kind)
    return lz4_block(raw, size)


def _cstr(b, p):
    end = b.index(b"\0", p)
    return b[p:end].decode("utf-8"), end + 1


def files(bundle):
    """{node path: bytes} for a UnityFS bundle."""
    check(bundle[:8] == b"UnityFS\0", "UnityFS signature")
    version = struct.unpack_from(">I", bundle, 8)[0]
    check(version in (6, 7, 8), "unsupported UnityFS version %d" % version)
    _, p = _cstr(bundle, 12)
    _, p = _cstr(bundle, p)
    total, packed, unpacked, flags = struct.unpack_from(">QIII", bundle, p)
    p += 20
    check(total == len(bundle), "UnityFS size field")
    if version >= 7:
        p = (p + 15) & ~15
    if flags & 0x80:
        info_at = len(bundle) - packed
    else:
        info_at, p = p, p + packed
    info = _decompress(flags & 0x3F, bundle[info_at:info_at + packed], unpacked)
    if flags & 0x200:
        p = (p + 15) & ~15
    count = struct.unpack_from(">I", info, 16)[0]
    q = 20
    blocks = []
    for _ in range(count):
        blocks.append(struct.unpack_from(">IIH", info, q))
        q += 10
    stream = bytearray()
    for size, length, block_flags in blocks:
        stream += _decompress(block_flags & 0x3F, bundle[p:p + length], size)
        p += length
    check(p <= info_at or flags & 0x80 == 0, "UnityFS blocks overlap their directory")
    nodes = struct.unpack_from(">I", info, q)[0]
    q += 4
    out = {}
    for _ in range(nodes):
        offset, size, _ = struct.unpack_from(">QQI", info, q)
        path, q = _cstr(info, q + 20)
        check(offset + size <= len(stream), "UnityFS node extent")
        out[path] = bytes(stream[offset:offset + size])
    return out


def text_assets(blob, name):
    """Candidate TextAsset scripts named `name`: an aligned length-prefixed name followed by an
    aligned length-prefixed script, as Unity serializes `m_Name` then `m_Script`."""
    encoded = name.encode()
    needle = struct.pack("<I", len(encoded)) + encoded
    pad = (-len(needle)) % 4
    found = []
    at = blob.find(needle)
    while at >= 0:
        p = at + len(needle) + pad
        if at % 4 == 0 and blob[at + len(needle):p] == b"\0" * pad and p + 4 <= len(blob):
            size = struct.unpack_from("<I", blob, p)[0]
            if 0 < size <= len(blob) - p - 4:
                found.append(blob[p + 4:p + 4 + size])
        at = blob.find(needle, at + 1)
    return found


# Tables the native catalog reads: the IIDX read set and the six decoded XML tables. An overlay
# that defines any of them would be merged by the client, so Net's catalog would be incomplete.
RELEVANT_TABLES = frozenset((
    "ItemParamCM2", "ItemParamEx2", "ItemParamEx", "ItemParam2", "Item_DrillParam",
    "Item_GunParam2", "MapInfoEx", "MapAttr", "CharacterInfo", "MonsterParamEx2"))
TABLE = re.compile(rb"<TABLE\b([^>]*)>")
NAME = re.compile(rb"""\bname\s*=\s*(?:"([^"]*)"|'([^']*)')""")


def tables(text):
    """{table name: SHA-256 of its element text} for one overlay document."""
    starts = [(m.start(), m.group(1)) for m in TABLE.finditer(text)]
    check(len(starts) == text.count(b"<TABLE"), "unparsed TABLE element")
    out = {}
    for i, (at, attributes) in enumerate(starts):
        m = NAME.search(attributes)
        check(m is not None, "TABLE without a name")
        name = (m.group(1) or m.group(2) or b"").decode("utf-8")
        end = starts[i + 1][0] if i + 1 < len(starts) else len(text)
        check(name not in out, "duplicate overlay table " + name)
        out[name] = hashlib.sha256(text[at:end]).hexdigest()
    return out


def catalog_inputs(bundle):
    """Overlay summaries plus whether a Resources key names `spineldatasettings`, which could
    redirect the public data URL and would need inspection. Unity stores Resources container keys
    lowercased; the mixed-case `SpinelDataSettings` MonoScript class record is not such a key."""
    nodes = files(bundle)
    override = any(b"spineldatasettings" in blob for blob in nodes.values())
    return {"overlays": overlays(nodes), "spineldatasettings": override}


def overlays(nodes):
    """{overlay: summary} for the catalog overlays; each must exist exactly once."""
    out = {}
    for name in OVERLAYS:
        hits = [s for blob in nodes.values() for s in text_assets(blob, name)]
        check(len(hits) == 1, "%s: %d TextAsset candidates" % (name, len(hits)))
        names = tables(hits[0])
        out[name] = {"bytes": len(hits[0]), "sha256": hashlib.sha256(hits[0]).hexdigest(),
                     "tables": len(names), "relevant": sorted(RELEVANT_TABLES & set(names)),
                     "table_sha256": names}
    return out
