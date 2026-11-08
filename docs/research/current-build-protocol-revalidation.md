# Current build protocol revalidation

Issue [#18](https://github.com/oDestroyeRo/opentrickster/issues/18), account-free static research, observed **2026-11-08 UTC**. The current artifact set is build **8502ebbf2fc9033d2fba50d02fd92002**. It replaces `f5e972fa2be429602c20d6168013a20f` used by the prior protocol/authentication reports. The static contract question is answered for this pinned set; **native authentication, server acceptance, resumption, logout, and gameplay acceptance remain UNRUN**.

The native adapter/probe must update the build pin, asset names, compiled-field locators, public opcode seed, and both directional maps. Credential transformation, custom CP874, the six-byte envelope, Aux markers **1951/1952**, and the listed authentication/startup wire layouts are unchanged. Transport lifecycle behavior changed. The new `GmHello` hook consumes incoming capability information; it adds no authentication send or mandatory startup wait.

This research read public files as bytes and statically decoded WASM/IL2CPP structures. It did not instantiate target WASM, execute target JavaScript, open a game socket, authenticate, read credentials, or read private browser/session/capture state. All tools, downloaded/public assets and derived binaries remain outside Git. Only this report is committed. Entity, combat, digging, inventory-completeness and quest research are outside this ticket.

## Provenance and limits

Prepared checkout: `/Users/dos/.t3/worktrees/opentrickster/t3-84efc8cf`; actual starting branch `research/revalidate-protocol-contracts`; actual starting SHA `6b8162f1f7a42b2bd9dd02c1b086be360914bb8d`; starting tracked/untracked status clean. Publication branch: `research/current-build-protocol-revalidation`. The authenticated author's GitHub noreply identity is used. No map, other ticket, production file, PR, or main-branch change is part of this work.

Read the checkout's AGENTS.md and its issue-tracker, triage-labels and domain pointers before research/tracker operations. Applied mattpocock research from the locally installed 1.3.2 research skill and the installed pstack 0.9.133 Prove It Works skill. The user's explicit single-worker instruction takes precedence over skill suggestions to delegate.

Prior reports were comparison leads, not current compatibility evidence. Their local bytes were checked against these immutable Git blobs:

- [Protocol/framing](https://github.com/oDestroyeRo/opentrickster/blob/331f397fe2d6440bc24b9ee51f9b7caa6100962b/docs/research/target-protocol-framing.md).
- [Authentication/handoff](https://github.com/oDestroyeRo/opentrickster/blob/97d6bf407a23bf87d054b40571d14224d0fb32ac/docs/research/target-authentication-handoff.md).
- [State layout, Aux/startup subset](https://github.com/oDestroyeRo/opentrickster/blob/3a6c421d903483f1c977f6006fe01476d8da3223/docs/research/target-state-message-layouts.md).
- [Native startup preflight](https://github.com/oDestroyeRo/opentrickster/blob/09a9dd7434d7bb56d6954220f4f6d942491bbb40/docs/research/native-startup-proof.md): stopped before account/socket work on deployment drift.
- [Current state completion](https://github.com/oDestroyeRo/opentrickster/blob/cb587e235b113aa26b04db018c31f1e26510f5bd/docs/research/target-state-completion.md), report SHA-256 `1a4672402c0ca8c7e20d2d5b34fc9f0c8d82e063d178374acb970fd4da1b9579`: provided the GameSession/GmHello lead. This report independently joined and inspected that startup hook without repeating its gameplay research.

Evidence classes below: **source** means current public binary/source inspection, **synthetic** means independent code exercised invented fixtures, and **live** means target server behavior. Only the first two were performed. A synthetic test demonstrates an implementable interpretation; it does not establish server policy.

## Public deployment pin

Cookie-free GETs of [config](https://tr-play.playall.in.th/spinel-config.json), [version](https://tr-play.playall.in.th/version), and [page](https://tr-play.playall.in.th/) returned HTTP 200 at 16:59:37–38 UTC on 2026-11-08. Config and version both identify the build above. Config publishes firstLogin host `45.150.128.227`, port `9980`. The fresh page references all four approved cached assets below. Its HTML bytes changed since the earlier #17 preflight; its asset references did not. This is an observed coherent artifact set, not a guarantee about a subsequent deployment. On any future mismatch, stop before credentials/sockets and record the new pin; do not silently mix builds.

Sizes and hashes below are **decoded bytes**, including files whose names end in `.gz`. The approved cache already contained decoded content. Validate magic (`\0asm`, `UnityWebData1.0\0`) rather than decompressing solely by filename. Fresh public requests here fetched only config/version/page; cached asset hashes and page references supply asset coherence, not an assertion of a fresh download of every asset.

| Public artifact | Decoded bytes | SHA-256 |
|---|---:|---|
| [spinel-config.json](https://tr-play.playall.in.th/spinel-config.json) | 124 | `084900013e6edde0ce0eca286dc7ac884a86da2ece1bae361ce0671852092602` |
| [version](https://tr-play.playall.in.th/version) | 44 | `9ce11c71ef039ffbe6bede54dfcf5b27afb887d44b2a79f57f959638e2e771b0` |
| [index.public.html](https://tr-play.playall.in.th/) | 37950 | `7690892df54224550298f836ff9950aa77d285fec2d661a14748b2b35c6ccb7b` |
| [41e1cc21f8b1ab11275e55eb6949af3c.loader.js](https://tr-play.playall.in.th/Build/41e1cc21f8b1ab11275e55eb6949af3c.loader.js) | 27914 | `5a4ba6acdfc6ad420a285cc1faa54dc694892655bedd752e0f3f7dde3bae6a41` |
| [7f982dae9f02da04f526a91e415474b4.framework.js.gz](https://tr-play.playall.in.th/Build/7f982dae9f02da04f526a91e415474b4.framework.js.gz) | 391375 | `d159059282768082bbfc54432ab93ad68f6eaa57f84883c9e09780810597d917` |
| [d9a4c4fd2737e740b82e1321839e352f.wasm.gz](https://tr-play.playall.in.th/Build/d9a4c4fd2737e740b82e1321839e352f.wasm.gz) | 43136343 | `381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404` |
| [6135c4139b62e558bb4c755b266f5fa2.data.gz](https://tr-play.playall.in.th/Build/6135c4139b62e558bb4c755b266f5fa2.data.gz) | 27724487 | `fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474` |

Parse the UnityWebData directory, rather than copying a previous offset: verify its 16-byte magic; read U32LE directory-end at 16; walk from 20 through records `(U32LE offset, U32LE length, U32LE UTF8-name-length, name)`. The exact `Il2CppData/Metadata/global-metadata.dat` entry is **offset 16748911, length 9343896**, SHA-256 **`7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade`**. It matches the approved standalone metadata cache. Validate every extent against the data length and the directory cursor against directory-end.

## Migration decisions

| Surface | Result compared with old pinned build | Adapter/probe action |
|---|---|---|
| Public build, filenames and raw artifacts | Changed | Replace the complete pin; reject partial drift before login |
| Opcode seed and all four map fingerprints | Changed | Regenerate from current public seed; never reuse old wire opcodes |
| Metadata methods, module pointers, bodies and compiled-field positions | Relocated/recompiled | Use current joins below; method names or metadata version alone do not prove compatibility |
| Marker and four client-version bytes | Same bytes by hash; new source-backed locations | Extract only after full artifact/locator/hash checks; retain as private runtime bytes |
| Primary credential transform and CP874 | Same | Preserve exact normalization, signedness and string widths |
| Six-byte frame and Aux grouping | Same | Keep stream buffering and separate raw waiter/session dispatch layers |
| FL, world, LS, existing-character selection and GS startup | Same listed layouts and ordering | Register all waits before sends; do not declare ready on GS opcode 5 alone |
| Gateway liveness | Changed | Follow heard-at/probe/page-visible logic below; avoid old unconditional timeout |
| Gateway busy classification | New explicit Busy result | Stop with a bounded busy result; no takeover or release-held retry |
| Browser release/abort bookkeeping | Changed | Preserve ownership and cleanup rules; do not import browser tokens |
| GameSession bootstrap | GmHello added; fields shifted | Accept/route optional capability listener without adding a ready requirement |

A future native run should fail closed on an artifact/hash mismatch, malformed or oversized frame, unrecognized security/busy state, refusal, timeout, or incomplete startup. No automatic second-password setup/change, occupied-account eviction, release-held operation, or repeated authentication is justified by this static evidence. This is an implementation stop policy; it is not a claim that every such situation is refused by the server.

## Current source joins: how to reproduce them

Addresses marked **memory** are reconstructed WASM linear-memory addresses. **Metadata offsets** are relative to the extracted metadata file. **Body/instruction offsets** are relative to the original decoded WASM file. Framework offsets are byte offsets in its decoded UTF-8 file. Do not substitute one address space for another.

The metadata starts with magic `0xfab11baf`, format 108. Header entries are `(offset, byte-size, count)` triples at `8 + 12*section-index`. Current records: methods **30 bytes**, type definitions **66**, fields **10**, parameters **10**, images **54**. There are **15,440 types**, **88,892 methods**, **490 WASM imported functions**, and **118,769 element slots**. String-literal rows are **4-byte adjacent data offsets**, with a sentinel; treating them as older 8-byte length/index rows gives false locations. Header section numbers used here: 0 literals, 1 literal-data, 2 names, 5 methods, 7 field defaults, 8 default-data, 19 types, 21 images, 23 field references.

Reconstruction, independently implemented in Python and JavaScript:

1. Parse the WASM sections, ULEB counts and active `i32.const ... end` element/data segments. Copy data bytes to their declared memory positions; retain element slot → function-index and code-section function-index → original body start/size. The body range includes its local-declaration vector and final `end`, but excludes the preceding ULEB body length. Account for all 490 imported functions.
2. For each relevant metadata image, find its NUL-terminated image name in reconstructed memory, then scan pointer references to that address. A current code-generation-module prefix is `(namePointer, methodPointerCount, methodPointers)`, three U32LE words. Validate count against that image's maximum method token RID and every pointer-table element against the actual element table. Require a unique candidate, rather than guessing an old module address.
3. Read a method's name index at row+0, declaring-type index U16 at +4, and token U32 at +18. Type name/namespace indexes are +0/+4. For the owning image, index module.methodPointers by `(token & 0xffffff)-1`; that memory word is an **element slot**, not a body address. Resolve the slot through the element section and the resulting function through the code section. Check original body length/hash and the pivotal instruction offsets below. Shared generic/runtime functions make name-only matching insufficient.
4. Reconstruct metadata registration at **memory `0x1a4e60`**. Its fourteen U32 words are `[0x36bf,0x1012a0,0x2569,0xae9c0,0xa73e,0xd75a0,0x3c50,0x35d850,0x3c50,0x36c990,0,0,0x67,0x37bad0]`. The type-pointer array is `0xd75a0` (42,814 entries), field-offset pointer array `0x35d850` (15,440). Validate each metadata type's byval index (U16 at type-row+8) through the type-pointer array; dereferencing its type-data pointer must recover that same type index. All 15,440 were checked. Object field offsets then come from `fieldOffsets[typeIndex][fieldOrdinal]`, signed I32LE.
5. For a compiled string load, read its usage cell; require usage kind `raw >> 29 == 4`, derive `literalIndex=(raw & 0x1fffffff)>>1`. The consecutive literal offsets delimit bytes in literal-data. Current literal table starts at 500; literal-data at 106648. For a compiled array require kind 6, resolve a 6-byte fieldRef row `(U16 runtimeTypeIndex, U32 fieldOrdinal)`, runtime type → declaring type via registration, and that type's fieldStart U32 at +20. Match the resulting field index in the 10-byte default rows `(U32 fieldIndex,U16 typeIndex,U32 dataIndex)` and add the default-data base. Never infer an array's length from unrelated adjacent fields: use the actual construction/copy instructions and its expected hash.

The parser layout was checked against pinned [Cpp2IL/LibCpp2IL](https://github.com/SamboyCoding/Cpp2IL/tree/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL), including metadata usage, module/registration, default value and string-literal definitions. That source explains structures; the current bytes and successful joins establish these particular values.

| Current image | Module address (memory) | Count | Method-pointer array (memory) |
|---|---:|---:|---:|
| Spinel.Net.dll | `0x212548` | 4791 | `0x392c70` |
| Spinel.Formats.dll | `0x21214c` | 324 | `0x37f050` |
| Spinel.Engine.Unity.dll | `0x212120` | 1934 | `0x37cf10` |
| Spinel.Game.dll | `0x21251c` | 18796 | `0x37f5a0` |
| Spinel.UI.Pc.dll | `0x2125a0` | 16606 | `0x3a9600` |
| Spinel.UI.Custom.dll | `0x212574` | 17533 | `0x3979f0` |

Static instruction inspection used [WABT 1.0.39](https://github.com/WebAssembly/wabt/releases/tag/1.0.39), installed npm integrity `sha512-ba+dRL/75VQQY7RkU/CgriGbkoWAfS8TDyUlJfJhJ8KhtXgMl5dhNvoPNUcQ9IWRhW8u41glMSuZeTvsYq2rRg==`. To avoid another full huge disassembly, a scratch copy preserved selected original bodies and all indices, replacing other bodies with unreachable stubs. Objdump output offsets were translated back by each body's relocation delta. Original-body hashes were independently checked against the untouched artifact; no result below depends on executing the scratch module. Full untouched `wasm-validate --enable-all` passed.

## Compiled field migration manifest

Only locators/lengths/hashes are published for the marker and client-version bytes. Their actual bytes are intentionally absent. These are not the public gateway build or opcode seed. Old offsets `0x72ad8` and `0x4cb7a0` must not survive in a probe.

| Field | Usage cell (memory) | Resolved index | Metadata offset | Length | SHA-256 |
|---|---:|---|---:|---:|---|
| build | `0x344aa0` | literal 1079 | `0x1f1a9` | 32 | `7cd4e6cdb7707127e907a239277913438058d9327d1a184c7bde25e0a4c79b7a` |
| seed | `0x3551dc` | literal 17930 | `0x6e72d` | 44 | `bc25174f10ae444884c9b36c53dd15c38415f007fd04e8c8a0757a766cecb838` |
| c2s | `0x35637c` | literal 19058 | `0x72a7a` | 3 | `99e0ee5a83b3906160f1eab2e4007117fccc4a26652dc94955a3c6628aab4926` |
| s2c | `0x35b3b4` | literal 24193 | `0x859ae` | 3 | `8e504f255032f8cc5db7a20678e49cd36df604ed4c189171109b8701c34739c4` |
| marker | `0x356f38` | literal 19809 | `0x7545b` | 5 | `b5a2c96250612366ea272ffac6d9744aaf4b45aacd96aa7cfcb931ee3b558259` |
| client_version | `0x3430c4` | fieldRef 249 → field 69623 | `0x4e7818` | 4 | `e4abdaa9e4fe688b531f6d5257aad881668ae7d84dd3e8fdf305cfcc0e29b9c6` |
| firstlogin_wait | `0x3430d0` | fieldRef 252 → field 69626 | `0x4e78c8` | 12 | `e85ae1df05cd9499a0e332f696bfb39e5484807b31ae52775a1048a1b9672862` |
| selection_wait | `0x3430e4` | fieldRef 257 → field 69631 | `0x4e7a10` | 12 | `fd61c7f1fde7eea7c0b75cb3f0600177bbee3dc76f4ea51cc5329224d2cac349` |
| md5_rotations | `0x3430a8` | fieldRef 242 → field 69616 | `0x4e7590` | 256 | `b89a927779b66394fd309fdda16496644a11ab469c5370e8e8a6a6ee25617e7f` |

The marker is loaded from SignIn MoveNext **10060**, instruction **`0x2f3023`**. Client version is initialized by Session `.cctor` **10050**, load **`0x2f1bb5`**, and written as raw four bytes by EnterWorld MoveNext **10056**. Gateway build/seed are joined through GatewayBuild `.cctor` **9938** (`0x2eaa8f`/`0x2eaa9c`) and Load **9940** (`0x2eaf8f`/`0x2eaf97`), and consumed by OpcodeMap.FromSeed **9939**. Wait arrays resolve to `[11502,11503,11508]` and `[2014,2013,2]` as three U32LE integers. Marker/client-version hashes match the old report; the current field locations were recovered through these joins, not reused by offset.

### Opcode map algorithm and fingerprints

Current public Base64 seed: **`Yi9ApMr88JAXsVztVJYSxdzPbba7CrrOlJ0GmnNHLXI=`**. Decode to 32 bytes. The old seed is incompatible. The algorithm is unchanged:

- Hash decoded seed followed by ASCII `c2s` or `s2c` with SHA-256; initialize `(a,b,c,d)` from digest bytes 0..15 as four U32LE values.
- Start `wire[i]=i` for all 65,536 entries. For `i=65535` down to 1, compute `r = rotl32(b*5,7)*9`, wrapping each multiplication to U32; `j = unsigned(r) % (i+1)`; swap `wire[i],wire[j]`.
- From the old state, set `(a,b,c,d) = (a^b^d, b^a^c, c^a^(b<<9), rotl32(b^d,11))`, wrapping to U32. This is simultaneous assignment.
- Set `inverse[wire[i]]=i`. Serialize each table as 65,536 U16LE entries before hashing. Outgoing uses the **c2s wire** table; incoming uses the **s2c inverse** table.

FromSeed **9939** proves rotation/modulo/state update, including original instructions `0x2eae73`, `0x2eae7c`, `0x2eaeae`. Both independent implementations verified every inverse entry, not just a few opcodes.

| Table | SHA-256 of 131072 bytes |
|---|---|
| c2s wire | `db98aa5f8e2df90f4d4baa756a9fad7849f6d307e2c155f5b6b5c9ba58c11fe1` |
| c2s inverse | `9e374006761f9bf18e3a59b6b4914c86b673f22230d9bab238e4c56120e350e4` |
| s2c wire | `e0386023dd44e6cd0e70099542b5f9943969feff662b5797cb73e5edab72f58d` |
| s2c inverse | `c5d1b355f0c5cf75af90935158df8d6b7309e860710b760fb1f04207169c6b9c` |

A seed-independent porting fixture is seed bytes `00 01 ... 1f`: c2s wire SHA-256 `c87a5dc1ad4dc4657d8ad4ababf6bb39e83daddb12cf61c76e50e89f65e6edab`, s2c `20a1907c6cadf8a2cf4ac1c36f5dbf36b8a086d08958eec0071ffec8a0663a8a`. With that synthetic c2s map, logical `0x1234`, Aux 0, payload `aa bb` yields `020045390000aabb`. This is not a server packet to send.

## Gateway framing, text and lifecycle

**Unchanged envelope:** U16LE payload length, U16LE mapped opcode, U16LE Aux, then payload. Maximum payload **32745**; reject declared 32746 or greater. Encoding uses Aux zero. There is no additional frame encryption/checksum in this path. GatewayFrames.Encode **9941** tests the limit at `0x2eaffd`, writes mapped opcode at `0x2eb04d`; Push **9943** reads/rejects length at `0x2eb1ce`/`0x2eb1d3`, reads wire opcode at `0x2eb1e8`, Aux at `0x2eb209`, and retains an incomplete suffix at `0x2eb270`. A WebSocket binary message is a stream chunk: preserve partial headers/payloads, consume coalesced frames in order, and keep counters in **bytes including the six-byte headers**. Connection constructor **9911** selects the gateway codec for this path; do not substitute the other generic frame codec.

**Aux dispatch unchanged:** raw Connection.Receive **9920** satisfies connection waiters independently of the GameServerLink session grouping in Receive **10560**. Aux 0 is delivered immediately or appended to its ordered startup backlog. Nonzero ordinary packets accumulate by Aux. Logical **1951** removes that group, flushes the accumulated packets in order and delivers the marker. Logical **1952** replaces that group's queue with an empty one, flushes its prior contents and consumes the marker. Preserve multiple independent groups. Proof: `0x30f666` subtracts 1951 and bounds the marker pair, `0x30f67a` selects removal versus replacement, `0x30f8df` distinguishes marker delivery. The markers are not 1931/1932, and are not global inventory-ready signals.

**Bridge URL unchanged:** at the root page, `wss://tr-play.playall.in.th/net`. New connection uses `?host=<escaped-host>&port=<port>&b=<public-build>&r=1&u=1`; resume uses `?resume=1&u=1&b=<public-build>`. Source: WebSocketTransport.BridgeUrl **9960**, GatewayLink.Connect **9959** (`0x2ebb76` appends flags), StartAttempt **9971**, Opened **9975**. Framework derives scheme/host/page folder; retain that derivation if the page is relocated. Open delivers binary ArrayBuffer separately from text and guards against callbacks from replaced sockets.

Text controls, as interpreted by current GatewayLink.Text **9977**:

| Direction | Form | Effect |
|---|---|---|
| S→C | `k <token> <holdSeconds>` | Exactly three fields; token retained privately; invariant Double with NumberStyles.None seconds ×1000 |
| S→C | `a <gatewayReceived>` | Exactly two fields; invariant Int64 with NumberStyles.None cumulative acknowledgement |
| C→S | `a <received>` | Cumulative inbound byte acknowledgement |
| C→S | `p` | Text liveness probe |
| C→S | `r <token> <received>` | Resume request after opening a resume attempt |
| S→C | `r <gatewayReceived> <newToken>` | Valid only while resuming; confirm and rotate token, then replay retained suffix |
| S→C | `g <reason>` | Invalidate token; `server` is handled as Away |
| S→C | `v <build>` | Exactly two fields; offer public build update |

Unknown or malformed controls fail the relevant transport path. Do not log control token values. Confirm **9978** ignores a stale ordinary acknowledgement below acked; a value above sent is invalid. It drops fully acknowledged queued bytes and advances a partial head offset. Resumed **9979** instead requires `acked <= n <= sent`, including rejection of a backward resume acknowledgement, then sends the remaining exact suffix. It must not serialize the frames again or reset the opcode map. Current counter/queue field offsets: received +72, sent +80, acked +88, ackSent +96, retained +104, headOffset +108. Retention cap is **1048576 bytes** in Send **9962**. Failure to receive a token within 5000 ms disables resumption/clears retained state and advances acked to sent; it is not evidence that the game login failed.

**Changed liveness:** current Binary **9976** and Text **9977** update `heardAt` (+168). Tick **9967** calls the new Ask **9969** after 5000 ms without hearing traffic; Ask sends `p`. Loss while a probe is pending requires page-visible state, at least two probe ticks, and 5000 ms elapsed. Source: visibility `0x2ec8cf`, tick count `0x2ec8f0`, threshold `0x2ec8fa`, heard-at test `0x2ec983`–`0x2ec99f`. ACK scheduling still uses 1000 ms when received changed and 10000 ms otherwise (`0x2ec9af`, `0x2ec9ca`). A Rust adapter needs a deliberate foreground/visibility policy; blindly preserving the old unconditional five-second timeout changes current behavior. Timers are client logic, not a recovered server SLA.

**Changed busy result:** GatewayLink.IsBusy **9951** is now consulted by LoginFailure.Of **10004**. A transport error containing public text `gateway busy` maps to new LoginProblem.Busy **8**, formerly a generic unreachable result. The check and classification occur at `0x2ef239` and `0x2ef257`. Treat it as a bounded stop; do not spin authentication retries.

**Release/abort:** framework `spinelSocket` starts at byte **116496**. `release(token,unloading)` uses same-origin `net/release`, POST body = raw token, `keepalive:true`; unload tries sendBeacon first. The current fetch rejection handler adds failed tokens to a deduplicated `inherited` list, unlike the old empty catch. `_SpinelLinkReleaseHeld` consumes that list. Current `_SpinelSocketInit` also guards page/tick callbacks with `dead` and hooks Module.onAbort to set dead, call leave with `game stopped`, and chain the previous handler. Do not read/import any browser token storage. A future native transport may release only tokens it owns; this source does not prove server release acknowledgement, deduplication, expiry, or immediate account availability.

| Current framework function | Decoded byte start |
|---|---:|
| `_SpinelLinkRelease` | 117561 |
| `_SpinelLinkReleaseHeld` | 117650 |
| `_SpinelLinkRemember` | 117826 |
| `_SpinelSocketBridgeUrl` | 124929 |
| `_SpinelSocketInit` | 125473 |
| `_SpinelSocketOpen` | 126546 |
| `_SpinelSocketSend` | 127742 |
| `_SpinelSocketSendText` | 127907 |

## Credential and string contract

UI LoginScreen's Submit local function **49172** trims the ID with String.Trim (`0x1054716`), trims only trailing ASCII space from the password (`0x105472b`), and calls LauncherCheck **49149** (`0x1054870`). The ID is at most 12 characters, printable ASCII `0x21..0x7e`, excluding comma, vertical bar and double quote; lowercase is used for reserved-name checks, not to rewrite the transmitted ID. Password validation permits at most 18 printable ASCII characters `0x20..0x7e`. Preserve leading/internal spaces. Do not publish the compiled reserved-name strings or other sourced account-like constants.

LoginKey.Derive **10012** independently enforces: take the first 18 UTF-16 code units, trim trailing ASCII U+0020, lowercase only ASCII A–Z, encode UTF-8 without BOM, MD5, select digest bytes **4 through 11 inclusive** and interpret those eight bytes as a **big-endian 64-bit bit pattern**. Source: substring `0x2efd43`, trim `0x2efd54`, ASCII conversion `0x2efda1`–`0x2efdb5`, MD5 call `0x2efe29`, byte span `0x2efe65`–`0x2efe89`, endian reversal `0x2efee1`–`0x2eff33`. FL writes that bit pattern as U64LE; LS uses the **signed I64 invariant decimal** string, via LoginKey.Text **10013**. U64 decimal for a high-bit-set key is incorrect. Rust should retain bits in u64 and use an explicit `as i64` bit interpretation for decimal serialization, not a fallible numeric conversion.

The span join was checked through the current generic metadata: usage `0x32e1ac` → methodRef16683 → method55163/inst463, ReadOnlySpan<byte> constructor; the apparent offset shift by 2 means four bytes and length is eight. MemoryMarshal.Read usage `0x335c3c` resolves UInt64 via generic inst286. The MD5 implementation **9810** uses the standard four IV words, 0x80 padding, bit-length suffix, four schedules, sine-derived constants, and little-endian digest output. Its 64 rotation words match the field hash above. [RFC 1321](https://www.rfc-editor.org/rfc/rfc1321) describes that primitive; the current WASM determines its credential use.

Wholly synthetic credential fixture: input `ExampleOnlyA1` → `exampleonlya1`; MD5 `07a3acf9d7cdb6ccf99588dce5636f15`; selected signed key **-2896457993852450596**; FL key bytes **`dc8895f9ccb6cdd7`**. No real user credential, ticket, token, account ID or character UID was used. Tests also cover ASCII case, 18-character ASCII truncation, preserved leading spaces, and signed boundaries 0, I64_MAX, I64_MIN and -1.

### Custom CP874

The current Cp874.BuildHigh **9605** contains the same 128 high-half code points in sixteen v128 stores. U16LE high-table SHA-256 is `208f421e2b1dfbda309916f8f25a1063390929acea2e31d2467b0a8be3a6757e`; whole 256-entry table SHA-256 is `32f78caea158a0dd47a634082c15619c1f57303152fc76dfff287575d88ee8c8`. ASCII bytes map identically. This table must be reproduced exactly; a platform's nominal CP874 codec can differ at undefined positions.

Rows below are Unicode hex code points for successive byte ranges 80–8f through f0–ff:

```text
80: 20ac 0081 0082 0083 0084 2026 0086 0087 0088 0089 008a 008b 008c 008d 008e 008f
90: 0090 2018 2019 201c 201d 2022 2013 2014 0098 0099 009a 009b 009c 009d 009e 009f
a0: 00a0 0e01 0e02 0e03 0e04 0e05 0e06 0e07 0e08 0e09 0e0a 0e0b 0e0c 0e0d 0e0e 0e0f
b0: 0e10 0e11 0e12 0e13 0e14 0e15 0e16 0e17 0e18 0e19 0e1a 0e1b 0e1c 0e1d 0e1e 0e1f
c0: 0e20 0e21 0e22 0e23 0e24 0e25 0e26 0e27 0e28 0e29 0e2a 0e2b 0e2c 0e2d 0e2e 0e2f
d0: 0e30 0e31 0e32 0e33 0e34 0e35 0e36 0e37 0e38 0e39 0e3a f8c1 f8c2 f8c3 f8c4 0e3f
e0: 0e40 0e41 0e42 0e43 0e44 0e45 0e46 0e47 0e48 0e49 0e4a 0e4b 0e4c 0e4d 0e4e 0e4f
f0: 0e50 0e51 0e52 0e53 0e54 0e55 0e56 0e57 0e58 0e59 0e5a 0e5b f8c5 f8c6 f8c7 f8c8
```

Unmappable input becomes `?`; a valid surrogate pair produces one fallback byte. Fixed-width `Chars(value,n)` requires encoded-byte length **strictly less than n**, emits bytes then zero pads all remaining width; exactly n is rejected. `Cstr(value)` emits encoded bytes and one NUL. Reader fixed strings consume n bytes while decoding only before the first NUL; Cstr requires a terminator. Current writers/readers **10027/9835/9843/9849** establish this. A Rust port can implement the input normalization over UTF-16 units explicitly and use this exact lookup/fallback table; do not rely on locale case conversion or lossy platform encoding.

## Authentication and existing-character startup

All opcodes in this section are **logical**, before mapping. All numeric wire fields are little-endian unless explicitly described as decimal text. `Raw(n)` consumes exactly n bytes; `Chars(n)` is the fixed-width custom CP874 field above. Preserve server-provided identity and ticket values in private runtime memory. Client waits are bounded timeouts, not ticket-expiry claims.

| Stage/opcode | Current payload/behavior | Source proof |
|---|---|---|
| FL C→S **11501** | `Chars(Id,19), Chars(compiledMarker,19), U64 key`: 46 bytes | SignIn MoveNext 10060: widths at `0x2f3011`, `0x2f302b`, key at `0x2f3048`, send `0x2f3098`; 15000 ms wait for 11502/11503/11508 |
| FL S→C **11503** | U32 error code | Same state machine; preserve first-login stage |
| FL S→C **11508** | `Chars(reason,256)`, then six U16: year, month, ignored, day, hour, minute | Same state machine; error code -1; timezone/meaning of ignored word not established |
| FL S→C **11502** | `Raw(12) ticket, U8 worldCount, worldCount × WorldEntry` | Same parser; ticket is opaque, not a printable credential |
| World C→S **11505** | U16 World followed by U16 Island: four bytes | EnterWorld MoveNext 10056, send `0x2f210b`; 15000 ms wait `[11506,11507]` packed at `0x2f2133` |
| World S→C **11507** | U32 error | EnterWorld MoveNext |
| World S→C **11506** | `Chars(host,16), U16 port`: 18 bytes | Close FL and open LS at returned endpoint |
| LS C→S **2000** | `Cstr(Id), Cstr(signedKeyDecimal), Raw(4) compiledClientVersion, Raw(12) ticket, U8(20)` | EnterWorld MoveNext, send `0x2f2823`; 15000 ms wait `[2002,2001]` packed at `0x2f284d` |
| LS S→C **2001** | U32 error; code 1067 additionally reads two Cstr details | Preserve stage/code and bounded sanitized detail handling |
| LS S→C **2002** | Character list below | Session.ParseCharacters 10048 |
| LS **2028** | Incoming listener installed before login; response is U32 low bits of floor(local milliseconds) | Session listener 10054 `0x2f1cef`, send `0x2f1d13`; registration `0x2f248f`; not an authentication key |
| Selection C→S **2012** | U64 selected existing-character UID | Select MoveNext 9877, send `0x2e5c99` |
| Selection S→C **2013** | U32 error | Same state machine |
| Selection S→C **2** | U16 ignored, U64 ignored, U32 error | GsError 9821; not opcode-3 success |
| Selection S→C **2014** | GS ticket below; then await logical 3 or 2 | Followup must be registered first at `0x2e5af8`; Followup callback 9926 captures `[3,2]` before processing further coalesced frames; outer wait `[2014,2013,2]`, 30000 ms |
| Selection S→C **3** | Success completes handoff after 2014 | Do not reconnect to GS on 2014 alone |
| GS C→S **4** | U64 UID, U16 infoIndex, U32 CRC: 14 bytes | Login MoveNext 10564 writes at `0x30fe82`, `0x30fea5`, `0x30fec8`, sends `0x30feee` |
| GS S→C **5 / 159** | Accepted / refusal; 159 throws ProtocolException in this path, not a recovered error-code layout | 20000 ms wait pair at `0x30ff3d` |
| GS startup **679 / 230 / 233** | Profile, clock/actor/position, stats/zone | Register all three before sending 4: `0x30fb62`, `0x30fba8`, `0x30fbee`; each 20000 ms; await all after acceptance |

Each WorldEntry is exactly **75 bytes**: U8 Active@0, U16 World@1, U16 Island@3, Chars Server(33)@5, Chars WorldName(33)@38, U16 MaxUsers@71, U16 Users@73. Do not swap World/Island because an older domain structure used different member offsets. Current runtime WorldEntry fields World+12 and Island+16 were independently checked.

The character list is U8 count, count × Raw(156) records, U32 SecondPasswordState, U32 SecondPasswordFails, Cstr SecondPasswordHint, two ignored U8 values, U8 UnlockedSlots, optional status trailer. Within each 156-byte record: U64 UID@0, U8 Slot@8, Chars Name(20)@9, U16 Type@29, U16 Level@43. Keep remaining bytes opaque rather than inventing resource fields. Current ReadCharacter **9838** and ParseCharacters **10048** prove these widths. Actor IDs seen later are not interchangeable with this UID.

The optional status trailer is U8 version, U8 count, count × `(U8 slot,U8 kind,U32 value)`. CharacterStatusWire.Read **10037** returns null if fewer than two bytes remain, version is not 1, or fewer than count×6 bytes remain after the prefix; the latter cases already consumed the two-byte prefix. Only kinds 1/2 are retained as OfflineBot/OfflineShop. Unknown/truncated status is not proof of an idle character. Preserve busy/security state and stop for an occupied/busy character instead of inferring permission to displace it.

Optional security is not part of the primary password transform. Current selection still sends UID only; it does not silently construct a lock hash. UI Enter **38184** branches on SecondPasswordState==0 to an information view. Later protected-operation policy is separate: SecondPasswordGate.NeedsLock **11238** requires state 3, a positive operation flag and a configured flag intersection; SendWithLock **11239** can wrap with opcode 927. Startup here does not require that packet. Preserve state/fails/hint and stop on unsupported required security; do not set/reset a secondary password or treat unknown state as unlocked. No account-specific security setting was observed.

GS ticket **2014**: `U64 Uid, U16 InfoIndex, U32 Crc, Cstr Host, U32 Port, seven U16 RestrictionMinutes`. Key reads in Select MoveNext are `0x2e5ef9`, `0x2e5f0c`, `0x2e5f1f`, `0x2e5f33`, `0x2e5f46`, loop `0x2e5fad`. Runtime members occupy +8, +16, +24, +32, +40, +48; runtime alignment does not widen the U32 CRC/port on wire. No ticket expiry, single-use/replay guarantee, server signature, or stable endpoint guarantee was recovered. An unrelated opcode 5509 SystemServerTicket is not this GS ticket and must not replace it.

Current GameSession.Connect async state machine is **`<ConnectAsync>d__36`**, function **13324** (the old state-machine name has shifted). It opens GameServerLink at `0x3b1f12`, tracks at `0x3b2009`, logs in at `0x3b2029`, constructs GameSession at `0x3b211f`, and closes on failure at `0x3b21bc`. Login **10564** then assigns the combined startup state only after the awaited baselines. Preserve this ordering in both a Rust adapter and any disposable probe.

### Occupied account and refusal policy

Error **60013** still maps to LoginProblem.AlreadyLoggedIn **1**. LoginFailure.Of **10004** uses a checked switch for code−60003 whose current table at memory **1943852** is `[0,6,6,6,6,6,2,3,4,2,1]`; 60016 is handled separately as value 5. Thus 60003 → wrong credentials (0), 60009/60012 → blocked (2), 60010 → withdrawn (3), 60011 → no account (4), 60013 → occupied (1). LoginAttempt.Run **10006** retains a release callback/retry branch for AlreadyLoggedIn (`0x2ef8f1`, `0x2ef902`). That browser behavior does not authorize an unattended native takeover: **stop on 60013** and surface occupied state. Stop on Busy 8 as well. Ticket expiration and server-side guarantees remain unknown.

### Minimum startup profile and resources

The current wire readers match the prior startup layout, independently joined to current bodies:

| Input | Field/offset and minimum needed | Current source |
|---|---|---|
| 230 | 10 bytes: U32 day/clock@0, U16 actorId@4, U16 X@6, U16 Y@8 | ReadStartClock 12341 |
| 679 | Chars Name(20)@17, U16 bodyType@37; 39 bytes for these fields; 69 bytes for the status-profile path | ReadStartProfile 12344; CharacterStatus ApplyStart 11470 |
| 233 | Twelve U16 stat words@184..207, U16 Bonus@208, U16 TmLevel@212, U16 Level@214 | StatBlock 11454, StatusRead 11457, ApplyStart 11470 |
| 233 | U16 Zone@219 | ReadStartZone 12343, instruction `0x374c8d` |
| 233 | U32 Galder@231, HP@235, MP@239, TmExp@243, Exp@247 | StartStatsParse 11455; full StartStats 11456 requires **251 bytes**, guard `0x34016b` |

The smaller status path's ≥217-byte guard does not authorize the full 251-byte StartStats reader. TmExp needs its own ≥247-byte extent. Missing resource bytes are unknown, not authoritative zero. ReadStartBody **12345** defaults to 679@37; if 233 has at least 274 bytes and byte273 is 2 or 3, it selects U16 at 679@53 or @56 respectively. Preserve those bounds. This research does not claim that a maximum-HP/MP formula or every 679/233 field was recovered. Current HP/MP values above are wire values; do not manufacture maxima. U16 actor identity in 230 and U64 selected-character UID are separate domains.

### New GmHello bootstrap hook

Current GameSession constructor **13257** calls **GmHello.Of 13258 at `0x3add4a`**, then sets recordingStartSet at +70 (`0x3add50`/`0x3add58`), invokes BindGameServer **13259 at `0x3add61`**, and clears recording (`0x3add68`). `Me` moved to +96 and `Late` to +104; independently resolved current field-offset rows confirm these changes.

Of uses a ConditionalWeakTable factory; usage cell `0x33d14c` has kind 2 and resolves current metadata method72257. Its nested factory **13397** calls GmHello constructor **13392 at `0x3b6091`**. That constructor enumerates IGameSession.get_StartSet (interface slot **1**, metadata method72735), looks for logical **32597** (`0x3b5c16`), calls the incoming ParseHello **13387** (`0x3b5c53`), and subscribes with IGameSession.OnPacket (slot **2**, method72736) for the same opcode (`0x3b5e23`/`0x3b5e8e`). Its callback **13395** calls Take **13393**, which parses the incoming payload at `0x3b5f08`, updates Info/Available and invokes Changed. IGameSession.Send is a distinct slot **3**, method72737; this hook does not call it. The factory and callback bodies were separately hash-checked.

This is an incoming capability-information hook installed before the startup backlog replay. It is not a new outbound authentication handshake, a mandatory startup wait, a world-entered guarantee, or an inventory-complete marker. Do not gate the minimal 5+679+230+233 baseline on receiving it, and do not infer privileges or implement related actions from its name. Broader StartSet/inventory readiness remains covered by #14's separate report and future live evidence.

## Current wire examples and synthetic acceptance checks

Values below are invented fixtures, never captured account data. They exercise the **current** maps. They do not include the real compiled marker/version or valid server tickets.

| Direction/logical opcode | Complete synthetic frame, or header and SHA-256 |
|---|---|
| c2s / 4 (14 payload bytes) | `0e00be69000008070605040302013412efcdab89` |
| s2c / 230 (10 payload bytes) | `0a00e7d5000040e2010041016400c800` |
| s2c / 679 (69 payload bytes) | header `450007490000`; SHA-256 `b4d1e6fbdbc09b1bc7d99b6b6cb6d8520f3e99f52fdf92698dad6a38fc6e2359` |
| s2c / 233 (251 payload bytes) | header `fb0077520000`; SHA-256 `7c0c081feac7cd5cc96d34c3ebb9c80378319f92d40d0f5e6208d65bc5950940` |

GS4 fixture uses UID `0x0102030405060708`, infoIndex `0x1234`, CRC `0x89abcdef`. Clock fixture is `(123456,321,100,200)`. Profile fixture is 69 zero bytes except `DemoHero` fixed/padded into bytes17..36 and U16 body7@37. Stats fixture is 251 zero bytes except twelve U16 values0..11@184, U16 `(208,4),(212,8),(214,42),(219,9)`, U32 `(231,500),(235,900),(239,600),(243,700),(247,800)`. These definitions reconstruct the longer frame hashes without embedding a long zero-filled dump.

Passed synthetic boundaries:

- Both directional maps and inverses, including all 65,536 reverse lookups; current versus synthetic-seed fingerprints; short seed rejected.
- Empty and maximum-32745 payloads, 32746 encode/declaration rejection, every split of a small two-frame stream and byte-at-a-time delivery, coalesced frames and retained suffix.
- Independent Aux groups, Aux-zero immediate delivery, 1951 removal/delivery and 1952 reset/consumption.
- Registering the 2014 followup before coalesced 2014+3 delivery; partial-head acknowledgements, stale ordinary ACK handling, out-of-range ACK rejection and backward resume rejection.
- Signed I64 boundary decimal/little-endian conversion; normalization/MD5 fixture; every CP874 byte round-trip and unmappable/surrogate fallback; fixed fields of 16/19/20/33/256 bytes reject exact-capacity content and truncated inputs; Cstr termination.
- World-entry 75-byte shape, FL 46-byte layout, LS raw-version/ticket placement, GS 14-byte shape, every truncated synthetic GS ticket, startup clock shorter than10 and full stats shorter than251 rejected. FL/LS fixture fields were invented, not extracted compiled marker/version bytes.

These tests are model checks against recovered contracts. They did not invoke the target client or connect to the target service. Raw-source verification separately checked the artifact hashes, directory, compiled tables, method-body joins and instruction locations; neither class substitutes for the other.

## Bounded reproduction and handoff

Scratch workspace: `/private/tmp/opentrickster-issue18.nity1j`. Approved current cache, read only: `/private/tmp/opentrickster-native17.0h1SYk`, restricted to the seven named public artifacts plus approved metadata. Reproduction scripts have no credential loading or socket code. Inspect source, then run:

```sh
node /private/tmp/opentrickster-issue18.nity1j/verify-current.cjs /private/tmp/opentrickster-native17.0h1SYk
node /private/tmp/opentrickster-issue18.nity1j/verify-codec-current.cjs
python3 /private/tmp/opentrickster-issue18.nity1j/verify-source.py
python3 /private/tmp/opentrickster-issue18.nity1j/synthetic-current.py
/tmp/opentrickster-issue13/tools/node_modules/wabt/bin/wasm-validate --enable-all /private/tmp/opentrickster-native17.0h1SYk/d9a4c4fd2737e740b82e1321839e352f.wasm.gz
```

Observed results: independent JS raw parser passed **five rediscovered modules, 75 metadata→module→element→original-body joins, all 15,440 byval-type joins, compiled-field fingerprints and wait arrays**; source verifier passed **128 CP874 entries, MD5 rotations, two wait arrays, 34 object-field offsets, 41 original instruction locations, independent Python maps/inverses and startup frame hashes**. Both synthetic suites and full WASM validation exited zero. Counts describe actual checked rows/locations, not a claim of whole-client equivalence. During verifier development, mismatched guessed backing-field names/instruction text failed assertions and were corrected against the actual source/registration; the final source verifier is strict and passes.

`parse.py`, `scoped.py`, `read.py` reconstruct and annotate only public artifacts. `migration-manifest.json` is a sanitized machine-readable handoff with pins, fields, maps, module pointers, selected method joins, object offsets, framework locators and synthetic fixtures. These tools/data remain outside Git; their fingerprints below bind the exact local evidence. The report's algorithms/tables and primary pins remain usable if the temporary files are unavailable.

| Outside-Git evidence file | SHA-256 |
|---|---|
| `public.py` | `3ff4c91f177fc4a7578db437c299853a703a346e6e47ca6df2de07f49764498b` |
| `parse.py` | `bfc1537740753bc4f40e201bf221162c438208c5613d4fd96e83a5c9b63540b9` |
| `scoped.py` | `ebf0d3e8efcac6cdc5ecf01179c16fc8a51e054688115d23c3d5cfa13a300fd5` |
| `read.py` | `1a184c68693c87f1c85233513732e42d30e6c9f93c2543e0f7dc8ecbeb429be0` |
| `verify-current.cjs` | `e4326628c61bbe9051e6693374cb8f452f46c412490ea5aa9eb62c0ade22fc00` |
| `verify-codec-current.cjs` | `0394bfbe59c50e6e0aa24e88ad4876b2fbb51544e078ffa868272cf5099754cf` |
| `verify-source.py` | `730a5af02ab3426531497630b83cc0fc50d57eef18729c9d28485cc982b56734` |
| `synthetic-current.py` | `f1210e2a1a267c35992f27a32688d59260318e81696f286a0b746d6fba470b1a` |
| `migration-manifest.json` | `f258e7ac8a99566adfd4019020a03cfc1d61e56a66d101ddbd0319a6bf756b84` |
| `joins-selected.json` | `43d70dee98aca5fc8e61f7d3658bb39ae13105b51d9d0450eb75bac2bb273bb4` |
| `field-offsets.json` | `8a6e2c79d26255fdd57543a71b32909c1be89d314d1c5c95f5a0203e0dc0ec19` |
| `synthetic-frames.json` | `34914ff380a51d83039d7af764455f69778fa1f5fec09e7a54f30a0964db97d8` |

For a future authorized native attempt: independently verify a fresh public build and all used compiled inputs; update the outside-Git probe/production adapter from this manifest; rerun its own account-free tests; then have the root review the evidence and serialize the already authorized live login. Preserve raw-order waits and Aux-order dispatch, stop on occupied/busy/security/refusal/timeout, and require all baseline fields before declaring a usable session. Actual login, selected-character acceptance, ticket lifetime, replay/resumption, clean server logout/account availability, and complete inventory/action readiness remain empirical acceptance work. No native success is claimed by closing the static revalidation question.

The worktree is intended to contain this report only; scratch artifacts are retained for the root's independent review. No background waits, watchers, live sessions or owned sockets were created. The publication/comment readback records the final report commit, remote ref and digest without requiring a self-referential commit hash inside this file.

## Exact current method/body manifest

All rows below were verified by the independent raw-artifact JavaScript parser. Image codes: **N** Spinel.Net.dll, **F** Spinel.Formats.dll, **U** Spinel.UI.Pc.dll, **E** Spinel.Engine.Unity.dll, **G** Spinel.Game.dll. `Index` is the metadata method row, token is the image-local token, slot is the current WASM element slot, and start/bytes/hash refer to the **original** body. Nested compiler-generated names must be interpreted with the row/token/image; some short names repeat. This table is a locator manifest, not a substitute for the field/order/control-flow evidence above.

| Image / method | Index | Token | Slot → function | Body start / bytes | Body SHA-256 |
|---|---:|---|---|---|---|
| F `Cp874.BuildHigh` | 84853 | `0x6000018` | 4476 → 9605 | `0x2ca806` / 444 | `a4287710d1b0fa0a23b277e10b4b69b7d2e6df225ce008b639c36dc73d2f3fc5` |
| F `Cp874.Encode` | 84856 | `0x600001b` | 4479 → 9608 | `0x2cabdb` / 652 | `4d76754178f988e19b8415ca79456c140509c3318869306b4ec3c26878dd7624` |
| F `Cp874.Decode` | 84858 | `0x600001d` | 4481 → 9610 | `0x2caeb5` / 213 | `3b3e89b10155e2950a7e93afa451193e0d0f37c1fd6cb0ef934ebe6b5865c765` |
| N `Md5.Hash` | 70726 | `0x60000f9` | 23864 → 9810 | `0x2e0d7e` / 1344 | `82d18c18f4a7a92bc441e1f3f4f5e1a71a6f1e87f3a494db4fa17764383b175b` |
| N `Characters.GsError` | 70506 | `0x600001d` | 23697 → 9821 | `0x2e191a` / 108 | `604391db37cd1012b9f36f391b4e2fa7ce2cdc5589a0e12d72f1ba07649c17ef` |
| N `PacketWriter.Cstr` | 70742 | `0x6000109` | 23880 → 9835 | `0x2e28e4` / 284 | `41ee626a33b8fa38f745d92be54129e6b078e4c105b4e90cc7098224a729654e` |
| N `Session.ReadCharacter` | 70788 | `0x6000137` | 23926 → 9838 | `0x2e2e91` / 382 | `750eb850b61eb4d43d1994653429ef175d1a383e65e1e75643f19676ece288c4` |
| N `PacketReader.Chars` | 70759 | `0x600011a` | 23897 → 9843 | `0x2e316e` / 264 | `1772161d0753d3962197dc29925a0a5eb8ee562f3da25d8fdd3150fd4d76146c` |
| N `PacketReader.Cstr` | 70758 | `0x6000119` | 23896 → 9849 | `0x2e3e58` / 270 | `6ea9cb766ae0aa0a5f5fc5339b0ed03c509a5fb119cf99355eccd92cdd1f891d` |
| N `<SelectCharacterAsync>d__14.MoveNext` | 70533 | `0x6000038` | 23724 → 9877 | `0x2e59e6` / 2689 | `56a6752c78d1012d6bb59cc16a4124024a9fb4c833ffa4a85e2b376e996032f0` |
| N `Connection..ctor` | 70561 | `0x6000054` | 23750 → 9911 | `0x2e8285` / 1029 | `447ff5ec9675da2617a399ec9eddde3beccea21d5aec8849d6a1f5e3fd5dc9e0` |
| N `Connection.Receive` | 70571 | `0x600005e` | 23760 → 9920 | `0x2e9303` / 2073 | `4cd5e0d0cd75724a119637cfd5a9d424979b9c716a0455ac366ff9b75411b01f` |
| N `<>c__DisplayClass2_0.<.ctor>b__0` | 70581 | `0x6000068` | 23770 → 9926 | `0x2ea11c` / 44 | `fb948ec576b463ce00e8f086194c5cd75f9b3d51149a712a417c5e7a21552683` |
| N `GatewayBuild..cctor` | 70597 | `0x6000078` | 23784 → 9938 | `0x2eaa2e` / 240 | `a880bca3fce442fdb0b747eb559115fdbf7a6cd594b05ae21dcc7d029049c800` |
| N `OpcodeMap.FromSeed` | 70733 | `0x6000100` | 23871 → 9939 | `0x2eab20` / 1122 | `5caa3883cf862ac568c961b14dba815c8c37464ef581a003488fea38cb7674ee` |
| N `GatewayBuild.Load` | 70598 | `0x6000079` | 23785 → 9940 | `0x2eaf83` / 66 | `fb86aaaf246bacb8c1345d602b6d0ec23864e62b7098487eb2141727b0fdb063` |
| N `GatewayFrames.Encode` | 70600 | `0x600007b` | 23787 → 9941 | `0x2eafc7` / 274 | `a298e0a56959cdfe33f64a14fe2e6e3bfe1c143d71f7b888074431a1315fea7c` |
| N `GatewayFrames.Push` | 70601 | `0x600007c` | 23788 → 9943 | `0x2eb0f1` / 540 | `5aabee33278091a2d942e9cf813558629bebacdcfb112c634a94b218d53f9f3f` |
| N `GatewayLink.IsBusy` | 70618 | `0x600008d` | 23795 → 9951 | `0x2eb76c` / 80 | `fd27f1ad985724714ad634940207b6bc9d4290be4401fac98055239ae42d02a7` |
| N `GatewayLink.ConnectAsync` | 70625 | `0x6000094` | 23802 → 9959 | `0x2eba34` / 706 | `faba13acdf2af2d1533404972aa57bb27a0d166f4d7b031c3d79d46a411f150a` |
| N `WebSocketTransport.BridgeUrl` | 70837 | `0x6000168` | 23965 → 9960 | `0x2ebcf8` / 271 | `0cf6244afff25e656f11979ca51d5cac3dfcdbd750f8e12e8dedad8471725711` |
| N `GatewayLink.Send` | 70626 | `0x6000095` | 23803 → 9962 | `0x2ebe75` / 376 | `0384f2e0e15671ae1dc6f087cee26c8081a7be2b035f62089d00465969e845e1` |
| N `GatewayLink.Close` | 70627 | `0x6000096` | 23804 → 9965 | `0x2ec421` / 880 | `e12c601e75078cc7b818086329368869b5e7f72a57b678f0cc77f348f5cf2ea9` |
| N `GatewayLink.Tick` | 70629 | `0x6000098` | 23806 → 9967 | `0x2ec7c6` / 1253 | `a752e72632fc09426fb276d613bd62e62511139f4c0eb5e822adf7c9d83f672b` |
| N `GatewayLink.Ask` | 70643 | `0x60000a6` | 23820 → 9969 | `0x2ecdf4` / 253 | `c18a4a16be8ace8c29162a408b452f31e1996a9785aec82b9260345dbead1de1` |
| N `GatewayLink.StartAttempt` | 70640 | `0x60000a3` | 23817 → 9971 | `0x2ecfd8` / 384 | `73dbd73a236db1785b265fff2ee22e69cc16b43a143b3403284f638cc0f920ae` |
| N `GatewayLink.Opened` | 70632 | `0x600009b` | 23809 → 9975 | `0x2ed2a3` / 462 | `f1247a36eb24b08c0d1c85f084abb977753d2909f501b33ba95f381847de8954` |
| N `GatewayLink.Binary` | 70633 | `0x600009c` | 23810 → 9976 | `0x2ed473` / 134 | `8d24134ee14f9787775afe8a41c12dcce3e5d0bc2014ee955e9ebaa43a8953a7` |
| N `GatewayLink.Text` | 70634 | `0x600009d` | 23811 → 9977 | `0x2ed4fb` / 1290 | `01a43f74023872decf9528a7ca8487b97fd71ffda69d51de269df88c006319d1` |
| N `GatewayLink.Confirm` | 70636 | `0x600009f` | 23813 → 9978 | `0x2eda07` / 239 | `7f45ead4f440a8efcb64ca97376fbacc3127124effe1008be152c5662456c154` |
| N `GatewayLink.Resumed` | 70638 | `0x60000a1` | 23815 → 9979 | `0x2edaf8` / 1161 | `dd4eaf8dca4c835e29abc56adc35a9a73c39ae7de8142a0b23778f618cd65359` |
| N `GatewayLink.Unconfirmed` | 70637 | `0x60000a0` | 23814 → 9981 | `0x2edfe0` / 600 | `9ec02d86c36eced3c0383ee8b16d472c05b70b669034ba1ad1612f4a1fdd7fe3` |
| N `LoginFailure.ProblemOf` | 70716 | `0x60000ef` | 23854 → 10003 | `0x2ef09c` / 43 | `96ed6b0c7be3d23b6a1cb2e6bb0da24af26e405c51df58a5bdbbe0d1c778e706` |
| N `LoginFailure.Of` | 70717 | `0x60000f0` | 23855 → 10004 | `0x2ef0c9` / 428 | `cea79982c84b3ae1a6286bdc6ef2b124a5678caed5facc4ae85e924253519b41` |
| N `<RunAsync>d__1.MoveNext` | 70720 | `0x60000f3` | 23858 → 10006 | `0x2ef33d` / 2050 | `8a744ab81cd762df5f31ffd429294d639b2b5fb8767fe1e54b998f658cbb457e` |
| N `LoginKey.Derive` | 70722 | `0x60000f5` | 23860 → 10012 | `0x2efcf5` / 576 | `601ea72d8fa0baea2182de94d88395df16eddd5e0771a504b67d41571628d893` |
| N `LoginKey.Text` | 70723 | `0x60000f6` | 23861 → 10013 | `0x2eff36` / 71 | `4de7df8de2d24d0c8722d90849e55cf3c0ab9657f85ea7f63c0dc2b7b1f2d7a4` |
| N `Md5..cctor` | 70727 | `0x60000fa` | 23865 → 10016 | `0x2f017c` / 475 | `33029383080f661cdde59589596167dc032277b221da2fd5c8c6951dc2b38d19` |
| N `PacketWriter.Chars` | 70743 | `0x600010a` | 23881 → 10027 | `0x2f0af2` / 399 | `51fd98b6d5abce55e7e12571a7680e441f462cf26f998c335936a9ed3f448c7f` |
| N `CharacterStatusWire.Read` | 70770 | `0x6000125` | 23908 → 10037 | `0x2f10d7` / 419 | `c39012244d819dcf6f709b403437e3ecbd780ac3ba3b31e99f4cbd2185911886` |
| N `Session.ParseCharacters` | 70786 | `0x6000135` | 23924 → 10048 | `0x2f192d` / 489 | `59fadf75055d32de394a5b8f493b9f6ec26ac164623f40c87f3a04b248b20678` |
| N `Session..cctor` | 70790 | `0x6000139` | 23928 → 10050 | `0x2f1b77` / 93 | `6534ed47d9336deff2766e3a4e6d849ce07839f79ecf99f1710ad919309a3581` |
| N `<>c__DisplayClass26_0.<EnterWorldAsync>b__0` | 70797 | `0x6000140` | 23935 → 10054 | `0x2f1c27` / 269 | `dd0038edc290d1310c18c46c2c400e550043762a11a9e3347cc51631f7d23f68` |
| N `<EnterWorldAsync>d__26.MoveNext` | 70799 | `0x6000142` | 23937 → 10056 | `0x2f1da2` / 3707 | `8dc7847f867093cf619dad6786f233398f0a065a4b7574127a1b9574241edefb` |
| N `<SignInAsync>d__22.MoveNext` | 70801 | `0x6000144` | 23939 → 10060 | `0x2f2cb6` / 3176 | `4ec4dc9acf0a2f986402a1992bfc30b092325d40135c94081f06ef320013ed62` |
| N `GameServerLink..ctor` | 72043 | `0x600061e` | 25170 → 10550 | `0x30ee4e` / 182 | `02cac4ac35d0958b1b94da07cd27f1a3dc2f22fb9ec9fd9ae2406bda07655f48` |
| N `GameServerLink.Attach` | 72049 | `0x6000624` | 25176 → 10556 | `0x30f213` / 506 | `00deee98cd7164d7016b4a80ce00a337086e77f6b156ce684d78c7f22427d6c5` |
| N `GameServerLink.Receive` | 72053 | `0x6000628` | 25180 → 10560 | `0x30f4b7` / 1214 | `ef89a982d6b120642abd7ce300ca521982435c529a9c01f63afc077f7211d7a8` |
| N `<LoginAsync>d__7.MoveNext` | 72059 | `0x600062e` | 25186 → 10564 | `0x30fa0b` / 3713 | `dfe40d0cf340e20b16875430fcbeced93c3f71d2d6b89f850e59b94b88a5d5f7` |
| N `SecondPasswordGate.NeedsLock` | 74540 | `0x6000fdf` | 27659 → 11238 | `0x333a3b` / 98 | `e4d9885c391f5cc8508ec958106e5982cc7ae2633044de64e046ae3bd4bccca5` |
| N `SecondPasswordGate.SendWithLock` | 74543 | `0x6000fe2` | 27662 → 11239 | `0x333a9f` / 238 | `04b15da62253a50047d902d4663639ac50bb7931a660948491aec977aa8839c5` |
| N `StartStatsWire.ParseStatBlock` | 74846 | `0x6001111` | 27965 → 11454 | `0x33fdb8` / 293 | `01bd9fae64ee5796122385479b2305cec025d0575092e075178531c11f032b6a` |
| N `StartStatsWire.ParseStartStats` | 74847 | `0x6001112` | 27966 → 11455 | `0x33fedf` / 139 | `24ec69931cea8026830ac8709b1378a748b56413ef32b38edccbe5473243c384` |
| N `StartStatsWire.StartStats` | 74848 | `0x6001113` | 27967 → 11456 | `0x33ff6c` / 832 | `8aec8609adf8b11a563e2a6ab7524f8d33046954dd940de0a16a80c21a69ef86` |
| N `Status.ReadStatBlock` | 74853 | `0x6001118` | 27972 → 11457 | `0x3402ae` / 391 | `eefaf5092b16a72c2b1d9ed618a6abc2b5a5be8e5c2c25ad3b2d7d95c657477d` |
| N `CharacterStatus.ApplyStart` | 74878 | `0x6001131` | 27997 → 11470 | `0x341c9a` / 655 | `80653fea7eaefdad93a37458ce9f8658cbdd9776e16fd67e42a7f49c7c2197c8` |
| N `Messages.ReadStartClock` | 73350 | `0x6000b39` | 26470 → 12341 | `0x374b82` / 156 | `ff0fda47346d77703ecf29cd21ec196b84e16567c006e776116cb23a2126fc7b` |
| N `Messages.ReadStartZone` | 73352 | `0x6000b3b` | 26472 → 12343 | `0x374c5a` / 65 | `638f3ad42abe2bc99a2e02def061198a6d23b4863b2a64dfbbe80ab08ca1f142` |
| N `Messages.ReadStartProfile` | 73353 | `0x6000b3c` | 26473 → 12344 | `0x374c9c` / 101 | `b43fe7d8856ef6391794f62a8d83201cc9d63c20fccf230acfdb6a13fadf79e7` |
| N `Messages.ReadStartBody` | 73354 | `0x6000b3d` | 26474 → 12345 | `0x374d02` / 106 | `42041fb86506ff09476f189de7916dacddb58a7312c680e0deae1128d153fd2b` |
| N `GameSession..ctor` | 72083 | `0x6000646` | 25210 → 13257 | `0x3ad98c` / 999 | `77c3db983dd5063c134388e18b1d0d98be58b46c1d0ab9da8b41e7a07e582d71` |
| N `GmHello.Of` | 72244 | `0x60006e7` | 25371 → 13258 | `0x3add75` / 224 | `0a64d0efdcee22d8a27a85b8d07e0adbf607c5bd5bdaab8526bdcf06d07198f9` |
| N `GameSession.BindGameServer` | 72120 | `0x600066b` | 25247 → 13259 | `0x3ade57` / 180 | `a3ba358aa0bf3aaaf50d14fa65529bd9acf350ecb2195a0a5034e1189cf1b8f3` |
| N `<ConnectAsync>d__36.MoveNext` | 72157 | `0x6000690` | 25284 → 13324 | `0x3b1e0c` / 1408 | `f6f5fba8a4ae770200be22586ac421da869c1223536dabe139b1b0f93d437268` |
| N `GmGiveawayWire.ParseHello` | 72241 | `0x60006e4` | 25368 → 13387 | `0x3b5380` / 315 | `decba2b54cc50cb81fd3e108316e0dfa523fa3d13c3cce0f6f198705da2eebc1` |
| N `GmHello..ctor` | 72251 | `0x60006ee` | 25378 → 13392 | `0x3b598c` / 1343 | `cb28a03cf2e2d4d17b30eb9d3872b3e84bcb7a7377053ad6d270227a1eda6bf4` |
| N `GmHello.Take` | 72252 | `0x60006ef` | 25379 → 13393 | `0x3b5ecd` / 248 | `a4e3b2f57cbbffe2b9a99de1c9b0c4c07a3914b9bf17e31bad9dbdbc26ef121b` |
| N `GmHello.<.ctor>b__13_0` | 72254 | `0x60006f1` | 25381 → 13395 | `0x3b6012` / 16 | `4abc83b560b0ae1362c36b59085e4f731eebba4816bddb22016dc641547f4172` |
| N `<>c.<Of>b__1_0` | 72257 | `0x60006f4` | 25384 → 13397 | `0x3b6061` / 54 | `b182fc0088a5653f3227b360f26e3c76a33ce97805976e31e71823675ea5dcb6` |
| E `<LoadFirstLogin>d__6.MoveNext` | 79892 | `0x6000623` | 3995 → 32925 | `0xa37830` / 2081 | `28cd8936b46cd376214119488365079a179780aee087b45189e2b40f7e9f086b` |
| U `CharSelectScreen.<CreateInfoView>b__128_0` | 18983 | `0x60005ab` | 47582 → 38066 | `0xbd429d` / 194 | `0c7eb6b9a1245c0d53b5479c8c3f49e14bd41c14dd30713a4566f3269e92ad7e` |
| U `CharSelectScreen.<CreateInfoView>b__128_1` | 18984 | `0x60005ac` | 47583 → 38067 | `0xbd4361` / 433 | `d82005834f421e0151a203eac6dbabfcad194cce2935e9201f235afc0270ea55` |
| U `<Enter>d__127.MoveNext` | 19132 | `0x6000640` | 47731 → 38184 | `0xbdb153` / 12844 | `704befd31b8e3404166b87bf46e120e667ac83f1eaac5ac632a70c74d9dbb2d5` |
| U `LoginScreen.LauncherCheck` | 19892 | `0x6000938` | 48450 → 49149 | `0x10536ec` / 551 | `586bbc172103f1a4258bc37ee82f4be941b183ab0d4a2903fed60341323bd82f` |
| U `<>c__DisplayClass42_0.<Enter>g__Submit\|8` | 19920 | `0x6000954` | 48478 → 49172 | `0x10546b1` / 767 | `9fd30a3838ea844b78cbf068c62d285fd99fca54ef679c873f91d54b57091682` |
