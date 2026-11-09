# Build ea1a68bd client-assisted sampler migration

Issue [#37](https://github.com/oDestroyeRo/opentrickster/issues/37). Public static research completed **2026-11-09 UTC**, from build **8502ebbf2fc9033d2fba50d02fd92002** to build **ea1a68bd89958fae94179f1e0835ffb5**. This report covers the client-assisted sampler and the #31 Trial A probe. The [native migration](build-ea1a68bd-migration.md) covers Net's other pins.

**No class or field that the client-assisted sampler reads changed meaning.** One read moved: `GameplayScreen.probedAt` is now at +624, because the build inserted a `drivers` reference (`Spinel.Game.Play.AvatarDrivers`) at +620. Every other difference is renumbering: 78 type-definition indices, 26 metadata usage cells, the typedef bound and the Heavy callback's function-table slot. Field offsets, by-value type kinds, instance sizes and the generic-context (RGCTX) slots that the sampler reads are unchanged.

`sampler.js` and its invented heap now target ea1a68bd. The native migration (#43) already moved the profile to ea1a68bd, so without this change the live sampler reports `unavailable` with reason `build`. With it, the build gate passes.

The work also found **five pre-existing read defects** in both builds. The first one stops every live sample before `ready`, so the sampler still fails closed. Two later checks could pass without testing their intended state once the earlier reads are fixed. This change fixes none of them.

The #31 Trial A probe was regenerated outside Git for ea1a68bd and statically validated. It was **never instantiated**. This work performed no login, game socket, gameplay, account access or target-code execution.

## Evidence boundary

The branch `feat/migrate-sampler-ea1a68bd` is based on main `604fb63`, which includes the #38 sampler planning change and the #43 native migration. It changes `crates/net/src/sampler.js`, `crates/net/tests/sampler.cjs`, `tools/build-migrate/sampler/`, this report and its index row. It does not change the profile, codec, catalog, decoders or the native `tools/build-migrate` files. It commits no game assets, binaries, captures, credentials or account values.

The [public page](https://tr-play.playall.in.th/), [configuration](https://tr-play.playall.in.th/spinel-config.json) and [version](https://tr-play.playall.in.th/version) were fetched without cookies at 2026-11-09T07:24:36Z. The configuration and version name ea1a68bd. The configuration's `dataVersion` equals the 8502ebbf value. The page names the four Build assets below. The saved `.gz` files hold decoded bytes; `derive.py` and `instrument.py` accept decoded or gzip input.

| Public artifact | Decoded bytes | SHA-256 |
| --- | ---: | --- |
| Page | 37583 | `f9cd52c6bf781b3d533bc0caa00328ff69e772000f7ee9ec8aba67dad60c55cd` |
| Configuration | 124 | `0e30790458ae431e691b66f05c8bd1fff11d109a23c2ccb014520715c7389b78` |
| Version | 44 | `c4ddd91c18ba505935d361296dca3559580f4d8b1880594dd7e9ed6db07741cf` |
| [WASM][W] | 43147580 | `8780db4414df5357d9595ef233579227a1303753d00ac6318d225036dd1bb277` |
| [Unity data][D] | 27730162 | `a3b60bc72c11d7e38c05ed7cd312c26f7161df31118401f0d56a9b4cba3693a3` |
| Embedded metadata, offset 16750046 | 9348436 | `2c7ae9e8bbd0d4e38b2e7be06cb0fabf8c0affa23d9400ea3b4c76aedc248056` |
| Framework | 393439 | `291c9124760f70c66c6754e1b6a56648e02d7b3423bf7a1d35088dce6a89871b` |
| Loader | 27914 | `8a7d5786f4b9c4db61c9687f97043cadbb0d3c8dc949e6cba38a666ec8c02a1a` |

The 8502ebbf side is the pinned WASM `381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404`, data `fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474` and metadata `7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade` from the [inventory baseline](native-inventory-baseline.md). Both builds use Unity 6000.6.5f1 and metadata format 108. ea1a68bd has 15447 type definitions (+7), 88943 methods (+51), 102541 function bodies (+43) and 493 imported functions (+3).

Evidence classes:

- **Rerunnable derivation.** `tools/build-migrate/sampler/derive.py` locates every pin by stable name and fingerprints the transcribed methods.
- **One-time scratch analysis.** A separately written metadata parser, a whole-program body comparison and callee-pairing scripts ran outside Git and are not retained. Their results are marked "one-time" below.
- **Synthetic checks.** The invented heap and the Trial A tests use invented memory only. They prove reader logic, not live correspondence.

## How the pins were derived

`derive.py` parses the WASM and the metadata embedded in the data asset. It finds the metadata registration structurally: its field-offset and type-size counts equal the typedef count. It matches code-generation modules by image name and method count. Then it resolves:

- type-definition indices by full name, with the by-value Il2CppType kind and instance size;
- field offsets by type and field name, with `<Name>k__BackingField` written as `Name`;
- metadata usage cells by identity: a TypeInfo by type name, a MethodRef by method, parameter types and generic arguments. Each must occur exactly once in initialized data;
- the Heavy callback slot by the method name `CarriedWeightUi/<>c__DisplayClass2_0::<Install>b__2()`;
- the class RGCTX slots that the sampler reads through `MethodInfo+28`, after collapsing constrained and field pairs into one runtime slot;
- fingerprints of all 112 overloads of the methods that the sampler transcribes. Calls name the import or managed callee. Constants that decode as metadata cells keep the cell's identity. Other static-data addresses become anonymous. Compiler-generated ordinals are canonicalized.

Two checks validate the locator. On 8502ebbf, `derive.py` reproduces every pinned index, cell, bound and slot in the previous `sampler.js` and fixture. The only derived values absent from both files are the two layout anchors listed below. On ea1a68bd, the separately written parser produced the same 78 type and 26 cell mappings (one-time).

## What changed between the builds

`derive.py --expect` reports 106 moved pins and 2 changed pins.

| Changed pin | 8502ebbf | ea1a68bd | Review |
| --- | --- | --- | --- |
| `GameplayScreen.probedAt` | +620 | +624 | A new `drivers : AvatarDrivers` reference occupies +620 and takes the 4 padding bytes that preceded `probedTime`. `probedTime` (+632), `overUi` (+640) and the 656-byte instance size are unchanged. |
| `GameplayScreen::MapPoint(Vector2)` fingerprint | `a0acf52d…` | `51851d96…` | Both bodies are 624 bytes with the same opcodes. Three instructions differ: an address constant and a store offset move from 620 to 624, and two 64-bit accesses now declare 8-byte alignment. |

The other 111 fingerprints are equal. They include `GameplayScreen::ClickAt`, whose only change is a compiler-generated ordinal. All four RGCTX layouts that the sampler reads are equal:

| Generic class | Slots the sampler reads |
| --- | --- |
| ``System.Nullable`1`` | +0 class `T`, +8 `value` offset, +12 `hasValue` offset |
| ``System.Collections.Generic.Dictionary`2`` | +36 `Entry.hashCode`, +40 class `TKey`, +44 `Entry.key`, +48 class `TValue`, +52 `Entry.value` |
| ``UnityEngine.InputSystem.InputControl`1`` | +4 class `TValue`, +16 `evaluateProcessorsEveryRead`, +28 `m_CachedValue` |
| ``Spinel.Net.OrderedMap`2`` | +52 ``Dictionary`2<TKey,i4>::TryGetValue``, +84 ``Dictionary`2<TKey,i4>::ContainsKey`` |

One-time checks extend this:

- Across the 78 pinned types, the only other new field is a `GameplayScreen` static, `BackgroundStalledKey : string`. The sampler reads no `GameplayScreen` statics.
- The 112 bodies and their native callees reach 579 native or shared-generic bodies and call 344 managed methods. Paired by call position, every managed callee except `MapPoint` has an equal fingerprint. 575 of the native bodies are equal. The other 4 differ only in function-table constants shifted by 54 slots, and the shifted slots hold fingerprint-equal functions.
- All 864 RGCTX owners and all 948 compared enum types are identical.
- The build adds a background-run feature with three new imports: `env.SpinelBackgroundState`, `env.SpinelBackgroundTake` and `env.SpinelBackgroundWant`. `GameFlow.Update` now calls `BackgroundRun.Follow`. The `GameplayScreen.Enter` coroutine registers background-run handlers and stores `drivers`. The sampler transcribes neither method.

The native migration left three methods cited by the [assisted input report](assisted-input-control.md) for this review: `MapPoint`, `ClickAt` and the `Enter` coroutine. This report covers all three. It does not record them in `tools/build-migrate/signatures.json`, so `migrate.py check` on ea1a68bd still fails on them.

## Sampler update

`sampler.js` changes only its build string, the keys of its by-value kind table, the typedef bound (15439 → 15446), type indices, cells, the Heavy slot (49652 → 49699) and the probe read (`screen+620` → `screen+624`). Every extent, type, RGCTX, version and consistency check is unchanged. So is the #38 continuation window in `sampler.rs`. A comment names the ea1a68bd `Keyboard` getters f34098/f34099 and f34042.

The invented heap moves the same pins. It writes `probedAt` at +624/+628 and the Heavy slot 49699. Three cases are new:

- a reference in `drivers` (+620) leaves the probe and pick intact;
- a probe written at the previous offset leaves the pointer unmapped;
- a delegate with the 8502ebbf Heavy slot 49652 fails with reason `mode`.

Reverting either `screen+624` or the Heavy slot in `sampler.js` fails the suite.

| Type definition | 8502ebbf | ea1a68bd | Pinned in |
| --- | ---: | ---: | --- |
| `Spinel.Engine.StageLayout` | 14434 | 14441 | layout anchor only |
| `Spinel.Engine.Unity.GameInput` | 13641 | 13644 | sampler and fixture |
| `Spinel.Engine.Unity.SpinelEngine` | 13662 | 13665 | sampler and fixture |
| `Spinel.Engine.Unity.Stage` | 13683 | 13686 | sampler and fixture |
| `Spinel.Engine.Unity.World.ActorView` | 13740 | 13743 | sampler and fixture |
| `Spinel.Engine.Unity.World.MapView` | 13771 | 13774 | sampler and fixture |
| `Spinel.Game.Play.AutoDrill` | 5779 | 5782 | sampler |
| `Spinel.Game.Play.AvatarFsm` | 5807 | 5810 | sampler and fixture |
| `Spinel.Game.Play.AvatarStatusFlags` | 5810 | 5813 | sampler and fixture |
| `Spinel.Game.Play.CombatModel` | 6101 | 6104 | sampler and fixture |
| `Spinel.Game.Play.CombatModel/Aging` | 6080 | 6083 | sampler and fixture |
| `Spinel.Game.Play.CombatModel/WaitingMove` | 6079 | 6082 | fixture |
| `Spinel.Game.Play.DigAssist` | 6215 | 6218 | sampler and fixture |
| `Spinel.Game.Play.DrillData` | 6225 | 6228 | sampler and fixture |
| `Spinel.Game.Play.DrillFeature` | 6256 | 6259 | sampler and fixture |
| `Spinel.Game.Play.DrillParam` | 6222 | 6225 | sampler and fixture |
| `Spinel.Game.Play.GunFlowing` | 6629 | 6632 | sampler and fixture |
| `Spinel.Game.Play.HotkeyMap` | 6741 | 6744 | sampler and fixture |
| `Spinel.Game.Play.ItemInfo` | 6756 | 6759 | sampler and fixture |
| `Spinel.Game.Play.KeyBinding` | 6892 | 6895 | sampler and fixture |
| `Spinel.Game.Play.PlayerStats` | 7412 | 7415 | sampler and fixture |
| `Spinel.Game.Play.RunToggle` | 7915 | 7918 | sampler and fixture |
| `Spinel.Game.Play.SkillsApi` | 8024 | 8027 | sampler and fixture |
| `Spinel.Game.Play.StatValues` | 7409 | 7412 | sampler and fixture |
| `Spinel.Game.World.Point` | 5590 | 5592 | sampler and fixture |
| `Spinel.Game.World.WarpRegion` | 5543 | 5545 | sampler and fixture |
| `Spinel.Game.World.WorldActor` | 5465 | 5467 | sampler and fixture |
| `Spinel.Game.World.ZoneInfo` | 5619 | 5622 | sampler and fixture |
| `Spinel.Net.Game.GameSession` | 11817 | 11820 | sampler and fixture |
| `Spinel.Net.Game.InvItem` | 12070 | 12073 | sampler and fixture |
| `Spinel.Net.Game.Inventory` | 12096 | 12099 | sampler and fixture |
| ``Spinel.Net.OrderedMap`2`` | 11317 | 11320 | sampler and fixture |
| `Spinel.UI.Pc.EditBox` | 2374 | 2375 | sampler and fixture |
| `Spinel.UI.Pc.Hud.Hud` | 5355 | 5357 | sampler and fixture |
| `Spinel.UI.Pc.Hud.HudCover` | 5356 | 5358 | sampler and fixture |
| `Spinel.UI.Pc.Play.CarriedWeightUi/<>c__DisplayClass2_0` | 2950 | 2952 | sampler and fixture |
| `Spinel.UI.Pc.Play.DigKeySettings` | 3183 | 3185 | sampler and fixture |
| `Spinel.UI.Pc.Screens.GameFlow` | 2619 | 2621 | sampler and fixture |
| `Spinel.UI.Pc.Screens.GameplayScreen` | 2667 | 2669 | sampler and fixture |
| `Spinel.UI.Pc.Screens.GameplayScreen/LoadedZone` | 2620 | 2622 | sampler and fixture |
| `Spinel.UI.Pc.Screens.GroundItems` | 2685 | 2687 | sampler and fixture |
| `Spinel.UI.Pc.Screens.GroundItems/Entry` | 2668 | 2670 | sampler |
| `Spinel.UI.Pc.Screens.PlayServices` | 2731 | 2733 | sampler and fixture |
| `Spinel.UI.Pc.UiPressOrigin` | 2480 | 2481 | sampler and fixture |
| `Spinel.UI.Pc.UiSkins` | 2483 | 2484 | sampler and fixture |
| `System.Action` | 8924 | 8927 | fixture |
| `System.Boolean` | 8959 | 8962 | sampler and fixture |
| ``System.Collections.Generic.Dictionary`2`` | 9988 | 9991 | sampler and fixture |
| ``System.Collections.Generic.Dictionary`2/Entry`` | 9982 | 9985 | sampler and fixture |
| ``System.Collections.Generic.IReadOnlyDictionary`2`` | 9997 | 10000 | sampler and fixture |
| ``System.Collections.Generic.List`1`` | 10003 | 10006 | sampler and fixture |
| ``System.Collections.Generic.Queue`1`` | 10012 | 10015 | fixture |
| `System.Delegate` | 9177 | 9180 | fixture |
| ``System.Func`1`` | 8932 | 8935 | sampler and fixture |
| `System.Int32` | 9020 | 9023 | sampler and fixture |
| `System.Int64` | 9021 | 9024 | sampler and fixture |
| `System.MulticastDelegate` | 9198 | 9201 | layout anchor only |
| ``System.Nullable`1`` | 9043 | 9046 | sampler and fixture |
| `System.Object` | 9200 | 9203 | sampler and fixture |
| `System.RuntimeType` | 9155 | 9158 | sampler and fixture |
| `System.Single` | 9064 | 9067 | sampler and fixture |
| `System.String` | 8914 | 8917 | sampler and fixture |
| `System.Type` | 9086 | 9089 | sampler and fixture |
| `System.UInt64` | 9092 | 9095 | sampler and fixture |
| `UnityEngine.EventSystems.EventSystem` | 14177 | 14184 | sampler and fixture |
| `UnityEngine.InputSystem.Controls.AnyKeyControl` | 10577 | 10580 | sampler |
| `UnityEngine.InputSystem.Controls.ButtonControl` | 10580 | 10583 | sampler and fixture |
| `UnityEngine.InputSystem.Controls.DiscreteButtonControl` | 10583 | 10586 | sampler |
| `UnityEngine.InputSystem.Controls.KeyControl` | 10588 | 10591 | sampler and fixture |
| `UnityEngine.InputSystem.InputAction` | 10211 | 10214 | sampler and fixture |
| `UnityEngine.InputSystem.InputActionMap` | 10227 | 10230 | sampler and fixture |
| `UnityEngine.InputSystem.InputActionState` | 10245 | 10248 | sampler and fixture |
| ``UnityEngine.InputSystem.InputControl`1`` | 10272 | 10275 | sampler and fixture |
| `UnityEngine.InputSystem.InputManager` | 10327 | 10330 | sampler and fixture |
| `UnityEngine.InputSystem.InputSystem` | 10203 | 10206 | sampler and fixture |
| `UnityEngine.InputSystem.Keyboard` | 10296 | 10299 | sampler and fixture |
| `UnityEngine.Vector2` | 13048 | 13051 | sampler and fixture |
| `UnityEngine.Vector2Int` | 13049 | 13052 | sampler and fixture |

| Metadata usage cell | 8502ebbf | ea1a68bd |
| --- | ---: | ---: |
| TypeInfo `UnityEngine.InputSystem.Controls.ButtonControl` | 3242296 | 3243056 |
| TypeInfo `Spinel.UI.Pc.Play.DigKeySettings` | 3244064 | 3244824 |
| TypeInfo `Spinel.UI.Pc.EditBox` | 3245684 | 3246444 |
| TypeInfo `UnityEngine.EventSystems.EventSystem` | 3246052 | 3246812 |
| TypeInfo `Spinel.UI.Pc.Screens.GameFlow` | 3246756 | 3247516 |
| TypeInfo `Spinel.UI.Pc.Hud.HudCover` | 3248164 | 3248924 |
| TypeInfo `UnityEngine.InputSystem.InputSystem` | 3249368 | 3250132 |
| TypeInfo `UnityEngine.InputSystem.Keyboard` | 3249944 | 3250708 |
| TypeInfo `Spinel.Engine.Unity.SpinelEngine` | 3255668 | 3256436 |
| TypeInfo `Spinel.UI.Pc.UiPressOrigin` | 3257428 | 3258196 |
| TypeInfo `Spinel.UI.Pc.UiSkins` | 3257440 | 3258208 |
| MethodRef ``mscorlib.dll|System.Collections.Generic.Dictionary`2::ContainsKey(!TKey)<i8,i4>`` | 3296664 | 3297464 |
| MethodRef ``mscorlib.dll|System.Collections.Generic.Dictionary`2::TryGetValue(!TKey,!TValue&)<string,System.Collections.Generic.List`1<Spinel.Game.Play.KeyBinding>>`` | 3298216 | 3299016 |
| MethodRef ``mscorlib.dll|System.Collections.Generic.Dictionary`2::TryGetValue(!TKey,!TValue&)<System.Type,object>`` | 3300460 | 3301272 |
| MethodRef ``Unity.InputSystem.dll|UnityEngine.InputSystem.InputControl`1::get_value()<r4>`` | 3309104 | 3309924 |
| MethodRef ``mscorlib.dll|System.Nullable`1::GetValueOrDefault()<bool>`` | 3331940 | 3332756 |
| MethodRef ``mscorlib.dll|System.Nullable`1::get_HasValue()<bool>`` | 3331948 | 3332764 |
| MethodRef ``mscorlib.dll|System.Nullable`1::GetValueOrDefault()<i4>`` | 3332368 | 3333184 |
| MethodRef ``mscorlib.dll|System.Nullable`1::get_HasValue()<i4>`` | 3332376 | 3333192 |
| MethodRef ``mscorlib.dll|System.Nullable`1::GetValueOrDefault()<vt:UnityEngine.Vector2>`` | 3333252 | 3334068 |
| MethodRef ``mscorlib.dll|System.Nullable`1::get_HasValue()<vt:UnityEngine.Vector2>`` | 3333256 | 3334072 |
| MethodRef ``mscorlib.dll|System.Nullable`1::get_HasValue()<vt:UnityEngine.Vector2Int>`` | 3333272 | 3334088 |
| MethodRef ``mscorlib.dll|System.Nullable`1::get_Value()<vt:UnityEngine.Vector2Int>`` | 3333276 | 3334092 |
| MethodRef ``Spinel.Net.dll|Spinel.Net.OrderedMap`2::Get(!TKey)<i4,Spinel.Engine.Unity.World.ActorView>`` | 3333612 | 3334428 |
| MethodRef ``Spinel.Net.dll|Spinel.Net.OrderedMap`2::ContainsKey(!TKey)<i4,Spinel.Game.Play.CombatModel/Aging>`` | 3333828 | 3334644 |
| MethodRef ``Spinel.Net.dll|Spinel.Net.OrderedMap`2::Get(!TKey)<u8,Spinel.Net.Game.InvItem>`` | 3334228 | 3335044 |

| Other pin | 8502ebbf | ea1a68bd |
| --- | ---: | ---: |
| Typedef bound in `type()` | 15439 | 15446 |
| Heavy callback element (`Delegate.method_ptr`) | 49652 | 49699 |
| `GameplayScreen.probedAt` | +620 | +624 |
| `Keyboard` getters cited in the comment | f34070/f34071, f34014 | f34098/f34099, f34042 |

`StageLayout` and `MulticastDelegate` anchor field layouts that the sampler reads at fixed offsets. Their indices are not pinned in code.

## Pre-existing defects

These reads were wrong in 8502ebbf and are unchanged in ea1a68bd. The invented heap encodes the sampler's current model, so the synthetic suite does not detect them. Rows follow read order. The first row stops every live sample, so the sampler fails closed today. Fixing them is outside #37.

| Sampler read | Actual layout in both builds | Live effect |
| --- | --- | --- |
| Service key `RuntimeType +12` as the Il2CppType | +12 is `type_info : MonoTypeInfo`. The type handle is `Type._impl` at +8. | Stops every sample at the first wanted service: `memory` when `type_info` is null. |
| `Inventory +12` and `GameplayScreen +244` as ``Dictionary`2`` | Both are ``OrderedMap`2``. The wrapper slot, RGCTX +52, is the internal ``Dictionary`2<TKey,i4>::TryGetValue``, not the map's own rows. | `type` |
| `screen +252` as `GroundItems`; `ground +68` as `GroundItems/Entry` | +252 is ``groundItems : Dictionary`2<i4,GroundItemMessage>``. `GroundItems` is `ground` at +256. Its +68 `hover` is a `HoverDraw`, whose `Entry` is at +12. | `type` |
| `EditBox` static +0 must be zero | Static +0 is `SelectedText`, a `TextStyle` value. The focused editor is `Focused` at static +24. | `routing` when the style's first word is non-zero. Otherwise the check passes without testing focus. |
| `Hud +128` as a boolean | +128 is `Waiting : System.Action`. | `shape` when a set delegate's low address byte exceeds 1. Otherwise it reads false. |

The Trial A probe reads `OrderedMap` through its entries array, so it does not share the second defect.

## Trial A probe (#31)

The regenerated probe is in `/private/tmp/opentrickster-inventory31-ea1a68bd.g2LX5M`. Its `instrument.py` ports the original recipe by method identity. It first regenerates the original `instrumentation.json`, `instrumented.wasm` and `future-probe.js` from 8502ebbf and requires byte identity. It then requires each instrumented method to have an equal fingerprint and opcode sequence in ea1a68bd, and maps every site by instruction index. The global layout, injected code and export names are unchanged.

| Instrumented method | 8502ebbf function and sites | ea1a68bd function and sites |
| --- | --- | --- |
| `GameServerLink::Receive` | f10560: entry `0x30f4ba` | f11699: entry `0x34f524` |
| `GameSession::.ctor` | f13257: `0x3add5b`, `0x3add6b` | f20878: `0x65c0b5`, `0x65c0c6` |
| `Inventory::.ctor` | f11986: entry `0x35ea6b`, `0x360351` | f13069: entry `0x3a023a`, `0x3a1b20` |
| `GameSession::EnterWorld` | f13270: entry `0x3ae563` | f20891: entry `0x65c8be` |
| `CarriedWeightUi/<>c__DisplayClass2_0::<Install>g__WeightChanged\|0` | f38669: entry `0xc0caf6`, end `0xc0cbe9` | f38700: entry `0xc0e35e`, end `0xc0e451` |

The `GameSession` constructor and `EnterWorld` bodies each grew by 2 bytes: two calls in each now encode the callee index in 3 bytes instead of 2. Byte offsets inside bodies therefore do not carry across builds; instruction indices do.

`future-probe.js` changes only its plan, the Inventory TypeInfo cell (3249528 → 3250292) and its build string. `test-probe.cjs` takes the decoded WASM path as an argument and uses the new cell. All 42 field offsets from the original check are unchanged across 12 types, including `Inventory`, `InvItem`, `ItemOption`, `GameSession`, `NetFrame` and the weight closure. Of the 57 #31 source joins, 56 have equal fingerprints; the exception is the `GameplayScreen.Enter` coroutine described above. `compare.py` now expects build ea1a68bd. Its `items.json` pin `50d82abd…` equals the file that the native migration carried forward. Neither migration re-fetched the server-side catalog.

| Outside-Git file | SHA-256 |
| --- | --- |
| `instrument.py` | `d3f6578ec632705b47cf148a748db01f0a095d60f8338187c052f0800800c373` |
| `instrumentation.json` | `3bcb13498cba2ac69435dfaa31f5bc2e9bffffbe621d2aa1e3b223472dd413d2` |
| `instrumented.wasm` (43148239 bytes, 16 splices) | `c48ea0f6acb1865560c36ac5d360b4a13d7988fc69aed9a162a318bc964e6ae4` |
| `future-probe.js` | `f9754a265ba1c5523c0c39f114f3cf22ad775c7a12880a020f64c82e0fd2f6d2` |
| `evidence.json` | `9299e0de70b9503ac6a6d29ccdbee1a71cbf389d7bd1ce1887019a37867fdeba` |
| `test-probe.cjs` | `369ddd815f4c64bba4cd6d2f729ebb78ed19c3082f7c24b14f66c5017b009cc2` |
| `compare.py` | `aba323c3acba0dd4a93c781631ec21879428f378c9253024a4524956577c9eae` |
| `test-compare.py` (unchanged) | `d156bb9adbff2892932f3f71bdf59b28323a22a998878d48300967b7495a60f3` |
| `reference.py` (unchanged) | `0ed6673fc812049a4dd3bd3be639247d6d4a410206e155d3c9370c83941b458a` |

## Reproduction and checks

```sh
D=tools/build-migrate/sampler/derive.py
python3 -I $D --build 8502ebbf2fc9033d2fba50d02fd92002 OLD_WASM OLD_DATA --out OLD.json   # exit 0
python3 -I $D --build ea1a68bd89958fae94179f1e0835ffb5 NEW_WASM NEW_DATA \
  --expect OLD.json --out NEW.json   # exit 1: 106 MOVED, 2 CHANGED; NEW.json equals ea1a68bd.json
node crates/net/tests/sampler.cjs   # PASS sampler 90 invented-heap cases
cargo test -p opentrickster-net --locked
cargo clippy -p opentrickster-net --all-targets --locked -- -D warnings
cargo fmt --all --check
```

In the probe directory:

```sh
python3 -I instrument.py DERIVE_PY ORIGINAL_DIR OLD_WASM OLD_DATA NEW_WASM NEW_DATA ea1a68bd89958fae94179f1e0835ffb5
wasm-validate --enable-all instrumented.wasm   # WABT 1.0.39
node --check future-probe.js
node test-probe.cjs NEW_WASM                   # decoded WASM only
```

`instrument.py` prints `PASS: original outputs reproduced from build 8502ebbf2fc9033d2fba50d02fd92002` and reports 56 of 57 source joins body-equal. `wasm-validate` exits 0. `test-probe.cjs` checks patch byte identity, rejection of other input, the synthetic reader and malformed-memory guards without instantiating the module. With the probe directory on `sys.path`, `test-compare.py` passes its 6 mismatch cases and `reference.checks()` passes 64 assertions.

## Remaining gaps

- The five pre-existing defects keep the live sampler from reaching `ready` in either build. A follow-up must correct the reads and the invented heap together.
- `tools/build-migrate/signatures.json` does not yet record the review of `MapPoint`, `ClickAt` and the `Enter` coroutine, so `migrate.py check` on ea1a68bd still fails.
- No live correspondence was tested. Synthetic suites prove reader logic only.
- `derive.py` treats native and shared-generic callees as anonymous. Their equality above comes from a one-time pairing, which should be repeated for each migration.
- The Trial A probe is static. Runtime capture and comparison remain under #21 and #23.

[W]: https://tr-play.playall.in.th/Build/6396dfe935f7f0e3745817a7761644bd.wasm.gz
[D]: https://tr-play.playall.in.th/Build/9c68dd2a390f8dfafc9311184fecb2b6.data.gz
