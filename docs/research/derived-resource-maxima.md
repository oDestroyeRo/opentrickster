# Derived resource maxima on build ea1a68bd

Issue [#47](https://github.com/oDestroyeRo/opentrickster/issues/47). Static research completed **2026-11-09 UTC** for build **ea1a68bd89958fae94179f1e0835ffb5** only. The question: can Net derive maximum HP, MP and weight (stats 9, 3 and 6) without S282, exactly as `CharacterStatus.Total` does?

**No, not completely.** Every packet-fed term of `Total` is now modelled exactly in `crates/net`, but the `GameParts` term is not provable from packets. The client attaches `GameParts` only after the `AvatarStatsUi` install completes. That install is asynchronous: it waits for two table loads, and no frame marks when it finishes. The term also adds an equipment snapshot taken at install or at an equip frame, the set bonuses of `ProfileTitles`, and the imprint bonus of `ImprintModel`. Each of those needs its own proof. Net therefore keeps all three maxima unknown unless a received S282 total supplies one. A received S282 total still wins.

A live oracle, kept outside Git, checks the modelled terms against the client's own memory. It also reports whether `GameParts` was installed when the snapshot was taken. It was not run for this report.

## Evidence boundary

All citations are to build ea1a68bd, by method name and WASM function index (`f<index>@<body offset>`). Methods were joined by canonical name with `tools/build-migrate`; no function index from an older build was reused. The decoded WASM is `8780db4414df5357d9595ef233579227a1303753d00ac6318d225036dd1bb277`, the same asset as the [native migration](build-ea1a68bd-migration.md).

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
| `GameParts` | Set only by `AvatarStats` ctor f13847@`3dad29` to `Part` f13849@`3db5d2`: `Equipment` f13850 + `SetItems` f13851 + `ImprintList::StatBonus` f24876. The ctor runs from `AvatarStatsUi` `<Install>d__0::MoveNext` f40539@`cd06b6` after it awaits the `ItemData.ItemInfos` table and a second table; `PlayFeatures::.cctor` f37436 registers `Install`. The ctor calls `Rebuild` f13848 but no recompute. `Equipment` sums the 23-slot snapshot built by `Rebuild`. Each worn item contributes its option property `10000 + stat` through `ItemTip::PropertyEntries` f25255, or else the catalog `StatPlus[stat]`; an unknown item is skipped. `SetItems` sums set bonuses over `ProfileTitles::Sets` f21635. `OnEquip` f13856 (82, 85) rebuilds and runs `Recompute(false)` when the UID is in the inventory. `OnSetItem` f13857 (837) can run `Recompute(false)`. `Dispose` f13860 detaches on uninstall. | None that proves it | **Unknown.** Net keeps every derived maximum unknown. |

The handlers for 631, 636 and 651 (`b__23_10`, `_12` and `_14`) only emit a `Reset` event, so they are not inputs. 826 `b__23_8` writes `Properties`, which `Total` does not read. 245 and 285 (`b__23_17`) write only TM level and TM experience.

## When each input changes

`Recompute(true)` runs on a StartSet 233, on every applied stat block (39, 629, 634 and 649), on 109, and on every `ImprintEvent` through `b__23_1` f12570. `ImprintLink` f13005 hooks 1386 (`Listed`), 1387 (`Used`) and 1389 (`Ended`), from RVA `218D32A0…`. The `Recompute(false)` body runs on 283, on `AuraStarted` (1070) and `AuraEnded` (1071), and through `AvatarStats` on 82, 85 and 837 once installed. Every recompute clears `Totals` and raises `TotalsChanged`.

Startup has its own order. `GameSession` ctor f20878@`65bce5` records the StartSet only during `BindGameServer` f20880. `CharacterStatus` ctor order:

1. `Soul::SeedOf` f12509 creates the `SoulSeedLink`, which replays the StartSet.
2. `ApplyStart` runs for each StartSet frame.
3. The ctor subscribes to aura and imprint events.
4. The ctor registers its frame hooks through the session's hook slot.

So StartSet aura frames reach the live aura before the 233 recompute, and they trigger no recompute of their own. StartSet 39, 109, 283, 784, 629, 634 and 649 never reach `CharacterStatus`.

## Net model

`crates/net/src/resources.rs` keeps these inputs per decoder and applies frames in arrival order.

- **StartSet boundary.** Frames through the startup cutoff H are treated as the StartSet, the same boundary the accepted inventory cutoff uses. In the StartSet only 233 and the soul-seed opcodes act. A StartSet 233 recomputes once the frame at H has been applied, after the whole StartSet aura.
- **Live frames.** After H, the block, points, buff, override and aura frames act as described above.
- **Unknown on doubt.** An input becomes unknown when a frame's effect is not provable:
  - a 109 shorter than 26 bytes;
  - a 634 shorter than 37 bytes;
  - a 784 that does not parse;
  - a 1070 shorter than 60 bytes;
  - a 1065 while the aura is non-zero, because `ParsePlanted` runs before the reset.
- **Recomputes Net cannot observe.** 82, 85 and 837 recompute only after the UI install, and imprint frames recompute only when their parser succeeds. Each such frame makes unknown whatever it would have changed. A later recompute that always runs restores what it can prove.

`maximum(stat)` returns the S282 total when present. Otherwise it would return `GameParts + core`, but `GameParts` is always unknown, so the maximum is unknown. The S282 invalidation list, the composite rule that a new maximum keeps the older current-value time, and the readiness flags are unchanged. `GameDefinitions` now carries the player level table from `game_definitions.json` or from the resolved `gulim13.fnt2`.

## What remains for #47

To report a derived maximum, Net needs a proof for each of these:

1. **Install state.** Whether `GameParts` is attached at the last `TotalsChanged`. The client reads it on every `Total` call but caches maxima only at a recompute, so the cached maxima can lag the install.
2. **Equipment.**
   - The `Rebuild` snapshot timing and the option-to-property mapping in `PropertyEntries` f25255. That covers the Kind 2 filter, the low-16-bit property and signed high-16-bit value split, the 32-entry cap, and its predicate `b__1` f19718.
   - The `StatPlus` catalog column, which is not loaded today.
3. **Sets.** The set-bonus table that `Install` loads, and the `ProfileTitles` inputs: 837 `OnSetItem`, its StartSet replay and `GameSession.Me`.
4. **Imprints.** The `ImprintModel` and `ImprintList` inputs.

## Live oracle

A sibling of the Trial A probe, outside Git, appends two exported pointer globals to the Trial A instrumentation:

- `this` at entry of `CharacterStatus::.ctor`;
- `this` at entry of `CombatModel::OnTotalsChanged`.

Every Trial A splice and `snapshot()` stay byte-identical. A new `statusSnapshot()` reads the client's own `CharacterStatus` vectors, live aura, level table, `GameParts` presence and cached `CombatStats` maxima. It computes the client's core and its `Total` where no delegate call is needed, and calls no target method. A replay tool built from a workspace copy of this branch dumps Net's core and reported maxima at the same receive frontier. A comparator prints only PASS, FAIL or UNKNOWN per stat, plus YES/NO flags.

Static validation passed:

- `node --check`;
- the patched bytes reproduce the generated module;
- `WebAssembly.validate` accepts the module, which was not instantiated;
- `snapshot()` output matches the Trial A probe on synthetic memory;
- synthetic reader and guard tests;
- synthetic comparator tests.

## Risks

- **The StartSet boundary is assumed to be H.** Frames between H and bind B would be StartSet frames that `CharacterStatus` hooks never see, while Net applies them. Trial A showed H = B for inventory only.
- **Late hooks are assumed to replay in arrival order.** The model assumes frames after bind reach `CharacterStatus` in arrival order, through late-hook replay (`LateHooks::Hook` f13120, `Arrive` f13121, `Flush` f13119). It also assumes `SoulSeedLink` and `CharacterStatus` exist before any post-H aura or status frame. Lazy construction through `StatusOf` f12549 and `SeedOf` f12509 could reorder those frames.
- **S282 invalidation is unchanged.** It still clears on the earlier opcode list. It does not clear on imprint, aura or 649/629/634 recomputes, and it clears on some frames that never recompute in this build.
- **Malformed known frames are stricter in Net.** Net's strict 282, 283 and 784 parsers reject trailing bytes that the client ignores. For 282 and 283 that still fails the frame, as before. For 784 it makes the overrides unknown.
