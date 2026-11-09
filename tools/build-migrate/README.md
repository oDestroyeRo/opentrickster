# Build migration

`migrate.py` re-derives every per-build pin that `crates/net` uses from a directory of public
artifacts, and proves that the method bodies behind Net's wire layouts did not change.
It needs only Python 3 and its standard library.

It reads bytes. It never instantiates the WASM, runs target JavaScript, opens a game socket,
signs in, or reads credentials. `fetch` makes cookie-free GETs of the config, version, page and
the four `Build/` assets that the page names. It refuses redirects and refuses to write inside
a Git work tree. Run every command with `python3 -I`.

## Commands

```sh
T=tools/build-migrate/migrate.py
python3 -I $T fetch /tmp/ot-NEW              # new, empty, outside-Git directory
python3 -I $T check /tmp/ot-NEW              # exit 0 only when every signature and pin matches
python3 -I $T derive /tmp/ot-NEW > new.json  # sanitized manifest: pins and method joins
python3 -I $T diff /tmp/ot-OLD /tmp/ot-NEW 'NAME' ['OLD=>NEW' ...]
python3 -I $T compare /tmp/ot-OLD /tmp/ot-NEW --image Spinel.Net.dll  # every method of an image
```

`check` exits 1 on any changed or unmatched method, any pin that differs from
`pins/<build>.json`, a catalog overlay that defines a table Net reads, or (with `--rust
crates/net/src`) a Rust literal that does not equal the derived pin. Use `--exclude-source
assisted-input-control.md` to check only methods cited by other reports.

A fresh `fetch` carries no catalog JSON, so `check` prints `SKIPPED pins.definitions` and
compares every other pin. Step 4 below supplies the catalog JSON.

## How it decides

- **Pins** come from build-independent joins, never from old offsets. For example, the gateway
  seed is the only 44-byte string literal that `GatewayBuild::.cctor()` initializes.
  `derive.COMPILED` lists every compiled field, its owner method and length. Fields that are
  not expected to change are also pinned by content hash.
- **Signatures** (`signatures.json`) cover each method whose original body SHA-256 or
  `[f]<function> @ <offset>` is cited by `docs/research/*.md` or `crates/net/README.md` for the
  baseline build. A fingerprint hashes the decoded body after replacing only relocatable
  immediates: callees become canonical method names, metadata-usage cells become the
  resolved type, method, field, literal hash or RVA-data hash, static-region and data addresses
  become `bss` or their pointed-to content, and IL2CPP initialization calls are sorted.
  Opcodes, field offsets, branch structure and every other constant stay literal. Per-build
  values (build, seed, marker, client version) are named `volatile` instead of hashed.
  An address into initialized data is described by its first 16 bytes. Longer lookup tables,
  such as `LoginFailure.Of`'s error map, are read in full through `derive.DATA_TABLES`.
- **Page**: the pin hashes the page without the Cloudflare Web Analytics beacon element, which
  the CDN injects before `</body>` in some responses only. The `8502ebbf` page pin predates this
  rule and includes the beacon.
- **Catalog inputs**: the Unity bundle is decoded (UnityFS, LZ4). The `libconfig_thai` and
  `libconfig_eng` overlays must not define a table that Net's catalog reads.

## Migrating to a new build

1. `fetch` into a new directory, then run `check` against it.
2. For each `CHANGED` method, run `diff`. If only names were renumbered, or a change does not
   alter a wire layout, opcode or wait set that Net implements, record the review with
   `accept CACHE NAME --review "why"`. Otherwise stop: do not change the production code for
   that path, and report the difference.
3. Review `derive` output, then save it without `methods` as `pins/<build>.json`.
4. If the catalog inputs are unchanged, `definitions OLD NEW OUT` carries the reviewed catalog
   JSON forward and prints its new hashes.
5. Update the Rust pins, then run `check NEW --rust crates/net/src` and the Net tests.

`sign BASELINE --cite docs/research/*.md crates/net/README.md` rebuilds `signatures.json` from
the citations. Run it only for a reviewed baseline, citing documents that describe that build.

Unit tests use synthetic inputs only:
`python3 -I -m unittest discover -s tools/build-migrate/tests`.
