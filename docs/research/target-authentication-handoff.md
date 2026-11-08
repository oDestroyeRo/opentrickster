# Target authentication, character handoff, and startup baseline

The goal is an implementation-ready specification for a macOS Rust/GPUI bot that visibly embeds the game in client-assisted mode and controls the same character without Unity in clientless mode. This report answers [Recover target authentication and character-handoff contracts](https://github.com/oDestroyeRo/opentrickster/issues/15), part of [Plan the two-mode Trickster bot MVP](https://github.com/oDestroyeRo/opentrickster/issues/2). Scope is one existing account/character, combat and digging; quest automation is outside the MVP.

**Result: the pinned client exposes a usable static contract from password transformation through server/channel selection, existing-character selection, GameServer entry, and the minimal initial own-state baseline.** Initial GameServer login and later zone handoff are different operations. Neither establishes that Unity and a native process can transfer control of the same live character. Native authentication, ticket acceptance, expiry, duplicate-use policy, and cross-process continuity remain unverified.

This was account-free static research. No account login, game/WebSocket session, private credential file, browser storage, live capture, target JS/WASM execution, credential replay, or ticket replay occurred. Public caches were read-only; analysis and synthetic fixtures stayed under `/tmp/opentrickster-issue15`. No game assets or real authentication values are included. The only fresh target request was an unauthenticated GET of public `spinel-config.json`.

Research date: 2026-11-08. Actual starting SHA: `6b8162f1f7a42b2bd9dd02c1b086be360914bb8d`, clean local branch `research/auth-handoff-contracts`. The authenticated developer and existing assignee both matched `oDestroyeRo`; no assignment change was necessary. Publication target is `research/target-authentication-handoff`. The resolution comment records the final commit and independently read remote SHA; a document cannot embed its own commit hash without changing that hash.

## Evidence and revision boundary

“Extracted” below means joined metadata, field offsets, dispatch operands, and compiled instructions. “Derived” means a transcription or offline synthetic model. No server behavior was observed by this investigator. The sanitized [live correlation report][R3] establishes that a visible client previously entered an existing character, not native authentication or the private values used there.

The [framing report][R1] and [artifact inventory][R2] supplied leads. Their relevant code joins were inspected here. The orchestrator's independent reproduction of the same-revision opcode tables, inverses, six-byte frame, fragmentation/coalescing, and seed/size failures is reused. The newly published [incoming-state report][R4] supplied startup leads; the readers and field writes below were then inspected directly. [W], [D]

| Inspected public body | Bytes | SHA-256 checked here |
| --- | ---: | --- |
| [Loader][L] | 27,914 | `de82b056302e1bfe1c0524e88b377535a508a9538fcc6a4f9533806c03d3548d` |
| [Framework][J] | 386,859 | `bd2fce60698f3cd563cc1d2a635afaf7045a4a6d78e9edd3f478f89c2e2993ca` |
| [Unity data][D] | 27,239,767 | `f1ab3ef3d2b6b04b28104bac55b25c007e78a24242a56c354b640428daf3053d` |
| [WASM][W] | 42,317,618 | `77a05d62cc5cc6fd5d1a1f5b4f4d25d8c0d1f9e8ea4740c08326f8ec83e2b446` |
| Extracted `global-metadata.dat` | 9,148,328 | `b7b7920f034cbdfb87eaf69557e17ea6b1513693acb9e905e918eaedbaf1a483` |
| [Public endpoint configuration][CFG], fresh GET | 124 | `4d5bb8fff52d2c258f55dc1f177c075ededeaecd48a54967ca8f8586e0ae7990` |

Cached `.gz` paths contain decoded response bodies. Metadata begins at data offset `16459759`; its magic/version are `0xfab11baf / 108`. Pinned build is `f5e972fa2be429602c20d6168013a20f`, Unity `6000.6.5f1`. Visible revision `6ab10889` is a different layer. The freshly fetched configuration's `build` matches the pinned build; the cached assets were rehashed, not freshly downloaded. Fixed locators and opcode permutations must be reconsidered if fingerprints change. [R1], [R2], [R3], [CFG]

All opcodes below are **logical**, with direction and connection specified. Outbound values go through the build's c2s permutation; inbound values come through its s2c inverse. Frame shape is `U16 payloadLength, U16 wireOpcode, U16 Aux, payload`, little-endian, after stream reassembly. A payload schema does not replace gateway connection, acknowledgement, retained-byte resume, or Aux behavior. [R1], [R4]

`U8/U16/U32/U64` mean unsigned wire widths; all multi-byte integers are little-endian. Managed `long` storage does not imply U64 on the wire. Offsets are zero-based payload offsets. A stated consumed prefix is not necessarily an enforced exact packet length.

## Strings and password transformation

`PacketWriter.Chars(text,n)` CP874-encodes, requires **encoded length < n**, and zero-pads to exactly n bytes. It rejects length equal to n; it does not silently truncate. `Cstr(text)` writes CP874 bytes plus one NUL, without a length prefix. Readers consume the whole fixed-width field, decoding through its first NUL, or consume through the C-string NUL; missing fixed bytes or a missing C-string terminator fail. [W] 18227, 18044, 18052, 18058; helpers 18034 and 14272.

The target implements its own CP874 table. ASCII maps directly. Thai `0xa1..0xda` maps to `U+0E01..U+0E3A`, `0xdf` to `U+0E3F`, and `0xe0..0xfb` to `U+0E40..U+0E5B`. Undefined Windows-codepage slots are not uniformly errors: `0xdb..0xde` and `0xfc..0xff` map to `U+F8C1..U+F8C8`; C1 slots and punctuation are explicitly mapped. Unknown Unicode code points encode as `?` (63); the encoder groups a surrogate pair as one code point. The derived codec in the verification block was compared with all 128 compiled high-byte mappings. Do not substitute UTF-8 for packet strings. [W] `Cp874.BuildHigh` 14269, `ByteOf` 14271, `Encode` 14272, `Decode` 14274.

The actual UI path is `PregameLoginModel.LogIn` → the screen's `Connect` delegate → login-screen Submit → `SignIn`/`LoginAttempt.RunAsync` → `Session.SignInAsync`. `PregameLoginModel` is a UI model, not a separate authentication protocol. Submit trims the ID with `String.Trim()` and only trailing ASCII spaces from the password, rejects either empty result, then applies `LauncherCheck`. Accepted IDs are at most 12 printable ASCII characters, excluding comma, pipe, and double quote; spaces are excluded too. A lowercase copy is used for a reserved-ID lookup, but the submitted ID retains its case. Passwords are at most 18 printable ASCII characters, including leading/internal spaces. [W] 49293, 47117 at `0xfccbba`, `0xfccbcf`, `0xfccd14`; 47094; 47124 → 18205/18206 → 18207/18259. [D] `LoginScreen` fields `IdEdit +32`, `PasswordEdit +36`, `Connect +44`, `SigningIn +60`.

The derived credential is:

1. Take at most the first 18 UTF-16 code units; the lower-level function truncates even though the UI rejects overlength input.
2. Remove trailing `U+0020` spaces; preserve leading spaces.
3. Convert only ASCII `A..Z` to `a..z`.
4. Encode UTF-8 without a BOM and compute MD5.
5. Take digest bytes `[4:12]` and interpret them as a big-endian 64-bit bit pattern.
6. FirstLogin writes that bit pattern as eight **little-endian** bytes. LoginServer instead writes its **signed Int64 invariant decimal** representation as a CP874 C-string. A high-bit value is negative text, not unsigned decimal or hexadecimal.

This is a password-derived credential, not encryption and not the user's ID. No salt, ID, or server challenge is used in this function. `SignInAsync`'s unboxed password field at `+24` feeds `LoginKey.Derive`; its ID field at `+20` is stored separately. This distinguishes password hashing from ID normalization. [W] 18259 at `0x4ca5c2`; 18212 at `0x4c7489`; 18213 at `0x4c76fc`/`0x4c7702`. [D] async field registration includes the boxed header; compiled unboxed offsets subtract eight bytes.

The MD5 identification comes from the target's padding, bit-length suffix, IV, round functions/index schedules, rotations, sine-derived constants, and four little-endian output words, not merely the class name. Its 64 rotation values at metadata `0x4cb518` match the standard; functions 18215/18216 build the constants. [W] 18019 at `0x4b8939`, 18215 at `0x4c77a2`, 18216 at `0x4c7911`; [MD5] specifies the standard. MD5 here describes compatibility with this client, not a proposed password-storage design.

A wholly synthetic example, `ExampleOnlyA1`, normalizes to `exampleonlya1`, whose MD5 is `07a3acf9d7cdb6ccf99588dce5636f15`. Its signed key is `-2896457993852450596`; its FirstLogin eight bytes are `dc8895f9ccb6cdd7`. Python and local OpenSSL agreed on this synthetic digest. No actual login ID/password or sourced credential fixture was used.

## Endpoint selection and staged login

For this web path, `ServerAddress.FirstLoginAsync` calls `WebPage.FirstLoginAsync`; its `LoadFirstLogin` reads relative `spinel-config.json`, deserializes `firstLogin.host/port`, and rejects missing/empty host or nonpositive port. The public configuration currently says host `45.150.128.227`, port `9980`, with the matching build. A separate nonweb path has command-line/environment/fallback endpoint handling; that is not evidence of the web deployment's selected endpoint. [W] 47535, 47536, 34084, 34089 at `0xa700fb` and `0xa7010f`; [CFG]

`SessionOptions.FirstLogin` supplies the first endpoint. Subsequent endpoints come from server replies below. In the inspected web transport these backend addresses are carried through the gateway at `wss://tr-play.playall.in.th/net`; they do not prove that direct native TCP connections are supported. The public-build URL flags and directional map come from the framing report. No endpoint here was used to open a game connection. [W] 18259, 18255, 16260; [R1]

### FirstLogin connection

`SignInAsync` closes previous login connections, stores the ID/key, opens FirstLogin, sends 11501, then waits up to 15,000 ms for one of 11502/11503/11508. That set is also recovered from the field-RVA array, not inferred from names. [W] 18259; [D] usage cell `0x3380c8` → field reference 246 → field 67974 → default-data `0x4cb850`, three Int32 values.

| Direction / logical opcode | Ordered payload or branch |
| --- | --- |
| C→FirstLogin `11501 / 0x2ced` | `Chars(Id,19), Chars(compiledMarker,19), U64(derivedKeyBits)`; exactly 46 bytes. |
| FirstLogin→C `11503 / 0x2cef` | `U32 code`; throws `LoginRefused` with stage `firstlogin`. |
| FirstLogin→C `11508 / 0x2cf4` | `Chars(reason,256)`, then six U16 words: year, month, ignored word, day, hour, minute. Formats words 1,2,4,5,6 as `year-month-day hour:minute`; throws stage `firstlogin`, code `-1`, with reason/until detail. The ignored word and time zone are unknown. |
| FirstLogin→C `11502 / 0x2cee` | `Raw[12] loginTicket, U8 count, count × WorldEntry`; saves the opaque 12 bytes unchanged. |

The five-byte compiled marker in the second field is **not the operator's password**. Its server-side authorization significance cannot be established client-side. To avoid indiscriminately publishing a potentially access-bearing constant, its value is omitted: metadata literal index `19380`, absolute metadata offset `0x72ad8`, length 5, SHA-256 `b5a2c96250612366ea272ffac6d9744aaf4b45aacd96aa7cfcb931ee3b558259`. Reproduction can identify it locally without logging it. This is distinct from the public opcode seed. [W] 18259; [D]

Each `WorldEntry` consumes 75 bytes:

| Offset | Wire field |
| ---: | --- |
| 0 | U8 Active |
| 1 | U16 World |
| 3 | U16 Island |
| 5 | Chars(ServerName,33) |
| 38 | Chars(WorldName,33) |
| 71 | U16 MaxUsers |
| 73 | U16 Users |

The client returns these rows for world/server-channel selection and sends the selected row's `World` and `Island` values. Do not infer a single independent channel ID, ordinal, or fixed production channel list. The authoritative values for selection on this path are the returned pair. [W] 18259 and 18255; [D] `WorldEntry` type 11167, fields `World +12`, `Island +16`, `ServerName +20`, `WorldName +24`.

### Selected world/channel and LoginServer connection

`Session.EnterWorldAsync` requires an existing FirstLogin connection. It performs the following transition, then retains the new LoginServer connection in the Session. [W] 18255 at `0x4c9540`.

| Direction / logical opcode | Ordered payload or branch |
| --- | --- |
| C→FirstLogin `11505 / 0x2cf1` | `U16 selected.World, U16 selected.Island`; four bytes. |
| FirstLogin→C `11507 / 0x2cf3` | `U32 code`; throws `LoginRefused`, stage `world`. |
| FirstLogin→C `11506 / 0x2cf2` | `Chars(host,16), U16 port`; 18 bytes consumed. Client closes FirstLogin and opens LoginServer at this endpoint. |
| C→LoginServer `2000 / 0x07d0` | `Cstr(Id), Cstr(signedDecimalKey), Raw[4] ClientVersion, Raw[12] loginTicket, U8(20)`. |
| LoginServer→C `2001 / 0x07d1` | `U32 code`; code 1067 additionally reads two C-strings as detail. Throws `LoginRefused`, stage `login`. |
| LoginServer→C `2002 / 0x07d2` | Character-list grammar below. |

Both response waits in `EnterWorldAsync` use 15,000 ms. The two expected pairs are `[11506,11507]` and `[2002,2001]`, packed into little-endian constants in the compiled initialization. [W] 18255.

`ClientVersion` is a compiled four-byte compatibility field copied verbatim, not the visible revision, build-ID string, or a freshly computed integer. Locator: `Session..cctor` 18249 at `0x4c9315`, usage cell `0x3380bc`, field reference 243, field 67971, metadata default-data absolute `0x4cb7a0`, four-byte SHA-256 `e4abdaa9e4fe688b531f6d5257aad881668ae7d84dd3e8fdf305cfcc0e29b9c6`. The final byte 20 is established; its server meaning is unknown. [D], [W]

This connection also registers S→C logical 2028, answered by C→S 2028 containing a U32 derived from the local millisecond clock, and S→C 5509, read as `U64 key, Cstr host, U32 port, U32 ticket` into a separate `SystemServerTicket`. That ancillary ticket is not the GameServer credential. This report does not expand SystemServer startup into another protocol investigation. [W] 18255, 18253, 18254, 18248.

### Character list and optional lock/busy state

Logical 2002 is parsed as:

```text
U8 Count
Count × Raw[156] CharacterEntry
U32 SecondPasswordState
U32 SecondPasswordFails
Cstr SecondPasswordHint
U8 ignored
U8 ignored
U8 UnlockedSlots
optional CharacterStatusWire
```

`ReadCharacter` copies each complete 156-byte record and extracts only these summary fields; other bytes are preserved, not assigned invented meanings. [W] 18247 at `0x4c90bd`, 18047 at `0x4baa30`.

| Record offset | Summary field |
| ---: | --- |
| 0 | U64 Uid |
| 8 | U8 Slot |
| 9 | Chars(Name,20) |
| 29 | U16 Type |
| 43 | U16 Level |

The optional trailer is `U8 version, U8 count, count × (U8 slot, U8 kind, U32 value)`. The reader returns null if fewer than two bytes remain, version is not 1, or fewer than `count*6` bytes remain after the prefix. Unknown-version/truncated-record cases have already consumed the two-byte prefix. It retains only kinds 1 and 2, corresponding to `OfflineBot` and `OfflineShop`; no offline automation behavior is inferred. `CharacterList.ReceivedAt` is a local clock sample. [W] 18236 at `0x4c8860`, 18247; [D] type 11170 enum defaults and type 11173.

The lock fields do not make the primary password transformation a two-factor exchange. Character selection 2012 below sends UID without a lock hash. The selection UI checks busy state and contains a confirmation path; its ability to request a selection does not establish takeover policy. `CharSelectScreen.Enter` has an information-view branch when `SecondPasswordState == 0`. [W] 47530, 47485 at `0xfeff6b`, 18086.

For later protected operations, `SecondPasswordGate.NeedsLock` requires state 3, a positive operation flag, and intersection with configured flags. `SendWithLock` uses outer logical **927 / 0x039f** and serializes `U16 innerOpcode, Cstr lockHash, Raw innerPayload`. `Characters.LockHash` is a separate transform: CP874-encode input, MD5, lowercase hex, take characters `[8:24]` (digest bytes 4..11 in hex). It shows no primary-login lowercase/trim transform. Full operation-flag policy and account-specific lock requirements are not recovered here; preserve state/failure/hint values and do not silently treat an unknown lock state as unlocked. [W] 15532 at `0x422312`, 15533 at `0x422376`/`0x4223f5`, 15433 at `0x41d69d`, 18018 at `0x4b8832`.

## Existing-character selection and initial GameServer entry

`Characters.SelectCharacterAsync` selects by **U64 UID**, not slot, name, account ID, or the later short actor ID. Before sending it registers a `Followup` for opcode 2014; that callback immediately starts a wait for `[3,2]`. It then sends 2012 and waits for `[2014,2013,2]`, using 30,000-ms waits. This registration order matters for a fast followup. [W] 18086, 18057, 18133; [D] usage `0x3380dc` → field reference 251 → field 67979 → `0x4cb998`, array `[2014,2013,2]`.

| Direction / logical opcode | Payload or action |
| --- | --- |
| C→LoginServer `2012 / 0x07dc` | U64 selected Uid; eight bytes. |
| LoginServer→C `2013 / 0x07dd` | U32 error code; `CharacterOpError(2013,code)`. |
| LoginServer→C `2 / 0x0002` | U16 ignored, U64 ignored, U32 error code; `CharacterOpError(2,code)`. Same reader is used during the followup. |
| LoginServer→C `2014 / 0x07de` | `U64 Uid, U16 InfoIndex, U32 Crc, Cstr Host, U32 Port, 7 × U16 RestrictionMinutes`. |
| LoginServer→C `3 / 0x0003` | Followup success marker; this path checks the opcode and does not parse its payload. Only then does selection return the ticket. |

The ticket is a client model with several fields, not a demonstrated JWT or signed struct. `Crc` is the client's field name; no CRC algorithm, signature format, server-side computation, or authenticity rule is established. Host and Port are returned, not selected from a compiled GameServer list. Port and Crc are each U32 on the wire, although stored in managed 64-bit fields. The seven restriction values are not evidence of ticket expiry. [W] 18086, `GsError` 18030; [D] `GameServerTicket` type 11175: `Uid +8`, `InfoIndex +16`, `Crc +24`, `Host +32`, `Port +40`, `RestrictionMinutes +48`.

`GameSession.ConnectAsync` opens a `GameServerLink` using ticket Host/Port, tracks the connection through Session, calls its `LoginAsync`, and creates/binds the GameSession after startup succeeds. A failure closes the newly opened GameServer link. It does not run the primary credential transformation again. [W] 16260 at `0x44c537`, close on failure `0x44c8e8`; 16171, 16172/16184, 18242, 16196, 16197.

Before sending the initial login, `GameServerLink.LoginAsync` starts waits for logical **679, 230, and 233**, each with 20,000 ms, and installs a 1261 honor-title listener. It sends:

| Direction / logical opcode | Ordered payload / branch |
| --- | --- |
| C→GameServer `4 / 0x0004` | `U64 ticket.Uid, U16 ticket.InfoIndex, U32 ticket.Crc`; 14 bytes. |
| GameServer→C `159 / 0x009f` | Refusal branch throws `ProtocolException`; this path does not decode an error-code payload. |
| GameServer→C `5 / 0x0005` | Accepted marker; only opcode checked here. |

The marker wait is also 20,000 ms. Opcode 5 alone is not the constructed session: it then awaits all three startup messages and builds `StartState`. `Attach` replays buffered GameServer frames into the session handler. Socket open, a transport byte acknowledgement, accepted marker 5, and a usable own-state baseline are distinct milestones. [W] 16184, send at `0x447219`–`0x4472b5`, refusal at `0x44742c`, startup construction at `0x447899`–`0x447a08`; 16176, 16197. [R4] describes Aux grouping/backlog delivery.

## Minimal initial own-state baseline

These are incoming GameServer **payload** offsets. They come from the three startup waits above, not guesses based on later movement packets. `GameServerLink.LoginAsync` assigns clock ID/position into `StartState +8/+12`, zone into `+20`, and name into `+24`. `GameSession` stores that StartState as `Me +92`. Its U16 actor ID is distinct from the U64 character UID used for selection/authentication. [W] 16184 at `0x447926`, `0x44797a`, `0x447981`; 16196; [D] StartState row `0x153d80`, GameSession row `0x152900`.

| Logical opcode | Offset | Read / minimal role |
| --- | ---: | --- |
| `230 / 0x00e6` | 0 | U32 DayClock; client stores the widened number. Time epoch/unit not established here. |
| 230 | 4 | U16 own actor Id. |
| 230 | 6, 8 | U16 own X, U16 own Y; expanded into integer Pos. Ten bytes consumed by `ReadStartClock`. |
| `679 / 0x02a7` | 17 | Chars(Name,20). |
| 679 | 37 | U16 profile BodyType; first 39 bytes needed by `ReadStartProfile`. |
| `233 / 0x00e9` | 184..207 | Twelve U16 stat values, in index order 0..11. |
| 233 | 208 | U16 Bonus, via `Status.ReadStatBlock`/`CharacterStatus.ApplyStart`. |
| 233 | 212 | U16 TmLevel via the same path. Bytes 210..211 are not assigned meaning by these readers. |
| 233 | 214 | U16 Level. |
| 233 | 219 | U16 Zone; `ReadStartZone` requires bytes through 220. |
| 233 | 231 | U32 Galder, widened to managed long. |
| 233 | 235 | U32 current Hp. |
| 233 | 239 | U32 current Mp. |
| 233 | 243 | U32 TmExp, through `CharacterStatus.ApplyStart` if payload length ≥247. |
| 233 | 247 | U32 Exp. |

Sources are `ReadStartClock` 17093, `ReadStartProfile` 17096, `ReadStartZone` 17095, `StartStatsWire.ParseStatBlock` 15010, `ParseStartStats` 15011, `Status.ReadStatBlock` 15013, and `CharacterStatus.ApplyStart` 15026. These readers leave many intervening bytes and tails uninterpreted; they do not describe a complete 233 record. [W]

`StartStatsWire.StartStats` scans the session's start frames for logical 233 and requires payload length **at least 251** before `ParseStartStats`. If unavailable/short, it returns no StartStats. It reads stat values at `184 + 2*i`, Level at 214, and U32 values at 231/235/239/247. `CharacterStatus.ApplyStart` separately accepts 233 at length ≥217, updates Points/Bonus/TmLevel/Level, recomputes totals, and conditionally reads TmExp. Its profile branch accepts 679 at length ≥69 and retains the first 69 bytes. Missing startup data therefore must not be reported as authoritative zero. [W] 15012 at `0x403b93`–`0x403bb1`; 15011; 15026 at `0x40574f`–`0x4057a9`, `0x4057f9`, `0x4058d1`–`0x405949`.

`ReadStartBody` has a small variant dependency worth retaining: it normally reads U16 BodyType from **679 offset 37**, but if 233 has at least 274 bytes, byte **233[273]** selects offset 53 for value 2 or offset 56 for value 3, still in the 679 payload. Other values use offset 37. This report does not infer the variant meanings or decode full appearance/effect/entity tails. [W] 17097 at `0x47dab9`.

The existing combat UI obtains StartStats, and the game-screen startup copies its current HP/MP into the UI seed. `CombatModel.Load` reads the stat array and uses `LevelTables.PlayerTotal` with indices **9, 3, and 6** to compute and store **MaxHp, MaxMp, and MaxWeight** respectively; it caps existing current HP/MP when maxima are positive. Thus current U32 HP/MP are recovered, but maxima are not a second pair of fixed U32 fields here. Their complete formula depends on CharacterStatus totals and level tables and is intentionally not guessed. [W] 46846 at `0xfad096`, `0xfad236`, `0xfad3ca`; 23353 at `0x6ae132`, `0x6ae1bf`–`0x6ae209`; 23354 at `0x6ae37b` is the exact remaining formula boundary. [D] StartStats fields `Hp +24`, `Mp +32`; CombatStats `Hp +16`, `MaxHp +24`, `Mp +32`, `MaxMp +40`, `MaxWeight +48`.

This baseline is enough to identify the own actor and initial zone/position and to seed **current** HP/MP for a later authorized correlation with the incoming-state report. It does not establish coordinate units, freshness, stat-index names beyond the proved uses, maximum-resource formulas, full startup inventory, or later action acceptance. The next comparison should preserve the same connection's order/Aux handling and distinguish current snapshots from subsequent deltas. [R4]

## Later zone handoff is a separate state transition

Incoming GameServer logical **154 / 0x009a** reaches `Messages.ParseHandoff`. The switch is proved by `opcode - 154`, `br_table 12 13 10` at `0x44a5ad`: the first target exits the block ending at `0x44ab42`, then calls parser 17091 at `0x44ab5e`. It is not assigned from a symbol name or the adjacent opcode 155. [W] 16232.

```text
S→C 154:
  U32 ignored
  U64 Uid
  Cstr Host
  U32 Port
  U32 Key
  U16 Zone
  U16 X
  U16 Y
  U8 Option

C→new GameServer 156 / 0x009c:
  U64 Uid
  U32 Key
  U8 Option
```

`ParseHandoff` stores these in `HandoffMessage`: `Uid +8`, `Host +16`, `Port +24`, `Key +32`, `Zone +40`, `Pos +44`, `Option +52`. GameSession writes the announced zone/position, stores the pending message, sets `handoffPending`, and emits the zone-change event. While pending, its handler returns before ordinary processing. This is local transition intent, not proof that arrival has succeeded. [W] 17091 at `0x47d78f`; 16232 at `0x44ab6f`–`0x44abb2`; [D] type 12021.

`CompleteZoneChangeAsync` opens the supplied endpoint and its send callback invokes `SendHandoffLogin` with the pending Uid/Key/Option. The serializer writes 13 bytes and sends logical 156. Its retry/failure path has two `TrySend` attempts and a `HandoffLoginFailedException`; a successful local send does not itself prove server acceptance. [W] 16256 at `0x44bcf3`, callback 16254 at `0x44bb96`, failure at `0x44c2d4`, serializer 16174 at `0x44654e`.

The initial 14-byte opcode-4 credential and later 13-byte opcode-156 credential are not interchangeable. There is no recovered “detach Unity and attach Rust” request in these paths, and no evidence that requesting a server-directed zone change is a valid way to move ownership between two independent clients. That requires a separate authorized continuity proof; it must not be specified as solved because a method is named `SendHandoffLogin`.

## Failure handling, lifetime, and credential boundaries

The inspected FirstLogin classification maps code 60003 to WrongIdOrPassword; 60009 and 60012 to Blocked; 60010 to Withdrawn; 60011 to NoGameAccount; 60013 to AlreadyLoggedIn; 60016 to LoginClosed; other codes to ServerError. FirstLogin's special `-1` refusal maps to Blocked with details. `LoginFailure.Of` applies that classification to `firstlogin` refusals; other exceptions fall through to Unreachable here. Preserve stage/raw code/detail instead of reusing this taxonomy for every server. [W] 18203, 18204; switch table at linear-memory `1929196`; [D] LoginProblem enum type 11155 with compressed defaults 0..7.

`LoginAttempt.RunAsync` has a release-held callback/retry branch for AlreadyLoggedIn. Its existence proves a client retry attempt, not successful logout, exclusivity transfer, duplicate-session permission, or the server's ownership policy. Gateway release has its own transport cleanup semantics. This research did not invoke either. [W] 18206; [R1]

| Value | Established role | What is not established |
| --- | --- | --- |
| Operator ID/password and derived key | Application authentication inputs; key used in FirstLogin and LoginServer. | Server password rules beyond this client's normalization; native acceptance. |
| Compiled five-byte marker | Fixed second 19-byte FirstLogin field; fingerprint/locator above. | Whether the server treats it as a marker, compatibility password, or other access check. Do not label it an account secret or assume it harmless solely because public. |
| Compiled four-byte ClientVersion | Fixed LoginServer compatibility field. | Server's accepted-version policy. |
| FirstLogin ticket | Opaque 12 bytes from 11502, saved and copied into 2000. | Internal schema/signature, generation, expiry, reuse, account binding. |
| GameServerTicket | 2014 envelope containing Uid, InfoIndex, Crc, endpoint, restrictions. First three fields form initial opcode 4. | Crc algorithm, signature, TTL, single-use or deduplication guarantees. |
| HandoffMessage Key | Server-supplied U32 from 154, copied with UID/Option into 156. | Transferability, expiry, replay safety, cross-client ownership. |
| GatewayLink token | Resumable **transport** capability supplied/rotated by text `k` and `r` controls; tied to byte positions and retained bytes. | Application authentication equivalence or permission to take over a character. |
| Public opcode seed | Compiled input to build-specific directional permutations. | No account authorization role is evidenced by this mapping function. |

The Session stores the derived key and opaque login ticket; `Close` closes connection fields but does not zero ID/key/ticket in the inspected implementation. No local ticket-expiration comparison or internal ticket validation was found on this path. The 15/30/20-second request waits are **not ticket lifetimes**. Gateway `holdSeconds` concerns transport retention, not application ticket expiry. A browser shutdown's `net/release` attempt does not prove application logout or portability of a retained stream to another process. [W] 18208 at `0x4c73b4`, 18259, 18255, 18086, 16184; [R1]

For the MVP specification, credentials and issued capabilities must remain runtime-only private data, outside Git and diagnostic fixtures. Represent the 12-byte ticket opaquely, and distinguish character UID, short actor ID, initial GameServer credential, zone-handoff credential, and gateway transport token. These are derived design constraints from the recovered contracts, not new product implementation.

## Reproduction and precise remaining proof

Use the immutable [framing report's standalone locator][R1], with the exact hashes above, then select the methods in the appendix and the matching code-generation module below. Its parsing uses metadata records and WASM sections only, without module instantiation. Metadata v108 is supported by the pinned [Cpp2IL metadata reader][C1], [method definition][C2], and [code-generation module layout][C3]. Field-offset registration follows [metadata registration][C4] and [type layout][C5]; RVA joins follow [metadata usage][C6], [field references][C7], and [field defaults][C8]. WABT 1.0.39 `wasm-objdump -d` produced the static instruction listing. [T1], [T2]

| Image | Code-generation module | Method count | Pointer array |
| --- | --- | ---: | --- |
| Spinel.Net.dll | `0x20ad44` | 4729 | `0x385d90` |
| Spinel.Game.dll | `0x20ad18` | 18411 | `0x372d70` |
| Spinel.UI.Custom.dll | `0x20ad70` | 16592 | `0x38aa20` |
| Spinel.UI.Pc.dll | `0x20ad9c` | 16433 | `0x39b730` |
| Spinel.Engine.Unity.dll | `0x20acc0` | 1622 | `0x370c70` |
| Spinel.Formats.dll | `0x20acec` | 290 | `0x3728c0` |

Metadata registration is at linear-memory `0x19e360`. Type pointers: count 41885, array `0xd52c0`; field-offset rows: count 15196, array `0x351ee0`. Type, method, field, parameter, image records are 66, 30, 10, 10, 54 bytes. Method-token RID minus one indexes the owning image's method-pointer array, yielding an element-table slot; resolve that to a WASM function index (484 imports precede code bodies). Async value types use unboxed offsets; object fields include the eight-byte managed header. Shared code bodies can have several names: e.g. `LoginKey.Text` shares function 18213 with unrelated methods. Use its own token to prove the join. [D], [W], [C1]–[C5]

Local verification passed for all 37 listed method joins, six code-generation modules, 33 field offsets across eight types, busy-state enum defaults, opcode arrays, all 128 CP874 high-byte mappings, and the synthetic shapes. The executable block below is a derived model, not decompiled target code or a network client. Its fabricated marker/version/ticket values have no claimed server validity. It checks normalization, signed formatting, string rules, widths, character offsets, every truncation of the synthetic GameServer ticket, and startup offsets. Existing frame/table verification is reused rather than re-run.

The bounded static ticket can close. The smallest next proof is an **independent offline read of the authentication/bootstrap joins and synthetic vectors at these hashes**, followed only under separate authorization by private correlation of startup 230/233/679 against visible own identity, zone/position, and current HP/MP. Publish field conclusions, never credentials or issued tickets. A separately authorized native-login experiment is required to establish server acceptance; a further ownership/continuity experiment is required to establish switching between Unity and the native dashboard. This report supplies neither guarantee.

Remaining bounded gaps are explicit: full 156-byte character details; complete lock-operation policy; server error-code dictionaries outside the inspected branches; internal/expiry/reuse semantics of issued credentials; stat maxima behind `LevelTables.PlayerTotal` 23354; and full appearance/effect/entity tails. None is replaced with another emulator's packet list. The incoming-handler investigation owns those tails and action-result completeness. No product build, server-side test, or independent reviewer ran in this single-researcher task.

## Derived offline verification

Save this block outside the repository and run with Python 3. It uses only standard-library code and wholly synthetic data; it makes no network requests.

```python
# Derived offline checks. Every identity, password, endpoint and ticket below is synthetic.
import hashlib
import struct

high = list(range(0x80, 0x100))
for byte, cp in {0x80:0x20ac, 0x85:0x2026, 0x91:0x2018, 0x92:0x2019,
                 0x93:0x201c, 0x94:0x201d, 0x95:0x2022, 0x96:0x2013,
                 0x97:0x2014}.items():
    high[byte-128] = cp
high[0xa1-128:0xdb-128] = range(0xe01, 0xe3b)
high[0xdb-128:0xdf-128] = range(0xf8c1, 0xf8c5)
high[0xdf-128] = 0xe3f
high[0xe0-128:0xfc-128] = range(0xe40, 0xe5c)
high[0xfc-128:] = range(0xf8c5, 0xf8c9)
codec = ''.join(map(chr, list(range(128)) + high))
inverse = {char: i for i, char in enumerate(codec)}
def enc(s): return bytes(inverse.get(c, 63) for c in s)
def dec(b): return ''.join(codec[c] for c in b)
def fixed(s, width):
    b = enc(s)
    if len(b) >= width: raise ValueError('no room for terminator')
    return b + bytes(width-len(b))
def cstr(s): return enc(s) + b'\0'
def key(s):
    # Accepted UI inputs are ASCII; UTF-16 truncation therefore equals this slice.
    assert s.isascii()
    s = s[:18].rstrip(' ')
    s = ''.join(chr(ord(c)+32) if 'A' <= c <= 'Z' else c for c in s)
    digest = hashlib.md5(s.encode('utf-8')).digest()
    return int.from_bytes(digest[4:12], 'big', signed=True)
class Reader:
    def __init__(self, b): self.b, self.p = b, 0
    def take(self, n):
        if n < 0 or self.p+n > len(self.b): raise ValueError('truncated')
        result = self.b[self.p:self.p+n]
        self.p += n
        return result
    def number(self, fmt): return struct.unpack('<'+fmt, self.take(struct.calcsize('<'+fmt)))[0]
    def chars(self, n): return dec(self.take(n).split(b'\0', 1)[0])
    def string(self):
        end = self.b.find(b'\0', self.p)
        if end < 0: raise ValueError('unterminated')
        return dec(self.take(end-self.p+1)[:-1])
def gs_ticket(b):
    r = Reader(b)
    v = [r.number(f) for f in ('Q', 'H', 'I')]
    v += [r.string(), r.number('I'), [r.number('H') for _ in range(7)]]
    return v, r.p

def rejects(f):
    try: f()
    except ValueError: return
    raise AssertionError('expected rejection')

# No source credential, real account identifier, or issued ticket is used.
k = key('ExampleOnlyA1')
assert k == key('EXAMPLEONLYA1   ')
assert key('ABCDEFGHIJKLMNOPQR') == key('ABCDEFGHIJKLMNOPQRmore')
assert key(' ExampleOnlyA1') != k  # leading password space survives
assert enc('\u0e01') == b'\xa1' and dec(b'\xdb') == '\uf8c1'
assert enc('\U0001f600') == b'?'
assert all(enc(dec(bytes([n]))) == bytes([n]) for n in range(256))
rejects(lambda: fixed('X'*19, 19))
rejects(lambda: Reader(b'abc').string())
rejects(lambda: Reader(b'\x01').number('H'))

uid = 0x0102030405060708
initial = fixed('DemoId', 19) + fixed('DEMO!', 19) + struct.pack('<q', k)
assert len(initial) == 46 and struct.unpack_from('<q', initial, 38)[0] == k
opaque12 = bytes(range(12))  # fabricated, never an issued ticket
login = cstr('DemoId') + cstr(str(k)) + b'VER?' + opaque12 + b'\x14'
r = Reader(login)
assert [r.string(), r.string(), r.take(4), r.take(12), r.number('B')] == [
    'DemoId', str(k), b'VER?', opaque12, 20]
assert r.p == len(login)
world = struct.pack('<BHH', 1, 2, 3) + fixed('DemoChannel', 33) + fixed('DemoWorld', 33) + struct.pack('<HH', 100, 7)
assert len(world) == 75
character = bytearray(156)
struct.pack_into('<QB', character, 0, uid, 2)
character[9:29] = fixed('DemoHero', 20)
struct.pack_into('<H', character, 29, 7)
struct.pack_into('<H', character, 43, 42)
assert (struct.unpack_from('<Q', character)[0], dec(character[9:29]).split('\0')[0], struct.unpack_from('<H', character, 43)[0]) == (uid, 'DemoHero', 42)
ticket = struct.pack('<QHI', uid, 0x1234, 0x89abcdef) + cstr('192.0.2.1') + struct.pack('<I7H', 9981, *range(7))
v, consumed = gs_ticket(ticket)
assert v == [uid, 0x1234, 0x89abcdef, '192.0.2.1', 9981, list(range(7))]
assert consumed == len(ticket)
for n in range(len(ticket)): rejects(lambda n=n: gs_ticket(ticket[:n]))
assert struct.pack('<QHI', *v[:3]).hex() == '08070605040302013412efcdab89'
assert len(struct.pack('<QIB', uid, 0x89abcdef, 2)) == 13

clock = struct.pack('<IHHH', 123456, 321, 100, 200)
assert struct.unpack('<IHHH', clock) == (123456, 321, 100, 200)
profile = bytearray(69)
profile[17:37] = fixed('DemoHero', 20)
struct.pack_into('<H', profile, 37, 7)
start = bytearray(251)
struct.pack_into('<12H', start, 184, *range(12))
for offset, value in [(208, 4), (212, 8), (214, 42), (219, 9)]:
    struct.pack_into('<H', start, offset, value)
for offset, value in [(231, 500), (235, 900), (239, 600), (243, 700), (247, 800)]:
    struct.pack_into('<I', start, offset, value)
assert struct.unpack_from('<12H', start, 184) == tuple(range(12))
assert struct.unpack_from('<H', start, 219)[0] == 9
assert [struct.unpack_from('<I', start, x)[0] for x in (231, 235, 239, 243, 247)] == [500, 900, 600, 700, 800]
assert key('ExampleOnlyA1') == -2896457993852450596
print('PASS: synthetic normalization, strings, packet widths, ticket truncations, and startup offsets')
```

## Method locators

Each row is a metadata-method/token → owning module → element/function → file-body join. Detailed instruction offsets appear with the claims above. `MoveNext` rows are the bodies of the named asynchronous operations. Most are in Spinel.Net.dll; UI rows use their named UI image, WebPage uses Spinel.Engine.Unity.dll, Cp874 uses Spinel.Formats.dll, and CombatModel uses Spinel.Game.dll. [D], [W]

| Method | Metadata index / token | WASM function / body file offset |
| --- | --- | --- |
| `PregameLoginModel.LogIn` | 8750 / `0x600222f` | 49293 / `0x107d722` |
| `<>c__DisplayClass42_0.<Enter>g__Submit\|8` | 18887 / `0x60008f8` | 47117 / `0xfccb55` |
| `LoginScreen.LauncherCheck` | 18859 / `0x60008dc` | 47094 / `0xfcbb8b` |
| `LoginKey.Derive` | 69175 / `0x60000eb` | 18212 / `0x4c7489` |
| `LoginKey.Text` | 69176 / `0x60000ec` | 18213 / `0x4c76cb` |
| `Md5.Hash` | 69179 / `0x60000ef` | 18019 / `0x4b8939` |
| `<SignInAsync>d__22.MoveNext` | 69254 / `0x600013a` | 18259 / `0x4ca469` |
| `<EnterWorldAsync>d__26.MoveNext` | 69252 / `0x6000138` | 18255 / `0x4c9540` |
| `Session.ParseCharacters` | 69239 / `0x600012b` | 18247 / `0x4c90bd` |
| `Session.ReadCharacter` | 69241 / `0x600012d` | 18047 / `0x4baa30` |
| `<SelectCharacterAsync>d__14.MoveNext` | 68996 / `0x6000038` | 18086 / `0x4bd5c1` |
| `<>c__DisplayClass2_0.<.ctor>b__0` | 69044 / `0x6000068` | 18133 / `0x4c1cf5` |
| `<LoginAsync>d__7.MoveNext` | 70512 / `0x6000624` | 16184 / `0x446dc3` |
| `<ConnectAsync>d__35.MoveNext` | 70609 / `0x6000685` | 16260 / `0x44c537` |
| `GameSession.Handle` | 70573 / `0x6000661` | 16232 / `0x44a1f3` |
| `Messages.ParseHandoff` | 71749 / `0x6000af9` | 17091 / `0x47d78f` |
| `<CompleteZoneChangeAsync>d__75.MoveNext` | 70607 / `0x6000683` | 16256 / `0x44bcf3` |
| `GameServerLink.SendHandoffLogin` | 70500 / `0x6000618` | 16174 / `0x44654e` |
| `Messages.ReadStartClock` | 71751 / `0x6000afb` | 17093 / `0x47d931` |
| `Messages.ReadStartZone` | 71753 / `0x6000afd` | 17095 / `0x47da0e` |
| `Messages.ReadStartProfile` | 71754 / `0x6000afe` | 17096 / `0x47da51` |
| `StartStatsWire.ParseStatBlock` | 73247 / `0x60010d3` | 15010 / `0x4037f4` |
| `StartStatsWire.ParseStartStats` | 73248 / `0x60010d4` | 15011 / `0x40391b` |
| `StartStatsWire.StartStats` | 73249 / `0x60010d5` | 15012 / `0x4039a8` |
| `CharacterStatus.ApplyStart` | 73279 / `0x60010f3` | 15026 / `0x4056f3` |
| `CombatModel.Load` | 36698 / `0x6000e5a` | 23353 / `0x6ae013` |
| `Characters.LockHash` | 68962 / `0x6000016` | 18018 / `0x4b8832` |
| `SecondPasswordGate.NeedsLock` | 72941 / `0x6000fa1` | 15532 / `0x422312` |
| `SecondPasswordGate.SendWithLock` | 72944 / `0x6000fa4` | 15533 / `0x422376` |
| `SecondPasswordGate.Write` | 72942 / `0x6000fa2` | 15433 / `0x41d69d` |
| `<LoadFirstLogin>d__6.MoveNext` | 77968 / `0x6000512` | 34089 / `0xa6fcc2` |
| `PacketWriter.Chars` | 69196 / `0x6000100` | 18227 / `0x4c8289` |
| `PacketWriter.Cstr` | 69195 / `0x60000ff` | 18044 / `0x4ba4af` |
| `PacketReader.Chars` | 69212 / `0x6000110` | 18052 / `0x4bad13` |
| `PacketReader.Cstr` | 69211 / `0x600010f` | 18058 / `0x4bba0e` |
| `Cp874.BuildHigh` | 83301 / `0x6000013` | 14269 / `0x3c29f8` |
| `Cp874.Encode` | 83304 / `0x6000016` | 14272 / `0x3c2dcd` |

## Sources

Target URLs are mutable; the body hashes above define this report's evidence. Tool/API documentation explains the parsing and disassembly methods, not the game's server policy.

[L]: https://tr-play.playall.in.th/Build/877598568ac7f0dc41389b0d6e411b8b.loader.js
[J]: https://tr-play.playall.in.th/Build/4937d6d47bade23f56e5ad4b5bf2e2b9.framework.js.gz
[D]: https://tr-play.playall.in.th/Build/9b2286992122d3c1cc081492dc31fee5.data.gz
[W]: https://tr-play.playall.in.th/Build/657890b2bd8548390dd7c7fad76093be.wasm.gz
[CFG]: https://tr-play.playall.in.th/spinel-config.json
[R1]: https://github.com/oDestroyeRo/opentrickster/blob/331f397fe2d6440bc24b9ee51f9b7caa6100962b/docs/research/target-protocol-framing.md
[R2]: https://github.com/oDestroyeRo/opentrickster/blob/6d996ef87a75bd8857ee998ddf30dcd99b80c1bf/docs/research/target-game-interfaces.md
[R3]: https://github.com/oDestroyeRo/opentrickster/blob/8920c260db742bbee93dd605be1d93450e4c428a/docs/research/live-action-correlation.md
[R4]: https://github.com/oDestroyeRo/opentrickster/blob/3a6c421d903483f1c977f6006fe01476d8da3223/docs/research/target-state-message-layouts.md
[C1]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppMetadata.cs
[C2]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppMethodDefinition.cs
[C3]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/BinaryStructures/Il2CppCodeGenModule.cs
[C4]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/BinaryStructures/Il2CppMetadataRegistration.cs
[C5]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/BinaryStructures/Il2CppType.cs
[C6]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/MetadataUsage.cs
[C7]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppFieldRef.cs
[C8]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppFieldDefaultValue.cs
[T1]: https://github.com/WebAssembly/wabt/tree/1.0.39
[T2]: https://github.com/WebAssembly/wabt/blob/1.0.39/README.md
[MD5]: https://www.rfc-editor.org/rfc/rfc1321
