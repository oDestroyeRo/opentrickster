"""Public deployment artifacts: fetch, gzip-by-magic decoding and the UnityWebData directory."""

import hashlib
import json
import re
import struct
import urllib.request
import zlib
from pathlib import Path

from .wasm import Malformed, check

ORIGIN = "https://tr-play.playall.in.th/"
METADATA = "Il2CppData/Metadata/global-metadata.dat"
KINDS = (".loader.js", ".framework.js.gz", ".wasm.gz", ".data.gz")
LIMITS = {"spinel-config.json": 4096, "version": 4096, "index.public.html": 262144}
ASSET_LIMIT = 128 * 1024 * 1024


def sha256(b):
    return hashlib.sha256(b).hexdigest()


def decoded(raw, limit=ASSET_LIMIT):
    """Decode gzip only when the bytes carry its magic, never because of a file name."""
    if raw[:2] != b"\x1f\x8b":
        return raw, "identity"
    d = zlib.decompressobj(16 + zlib.MAX_WBITS)
    out = d.decompress(raw, limit + 1)
    check(len(out) <= limit and d.eof and not d.unused_data, "gzip body is truncated or oversized")
    return out, "gzip"


def asset_names(page):
    """The four Build/ assets the page references, by role. Each role must occur once."""
    text = page.decode("utf-8")
    names = sorted(set(re.findall(r"Build/([0-9a-f]{32}\.(?:loader\.js|framework\.js\.gz|wasm\.gz|data\.gz))", text)))
    out = {}
    for name in names:
        kind = next(k for k in KINDS if name.endswith(k))
        check(kind not in out, "page references two %s assets" % kind)
        out[kind] = name
    check(len(out) == 4, "page does not reference exactly four Build assets")
    return [out[k] for k in KINDS]


def unity_directory(data):
    """Walk the UnityWebData directory with the same bounds as the Rust profile."""
    check(data[:16] == b"UnityWebData1.0\0", "UnityWebData magic")
    end = struct.unpack_from("<I", data, 16)[0]
    check(20 <= end <= len(data), "directory end")
    p = 20
    entries = []
    while p < end:
        check(p + 12 <= end, "truncated directory record")
        offset, length, n = struct.unpack_from("<III", data, p)
        p += 12
        check(n <= end - p, "directory name extent")
        name = data[p:p + n].decode("utf-8")
        p += n
        check(end <= offset and offset + length <= len(data), "directory entry extent")
        entries.append({"name": name, "offset": offset, "length": length})
    check(p == end, "directory cursor does not stop at its end")
    return entries


class Cache:
    """A public-artifact directory in the layout VerifiedProfile::load_cache reads."""

    def __init__(self, root):
        self.root = Path(root)
        self.config = self.read("spinel-config.json")
        self.version = self.read("version")
        self.page = self.read("index.public.html")
        c = json.loads(self.config)
        v = json.loads(self.version)
        check(c.get("build") == v.get("build") and re.fullmatch(r"[0-9a-f]{32}", c.get("build", "")),
              "config and version disagree on the build")
        self.build = c["build"]
        self.data_version = c.get("dataVersion")
        self.first_login = c.get("firstLogin")
        self.names = asset_names(self.page)
        self.assets = {}
        for name in self.names:
            raw = self.read(name, ASSET_LIMIT)
            body, encoding = decoded(raw)
            self.assets[name] = {"bytes": body, "encoding": encoding}
        self.loader, self.framework, self.wasm, self.data = (self.assets[n]["bytes"] for n in self.names)
        check(self.wasm[:4] == b"\0asm", "WASM magic")
        self.directory = unity_directory(self.data)
        matches = [e for e in self.directory if e["name"] == METADATA]
        check(len(matches) == 1, "metadata entry is missing or duplicated")
        self.metadata_entry = matches[0]
        e = self.metadata_entry
        self.metadata = self.data[e["offset"]:e["offset"] + e["length"]]

    def read(self, name, limit=None):
        path = self.root / name
        limit = LIMITS.get(name, limit)
        check(path.is_file(), "missing public artifact " + name)
        check(path.stat().st_size <= limit, "oversized public artifact " + name)
        return path.read_bytes()

    def entry(self, name):
        matches = [e for e in self.directory if e["name"] == name]
        check(len(matches) == 1, "Unity data entry missing or duplicated: " + name)
        e = matches[0]
        return self.data[e["offset"]:e["offset"] + e["length"]]


def inside_git(path):
    path = Path(path).resolve()
    return any((p / ".git").exists() for p in [path, *path.parents])


def fetch(root):
    """Cookie-free public GETs of config, version, page and its four Build assets.

    Assets are stored decoded under their published names, matching the Rust cache layout.
    Redirects, non-200 responses and oversized bodies fail. No other request is made.
    """
    root = Path(root)
    check(not inside_git(root), "refusing to write public artifacts inside a Git work tree")
    root.mkdir(parents=True, exist_ok=True)
    check(not any(root.iterdir()), "fetch needs a new or empty directory")
    opener = urllib.request.build_opener(_NoRedirect)
    record = {}

    def get(path, limit):
        request = urllib.request.Request(ORIGIN + path, headers={
            "User-Agent": "Mozilla/5.0 (opentrickster build-migrate)", "Cache-Control": "no-cache"})
        with opener.open(request, timeout=120) as response:
            check(response.status == 200 and response.geturl() == ORIGIN + path, "unexpected response")
            body = response.read(limit + 1)
            check(len(body) <= limit, "oversized response")
            return body, response.headers.get("Date", "")

    for name, path in (("spinel-config.json", "spinel-config.json"), ("version", "version"),
                       ("index.public.html", "")):
        body, date = get(path, LIMITS[name])
        (root / name).write_bytes(body)
        record[name] = {"bytes": len(body), "sha256": sha256(body), "date": date}
    for name in asset_names((root / "index.public.html").read_bytes()):
        raw, date = get("Build/" + name, ASSET_LIMIT)
        body, encoding = decoded(raw)
        (root / name).write_bytes(body)
        record[name] = {"raw_bytes": len(raw), "raw_sha256": sha256(raw), "encoding": encoding,
                        "bytes": len(body), "sha256": sha256(body), "date": date}
    return record


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        raise Malformed("redirects are refused")
