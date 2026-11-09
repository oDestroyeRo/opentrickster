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
| Hotkey install | `Hud +128` as a boolean | +128 is `Waiting : System.Action`. The state is `installing : bool` at +132. | `Hud.OnKey` f48516 passes the key to `WaitForKey` f48518 while +132 is true. `Hud.PressHotkey` f48491 invokes `Waiting` only while +132 is true. | +132 must be false (`routing`). |

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
| Hotkey install | "installing HUD routes the next key elsewhere" |
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

## Limits

- No sample has reached `ready` against a live client. The evidence is static or synthetic.
- The `RUNTIME` offsets come from this build's compiled libil2cpp, not from Unity source.
- The ground-item check is a replacement. It assumes that item actors carry the same hit boxes and hidden flags as the game's own world pick. It still reads `HoverDraw.Entry` in case a later build sets it.
- The game tests `EditBox.Focused` with Unity's object inequality, which also treats a destroyed editor as null. The sampler tests for any reference, so it can refuse where the game would route keys to the HUD.
- `Hud.OnKey` also drops keys while a message box is open (`MessageBox.IsOpen`) or when an editor took a key in the current frame (`EditBox` static +28, `KeyTakenFrame`). The sampler checks neither, before or after this change.
