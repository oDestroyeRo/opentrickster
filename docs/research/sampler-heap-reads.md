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

## Live sample refusals

Same build, 2026-11-10. This is static research on the same public assets. It performed no login, game socket, gameplay, account access or target-code execution. The refusals were observed live on an idle in-world character with the drill feature and a worn gun.

**Result.** Every live in-world sample was unavailable. Six reads assumed a client state that the live client does not have. Each read now follows the client code that owns the state. An audit of the other reads found a seventh refusal that could never clear, and fixed it. **No live sample has reached `ready`.**

### Six live refusals

| Refusal | Old read | Live client state | Evidence | Fix |
| --- | --- | --- | --- | --- |
| `routing` | `SkillsApi.combatPlaysEsa` (+124) set | A setting that records whether `CombatModel` existed when `SkillsApi` was built | `SkillsApi..ctor` f17848 is the only writer. `OnEvent` f17889 reads it only to skip the API's own ESA playback. | Drop the read and its pin. |
| `routing` | `Hud.keyMask` (+68) not 0 | `0x7FFFF` from `Hud..ctor` f48528, then masks from `GameUiMode.Masks` f21418 through `set_UiMode` f48477 | `Hud.KeyPasses` f48492 tests the mask through `GameUiMode.KeyOn` f21422. `KeyOn` passes every name that `KeyBit` f21420 gives no bit, and `Dig` and `Run` have none. | Drop the read and its pin. |
| `type` | `Keyboard.current` of class `Keyboard` 10299 | A `FastKeyboard` 10304 | `Keyboard.MakeCurrent` f34170 stores any `Keyboard`. `FastKeyboard` is the only subclass. It declares no instance fields and keeps the 320-byte instance size. | Accept exactly 10299 and 10304. |
| `input` | ``InputControl`1.m_CachedValue`` only | Read-value caching off (`InputManager` +736 false) | ``InputControl`1.get_value`` f69067 then computes the value from device state on every call. | Read the device state as the client does. See [Uncached buttons](#uncached-buttons). |
| `type` | `GunFlowing.infos` (+28) as ``Dictionary`2<i8,ItemInfo>`` | An `ItemMap` 6764 | `ItemMap.TryGetValue` f24927 finds the row index in `slots`, a ``Dictionary`2<i8,i4>``, and returns `rows[index]`. `ItemMap.Add` f24922 is the only writer. | Look up only the worn ids. See [Item lookup](#item-lookup). |
| `memory` | The `DigKeySettings` TypeInfo cell 0x318318 as a class pointer | The cell still holds its metadata-usage word, 536902459 (kind 1, index 15773) | f2690 replaces the word with the class only when a method that uses the cell first runs. `DigKeySettings.get_SemiAuto` f44679 had not run. | Pin each cell's usage word. See [Unused metadata cells](#unused-metadata-cells). |

`derive.py` fingerprints 21 of the client methods that these fixes rest on: `Hud.KeyPasses`, the `GameUiMode` key methods, six `SkillsApi` methods, the input-state readers, the `ItemMap` methods and `Int64.GetHashCode`. A build that changes one of them fails the migration check.

### Uncached buttons

With caching off, `get_value` f69067 computes each button from device state:

- `get_unprocessedValue` f69068 calls virtual slot 17 on `currentStatePtr` f33851. That pointer is the front buffer that `InputStateBuffers.GetFrontBufferForDevice` f34662 maps from `m_Device.m_DeviceIndex` (`GetDeviceIndex` f33849).
- `KeyControl` and `ButtonControl` inherit the `AxisControl` reader f34935. It has `BYTE` and `FLT` fast paths. Otherwise it calls `InputStateBlock.ReadFloat` f34653 and an inlined `Preprocess`. Keyboard keys are 1-bit `BIT` fields, read by `MemoryHelpers.ReadSingleBit` f35104.
- `DiscreteButtonControl` f34957, the class of the keyboard's shift, ctrl and alt controls, tests `MemoryHelpers.ReadMultipleBitsAsUInt` f34958 against `minValue` and `maxValue`.
- `ProcessValue` f69069 runs `m_ProcessorStack` only when the stack is not empty.

The sampler reads the same bytes and composes wider values from single bytes, because device state can be unaligned. It refuses `input` for a control with processors or preprocessing, a wrapping discrete range, another state format or device, or a device without a front buffer. It keeps the cached path for when caching is on.

### Item lookup

The sampler walks `ItemMap.slots` as ``Dictionary`2.FindEntry`` f64786 does with a null comparer. `Int64.GetHashCode` f71422 gives the hash, and the hash picks a 1-based bucket head. The walk follows `Entry.next` (RGCTX +160) until the id matches. It reads only the probed entries, so a 100,001-item map stays inside the 200,000-read budget that a full scan would exceed. It refuses `generic` when `slots` has a comparer, a chain runs past the entry count, an index falls outside `rows[0, count)`, or a row holds another id. The RGCTX comes from the ``ContainsKey<i8,i4>`` cell, which the prologue of `ItemMap.Add` resolves before the first row exists.

An id that the map lacks has no info. The map is complete before `GunFlowing` exists, because `GunFlowingUi/<Install>d__17` f46675 awaits `ItemIcons.Infos` first. `GunFlowing.HoldsGun` f24383 skips an item without info, and `DrillFeature.get_WornDrill` f20238 returns no drill. The sampler now reports `gunWorn` and `toolUsable` as false in that case. Before, it refused `cold`, and that refusal never cleared.

### Unused metadata cells

A metadata-usage cell holds `(kind << 29) | (index << 1) | 1` until f2690 resolves it. f2690 runs in the prologue of a method that uses the cell, the first time that method runs. `derive.py` now pins each cell's word as `cellWords`. The sampler reads the pinned word as "not used yet". Any other odd or zero word is a wrong pin and refuses `build`.

A class whose cell is unused, or whose `static_fields` is null, still has its default statics:

| Cell | Default statics | Sample |
| --- | --- | --- |
| `EditBox` | No focused editor | Continues |
| `HudCover` | No covering window | Continues |
| `UiPressOrigin` | No press. `get_LeftOnUi` f50216 and `get_RightOnUi` f50217 return false while `Current` is null. | Continues |
| `DigKeySettings` and its two ``Nullable`1<bool>`` accessors | No cached setting. `get_SemiAuto` f44679 caches PlayerPrefs `spinel.custom.digKeySemiAuto` on its first call. | Continues with `dig.manual` null |
| `EventSystem` | No event system | `routing`, as for an empty list |
| `GameFlow`, `SpinelEngine`, `Keyboard` and the input-system statics | Not loaded | `cold`, as for a null `GameFlow.Current`, `SpinelEngine.Instance` or `Keyboard.current` |
| Each MethodRef cell, and a null RGCTX | Method not run yet | `cold` |

A null `dig.manual` refuses only dig actions, because `sampler.rs` parses it as `None`. The sampler reads `UiSkins` and `DigKeySettings` only when the drill feature exists without an automatic pet, and `DigKeySettings` only for the Custom skin. The snapshot revision follows `manual`, not the raw skin and setting. Live, the `GameFlow` cell held its usage word (536905459, index 17273) for a moment during loading. A `cold` refusal there is correct.

### Routing fields kept

The routing check keeps four `SkillsApi` fields:

| Field | Why the sampler waits |
| --- | --- |
| `target` (+52) | An active skill target pick. `WorldClick` f17899 gives it the next world click. `OnEvent` f17889 clears it on event 243. |
| `shownTarget` (+56) | `Update` f17908 sets it to `target` each frame, so the two differ for one frame at most. |
| `UiWait` (+125) | A scene wait that the client's own auto-combat respects. `SetUiWait` f17863 is the only writer. Quest UIs f39697, f41050, f46817 and f47039 pass a scene event's flag. The ESA host methods f45405, f45406 and f45407 pass false. |
| `heldCast` (+188) | An own cast that waits to play. `OnEvent` f17889 sets it and queues `PlayHeldCast` f17894, which clears it. |

### Audit of the other reads

The audit asked three questions of every other read. Does a read of an interface, abstract or unsealed type accept exactly the classes that the client stores? Can a refusal meant to be transient hold while the character is idle? Does a read need a feature, setting or map property that the client may lack?

| Read | Type or state | Client facts | Verdict |
| --- | --- | --- | --- |
| `Keyboard.current` | `Keyboard`, unsealed | `FastKeyboard` 10304 | Fixed above |
| `GunFlowing.infos` | ``IReadOnlyDictionary`2<i8,ItemInfo>`` | `ItemMap` 6764, the only implementation | Fixed above |
| Worn item infos | `ItemMap` lookup | A missing id means no info | Fixed above |
| Keyboard controls | `KeyControl`, `ButtonControl` | `KeyControl` has no subclasses. `button()` accepts exactly `AnyKeyControl` 10580, `ButtonControl` 10583, `DiscreteButtonControl` 10586 and `KeyControl` 10591. The other subclass, `TouchPressControl`, is not a keyboard control. | Already correct |
| `GameFlow` screen (+84) | `GameFlow.Screen`, abstract | Eight sealed screens. Only `GameplayScreen` 2669 has the world. | Already correct. Another screen refuses `type`. `sampler.rs` maps every reason except `build` to `NetError::Unsupported`, so another reason would change nothing. |
| `EventSystem.m_EventSystems` entry | `EventSystem`, unsealed | The client creates only `EventSystem` (`GameInput.CreateEventSystem` f36204). `MultiplayerEventSystem` 10405 appears only in `InputSystemUIInputModule` f33316. | Live check. A scene could place a `MultiplayerEventSystem`, which refuses `type`. |
| Collections and input state | ``Dictionary`2``, ``List`1``, ``OrderedMap`2``, `InputActionState`, `InputManager` | The build defines no subclass that these fields can hold | Already correct |
| `InputControl.m_Device`, and `session` in `CombatModel`, `GunFlowing` and `DrillFeature` | Interface or base class | Compared by identity with an object that the sampler already checked | Already correct |
| `UiPressOrigin` press flags (+32, +33) | Transient | `UiPressOrigin.Update` f50219 clears both every frame and sets them only in a press frame. | Already correct |
| `HudCover` covers | Open windows | `KitWindow.KeepCover` f62791 adds a cover. `OnClosed` f62887 and `OnDestroy` f62888 remove it. The game ignores world input while covered (`get_Covered` f48502 in `GameplayScreen.Advance` f38568 and `OnWorld` f38576). | Correct by design. An open window refuses `routing`. |
| `EventSystem.m_CurrentSelected` (+36) | Selected UI object | `Selectable.OnPointerDown` f29855 selects. `InputSystemUIInputModule.ProcessPointerButton` f33310 clears it only on a later click elsewhere. | Live check. A UI click can leave an object selected, and every sample then refuses `routing`. |
| `EventSystem.m_HasFocus` (+40) | Application focus | Persists by design | Already correct |
| `SkillsApi` and `Hud` waits | Pending input | See [Routing fields kept](#routing-fields-kept) and [Semantics](#semantics) | `combatPlaysEsa` and `keyMask` fixed. The others are already correct. |
| `RunToggle.Ready` (+24) | Run mode restored | `Restore` f25907 is the only writer, and it only sets the flag. `GameplayScreen/<Enter>d__283` f48925 calls `Restore` for opcode 309 in the session's start set (`0x10340a4`) and registers a handler for later 309 packets (f38370, at `0x103497a`). | Already correct. Until then, `RunToggle.On` lacks the saved mode. |
| `HotkeyMap.Bindings` `Dig` and `Run` | Catalog actions | `HotkeyMap.MakeCatalog` f24832 builds `Run` (default `KeyR`) and `Dig` (default `KeyD`), neither locked. The constructor f24839 calls `Rebuild` f24840, which stores every catalog action in `Bindings`. | Already correct. Both exist before the deferred drill install. A cleared or multi-key binding refuses `binding`. |
| `LoadedZone.Warps` (+24) | Portal list | `Portals.WarpRegions` f26766 always returns a new list. `EnterCampScene` f48931 and `ShowLocalScene` f48976 store an empty list. | Already correct on every map |
| `WorldActor.path` (+12) | Path list | The constructor f26861 allocates it. `Stop` f26872 only clears it. | Already correct |
| `CombatModel` sets (+104, +124, +128, +136) and `Fsm` (+112) | Readonly fields | Set at construction | Already correct |
| `UiSkins.started` | Set on first use | `GameplayScreen.Advance` f38568 calls `UiSkins.get_Current` f50226 every frame. | Already correct. The `cold` refusal clears on the first world frame. |
| `GameplayScreen.probedTime` (+632) | Pointer probe time | Only `MapPoint` f38566 writes it, so it starts at 0. | Already correct. Only a damaged read is negative. |

### Live checks

Static analysis cannot settle these states:

- Digging, combat, open windows and other maps. The live run covered only an idle character.
- `EventSystem.m_CurrentSelected` after a click on a UI element. If it stays set, every sample refuses `routing`.
- A scene-placed `MultiplayerEventSystem`. It refuses `type`.
- `SkillsApi.UiWait` after an interrupted scene. If it stays set, every sample refuses `routing`.
- `RunToggle.Ready`. If the server never sends opcode 309, every sample refuses `mode`.
- The Custom skin before the first dig-key press. `dig.manual` stays null, which refuses only dig actions.

### Live-refusal verification

Each fix has cases in `crates/net/tests/sampler.cjs`. A scratch runner replayed each fix's new cases against the commit before the fix, with the read checker off. A mutation pass then broke each fix in a scratch copy of the current `sampler.js`, one change at a time. The full suite failed for every change.

| Fix | Cases | Before the fix | Mutations |
| --- | --- | --- | --- |
| `combatPlaysEsa` | "combat ESA playback is a setting, not pending input" | `routing` | 1 of 1 fails. Restoring the read refuses `routing`. |
| `keyMask` | "HUD key mask N leaves Dig and Run routed" for N of 524287 (`0x7FFFF`), 12289 (`0x3001`) and 0 | `type`, from the earlier keyboard read | 1 of 1 fails. Restoring the read refuses `routing` for 524287 and 12289. |
| `FastKeyboard` | "FastKeyboard is the current keyboard" and one refusal case | `type` | 1 of 1 fails |
| Uncached buttons | The 15 cases whose names start with "uncached" | 13 of 15 differ. The other 2 refuse `input` both before and after. | 2 of 2 fail |
| `ItemMap` | The base sample, 2 gun cases and 6 `ItemMap` cases | `type` for all 9 | 7 of 7 fail |
| Unused cells | 66 cases, including every pinned cell left unused and given another word | All 66 differ | 14 of 14 fail |
| Missing item info | "worn item ids missing from the ItemMap" | `cold` | 2 of 2 fail |

The Rust test `manual_dig_arming_hold_acknowledgement_and_cleanup_are_distinct` covers an unknown and a semi-automatic setting. Both refuse dig actions.

```sh
python3 -I tools/build-migrate/sampler/derive.py --build ea1a68bd89958fae94179f1e0835ffb5 WASM DATA \
  --expect MAIN.json --out NEW.json   # exit 1: 5 MOVED, 60 CHANGED; NEW.json equals ea1a68bd.json
python3 -I tools/build-migrate/sampler/derive.py --build ea1a68bd89958fae94179f1e0835ffb5 WASM DATA \
  --expect tools/build-migrate/sampler/ea1a68bd.json --out NEW.json   # exit 0: 0 MOVED, 0 CHANGED
node crates/net/tests/sampler.cjs   # PASS sampler 223 invented-heap cases
```

`MAIN.json` is `ea1a68bd.json` at `2832f6f`. The 5 moved pins are the new `cellWords` table, the `InputStateBuffers` cell and three new types. The 60 changed pins add the fields, bases, sizes, RGCTX slots and method fingerprints that the new reads use, and remove `combatPlaysEsa` and `keyMask`.

## Limits

- No sample has reached `ready` against a live client. The evidence is static or synthetic.
- The `RUNTIME` offsets come from this build's compiled libil2cpp, not from Unity source.
- The ground-item check is a replacement. It assumes that item actors carry the same hit boxes and hidden flags as the game's own world pick. It still reads `HoverDraw.Entry` in case a later build sets it.
- The game tests `EditBox.Focused` with Unity's object inequality, which also treats a destroyed editor as null. The sampler tests for any reference, so it can refuse where the game would route keys to the HUD.
- Nobody has timed the deferred drill install. The live table lacked `DrillFeature` for at least 20 seconds.
- A later build could add another source of dig state. A migration must recheck the provider and caller lists in [Drill lifecycle](#drill-lifecycle), because no pin covers them.
- `Hud.OnKey` also drops keys while a message box is open (`MessageBox.IsOpen`) or when an editor took a key in the current frame (`EditBox` static +28, `KeyTakenFrame`). The sampler checks neither, before or after this change.
- The six live refusals came from one idle character. The states in [Live checks](#live-checks) have no live evidence.
- The uncached button reader models the formats that keyboard controls use. Any other format refuses `input`.
