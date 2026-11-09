# Build ea1a68bd migration

Issue [#37](https://github.com/oDestroyeRo/opentrickster/issues/37), account-free static research, observed **2026-11-09 UTC**. The deployment moved from build **8502ebbf2fc9033d2fba50d02fd92002** to **ea1a68bd89958fae94179f1e0835ffb5**. This report re-derives every per-build pin that `crates/net` uses and compares the method bodies behind Net's wire layouts. It follows the playbook in [current-build revalidation](current-build-protocol-revalidation.md).

**Result.** The gateway build, opcode seed, both directional maps, asset names and every compiled-field locator moved. No wire layout, opcode constant or wait set that Net implements changed. In `Spinel.Net.dll`, only `GatewayLink.Tick` and `GatewayLink.get_Done` changed. Both are browser link policy, not frames. Net's pins now target ea1a68bd and still fail closed: the ignored cache test refuses the 8502ebbf cache with `BuildChanged`.

Evidence classes: **source** [S] means static decoding of current public artifacts. **synthetic** [Y] means checks on invented fixtures. **live** means target-server behavior. Only source and synthetic evidence were produced. No live acceptance is claimed.

## Provenance and limits

Work ran on branch `feat/migrate-build-ea1a68bd`, based on `origin/main` `3a34004`. Cookie-free GETs fetched the [config](https://tr-play.playall.in.th/spinel-config.json), [version](https://tr-play.playall.in.th/version), [page](https://tr-play.playall.in.th/), and the four `Build/` assets the page names. The responses were dated 07:26:15–07:26:45 GMT. Gzip was detected by magic. The decoded files live in an outside-Git cache. None of them is committed.

No target WASM or JavaScript was executed. No game socket was opened, no account was used, and no credential, compiled key or private state was read or printed. The marker, client-version bytes, catalog key and MD5 rotation table appear below only as locators, lengths and SHA-256 values.

The encrypted `pack/` catalog responses, `data-server.json` and `data-key` were **not** re-fetched. `dataVersion` is still `KIcgbcTZu7eh`. The catalog checks reuse the decoded files from that data version. The `data-key` hash and the six `FILES` pins in `catalog.rs` are therefore unchanged and unverified for this build.

## Method

The new [`tools/build-migrate`](../../tools/build-migrate/README.md) tool (Python 3 standard library) produced every value in this report. For the old cache, it reproduced every 8502ebbf pin that the earlier reports and Rust source record. That covers `pins/8502ebbf2fc9033d2fba50d02fd92002.json`, written by hand from those documents, and the `--rust` literal check run before the Rust edit. Every cited 8502ebbf body hash also resolved to a joined method in that build. That reproduction validates the tool.

- **Joins** use canonical names (`[image]Namespace.Type/Nested::Method(params)`), usage kinds, lengths and, for fields expected to stay constant, content hashes. Old offsets are never reused. Usage kind 5 indexes three consecutive tables: methodRefsOnType (method, class instance), genericMethodRefs (method, method instance) and methodSpecs (method, class instance, method instance). The methodSpecs base was 23634 in 8502ebbf.
- **Fingerprints** hash the decoded body after replacing only relocatable immediates. Callees become canonical names, usage cells become their resolved target, static and data addresses become `bss` or their pointed-to content, and IL2CPP initialization runs are sorted. Opcodes, field offsets, branches and all other constants remain. The build, seed, marker and client-version usages are named `volatile`.
- **Signatures** cover every method whose 8502ebbf body SHA-256 or `f@offset` citation appears in `docs/research/*.md` or the pre-migration `crates/net/README.md` (`origin/main` `3a34004`): 400 names from 382 bodies. Two cited runtime helpers have no metadata name (f64765, f65640). They are covered only through their callers' helper skeletons.

## Deployment and assets

| Pin | 8502ebbf | ea1a68bd |
|---|---|---|
| `spinel-config.json` (124 bytes) | `084900013e6edde0ce0eca286dc7ac884a86da2ece1bae361ce0671852092602` | `0e30790458ae431e691b66f05c8bd1fff11d109a23c2ccb014520715c7389b78` |
| `version` (44 bytes) | `9ce11c71ef039ffbe6bede54dfcf5b27afb887d44b2a79f57f959638e2e771b0` | `c4ddd91c18ba505935d361296dca3559580f4d8b1880594dd7e9ed6db07741cf` |
| page | 37950 / `7690892d…` | 37583 / `f9cd52c6bf781b3d533bc0caa00328ff69e772000f7ee9ec8aba67dad60c55cd` |
| loader | `41e1cc21….loader.js` 27914 / `5a4ba6ac…` | `91193a291f9ba5b072d1057270cdcf58.loader.js` 27914 / `8a7d5786f4b9c4db61c9687f97043cadbb0d3c8dc949e6cba38a666ec8c02a1a` |
| framework | `7f982dae….framework.js.gz` 391375 / `d1590592…` | `06b4021535921ba830a3e2a51effeeef.framework.js.gz` 393439 / `291c9124760f70c66c6754e1b6a56648e02d7b3423bf7a1d35088dce6a89871b` |
| WASM | `d9a4c4fd….wasm.gz` 43136343 / `381a183e…` | `6396dfe935f7f0e3745817a7761644bd.wasm.gz` 43147580 / `8780db4414df5357d9595ef233579227a1303753d00ac6318d225036dd1bb277` |
| Unity data | `6135c413….data.gz` 27724487 / `fa85bfa0…` | `9c68dd2a390f8dfafc9311184fecb2b6.data.gz` 27730162 / `a3b60bc72c11d7e38c05ed7cd312c26f7161df31118401f0d56a9b4cba3693a3` |
| metadata | offset 16748911, 9343896 / `7313f719…` | offset 16750046, 9348436 / `2c7ae9e8bbd0d4e38b2e7be06cb0fabf8c0affa23d9400ea3b4c76aedc248056` |

Sizes and hashes are of decoded bytes. Metadata magic stays `af1bb1fa 6c000000` (format 108). Only `build` changed in the config. `firstLogin` and `dataVersion` are byte-for-byte equal.

## Compiled fields

| Field | Owner | 8502ebbf cell / target / offset | ea1a68bd cell / target / offset | Len | SHA-256 |
|---|---|---|---|---:|---|
| build | `GatewayBuild::.cctor` | `0x344aa0` / literal 1079 / `0x1f1a9` | `0x35749c` / literal 19940 / `0x75e57` | 32 | `861ecf9f31d8f480e79fbc947a733784d6888db7332f875c6758bf1be522e096` (was `7cd4e6cd…`) |
| seed | `GatewayBuild::.cctor` | `0x3551dc` / literal 17930 / `0x6e72d` | `0x344da0` / literal 1057 / `0x1f114` | 44 | `b556afb4f46147c318705ac82b6e0e3211e4c20138a7c82425f88ff3395a4441` (was `bc25174f…`) |
| c2s | `GatewayBuild::.cctor` | `0x35637c` / 19058 / `0x72a7a` | `0x3566e4` / 19062 / `0x72b1f` | 3 | unchanged `99e0ee5a…` |
| s2c | `GatewayBuild::.cctor` | `0x35b3b4` / 24193 / `0x859ae` | `0x35b720` / 24198 / `0x85a73` | 3 | unchanged `8e504f25…` |
| marker | `<SignInAsync>d__22::MoveNext` | `0x356f38` / 19809 / `0x7545b` | `0x3572a0` / 19813 / `0x75500` | 5 | unchanged `b5a2c962…` |
| client_version | `Session::.cctor` | `0x3430c4` / fieldRef 249 → 69623 / `0x4e7818` | `0x34341c` / 249 → 69652 / `0x4e8308` | 4 | unchanged `e4abdaa9…` |
| firstlogin_wait | `<SignInAsync>d__22::MoveNext` | `0x3430d0` / 252 → 69626 / `0x4e78c8` | `0x343428` / 252 → 69655 / `0x4e83b8` | 12 | unchanged; `[11502,11503,11508]` |
| selection_wait | `<SelectCharacterAsync>d__14::MoveNext` | `0x3430e4` / 257 → 69631 / `0x4e7a10` | `0x34343c` / 257 → 69660 / `0x4e8500` | 12 | unchanged; `[2014,2013,2]` |
| md5_rotations | `Md5::.cctor` | `0x3430a8` / 242 → 69616 / `0x4e7590` | `0x343400` / 242 → 69645 / `0x4e8080` | 256 | unchanged `b89a9277…` |
| catalog key | `Libconfig::.cctor` | `0x342df8` / 70 → 79074 / `0x4ed2a8` | `0x343150` / 70 → 79111 / `0x4edd98` | 16 | unchanged `b5b8c3c8…` |
| normal-form effects | `ItemsModel::.cctor` | `0x342ec0` / 120 → 53446 / `0x4d09c8` | `0x343218` / 120 → 53474 / `0x4d14b8` | 24 | unchanged; `[15,21,23,24,25,26]` |
| gun slots | `GunFlowing::.cctor` | `0x34301c` / 207 → 53533 / `0x4d16d0` | `0x343374` / 207 → 53561 / `0x4d21c0` | 16 | unchanged; `[4,9,1,22]` |

The compiled build string equals the config build. The public seed is now **`2/WjKXyCAPUpaeCR6GvABIVYGlCB4ya96Nes0dnYxfQ=`**. `OpcodeMap.FromSeed` is fingerprint-identical, so the algorithm is unchanged. The new SHA-256 values of the 131072-byte tables are: c2s wire `977a07329e40e76a8e9c7defd7f4fddab0b4a91549ddfb91c6b4ad9b330e1b74`, c2s inverse `2748c043b44b39d14b2167a03bf980d9465300b7b9c1d6bdc465df3adfa98b15`, s2c wire `127612d7863ef59d81ec45955eb0d94c63b17aff20202aebb559694becec7e5c`, and s2c inverse `c0d04331f75045d8c167e591dc4743386c42911238195e372413d353bd248faf`. The synthetic seed fixture (`c87a5dc1…`) is unchanged. [S][Y]

## Catalog inputs

A standard-library UnityFS/LZ4 decoder read `data.unity3d`. For 8502ebbf it reproduced the reviewed overlay hashes. Both overlays changed in ea1a68bd: `libconfig_thai` is 1428612 bytes, `ce9bbf37ce4a3b4073931516c733e3f4106c443d5091562c18ae4d7e8fb8d257`, and `libconfig_eng` is 1123380 bytes, `99b45f266e9f8b2b89cd6c18e4fcb1842b9357ad4f4b2ed65dcd7220b6db96c4`. Each still has 127 tables. Only `ClassicCustomUIParams` and `SpinelCustomUIParams` differ. Neither overlay defines any IIDX read-set table or any of the six decoded tables. No lowercase `spineldatasettings` Resources key exists. The mixed-case `SpinelDataSettings` MonoScript record is present in both builds. `LibconfigOverlay.Apply`, `Tables` and `InstallSpinel` are fingerprint-identical. [S]

The catalog key, effects and gun slots are unchanged, and the data version is unchanged. `game_definitions.json` therefore differs only in its `build` string. `migrate.py definitions` checks those conditions before carrying the reviewed file forward. The new file's hash is `d2d724bc329e4eef19f55089473530fd5481d6eaefbea72bd4e7ea51e9986d8e` (was `4a7c86ea…`). `items.json` is unchanged at `50d82abd…`. Independently, Rust `catalog::load` resolved the decoded catalog through the new metadata locators, and `verified_public_catalog` passed. [S][Y]

## Method bodies

**Cited methods.** 394 of the 400 signatures are normalized-identical. Of the six changes, three are in Net's scope and were reviewed and accepted in `signatures.json`:

- `GatewayLink.ConnectAsync` and `GatewayLink.Opened` changed only in compiler-generated closure ordinals (`DisplayClass74_0`/`b__74_0` → `75_0`, `DisplayClass81_0` → `85_0`). The renamed closure bodies are normalized-identical.
- `GatewayLink.Tick` (new m70649, f9971, body `0x2eca28`/1271) now gates probe-tick counting (`0x2ecb31`–`0x2ecb41`) and reconnect `StartAttempt` (`0x2ecd9e`–`0x2ecdae`) on `pageVisible (+65) || backgroundRunning (+66)` instead of `pageVisible` alone. The new field is set by the new `GatewayLink.BackgroundRunning`. Thresholds, timers and the `p` probe are unchanged. Net's transport keeps its always-visible foreground policy, which already takes the visible branch.

The other three changes are cited only by [assisted input](assisted-input-control.md), which belongs to the sampler migration. They are left unaccepted, so a full `check` fails until that migration reviews them:

- `GameplayScreen.MapPoint`: `probedAt` moved from `+620` to `+624` because the new field `drivers` (`AvatarDrivers`) now occupies `+620`. `probedTime` stays at `+632`. The sampler reads `screen+620`.
- `GameplayScreen.ClickAt` changed only in the `<EnterWarp>d__380` → `d__384` ordinal.
- `GameplayScreen/<Enter>d__283.MoveNext` adds `BackgroundRun` registration and the `drivers` store, and renumbers lambdas.

**Whole images.** `migrate.py compare` pairs renamed compiler-generated members:

| Image | Same | Changed | Only in ea1a68bd | Unpaired trivial closure constructors |
|---|---:|---:|---:|---:|
| `Spinel.Net.dll` | 4660 | 2: `GatewayLink.Tick`, `GatewayLink.get_Done` | 3: `GatewayLink.BackgroundRunning`, `GatewayLink.get_OnTime`, `MyCampLink.get_HouseValues` | 21 |
| `Spinel.Formats.dll` | 322 | 2: `SubImage.PackFrom`, `SubImage.Read` | 0 | 0 |
| `Spinel.Game.dll` | 17960 | 75, none cited | 7 | 82 |

`GatewayLink.get_Done` now also requires `deferredClose (+220) == null`. All other `Spinel.Net.dll` readers, writers, opcode constants and wait sets are normalized-identical. That includes the state decoders, which [state completion](target-state-completion.md) cites without body hashes. The changed `Spinel.Game.dll` methods are features Net does not implement: AutoQuest, ItemPackWindow, WorldRouter/Travel, Tm dye and others. Dig, gun, item, combat-model and inventory paths cited by the reports are identical. [S]

**Rejoined rows used by Net's README.** Logical 24 in `SendMove` is now at `0x65d7c5`. `Attack` sends 288 at `0x31f28f` and 29 at `0x31f341`. `CombatModel.WorldClick` calls it at `0x54ab21`. The `HoldsGun` subtype-54 check spans `0x77a8a2`–`0x77a8a7`. Each was mapped by identical normalized instruction index. [S]

| Method | Metadata row | Function | Body | SHA-256 |
|---|---:|---:|---|---|
| `GameSession.SendMove` | 72131 | 20903 | `0x65d730` / 231 | `afc2879c67969afcb3a8fe98d0de800907e7ac6b8ed2008ec80305fbbc7280b7` |
| `CombatLink.Attack` | 71411 | 10814 | `0x31f22c` / 386 | `48ca4eaf9704d4f735b03a03720ad3b867ffcc44d9ee7c8019e66d8c9c12ec93` |
| `GunFlowing.HoldsGun` | 40861 | 24383 | `0x77a669` / 614 | `10abc2bbde9fb343a8a2bd8210c5e3e54140ffedc869f1650f4790800a8fd7cb` |
| `GunFlowing.Shoot` | 40877 | 24401 | `0x77b75b` / 427 | `b0582e31b320f45c43fdc9b3e026b61bbb255196d8a465b22d3ad1beb754d370` |
| `Gun.Crc` | 72645 | 12914 | `0x398d39` / 30 | `6dcbf85444a0383346b827c7555c8749e0979b27e8ceae970dd4cc17bc2aca02` |
| `Gun.Shot` | 72646 | 12915 | `0x398d58` / 89 | `b8299b57e7f6a6920fb671275255d40ab233808be2a491877661842a93ed4726` |

`migrate.py derive` prints the joins for all 400 signed methods.

## Reproduction

```sh
T=tools/build-migrate/migrate.py
python3 -I $T fetch "$NEW"                       # outside Git
python3 -I $T check "$OLD"                       # PASS: 400 signatures, 8502ebbf pins
python3 -I $T check "$NEW" --rust crates/net/src --exclude-source assisted-input-control.md   # PASS
python3 -I $T check "$NEW"                       # FAIL: only the 3 sampler-owned methods above
python3 -I $T compare "$OLD" "$NEW" --image Spinel.Net.dll
python3 -I $T definitions "$OLD" "$NEW" "$OUT"   # carried catalog JSON hashes
python3 -I -m unittest discover -s tools/build-migrate/tests
OPENTRICKSTER_PUBLIC_CACHE="$NEW" cargo test -p opentrickster-net --locked -- --ignored verified_public_cache
```

## Remaining gaps

- The sampler's `GameplayScreen` reads (`screen+620`) and its build pin belong to the sampler migration.
- Server-side catalog pins (`data-server.json`, `data-key`, the six decoded `FILES`) were not re-fetched for this build.
- No live sign-in, frame exchange or gameplay was attempted. Server acceptance of the new seed and maps is unproven.
- Fingerprints treat an address that points into initialized data as its first 16 bytes, with pointers masked. Longer tables need an explicit `DATA_TABLES` entry. The only such table today is `LoginFailure.Of`'s map, which moved from memory 1943852 to 1944300 and is still `[0,6,6,6,6,6,2,3,4,2,1]`. Error 60013 therefore still maps to occupied.
