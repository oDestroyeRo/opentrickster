# Derived resource maxima on build ea1a68bd

Issue [#47](https://github.com/oDestroyeRo/opentrickster/issues/47). Static research completed **2026-11-09 UTC**, and the `GameParts` follow-up **2026-11-10 UTC**, for build **ea1a68bd89958fae94179f1e0835ffb5** only. The question: can Net derive maximum HP, MP and weight (stats 9, 3 and 6) without S282, exactly as `CharacterStatus.Total` does?

**Yes, for the installed steady state, and only while every input is proven.** Every packet-fed term of `Total` is modelled exactly in `crates/net`, and so is the `GameParts` term: the worn equipment snapshot, the own set bonuses and the imprint bonus. Net reports `GameParts + core`, the `Total` the client computes once `GameParts` is installed. Any input Net cannot prove leaves the maximum unknown, and a received S282 total still wins.

Two limits remain. The client attaches `GameParts` only after an asynchronous UI install that no frame marks, so Net reports the state the client converges to, not its state before the install. Whether the server enforces the same maximum cannot be proven from the client build.

A live oracle, kept outside Git, checks Net's core, `GameParts` and maximum against the client's own memory. It computes the client's `GameParts` from the client's own snapshot and tables. It was not run for this report.

## Evidence boundary

All citations are to build ea1a68bd, by method name and WASM function index (`f<index>@<body offset>`). Methods were joined by canonical name with `tools/build-migrate`; no function index from an older build was reused. "Folded" marks a body that the compiler shares between several methods; the citation names the method this build calls. The decoded WASM is `8780db4414df5357d9595ef233579227a1303753d00ac6318d225036dd1bb277`, the same asset as the [native migration](build-ea1a68bd-migration.md).

This work parsed public bytes only. It performed no login, game socket, gameplay or account access, and it never instantiated or executed target code. Tests use synthetic fixtures. No captures, payloads, UIDs, item names or character values appear here.

## `CharacterStatus.Total`

`CharacterStatus::Total(int stat, int base)` f12568@`3840d1` returns 0 for `(uint)stat > 11`. It returns `Totals[stat]` when that nullable entry has a value. Otherwise it returns this wrapping I32 sum:

```text
(GameParts != null ? GameParts(stat) : 0)
  + (recomputedBase[stat] ?? base) + Buffs[stat] + recomputedAura[stat]
```

`base` comes from `LevelTables::PlayerTotal` f17531@`54400d`. It is `isNaN(v) ? 0 : trunc_sat_i32(v)` with `v = LevelTables::StatBase(Points, stat)` f17622@`54e05f`. `CombatData::StatLevel` f17623@`54e151` gives `L = floor(Points[stat] * 0.25) + 1`. `StatBase` returns 0.0 when `L - 1 > 998`; otherwise it reads `player[((L - 1) * 12 + stat) * 3 + 2]`. `CombatData` `<Load>d__15::MoveNext` f17632@`54ebcf` loads the player table from `gulim13.fnt2`: the literal's SHA-256 is `828056b1…`. The pinned `level_tables.player` in `game_definitions.json` equals that asset's 35,964 values.

`CombatModel::OnTotalsChanged` f17535@`54421a` evaluates `PlayerTotal` for 9, 3 and 6 with the live `Points` and stores the results in `CombatStats` `MaxHp` (+24), `MaxMp` (+40) and `MaxWeight` (+48). The displayed maxima are therefore cached at the last `TotalsChanged`, not evaluated continuously.

`CharacterStatus` field offsets: `Points` +8, `Buffs` +36, `BaseOverrides` +40, `Totals` +44, `GameParts` +48, `TotalsChanged` +52, `recomputedBase` +56, `recomputedAura` +60, `seed` +64 and `session` +104. The constructor f12553@`38242b` sets `Points`, `Buffs` and `recomputedAura` to zeroed `int[12]`, and `BaseOverrides`, `Totals` and `recomputedBase` to null `Nullable<int>[12]`.

## Inputs

| Term | Client source | Frames | Net |
| --- | --- | --- | --- |
| `Totals[s]` | `<.ctor>b__23_6` f12576@`384900`. `ParseIndexedValues(wide)` f12544@`381e11` reads count U8, then (stat U8, value I32) pairs. Stats ≤ 11 are kept. `Write` f12567@`384059` writes only stat 2 (callers f16903 and f45389). | 282 | Implemented, unchanged. It still wins over every derived term. |
| `Points` → `base` | StartSet 233 of at least 217 bytes: `ApplyStart` f12554@`383471` calls `ReadStatBlock(payload, 184)` f12541@`381a85`, then `Recompute(true)`. `ApplyBlock` f12565@`383eda` reads the same block when `len ≥ offset + 33`: 39 `b__23_2` f12571 and 629 `b__23_9` f12579 at offset 0; 634 `b__23_11` f12581 at offset 0 after reading U32 bytes 33..37; 649 `b__23_13` f12583 at offset 69. 109 `b__23_3` f12573 uses `ParsePointsAdded` f12543@`381d0a`: U16 bonus, then 12 U16 points. The block is 12 U16 points, bonus U16 at +24, TM level U16 at +28 and level U16 at +30. A later 233 has no `CharacterStatus` hook. | 233 (StartSet), 39, 629, 634, 649, 109 | Implemented. Uses the level table from either definitions source. |
| `BaseOverrides` → `recomputedBase` | 784 `b__23_7` f12577@`384bd1`: wide indexed values; stats ≤ 11 set `BaseOverrides[stat]`; no recompute. `Recompute(clear)` f12557@`38381a` clears `BaseOverrides` when `clear` is set, then copies it into `recomputedBase`. | 784, plus every recompute | Implemented. |
| `Buffs` | 283 `b__23_5` f12575@`3845ce`: wide indexed values; stats ≤ 11 overwrite `Buffs[stat]`; unlisted stats keep their value. Then the `Recompute(false)` body runs inline. | 283 | Implemented. |
| `SoulSeedLink.Aura.Bonus` → `recomputedAura` | `SoulSeedLink` ctor f12516@`37fcfe` replays the StartSet through `Receive` f12517@`380266`, then hooks 1065, 1068, 1070, 1071, 1072 and 1073 (RVA `5EC725B7…`). Effects: 1070 sets `Aura = ParseAura` f12506@`37eb06` (I32 type, remaining and level, then 12 I32 bonuses) and emits `AuraStarted`; 1071 resets the aura and emits `AuraEnded`; 1065 runs `ParsePlanted` f12504 and then resets the aura with no aura event. `SoulSeedAura` ctor f12484 zeroes `Bonus`. `CharacterStatus` `b__23_0` f12569 runs the `Recompute(false)` body on `AuraStarted` and `AuraEnded`. `Recompute` copies `Bonus[i]`, or 0 past its length. | 1065, 1070, 1071 | Implemented. |
| `GameParts` | `AvatarStats::Part` f13849@`3db5d2`, set only by the `AvatarStats` ctor f13847@`3dad29` once the asynchronous `AvatarStatsUi` install completes. It adds the worn equipment snapshot, the own set bonuses and the imprint bonus. See [`GameParts`](#gameparts). | Item deltas (68, 561, 160, 770, 262, 246, 260, 843), 518, 521, 1386, 1387, 1487 and 1489 in and after the StartSet; StartSet 231 and 233; live 82, 85, 151 and 837; 230 and 437 for `Me`; any of 181 denied opcodes. | Implemented for the installed steady state. Unknown whenever an input is unproven. |

The handlers for 631, 636 and 651 (`b__23_10`, `_12` and `_14`) only emit a `Reset` event, so they are not inputs. 826 `b__23_8` writes `Properties`, which `Total` does not read. 245 and 285 (`b__23_17`) write only TM level and TM experience.

## When each input changes

`Recompute(true)` runs on a StartSet 233, on every applied stat block (39, 629, 634 and 649), on 109, and on every `ImprintEvent` through `b__23_1` f12570. `ImprintLink` f13005 hooks 1386 (`Listed`), 1387 (`Used`) and 1389 (`Ended`), from RVA `218D32A0…`. The `Recompute(false)` body runs on 283, on `AuraStarted` (1070) and `AuraEnded` (1071), and through `AvatarStats` on 82, 85 and 837 once installed. Every recompute clears `Totals` and raises `TotalsChanged`.

Startup has its own order. `GameSession` ctor f20878@`65bce5` records the StartSet only during `BindGameServer` f20880. `CharacterStatus` ctor order:

1. `Soul::SeedOf` f12509 creates the `SoulSeedLink`, which replays the StartSet.
2. `ApplyStart` runs for each StartSet frame.
3. The ctor subscribes to aura and imprint events.
4. The ctor registers its frame hooks through the session's hook slot.

So StartSet aura frames reach the live aura before the 233 recompute, and they trigger no recompute of their own. StartSet 39, 109, 283, 784, 629, 634 and 649 never reach `CharacterStatus`.

## `GameParts`

`GameParts` is `AvatarStats::Part`. Net derives it in `crates/net/src/parts.rs` as the client's steady state with `GameParts` installed, and only while every input below is proven.

### Composition

`Part(stat)` f13849@`3db5d2` is the wrapping I32 sum of three terms:

- `Equipment(stat)` f13850@`3db64d`, over the worn snapshot;
- `SetItems(stat)` f13851@`3db7cb`, over the own set list;
- the imprint term, an inlined `Imprints` f13852@`3dbbe3`. `ImprintModel::Of` f24893@`7a1700` only looks the session's model up and creates none, so with no model the term is 0. With a model, `ImprintList::StatBonus` f24876@`7a0005` returns `accumulators[stat]` for a stat ≤ 11.

`AvatarStats` fields: session +8, `CharacterStatus` +12, `Inventory` +16, `ProfileTitles` +20, the item-info delegate +24, the set-bonus delegate +28 and the worn snapshot +32.

### Equipment

`Rebuild` f13848@`3db34f` fills a 23-entry snapshot. A zero slot UID marks an empty slot, and its entry stays null: `i64.eqz` at `3db4b4` skips the slot before the `Inventory::Get` f12892@`397d1f` call at `3db542`. For every other slot UID, `Rebuild` stores `InvItem::Clone` f13049@`39f52f` of the `Get` row, or null when the inventory holds no such row. The snapshot is a copy, so a later change to a worn row reaches `Part` only at the next `Rebuild`. `Rebuild` also calls `EquipmentBonuses::ItemProperties` f20522@`63e30e`, which `Part` does not read.

`Equipment` skips null entries and rows whose `ItemId` is 0. For every other entry it adds the inlined `ItemStat` f13853@`3dbc40`:

1. `Info` f13854@`3dbcdb` calls the item-info delegate. A null `ItemInfo` skips the entry.
2. `ItemTip::PropertiesOf` f25257@`7c34e3` wraps `ItemTip::PropertyEntries` f25255@`7c2f76`. `ItemTip/Properties::Sum` f19710@`5fcedc` adds the values of the entries whose property is `10000 + stat`. A non-zero sum, converted with saturation, is the entry's value.
3. Otherwise the value is `StatPlus[stat]`, converted with saturation and narrowed to I16. It is 0 past the end of the list or for a null list.

`PropertyEntries` returns null unless the row's wire `Kind` is 2, and `Sum` of null is 0. For Kind 2 it lists:

1. each option, while the list holds at most 31 entries. The entry kind is the option `Id`, the property is the low 16 bits of `A`, and the value is the signed high 16 bits of `A`;
2. then each non-zero `StatPlus[i]`, while the list holds at most 31 entries, as kind 10 with property `10000 + i`. It is skipped when a kind-10 entry with that property already exists (`<PropertyEntries>b__1` f19718@`5fd80c`, folded).

`Items::ReadItemChange` f12862@`395d6a` reads options only for ops 2 and 4 of Kind 2 (`ReadItemBody` f12859@`3955db` reads the rest). A row stored by ops 6 or 8 therefore has an empty option list in the client.

Net follows these steps exactly (`parts::item_stat` and `property_sum`). A worn row whose catalog entry or `StatPlus` list is missing from Net's definitions makes the term unknown, not 0, because the client loads its own table.

### Catalog `StatPlus`

`ItemInfo.StatPlus` (+140) is the item's list of catalog stat bonuses, or null. Net now keeps it on both definitions paths:

- the resolved catalog keeps the stat list that `catalog::items` used to skip. A negative length is the client's null list;
- `items.json` carries it as `StatPlus`.

In the ea1a68bd resolved catalog, every one of the 18,118 rows carries 12 values (`verified_public_catalog`). The pinned 8502ebbf `items.json` (`50d82abd…`) also carries 12 values for each of its 18,118 rows. Net refuses that JSON for ea1a68bd, so the resolved catalog is the only source on this build. No pin or hash changed: both sources already held the column.

### `SetItemParam`

The install's second table is `SetItemParam`, loaded through `GameAssets::Table` f30675@`9770c6`. `SetBonuses` f13858@`3dbff1` keys each row by `Long(ID)` and keeps the first row per key. It stores 12 bonus columns, each as `Long` narrowed to I16. `Long` f13859@`3dc376` parses the trimmed text as an invariant integer, else 0. The `.cctor` f13861@`3dc775` names the columns in stat order. `MaxMPPlus` is stat 3, `MaxWTPlus` stat 6 and `MaxHPPlus` stat 9.

The resolved catalog now decodes this table through the existing pinned-table path: 247 rows, SHA-256 `2ef5e986e0865015a1618c8bad923a694a14eb711a58cd851c12d15c9d56e022`. Non-zero bonuses occur in 138 rows for stat 9, 87 for stat 3 and 16 for stat 6. Neither `libconfig` overlay defines the table; both still hold 127 tables. `tools/build-migrate` now lists `SetItemParam` among the relevant tables, so a future overlay that defines it is refused. The JSON definitions do not carry the table.

### Sets

`SetItems` f13851@`3db7cb` reads `GameSession.Me` through `IGameSession.get_Me`. The getter, folded into f13284@`3ae92e`, loads `<Me>k__BackingField` (+96), a `StartState` whose `Id` is at +8. `SetItems` then calls `ProfileTitles::Sets(Me.Id)` f21635@`693201`. It adds the U16 bonus `rows[set][stat]` once per listed set entry, skipping a null row and a stat past the row's end. It returns the sum narrowed to I16. A set ID without a `SetItemParam` row therefore adds nothing.

The `ProfileTitles` ctor f21664@`695847` registers four hooks and replays no StartSet frame:

- 999 `b__0` f21671@`695f94` changes titles only.
- 837 `b__1` f21672@`69612a` calls `ServerNotices::ParseSetItem` f12335@`375753`. It reads the character as the low 16 bits of a U32, the set as a U32 and completion as a U8; a shorter frame throws first. `OnSetItem` f21667@`695c05` returns when `KnownSet` rejects the set. Otherwise it appends a complete set to that character's list, or swap-removes one matching entry for an incomplete set.
- 15 `b__2` f21673@`696143` removes another character's lists and returns early for `Me`.
- 437 `b__3` f21674@`6962b9` takes `Me`'s list, clears every list, and stores `Me`'s list under the new ID from the frame's first U16. `GameSession::Handle` f20915@`65de1f` then moves `Me`.

`KnownSet` has a folded setter (f12608). `ProfileTitleUi` `<Install>d__3::MoveNext` f41701 sets it to a membership test over the `SetItemParam` IDs that `ProfileTitleUi::Tables` f41691@`d4f679` loads. Whether or not that UI is installed, the filter drops only IDs without a bonus row, which add nothing anyway.

Net takes `Me` from the first 230 and moves it on each live 437. It keeps the own list as a multiset of set IDs and applies each live 837 for `Me`. When the definitions carry the `SetItemParam` table, Net skips an 837 whose set has no row, whichever character it names, because that set adds nothing to any list. Net ignores StartSet 837 frames, because `ProfileTitles` never sees them. The term becomes unknown when:

- a live 230 names a different character, because the own list may now be another character's;
- an 837 that Net does not skip arrives while `Me` is unknown. A StartSet 230 or 437 that names a different character makes `Me` unknown until the next live 437;
- a complete set would bring the own list past 1024 distinct set IDs;
- the own list is not empty and the definitions carry no `SetItemParam` table.

### Imprints

The `ImprintLink` ctor f13005@`39c8c8` replays the StartSet through `Receive` f13006@`39ce5d` and hooks 1386, 1387 and 1389. It keeps a log of raised events for a later `ImprintModel`, and clears that log before it parses a 1386.

- `Imprint::ParseList` f12998@`39c535` reads a U8 count, then 24-byte entries, and raises `Listed`.
- `ParseUsed` f12999@`39c62a` reads U32, U64 and U64, then an item change. Only a complete parse applies the change to `Inventory` and raises `Used`.
- `ParseEnded` f13000@`39c6e9` raises `Ended`.

A parse that throws raises no event.

The `ImprintModel` ctor f24891@`7a0e5c` builds its `ImprintList`; the list ctor f24868@`79fad2` zeroes the accumulators. `ImprintModel::Apply` f24892@`7a13db` applies each event with `Clear` f24878@`7a0048`, `Add` f24880@`7a00cc` and `Remove` f24882@`7a03e8`. `Listed` with no entries clears the list. Neither `Ended` nor the model's tick can add to an empty list.

Net does not model list entries. It proves the term 0 only while the list is provably empty: from the start, and after a parsed 1386 with count 0. A parsed 1386 with entries, or a parsed 1387, makes the term unknown until the next empty 1386. A malformed 1386 changes nothing: an empty list stays empty either way.

### Inventory behind the snapshot

`Part` reads the client's `Inventory` rows and slots, so Net mirrors them for every worn UID. A UID-0 row is never worn: `Rebuild` skips a zero UID, and clearing a slot writes zero, so no change to that row moves a slot.

**Replay.** The `Inventory` ctor f13069@`3a0235` replays the StartSet in two passes, then hooks live frames. The first pass stores the 231 rows and the 233 slots. A 233 slot keeps a UID only when the row is already held. The second pass runs `Special` f13070@`3a1b4d` for 518 and 521. StartSet item deltas are not replayed, so their rows are unproven until a later StartSet 231 lists them again. `ImprintLink` applies StartSet 1387 item changes after this replay.

**Hooks.** `SetSlot` f13071@`3a1c07` stores a UID in a slot and clears the first other slot that holds it. `Apply` f12891@`3977fd` replaces the row for ops 2, 4, 6 and 8 of Kind 1 or 2. For ops 3, 5 and 7 it removes the row and clears the first slot that holds its UID. The live hooks are:

| Opcode | Hook | Effect on rows and slots |
| --- | --- | --- |
| 82 | `b__5_13` f13102@`3a368c` | `SetSlot` for a held UID |
| 85 | `b__5_14` f13103@`3a3732` | Clears the slot |
| 518, 521 | `b__5_0` f13086@`3a2d7d`, `b__5_1` f13087@`3a2d8e` | `Special`: 518 stores a held UID in slot 20; 521 empties slot 20 when it holds that UID |
| 68 | `b__5_11` f13100@`3a347c` | `Apply` to a held row only |
| 561, 770, 160 | `b__5_2` f13088, `b__5_3` f13089, `b__5_4` f13090 | `Apply` |
| 262, 246, 260 | `b__5_21` f13095@`3a324a` | `Apply` of up to three entries |
| 843 | `b__5_7` f13096 | `Apply` |
| 151 | `b__5_18` f13107@`3a3884` | Emits `Used` for a held row; `ItemsModel::OnInventory` f19793@`6050a4` then consumes a piece, possibly the row |
| 41, 83, 86, 35 | `b__5_12` f13101@`3a35f5`, `b__5_15` f13104@`3a37ea`, `b__5_16` f13105@`3a381d`, `b__5_17` f13106@`3a3850` | Emit an event only |
| 261 | `<>c::b__5_10` | None. It is folded into a two-byte empty body (f1391@`bb66c`), so 261 is not denied |
| 586, 590, 1152, 587, 599, 601 | `b__5_5` f13091, `b__5_6` f13093, `b__5_8` f13097, `b__5_9` f13098, `b__5_22` f13099@`3a33dd` | No decoder grammar; 599 and 601 `Apply` a `ParseGive` f13060 result. Denied |

The client ignores a live 231 or 233. After H, Net applies a 231 alone, so a listed row that differs from Net's cached row becomes unproven.

**Rebuild triggers.** `AvatarStats::OnEquip` f13856@`3dbd46` runs on 82 and 85 after the `Inventory` hook. When the payload holds at least 8 bytes and `Inventory::Get` of its leading UID is non-null, it calls `Rebuild` and `Recompute(false)`. `Special` never rebuilds.

**Local consumers.** Some client code changes rows with no frame:

- `GunFlowing::Special` f24377@`779f15` and `SpecialOn` f24379@`77a1af` set `Loaded` (setter folded into f7167) to the ammunition in slot 20. `TakeBunch` f24403@`77b9af` and `Fired` f24402@`77b908` then consume it locally, also through `GunFlowingUi` `<Install>b__2` f46663@`f4b651`. Net treats a slot-20 row of catalog Type 29 without `FlagsEx` bit 64, or with no catalog row, as volatile once loaded. Such a row makes the term unknown when it adds a non-zero value.
- `ItemHolders` ctor f25046@`7b0545` hooks 518 and 521 (`b__19_0` f25056, `b__19_1` f25057) into its own `Special` f25047@`7b0ae7`. `Take` f25052@`7b1010` calls `Inventory::Consume` f13081@`3a2426`. `ConsumeLoaded` f25053@`7b14e0` calls `Take`. `ConsumeThrown` f25054@`7b1555` calls `Take` and `Inventory::Apply`. Skill scripts reach them through `SkillEsaView/Host::ConsumeLoaded` f45405@`edfa09` and `ConsumeItem` f45407@`edfb00`. Net does not model them.
- `TutorialSystemUi/<>c__DisplayClass59_0::<Install>b__0` f40934@`cfeb3f` wraps one `ItemChange` and calls `Inventory::Apply`. Its `TutorialCourse` source was not traced.

**Denied opcodes.** A backward call and delegate trace from the `Inventory` mutators found every opcode whose handler can change rows or slots outside the modelled hooks: 183 in all. Net models two of them, 1487 and 1489, as [soul frames](#soul-frames). `parts::UNMODELED` lists the other 181. Any of them, StartSet included, makes `GameParts` unknown for the rest of the session.

| Owner | Opcodes |
| --- | --- |
| `Inventory` hooks without a grammar | 586, 587, 590, 599, 601, 1152 |
| `NpcLink` | 71, 73, 74, 75, 77, 78, 112, 113, 115, 116, 117, 168, 169 |
| `TradeLink` | 94, 95, 96, 98, 100–105, 170, 171, 174, 175, 178, 179 |
| `PlayerShopLink` | 342, 343, 345, 346, 348, 349, 351, 353, 354, 356, 358, 360, 363, 367–370, 449, 473, 558, 853 |
| `PetWindows` | 706, 859, 860, 862, 863, 865, 866, 878, 879, 881, 882, 884, 885, 913, 914, 916, 917, 919, 920, 976, 979, 980, 984, 986, 987, 1208, 1209, 1211, 1212, 1214, 1215, 1217, 1218, 1287, 1288, 1290, 1291, 1293, 1294 |
| `GuardianState` | 1098, 1100, 1102, 1103, 1106, 1108, 1141, 1142, 1144, 1173, 1174, 1177, 1178, 1179, 1184, 1185, 1488 |
| `SoulLink` | 1091, 1465, 1467, 1468, 1470, 1471, 1473, 1474, 1476, 1477, 1479, 1480, 1482, 1490, 1491 |
| `QuestBook` | 150, 195, 219, 831, 873 |
| `EquipComposingLink` | 859, 860, 862, 863, 865, 866 |
| `EquipMergeLink` | 1270, 1271, 1273 |
| `HarconStageFeature` | 1365–1369 |
| `ReformLink` | 904, 905, 907, 908, 910, 911, 1019, 1021 |
| `RecycleLink` | 1010, 1014, 1016 |
| `RefineLink` | 697, 700, 1450 |
| `WarehouseLink` | 140, 143, 199, 202 |
| `GuildDepotLink` | 1312, 1314 |
| `MailLink` | 1237, 1240 |
| One opcode each | `ItemsModel` 89, `CompoundLink` 190, `ExchangeShopLink` 255, `PlayerNoticesFeature` 293, `CashShopLink` 335, `ItemMixLink` 705, `GuildBattleFeature` 720, `ResetMixLink` 735, `LookChangeFeature` 788, `MegaphoneItems` 802, `RShopLink` 898, `FortuneLink` 975, `MyViewRecords` 1088, `DecomposeLink` 1266, `BoxesModel` 1284, `HarconShopLink` 1391, `MeltingLink` 1429, `SlotExtendLink` 1435, `GhostExchangeLink` 1443 |

Two registrations do not name a literal opcode. `PlayerShopLink` `b__3_22` and `b__3_23` handle 32531 and 32532, which only open a window or emit `Logout`. `EquipMergeLink` `b__23_3` handles 1276 and 1277, which only emit `Closed`. None of them is denied.

### Soul frames

A 1489 and a 1487 arrive right after H at login. Two components hook them, and neither exists until a UI or feature creates it:

- `SoulLink` hooks both, from RVA `160A8C84…`. `Soul::SoulOf` f12508@`37ecdd` creates it for `TartarosFeature`, `SoulSeedFeature`, `StatusBarUi` and `GuardianUi`.
- `GuardianState` hooks 1487. `GuardianState::Of` f23856@`74d227` creates it for `SoulUi` and `GuardianUi`.

Each ctor, `SoulLink` f12513@`37ef8d` and `GuardianState` f23857@`74d309`, replays the StartSet through `Receive` and then hooks its opcodes. Every other I32 constant or static array that holds 1487 or 1489 belongs to a script, effect, prime or Unicode table. The handlers parse the whole frame before they act:

- **1489.** `SoulLink::Receive` f12514@`37f4ee` calls `Soul::ParseSync` f12492@`37deaf`. It reads an I32, then `Soul::ReadChanges` f12488@`37dbd2` reads a U16 count and that many `Items::ReadItemChange` f12862@`395d6a` into a new list. `Receive` then sets `Soul` and emits `SoulChanged`. For each change whose UID `Inventory` holds, it calls `Inventory::Consume` f13081@`3a2426` for one piece. Last, it passes the list to `Inventory::Apply`, which only raises `ItemsChanged` when the list is empty.
- **1487 in `SoulLink`.** A payload shorter than 4 bytes returns. Otherwise `SetSoul` f12520@`3809ba` stores an I32 and emits `SoulChanged`.
- **1487 in `GuardianState`.** `Receive` f23858@`74de23` calls `SoulManage::ParseState` f12531@`381137`. It reads 37 bytes of fixed fields, then a U8 flag at offset 37 and, when the flag is non-zero, a `ReadItemChange` at offset 38. `PenaltyLine` f23861@`74ee69` then shows or queues a system line. `Receive` stores the state, passes a parsed change to `Inventory::Apply`, and invokes `Changed`.

Nothing else reaches a row or slot:

- `SoulLink::Emit` f12518@`3804ca` catches each listener's exception. `SoulLink::On` f12521 has two callers. `TartarosFeature::OnEvent` f18214@`58358f` ignores `SoulChanged`. `StatusBarUi` `b__5` f41025@`d079fb` calls `StatusBar::SetSoul` f40864@`cfa528`, which stores two fields and repaints.
- `GuardianState::add_Changed` f23854 has two callers. Their handlers are `GuardianPageView::OnChanged` f45627@`ef57f1` and `MyViewGuardianPage::OnChanged` f58358@`14335a6`.
- `Inventory::Emit` f12893@`397d86` catches each listener's exception.
- The direct-call closures of the parsers, `SetSoul`, `Emit`, `PenaltyLine`, these listeners and `Inventory::Emit` reach no `Inventory` mutator, `AvatarStats::Rebuild` f13848 or `CharacterStatus::Recompute` f12557. Both `Receive` closures reach `Inventory::Apply`.

A read past the payload throws from `PacketReader::Need` f9830@`2e1ed2`, before any effect. `GameSession::Handle` f20915@`65de1f` catches a handler's exception, logs it and runs the next handler (`65dff6`). `LateHooks::Flush` f13119@`3a3fc2` does the same for the frames it replays to a late hook (`3a4496`). A ctor's StartSet replay does not catch: its `finally` rethrows (`37f2be`, `74d806`), as `ImprintLink`'s does (`39cc22`). The install that created the component then fails. Each ctor sets every field its replay touches before the replay, and `Items::InventoryOf` f12878@`396a2d` creates the inventory on demand, so a frame that parses cannot throw there.

Net applies these rules in and after the StartSet:

- A 1489 with count 0, or a 1487 of at least 38 bytes with flag 0, leaves `Parts` unchanged. The client ignores trailing bytes.
- A frame that parses with an item change makes `GameParts` unknown. Whether a lazy component exists to apply it, and when its ctor replays a StartSet change, is unobservable.
- A frame that throws leaves `Parts` unchanged after H. In the StartSet it makes `GameParts` unknown, because it fails a later install.

The login frames, a 6-byte 1489 and a 38-byte 1487, are too short to hold an item change. They keep the maxima whatever their count or flag.

The other `SoulLink` and `GuardianState` opcodes stay denied. 1467, 1473, 1479 and 1491 change rows through `Inventory::Apply` or `Soul::ApplyProperty`. The others reach the same `Receive` and were not modelled.

### Install and steady state

`PlayFeatures::.cctor` f37436@`b8c21b` registers `AvatarStatsUi.Install`. Its state machine `<Install>d__0::MoveNext` f40539@`cd06b6` awaits `ItemData::ItemInfos` f24953@`7a52c3` and the `SetItemParam` table together. It then calls the `AvatarStats` ctor at `cd0e3c`. The closure holds the item infos at +8 and the set rows at +12.

- **Failed loads.** The closure keeps the item infos only when their task ran to completion. When the set task did not complete, the install passes `SetBonuses(null)` and logs through `Debug::LogWarning` f4036@`1749ff`. A failed load leaves `Part` without the equipment term or the set term.
- **No recompute.** The ctor stores `GameParts`, calls `Rebuild`, and registers 82, 85 (`OnEquip`) and 837 (`OnSetItem` f13857@`3dbdb9`). It runs no recompute. It replaces any earlier `AvatarStats` of the session; `Dispose` f13860@`3dc422` detaches on uninstall.
- **Hook order.** `ProfileTitles`, `AvatarStats` and `ImprintLink` register through `GameSession::OnPacket` f20884@`65c458`, which routes through `LateHooks::Hook` f13120@`3a48c6`. `GameSession::AddHandler` f20890@`65c77b` appends to the opcode's handler list, so hooks run in registration order: the `Inventory` hook before `OnEquip`.

From the install on, every `Total` call reads `Part`. The equipment term is fixed between rebuilds, and the set and imprint terms are read live. That is the steady state Net reports. The cached `CombatStats` maxima include it only after the next recompute, because the install raises none.

The install moment cannot be observed from frames. The snapshot equals Net's slots at H only if no worn slot or row changed between H and the install. Net therefore marks the term stale when a worn slot or worn row changes after H without a rebuild. A live 82 or 85 whose leading UID the client holds clears that mark, because the snapshot then reflects that frame whether the install came before it or after it.

### Server enforcement

Whether the server enforces this maximum cannot be proven from the client build. The client never receives the server's own computation. The only server-sent maximum is an S282 total, which both the client's `Total` and Net prefer. Known ways the server could differ from the client:

- Kind 2 rows stored by ops 6 and 8 reach the client without options. The client then holds an empty list. Only a later 231 row or op 2 or 4 change carries that row's options again. The in-place writers, `PetProperty::Set` f10955 and `GuildDepot::MergeOptions` f12860, are reached only from denied opcodes.
- If the server reports login-time sets only in StartSet 837 frames, `ProfileTitles` never sees them.
- A failed install load drops a term on the client only.

Net matches the client's `Total`, which the client caches and displays at its next recompute. The one exception is the first case above: while a worn Kind 2 row lacks wire options, Net leaves the maximum unknown rather than report one the server may not share. Net's existing rules apply to a derived maximum unchanged: a received HP or MP value is clamped to it, and a composite drops it while the current value exceeds it.

### Page path

The page bridge forwards every frame that derived maxima read, so a page-bound decoder applies the same inputs as a native one:

- `page::INCOMING` carries these with their payload: the core frames (39, 109, 282, 283, 629, 634, 649, 784 and 1070), the inventory and slot frames (231, 233, the item deltas, 82, 85, 151, 518 and 521), 230 and 437 for `Me`, the 837 set frames, the 1386 and 1387 imprint frames, and the login-time 1487 and 1489, whose handlers `Parts` models.
- `page::HEADER_ONLY` carries only the header of frames whose arrival alone matters. These are the inventory inputs without a decoder grammar, which invalidate carried weight, and the 1065, 1071 and 1389 recomputes, which never read their payload.
- Every other `parts::UNMODELED` opcode also crosses as a bare header. Its arrival leaves `GameParts` unknown, as on the native path.

`page::tests::the_page_withholds_no_input_of_derived_maxima_or_carried_weight` checks all 65,536 logical opcodes against a decoder before H and after it. A dropped opcode must change no input. `Status` and `Parts` are probed with every payload of a synthetic session, and the decoder with an empty one. A header-only opcode must change the inputs and the decoder's outcome the same way whatever its payload. The observer tests feed the same synthetic session to a native observer and, through the real bridge script, to a page-bound one, and compare the maxima after every frame.

Bind order decides whether the page saw the StartSet. A page bound before the game login sees it from the first frame, and its decoder derives the native maxima. A page bound once a startup or decoder exists, at H or after it, marks `GameParts` lost (`Decoder::lose_game_parts`). StartSet frames may have reached neither that decoder nor the page. The maxima then stay unknown until the next game login starts a fresh startup. A received S282 total is still reported. Desktop binds the page before it opens the host, so the assisted app takes the first path.

## Net model

`crates/net/src/resources.rs` keeps the `CharacterStatus` inputs per decoder and applies frames in arrival order. `crates/net/src/parts.rs` keeps the `GameParts` inputs alongside.

- **StartSet boundary.** Frames through the startup cutoff H are treated as the StartSet, the same boundary the accepted inventory cutoff uses. In the StartSet only 233 and the soul-seed opcodes act on `CharacterStatus`. A StartSet 233 recomputes once the frame at H has been applied, after the whole StartSet aura. `Parts` closes its own StartSet replay at the same frame. When that frame is the 233, the replay closes before Net reports the 233's HP and MP, so the Baseline carries the maxima the StartSet proves.
- **Live frames.** After H, the block, points, buff, override and aura frames act as described above.
- **Unknown on doubt.** An input becomes unknown when a frame's effect is not provable:
  - a 109 shorter than 26 bytes;
  - a 634 shorter than 37 bytes;
  - a 784 that does not parse;
  - a 1070 shorter than 60 bytes;
  - a 1065 while the aura is non-zero, because `ParsePlanted` runs before the reset.
- **Recomputes Net cannot observe.** Imprint frames recompute only when their parser succeeds, and 82, 85 and 837 recompute only after the install. Each such frame makes unknown whatever it would have changed. A later recompute that always runs restores what it can prove.
- **`GameParts`.** `Parts` mirrors the 23 client slots (each a UID, empty, or unproven), the UIDs whose client row is unproven, loaded ammunition, `Me`, the own set multiset and imprint emptiness. It reports `Part` only when all of these hold:
  - the StartSet has closed, no denied opcode has arrived, and no soul frame has carried an item change or thrown in the StartSet;
  - Net has never had to keep more than 8192 unproven UIDs at once;
  - no worn change waits for a rebuild, and the imprint list is provably empty;
  - every slot is proven, no worn UID is unproven, every worn Kind 2 row carried its options, and every worn row's catalog entry and `StatPlus` list exist;
  - the set term is proven.

`maximum(stat)` returns the S282 total when present. Otherwise it returns `GameParts + core` with wrapping I32 arithmetic, when both are proven and the sum is positive. The S282 invalidation list and the composite rule that a new maximum keeps the older current-value time are unchanged. The readiness flags now follow derived maxima, and they refresh after every frame and when a page is bound. `GameDefinitions` carries:

- the player level table from `game_definitions.json` or from the resolved `gulim13.fnt2`;
- the catalog `StatPlus` lists from either source;
- the `SetItemParam` bonuses, from the resolved catalog only.

## Live oracle

A sibling of the Trial A probe, outside Git, appends four exported pointer globals to the Trial A instrumentation. Each captures `this` at the entry of one constructor or handler:

- `CharacterStatus::.ctor`;
- `CombatModel::OnTotalsChanged`;
- `AvatarStats::.ctor`;
- `ImprintModel::.ctor`.

Every Trial A splice and `snapshot()` stay byte-identical. `statusSnapshot()` reads fields only and calls no target method. It reads the client's own `CharacterStatus` vectors, live aura, level table and cached `CombatStats` maxima. It also computes `Part` from the client's own snapshot, item-info and set tables, `Me`, own set list and imprint accumulators. Before reading, it checks that the `GameParts` delegate targets the captured `AvatarStats` and that the class, session and install closure match.

A replay tool built from a workspace copy of this branch dumps Net's core, `GameParts`, maximum and `Parts` state at the same receive frontier. A comparator prints only PASS, FAIL or UNKNOWN for each stat's core, `GameParts` and maximum, plus YES/NO flags. The flags cover whether the cache matches, whether the install happened and could be read, whether the snapshot UIDs and own set list match, and whether imprints are zero.

Static validation passed:

- `node --check`;
- the patched bytes reproduce the generated module;
- `WebAssembly.validate` accepts the module, which was not instantiated;
- `snapshot()` output matches the Trial A probe on synthetic memory;
- synthetic reader, `Part` and guard tests;
- synthetic comparator tests.

## Risks

- **The StartSet boundary is assumed to be H.** Frames between H and bind B would be StartSet frames that the late components never see, while Net applies them as live. Trial A showed H = B for inventory only.
- **Late hooks are assumed to replay in arrival order.** The model assumes frames after bind reach `CharacterStatus`, `ProfileTitles`, `AvatarStats` and `ImprintLink` in arrival order, through late-hook replay (`LateHooks::Hook` f13120, `Arrive` f13121, `Flush` f13119). It also assumes `SoulSeedLink` and `CharacterStatus` exist before any post-H aura or status frame. Lazy construction through `StatusOf` f12549 and `SeedOf` f12509 could reorder those frames. A `ProfileTitles` created after a post-H 837 that replay does not cover would miss that set.
- **The install is assumed to complete after H with both tables loaded.** A failed load leaves the client's `Part` without the equipment or set term, while Net reports both. The oracle shows this as `readable NO`.
- **The cached maxima can lag the derived value.** The install raises no recompute, so the displayed maxima exclude `GameParts` until the next recompute.
- **Frame-free consumers are not modelled.** Net does not model `ItemHolders` consumption from skill scripts or the `TutorialSystemUi` item change. If one changes a worn row or slot, the client's next rebuild differs from Net's. Native sessions send their own actions, so these paths run only where the official client runs skills or the tutorial.
- **Denied opcodes end derived maxima for the session.** Any trade, shop, NPC, pet, guardian, quest or crafting frame, StartSet included, leaves `GameParts` unknown until a new session. A 1487 or 1489 does so only when it carries an item change, or throws in the StartSet.
- **A live 231 diverges rows.** The client ignores it, so any listed row that differs from Net's cache becomes unproven.
- **Imprint entries are not modelled.** Any non-empty imprint list leaves the term unknown.
- **The JSON definitions carry no set bonuses.** A non-empty own set list is unknown on that path. On ea1a68bd the JSON path is refused anyway.
- **The page path relies on its forwarding lists.** A new input of derived maxima reaches a page-bound decoder only once it joins `page::INCOMING`, or `page::HEADER_ONLY` when only its arrival matters. The completeness test catches a missed input only when a session payload or an empty one exercises it. A page bound after the game login keeps the maxima unknown until the next login.
- **Carried weight ignores the denied callers.** After H, the carried-weight model fails only on a late 231, a stray row or an `Inventory` hook without a grammar (`decode::UNDECODED_INVENTORY`). The trace above found 177 more opcodes whose handlers can change rows: the other 175 denied opcodes, and 1487 and 1489 when they carry an item change. A parsed 1387 applies an item change too. Any of them can change the client's carried weight while Net's stays as it was. This affects the accepted inventory cutoff, not the maxima, and needs its own fix.
- **Server enforcement is unprovable**, as described above.
- **S282 invalidation is unchanged.** It still clears on the earlier opcode list. It does not clear on imprint, aura or 649/629/634 recomputes, and it clears on some frames that never recompute in this build.
- **Malformed known frames are stricter in Net.** Net's strict 282, 283 and 784 parsers reject trailing bytes that the client ignores. For 282 and 283 that still fails the frame, as before. For 784 it makes the overrides unknown.
