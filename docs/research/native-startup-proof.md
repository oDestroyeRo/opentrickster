# Native startup probe: stopped at a changed target build

**Native authentication and startup remain unproven.** The disposable Node process stopped at its public-build preflight on 2026-11-08, before reading the credential file or opening a game socket. The deployment changed from the prerequisite reports' build `f5e972fa2be429602c20d6168013a20f` to `8502ebbf2fc9033d2fba50d02fd92002` during this investigation. Both public configuration endpoints advertised the new build, the page referenced four different assets, and fresh downloads had different fingerprints. Sending the old build's recovered authentication packets would not test a verified current contract. [CFG], [V], [PAGE], [L2], [J2], [D2], [W2]

This is a progress report for [Verify native authentication and startup without the game client (#17)][I17], under [the MVP planning map (#2)][MAP]. Leave #17 open. The smallest prerequisite is to revalidate the gateway maps/framing and authentication/startup contracts against the new fingerprints below, including newly located compiled marker and compatibility fields. Then run one bounded native login sequence if the advertised build still matches. No application implementation, PR, or merge is part of this work.

The destination remains a macOS application, preferably Rust/GPUI, with the game embedded in client-assisted mode and an information dashboard in clientless mode. The MVP controls one account and its existing character for combat and digging; quest flows are research only. This result changes none of those requirements and supplies no evidence of native action acceptance or mode continuity. [MAP]

## Scope and provenance

The checkout began clean at `6b8162f1f7a42b2bd9dd02c1b086be360914bb8d` on `research/native-startup-proof`. The local and intended published branch names are identical. Repository identity, the assigned ticket, the map, and the authenticated developer were read before work; #17 was already assigned to `oDestroyeRo`. The final revision is recorded in the issue comment and delivery response, because a report cannot contain its own commit hash without changing it.

The checkout's `AGENTS.md` and all three `docs/agents/` pointers were read. No glossary or ADR files existed here. The research and Prove It Works skills were applied directly, following the instruction to use no subagents. There was one writer in this worktree. Only this report is committed; the map, other issues, production code, and main are unchanged.

Four prerequisite/context files were compared byte-for-byte with their stated Git blobs before use:

| Report | Verified immutable revision | Use here |
| --- | --- | --- |
| [Gateway framing and directional maps][R13] | `331f397fe2d6440bc24b9ee51f9b7caa6100962b` | Transport contract and locators |
| [Authentication, handoff, and startup][R15] | `97d6bf407a23bf87d054b40571d14224d0fb32ac` | Login serializers, selection, initial-state readers |
| [Incoming message layouts][R14] | `3a6c421d903483f1c977f6006fe01476d8da3223` | Aux grouping and reader semantics |
| [Live action correlation][R12] | `8920c260db742bbee93dd605be1d93450e4c428a` | Sanitized context only; no private capture accessed |

The parent's independent same-revision metadata-to-WASM joins and synthetic authentication/codec verification were reused. The existing public caches remained read-only. The disposable probe, public downloads, synthetic fixtures, and sanitized evidence are in `/private/tmp/opentrickster-native17.0h1SYk`, outside Git. No Unity/game/browser execution occurred, and no direct TCP connection was attempted.

## Evidence of the revision change

Earlier public GETs in this investigation returned the prerequisite build. That configuration's SHA-256 was `c9d15144ee0103975a8b4eca58c0bb36433d9b64d367bcf736d163b92997431b`; comparison with #15's configuration showed only `dataVersion` had changed. The earlier page still referenced all four prerequisite asset filenames. The cached loader/framework/data/WASM/metadata were rehashed successfully against #15. The compiled marker and client-version slices were checked against their published fingerprints without printing their bytes. [R13], [R15]

At `2026-11-08T16:46:45.471Z`, the probe began its single preflight execution. It stopped after **256 ms**, with sanitized result `build_changed`, stage `preflight`, exit status **2**. The build comparison precedes credential loading, the exclusive live-attempt latch, and socket construction in the inspected probe source. The evidence contains an empty connection list; the latch was absent. Thus **zero logical login sequences began**. This was not a server authentication refusal.

Subsequent independent Node and Python public GETs agreed on the changed build; Python's readback was at `16:47:23 UTC`. The following are SHA-256 hashes of received bodies, not compressed-on-disk size assumptions. The `.gz` asset responses were already decoded. All listed public retrievals returned HTTP 200. [CFG], [V], [PAGE], [L2], [J2], [D2], [W2]

| Public body after the stop | Received bytes | SHA-256 |
| --- | ---: | --- |
| [Configuration][CFG] | 124 | `084900013e6edde0ce0eca286dc7ac884a86da2ece1bae361ce0671852092602` |
| [Version][V] | 44 | `9ce11c71ef039ffbe6bede54dfcf5b27afb887d44b2a79f57f959638e2e771b0` |
| [Entry page][PAGE] | 37,583 | `ccb9859c7a626b0d5e419d7ebe2a1a30b652293be27c70773fc6793ac6c4101d` |
| [Current loader][L2] | 27,914 | `5a4ba6acdfc6ad420a285cc1faa54dc694892655bedd752e0f3f7dde3bae6a41` |
| [Current framework][J2] | 391,375 | `d159059282768082bbfc54432ab93ad68f6eaa57f84883c9e09780810597d917` |
| [Current WASM][W2] | 43,136,343 | `381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404` |
| [Current data package][D2] | 27,724,487 | `fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474` |
| Current extracted metadata | 9,343,896 | `7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade` |

The current Unity data directory places `Il2CppData/Metadata/global-metadata.dat` at offset **16748911**, length **9343896**. Its magic/version remain `0xfab11baf / 108`. The old slice began at **16459759**, length **9148328**. Matching metadata format version does not establish matching method indices, pointers, constants, or protocol behavior. The new assets were downloaded and fingerprinted only; they were not executed or disassembled. [D2], [R15]

The observations establish revision drift, not the cause of deployment, its exact rollout time, whether an old build remains accepted, or whether any particular protocol contract changed. No old-build compatibility experiment was attempted.

## Prepared contract and account-free checks

The probe transcribes the old pinned contract into standalone Node code, using Node's WebSocket implementation. It neither imports nor executes target JavaScript or WASM. Its binary-send allowlist is limited to FirstLogin `11501/11505`, LoginServer `2000/2012/2028`, and GameServer `4`. Repeated stage sends are prohibited; 2028 is a source-supported clock reply. There is no movement, attack, digging, quest, inventory operation, character creation/deletion, security-change, session-takeover, reconnect, or ticket-replay implementation.

Relevant primary instructions were inspected using the existing static disassembly, with metadata-backed locators from the immutable reports:

| Concern | Old-build primary code checked | Probe requirement |
| --- | --- | --- |
| FirstLogin and world transition | WASM 18259, 18255; e.g. `0x4ca5c2`, `0x4ca5f4`, `0x4c98a9`, `0x4c98e8`, `0x4c9a6e` | Exact password transform and fixed fields; returned world/island pair; 15-second application waits |
| Selection ordering | WASM 18086 and 18133, described in [R15] | Install the 2014 callback before sending 2012; the callback registers the 3/2 followup wait synchronously; retain intervening frames |
| Initial GameServer login | WASM 16184, `0x446f5f–0x446fc1`, `0x447219–0x44731a` | Register startup waits before the 14-byte opcode-4 send; wait for 5/159 and 679/230/233, with 20-second application timeouts |
| Connection versus session delivery | WASM 18127 and 16180, `0x446913`, `0x446a14`, `0x446c91` | Connection-level waits retain raw decoded frames; separate GameServer backlog honors Aux grouping |
| Own startup state | WASM 17093, 17095, 15011; `0x47d97a`, `0x47d99b`, `0x47d9a5`, `0x47da41`, `0x40397a`, `0x403989` | U16 actor at 230@4, U16 X/Y at @6/@8; U16 zone at 233@219; U32 current HP/MP at @235/@239; CP874 name from 679@17 |
| Optional-lock decline | WASM 47485, 47345, 47369, 47370; `0xfeff6b`, `0xfe3312`, `0xfe60d9`, `0xfe619d` | State-zero prompt/decline follows local information-view code; no security-setting packet is needed for this choice |
| Own-link cleanup | [Old framework][J1], release/leave functions around byte offsets 114301 and 114814 | Best-effort `net/release` only for tokens issued to this process; socket closure and HTTP status are separate from application logout |

These locators belong to the **old** artifacts in [R13]/[R15], not the new downloads. The old five-byte compiled marker and four-byte client-version field were extracted locally and fingerprint-checked. Their bytes were never printed. Their server authorization significance remains unproven.

The transport transcription uses the six-byte little-endian envelope, build-seeded directional permutation, binary-byte acknowledgements, retained-byte accounting, text controls, and stream reassembly across WebSocket message boundaries. It implements nonzero-Aux buffering: 1951 flushes/removes the group and delivers the marker; 1952 flushes/replaces the group and consumes the marker. Aux-zero messages deliver immediately. Those are client behaviors; they do not establish server transaction semantics. [R13], [R14]

`node probe.cjs --selftest` passed against the final local probe, without reading credentials or opening sockets. Checks covered pinned artifact/compiled-field/map fingerprints; every split of a synthetic coalesced frame pair; queued early replies; a coalesced 2014/3 followup; both Aux flush forms; password normalization and signed-key derivation; all CP874 byte roundtrips; fixed-string rejection; synthetic world/character/ticket parsing; every truncation of the synthetic ticket; startup widths and all shorter-than-251 stat payloads; byte acknowledgements inside a retained array, stale acknowledgements and out-of-range rejection; busy-code 60013 handling; oversized frames; and rejection of an unauthorized movement send. These verify the disposable transcription, not live acceptance.

The probe allows only a sole active world row and sole existing character, stops on a busy character or unexpected security state, and keeps private values in process memory. It would compare the returned ticket UID and profile name privately with selection, while publishing only presence/type/consistency. None of those account-specific branches was reached.

## Achieved and unachieved milestones

| Milestone | Actual result |
| --- | --- |
| Old-revision source and synthetic checks | Passed; no live inference |
| Current public build check | Failed closed: new build and different assets |
| Gateway connection / token / byte acknowledgements | Not attempted; zero sockets and no tokens |
| FirstLogin 11501 → 11502/3/8 | Not attempted |
| World 11505 → 11506/7 | Not attempted |
| LoginServer 2000 → 2002/1 | Not attempted |
| Sole existing character 2012 → 2014 then 3 | Not attempted |
| GameServer 4 → 5/159 | Not attempted |
| Startup 679/230/233; identity, position, zone, current HP/MP | Not observed; no real values or structural consistency claims |
| Normal refusal/error branches | Statically inspected/synthetically checked where stated; no server refusal observed |
| Cleanup | Process exited; zero probe-owned links to close or capabilities to release |
| Server logout | Not tested; no account login occurred |

The script's `finally` path zeroed its tracked buffers. No private network data existed because preflight stopped first. Immutable managed strings and runtime-owned buffers are released by process exit, not guaranteed secure erasure. The credential-delivery file was not read by the live preflight; publication checks separately read it programmatically to scan for leaks. The original delivery remains outside Git and is not deleted. No raw auth frames, credentials, derived credentials, account/character identifiers, game-state values, or tokens were written to scratch or the report.

## Reproduction and delivery checks

Local artifacts are intentionally outside Git and are not required product files:

| Artifact | SHA-256 |
| --- | --- |
| `probe.cjs` | `5bb6f7b51b875b989c91c06528b9a1d06db0cc2b98fc2821824d690ab497b37d` |
| `sanitized-evidence.json` | `71bc95941479f0aa3587b9e6ca481344dd0804516831ce7f0e649e8aa1dc9199` |

The sanitized evidence records runtime `v26.10.0`, start/end UTC, the 256-ms preflight stop, empty links, and cleanup. Its `allOwnedSocketsClosed: true` is vacuous for zero connections and must not be presented as a logout result. The requested one-login experiment has not begun; no login was retried. No background wait, browser session, PR watch, or probe process remains running.

Recheck the existing local transcription without network/account access:

```sh
node --check /private/tmp/opentrickster-native17.0h1SYk/probe.cjs
node /private/tmp/opentrickster-native17.0h1SYk/probe.cjs --selftest
```

For account-free verification of the current-public-versus-pinned mismatch, this standard-library script requires no local cache, credential, or game session. Mutable endpoints may change again; it prints build-coherence booleans and response fingerprints, never credentials or compiled field bytes:

```python
import hashlib, json, urllib.request

pinned = "f5e972fa2be429602c20d6168013a20f"
base = "https://tr-play.playall.in.th/"
builds = []
for name in ("spinel-config.json", "version"):
    request = urllib.request.Request(base + name, headers={
        "User-Agent": "Mozilla/5.0", "Cache-Control": "no-cache"})
    with urllib.request.urlopen(request, timeout=15) as response:
        body = response.read()
        assert response.status == 200
    build = json.loads(body)["build"]
    builds.append(build)
    print(name, "matches_pinned", build == pinned,
          "sha256", hashlib.sha256(body).hexdigest())
print("public_endpoints_agree", len(set(builds)) == 1)
```

Fresh asset verification uses the current page's four public URLs below, hashes decoded response bodies, and compares them with the table. Extract metadata by parsing the UnityWebData directory, rather than reusing the old absolute package offset. To repeat the stop offline, compare the saved fresh configuration/version `build` fields with the pinned constant; neither should match, and the saved evidence must have `links == []`, `failure.kind == "build_changed"`, and `outcome == "stopped"`.

Publication verification includes staged and committed credential scans (raw/normalized input, password MD5, selected 64-bit bytes in both endian orders and signed/unsigned decimal forms), one-report scope, Markdown/reference readback, and whitespace checks. Remote ref and published report must be independently read back before posting the single progress comment; the delivery response supplies the resulting commit and comment URL. No application build or CI/runtime gameplay proof is claimed.

## Remaining prerequisite

Revalidate the relevant portions of [#13][I13] and [#15][I15] against the **new current artifact fingerprints**, without assuming that old metadata offsets, function indices, gateway seed, marker/version fields, or message readers survived unchanged. This research does not modify those tickets. Once the current protocol evidence is established and the target still matches it, #17 can run the authorized bounded native sequence. A successful startup would still leave action acceptance, subsequent state updates, reliable logout/reconnect, mode ownership transfer, and the embedded macOS game runtime unproven.

## Sources

[I17]: https://github.com/oDestroyeRo/opentrickster/issues/17
[MAP]: https://github.com/oDestroyeRo/opentrickster/issues/2
[I13]: https://github.com/oDestroyeRo/opentrickster/issues/13
[I15]: https://github.com/oDestroyeRo/opentrickster/issues/15
[R13]: https://github.com/oDestroyeRo/opentrickster/blob/331f397fe2d6440bc24b9ee51f9b7caa6100962b/docs/research/target-protocol-framing.md
[R15]: https://github.com/oDestroyeRo/opentrickster/blob/97d6bf407a23bf87d054b40571d14224d0fb32ac/docs/research/target-authentication-handoff.md
[R14]: https://github.com/oDestroyeRo/opentrickster/blob/3a6c421d903483f1c977f6006fe01476d8da3223/docs/research/target-state-message-layouts.md
[R12]: https://github.com/oDestroyeRo/opentrickster/blob/8920c260db742bbee93dd605be1d93450e4c428a/docs/research/live-action-correlation.md
[CFG]: https://tr-play.playall.in.th/spinel-config.json
[V]: https://tr-play.playall.in.th/version
[PAGE]: https://tr-play.playall.in.th/
[L2]: https://tr-play.playall.in.th/Build/41e1cc21f8b1ab11275e55eb6949af3c.loader.js
[J2]: https://tr-play.playall.in.th/Build/7f982dae9f02da04f526a91e415474b4.framework.js.gz
[W2]: https://tr-play.playall.in.th/Build/d9a4c4fd2737e740b82e1321839e352f.wasm.gz
[D2]: https://tr-play.playall.in.th/Build/6135c4139b62e558bb4c755b266f5fa2.data.gz
[J1]: https://tr-play.playall.in.th/Build/4937d6d47bade23f56e5ad4b5bf2e2b9.framework.js.gz
