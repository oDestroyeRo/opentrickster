# Target incoming state and action-result layouts

The goal is an implementation-ready specification for a macOS bot with a visible embedded game in client-assisted mode and an information dashboard in clientless mode, controlling the same single character. This report investigates [Recover incoming character and action-result message layouts](https://github.com/oDestroyeRo/opentrickster/issues/14), part of [Plan the two-mode Trickster bot MVP](https://github.com/oDestroyeRo/opentrickster/issues/2). It covers state needed for combat and digging. It does not implement a bot or automate quests.

**Result: the priority position and manual dig-result paths are recovered from network dispatch through client-state writes.** Additional supported layouts cover HP/MP, monster HP, target information, damage, and inventory changes. Entity appearance and automatic-digging state remain partial. The latest live candidates also have useful static explanations: `0x002f` is routed to another character's dig-start handling; active cancellation can produce nonzero `0x0030` payloads; `0x01dd` has a seven-byte gun-shot serializer; and `0x1587` has a separate system-timer sender. None establishes an accepted live action.

The research ticket remains open for the schema gaps listed below. These are static client contracts, not observed server guarantees. No login, game session, packet replay, target-code execution, private capture, browser storage, or credentials were used by this investigator. A handler named `OnAccepted`, an item-found caption, a damage animation, and a byte acknowledgement are different evidence from server acceptance or actual inventory receipt.

Research date is 2026-11-08. The clean starting revision was `6b8162f1f7a42b2bd9dd02c1b086be360914bb8d`, on prepared local branch `research/action-message-layouts`. Publication is limited to `research/target-state-message-layouts`. The authenticated user and existing assignee both matched `oDestroyeRo`; no reassignment was needed. The issue comment records the final commit and verified remote SHA.

## Evidence and revision boundary

“Extracted” means metadata records, public artifact bytes, or compiled instructions. “Derived” means a transcription or synthetic model. “Unknown” identifies a missing contract. There is no direct live observation by this investigator. The sanitized [live action correlation report][R3] was read as external observation evidence. Its temporal candidates are not promoted to established action/result pairs.

The [gateway decoder report][R1] supplies the supported framing, directional opcode maps, and metadata-v108 locator method. The orchestrator independently reproduced its four table fingerprints, inverse maps, six-byte frame, fragmentation/coalescing, and size/seed rejection cases at this revision. That proof is reused, not counted as a new execution here. This investigation independently rehashed the cached data, WASM, and metadata below and checked the additional method and field joins. It did not make a fresh deployment-drift check. [R1], [W], [D]

| Public input | Decoded bytes | SHA-256 checked here |
| --- | ---: | --- |
| [WASM][W] | 42,317,618 | `77a05d62cc5cc6fd5d1a1f5b4f4d25d8c0d1f9e8ea4740c08326f8ec83e2b446` |
| [Unity data][D] | 27,239,767 | `f1ab3ef3d2b6b04b28104bac55b25c007e78a24242a56c354b640428daf3053d` |
| Extracted metadata | 9,148,328 | `b7b7920f034cbdfb87eaf69557e17ea6b1513693acb9e905e918eaedbaf1a483` |

The `.gz` names in this cache hold decoded bodies. Metadata starts at data offset `16459759`, with magic `0xfab11baf` and version `108`. The pinned deployment build is `f5e972fa2be429602c20d6168013a20f`, Unity `6000.6.5f1`. Visible revision `6ab10889` is a separate identifier. These identifiers come from the matching artifact investigation and the cited live report, not a new screen observation here. A changed artifact fingerprint invalidates fixed locators and the build-seeded wire map. [R1], [R3]

All incoming opcodes below are **logical values after the inverse s2c map**. Do not put them directly in the wire-opcode field or apply the outbound c2s inverse. The gateway frame is `U16 payloadLength, U16 wireOpcode, U16 Aux, payload`, little-endian, with maximum payload length `32745`. Reassemble the byte stream across WebSocket chunks before translating or parsing. [R1]

Payload offsets are zero-based. `U8/U16/U32/U64` denote unsigned integers of those widths; multi-byte values are little-endian. `I16/I32` denote signed two's-complement values. “Consumed” does not mean the target enforces an exact whole-payload length. Most readers leave additional bytes untouched. Preserve unresolved tails and numeric values rather than inventing semantics.

## Metadata, code, and field locators

The analysis reused WABT 1.0.39 disassembly and the pinned [Cpp2IL v108 readers][C1]. No target module was instantiated. Metadata type, method, field, parameter, and image rows are respectively 66, 30, 10, 10, and 54 bytes in this build. The declaring type selects the owning image; token RID minus one indexes that image's method-pointer array. Each entry is a WASM element-table slot, which resolves to a function index. This is a record-to-instruction join, not a symbol-string guess. [R1], [C1], [C2], [C3], [D], [W]

| Image | Code-generation module in linear memory | Method count | Pointer array |
| --- | --- | ---: | --- |
| `Spinel.Net.dll` | `0x20ad44` | 4729 | `0x385d90` |
| `Spinel.Game.dll` | `0x20ad18` | 18411 | `0x372d70` |

Metadata registration is at `0x19e360`. Its type-pointer array is `0xd52c0`, with 41885 entries; its field-offset pointer array is `0x351ee0`, with 15196 entries. A type-definition entry points to that type's field-offset row. Index the row by field ordinal. Object offsets below include the managed header; unboxed value layouts do not. [C4], [C5], [D], [W]

Static opcode arrays were resolved through field-RVA metadata usage, a six-byte field reference containing type index and field ordinal, then a ten-byte field-default record containing field index, type index, and default-data offset. This recovers registration operands independently of parser switch cases. [C6], [C7], [C8], [D], [W]

## Incoming dispatch and Aux buffering

`GameServerLink..ctor` registers `Receive` through `Connection.OnAny` at `0x4462ac`. `GameSession.BindGameServer` attaches `GameSession.Handle` at `0x448663`. The gateway decoder therefore feeds these handlers through the established connection path. The unboxed `NetFrame` passed here has logical opcode at `+0`, Aux at `+4`, and payload-array reference at `+8`. [W] functions 16170, 16180, 16197; [R1]

`GameServerLink.Receive` supplies a client-side grouping use for nonzero Aux. This does not prove server transaction, atomicity, or replay guarantees. [W] 16180, `0x446913`, `0x446a14`, `0x446c78` onward.

| Input branch | Compiled behavior |
| --- | --- |
| `Aux == 0` | Deliver immediately, or backlog if no handler is attached. |
| Nonzero Aux, ordinary opcode | Append to that Aux group's list. |
| Nonzero Aux, `1951 / 0x079f` | Remove the group entry, deliver accumulated frames in list order, then deliver the marker. |
| Nonzero Aux, `1952 / 0x07a0` | Replace the group entry with an empty list, deliver the previous list, and consume the marker. |

Dropping Aux or immediately dispatching every complete frame differs from this client. Marker handling is under the nonzero-Aux branch; an Aux-zero marker is delivered like other Aux-zero frames. [W] 16180.

`GameSession.Handle` returns while `handoffPending` at `+71` is true. Otherwise it can record the start set, invokes `LateHooks.Arrive`, calls a snapshot of registered handlers from the opcode dictionary at `+28`, catches/logs handler exceptions, and then runs its built-in switch. Subscribers can therefore run before built-in state writes. [W] 16232 at `0x44a1f3`.

## Current character position

Logical 24, 25, 26, and 27 reach `Messages.ParseMove` at `0x44a815`, then `ApplyMove`. Only two variants include an origin. [W] 16232, 17085, 16233.

| Logical opcode | Sequential payload | Consumed |
| --- | --- | ---: |
| `24 / 0x18`, `26 / 0x1a` | `U16 Id, U16 FromX, U16 FromY, U16 ToX, U16 ToY, U8 Mode` | 11 |
| `25 / 0x19`, `27 / 0x1b` | `U16 Id, U16 ToX, U16 ToY, U8 Mode` | 7 |
| `729 / 0x2d9` | Same seven-byte form; separate dispatch emits movement only for another ID | 7 |
| `437 / 0x1b5` | `U16 Id, U16 X, U16 Y`, via `ParseEnterZone` | 6 |

`ParseMove` sets nullable `From` only for 24 or 26 and expands coordinates into integer `Pos` fields. `ApplyMove` first emits `GameEvent.Move`. If `message.Id == Me.Id`, it writes `message.To` into `Me.Pos` at `0x44b0a6` through `0x44b0ad`. Its local movement-timing updates do not prove a uniquely matched server acknowledgement. [W] 17085 at `0x47d3d1`, 16233 at `0x44af91`.

Own-ID opcode 25 is held at `GameSession +132` and queued through `JsMicrotasks.Queue(ApplyHeldStop)`. A previous held stop can be applied before another own movement message. Logical `748 / 0x2ec` clears it. Opcode 729 skips self and emits another actor's movement without the own-position write. [W] 16232 at `0x44a7f6` through `0x44a8c2`; 16234 at `0x44b192`.

Despite its name, `ParseEnterZone` does not read a zone ID in its first word. Logical 437 writes that word to **`Me.Id`**, then writes `Me.Pos`, at `0x44ad3a` and `0x44ad47`. The field-offset table distinguishes `Id` from `Zone`. [W] 17089, 16232; [D]

| Type / field-offset row | Relevant object fields |
| --- | --- |
| `MoveMessage`, type 12016 / `0x153d20` | `Id +8`, nullable `From +12`, `To +24`, `Mode +32` |
| `StartState`, type 12022 / `0x153d80` | `Id +8`, `Pos +12`, `Zone +20`, `Name +24` |
| `GameSession` / `0x152900` | `Me +92` |

This proves an incoming-message update separately from outbound `RequestMove`/`SendMove` and local avatar movement. It does not establish coordinate units, collision acceptance, freshness, or server authority. The live report's nine-byte **outbound** opcode-24 payload is not this eleven-byte **inbound** opcode-24 payload. Direction matters even when logical opcode values match. [R1], [R3], [W]

## Digging handlers and outcomes

`DrillFeature..ctor` registers these callbacks through `IGameSession.OnPacket`, interface slot 2, whose dispatch offset is `192 + 2*8 = 208`. The closures call concrete readers and state methods. [D] interface type 11857; [W] 25142.

| Logical opcode | Registered callback and action |
| --- | --- |
| `45 / 0x2d` | 22466 calls `OnAccepted`, without reading payload. |
| `46 / 0x2e` | 22467 clears flag bits `0x0c`, without decoding a result record. |
| `371 / 0x173` | 22468 calls `ParseSpot(payload, true)`, then `OnSpot`. |
| `50 / 0x32` | 22469 calls `ParseResult`, then conditionally `Result`. |
| `47 / 0x2f` | 22470 calls `ParseSpot(payload, false)`, skips own ID, then `OtherBegin`. |
| `51 / 0x33` | 22471 calls `ParseOtherResult`, skips own ID. Full schema not transcribed here. |
| `785 / 0x311` | 22457 reads U16 ID, calls `OnAutoFlag(Id, true)`. |
| `794 / 0x31a` | 22458 reads U16 ID, calls `OnAutoFlag(Id, false)`. |
| `792 / 0x318` | 22459 calls `OnAutoSpotTaken`; does not read payload. |
| `793 / 0x319` | 22460 reads U16 refusal value, calls `OnAutoSpotRefused`. Numeric meanings remain unknown. |

Own-spot and result registration operands occur at `0x76244b` and `0x762503`; other-character operands at `0x7625ba` and `0x762671`; automatic notifications at `0x761c65`, `0x761d1d`, `0x761dd5`, and `0x761e8d`. [W] 25142.

`OnAccepted` sets avatar-FSM state to 5 when necessary. If `AutoMode` is true and `Auto` is absent, it sets the auto flag and constructs `AutoDrill`. These are actual local state transitions from incoming logical 45, not just a method name. There is no actor ID or request ID read by this callback, so the empty `0x002d` candidate in [R3] cannot independently establish causal pairing to a particular send. [W] 25169 at `0x763a58`, state call `0x763a70`, AutoDrill construction `0x763ada`.

### Spot grammar

`ParseSpot` reads `U16 Id, U16 X, U16 Y, U8 branch`. If branch is nonzero, it reads `I16 Unusable`. Otherwise it reads `U8 BottomUnits, U8 ItemDepthUnits, U8 Nature`, multiplying the first two by 5. The own form then reads `U16 Key, U8 Stress`; the other-character form instead reads `U8 Auto`, interpreted as nonzero. [W] 15949 at `0x439b5d`.

Own-spot consumes 12 bytes for the unusable branch or 13 for the depth branch. Other-spot consumes 10 or 11 respectively. Thus the live candidate length 11 is compatible with the other-character depth form, but its actor and values were not provided and cannot be reconstructed. The handler explicitly skips own ID. Its repetition does not prove own progress or completion. `Nature`, `Unusable`, and depth units beyond the scaling operation remain unresolved. [R3]; [W] 15949, 22470.

`OnSpot` requires `spot.Id == Me.Id` and avatar-FSM state 5. It stores spot at `DrillFeature +68`, key at `+72`, and sets `infoReceived +64`. `DrillSpot`, type 11505, row `0x1522e0`, has `Id +8, X +12, Y +16, Unusable +20, Bottom +24, ItemDepth +28, Nature +32, Key +36, Stress +40, Auto +44`. This proves received spot information, not finished digging. [W] 25173 at `0x763f05`; [D]

### Own dig-result grammar and state writes

`ParseResult` reads seven bytes: `U8 kind` at 0, `U16 value` at 1, `U32 item/value` at 3. The last field is zero-extended into a managed 64-bit value, not read as U64. [W] 15950 at `0x439c55`.

Callback 22469 applies the result when avatar-FSM state is 5 or `AnyState(kind)` is true. `AnyState` is unsigned `(kind-11) < 9`, selecting 11 through 19. `Seconds(kind)` selects 11 through 16; only those kinds pass the U16 value as the fourth `Result` argument, otherwise zero. WASM `select` operand order was checked against the [core instruction specification][S1]. [W] 22469, 15942 at `0x4399a0`, 15943 at `0x4399ab`.

| Kind | Compiled interpretation in `DrillFeature.Result` |
| ---: | --- |
| 2, 9 | `Nothing` feedback. |
| 3 | Uses U32 value through `ItemName`; emits `DiggedOut` feedback and `etc_item_digout` sound. |
| 4 | `LowDrillGrade`. |
| 6 | `EmptyDrillLife`. |
| 7 | `NotDigableGround`. |
| 8, 19 | `CrazyDrillSucess` key and item-related feedback. Spelling is the client's. |
| 11 | `CrazyDrillStart`; resets stress and stores remaining milliseconds as `value * 1000`. |
| 12, 13, 14 | `CrazyDrillRemainTime`; stores remaining milliseconds as `value * 1000`. |
| 15 | `QuitByEquipDrill`. |
| 16 | `QuitByMapMove`. |
| 17 | `NoCrazyModeMap`. |
| 18 | `QuitByAutoDrillPet`. |
| Other values | No specialized branch established before the shared tail; preserve the number. |

These are client text keys/operations, not independently confirmed outcomes. The shared tail emits an own-result event containing current ID and kind, calls `EndDig(false)` at `0x76645f`, and stores `resultMark = 255` at `0x76649c`. Some crazy-mode branches can press again when the FSM and flags permit. Kind 3 does not itself establish an inventory insertion. Actual receipt needs independent inventory or ground-item evidence. [W] 25190 at `0x765910`.

`DrillFeature`, type 6133, row `0x137290`, has `flags +60, state +100, ElapsedMs +116, resultMark +120, starts13 +124, Crazy +154, CrazyRemainMs +156, Stress +160, AutoMode +192, Auto +196, AutoFlagOn +200`. These fields distinguish local timing/counters, automatic state, and incoming result-driven changes. [D], [W]

### Selected-point start and nonzero cancellation

`DrillView.Click` calls `DrillFeature.Press` at `0xe0d1c5`, passing a position and an `InWarp` result. `DigAssist.Start` also calls `Press` at `0x75d2ac`. `Press` reads the position's two floating-point coordinates, converts them to integers, obtains the current move mode from `moveMode`, calls `DigPayload`, and sends logical `44 / 0x2c`. Its five bytes are `U16 X, U16 Y, U8 mode`. The live final byte 2 can therefore be a mode supplied by this path; it is not by itself an “automatic” flag. Actual live caller attribution remains unproven. [W] 42594 at `0xe0d127`, 25076 at `0x75d0a0`, 25079 at `0x75d753`, calls `0x75d816` and `0x75d827`; [R3]

`Drill.CancelPayload(starts, elapsedMs)` writes two U16 values, retaining their low 16 bits. The previously inspected zero-argument branch is only one call site. In active avatar-FSM state 5, `DrillFeature.Cancel` calls `EndDig(true)` unless its internal state is 7, which takes `EndLocal`. `EndDig(true)` supplies signed integer `starts13 / 13` and `ElapsedMs` to `CancelPayload`, then sends logical `48 / 0x30`. This can produce a nonzero four-byte payload. `RightClick` and `CancelDrilling` also reach this active-end path. [W] 15946 at `0x439a56`; 25074 at `0x75cf15`, call `0x75cf7f`; 25086 at `0x75e2e3`, arguments `0x75e3bc` through `0x75e3cb`, send operand `0x75e3dc`; 25073, 25150.

Other pending-state branches in `Cancel` and `Withdraw` still send `CancelPayload(0,0)`. An own result calls `EndDig(false)`, which does not send that cancel message. The supplied live observation did not include raw values, so matching its nonzero payload to the two counters is an inference pending correlation. It must not be replaced with four zeros. This static explanation requires neither account access nor guessing the private bytes. [W] 25074, 25089 at `0x75e6d2`, 25086; [R3]

### Separate automatic-spot path

`AutoHost.RequestSpot` calls `AutoSpotPayload` at `0x66436c` and sends logical **791 / `0x317`**. `AutoSpotPayload` delegates to `DigPayload`, producing the same `U16 X, U16 Y, U8 mode` shape. Shared shape does not make it interchangeable with selected-point `0x2c`. `DrillView.Click` has a separate `AutoPress` call at `0xe0d264`. [W] 22438 at `0x664317`, 15952 at `0x439d9f`, 15945, 42594, 25165.

`OnAutoFlag` has an asymmetric own-ID branch: true can set `AutoFlagOn` if absent; own-ID false returns there rather than symmetrically clearing it. Other IDs emit an auto-flag event. `OnAutoSpotTaken` reaches `AutoDrill.SpotTaken`; refusal has additional branches. Their timers, reason values, and cleanup are not fully recovered. `ParseBang` is a separate parser used for logical 1074 and 1206 with different Boolean arguments, not the seven-byte manual result. [W] 25156 at `0x7632a7`, 25158 at `0x76348b`, 25159 at `0x7634c6`, 15955 at `0x439e24`, 22472/22473.

## Resource and combat messages

`Combat..cctor` initializes the following opcode array; `CombatLink..ctor` loops it to register `Receive`. Inclusion proves registration, not recovery of every schema. [W] 17974 at `0x4b561c`, 17975 at `0x4b567f`.

```text
31, 42, 43, 32, 36, 37, 38, 1030, 39, 286, 289, 163,
761, 763, 151, 91, 224, 292, 485, 1438, 923, 668, 719, 280
```

The array's metadata-usage cell is `0x3380c4`, field-reference index 245, field index 67973, and default-data absolute offset `0x4cb7f0`. `Receive` calls `Combat.ParseMessage` after certain swing/skill/item branches. It also collects by Aux and flushes, including a logical-1951 callback. `CombatModel` subscribes through `CombatLink.On`, routes message events through `OnEvent` and `Apply`, and can defer them through actor queues before `ApplyMessage`. Parsing does not imply immediate state application. [D]; [W] 17986 at `0x4b6512`, 17970 at `0x4b4fb3`, 17987; subscription `0x6adfac`, 23408, 23387, 23390.

| Logical opcode | Payload reads and parsed message |
| --- | --- |
| `36 / 0x24` | I32 at 0, `HpChanged`. |
| `37 / 0x25` | I32 at 0, `MpChanged`. |
| `38 / 0x26` | Four bytes at 0, widened unsigned to 64-bit `ExpChanged`. |
| `1030 / 0x406` | Same width/widening for `ExpGain`. |
| `32 / 0x20` | Padded U16 at 0, `AttackEnd` ID. |
| `43 / 0x2b` | Padded U16 ID at 0; optional U8 cause at 2, default zero; `Faint`. |
| `286 / 0x11e` | Padded U16 ID at 0, required I32 HP at 2, padded U16 Lives at 6. Full form 8 bytes. |
| `289 / 0x121` | Padded U16 ID at 0, required I32 HP at 2, required I32 MP at 6, padded U16 Stage at 10, optional U8 Boss at 12 interpreted as nonzero. Full form 13 bytes. |
| `42 / 0x2a` | U16 Target followed by the damage block below. |

“Padded” is specific to `Combat.U16At`, which supplies zero for absent bytes. `Combat.I32At` instead constructs a checked four-byte span and rejects truncation. Thus monster HP can accept length 6 with Lives zero, but not length 5. Target info can accept length 10 with Stage zero and Boss false. These are parser fallbacks, not proof that an omitted field's real value is zero. [W] 17968 at `0x4b4e89`, 17969 at `0x4b4ec8`, span helper 67999 at `0x173e4bb`, bounds failure `0x173e584`, read helper 64762 at `0x168aa51`.

`ApplyMessage` routes HP to `SetMyHp`, clamps at zero and at MaxHp when MaxHp is positive, writes Stats.Hp, and emits change. MP receives analogous clamping and a direct write. EXP messages replace or add to Exp. `CombatStats`, type 5951, row `0x136210`, has `Hp +16, MaxHp +24, Mp +32, MaxMp +40, MaxWeight +48, Exp +56`. HP/MP storage is floating-point after wire-integer conversion. Maxima are derived through status/tables in `CombatModel.Load`, not supplied by four-byte HP/MP updates. [W] 23390, HP call `0x6b1b4f`, MP write `0x6b1bc0`; 23368 write `0x6af070`; 23353 at `0x6ae013`; [D]

`MonsterHp` reaches `UpdateMonsterHp`, storing `max(0,Hp)` in the monster-HP dictionary keyed by ID and updating loaded views. Lives is not used by this updater. `TargetInfo` reaches HP-view logic; that does not prove a selected-target property update. `CombatModel`, type 5979, row `0x136380`, has `Stats +12, Target +60, Fainted +68, monsterHp +76, playerHp +80`. [W] 23367 at `0x6aee63`, 23390 at `0x6b1ef6` onward; [D]

### Damage block

Logical 42 calls `ParseDamage`, which reads Target then `ReadDamage`. Minimum consumed length is 26 bytes, or 28 with Guard. [W] 17963 at `0x4b4aa3`, 17964 at `0x4b4b2b`.

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 2 | U16 Target |
| 2 | 4 | I32 Amount |
| 6 | 4 | U32 read and discarded; meaning unknown |
| 10 | 4 | U32 KindBits; stored Kind is `KindBits & 127` |
| 14 | 4 | U32 Element |
| 18 | 4 | U32 Flags |
| 22 | 4 | U32 Extra |
| 26 | 2, optional | U16 Guard if at least two bytes remain, otherwise zero |

A single trailing byte after the prefix remains unread. Kind/element/flags/extra have no complete enum assignment here. Counter and guard-damage opcodes use different readers. In a qualifying actor branch, `ApplyDamage` subtracts nonnegative damage from own Stats.Hp if not fainted and calls `SetMyHp` at `0x6b09d0`; other branches update monster HP. These estimates coexist with direct HP messages. Do not treat every damage cue as an absolute snapshot or subtract again after a snapshot without ordering evidence. [W] 17964 through 17966, 17970; 23378 at `0x6b0758`.

### Alternate attack-control candidate

`GunFlowing.Click` checks `GunWorn` and calls `Shoot`. In its Ready branch, `Shoot` computes a check value through `Gun.Crc`, calls `Gun.Shot`, and sends logical **477 / `0x01dd`**. `Shot` serializes `U16 target, U16 crc, U16 bunch, U8 mode`, seven bytes. Parameter names were checked in metadata parameter rows, and the widths in actual writer calls. `Shoot` supplies its Bunch field and the caller's moveMode. This is a concrete alternate outbound combat path compatible with [R3]'s size/opcode; its exact live UI caller and hit outcome remain unproven. [W] 30105 at `0x91804b`, 30106 at `0x91806a`, serializer call `0x918117`, opcode `0x918129`; 16584 at `0x45fe32`; [D] methods 39598, 39599, 71029.

The compiled `Gun.Crc(gaugeSpeed,powRange,powStartPoint,key,critical)` operation is `((((powRange << 3) XOR (gaugeSpeed << 7) XOR powStartPoint) << 5) XOR key) OR critical`, masked to 16 bits, with WASM 32-bit wrapping shifts. This records a serializer dependency, not a recommendation to bypass the gun-state/ready logic. [W] 16583 at `0x45fe13`; [D] method 71028.

Logical **5511 / `0x1587`** is sent with null/empty payload by `SystemServerLink.Tick` through `Connection.Send` at `0x408309`. The surrounding timer code advances its due value by 60000. This establishes an independent timer sender, not an attack operation. The report does not infer the server's meaning or identify the live socket from timing alone. [W] 15070 at `0x4080db`; [D] method 73320.

## Inventory changes

`Inventory..ctor` registers single-change logical **561 / `0x231`** and **160 / `0xa0`**, whose callbacks use `ReadItemChange` then `Apply`. Logical **770 / `0x302`** first consumes/discards `U64, U32, U64`, then uses the same reader. The prefix's semantics remain unknown. [W] 16737; callbacks 16757 at `0x46a001`, 16759 at `0x46a130`, 16758 at `0x46a08b`.

List-update logical **262 / `0x106`, 246 / `0xf6`, 260 / `0x104`** come from a static array and reach callback 16764, `ParseItemUpdate`, and `Apply`. Its RVA usage cell is `0x338054`, field-reference index 217, field index 67945, default-data absolute offset `0x4cb168`. [W] 16737 at `0x4683cf` onward, 16764 at `0x46a4ba`, 16725 at `0x466b4e`; [D]

```text
ItemChange := U8 Op, U8 Kind, ItemBody(Kind), optional Options
ItemBody(1) := U32 ItemId, U64 Uid, U16 Count
ItemBody(2 or 3) := U32 ItemId, U64 Uid, U16 Count, U8 S0, U8 S1, U8 S2, U8 S3
ItemBody(other) := no further bytes; zero/default fields in this reader
Options := U8 N, N * (U32 Id, U32 A, U32 B)
Options is present only when Op is 2 or 4 AND Kind is 2.
ItemUpdateList := U16 Count, min(Count, 3) * ItemChange
```

This grammar is from functions 16631, 16632, and 16633. The four state bytes remain semantically unknown. Option A/B are U32 widened to managed 64-bit fields. The **three-change cap** is compiled behavior, not a proven server limit. Additional changes remain unread. Startup `ReadInventoryList` instead loops its full U16 count, reads Kind plus body without Op, and includes options for Kind 2. Do not use the capped delta grammar for startup. [W] 16725, 16722 at `0x466807`.

`Inventory.Apply` uses bitmasks 340 and 168 at `0x45eb16` and `0x45eb1f` to select operation sets. [W] 16560 at `0x45e8eb`.

| Op | State effect |
| --- | --- |
| 2, 4, 6, 8 | For Kind 1 or 2, replace/insert whole item in `byUid[Uid]`; other kinds do not take this branch. |
| 3, 5, 7 | Remove UID; clear matching equipment slots and emit their changes where applicable. |
| 1 | For an existing UID, copy State when Kind 2, otherwise update Count. Absent UID is ignored. |
| Other | No supported update branch here. |

`Inventory`, type 11918, row `0x153620`, has `byUid +12, slots +16, galderValue +24`. `InvItem`, type 11892, row `0x153550`, has `Uid +8, ItemId +16, Kind +24, Count +28, State +32, Options +36`. `ItemChange` type 11893, row `0x153568`, has `Op +8, Item +12`; `ItemOption` type 11891, row `0x153544`, has `Id +8, A +16, B +24`. Normal completion emits `ItemsChanged`. [D]; [W] 16560 at `0x45edd8` onward.

This is an incoming inventory reducer, not proof that a dig result is paired with an item update or that replay across resume is safe. Currency, complete equipment snapshots, and all item variants remain incomplete.

## Entity appearance and identity

Built-in logical **7, 8, 9** reach `ParseAppear`, then `GameEvent.Appear`. This does not directly replace Me. [W] 16232 at `0x44a56f` and `0x44a6ce`; 17081 at `0x47ca85`.

| Offset | Field |
| ---: | --- |
| 0 | U16 Id |
| 2 | U16 Type |
| 4 | U16 Kind |
| 6 | U8 Hair |
| 7 | U8 Job |
| 8 | I16 Level |
| 10 | U32 Flags |
| 14, 16 | U16 PosX, U16 PosY |
| 18, 20 | U16 DestX, U16 DestY |
| 22 onward | NUL-terminated name via `Cstr` and `Spinel.Formats.Cp874.Decode` |

After the name terminator come `U8 State, U8 Angle, U8 Speed, U32 ObjectState`, `EsBlock.Read`, and a discarded U32. Angle becomes `2*pi*byte/256`. `EsBlock.Read` first reads a presence byte; zero gives an empty block, nonzero enters eight categories with counts and table-dependent records. That complete variable schema is not recovered. [W] 17081 at `0x47cc69` through `0x47cda3`; 18058 at `0x4bba0e`, 14274, 16011 at `0x43c823`.

After the effect block and discarded U32, Type 1 reads a player body; Type 2 discards U16 then reads U32 HeadMark; Type 3 reads U16 Target; Types 4 and 16 read nullable U16 Owner. Type 16 can additionally read a 14-byte SoulShade body. Player-body and nonempty-effect parsing remain prerequisites for a general decoder. Exception handling can return partial appearance objects; its full fallback policy is not transcribed here. [W] 17081 at `0x47cdb6` onward, 17080 at `0x47c5dd`.

`AppearMessage` type 12013, row `0x153c80`, has `Id +8, Type +12, Kind +16, Level +20, Flags +32, Pos +40, Dest +48, Name +56, State +60, ObjectState +64, Angle +72, Speed +80, Player +84, Owner +88, HeadMark +96, Target +100, SoulShade +104, States +108, Unread +112`. Parser writes match these fields. [D], [W]

Downstream `MonsterTargets.OnAppear` skips own ID, calls `Register`, then `ApplyAppearTarget`. Register maintains an ID-keyed objects dictionary with Type, Kind, and Name, replacing incompatible prior records. ApplyAppearTarget uses Type 3's nonzero Target when target-type checks permit, calling Set or clearing a mark. Full subscription-to-world-instantiation and despawn paths remain incomplete. [W] 29109 at `0x8c0513`, 29110 at `0x8c05e1`, 29111 at `0x8c073d`; [D] type 6939, row `0x13b100`, `objects +20, marks +24`.

## Remaining facts and implementation boundary

The complete layouts above support offline parsers and typed incoming events. They do not establish a complete clientless state adapter. A native model should distinguish received snapshots, damage-derived estimates, outgoing intent, and local visual state. This is an implementation inference from the contracts, not additional target behavior.

| Missing fact | Next concrete evidence target |
| --- | --- |
| Initial own identity, zone, resources, maxima | Login state-machine function 16184 and StartState writes; `StartStatsWire.StartStats` 15012, `ParseStartStats` 15011, `ParseStatBlock` 15010; `CharacterStatus.ApplyStart` 15026; `CombatModel.Load` 23353. |
| General entity decoding | Finish `EsBlock.Read` 16011, `ReadPlayerBody` 17080, partial-object policy in 17081, then spawn/despawn consumers. |
| Automatic digging outcome/readiness | `OnAutoSpotTaken` 25158, `OnAutoSpotRefused` 25159, AutoDrill timers/cleanup, `ParseBang` 15955. |
| Complete combat ordering/enums | `CombatLink.Receive` 17986, `BundleEnd` 17987, skill/counter/guard readers, actor queues, damage-kind definitions. |
| Full inventory initialization | Equipment/currency snapshots and other item variants; deltas do not establish an initial inventory. |
| Live attribution and authority | Same-build decoded incoming ID/result plus independent resulting state. The supplied live report obtained no authoritative movement, combat, or completed dig outcome. |

The smallest next static prerequisite is a complete initial own-state/StartStats schema so these deltas have a known identity and resource baseline. For the live correlation worker, the immediate useful checks are own ID in the incoming movement variants and own-result logical 50 or own-spot logical 371, respecting Aux buffering. The empty 45 handler has no ID, and 47 skips self. Do not claim acceptance from a send, ACK, caption, or animation. [W], [R3]

## Locator appendix

These rows were checked against metadata, method-pointer arrays, the WASM element section, and function-body indices. Method is the zero-based metadata index; Table is the element slot; Body is a decoded WASM file offset. N means `Spinel.Net.dll`; G means `Spinel.Game.dll`. [D], [W]

| Image | Method and name | Token | Table | Function | Body |
| --- | --- | --- | ---: | ---: | --- |
| N | 70496 `GameServerLink..ctor` | `0x06000614` | 24332 | 16170 | `0x4461fb` |
| N | 70506 `GameServerLink.Receive` | `0x0600061e` | 24342 | 16180 | `0x44686e` |
| N | 70572 `GameSession.BindGameServer` | `0x06000660` | 24408 | 16197 | `0x4485b3` |
| N | 70573 `GameSession.Handle` | `0x06000661` | 24409 | 16232 | `0x44a1f3` |
| N | 70575 `GameSession.ApplyMove` | `0x06000663` | 24411 | 16233 | `0x44af91` |
| N | 71743 `Messages.ParseMove` | `0x06000af3` | 25572 | 17085 | `0x47d3d1` |
| N | 71747 `Messages.ParseEnterZone` | `0x06000af7` | 25576 | 17089 | `0x47d65d` |
| N | 71739 `Messages.ParseAppear` | `0x06000aef` | 25568 | 17081 | `0x47ca85` |
| N | 70168 `Drill.ParseSpot` | `0x060004cc` | 24004 | 15949 | `0x439b5d` |
| N | 70169 `Drill.ParseResult` | `0x060004cd` | 24005 | 15950 | `0x439c55` |
| N | 70171 `Drill.AutoSpotPayload` | `0x060004cf` | 24007 | 15952 | `0x439d9f` |
| G | 37535 `DrillFeature..ctor` | `0x0600119f` | 8716 | 25142 | `0x7618c2` |
| G | 37660 `<>c__DisplayClass122_0.<.ctor>b__12` | `0x0600121c` | 8841 | 22468 | `0x664c1d` |
| G | 37661 `<>c__DisplayClass122_0.<.ctor>b__13` | `0x0600121d` | 8842 | 22469 | `0x664c37` |
| G | 37594 `DrillFeature.Result` | `0x060011da` | 8775 | 25190 | `0x765910` |
| G | 37629 `AutoHost.RequestSpot` | `0x060011fd` | 8810 | 22438 | `0x664317` |
| N | 69836 `Combat..cctor` | `0x06000380` | 23672 | 17974 | `0x4b561c` |
| N | 69837 `CombatLink..ctor` | `0x06000381` | 23673 | 17975 | `0x4b567f` |
| N | 69848 `CombatLink.Receive` | `0x0600038c` | 23684 | 17986 | `0x4b6512` |
| N | 69825 `Combat.ParseDamage` | `0x06000375` | 23661 | 17963 | `0x4b4aa3` |
| N | 69828 `Combat.ReadDamage` | `0x06000378` | 23664 | 17964 | `0x4b4b2b` |
| G | 36733 `CombatModel.ApplyMessage` | `0x06000e7d` | 7921 | 23390 | `0x6b1682` |
| G | 36709 `CombatModel.SetMyHp` | `0x06000e65` | 7898 | 23368 | `0x6aefeb` |
| G | 36708 `CombatModel.UpdateMonsterHp` | `0x06000e64` | 7897 | 23367 | `0x6aee63` |
| N | 71280 `Inventory..ctor` | `0x06000924` | 25109 | 16737 | `0x467455` |
| N | 71249 `Items.ReadItemChange` | `0x06000905` | 25078 | 16631 | `0x461e30` |
| N | 71247 `Items.ReadItemBody` | `0x06000903` | 25076 | 16632 | `0x461ed3` |
| N | 71248 `Items.ReadOptions` | `0x06000904` | 25077 | 16633 | `0x46206f` |
| N | 71253 `Items.ParseItemUpdate` | `0x06000909` | 25082 | 16725 | `0x466b4e` |
| N | 71290 `Inventory.Apply` | `0x0600092e` | 25119 | 16560 | `0x45e8eb` |
| N | 71314 `Inventory.<.ctor>b__5_21` | `0x06000946` | 25143 | 16764 | `0x46a4ba` |
| G | 37573 `DrillFeature.OnAccepted` | `0x060011c5` | 8754 | 25169 | `0x763a58` |
| G | 37549 `DrillFeature.Cancel` | `0x060011ad` | 8730 | 25074 | `0x75cf15` |
| G | 37578 `DrillFeature.EndDig` | `0x060011ca` | 8759 | 25086 | `0x75e2e3` |
| N | 70165 `Drill.CancelPayload` | `0x060004c9` | 24001 | 15946 | `0x439a56` |
| G | 39598 `GunFlowing.Click` | `0x060019ae` | 10778 | 30105 | `0x91804b` |
| G | 39599 `GunFlowing.Shoot` | `0x060019af` | 10779 | 30106 | `0x91806a` |
| N | 71029 `Gun.Shot` | `0x06000829` | 24865 | 16584 | `0x45fe32` |
| N | 71028 `Gun.Crc` | `0x06000828` | 24864 | 16583 | `0x45fe13` |
| N | 73320 `SystemServerLink.Tick` | `0x0600111c` | 27148 | 15070 | `0x4080db` |

## Synthetic checks and reproduction

The following standalone Python transcription runs 38 checks. Save the fenced block outside Git and run it with Python 3. It checks handwritten synthetic movement, enter-position, own/other spot, result, damage, monster-HP, target-info, and item-change payloads, truncation and tail behavior, the inventory reducer subset, and the newly traced cancel/gun serialization examples. All identifiers and values are synthetic. It does not execute target code, emulate the server, validate live acceptance, or prove the unimplemented entity and bootstrap branches.

The Build the Lever principle shaped the verification choice: reusable readers and assertions replaced unrepeatable manual byte calculations. The checks prove that the documented model and fixtures agree. The separately inspected instructions, registrations, and field writes are what support the model's correspondence to this target.

```python
import struct

class Reader:
    def __init__(self, data):
        self.data, self.offset = data, 0
    def read(self, fmt):
        size = struct.calcsize('<' + fmt)
        if self.offset + size > len(self.data):
            raise ValueError('truncated')
        result = struct.unpack_from('<' + fmt, self.data, self.offset)
        self.offset += size
        return result[0] if len(result) == 1 else result
    def tail(self):
        return self.data[self.offset:]

def move(data, opcode):
    r = Reader(data)
    ident = r.read('H')
    origin = r.read('HH') if opcode in (24, 26) else None
    return (ident, origin, r.read('HH'), r.read('B')), r.tail()

def spot(data, own):
    r = Reader(data)
    fields = dict(zip(('id', 'x', 'y'), r.read('HHH')))
    if r.read('B'):
        fields['unusable'] = r.read('h')
    else:
        fields.update(bottom=5*r.read('B'), depth=5*r.read('B'), nature=r.read('B'))
    if own:
        fields.update(key=r.read('H'), stress=r.read('B'))
    else:
        fields['auto'] = bool(r.read('B'))
    return fields, r.tail()

def dig_result(data):
    r = Reader(data)
    kind, value, item = r.read('BHI')
    return (kind, value, item, value if 11 <= kind <= 16 else 0), r.tail()

def damage(data):
    r = Reader(data)
    ident, amount, unknown, bits, element, flags, extra = r.read('HiIIIII')
    guard = r.read('H') if len(r.tail()) >= 2 else 0
    return (ident, amount, unknown, bits & 127, bits, element, flags, extra, guard), r.tail()

def padded_u16(data, offset):
    return int.from_bytes(data[offset:offset+2], 'little')

def monster_hp(data):
    r = Reader(data)
    r.offset = 2
    return padded_u16(data, 0), r.read('i'), padded_u16(data, 6)

def target_info(data):
    r = Reader(data)
    r.offset = 2
    hp, mp = r.read('ii')
    return padded_u16(data, 0), hp, mp, padded_u16(data, 10), bool(data[12]) if len(data) > 12 else False

def item_change(r):
    op, kind = r.read('BB')
    item = dict(kind=kind, id=0, uid=0, count=0, state=None, options=[])
    if kind in (1, 2, 3):
        item.update(zip(('id', 'uid', 'count'), r.read('IQH')))
        if kind in (2, 3):
            item['state'] = r.read('BBBB')
    if op in (2, 4) and kind == 2:
        item['options'] = [r.read('III') for _ in range(r.read('B'))]
    return op, item

def item_updates(data):
    r = Reader(data)
    count = r.read('H')
    return [item_change(r) for _ in range(min(count, 3))], r.tail()

def apply_item(items, change):
    op, item = change
    uid = item['uid']
    if op in (2, 4, 6, 8) and item['kind'] in (1, 2):
        items[uid] = dict(item)
    elif op in (3, 5, 7):
        items.pop(uid, None)
    elif op == 1 and uid in items:
        key = 'state' if item['kind'] == 2 else 'count'
        items[uid][key] = item[key]

# Handwritten bytes: synthetic values, payloads only, no target execution.
checks = 0

def check(actual, expected):
    global checks
    assert actual == expected, (actual, expected)
    checks += 1

def rejects(fn):
    global checks
    try:
        fn()
    except ValueError:
        checks += 1
        return
    raise AssertionError('expected truncation')

long_move = bytes.fromhex('3412 6400 c800 6500 c900 02')
short_move = bytes.fromhex('3412 6500 c900 02')
for op in (24, 26):
    check(move(long_move, op), ((0x1234, (100, 200), (101, 201), 2), b''))
for op in (25, 27, 729):
    check(move(short_move, op), ((0x1234, None, (101, 201), 2), b''))
rejects(lambda: move(short_move[:-1], 25))
check(move(short_move+b'\xff', 25)[1], b'\xff')
check(Reader(bytes.fromhex('3412 6400 c800')).read('HHH'), (0x1234, 100, 200))
check(spot(bytes.fromhex('3412 6400 c800 00 0a 06 02 cdab 07'), True),
      ({'id':0x1234,'x':100,'y':200,'bottom':50,'depth':30,'nature':2,'key':0xabcd,'stress':7}, b''))
check(spot(bytes.fromhex('3412 6400 c800 01 feff cdab 07'), True)[0]['unusable'], -2)
check(spot(bytes.fromhex('3412 6400 c800 00 0a 06 02 01'), False)[0]['auto'], True)
check(spot(bytes.fromhex('3412 6400 c800 01 feff 00'), False)[0]['auto'], False)
rejects(lambda: spot(bytes.fromhex('3412 6400 c800 00'), True))
check(dig_result(bytes.fromhex('03 0000 78563412')), ((3,0,0x12345678,0), b''))
check(dig_result(bytes.fromhex('0b 1e00 00000000')), ((11,30,0,30), b''))
check(dig_result(bytes.fromhex('03 0000 ffffffff'))[0][2], 4294967295)
rejects(lambda: dig_result(bytes.fromhex('03 0000')))
d = bytes.fromhex('3412 19000000 00000000 81000000 02000000 04000000 06000000 0900')
check(damage(d), ((0x1234,25,0,1,129,2,4,6,9), b''))
check(damage(d[:26])[0][-1], 0)
check(damage(d[:27])[1], b'\x09')
rejects(lambda: damage(d[:25]))
check(monster_hp(bytes.fromhex('3412 90010000 0200')), (0x1234,400,2))
check(monster_hp(bytes.fromhex('3412 90010000')), (0x1234,400,0))
check(monster_hp(bytes.fromhex('3412 90010000 ff')), (0x1234,400,255))
rejects(lambda: monster_hp(bytes.fromhex('3412 900100')))
check(target_info(bytes.fromhex('3412 90010000 32000000 0300 01')), (0x1234,400,50,3,True))
check(target_info(bytes.fromhex('3412 90010000 32000000')), (0x1234,400,50,0,False))
check(Reader(bytes.fromhex('fbffffff')).read('i'), -5)
item = bytes.fromhex('02 02 78563412 0807060504030201 0300 01020304 01 09000000 0a000000 0b000000')
r = Reader(item)
c = item_change(r)
check(c, (2, {'kind':2,'id':0x12345678,'uid':0x0102030405060708,'count':3,
              'state':(1,2,3,4),'options':[(9,10,11)]}))
check(r.tail(), b'')
rejects(lambda: item_change(Reader(item[:-1])))
items = {}
apply_item(items, c)
check(items[c[1]['uid']]['count'], 3)
apply_item(items, (1, dict(c[1], state=(5,6,7,8), count=99)))
check((items[c[1]['uid']]['state'],items[c[1]['uid']]['count']), ((5,6,7,8),3))
apply_item(items, (3, c[1]))
check(items, {})
# Unknown kind consumes just op+kind; client list reader consumes <=3 entries.
check(item_updates(bytes.fromhex('0400 0063 0063 0063 0063'))[1], bytes.fromhex('0063'))
check(struct.pack('<HH', 2, 1500), bytes.fromhex('0200 dc05'))
crc = (((5 << 3) ^ (3 << 7) ^ 7) << 5) ^ 0x1234
check((crc | 1) & 65535, 0x27d5)
check(struct.pack('<HHHB', 0x1234, 0x27d5, 2, 2), bytes.fromhex('3412 d527 0200 02'))
print(f'PASS: {checks} synthetic payload and reducer checks')
```

For static reproduction, use the exact public WASM and data hashes above, extract the metadata slice, and follow the decoder report's standalone locator. Extend its selected owners to the appendix methods and select the two code-generation modules listed here. Resolve each method token through the pointer array and element table, and inspect the resulting WABT disassembly. Branch-table labels refer to enclosing WASM blocks, not opcode numbers; resolve the nesting before assigning a case. Verify state writes using the field-offset rows rather than guessed object layouts. No target execution is needed. [R1], [C1], [S1]

Local verification passed for 40 metadata/method-pointer/element/function joins and 11 field-offset rows. The payload transcription passed all 38 checks. Report readback, reference resolution, whitespace, one-file scope, and committed-content checks are part of delivery. The issue comment records the actual committed and pushed revision. No product build, live gameplay check, server-authority check, or independent reviewer ran in this account-free single-worker task.

## Sources

Target URLs are mutable; the fingerprints identify the inspected revision. Tool documentation supports interpretation of the formats. Protocol claims come from the target's own compiled artifacts, with exact locators above.

[W]: https://tr-play.playall.in.th/Build/657890b2bd8548390dd7c7fad76093be.wasm.gz
[D]: https://tr-play.playall.in.th/Build/9b2286992122d3c1cc081492dc31fee5.data.gz
[R1]: https://github.com/oDestroyeRo/opentrickster/blob/331f397fe2d6440bc24b9ee51f9b7caa6100962b/docs/research/target-protocol-framing.md
[R3]: https://github.com/oDestroyeRo/opentrickster/blob/8920c260db742bbee93dd605be1d93450e4c428a/docs/research/live-action-correlation.md
[C1]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppMetadata.cs
[C2]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppMethodDefinition.cs
[C3]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/BinaryStructures/Il2CppCodeGenModule.cs
[C4]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/BinaryStructures/Il2CppMetadataRegistration.cs
[C5]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/BinaryStructures/Il2CppType.cs
[C6]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/MetadataUsage.cs
[C7]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppFieldRef.cs
[C8]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppFieldDefaultValue.cs
[S1]: https://webassembly.github.io/spec/core/exec/instructions.html
