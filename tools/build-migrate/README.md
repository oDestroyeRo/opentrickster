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
python3 -I $T repin /tmp/ot-NEW --from tools/build-migrate/pins/OLD.json --rust crates/net/src
```

`check` exits 1 on any changed or unmatched method, any pin that differs from
`pins/<build>.json`, a catalog overlay that defines a table Net reads, or (with `--rust
crates/net/src`) a Rust literal that does not equal the derived pin. Use `--exclude-source
assisted-input-control.md` to check only methods cited by other reports.

A fresh `fetch` carries no catalog JSON, so `check` prints `SKIPPED pins.definitions` and
compares every other pin. Step 4 below supplies the catalog JSON.

`repin` rewrites the Rust literals that hold the pins of the build in `--from` to the values of
the build in `CACHE`. It uses the same table as `check --rust`, `derive.rust_sites`, so a literal
that `check` accepts is a literal that `repin` finds. Each old pin must match exactly once. A
missing or duplicated site stops the command with the file and the pin, and writes nothing.
Values are replaced where they sit. Layout, comments and digit separators stay, so the files stay
rustfmt-shaped.

`repin` also moves the two strings that only follow the build: `INVENTORY_CUTOFF_BUILD` in
`decode.rs` and `samplerBuild` in `sampler.js`. A follower that holds another build stays and is
reported as `KEPT`. The `definitions.rs` hashes stay when `CACHE` carries no catalog JSON. They
guard only a `game_definitions.json` that a person places in a cache. `fetch_public` never writes
one. It downloads the catalog files that `catalog::load` reads, and Net falls back to that loader
when no JSON is present.

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

## Automatic migration

`watch.py` decides whether a new official build can ship without a person. The workflow
`.github/workflows/game-build.yml` runs it every hour, at minute 17, and on demand.

```sh
python3 -I tools/build-migrate/migrate.py fetch /tmp/ot-NEW
python3 -I tools/build-migrate/watch.py /tmp/ot-NEW --work /tmp/ot-work
```

The command prints one outcome. With `--github-output FILE` it also writes `outcome`, `build` and
`previous`, plus `short`, `title`, `body_file` and `branch` when there is a body to post.
Details go to stderr. It exits 2 when the tool itself fails, for example when the fetched config
or version is missing or the two name different builds.

| Outcome   | Meaning                                                    | Tree                              |
| --------- | ---------------------------------------------------------- | --------------------------------- |
| `current` | The deployed build is the build that `BUILD` pins.         | Unchanged.                        |
| `proven`  | Every proof below holds.                                   | Pins, Rust and sampler rewritten. |
| `review`  | A proof is missing, or the rewrite could not be verified.  | Unchanged.                        |

A build is `proven` only when all of these hold:

1. Every signed method matches a reviewed fingerprint. A `CHANGED` or `UNMATCHED` method fails.
2. No overlay defines a table that Net's catalog reads, and the bundle does not name
   `SpinelDataSettings`.
3. The pins that a build does not derive again are equal to the reviewed pins: the metadata
   magic, `dataVersion`, the owner and length of each compiled field, the pinned contents, the
   login failure table, and the catalog summary (`derive.invariants`).
4. The config and version equal the reviewed files once the new build string is put back as
   the old one. So the first-login endpoint and the data version did not move.
5. The sampler pins derive for the new build with no `MOVED` or `CHANGED` line. The reviewed
   file is the one in `sampler/` whose `build` equals the pinned `BUILD`. It is found by content.
6. `pins/<new>.json` does not exist yet. A return to a reviewed build needs a person.
7. `repin` finds every old pin exactly once, in `crates/net/src` and `sampler.js`.
8. `check NEW --rust crates/net/src` passes on the files as written. On any failure the watcher
   restores every file it wrote.

On `proven` the watcher writes `pins/<new>.json` (the derived manifest without `methods`),
rewrites the Rust pins, writes the new sampler pins over the file that named the old build, and
writes `pr-body.md`. On `review` it writes `issue-body.md` with the `CHANGED`, `UNMATCHED`,
`CATALOG`, `PIN`, `SAMPLER` and `REPIN` lines that stopped it.

The sampler pins file keeps its name, `ea1a68bd.json`, because `crates/net/tests/sampler.cjs`
opens it by that name. Its `build` field holds the pinned build.

The inventory cutoff carries forward. The trials in `docs/research/native-inventory-trials.md`
verified the signed client methods, so when every one of them still matches its reviewed
fingerprint, `INVENTORY_CUTOFF_BUILD` moves with `BUILD`. When a signed method changed, the
outcome is `review` and a person decides.

### Workflow

The workflow runs on `main` only. It holds `contents`, `pull-requests`, `issues` and `actions`
write permission and uses `GITHUB_TOKEN`. Runs never overlap (concurrency group `game-build`).

- `current`: it stops.
- `proven`: it skips when a pull request from `auto/game-build-<first 8 hex>` is already open.
  Otherwise it installs Rust 1.99.1, runs `cargo fmt -p opentrickster-net` and
  `cargo test -p opentrickster-net --locked`, commits `fix(net): pin game build <first 8 hex>`
  to that branch, pushes, and opens a pull request. A pull request opened with `GITHUB_TOKEN`
  starts no workflow, so it dispatches `ci.yml` on the branch, finds the run by head SHA and
  waits for it. On success it squash merges with `--match-head-commit`, then dispatches `ci.yml`
  on `main`. The `fix` type makes that run release a PATCH version. On a CI failure it leaves
  the pull request open and fails the job.
- `review`: it opens the issue `Game build <first 8 hex> needs review` with the label
  `needs-triage`, unless an open issue has that title.

Only `watch.py` holds logic. The workflow runs Git and `gh`.

### Limits

- Only methods in `signatures.json` are proven. Other methods can differ.
- The server catalog files are not fetched again. `dataVersion` must be unchanged. Net also
  refuses another data version when it downloads the catalog.
- `loader.js` and `framework.js` are pinned by hash and not analyzed.
- A republish under the same build id, such as a changed descriptor or asset hash, reports
  `current`.
- Server acceptance of the new seed and opcode maps is not tested.
- The hourly run first reads the 4 KiB `version` file. It downloads the full build, about 71 MB,
  only when the deployed build differs from `BUILD`.
- The watcher does not use `--exclude-source`. Every signed method must match.

## Migrating by hand

Migrate by hand for every `review` issue, and whenever a method changed, the catalog inputs
changed, a sampler pin moved, or the descriptors differ beyond the build string.

1. `fetch` into a new directory, then run `check` against it.
2. For each `CHANGED` method, run `diff`. If only names were renumbered, or a change does not
   alter a wire layout, opcode or wait set that Net implements, record the review with
   `accept CACHE NAME --review "why"`. Otherwise stop: do not change the production code for
   that path, and report the difference.
3. Review `derive` output, then save it without `methods` as `pins/<build>.json`.
4. If the catalog inputs are unchanged, `definitions OLD NEW OUT` carries the reviewed catalog
   JSON forward and prints its new hashes.
5. Run `repin CACHE --from pins/<old>.json --rust crates/net/src`, then run `check NEW --rust
   crates/net/src` and the Net tests.
6. Derive the sampler pins as `sampler/README.md` describes and copy the `MOVED` lines into
   `sampler.js`.
7. Set `INVENTORY_CUTOFF_BUILD` to `None` if a signed client method changed, until the trials
   pass on the new build.

Once a reviewed change is recorded with `accept` and merged in `signatures.json`, a later run can
prove the build.

`sign BASELINE --cite docs/research/*.md crates/net/README.md` rebuilds `signatures.json` from
the citations. Run it only for a reviewed baseline, citing documents that describe that build.

Unit tests use synthetic inputs only:
`python3 -I -m unittest discover -s tools/build-migrate/tests`.
