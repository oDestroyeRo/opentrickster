"""Per-build pins derived from one public-artifact cache.

Every value here is located through a build-independent join (canonical method names, usage
kinds, byte lengths and, for non-volatile inputs, content hashes), never through an old offset.
"""

import base64
import hashlib
import json
import re
import struct

from . import artifacts, il2cpp, unityfs, wasm
from .fingerprint import ADDRESS_FLOOR, Fingerprinter, is_memory
from .wasm import Malformed, check

NET = "[Spinel.Net.dll]Spinel.Net."
GAME = "[Spinel.Game.dll]Spinel.Game.Play."
GATEWAY_BUILD = NET + "GatewayBuild::.cctor()"
SIGN_IN = NET + "Session/<SignInAsync>d__22::MoveNext()"

# name, owner method, usage kind, byte length, pinned SHA-256 or None for a per-build value,
# and how the manifest may show the bytes ("text", "u32", "i32" or None for hash only).
COMPILED = (
    ("build", GATEWAY_BUILD, il2cpp.USAGE_LITERAL, 32, None, "text"),
    ("seed", GATEWAY_BUILD, il2cpp.USAGE_LITERAL, 44, None, "text"),
    ("c2s", GATEWAY_BUILD, il2cpp.USAGE_LITERAL, 3,
     "99e0ee5a83b3906160f1eab2e4007117fccc4a26652dc94955a3c6628aab4926", "text"),
    ("s2c", GATEWAY_BUILD, il2cpp.USAGE_LITERAL, 3,
     "8e504f255032f8cc5db7a20678e49cd36df604ed4c189171109b8701c34739c4", "text"),
    ("marker", SIGN_IN, il2cpp.USAGE_LITERAL, 5, None, None),
    ("client_version", NET + "Session::.cctor()", il2cpp.USAGE_FIELD_RVA, 4, None, None),
    ("firstlogin_wait", SIGN_IN, il2cpp.USAGE_FIELD_RVA, 12,
     "e85ae1df05cd9499a0e332f696bfb39e5484807b31ae52775a1048a1b9672862", "u32"),
    ("selection_wait", NET + "Characters/<SelectCharacterAsync>d__14::MoveNext()",
     il2cpp.USAGE_FIELD_RVA, 12,
     "fd61c7f1fde7eea7c0b75cb3f0600177bbee3dc76f4ea51cc5329224d2cac349", "u32"),
    ("md5_rotations", NET + "Md5::.cctor()", il2cpp.USAGE_FIELD_RVA, 256,
     "b89a927779b66394fd309fdda16496644a11ab469c5370e8e8a6a6ee25617e7f", None),
    ("catalog_key", "[Spinel.Formats.dll]Spinel.Formats.Libconfig::.cctor()",
     il2cpp.USAGE_FIELD_RVA, 16,
     "b5b8c3c831b51613bc8701615db8ec52cda99e4610d0f452a9c984619ba9dea0", None),
    ("normal_form_effects", GAME + "ItemsModel::.cctor()", il2cpp.USAGE_FIELD_RVA, 24,
     "65a96e5111fde179295e050214fd70276da0446bb6fec006bfc2bc87b759efc3", "i32"),
    ("gun_slots", GAME + "GunFlowing::.cctor()", il2cpp.USAGE_FIELD_RVA, 16,
     "f79067ab1dc0e54d3c4b8d6c007d3a06f9ba33296ce8d0e6e038b335115bf800", "i32"),
)
VOLATILE = tuple(spec[0] for spec in COMPILED if spec[4] is None)

# Lookup tables the compiler placed in initialized data: name, owner, I32 entries. A fingerprint
# sees only the first 16 bytes behind an address, so these are read in full.
DATA_TABLES = (
    ("login_failure", NET + "LoginFailure::Of([mscorlib.dll]System.Exception)", 11),
)


class Build:
    """One cache, parsed and joined. Construction never executes target code."""

    def __init__(self, root):
        self.cache = artifacts.Cache(root)
        self.module = wasm.Module(self.cache.wasm)
        self.meta = il2cpp.Metadata(self.cache.metadata)
        self.rt = il2cpp.Runtime(self.meta, self.module)
        self._by_name = None
        plain = Fingerprinter(self.rt)
        self.compiled = {spec[0]: locate(self, plain, spec) for spec in COMPILED}
        volatile = {self.compiled[name]["usage"]: name for name in VOLATILE}
        self.fingerprinter = Fingerprinter(self.rt, volatile)

    def methods_named(self, name):
        """Metadata method indices whose canonical name is `name`."""
        if self._by_name is None:
            self._by_name = {}
            for index, m in enumerate(self.meta.methods):
                self._by_name.setdefault(self.meta.string(m["name"]), []).append(index)
        simple = name.split("::", 1)[1].split("(", 1)[0]
        return [i for i in self._by_name.get(simple, ()) if self.rt.method_name(i) == name]

    def function_of(self, name):
        """(method index, join) for a canonical name that must resolve to one joined body."""
        found = self.methods_named(name)
        if len(found) != 1:
            raise Malformed("%s method: %s" % ("ambiguous" if found else "missing", name))
        joined = self.rt.method_function.get(found[0])
        if joined is None:
            raise Malformed("method has no generated body: " + name)
        return found[0], joined

    def join_record(self, name):
        index, (slot, function, start, size) = self.function_of(name)
        body = self.module.data[start:start + size]
        return {"name": name, "index": index, "token": "0x%x" % self.meta.methods[index]["token"],
                "slot": slot, "function": function, "body": "0x%x" % start, "bytes": size,
                "sha256": hashlib.sha256(body).hexdigest()}


def target_bytes(build, kind, index):
    if kind == il2cpp.USAGE_LITERAL:
        return index, build.meta.literal(index)
    field_index, _ = build.rt.field_ref(index)
    return field_index, build.rt.rva_data(field_index)


def locate(build, fingerprinter, spec):
    name, owner, kind, length, pinned, show = spec
    _, (_, function, _, _) = build.function_of(owner)
    found = []
    for cell, k, index in fingerprinter.usages(function):
        if k != kind:
            continue
        resolved, (offset, data) = target_bytes(build, k, index)
        digest = hashlib.sha256(data).hexdigest()
        if len(data) == length and (pinned is None or digest == pinned):
            found.append((cell, index, resolved, offset, data, digest))
    if pinned is None:
        # A per-build value must be the only target of its kind and length in its owner.
        others = {f[1] for f in found}
        check(len(others) == 1, "%s: %d candidate targets in %s" % (name, len(others), owner))
    else:
        check(found, "%s: pinned content %s… is absent from %s" % (name, pinned[:12], owner))
        check(len({f[1] for f in found}) == 1, "%s: pinned content is ambiguous" % name)
    cell, index, resolved, offset, data, digest = found[0]
    record = {"method": owner, "cell": "0x%x" % cell,
              "target": ("literal %d" if kind == il2cpp.USAGE_LITERAL else "fieldRef %d → field %d")
              % ((index,) if kind == il2cpp.USAGE_LITERAL else (index, resolved)),
              "offset": "0x%x" % offset, "length": length, "sha256": digest,
              "usage": build.rt.describe_usage(kind, index)}
    if show == "text":
        record["value"] = data.decode("ascii")
    elif show in ("u32", "i32"):
        record["value"] = list(struct.unpack("<%d%s" % (length // 4, "I" if show == "u32" else "i"), data))
    return record


def rotl(x, n):
    return ((x << n) | (x >> (32 - n))) & 0xFFFFFFFF


def opcode_map(seed, direction):
    """The gateway opcode permutation (Spinel.Net.OpcodeMap.FromSeed) and its inverse."""
    check(len(seed) == 32 and direction in (b"c2s", b"s2c"), "opcode map input")
    a, b, c, d = struct.unpack_from("<4I", hashlib.sha256(seed + direction).digest())
    wire = list(range(65536))
    for i in range(65535, 0, -1):
        j = (rotl((b * 5) & 0xFFFFFFFF, 7) * 9 & 0xFFFFFFFF) % (i + 1)
        wire[i], wire[j] = wire[j], wire[i]
        a, b, c, d = a ^ b ^ d, b ^ a ^ c, c ^ a ^ ((b << 9) & 0xFFFFFFFF), rotl(b ^ d, 11)
    inverse = [0] * 65536
    for logical, mapped in enumerate(wire):
        inverse[mapped] = logical
    return wire, inverse


def table_hash(table):
    return hashlib.sha256(struct.pack("<65536H", *table)).hexdigest()


def opcode_maps(seed_text):
    seed = base64.b64decode(seed_text, validate=True)
    out = {}
    for direction in ("c2s", "s2c"):
        wire, inverse = opcode_map(seed, direction.encode())
        out[direction + "_wire"] = table_hash(wire)
        out[direction + "_inverse"] = table_hash(inverse)
    return out


def pins(build):
    """The sanitized per-build manifest: everything Net pins for this build."""
    cache = build.cache
    directory = artifacts.unity_directory(cache.data)
    entry = next(e for e in directory if e["name"] == artifacts.METADATA)
    compiled = {k: {f: v for f, v in r.items() if f != "usage"} for k, r in build.compiled.items()}
    check(compiled["build"]["value"] == cache.build, "compiled gateway build differs from config")
    return {
        "build": cache.build,
        "deployment": {name: {"bytes": len(b), "sha256": artifacts.sha256(b)}
                       for name, b in (("spinel-config.json", cache.config), ("version", cache.version),
                                       ("index.public.html", cache.page))},
        "dataVersion": cache.data_version,
        "assets": [{"name": name, "bytes": len(a["bytes"]), "sha256": artifacts.sha256(a["bytes"])}
                   for name, a in cache.assets.items()],
        "metadata": {"offset": entry["offset"], "bytes": entry["length"],
                     "sha256": artifacts.sha256(cache.metadata),
                     "magic": cache.metadata[:8].hex()},
        "compiled": compiled,
        "opcode_maps": opcode_maps(compiled["seed"]["value"]),
        "catalog": catalog(cache),
        "tables": {spec[0]: data_table(build, spec) for spec in DATA_TABLES},
        **definitions(cache),
    }


def data_table(build, spec):
    """The one initialized-data table that the owner method loads from, read as I32 entries."""
    name, owner, count = spec
    fp = build.fingerprinter
    _, (_, function, _, _) = build.function_of(owner)
    found = set()
    for ins in wasm.decode(build.module, function)[1]:
        value = ins.imm[0] if ins.op == "i32.const" else ins.imm[1] if is_memory(ins) else None
        if value is None or not (ADDRESS_FLOOR <= value < fp.data_end) or not fp.initialised(value):
            continue
        if not (fp.cells[0] <= value < fp.cells[1]):
            found.add(value)
    check(len(found) == 1, "%s: %d data tables in %s" % (name, len(found), owner))
    address = found.pop()
    return {"address": address, "value": list(struct.unpack_from("<%di" % count, build.rt.mem, address))}


DEFINITIONS = ("game_definitions.json", "items.json")


def definitions(cache):
    """Hashes of the resolved catalog JSON when the cache carries it (VerifiedProfile prefers it)."""
    present = [(cache.root / n).is_file() for n in DEFINITIONS]
    if not any(present):
        return {}
    check(all(present), "the cache carries only one catalog JSON file")
    blobs = {n: (cache.root / n).read_bytes() for n in DEFINITIONS}
    check(json.loads(blobs[DEFINITIONS[0]]).get("build") == cache.build,
          "game_definitions.json names another build")
    return {"definitions": {n: artifacts.sha256(b) for n, b in blobs.items()}}


def catalog(cache):
    """Catalog inputs carried by the Unity bundle: overlay identities and relevance."""
    found = unityfs.catalog_inputs(cache.entry("data.unity3d"))
    return {"overlays": {name: {k: v for k, v in o.items() if k != "table_sha256"}
                         for name, o in found["overlays"].items()},
            "spineldatasettings": found["spineldatasettings"]}


def rust_expectations(p):
    """(file, regex) pairs every pin must satisfy in crates/net/src after a migration."""
    c = p["compiled"]
    out = [("profile.rs", r'pub const BUILD: &str = "%s";' % p["build"]),
           ("profile.rs", r"const MAX_METADATA: usize = %d;" % p["metadata"]["bytes"]),
           ("profile.rs", r'"%s"' % p["metadata"]["sha256"]),
           ("profile.rs", r'"%s"' % p["deployment"]["spinel-config.json"]["sha256"]),
           ("profile.rs", r'"%s"' % p["deployment"]["version"]["sha256"]),
           ("codec.rs", r'const SEED: &str = "%s";' % re.escape(c["seed"]["value"]))]
    for asset in p["assets"]:
        out.append(("profile.rs", r'name: "%s",\s*size: %d,\s*hash: "%s"'
                    % (re.escape(asset["name"]), asset["bytes"], asset["sha256"])))
    for name in ("marker", "client_version", "build", "seed"):
        f = c[name]
        out.append(("profile.rs", r'field\(\s*metadata,\s*%d,\s*%d,\s*"%s",?\s*\)'
                    % (int(f["offset"], 16), f["length"], f["sha256"])))
    for name in ("catalog_key", "normal_form_effects", "gun_slots"):
        f = c[name]
        start = int(f["offset"], 16)
        out.append(("catalog.rs", r'\.get\(0x%x\.\.0x%x\)[^"]*"%s"' % (start, start + f["length"], f["sha256"])))
    for key, digest in p["opcode_maps"].items():
        out.append(("codec.rs", r'"%s"' % digest))
    for digest in p.get("definitions", {}).values():
        out.append(("definitions.rs", r'"%s"' % digest))
    return out
