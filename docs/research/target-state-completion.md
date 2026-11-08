# Incoming state completion: entities, automatic digging, and inventory

This report answers the bounded static-source question in [issue 14][I14], under the [MVP map][MAP]. It extends the [original incoming-state report][R14], reuses the [startup baseline][R15] and [framing report][R13], and accounts for the deployment change recorded in [the native preflight report][R17]. The destination remains a macOS Rust/GPUI specification: an embedded game in client-assisted mode, an information dashboard in clientless mode, one existing character, combat and digging. Quest automation remains outside the MVP. No application is implemented here.

**Result:** the selected entity tails, automatic-dig notifications/state transitions, startup inventory list/equipment fields, and inventory reducers have recoverable contracts in both inspected builds. A dig result, a ground item, a pickup caption, and an inventory mutation are separate observations. The client has no inventory-complete wire sentinel in the inspected startup path. Neither these contracts nor a synthetic pass establish authoritative gameplay, a complete live inventory, or a working clientless session.

The work began clean at `6b8162f1f7a42b2bd9dd02c1b086be360914bb8d`, branch `research/target-state-completion`. Only this report is committed. Public caches were read-only; all new tools, disassembly, fixtures and outputs are under `/tmp/opentrickster-state-completion`. No credentials, browser state, private captures, probe scripts, private-scan files, game sockets, authentication, target execution, or account/session artifacts were accessed. No subagents were used. Research and Prove It Works were applied directly; source readback and offline structure checks are the appropriate proof for this document, rather than an application build.

## Revision and evidence boundary

The original traces use build **O**, `f5e972fa2be429602c20d6168013a20f`. During this task the deployment changed to build **C**, `8502ebbf2fc9033d2fba50d02fd92002`, reported at 2026-11-08 16:46 UTC. “Current” below means that newly supplied, fingerprinted build, not a promise that the mutable website still serves it. Both decoded artifact pairs and both metadata slices were independently hashed here. Files ending `.gz` in the supplied caches contain decoded bodies. [R17]

| Artifact | O bytes / SHA-256 | C bytes / SHA-256 |
| --- | --- | --- |
| WASM | 42,317,618 / `77a05d62cc5cc6fd5d1a1f5b4f4d25d8c0d1f9e8ea4740c08326f8ec83e2b446` | 43,136,343 / `381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404` |
| Unity data | 27,239,767 / `f1ab3ef3d2b6b04b28104bac55b25c007e78a24242a56c354b640428daf3053d` | 27,724,487 / `fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474` |
| Metadata | 9,148,328 / `b7b7920f034cbdfb87eaf69557e17ea6b1513693acb9e905e918eaedbaf1a483` | 9,343,896 / `7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade` |

Metadata begins at data offset **16459759** in O and **16748911** in C. Both have magic/version `0xfab11baf / 108`, 30-byte method rows and 66-byte type rows. Method row: name at 0, declaring type U16 at 4, token at 18; type row: name at 0, namespace at 4. `(token RID - 1)` indexes the owning module pointer array, yielding an element-table slot, then a WASM function. Function-body offsets in the appendix are offsets in the decoded WASM, not virtual addresses. [DO], [DC], [WO], [WC]

| Registration | O | C |
| --- | --- | --- |
| Net: module / count / pointer array | `0x20ad44 / 4729 / 0x385d90` | `0x212548 / 4791 / 0x392c70` |
| Game | `0x20ad18 / 18411 / 0x372d70` | `0x21251c / 18796 / 0x37f5a0` |
| UI.Pc | `0x20ad9c / 16433 / 0x39b730` | `0x2125a0 / 16606 / 0x3a9600` |
| Runtime type-pointer array | `0xd52c0` | `0xd75a0` |
| Type field-offset pointer array | `0x351ee0` | `0x35d850` |
| Imported functions | 484 | 490 |

Methods were paired by module, qualified declaring/nested type, method and parameter count, then located independently in C. A normalized instruction comparison retained wire constants, branches, reader-call identities and field offsets; it abstracted relocation addresses, metadata-use cells and unnamed generated helpers. **That comparison is a review aid, not a semantic-equivalence proof.** Changed paths were inspected, field offsets resolved, and the actual effect/RVA tables compared. C has these relevant differences:

- `CombatModel` moved `monsterHp` from +76 to **+88**, `playerHp` +80 to **+92**, `attackTargets` +92 to **+104**, and `fading` +124 to **+136**. `Target +60` and `Fainted +68` are unchanged. Faint handling now includes the own-faint operations inline and clears a new `rebirthAsked +84`; restart/rebirth behavior is outside this report's contract.
- `DrillFeature.ItemThrown` retains the own-ID gate and AutoDrill notification, and additionally invokes **ThrownByMe +224** with the ground-item ID. The UI callback moved from O `DrillUi...<Install>b__16`/42568 to C `b__18`/44717; its actual ItemEvent-14 path was inspected, not paired by callback suffix.
- `GameSession.recordingStartSet` moved +69 to **+70**, `Me` +92 to **+96**, and `late` +100 to **+104**. A new `GmHello.Of` call appears before recording/replaying the backlog. That bootstrap hook is explicitly left to the independent gateway/authentication revalidation work. It is not evidence of a new inventory-complete marker.

The selected wire grammars and AutoDrill branch constants below agree in O and C. This does **not** revalidate gateway maps, authentication, all startup traffic, arbitrary combat skills/PvP, or every UI/engine consumer. The original manual digging, resource and damage evidence remains linked rather than repeated merely to increase check counts. Their selected shared readers were compared where needed here; the whole prior report is not certified for C.

Unless explicitly identified as SystemServer traffic, opcodes below are **logical GameServer opcodes**, with direction stated. `U8/U16/U32/U64` are wire widths; multi-byte values are little-endian. `Cstr` means the target's NUL-terminated CP874 string, not UTF-8. Managed 64-bit fields often receive U32 wire values. A consumed prefix does not imply the client enforces exact packet length. Aux grouping is as documented in [R14]: zero delivers immediately; nonzero groups await 1951/1952 handling. This is not server transaction atomicity. Current transport revalidation belongs to the separate worker.

## Appearance, identity, and targeting

S→C **7, 8, 9** enter `GameSession.Handle`, call `Messages.ParseAppear`, and emit `GameEvent.Appear`. In C the parser call is at **0x3affc5**. The common prefix is:

| Offset | Wire field |
| ---: | --- |
| 0, 2, 4 | U16 Id, Type, Kind |
| 6, 7, 8 | U8 Hair, U8 Job, I16 Level |
| 10 | U32 Flags |
| 14, 16, 18, 20 | U16 PosX, PosY, DestX, DestY |
| 22 onward | Cstr Name; U8 State, Angle, Speed; U32 ObjectState; EsBlock; discarded U32; typed tail |

Angle is converted to `2*pi*byte/256`. These are observed object fields, not navigation prediction. `AppearMessage` has Id +8, Type +12, Kind +16, Level +20, Flags +32, Pos +40, Dest +48, Name +56, State +60, ObjectState +64, Angle +72, Speed +80, Player +84, Owner +88, HeadMark +96, Target +100, SoulShade +104, States +108, Unread +112. O type/field row: `12013 / 0x153c80`; C: **12191 / 0x158fa0**. [WO], [WC], [DO], [DC]

### Effect block

`EsBlock.Read` consumes U8 presence. Zero ends the block. Nonzero means **eight groups**, numbered 0–7; each group starts with U8 entry count. Each entry has the nine-byte prefix `U16 Esa, U16 Instance, U8 V14, U16 Skill, U16 V04`, followed by a body determined by Esa. Descriptor **0 = U8, 1 = U16, 5 = bytes through NUL, 2/3/4 = U32**. Descriptor 5 skips rather than decodes; if its NUL is missing the target advances to end-of-buffer. The complete compiled body table is included as `ES` in the verification block. Its 47 keys and descriptor values were extracted separately from both cctors and compared, including the backing RVA data. Names/meanings of V14/V04 and the effect values are not recovered. [WO] 16011/16014; [WC] 10388/10391.

Esa **1425** overrides the table: `U16 n, n*U32, U32 m, m*U16`, then **one extra U16 iff n != 0**. The entry's `Body` stores exactly the consumed body bytes. For an Esa absent from the table the target consumes **zero body bytes**; it does not read a length or skip an unknown extension. Thus unseen effects are not a length-delimited extension mechanism. The strict example below quarantines unclassified keys and malformed strings; that stricter decision is proposed adapter behavior, not a claim that the target rejects them.

`EsEntry` fields Group +8, Esa +12, Instance +16, V14 +20, Skill +24, V04 +28, Body +32 are unchanged; O type/row `11532 / 0x152410`, C **11699 / 0x157640**. The four copied descriptor arrays are at O metadata `0x4caed0, 0x4cb3d8, 0x4cb308, 0x4cb348`; C **0x4e6f48, 0x4e7450, 0x4e7380, 0x4e73c0**, each 12 bytes. Other table values are cctor immediates. [DO], [DC]

### Player and other typed tails

After the discarded U32, the type discriminator chooses:

| Type | Tail / interpretation supported by consumers |
| --- | --- |
| 1 | PlayerBody below. Player actors, not the basic monster attack allowlist. |
| 2 | discarded U16, U32 HeadMark. The static `NpcTypes` set is `{2,5,14}`. This is categorization, not proof of a particular interaction being legal. |
| 3 | U16 Target. This is the **monster's target**, not the local player's selected target. |
| 4 or 16 | U16 Owner, stored as a present nullable value. There is no nullable-presence byte on the wire. Compare Owner to the known own actor ID to classify ownership. |
| 16 additionally | If remaining bytes >=14: U8 Element, U32 Value1452, U32 Value1448, U32 Exp, U8 Variant. Otherwise the target leaves the short optional tail unread. |
| Other | No typed read in this switch. Preserve classification/unknown bytes; do not call every remaining type a monster. |

`ReadPlayerBody` consumes, in order:

```text
U8 Attributes; U32 AlternateKind; U8 mask
if mask&1:  Cstr HeadNotice
if mask&2:  Cstr CardBattleNotice
if mask&4:  Cstr ShopSign
if mask&8:  discarded U8; Cstr RoomTitle
if mask&16: discarded Cstr
U32 A_mask; one U32 item value per enabled A bit, in table order
U32 B_mask; one U32 item value per enabled B bit, in table order
discarded U8; Cstr GuildName; Cstr GuildTitle
U8 EmblemTemplate; U8 EmblemColor; U32 EmblemMark; U8 League; U8 Marker
if mask&32: U32 n; skip n*4 bytes (n >= 0x20000000 throws overflow)
U16 PkKills; U16 PkDeaths
if mask&64: discarded Cstr
U16 TitleType; U32 TitleId; U32 TitleExtra; discarded U8; discarded U16
if TitleType is 4 or 6: Cstr TitlePartner
U8 ShopStand
```

A writes `Worn[slot]` in a 23-element array. Equipment starts as a **clone of Worn**, then B overrides selected slots. The mask/value mapping is not a dense bit-to-slot mapping:

```text
A bits : 1,2,4,32,64,128,256,512,1024,2048,4096,8192,16384,32768,65536,262144,524288
A slots: 0,1,2, 5, 6, 10, 11, 12,  13,  14,  15,  16,   17,   18,   19,     7,    21
B bits : 1,2,4,4096,8192,524288
B slots: 0,1,2,  15,  16,    21
```

Both builds' data arrays compare equal. O offsets A bits/slots, B bits/slots are `0x4cb658 / 0x4cb7a8 / 0x4cb0b8 / 0x4cb870`; C offsets are **0x4e76d0 / 0x4e7820 / 0x4e7130 / 0x4e78e8**. Lengths are 136, 68, 48, 24 bytes: bits are stored as U64 constants, slots as U32; packet masks and values are still U32. C usage cells are `0x3430b4 / 0x3430c8 / 0x343050 / 0x3430d8`. No branch consumes mask bit 128 or A/B bits outside these lists. The strict fixture rejects them; the target ignores them. [DO], [DC]; cctors O17098/C12346.

An empty-name appearance with no effects has a 35-byte common portion. Minimum PlayerBody is **43 bytes**, so a minimal type-1 appearance is 78 bytes; a type-3 monster is 37, type-2 NPC 41, owned type-4 37, and type-16 with the additional shade body 51. These sizes are branch-specific fixtures, not universal fixed packet sizes.

`ParseAppear` is permissive in ways a state adapter must expose: the fixed prefix is outside the effect catch; an effect `PacketException` can return a partial appearance; the discarded U32 after effects is outside the typed-tail catch; the typed tail catches `PacketException`/`OverflowException` and can return partial state. `Unread` is populated only for player/owner forms, so its default zero does **not** prove a monster packet was completely consumed. Proposed MVP rule: retain raw bytes and an explicit complete/partial classification; never make a malformed partial actor actionable merely because ID/type parsed. [WO] 17081; [WC] 12329.

### State writes and removal

`MonsterTargets.OnAppear` skips own ID, registers ID→Type/Kind/Name, replaces incompatible identities, then handles the type-3 nonzero Target. That target must resolve to known type 1 or 3; otherwise its mark is cleared. Destruction removes the registry entry and referencing marks. `GameplayScreen.Appear` creates/reuses the actor/view, places it at received Pos, and follows movement when Dest differs; incompatible actor types are replaced. `RemoveActor` disposes the view and removes actor dictionaries. These visual/interpolation operations are separate from the received position fields. See paired locator appendix. [WO], [WC]

S→C **15/16** emit Disappear with Id (the client tolerates a missing/one-byte U16, padding absent bytes with zero). **6/21** strictly read U16 Id, skip own ID, and emit Disappear with Warp=true. These forms must not be mistaken for an HP/death result. Movement **24/26** is `U16 Id, U16 FromX, FromY, ToX, ToY, U8 Mode` (11 bytes); **25/27**, and the separately dispatched **729**, use `U16 Id, U16 ToX, ToY, U8 Mode` (7 bytes). 729 skips own ID. `ApplyMove` emits the event and writes own Pos only after comparing actor ID. The prior report's held-own-stop behavior remains relevant. Current `ParseMove` retains the reader/branch sequence; current Handle/ApplyMove use the relocated Me field. [R14]; O17085/16233, paired C locators below.

Basic combat targeting is narrower than the pick helpers. `ObjectPick.TargetingMask` accepts types 1–7 and 14; `Aimable` excludes only 4/16. `CombatModel.WorldClick` requires the relevant click path, own not fainted, a present pick of **Type 3**, and an ID absent from `fading`; after an optional GunClick interception it stores `Target +60` and calls `CombatLink.Attack`. C calls Attack at **0x53e236**. Thus “aimable” is not an attack allowlist, nor is a monster candidate proof of server-side vulnerability, range or permission. Unknown effect meanings/ObjectState flags are retained rather than invented. [WO] 31756/31758/23410; [WC] 14031/14033/17422.

The inspected combat parser is structurally unchanged in C. Preserve [R14]'s exact HP/damage readers and their truncation distinctions. Monster HP is keyed by actor ID and clamped at zero, now at `CombatModel.monsterHp +88`; `TargetInfo` affects HP display and is not selected-target acknowledgement. S→C **43 / 0x2b** is Faint: tolerant U16 Id, optional U8 Mode at offset 2 (default zero if absent). Faint clears a matching selected target and attack-target bookkeeping. Type-3 faint calls `UpdateMonsterHp(id,0)` (C **0x53a647**) and enters fading; it does not itself remove the actor. Own faint and later disappearance are separate transitions. HP zero, faint, fade animation, and despawn should remain separate observations. [WO] 17970/23385; [WC] 13214/17397/17378/17402.

## Automatic digging versus a manually selected point

Automatic and manual requests use different logical opcodes. Both serializers produce **five bytes** `U16 X, U16 Y, U8 MoveMode`, but automatic `AutoHost.RequestSpot` sets `DrillFeature.Flags |= 8` then sends **C→S 791 / 0x317**; manual selected-point digging sends **44 / 0x2c**. `AutoSpotPayload` delegates to `DigPayload`. The distinct opcode and local AutoDrill controller establish the distinction; the meaning “automatic” is not proven for a particular MoveMode value. [WO] 22438/15952/15945; [WC] 14139/10328/10321; [R14]

`DrillFeature..ctor` registers these automatic callbacks. Registration operands, callback reads and state branches were compared in C; the complete constructor retained its normalized instruction structure.

| Direction / logical opcode | Consumed payload and effect |
| --- | --- |
| C→S 785 / 0x311 | Empty payload through AutoHost.Send when constructing AutoDrill. |
| S→C 785 / 0x311 | U16 actor ID → OnAutoFlag(true). For own ID this sets the feature flag if needed; it does not by itself construct the controller. |
| S→C 794 / 0x31a | U16 actor ID → OnAutoFlag(false). Other-actor indication; own false returns without directly clearing the own flag. |
| C→S 791 / 0x317 | Five-byte selected automatic spot request above. |
| S→C 792 / 0x318 | Callback reads no payload → OnAutoSpotTaken. If Auto exists, set avatar FSM 5 and emit SpotTaken; otherwise call Cancel. No actor/request ID is read. |
| S→C 793 / 0x319 | U16 refusal → OnAutoSpotRefused. Values 2/3 clear DrillFeature.State and emit SpotFailed; other values produce error feedback and Stop(false). Keep the raw code; its server meaning is not established. |
| C→S 790 / 0x316 | Empty payload when entering local cooldown. |
| S→C 796 / 0x31c | Callback reads no payload → CooldownBegin, if Auto exists. |
| C→S 794 / 0x31a | Empty stop request through AutoHost.Send. Different direction and layout from inbound 794. |
| S→C 795 / 0x31b | Callback reads no payload → Stop(false), if Auto exists. |
| S→C 82 / 0x52; 83 / 0x53 | Auto callbacks read no payload and emit Equipped/EquipFailed only while AutoRunning. Other subscribers can read the same frame; 82 has the inventory equipment schema below. |

“No payload read” means no minimum length in that callback, not proof that a server always sends an empty payload. Refusal/spot events lack request correlation beyond the one local controller. Startup, pathfinding, local drill gauge and server outcomes must not be collapsed into a single automatic flag. [WO] 25142 and 22457–22465; [WC] 21366 and 14158–14166.

### Controller transitions

Both metadata enums and state writes give these phases: **0 Start, 1 WaitDrill, 2 PickSpot, 3 WaitBegin, 4 Charge, 5 Drill, 6 WaitItem, 7 Cooldown**. `AutoDrill` fields are unchanged: State +24, Retries +28, Far +32, CooldownMs +36, Counting +40, ItemExpected +41, nullable ground Item +44, Killed +52, Ended +53, ending +54. C type/field row **5779 / 0x139330**. Local `DrillFeature` state/gauge and avatar FSM are separate machines. [DO], [DC]; O24680/24703/24704, C27415/27438/27439.

| Current phase / event | State write or operation |
| --- | --- |
| Start + Result | Clear counting/item state, enter Cooldown, send 790. |
| WaitDrill + Equipped | Enter PickSpot; clear Far/Retries; run PickSpot. EquipFailed sends 794 and marks Killed. |
| PickSpot + SpotTaken | Clear Far/Retries; enter WaitBegin. SpotFailed increments retries and tries again. PickSpot refuses at retries >=7; sends 794 and marks Killed. Failed finder attempts are queued for Update rather than recursively retried without bound. |
| Start or WaitBegin, local driller stage 2/3 | Stage 2 presses local button 3; stage 3 advances to Charge. This comes from Tick, not a newly received wire message. |
| Charge, local stage 5 | Release button (0) and enter Drill; otherwise press button 3. |
| Drill, local stage 7 | Release button, enter WaitItem, clear ItemExpected. Otherwise button 3 when gauge <= marker+10, else button 2. These are local gauge controls, not packet MoveMode meanings. |
| WaitBegin/Drill/WaitItem + Result | ToCooldown. Charge + DrillEnded also does so; Drill + DrillEnded does so only when local stage !=7. |
| WaitItem + ItemAppeared | Ask ItemLanded(ground ID). A missing nullable result sends 794 and kills the controller; a present result records the item/expected state. Tick waits for landing/movement/pickup conditions and ultimately advances to cooldown. This is not an inventory insertion. |
| Cooldown + CooldownBegin | Counting=true, CooldownMs=0. Tick accumulates only while Counting. Missing PetCoolTime stops; elapsed greater than the pet cooldown advances to WaitDrill and clears counting/time. |

`ToCooldown` clears nullable Item; when leaving Charge/Drill it releases the button, sets phase 7 and sends 790. Result values do not all mean successful digging. The shared own-result handler still runs normal result feedback/end logic after forwarding the result kind into AutoDrill. Selected synthetic transitions in the lever test these event/phase distinctions; it is not a full pathfinder or gauge simulator. [WO] 24705/25190; [WC] 27440/21417.

`Stop(send)` is handled while alive: when send=true it emits 794; it marks Killed and stops moving if phase Start. Update calls End; End guards repeated completion, cancels targeting, clears Item, marks Ended, clears Far and calls host Finished. Finished clears `AutoFlagOn` and removes the Auto reference. Zone changes call Stop(false); the portal-result callback calls Stop(true) on the inspected success-code-zero branch while AutoRunning. UI interruption can call Stop(true). These local cleanup paths do not prove server cancellation or inventory receipt. [WO] 24708/24693/22455/25201/22462/25165; [WC] paired core stop/cleanup callbacks in appendix.

Reuse the prior manual contracts without changing their meaning: inbound **45** reads no actor/request ID and advances local FSM; inbound **47** is the other-actor spot form and skips own ID; own spot is **371 / 0x173** and own result **50 / 0x32**. Active outbound **48** cancellation uses nonzero `starts13 / 13` and elapsed fields; the zero/zero pending branch is not universal. Do not use automatic Stop's empty 794 layout for manual 48. Outbound **477 / 0x1dd** has the seven-byte GunFlowing.Shoot path; **5511 / 0x1587** is an independent system timer. These prior attribution corrections survive as evidence boundaries; unverified live attribution is not upgraded here. [R14]

## Ground items, result feedback, and receipt

GameSession dispatches **10–14 and 617** through `Messages.ParseGroundItem` to ItemEvent (C parser call **0x3afedc**). The grammar is:

```text
if opcode in {12,14,617}: U16 SourceId
U16 GroundId; U16 PosX; U16 PosY
if opcode in {13,14}: U16 FromX; U16 FromY
U16 Shadow (nonzero => true); three discarded U8
U8 ItemKind; U32 ItemId; discarded U64; U16 Count
```

Lengths consumed are 26 bytes for 10/11, 28 for 12/617, 30 for 13, and **32 for 14**. Unlike ItemBody, this reader's final fields do not branch on ItemKind and do not read the four item-state bytes. GroundItemMessage stores GroundId +8, Pos +12, From +20, nullable SourceId +32, ItemKind +40, ItemId +48, Count +56, Shadow +60. C `DrillUi` callback 44717 accepts specifically ItemEvent **14**, requires SourceId present, then calls ItemThrown(source,ground ID) at **0xe9893e**. ItemThrown requires source==own ID before notifying AutoDrill; C also emits ThrownByMe. This is a concrete correlation handle for automatic digging. [WO] 17083/42568/25161; [WC] 12331/44717/21388.

Separate bang notifications **1074 / 0x432** and **1206 / 0x4b6** call ParseBang with love=false/true. It reads U32 and sign-extends it to Count; signed Count>10 returns after those four bytes. Otherwise it reads U16 actor ID and, for positive Count, that many records:

```text
U16 GroundId; U16 FromX; U16 FromY; U16 PosX; U16 PosY
three discarded U8; U8 Kind; ItemBody(Kind), WITHOUT options
```

Non-love additionally reads U8 Cake after the records; love does not. Negative counts produce no record loop in the target but still read actor/cake. The strict fixture rejects negative/>10 counts. A one-record Kind-1 bang is 35 bytes, or 34 for love. OnBang/BangTick schedules ground-item/visual effects, not Inventory.Apply. Do not label all bang traffic “automatic dig success.” [WO] 15955/22472/22473/25193/25177; [WC] 10331 plus unchanged parsed grammar.

S→C **34 / 0x22** is `U16 Player, U16 GroundItem, U8 Kind, ItemBody(Kind)` **without options or Op**. `ItemsModel..ctor` registers it (O `0x9799e4`); ParsePickedUp feeds an own-ID-filtered caption path. Another consumer updates/removes the ground item. Neither inspected consumer calls Inventory.Apply. A Kind-1 fixture is 19 bytes, Kind-2/3 23 bytes. “Picked up” feedback is therefore distinct from an observed inventory mutation. For receipt, correlate a UID/count/state change through the inventory paths below, after a usable baseline, and retain the ground ID and actor separately. [WO] 16732/31288/46979/46916; [WC] 11981/20934/49034/48971, plus the independently retained inventory reducer.

## Inventory baseline and reduction

`Inventory..ctor` replays `GameSession.StartSet` **in order**. Logical **231 / 0xe7** calls ReadInventoryList, then stores only Kind 1/2 entries by UID. The reader is `U16 count`, then count repetitions of **U8 Kind + ItemBody**, with Options for every Kind-2 entry. There is no Op byte. It loops the full U16 count; do not substitute the prior delta reader's three-entry cap. Zero-count `00 00` is valid. Multiple 231 frames merge/replace UIDs; they do not clear the dictionary between frames. C call **0x35eed9**. [WO] 16722/16737; [WC] 11971/11986.

| Body | Exact consumed fields |
| --- | --- |
| Kind 1 | U32 ItemId, U64 Uid, U16 Count |
| Kind 2 or 3 | Same, followed by four state bytes |
| Other Kind | Target returns its default body without consuming a body schema. Strict adapter should classify unsupported rather than guess a width. |
| Options | U8 count, count × `(U32 Id, U32 A, U32 B)` |

Startup **233 / 0xe9** also calls ReadStartEquipment: payload **0..183 = 23 × U64 slot UID**; values below 2 normalize to zero. Reader jumps to **231** and reads U32 Galder, so this reader alone needs at least **235 bytes**. It is an overlapping view of StartStats, whose wider own-resource baseline is in [R15]; do not truncate that packet to 235. The constructor installs a slot only if its UID is already in byUid **at that point in replay**; missing UIDs are discarded, with no delayed fixup in this constructor. C call **0x35f064**. Currency stores to Inventory +24. Special 518/521 startup replay separately sets/clears slot **20** for a known UID. [WO] 16731/16737/16738; [WC] 11980/11986; [R15]

The old startup waits established 679/profile, 230/identity-clock-position and 233/stats; **231 was not one of those waits**. Their current authentication/bootstrap validity is the separate worker's responsibility. In both inspected GameSession constructors, recordingStartSet is enabled immediately around BindGameServer/backlog replay and disabled afterward (C **0x3add58 → 0x3add61 → 0x3add68**); Handle appends only while recording. No inventory-ready bit or explicit inventory-list-ended wire marker was recovered in this path, and Inventory does not normally subscribe to 231 after construction. A missing 231 is different from a valid empty list. The new GmHello bootstrap hook is not examined as an inventory guarantee. [WO] 16196/16232/16737; [WC] 13257/13296/11986.

**Derived readiness contract:** record separate facts `own_baseline_seen`, `inventory_list_seen`, and `equipment_references_resolved`. Process startup frames in order, retain missing equipment references diagnostically, and never describe an unobserved inventory as empty. A consistent list/slot view permits dashboard display and UID-based reducer tests; it does not prove the server's entire inventory was delivered. Gate autonomous use/equip/dig on the selected tool's actually received item/slot state and the revalidated startup boundary. A complete-live-inventory claim still needs bounded ordering/completeness evidence. No weight/maxima formulas are needed to make that distinction.

Existing `ReadItemChange` is `U8 Op, U8 Kind, ItemBody`, followed by Options only for **Op 2/4 and Kind 2**. Existing logical **561/160** use one change; **770** skips U64/U32/U64 then one change; **262/246/260** use U16 count but consume at most three changes. **843** instead uses a full U32 count of ItemChange. Reuse [R14]'s exact deltas rather than applying startup list grammar to them. Inventory.Apply's operations are unchanged in C:

| Op | State write |
| --- | --- |
| 2,4,6,8 | Add/replace by UID for Kind 1/2. |
| 3,5,7 | Remove UID and clear equipment slots referencing it. |
| 1 | Existing UID only: Kind 2 copies State; otherwise replaces Count. It does not add Count, create a missing UID, or necessarily refresh options. |
| Other | No supported update branch. Preserve unsupported input rather than inventing an effect. |

Normal reducer completion emits ItemsChanged. C Inventory type/field row is **12096 / 0x158940**, with byUid +12, slots +16, Galder +24 unchanged. This is the receipt evidence; it still does not prove which outgoing action caused the delta. Extra paths sharpen that distinction: S→C **68** reads one ItemChange, requires the existing UID, calculates dropped count, applies it and emits **Dropped**; it is not a pickup receipt. **41** replaces Galder from U32. **82** reads U64 UID + U8 slot and sets it only for a known UID and slot <=22. **85** discards U64, reads U8 slot, and clears a nonempty valid slot without checking the discarded UID. [WO] 16560/16769–16772; [WC] 11808/12017–12020.

## Sufficient boundary and the next bounded live checks

The bounded source question can be resolved with these layouts and transitions, together with the immutable baseline and prior manual/resource report. This is **not field-level completion of all game content**. Effect-value semantics, discarded fields, unsupported masks/kinds, all gift/couple/trade operations, skill/PvP/counter ordering, navigation and server restrictions, and new restart/bootstrap features remain outside the demonstrated contract. Partial/unknown branches must stay visible in the adapter; the safe fixtures are deliberately stricter than target fallbacks.

After the independent current-build gateway/authentication revalidation and one valid startup, a later authorized worker can correlate without another blind capture:

| Question | Minimal selected evidence |
| --- | --- |
| Monster identity/lifecycle | Inbound 7/8/9 Type 3, ID and the complete effect/typed tail; movement 24–27/729 for that ID; monster HP 286, target info 289, faint 43, and disappearance 6/15/16/21. Keep monster Target separate from local Target. |
| Manual versus automatic | Outbound 44 versus 791, five-byte position/MoveMode; automatic 785/792/793/795/796 phase events; own 371 and 50. Do not correlate by empty 45/792 alone or by MoveMode 2. |
| Ground outcome versus receipt | Inbound 14 SourceId==own ID and GroundId; optional 34 Player==own ID/GroundItem; independently reduced 561/160/770/list changes with UID/count/state, or 41 for currency. Preserve ordering and Aux release boundaries. |
| Inventory readiness | Actual order and presence of 231 lists versus 233 slot references and delivery to the StartSet window, including valid empty list. Observe a stable usable snapshot before asserting a complete baseline. |
| Cancellation | Distinguish manual 48's active/pending branch from empty automatic 794; record local stopping/ended separately from any inbound 795 or later outcome. |

No live check above ran here. No game account process, background wait, browser tab, PR watch, PR or merge was started. Only this report's branch and issue 14 are delivery targets; the root owns map integration.

## Paired primary locators

The following rows are selected evidence, not a claim that equal method names prove compatibility. N/G/P mean Net/Game/UI.Pc. `O function@body → C function@body`; metadata row/token/table are for C. For old rows already published, reuse [R14] rather than recounting their proof. Raw field rows and table-data offsets above provide the state-write cross-check.

| Method | O function @ body | C function @ body | C metadata row / token / element slot |
| --- | --- | --- | --- |
| G `Spinel.Game.World.ObjectPick::TargetingMask` | `31756 @ 9af63e` | `14031 @ 3e82ad` | `34572 / 0x60001b2 / 5210` |
| G `Spinel.Game.World.ObjectPick::Aimable` | `31758 @ 9af9f4` | `14033 @ 3e8663` | `34575 / 0x60001b5 / 5213` |
| G `AutoDrill::.ctor` | `24680 @ 738629` | `27415 @ 892e81` | `36000 / 0x6000746 / 6536` |
| G `AutoDrill::End` | `24693 @ 7389f1` | `27428 @ 893249` | `36016 / 0x6000756 / 6552` |
| G `AutoDrill::Tick` | `24703 @ 739e28` | `27438 @ 894680` | `36020 / 0x600075a / 6556` |
| G `AutoDrill::Handle` | `24704 @ 73ad39` | `27439 @ 895591` | `36021 / 0x600075b / 6557` |
| G `AutoDrill::ToCooldown` | `24705 @ 73b34b` | `27440 @ 895ba3` | `36022 / 0x600075c / 6558` |
| G `AutoDrill::PickSpot` | `24701 @ 739a28` | `27436 @ 894280` | `36023 / 0x600075d / 6559` |
| G `AutoDrill/<>c__DisplayClass67_0::<Message>b__0` | `24708 @ 73b58f` | `27443 @ 895de7` | `36030 / 0x6000764 / 6566` |
| G `CombatModel::UpdateMonsterHp` | `23367 @ 6aee63` | `17378 @ 538204` | `37871 / 0x6000e95 / 8375` |
| G `CombatModel::Faint` | `23385 @ 6b0e64` | `17397 @ 53a369` | `37889 / 0x6000ea7 / 8393` |
| G `CombatModel::ApplyMessage` | `23390 @ 6b1682` | `17402 @ 53ac1c` | `37897 / 0x6000eaf / 8400` |
| G `CombatModel::WorldClick` | `23410 @ 6b4a19` | `17422 @ 53dfb7` | `37914 / 0x6000ec0 / 8417` |
| G `DrillFeature::.ctor` | `25142 @ 7618c2` | `21366 @ 68be09` | `38711 / 0x60011dd / 9207` |
| G `DrillFeature::StartAuto` | `25154 @ 763158` | `21381 @ 68d849` | `38743 / 0x60011fd / 9239` |
| G `DrillFeature::OnAutoSpotTaken` | `25158 @ 76348b` | `21385 @ 68db7c` | `38747 / 0x6001201 / 9243` |
| G `DrillFeature::OnAutoSpotRefused` | `25159 @ 7634c6` | `21386 @ 68dbb7` | `38748 / 0x6001202 / 9244` |
| G `DrillFeature::ItemThrown` | `25161 @ 76360c` | `21388 @ 68dcfd` | `38749 / 0x6001203 / 9245` |
| G `DrillFeature::OnFsm` | `25170 @ 763ae9` | `21397 @ 68e243` | `38759 / 0x600120d / 9255` |
| G `DrillFeature::Result` | `25190 @ 765910` | `21417 @ 69006a` | `38779 / 0x6001221 / 9275` |
| G `DrillFeature/AutoHost::RequestSpot` | `22438 @ 664317` | `14139 @ 3edc5a` | `38814 / 0x6001244 / 9310` |
| G `DrillFeature/AutoHost::Send` | `22440 @ 6643ff` | `14141 @ 3edd42` | `38816 / 0x6001246 / 9312` |
| G `DrillFeature/AutoHost::Finished` | `22455 @ 664a63` | `14156 @ 3ee3a4` | `38831 / 0x6001255 / 9327` |
| G `DrillFeature/<>c__DisplayClass122_0::<.ctor>b__1` | `22457 @ 664ae3` | `14158 @ 3ee424` | `38834 / 0x6001258 / 9330` |
| G `DrillFeature/<>c__DisplayClass122_0::<.ctor>b__2` | `22458 @ 664afd` | `14159 @ 3ee43e` | `38835 / 0x6001259 / 9331` |
| G `DrillFeature/<>c__DisplayClass122_0::<.ctor>b__3` | `22459 @ 664b17` | `14160 @ 3ee458` | `38836 / 0x600125a / 9332` |
| G `DrillFeature/<>c__DisplayClass122_0::<.ctor>b__4` | `22460 @ 664b25` | `14161 @ 3ee466` | `38837 / 0x600125b / 9333` |
| G `DrillFeature/<>c__DisplayClass122_0::<.ctor>b__5` | `22461 @ 664b3d` | `14162 @ 3ee47e` | `38838 / 0x600125c / 9334` |
| G `DrillFeature/<>c__DisplayClass122_0::<.ctor>b__6` | `22462 @ 664b58` | `14163 @ 3ee499` | `38839 / 0x600125d / 9335` |
| G `DrillFeature/<>c__DisplayClass122_0::<.ctor>b__7` | `22463 @ 664b94` | `14164 @ 3ee4d5` | `38840 / 0x600125e / 9336` |
| G `DrillFeature/<>c__DisplayClass122_0::<.ctor>b__8` | `22464 @ 664bad` | `14165 @ 3ee4ee` | `38841 / 0x600125f / 9337` |
| G `DrillFeature/<>c__DisplayClass122_0::<.ctor>b__9` | `22465 @ 664bd2` | `14166 @ 3ee513` | `38842 / 0x6001260 / 9338` |
| G `ItemsModel::.ctor` | `31235 @ 979503` | `20871 @ 662466` | `42159 / 0x6001f55 / 12636` |
| G `ItemsModel/<>c__DisplayClass40_0::<.ctor>b__0` | `31288 @ 97fb6d` | `20934 @ 66989a` | `42230 / 0x6001f9c / 12707` |
| G `MonsterTargets::.ctor` | `29106 @ 8bfc14` | `26319 @ 82f29c` | `43194 / 0x6002360 / 13665` |
| G `MonsterTargets::OnAppear` | `29109 @ 8c0513` | `26322 @ 82fb9b` | `43197 / 0x6002363 / 13668` |
| G `MonsterTargets::Register` | `29110 @ 8c05e1` | `26323 @ 82fc69` | `43199 / 0x6002365 / 13670` |
| G `MonsterTargets::ApplyAppearTarget` | `29111 @ 8c073d` | `26324 @ 82fdc5` | `43200 / 0x6002366 / 13671` |
| G `MonsterTargets::Destroy` | `29112 @ 8c08a1` | `26325 @ 82ff29` | `43205 / 0x600236b / 13676` |
| N `Combat::ParseMessage` | `17970 @ 4b4fb3` | `13214 @ 3aaaaf` | `71379 / 0x6000386 / 24506` |
| N `Drill::DigPayload` | `15945 @ 4399ff` | `10321 @ 3027cb` | `71711 / 0x60004d2 / 24838` |
| N `Drill::ParseSpot` | `15949 @ 439b5d` | `10325 @ 30291a` | `71715 / 0x60004d6 / 24842` |
| N `Drill::ParseResult` | `15950 @ 439c55` | `10326 @ 302a07` | `71716 / 0x60004d7 / 24843` |
| N `Drill::AutoSpotPayload` | `15952 @ 439d9f` | `10328 @ 302b4a` | `71718 / 0x60004d9 / 24845` |
| N `Drill::ParseBang` | `15955 @ 439e24` | `10331 @ 302bcd` | `71721 / 0x60004dc / 24848` |
| N `EsBlock::Read` | `16011 @ 43c823` | `10388 @ 3055a3` | `71820 / 0x600053f / 24947` |
| N `EsBlock::.cctor` | `16014 @ 43cd53` | `10391 @ 305abc` | `71823 / 0x6000542 / 24950` |
| N `GameSession::.ctor` | `16196 @ 4481ed` | `13257 @ 3ad98c` | `72083 / 0x6000646 / 25210` |
| N `GameSession::Handle` | `16232 @ 44a1f3` | `13296 @ 3afad2` | `72121 / 0x600066c / 25248` |
| N `GameSession::ApplyMove` | `16233 @ 44af91` | `13297 @ 3b085d` | `72123 / 0x600066e / 25250` |
| N `Items::ReadItemBody` | `16632 @ 461ed3` | `11776 @ 353e0c` | `72846 / 0x6000941 / 25966` |
| N `Items::ReadOptions` | `16633 @ 46206f` | `11781 @ 3546dc` | `72847 / 0x6000942 / 25967` |
| N `Items::ReadItemChange` | `16631 @ 461e30` | `11779 @ 35459b` | `72848 / 0x6000943 / 25968` |
| N `Items::ReadInventoryList` | `16722 @ 466807` | `11971 @ 35de5d` | `72849 / 0x6000944 / 25969` |
| N `Items::ReadStartEquipment` | `16731 @ 466fd6` | `11980 @ 35e606` | `72858 / 0x600094d / 25978` |
| N `Items::ParsePickedUp` | `16732 @ 467266` | `11981 @ 35e87e` | `72859 / 0x600094e / 25979` |
| N `Inventory::.ctor` | `16737 @ 467455` | `11986 @ 35ea66` | `72879 / 0x6000962 / 25999` |
| N `Inventory::Apply` | `16560 @ 45e8eb` | `11808 @ 35602e` | `72889 / 0x600096c / 26009` |
| N `Inventory::Special` | `16738 @ 468d71` | `11987 @ 36037e` | `72897 / 0x6000974 / 26017` |
| N `Inventory::<.ctor>b__5_11` | `16769 @ 46a6f8` | `12017 @ 361cad` | `72918 / 0x6000989 / 26038` |
| N `Inventory::<.ctor>b__5_12` | `16770 @ 46a874` | `12018 @ 361e26` | `72919 / 0x600098a / 26039` |
| N `Inventory::<.ctor>b__5_13` | `16771 @ 46a90d` | `12019 @ 361ebd` | `72920 / 0x600098b / 26040` |
| N `Inventory::<.ctor>b__5_14` | `16772 @ 46a9b6` | `12020 @ 361f63` | `72921 / 0x600098c / 26041` |
| N `Messages::ReadPlayerBody` | `17080 @ 47c5dd` | `12328 @ 3738a9` | `73337 / 0x6000b2c / 26457` |
| N `Messages::ParseAppear` | `17081 @ 47ca85` | `12329 @ 373d30` | `73338 / 0x6000b2d / 26458` |
| N `Messages::ParseGroundItem` | `17083 @ 47d0d8` | `12331 @ 37435f` | `73340 / 0x6000b2f / 26460` |
| N `Messages::ParseMove` | `17085 @ 47d3d1` | `12333 @ 374641` | `73342 / 0x6000b31 / 26462` |
| N `Messages::.cctor` | `17098 @ 47db26` | `12346 @ 374d6e` | `73355 / 0x6000b3e / 26475` |
| P `GameplayScreen::RemoveActor` | `35892 @ b3cc4b` | `38488 @ bf72c0` | `19391 / 0x6000743 / 47987` |
| P `GameplayScreen::Appear` | `35888 @ b3bb9d` | `38484 @ bf621c` | `19406 / 0x6000752 / 48002` |
| P `GroundItems::PickedUp` | `46916 @ fb90e6` | `48971 @ 1040f1e` | `19658 / 0x600084e / 48242` |
| P `GroundItems/<>c__DisplayClass40_0::<Install>b__2` | `46979 @ fc13f9` | `49034 @ 1048d93` | `19743 / 0x60008a3 / 48323` |
| P `Spinel.UI.Pc.Play.DrillUi callback for ItemEvent 14` | `42568 @ e090e8` | `44717 @ e9878f` | `22397 / 0x6001301 / 50864` |

## Reproduction and offline verification

The executable block below is the complete minimal verification lever: save it **outside Git** as `verify-state.py` and supply the decoded data/WASM pair. It uses only the Python standard library and does not execute target code or access the network. It rejects mixed builds, extracts and hashes metadata, follows selected method→module-pointer→element→body joins, checks copied descriptor/mask/opcode data and state-field offsets, then exercises the transcribed grammars and selected reductions. The old path reuses prior core evidence and checks the new additions; the current path resolves the selected changed-build methods independently.

```sh
python3 /tmp/verify-state.py /path/to/decoded.data /path/to/decoded.wasm
```

Both pinned revisions passed. A deliberate old-data/current-WASM pairing must fail with `mixed or changed build`. Fixtures use invented IDs/values only: all compiled Es descriptor shapes, special 1425's n=0/n=1 branches, each player optional mask, both title-partner variants, all A/B slots and override behavior, actor-tail variants, inventory kind/options/count/state/delete behavior, equipment references and truncation, automatic event/phase distinctions, all six ground forms, pickup and bang forms. Rejections include missing strings, truncated fields/options, impossible counts, unknown strict-policy branches and unclassified tails. The lever's exact-length checks and strict quarantine are proposed adapter policy; target permissiveness is documented separately above. These tests verify the transcription's consistency, not server authority or every target exception path.

For source review, use WABT **1.0.39** `wasm-objdump -d decoded.wasm > /tmp/state-module.dis` and inspect only the appendix function ranges. Keep the large disassembly outside Git. Resolve branch-table depths using enclosing block nesting. Recover type field offsets through the registration array, rather than transplanting old managed offsets. For copied field-RVA values: a usage cell's encoded kind 6 identifies a field-reference row; its 6 bytes are runtime-type U16 and field ordinal U32. Resolve the runtime type-pointer array to its type-definition index, add that type's fieldStart, find the 10-byte field-default row `(U32 field,U16 type,I32 dataOffset)`, and add the metadata default-data base. The paired table offsets above make this review reproducible without account data.

```python
import hashlib
import pathlib
import struct
import sys

data = pathlib.Path(sys.argv[1]).read_bytes()
wasm = pathlib.Path(sys.argv[2]).read_bytes()
revisions = {
"f1ab3ef3d2b6b04b28104bac55b25c007e78a24242a56c354b640428daf3053d":
    ("old", "77a05d62cc5cc6fd5d1a1f5b4f4d25d8c0d1f9e8ea4740c08326f8ec83e2b446",
     16459759,9148328,"b7b7920f034cbdfb87eaf69557e17ea6b1513693acb9e905e918eaedbaf1a483"),
"fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474":
    ("current", "381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404",
     16748911,9343896,"7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade")}
revision, fingerprint, offset, size, meta_fingerprint = revisions[hashlib.sha256(data).hexdigest()]
assert hashlib.sha256(wasm).hexdigest() == fingerprint, "mixed or changed build"
meta = data[offset:offset+size]
assert hashlib.sha256(meta).hexdigest() == meta_fingerprint
assert struct.unpack_from("<II", meta) == (0xFAB11BAF, 108)

p = 8

def u():
    global p
    value = shift = 0
    while True:
        byte = wasm[p]
        p += 1
        value |= (byte & 127) << shift
        shift += 7
        if byte < 128:
            return value

sections = {}
while p < len(wasm):
    kind = wasm[p]
    p += 1
    size = u()
    sections[kind] = p
    p += size

p = sections[2]
imports = u()
for _ in range(imports):
    for _ in range(2):
        size = u()
        p += size
    assert wasm[p] == 0
    p += 1
    u()
assert imports == (484 if revision == "old" else 490)

p = sections[9]
assert u() == 1 and u() == 0 and wasm[p] == 0x41
p += 1
start = u()
assert wasm[p] == 0x0b
p += 1
elements = [None] * start + [u() for _ in range(u())]

memory = bytearray(64 * 1024 * 1024)
p = sections[11]
for _ in range(u()):
    assert u() == 0 and wasm[p] == 0x41
    p += 1
    address = u()
    assert wasm[p] == 0x0b
    p += 1
    size = u()
    memory[address:address + size] = wasm[p:p + size]
    p += size

p = sections[10]
bodies = {}
for function in range(imports, imports + u()):
    size = u()
    bodies[function] = p
    p += size

strings = struct.unpack_from("<III", meta, 8 + 12 * 2)[0]
methods, method_bytes, method_count = struct.unpack_from("<III", meta, 8 + 12 * 5)
types, type_bytes, type_count = struct.unpack_from("<III", meta, 8 + 12 * 19)
assert method_bytes == 30 * method_count and type_bytes == 66 * type_count

def name(index):
    begin = strings + index
    return meta[begin:meta.index(b"\0", begin)].decode()

if revision == "old":
    modules = {'G': (2141464, 18411, 3616112), 'N': (2141508, 4729, 3693968), 'P': (2141596, 16433, 3782448)}
    joins = [('P', 18373, 35892, 11783243),
     ('P', 18387, 35888, 11778973),
     ('P', 21335, 42568, 14717160),
     ('G', 33440, 31756, 10155582),
     ('G', 33443, 31758, 10156532),
     ('G', 34853, 24680, 7570985),
     ('G', 34869, 24693, 7571953),
     ('G', 34873, 24703, 7577128),
     ('G', 34874, 24704, 7580985),
     ('G', 34875, 24705, 7582539),
     ('G', 34876, 24701, 7576104),
     ('G', 34883, 24708, 7583119),
     ('G', 36725, 23385, 7016036),
     ('G', 36750, 23410, 7031321),
     ('G', 37562, 25158, 7746699),
     ('G', 37563, 25159, 7746758),
     ('G', 37564, 25161, 7747084),
     ('G', 37629, 22438, 6701847),
     ('G', 37631, 22440, 6702079),
     ('G', 37646, 22455, 6703715),
     ('G', 40906, 31288, 9960301),
     ('G', 41874, 29110, 9176545),
     ('G', 41875, 29111, 9176893),
     ('G', 41880, 29112, 9177249),
     ('N', 70171, 15952, 4431263),
     ('N', 70174, 15955, 4431396),
     ('N', 70273, 16011, 4442147),
     ('N', 70276, 16014, 4443475),
     ('N', 71250, 16722, 4614151),
     ('N', 71259, 16731, 4616150),
     ('N', 71260, 16732, 4616806),
     ('N', 71319, 16769, 4630264),
     ('N', 71320, 16770, 4630644),
     ('N', 71321, 16771, 4630797),
     ('N', 71322, 16772, 4630966),
     ('N', 71738, 17080, 4703709),
     ('N', 71741, 17083, 4706520),
     ('N', 71756, 17098, 4709158)]
else:
    modules = {'G': (2172188, 18796, 3667360), 'N': (2172232, 4791, 3746928), 'P': (2172320, 16606, 3839488)}
    joins = [('N', 71711, 10321, 3155915),
     ('N', 71715, 10325, 3156250),
     ('N', 71716, 10326, 3156487),
     ('N', 71718, 10328, 3156810),
     ('N', 71721, 10331, 3156941),
     ('N', 71820, 10388, 3167651),
     ('N', 71823, 10391, 3168956),
     ('N', 72083, 13257, 3856780),
     ('N', 72121, 13296, 3865298),
     ('N', 72123, 13297, 3868765),
     ('N', 72889, 11808, 3498030),
     ('N', 72848, 11779, 3491227),
     ('N', 72846, 11776, 3489292),
     ('N', 72847, 11781, 3491548),
     ('N', 72849, 11971, 3530333),
     ('N', 72858, 11980, 3532294),
     ('N', 72859, 11981, 3532926),
     ('N', 72879, 11986, 3533414),
     ('N', 72897, 11987, 3539838),
     ('N', 72918, 12017, 3546285),
     ('N', 72919, 12018, 3546662),
     ('N', 72920, 12019, 3546813),
     ('N', 72921, 12020, 3546979),
     ('N', 73337, 12328, 3618985),
     ('N', 73338, 12329, 3620144),
     ('N', 73340, 12331, 3621727),
     ('N', 73342, 12333, 3622465),
     ('N', 73355, 12346, 3624302),
     ('N', 71379, 13214, 3844783),
     ('G', 38814, 14139, 4119642),
     ('G', 38816, 14141, 4119874),
     ('G', 38831, 14156, 4121508),
     ('G', 38834, 14158, 4121636),
     ('G', 38835, 14159, 4121662),
     ('G', 38836, 14160, 4121688),
     ('G', 38837, 14161, 4121702),
     ('G', 38838, 14162, 4121726),
     ('G', 38839, 14163, 4121753),
     ('G', 38840, 14164, 4121813),
     ('G', 38841, 14165, 4121838),
     ('G', 38842, 14166, 4121875),
     ('G', 37871, 17378, 5472772),
     ('G', 37889, 17397, 5481321),
     ('G', 37897, 17402, 5483548),
     ('G', 37914, 17422, 5496759),
     ('G', 36000, 27415, 8990337),
     ('G', 36016, 27428, 8991305),
     ('G', 36023, 27436, 8995456),
     ('G', 36020, 27438, 8996480),
     ('G', 36021, 27439, 9000337),
     ('G', 36022, 27440, 9001891),
     ('G', 36030, 27443, 9002471),
     ('G', 38711, 21366, 6864393),
     ('G', 38743, 21381, 6871113),
     ('G', 38747, 21385, 6871932),
     ('G', 38748, 21386, 6871991),
     ('G', 38749, 21388, 6872317),
     ('G', 38759, 21397, 6873667),
     ('G', 38779, 21417, 6881386),
     ('G', 43194, 26319, 8581788),
     ('G', 43197, 26322, 8584091),
     ('G', 43199, 26323, 8584297),
     ('G', 43200, 26324, 8584645),
     ('G', 43205, 26325, 8585001),
     ('G', 42159, 20871, 6693990),
     ('G', 42230, 20934, 6723738),
     ('G', 34572, 14031, 4096685),
     ('G', 34575, 14033, 4097635),
     ('P', 19406, 38484, 12542492),
     ('P', 19391, 38488, 12546752),
     ('P', 19658, 48971, 17043230),
     ('P', 19743, 49034, 17075603),
     ('P', 22397, 44717, 15304591)]
for module, index, expected, offset in joins:
    base, count, pointers = modules[module]
    assert struct.unpack_from("<II", memory, base + 4) == (count, pointers)
    row = struct.unpack_from("<IHHIIHIHHHH", meta, methods + index * 30)
    slot = (row[6] & 0xffffff) - 1
    assert slot < count
    table = struct.unpack_from("<I", memory, pointers + 4 * slot)[0]
    assert elements[table] == expected and bodies[expected] == offset
print("PASS:", revision, "pinned artifacts and selected method locators")

# These arrays are data, not inferred field sizes. Offsets are within metadata.
ab,aslot,bb,bs = ((0x4cb658,0x4cb7a8,0x4cb0b8,0x4cb870) if revision == "old"
                 else (0x4e76d0,0x4e7820,0x4e7130,0x4e78e8))
A = list(zip(struct.unpack_from("<17Q",meta,ab),struct.unpack_from("<17I",meta,aslot)))
B = list(zip(struct.unpack_from("<6Q",meta,bb),struct.unpack_from("<6I",meta,bs)))
assert A == list(zip([1,2,4,32,64,128,256,512,1024,2048,4096,8192,16384,32768,65536,262144,524288],
                     [0,1,2,5,6,10,11,12,13,14,15,16,17,18,19,7,21]))
assert B == list(zip([1,2,4,4096,8192,524288], [0,1,2,15,16,21]))

assert struct.unpack_from("<3I", meta, 0x4cb168 if revision == "old" else 0x4e71e0) == (262,246,260)
for off,expected in zip((0x4caed0,0x4cb3d8,0x4cb308,0x4cb348) if revision == "old"
                        else (0x4e6f48,0x4e7450,0x4e7380,0x4e73c0),
                       ((1,4,2),(4,1,2),(2,2,2),(1,2,2))):
    assert struct.unpack_from("<3I",meta,off) == expected

field_table = 0x351ee0 if revision == "old" else 0x35d850
field_base = struct.unpack_from("<I",meta,8+12*11)[0]
selected_fields = [
    ((11532,11699), "EsEntry", {"Group":8,"Esa":12,"Body":32}),
    ((12012,12190), "PlayerLook", {"Attributes":8,"Equipment":24,"Worn":28,"ShopStand":96}),
    ((11918,12096), "Inventory", {"byUid":12,"slots":16,"galderValue":24}),
    ((5659,5779), "AutoDrill", {"<State>k__BackingField":24,"<Item>k__BackingField":44,
                               "<Killed>k__BackingField":52,"<Ended>k__BackingField":53}),
    ((5979,6101), "CombatModel", {"<Target>k__BackingField":60,"<Fainted>k__BackingField":68,
                                  "monsterHp":76 if revision=="old" else 88,
                                  "fading":124 if revision=="old" else 136})]
for indices,owner,wanted in selected_fields:
    index = indices[revision == "current"]; row = types+66*index
    assert name(struct.unpack_from("<I",meta,row)[0]) == owner
    begin = struct.unpack_from("<I",meta,row+20)[0]
    count = struct.unpack_from("<H",meta,row+46)[0]
    field_row = struct.unpack_from("<I",memory,field_table+4*index)[0]
    actual = {name(struct.unpack_from("<I",meta,field_base+10*(begin+j))[0]):
              struct.unpack_from("<I",memory,field_row+4*j)[0] for j in range(count)}
    assert all(actual[k] == v for k,v in wanted.items()),(owner,actual)
print("PASS: descriptor arrays, masks, inventory opcode array and selected field offsets")

ES = {
1118:(2,),1153:(0,3),1186:(4,2),1192:(1,4),1208:(0,),1248:(0,),
1267:(1,4),1295:(4,1),1309:(1,1),1316:(1,4,2),1333:(2,2),1353:(1,4,2),
1357:(4,1,2),1358:(1,4,2),1361:(2,2),1369:(1,4),1371:(1,4),1373:(1,4),
1375:(1,4),1377:(1,4),1380:(4,1),1386:(2,2),1388:(5,5),1390:(0,),1394:(2,2),
1423:(2,2,2),1476:(3,),1479:(1,2,2),1483:(2,2,2),1487:(3,),1492:(1,2,2),
1494:(0,),1524:(2,),1525:(2,),1526:(2,),1527:(2,),1529:(2,),1531:(2,),
1533:(2,2),1534:(2,),1537:(2,),1540:(2,),1541:(2,),1542:(2,),1543:(2,),
1545:(2,),1547:(2,)}
pack = lambda fmt, *v: struct.pack('<' + fmt, *v)
class Invalid(ValueError): pass
class Reader:
    def __init__(self, b): self.b, self.p = b, 0
    def take(self, n):
        if n < 0 or n > len(self.b) - self.p: raise Invalid('truncated')
        old = self.p; self.p += n
        return self.b[old:self.p]
    def get(self, f): return struct.unpack('<' + f, self.take(struct.calcsize('<' + f)))[0]
    def cstr(self):
        end = self.b.find(b'\0', self.p)
        if end < 0: raise Invalid('missing NUL')
        return self.take(end + 1 - self.p)[:-1]
    def end(self):
        if self.p != len(self.b): raise Invalid('unclassified tail')

def es(r):
    rows = []
    if r.get('B') == 0: return rows
    for group in range(8):
        for _ in range(r.get('B')):
            key, instance, v14, skill, v04 = struct.unpack('<HHBHH', r.take(9))
            start = r.p
            if key == 1425:
                n = r.get('H'); r.take(4 * n)
                m = r.get('I'); r.take(2 * m)
                if n: r.take(2)
            elif key in ES:
                for descriptor in ES[key]:
                    if descriptor == 5: r.cstr()
                    else: r.take({0:1,1:2}.get(descriptor,4))
            else: raise Invalid('unclassified Es key')
            rows.append((group,key,instance,v14,skill,v04,r.b[start:r.p]))
    return rows

def player(r):
    attributes, alternate, mask = r.get('B'), r.get('I'), r.get('B')
    if mask & 128: raise Invalid('unclassified player bit')
    for bit in (1,2,4,8,16):
        if mask & bit:
            if bit == 8: r.take(1)
            r.cstr()
    worn = [0]*23
    a = r.get('I')
    if a & ~sum(bit for bit,slot in A): raise Invalid('unclassified A bit')
    for bit,slot in A:
        if a & bit: worn[slot] = r.get('I')
    equipped = worn.copy()
    b = r.get('I')
    if b & ~sum(bit for bit,slot in B): raise Invalid('unclassified B bit')
    for bit,slot in B:
        if b & bit: equipped[slot] = r.get('I')
    r.take(1); r.cstr(); r.cstr(); r.take(8)
    if mask & 32:
        n = r.get('I')
        if n >= 0x20000000: raise Invalid('count overflow')
        r.take(4*n)
    kills, deaths = r.get('H'), r.get('H')
    if mask & 64: r.cstr()
    title = r.get('H'); r.take(11)
    partner = r.cstr() if title in (4,6) else None
    stand = r.get('B')
    return worn,equipped,title,partner,stand

def appearance(b):
    r = Reader(b)
    ident,kind = r.get('H'),r.get('H')
    r.take(18); r.cstr(); r.take(7); effects = es(r); r.take(4)
    tail = None
    if kind == 1: tail = player(r)
    elif kind == 2: tail = struct.unpack('<HI',r.take(6))
    elif kind == 3: tail = r.get('H')
    elif kind in (4,16):
        tail = (r.get('H'),None)
        if kind == 16 and r.p < len(b): tail = (tail[0],struct.unpack('<BIIIB',r.take(14)))
    r.end()
    return ident,kind,effects,tail

def body(r, kind, options=False):
    if kind not in (1,2,3): raise Invalid('unclassified item kind')
    item = dict(kind=kind,id=r.get('I'),uid=r.get('Q'),count=r.get('H'))
    if kind in (2,3): item['state'] = r.take(4)
    if options and kind == 2:
        item['options'] = [struct.unpack('<III',r.take(12)) for _ in range(r.get('B'))]
    return item

def inventory(b):
    r = Reader(b); items = {}
    for _ in range(r.get('H')):
        item = body(r,r.get('B'),True)
        if item['kind'] in (1,2): items[item['uid']] = item
    r.end(); return items

def change(b):
    r = Reader(b); op,kind = r.get('B'),r.get('B')
    item = body(r,kind,op in (2,4)); r.end()
    return op,item

def apply(items, slots, delta):
    op,item = delta; uid = item['uid']
    if op in (2,4,6,8) and item['kind'] in (1,2): items[uid] = item.copy()
    elif op in (3,5,7):
        items.pop(uid,None)
        slots[:] = [0 if v == uid else v for v in slots]
    elif op == 1 and uid in items:
        key = 'state' if item['kind'] == 2 else 'count'
        items[uid][key] = item[key]

def equipment(b,items):
    r = Reader(b)
    wanted = [r.get('Q') for _ in range(23)]
    wanted = [v if v >= 2 else 0 for v in wanted]
    r.take(47); galder = r.get('I')
    return [v if v in items else 0 for v in wanted],galder,{v for v in wanted if v and v not in items}

# Selected event transitions; intentionally not a complete AutoDrill simulator.
def auto_event(phase,event,stage=0):
    if event == 'Result' and phase in (0,3,5,6): return 7,[790]
    if event == 'DrillEnded' and (phase == 4 or phase == 5 and stage != 7): return 7,[790]
    if phase == 2 and event == 'SpotTaken': return 3,[]
    return phase,[]
def auto_stop(phase,send): return dict(killed=True,send=[794] if send else [],stop_move=phase==0)

checks = 0
def check(actual,expected):
    global checks
    assert actual == expected,(actual,expected)
    checks += 1
def reject(fn):
    global checks
    try: fn()
    except Invalid: checks += 1
    else: raise AssertionError('expected rejection')

def es_entry(key,tail): return pack('HHBHH',key,7,8,9,10)+tail
for key,descriptors in ES.items():
    tail = b''.join(b's\0' if d==5 else b'\x11'*{0:1,1:2}.get(d,4) for d in descriptors)
    fixture = b'\1\1'+es_entry(key,tail)+b'\0'*7
    r = Reader(fixture); check(es(r)[0][-1],tail); r.end()
    reject(lambda: es(Reader(fixture[:-1])))
for n in (0,1):
    special = pack('H',n)+b'\0'*(4*n)+pack('I',2)+pack('HH',8,9)+(pack('H',10) if n else b'')
    r = Reader(b'\1\1'+es_entry(1425,special)+b'\0'*7)
    check(es(r)[0][-1],special); r.end()
reject(lambda: es(Reader(b'\1\1'+es_entry(65535,b'')+b'\0'*7)))
reject(lambda: es(Reader(b'\1\1'+es_entry(1425,pack('HI',0,0xffffffff)))) )
reject(lambda: es(Reader(b'\1\1'+es_entry(1388,b'no terminator'))))

def player_fixture(mask=0,title=0,a=0,b=0):
    data = pack('BIB',3,4,mask)
    for bit in (1,2,4,8,16):
        if mask & bit: data += (b'\x02' if bit==8 else b'')+b'x\0'
    data += pack('I',a)+b''.join(pack('I',100+slot) for bit,slot in A if a&bit)
    data += pack('I',b)+b''.join(pack('I',200+slot) for bit,slot in B if b&bit)
    data += b'\0\0\0'+pack('BBIBB',1,2,3,4,5)
    if mask & 32: data += pack('III',2,6,7)
    data += pack('HH',8,9)
    if mask & 64: data += b'x\0'
    data += pack('HIIBH',title,10,11,0,0)
    if title in (4,6): data += b'partner\0'
    return data+b'\x01'
minimal = player_fixture(); check(len(minimal),43)
for mask in (0,1,2,4,8,16,32,64,127):
    for title in (0,4,6):
        fixture = player_fixture(mask,title,sum(x for x,s in A),sum(x for x,s in B))
        r = Reader(fixture); value = player(r); r.end()
        check((value[0][0],value[1][0],value[0][7],value[1][7]),(100,200,107,107))
        check(value[3],b'partner' if title in (4,6) else None)
        reject(lambda: player(Reader(fixture[:-1])))
reject(lambda: player(Reader(pack('BIB',0,0,128))))
reject(lambda: player(Reader(pack('BIBI',0,0,0,8))))
overflow = player_fixture(32)
reject(lambda: player(Reader(overflow[:25]+pack('I',0x20000000)+overflow[29:])))

def appear_fixture(kind,tail):
    return pack('HHHBBhIHHHH',100,kind,9,1,2,3,0,10,20,10,20)+b'\0'+pack('BBBI',0,0,4,0)+b'\0'+pack('I',0)+tail
for kind,tail,size in [(1,minimal,78),(2,pack('HI',0,3),41),(3,pack('H',101),37),
                       (4,pack('H',101),37),(5,b'',35),(14,b'',35),
                       (16,pack('H',101)+pack('BIIIB',1,2,3,4,5),51)]:
    fixture = appear_fixture(kind,tail); check(len(fixture),size)
    check(appearance(fixture)[1],kind)
    reject(lambda: appearance(fixture[:-1]))
check(appearance(appear_fixture(16,pack('H',101)))[3],(101,None))
reject(lambda: appearance(appear_fixture(3,pack('H',101))+b'\0'))

item1 = pack('IQH',1001,42,3)
item2 = pack('IQH',1002,43,1)+b'\1\2\3\4'
initial = pack('H',2)+b'\1'+item1+b'\2'+item2+b'\1'+pack('III',9,10,11)
items = inventory(initial); check(set(items),{42,43}); check(inventory(b'\0\0'),{})
reject(lambda: inventory(initial[:-1])); reject(lambda: inventory(b'\1\0\xff'))
start = pack('QQ',42,1)+b'\0'*168+b'\0'*47+pack('I',123)+b'\0'*16
check(len(start),251)
slots,galder,missing = equipment(start,items)
check((slots[0],slots[1],galder,missing),(42,0,123,set()))
check(equipment(start,{})[2],{42}); reject(lambda: equipment(start[:234],items))
apply(items,slots,change(b'\1\1'+pack('IQH',1001,42,7))); check(items[42]['count'],7)
apply(items,slots,change(b'\1\2'+pack('IQH',1002,43,99)+b'\5\6\7\10'))
check((items[43]['count'],items[43]['state']),(1,b'\5\6\7\10'))
apply(items,slots,change(b'\3\1'+item1)); check((42 in items,slots[0]),(False,0))
apply(items,slots,change(b'\1\1'+item1)); check(42 in items,False)
apply(items,slots,change(b'\2\1'+item1)); check(items[42]['count'],3)
reject(lambda: change(b'\2\2'+item2+b'\1'+pack('II',9,10)))
# Feedback and ground appearance never call the inventory reducer in this model.
snapshot = dict(items); auto_event(6,'Result'); check(items,snapshot)
check(pack('HHB',10,20,2).hex(),'0a00140002')
for phase in (0,3,5,6): check(auto_event(phase,'Result'),(7,[790]))
check(auto_event(2,'SpotTaken'),(3,[])); check(auto_event(3,'SpotTaken'),(3,[]))
check(auto_event(5,'DrillEnded',7),(5,[])); check(auto_event(5,'DrillEnded',6),(7,[790]))
check(auto_event(4,'DrillEnded',7),(7,[790]))
check(auto_stop(3,False),dict(killed=True,send=[],stop_move=False))
check(auto_stop(0,True),dict(killed=True,send=[794],stop_move=True))


def ground(op,b):
    r = Reader(b)
    source = r.get('H') if op in (12,14,617) else None
    ident = r.get('H'); pos = struct.unpack('<HH',r.take(4))
    origin = struct.unpack('<HH',r.take(4)) if op in (13,14) else None
    shadow = r.get('H') != 0; r.take(3); kind = r.get('B')
    item,uid,count = r.get('I'),r.get('Q'),r.get('H'); r.end()
    return source,ident,pos,origin,shadow,kind,item,count

def picked(b):
    r = Reader(b); actor,ident = r.get('H'),r.get('H')
    item = body(r,r.get('B')); r.end()
    return actor,ident,item

def bang(b,love):
    r = Reader(b); count = r.get('i')
    if count < 0 or count > 10: raise Invalid('unsupported bang count')
    actor = r.get('H'); items = []
    for _ in range(count):
        ident = r.get('H'); origin = r.take(4); pos = r.take(4); r.take(3)
        items.append((ident,body(r,r.get('B'))))
    cake = None if love else r.get('B'); r.end()
    return actor,items,cake
for op,length in ((10,26),(11,26),(12,28),(13,30),(14,32),(617,28)):
    fixture = (pack('H',100) if op in (12,14,617) else b'')+pack('HHH',101,10,20)
    if op in (13,14): fixture += pack('HH',30,40)
    fixture += pack('H',1)+bytes(3)+b'\1'+item1
    check(len(fixture),length); check(ground(op,fixture)[1],101)
    reject(lambda: ground(op,fixture[:-1]))
fixture = pack('HHB',100,101,2)+item2
check(len(fixture),23); check(picked(fixture)[2]['uid'],43)
reject(lambda: picked(fixture[:-1]))
for love in (False,True):
    fixture = pack('iH',1,100)+pack('HHHHH',101,10,20,30,40)+bytes(3)+b'\1'+item1
    if not love: fixture += b'\2'
    check(len(fixture),35 if not love else 34)
    check(bang(fixture,love)[0],100); reject(lambda: bang(fixture[:-1],love))
reject(lambda: bang(pack('iH',11,100),False))
reject(lambda: bang(pack('iH',-1,100)+b'\0',False))
print(f'PASS: {checks} synthetic structure, rejection and selected state checks')
```

Validation also includes readback of this exact Markdown's extracted code, valid reference definitions, a single-report Git diff, whitespace checks, and byte-for-byte published-file readback after normal push. The issue comment supplies the final immutable commit SHA and remote verification; a report cannot embed its own commit hash without changing that hash. No application test/build, live login/action, or independent reviewer is claimed. Scratch is retained outside Git for reproduction, with no running analysis process or account resources to release.

## Sources

[WO]: https://tr-play.playall.in.th/Build/657890b2bd8548390dd7c7fad76093be.wasm.gz
[DO]: https://tr-play.playall.in.th/Build/9b2286992122d3c1cc081492dc31fee5.data.gz
[WC]: https://tr-play.playall.in.th/Build/d9a4c4fd2737e740b82e1321839e352f.wasm.gz
[DC]: https://tr-play.playall.in.th/Build/6135c4139b62e558bb4c755b266f5fa2.data.gz
[I14]: https://github.com/oDestroyeRo/opentrickster/issues/14
[MAP]: https://github.com/oDestroyeRo/opentrickster/issues/2
[R13]: https://github.com/oDestroyeRo/opentrickster/blob/331f397fe2d6440bc24b9ee51f9b7caa6100962b/docs/research/target-protocol-framing.md
[R14]: https://github.com/oDestroyeRo/opentrickster/blob/3a6c421d903483f1c977f6006fe01476d8da3223/docs/research/target-state-message-layouts.md
[R15]: https://github.com/oDestroyeRo/opentrickster/blob/97d6bf407a23bf87d054b40571d14224d0fb32ac/docs/research/target-authentication-handoff.md
[R17]: https://github.com/oDestroyeRo/opentrickster/blob/09a9dd7434d7bb56d6954220f4f6d942491bbb40/docs/research/native-startup-proof.md
