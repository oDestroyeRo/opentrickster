# Native authentication and startup on the current build

The question in [#17][I17] is answered for build **8502ebbf2fc9033d2fba50d02fd92002**. One standalone Node process authenticated, selected the existing character, and read the initial own actor, position, zone, and current HP/MP without executing Unity or a browser. The server accepted FirstLogin, world/channel selection, LoginServer, character handoff, and GameServer login. Startup required **5 plus 679, 230, and 233**. Acceptance on opcode 5 alone was insufficient.

The live sequence started at **2026-11-08T18:11:06.895Z**, reached the complete baseline after **1801 ms**, and exited with status **0** after **1964 ms**. These are elapsed timestamps from one observation, including preflight and connection setup, not a latency benchmark. The result proves bounded native authentication and initial-state observation. It does not prove combat, digging, a complete clientless bot, reconnect, mode transfer, or server logout.

## Scope and evidence

This continuation began on `research/native-startup-current-build` at `78e604d73b7c59b4199f47b321884a036c7f7278`, with staged, unstaged, and untracked status clean. The checkout is `/Users/dos/.t3/worktrees/opentrickster/t3-195486cd`. Only this Markdown report belongs to its commit. No PR or merge is part of this research.

The project AGENTS.md and its issue-tracker, triage-labels, and domain pointers were read. Final review checked GLOSSARY.md terminology. No ADR directory was present. The research and Prove It Works skills were applied directly under the explicit instruction to use no subagents. Poteto investigation guidance supplied the source review and documentation checks. Prove It Works required actual socket-close events, process-liveness inspection, and publication readback. Model the Domain led to an explicit liveness state with a pending probe and visible-state policy. No product code changed.

The [current-build report][R18] was read in full and compared byte-for-byte with commit `df5fe0a5e8e45072845538fc74967a1757b6d9d7`. Its migration manifest SHA-256 is `f258e7ac8a99566adfd4019020a03cfc1d61e56a66d101ddbd0319a6bf756b84`. The parent's independently passed 75 method joins, 15440 type joins, maps, and source-boundary fixtures were reused rather than repeated. This continuation independently rehashed the inputs used by the probe and tested its own transcription.

The [earlier #17 report][R17] recorded zero login sequences because build drift stopped preflight. The old probe was copied into `/private/tmp/opentrickster-native-current.sba0qopn`. The original probe and shared caches stayed unchanged. All private network contents stayed in memory. Scratch contains source, invented fixtures, a single-use latch, and sanitized evidence only.

## Current deployment and source pins

Before reading credentials or constructing a WebSocket, the live process verified all four cached public artifacts, the extracted metadata, the UnityWebData directory entry, the compiled slices, the current public seed, both maps, and both inverse maps. It then fetched cookie-free config, version, and page bodies. Both config endpoints matched the pinned build. The page referenced all four pinned assets. [R18], [CFG], [VER], [PAGE]

| Fresh public response | Bytes | SHA-256 |
| --- | ---: | --- |
| Config | 124 | `084900013e6edde0ce0eca286dc7ac884a86da2ece1bae361ce0671852092602` |
| Version | 44 | `9ce11c71ef039ffbe6bede54dfcf5b27afb887d44b2a79f57f959638e2e771b0` |
| Page | 37583 | `ccb9859c7a626b0d5e419d7ebe2a1a30b652293be27c70773fc6793ac6c4101d` |

All three responses were HTTP 200. Page HTML differed from the #18 observation while its asset references matched. The four asset hashes below describe decoded cached bytes verified against that page and the immutable report, not fresh downloads in this continuation.

| Public artifact | SHA-256 |
| --- | --- |
| [Loader][LOADER] | `5a4ba6acdfc6ad420a285cc1faa54dc694892655bedd752e0f3f7dde3bae6a41` |
| [Framework][FRAMEWORK] | `d159059282768082bbfc54432ab93ad68f6eaa57f84883c9e09780810597d917` |
| [WASM][WASM] | `381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404` |
| [Data][DATA] | `fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474` |
| Metadata | `7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade` |

The data-directory parser located metadata at offset 16748911, length 9343896, and compared it with `current-global-metadata.dat`. The marker came from metadata offset `0x7545b`, length 5, hash `b5a2c96250612366ea272ffac6d9744aaf4b45aacd96aa7cfcb931ee3b558259`. Client-version bytes came from `0x4e7818`, length 4, hash `e4abdaa9e4fe688b531f6d5257aad881668ae7d84dd3e8fdf305cfcc0e29b9c6`. Actual field bytes are omitted. The current public seed is `Yi9ApMr88JAXsVztVJYSxdzPbba7CrrOlJ0GmnNHLXI=`. [R18]

| Current table, serialized U16LE | SHA-256 |
| --- | --- |
| c2s wire | `db98aa5f8e2df90f4d4baa756a9fad7849f6d307e2c155f5b6b5c9ba58c11fe1` |
| c2s inverse | `9e374006761f9bf18e3a59b6b4914c86b673f22230d9bab238e4c56120e350e4` |
| s2c wire | `e0386023dd44e6cd0e70099542b5f9943969feff662b5797cb73e5edab72f58d` |
| s2c inverse | `c5d1b355f0c5cf75af90935158df8d6b7309e860710b760fb1f04207169c6b9c` |

Current WASM functions 9967, 9969, 9976, and 9977 supplied the liveness logic. Inbound binary or text updates heard-at and clears a pending probe. After five seconds without traffic, Ask sends `p`. A pending probe becomes a loss only while visible, after at least two probe ticks and five elapsed seconds. The native process uses visible=true and a one-second tick, matching the framework cadence. Synthetic checks also cover hidden state and missing-token behavior. No liveness probe or byte acknowledgement was needed during the short live observation. [R18], [FRAMEWORK], [WASM]

Gateway busy classification uses current IsBusy 9951 and LoginFailure.Of 10004. Both occupied-account code 60013 and gateway Busy 8 stop the sequence. No release-held retry or takeover exists. The current incoming GmHello listener adds no outbound send or required startup wait. Logical 32597 was absent from this bounded capture, which does not prevent the minimum baseline. [R18]

## Account-free proof before login

The final probe passed syntax checking and `--selftest`. That mode replaces fetch and WebSocket with offline guards and prohibits reading the real credential path. Its checks cover the following behavior:

- Full artifact and compiled-slice hashes, all four map hashes, every inverse entry, and the exact custom CP874 table and byte round-trips.
- Synthetic credential normalization, signed key derivation, fixed-string rejection, world and character layouts, every truncated ticket, and minimum startup widths.
- Every split of a coalesced frame pair, queued early replies, a 2014 callback installed before selection, and synchronously registered 3/2 waits before the next coalesced frame.
- Independent Aux groups and both 1951/1952 flush forms. Raw connection waits complete independently of grouped session delivery.
- Partial-head byte ACKs, stale ACKs, out-of-range rejection, oversized frames, prohibited outbound opcodes, duplicate stage sends, and fixed outbound widths.
- Visible/hidden liveness, two-tick loss, inbound reset, ACK scheduling, no-token retention expiry, busy classification, and one-time release of a synthetic owned token.

`simulate.cjs` executed the entire final probe with synthetic credentials, in-memory files, fake sockets, and fake HTTP responses. Six scenarios passed. Success included a two-world list selecting the first active row, a source-supported 2028 reply, coalesced 2014 plus 3, and all startup frames arriving synchronously before opcode 5, with nonzero Aux still held by session groups. The other scenarios stopped on build drift, 60013, gateway busy, ambiguous characters, and GS 159. Drift produced zero credential reads and zero sockets. Every created synthetic socket closed, and only issued tokens were released.

During local preparation, a duplicated NUL in the data-magic comparison failed preflight and was corrected against the artifact. The gateway-busy simulation initially expected a release despite receiving no token; that test expectation was corrected. Both were resolved and all final checks passed before real credential access. They caused no live login or retry.

## One live sequence and observed gates

The real credential JSON was read programmatically after current-build checks and a mode-0600 check. The single-use latch prohibited another live run. The probe used the first active world/channel in server order and the sole existing character. Both choices were unambiguous in this response. Optional security followed the already-authorized local decline path. No setting-change packet was sent.

| Gate | Actual server result | Elapsed ms from process start |
| --- | --- | ---: |
| Preflight | Current profile passed before credentials and sockets | 343 |
| FirstLogin | One 11501 send, accepted by 11502 | 712 |
| World/channel | One 11505 send, accepted by 11506 | 782 |
| LoginServer | One 2000 send, accepted by 2002 | 1325 |
| Existing character | One 2012 send, 2014 ticket then opcode 3 | 1417 |
| GameServer | One 4 send, accepted by 5 | 1801 |
| Full baseline | All 679, 230, and 233 received and decoded | 1801 |
| Cleanup and exit | All owned sockets closed, exit 0 | 1964 |

No server refusal, security stop, busy result, or timeout occurred. No repeated derivation or authentication attempt occurred. The LS 2028 listener was ready before login, but no real 2028 arrived and none was sent. The complete live binary-send set was exactly 11501, 11505, 2000, 2012, and 4, once each. Their serialized fields follow the current functions 10060, 10056, 9877, and 10564 documented in [R18].

| Connection | Binary chunks / decoded frames | Queued unmatched frames at closure |
| --- | ---: | ---: |
| FirstLogin | 2 / 3 | 1 |
| LoginServer | 4 / 4 | 1 |
| GameServer | 6 / 21 | 17 |

All decoders ended with zero incomplete suffix bytes. Extra frames remained in memory without being mistaken for awaited replies. All observed GS frames had Aux zero. Thus nonzero grouping and fragmented-frame behavior have synthetic proof, not live exercise in this run. The observed raw GS baseline payload lengths were 679=69, 230=10, and 233=278 bytes. The full stats minimum remained 251 bytes. Other received packets were not interpreted as complete inventory or action readiness.

## Private consistency checks

Only presence, widths, and consistency are published. No actual identifier, name, position, zone, resource value, ticket, token, password, normalized input, derived key, or raw authentication payload appears here.

| Checked state | Live result and source interpretation |
| --- | --- |
| Persistent selection identity | U64 present; returned ticket UID equaled the selected UID |
| Profile identity | CP874 name present; startup profile equaled the selected character name |
| Own actor | U16 from 230@4 present; kept separate from the persistent U64 UID domain |
| Position | U16 pair from 230@6 and @8 present |
| Zone | U16 from 233@219 present |
| Current HP and MP | U32 at 233@235 and @239 present; no invented maximum values |

These offsets are supported by current ReadStartClock 12341, ReadStartZone 12343, ReadStartProfile 12344, and StartStats 11454-11456. [R18] Successful decoding establishes an observed baseline according to those readers. It does not establish subsequent state updates, a maximum-resource formula, or identity equivalence between actor and persistent UID.

## Cleanup and limits

FirstLogin closed after the world endpoint response. Final cleanup closed LoginServer and GameServer. Each of the three owned connections received one private gateway token. Each owned-token POST to `net/release` returned HTTP **204**, and all three WebSocket close events reported code **1000**, clean=true. No browser token storage or release-held list was read. The release implementation follows the current framework's same-origin token POST. [FRAMEWORK]

HTTP 204 and clean WebSocket closure are observed transport cleanup. They do not prove application logout, immediate account availability, server token expiry, or future reconnect success. No follow-up login was used to test those questions.

The finally path zeroed tracked mutable buffers and dropped credential references. Process exit releases immutable JavaScript strings and runtime-owned buffers; secure erasure of those objects is not claimed. A subsequent anchored `pgrep` found no probe process. There are no probe-owned background tasks, browsers, or sockets left running. The original private credential delivery remains outside Git and unchanged. No raw private capture was written.

The probe sent no movement, combat, digging, quest, purchase, inventory-changing, character-creation/deletion, security-change, takeover, resume, or ticket-replay command. Native action acceptance, long-lived liveness, byte-ACK exchange, reconnect, server logout, client-assisted/native ownership transfer, and complete clientless gameplay remain untested. The root owns subsequent MVP work and the planning map.

## Reproduction and publication evidence

The single live experiment is consumed. The following commands perform account-free checks only:

```sh
node --check /private/tmp/opentrickster-native-current.sba0qopn/probe.cjs
node /private/tmp/opentrickster-native-current.sba0qopn/probe.cjs --selftest
node /private/tmp/opentrickster-native-current.sba0qopn/simulate.cjs
```

| Outside-Git artifact | SHA-256 |
| --- | --- |
| `probe.cjs` | `74c4b18060b78b6dc71bc410b0ca891644d1bfb38df265b20ec2a78ce49e1b4c` |
| `simulate.cjs` | `ba5948cef3d0f0f593e4970123adb1bf91d18b8b8a7eacf2f644673d2185b3ba` |
| `sanitized-evidence.json` | `361af5c0fcf003572f964be66eb9bf191e03b03758ba9aeae9d595549fab59c7` |

The evidence records Node v26.10.0, one logical sequence, start/end UTC, stage counts and timings, schema/consistency booleans, and cleanup. Probe and evidence hashes bind the actual executed local files. This is disposable feasibility research, not a committed runtime adapter or application build.

Publication gates are staged and committed scans for credential plaintext, normalization, hashes, selected MD5 bytes in both endian orders, signed/unsigned decimals and encodings, plus compiled-field bytes. The root's existing `verify_report.py` also checks the committed report, one-file scope, and whitespace. Final delivery records the commit, GitHub branch-ref and report-byte readback, and one resolution comment on #17. Closure is justified by all bounded authentication and baseline gates passing. No PR or merge is requested.

[I17]: https://github.com/oDestroyeRo/opentrickster/issues/17
[R18]: https://github.com/oDestroyeRo/opentrickster/blob/df5fe0a5e8e45072845538fc74967a1757b6d9d7/docs/research/current-build-protocol-revalidation.md
[R17]: https://github.com/oDestroyeRo/opentrickster/blob/09a9dd7434d7bb56d6954220f4f6d942491bbb40/docs/research/native-startup-proof.md
[CFG]: https://tr-play.playall.in.th/spinel-config.json
[VER]: https://tr-play.playall.in.th/version
[PAGE]: https://tr-play.playall.in.th/
[LOADER]: https://tr-play.playall.in.th/Build/41e1cc21f8b1ab11275e55eb6949af3c.loader.js
[FRAMEWORK]: https://tr-play.playall.in.th/Build/7f982dae9f02da04f526a91e415474b4.framework.js.gz
[WASM]: https://tr-play.playall.in.th/Build/d9a4c4fd2737e740b82e1321839e352f.wasm.gz
[DATA]: https://tr-play.playall.in.th/Build/6135c4139b62e558bb4c755b266f5fa2.data.gz
