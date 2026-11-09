# Client-assisted sampler heap reads

Build **`ea1a68bd89958fae94179f1e0835ffb5`** (Unity 6000.6.5f1, metadata format 108), 2026-11-09. This is static research on the public Build assets pinned in the [sampler migration](build-ea1a68bd-sampler-migration.md). It performed no login, game socket, gameplay, account access or target-code execution.

**Result.** The migration recorded five sampler reads that do not match the build. This change fixes those five and a sixth, earlier read that masked them. The sampler decoded runtime type handles as type-definition indices, so every live sample stopped with `shape` at the first type check. Each read now uses the field that metadata and compiled code define. The invented heap is built from the pinned layouts, and the test fails any read that does not land on a pinned field of the matching type. **No live sample has reached `ready`.**

## Fixed reads

Function numbers are ea1a68bd's. One-time scratch scripts disassembled the bodies statically and are not retained.

| Read | Old read | Field and type in ea1a68bd | Evidence | Fix |
| --- | --- | --- | --- | --- |
| Type handles | `Il2CppType.data` as a type-definition index in [0, 15446] | For class and value-type kinds, `data` points at the type's record in the loaded metadata. The index is (`data` − (`s_GlobalMetadata` + `typeDefinitionsOffset`)) / (`typeDefinitionsSize` / `typeDefinitionsCount`). | f2486 computes it from cells 4659832 and 4659828 and header +236, +240 and +244. | Decode the handle at the 66-byte record stride. Refuse an off-stride handle (`type`). Require the header's sanity value `0xFAB11BAF` and the pinned triple [7487400, 1019502, 15447] (`build`). |
| Service key | `RuntimeType +12` as the `Il2CppType*` | +12 is `type_info`. `Type._impl : RuntimeTypeHandle` is at +8, and its `value : iptr` is the `Il2CppType*`. | Metadata. | Read +8. |
| Inventory and actors | `Inventory +12` and `GameplayScreen +244` as ``Dictionary`2``, with RGCTX +52 as their `TryGetValue` | `Inventory.byUid` is ``OrderedMap`2<u8,InvItem>`` and `GameplayScreen.actors` is ``OrderedMap`2<i4,ActorView>``. ``OrderedMap`2`` holds `index` +8, `slots : KeyValuePair<TKey,TValue>[]` +12, `gone : bool[]` +16, `used : i4` +20, `gaps : i4` +24 and `version : i4` +28. RGCTX +52 is the inner ``Dictionary`2<TKey,i4>::TryGetValue``. | Metadata layout. The enumerator's `MoveNext` f64946 yields `slots[i]` for each `i` below `used` whose `gone[i]` is false. `Get` f69482 and `Remove` f69478 use the same fields. | `orderedMap()` walks the rows as f64946 does. It checks the map's class arguments, its RGCTX +12 (pair class) and +16 (`get_Value`), the pair's key and value offsets, and that the tombstone count equals `gaps`. |
| Ground items | `screen +252` as `GroundItems`, and `ground +68` as `GroundItems/Entry` | `ground : GroundItems` is at +256; +252 is a ``Dictionary`2``. `hover : HoverDraw` is at +68, and `HoverDraw.Entry` at +12. `entries : Dictionary<i4,Entry>` is at +36, and `Entry.View : ActorView` at +12. | `HoverDraw` is nested-private. Only the `GroundItems` constructor f48926 uses `hover`, to store a new `HoverDraw`. The `HoverDraw` methods f2806 and f49041 store only `owner`. No code writes `Entry`. `GroundItems.Click` f49009 finds a clicked item through `entries` by actor id. | Read +256 and `hover.Entry`. Because `Entry` stays null, also hit-test each `entries` row's `View` actor against the mapped pointer, with the hit box used for other actors. A row key that differs from its actor id fails with `generic`. |
| Editor focus | `EditBox` static +0 must be zero | Static +0 is `SelectedText : TextStyle`. `Focused : EditBox` is static +24. | `Hud.OnKey` f48516 does not route a key while static +24 holds a live editor. | Static +24 must be null (`routing`). |
| Hotkey install | `Hud +128` as a boolean | +128 is `Waiting : System.Action`. The state is `installing : bool` at +132. | `Hud.OnKey` f48516 passes the key to `WaitForKey` f48518 while +132 is true. `Hud.PressHotkey` f48491 invokes `Waiting` only while +132 is true. | +132 must be false (`routing`). [Deferred features](#deferred-features-after-world-entry) later replaced this check with a pending-key check. |

The audit also changed these reads:

- **Extents.** `object()` took hand-written sizes. Five were smaller than the metadata instance size: `GameSession` 140 (144), `CombatModel` 276 (280), `Inventory` 44 (48), `ItemInfo` 260 (264) and `String` 12 (16). `dictionary()` checked 40 of 48 bytes. An object cut off by the end of memory could pass. Sizes now come from one `typeSizes` table that the test compares with the pins, and `dictionary()` checks 48 bytes.
- **Fading.** `CombatModel.fading` (+136) is an ``OrderedMap`2<i4,Aging>``. The sampler read its `index` dictionary through RGCTX +84 (`ContainsKey`). That read was correct. It now uses `orderedMap()` like the other two maps.
- **Bound.** `type()` takes its bound from the pinned type-definition count, not the literal 15446.

Every earlier refusal remains, and the #38 continuation window is unchanged. Two checks now test different fields. A delegate in `Hud.Waiting` or a non-zero `EditBox.SelectedText` no longer refuses the sample, because neither field holds routing state.

## Read checker

`tools/build-migrate/sampler/derive.py` now pins each field that the sampler reads as `[offset, type]`. It also pins enum storage types, base classes, the layouts of the generic instances that the sampler reads, and the metadata globals. Before it lays out a generic instance, it must reproduce the metadata offsets of every pinned non-generic type.

`crates/net/tests/sampler.cjs` builds each invented object from those pins. A recording `DataView` maps every sampler read to a pinned field, or to a libil2cpp structure in the test's `RUNTIME` table. A read that hits no field, or reads a field with the wrong width or type, fails the suite and names the field. The suite also fails in these cases:

- a pinned field, cell, RGCTX slot or runtime field is never read;
- the sizes, kinds, cells, type-definition triple or build in `sampler.js` differ from the pins;
- the pins name a Unity version other than 6000.6.5f1, because the `RUNTIME` offsets belong to that version.

After the fixes, every read lands on a pinned field of its type. Building from the pins also corrected three fixture errors that the sampler did not depend on: the field order of ``Nullable`1`` values, the ``InputControl`1<r4>`` field offsets (+176 and +184) and the `AutoPets` value type (`i4`, not object).

| libil2cpp structure | Fields read | Evidence |
| --- | --- | --- |
| `Il2CppGlobalMetadataHeader` | sanity +0; type-definition offset, size and count +236, +240, +244 | f2486 |
| `Il2CppType` | `data` +0, `type` +6 (byte) | f2486 switches on +6 |
| `Il2CppClass` | `byval_arg` +16, `element_class` +32, `interfaceOffsets` +72, `static_fields` +76, `instance_size` +128, `stack_slot_size` +132, `element_size` +140, `interface_offsets_count` +182, vtable +192 | `Object::New` f2294 (+128); `Array::New` f2291 (+140); interface dispatch in f3011 (+76, +72 rows of 12 bytes, +182, vtable entries of 8 bytes); f64946 and f69482 copy +132 bytes |
| `MethodInfo` | `klass` +16, `rgctx_data` +28 | lazy RGCTX setup f2456 |
| `Il2CppArray` | `klass` +0, `bounds` +8, `max_length` +12, data +16 | `Array::New` f2291 |
| `Il2CppGenericClass`, `Il2CppGenericInst` | `type` +0, `class_inst` +4; `type_argc` +0, `type_argv` +4 | f2486 follows `type` +0. `derive.py` decodes the build's own generic records with these offsets. |

An RGCTX slot for a field of a value type holds the field's offset in the unboxed value: f69482 adds the pair's RGCTX +16 to the start of the array element.

## Verification

Each fixed read has a case that fails on the old read:

| Fixed read | Cases that fail on the old read, with the read checker off |
| --- | --- |
| Type handles | the base sample and every fixed-read case (`shape`) |
| Service key | the base sample (`memory`) and "service key identity comes from Type._impl" |
| Inventory and actors | the base sample, "tombstoned actor row skipped" and "tombstoned inventory row" (`type`) |
| Ground items | the base sample and the ground-item cases |
| Editor focus | "focused EditBox owns the keys" and "EditBox static ahead of Focused is not focus" |
| Hotkey install | "Hud.Waiting callback is not routing state" |
| Extents | "truncated dictionary"; a `GameSession` size of 140 fails the `typeSizes` comparison |

Restoring any one old read in a scratch copy of `sampler.js` fails the full suite. The read checker also catches three mutations: a 32-bit integer read of `DrillFeature.Gauge` (`r4`), the `GameplayScreen.ready` read at +452 (`cameraX`) instead of +400, and the ``InputControl`1`` cached-value slot at RGCTX +16 instead of +28.

```sh
D=tools/build-migrate/sampler/derive.py
python3 -I $D --build 8502ebbf2fc9033d2fba50d02fd92002 OLD_WASM OLD_DATA --out OLD.json   # exit 0
python3 -I $D --build ea1a68bd89958fae94179f1e0835ffb5 NEW_WASM NEW_DATA \
  --expect OLD.json --out NEW.json   # exit 1: 111 MOVED, 2 CHANGED; NEW.json equals ea1a68bd.json
node crates/net/tests/sampler.cjs   # PASS sampler 120 invented-heap cases
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The two changed pins are the migration's `probedAt` move and `MapPoint` fingerprint. The 111 moved pins are 81 type definitions, 25 cells, the Heavy slot, the type-definition bound and the 3 metadata globals.

## Deferred features after world entry

Same build, 2026-11-10. This is static research on the same public assets. It performed no login, game socket, gameplay, account access or target-code execution.

**Result.** After world entry, every live sample refused `cold`. The sampler required six services, and the service table lacked `DrillFeature` (6259) and `DigAssist` (6218) for 5 to 20 seconds. The client installs both in a deferred feature that starts after the game shows. The sampler now reports `dig: null` while both are absent, and the sample can be `ready`. The engine's dig capability stays off until a sample reads the drill feature. The `Hud.installing` check would have refused the same window, so it now refuses only while a deferred key is pending.

### Drill lifecycle

| Question | Answer | Evidence |
| --- | --- | --- |
| Who calls `DrillFeature.CreateAsync`, and when | Only `DrillUi/<Install>d__3::MoveNext` f44759, after it awaits `ItemIcons.Infos` and `Wait<CombatModel>`. | `CreateAsync` f20236 has one caller, at `0e9b627`. Its state machine f20352 awaits `DrillData.LoadAsync` f20209, which loads named tables through `GameAssets.Table` f30675, and then calls the constructor f20235. |
| How and when both services are registered | f44759 provides `DrillFeature` at `0e9b7a6` and awaits `Wait<ItemsModel>`. It then constructs `DigAssist` (f20137, `0e9bec8`) and provides it at `0e9bf9c`. With no await in between, it registers the `Dig` hotkey (`Hud.RegisterHotkey` at `0e9c1e4`), the drill world click (`0e9c23d`), a pick mode and `AvatarDrivers.Add(DigAssist)` f13820. Its cleanup lambda f44752 removes both services. | f44759 awaits only at `0e9b297`, `0e9b64b` and `0e9bbc3`. No other function provides either service. `DrillUi.Install` f44731 appears only in `PlayFeatures.Deferred`, entry 14 ("drill"), which `PlayFeatures..cctor` f37436 builds. |
| When deferred features run | `GameplayScreen/<Enter>d__283` f48925 installs `PlayFeatures.Core` through `InstallFeatures` f38485 (`1035b6a`) before `EnterWorld` (`1035c3e`). It starts `InstallLate` f38490 at `1036144`, after it logs that the game shows. `<InstallLate>d__272` f48948 sets `Hud.installing`, starts every deferred entry at once, and waits for all of them or 60 seconds. Then it clears `installing`. Each late install (f48944) first awaits `LoadPacing.Pause()` or `GameplayScreen.lateRush`. | `FrameBudget.Pause` f36085 yields one frame only when the frame's time or allocation budget is spent. The code has no fixed bound, so table loading and frame pacing set the delay (inferred). |
| Can anything else hold dig state while both are absent | No dig request or dig packet handler exists without `DrillFeature`. The shared exception is the avatar state machine. | The `Spinel.Net.Game.Drill` payload builders f11465, f11466, f11468 and f11472 have callers only in `DrillFeature` (`Press` f20165, `Cancel` f20160, `EndDig` f20173, `Withdraw` f20176, `AvatarUpdate` f20272 and `AutoHost.RequestSpot` f20313). Every drill parser is called only from `DrillFeature/<>c__DisplayClass122_0` constructor lambdas. Only `DrillFeature.OnAccepted` f20265 and `OnAutoSpotTaken` f20254 call `AvatarFsm.Set` f13839 with 5, but five other callers pass a variable state. |
| What user action must happen first | None. Deferred installs start on their own when the game shows. | A key can only speed them up. `WaitForKey` f48518 invokes `Hud.Waiting`, which is `RushLate` f38491, and `RushLate` completes `lateRush`. |
| Does the client's dig key create them on demand | No. The `Dig` hotkey exists only after `DigAssist` is provided. | `HotkeyMap.MakeCatalog` f24832 builds `Dig` with `HotkeyAction.Spinel` (+21) false (inferred from argument order), so `WaitForKey` does not skip it. During installs, a dig key waits in `waitAction` and rushes the deferred features. `RegisterHotkey` f48490 moves a matching wait to `due`, and `Hud.Update` f48500 replays it once. After installs end, `set_Installing(false)` f48497 clears the wait, so an early dig key does nothing. |

### Other required services and `cold` reads

| Service | Provider | List | Absent |
| --- | --- | --- | --- |
| `CombatModel` 6104 | `CombatFeature/CombatUi::.ctor` f39216 | Core | `cold` |
| `GunFlowing` 6632 | `GunFlowingUi/<Install>d__17::MoveNext` f46675, at `0f4d106`. It first awaits `ItemIcons.Infos`, `GunData.LoadAsync`, `Wait<CombatModel>` and `Wait<ItemsModel>` (`0f4cdc3`). | Core, entry 15 | `cold`. The live table gained it a few seconds after entry. |
| `SkillsApi` 8027 | `SkillsFeature/<Install>d__3::MoveNext` f44386, at `0e7487e` | Core | `cold` |
| `AvatarStatusFlags` 5813 | `SkillWindow.Install` f44224, which f44386 calls at `0e751c6` with no await after the `SkillsApi` provide | Core | `cold` |

The other `cold` reads need no user step either. `Inventory..ctor` f13069 allocates the 23 slots and fills them from received packets, so fewer than 23 slots means a damaged read. Equipped item rows come from the same packets, and their item infos come from the table that f46675 loads before it constructs `GunFlowing` (inferred from the only constructor call, at `0f4d039`). `UiSkins.Start` f50227 sets `UiSkins.started`, and `UiSkins.get_Current` calls it on first use. Two `cold` reads depend on the drill feature: the worn drill's parameter row and an active dig without its parameter or tool. Both now run only when the drill feature exists.

### Semantics

- **Sample.** With both services absent, `dig` is `null` and the sample can be `ready`. `DrillFeature` without `DigAssist` refuses `cold`, because the install is one await on the Core `ItemsModel` away from done. `DigAssist` without `DrillFeature` refuses `changing`, because the client never builds that state. The avatar state comes from `CombatModel.Fsm` (+112). When the drill feature exists, `DrillFeature.fsm` must be the same object.
- **Net.** `Snapshot.dig` is `Option<DigState>`, and the parser requires the field to be present. `idle` treats an absent drill feature as no dig work, but still requires avatar state 0 or 1. Planning `DigAt`, the gauge decision and dig cleanup need dig state.
- **Engine.** An assisted Baseline reports `dig_cycle` false. The observer emits `Event::DigCycle(true)` on the first accepted `ready` sample with dig state, and `DigCycle(false)` on one without. An unavailable sample changes nothing. The reducer applies the event only while the session is online. A closing embedded adapter forces it false.
- **Routing.** `Hud.installing` (+132) no longer refuses. A pending deferred key refuses `routing`: `waitMenu` (+136), `waitAction` (+140) or `due` (+148).

The routing change rests on these reads:

- `Hud.OnKey` f48516 runs registered hotkeys first. It reads +132 once, at `1007fe4`, and only then calls `WaitForKey`.
- `WaitForKey` f48518 skips a binding whose `HotkeyMap` entry has byte +21 set, or whose name already has hotkeys or menu buttons (`TryGetValue` at `10085f3` and `1008644`, count above 0).
- `OnKeyUp` f48520, `PressHotkey` f48491, `PressMenuButton` f48495, `RunMenuButton` f48408 and the `RegisterMenuButton` lambda f48542 are the only other readers of +132. Each defers only a hotkey or menu button that is not registered yet.
- `set_Installing` f48497 clears +136 and +140 when it is given false. `RegisterHotkey` f48490 and `RegisterMenuButton` f48494 move a matching wait to `due`. `Hud.Update` f48500 clears `due` and invokes it.
- No mouse path reads +132. The page sends the dig key only for `DigAt`, which needs dig state, so the `Dig` hotkey is registered by then.

`derive.py` now pins the three wait fields and fingerprints every `Hud` method above, so a build that changes this routing fails the migration check.

### Deferred-feature verification

These cases fail on a scratch copy of the previous `sampler.js`, run with the previous pins plus the three wait fields:

| Behavior | Cases |
| --- | --- |
| Absent drill feature | "deferred drill install has not run", "absent drill still reports the shared avatar state", the late-arrival sequence (`cold` before) |
| Helper without its drill | "helper without its drill" (`cold` before) |
| Installing alone | "installing HUD alone keeps registered keys" (`routing` before) |
| Pending deferred key | "deferred key waits for its action", "deferred menu key waits for its button", "deferred key queued for replay" (`ready` before) |

"drill provided before its helper", the four core-service cases and "drill and combat disagree on the avatar state" keep the previous result. The Rust tests `absent_drill_feature_is_no_dig_work_and_never_plans_a_dig`, `assisted_dig_cycle_waits_for_a_sample_that_reads_the_client_dig_feature` and `dig_cycle_follows_a_client_dig_feature_that_arrives_after_baseline` cover parsing, planning, the observer and the engine.

```sh
python3 -I tools/build-migrate/sampler/derive.py --build ea1a68bd89958fae94179f1e0835ffb5 WASM DATA \
  --expect tools/build-migrate/sampler/ea1a68bd.json --out NEW.json   # exit 1: 0 MOVED, 15 CHANGED
node crates/net/tests/sampler.cjs   # PASS sampler 133 invented-heap cases
```

The 15 changed pins are the three wait fields, the removed `installing` field and 11 `Hud` fingerprints.

## Limits

- No sample has reached `ready` against a live client. The evidence is static or synthetic.
- The `RUNTIME` offsets come from this build's compiled libil2cpp, not from Unity source.
- The ground-item check is a replacement. It assumes that item actors carry the same hit boxes and hidden flags as the game's own world pick. It still reads `HoverDraw.Entry` in case a later build sets it.
- The game tests `EditBox.Focused` with Unity's object inequality, which also treats a destroyed editor as null. The sampler tests for any reference, so it can refuse where the game would route keys to the HUD.
- Nobody has timed the deferred drill install. The live table lacked `DrillFeature` for at least 20 seconds.
- A later build could add another source of dig state. A migration must recheck the provider and caller lists in [Drill lifecycle](#drill-lifecycle), because no pin covers them.
- `Hud.OnKey` also drops keys while a message box is open (`MessageBox.IsOpen`) or when an editor took a key in the current frame (`EditBox` static +28, `KeyTakenFrame`). The sampler checks neither, before or after this change.
