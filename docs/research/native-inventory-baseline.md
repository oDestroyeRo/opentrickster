# Native inventory baseline and carried weight

Issue [#31](https://github.com/oDestroyeRo/opentrickster/issues/31), part of [#21](https://github.com/oDestroyeRo/opentrickster/issues/21). Public static research completed **2026-11-08 UTC**, build **8502ebbf2fc9033d2fba50d02fd92002**.

The client has a precise inventory initialization boundary. It records the game-server backlog synchronously during `GameSession` construction, then `Inventory` replays that fixed set. The weight display sums this cache. **The inspected source does not establish a server inventory-completion marker or a received numeric current-weight value.** It does establish the exact cache and weight that a native implementation must match.

The remaining proof is a bounded comparison. Test whether **H, the last raw receipt among accepted S5, S679, S230 and S233**, already supplies the same initial inventory as the client's actual replay boundary **B**. Require delivery through H, compare the complete cache and exact weight, then repeat the validated cutoff in a serialized native-only boot. H is a candidate until that experiment passes. S233 alone, the first S231, a zero-count list, elapsed quiet time and S282 total 6 do not establish completeness.

An outside-Git diagnostic shim, static instrumenter, source checker, reference reducer and private comparator are ready for root to use. The shim was generated and statically validated, **never instantiated**. This investigation performed no login, gameplay, account access, private capture access or target-code execution. Native implementation and live acceptance remain in #21/#23.

## Evidence boundary

The checkout began clean on `research/inventory-baseline`, parent `fe3458b6d7ef3f191dfcaaa46d7ee75df30b5b23`. Its tree is `53997f6d817e423321b5df592e04984e1215f7fd`, identical to merged main `62ae793384aa61db96faa3ea1fbf855d3fd76ba4`. Historical base `5e419ff...` is not this report's parent. This is the only repository file changed. No production code, assets, diagnostic binaries or account state are committed.

AGENTS.md, its tracker/domain pointers, GLOSSARY.md, MVP/architecture documents, the research skill and pstack investigation guidance were read. The explicit bounded-agent instruction superseded skill delegation. No helpers were started. The work used the configured runtime and did not access the secrets directory or native/private probe output.

The [public page](https://tr-play.playall.in.th/), [configuration](https://tr-play.playall.in.th/spinel-config.json) and [version](https://tr-play.playall.in.th/version) were fetched again. Configuration SHA-256 is `084900013e6edde0ce0eca286dc7ac884a86da2ece1bae361ce0671852092602`; version SHA-256 is `9ce11c71ef039ffbe6bede54dfcf5b27afb887d44b2a79f57f959638e2e771b0`. Both identify the pinned build. Page SHA-256 is `ccb9859c7a626b0d5e419d7ebe2a1a30b652293be27c70773fc6793ac6c4101d` and names the pinned WASM/data assets.

Only the three explicitly authorized files were opened in the mixed `native17` directory. Their `.gz` filenames currently contain decoded bytes. The checker detects gzip magic before decoding.

| Primary artifact | Decoded bytes | SHA-256 |
| --- | ---: | --- |
| [WASM][W] | 43136343 | `381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404` |
| [Unity data][D] | 27724487 | `fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474` |
| Embedded metadata, offset 16748911 | 9343896 | `7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade` |

Metadata format 108 has 30-byte methods, 66-byte types and 10-byte fields. There are 490 imported WASM functions. A method's token joins its owning module pointer array to an element-table entry and then to an original function body. Field offsets come from registration array `0x35d850`. `m` denotes metadata method index, `f` WASM function index, and `@` the original decoded WASM file offset. Object offsets and wire offsets are distinct.

The immutable [protocol revalidation][R17], [incoming-state completion][R14], [definitions][R27], [assisted input][R28] and [gun control][R29] reports supplied leads. Critical joins below were independently checked against the pinned primary files. WABT 1.0.39 was used for static disassembly/decompilation only. Exception-region bodies omitted by the decompiler were read in disassembly.

## Startup waits and the real replay window

The authentication waits do not supply inventory evidence. Current `EnterWorldAsync` f10056 waits 15000 ms for 11506/11507, then 2002/2001. Selection f9877 waits up to 30000 ms for 2014/2013/2 and captures the follow-up 3/2 before consuming coalesced frames. First-login f10060 waits 15000 ms for 11502/11503/11508. These are the current methods in [W]; field/key reconstruction is outside this issue and remains in [R17]. No authentication value was printed or copied into this report.

The inventory-relevant sequence is fully localizable in [W]:

| Step | Current source behavior |
| --- | --- |
| Open game link | `GameSession.ConnectAsync` f13324 calls `GameServerLink.OpenAsync` at `3b1f12`. Its body f13248 awaits `Connection.OpenAsync`; it does not wait for an inventory list. |
| Register startup waits | f10564 registers **679**, **230**, **233** at `30fb62`, `30fba8`, `30fbee`, each with a 20000 ms timeout, before sending login 4 at `30feee`. |
| Acceptance | f10564 waits for **5 or 159**, packed at `30ff3d`, with 20000 ms timeout. 159 refuses this startup. Successful 5 is followed by `WhenAll` of the three registered startup tasks at `310370`. |
| Finish native startup object | f10564 reads the three payloads into `StartState`; f13324 calls `GameSession..ctor` only after login completion, at `3b211f`. No S231 task, list count, terminal category or inventory waiter is added. |
| Record replay | f13257 enables `recordingStartSet +70` at `3add58`, calls `BindGameServer` f13259 at `3add61`, then clears the flag at `3add68`. This window has no await. |
| Drain backlog | f13259 calls `GameServerLink.Attach` f10556. Attach installs `handler +12`, swaps `backlog +16` to a new list, and synchronously invokes the handler for every element of the old list. The invocation is at `30f351`. |
| Capture StartSet | `GameSession.Handle` f13296 tests flag +70 at `3afba5` and appends the frame to `startSet +36` while it is set. Packets received afterward are ordinary live dispatch and are not appended. |
| Create inventory | `Items.InventoryOf` f11795 uses a per-session cached factory; f11985 calls `Inventory..ctor` f11986. This can happen later than B, while the gameplay screen loads. The constructor has no network await or send. |
| Enter world | After loading and installing gameplay features, `GameplayScreen.Enter` f48894 calls `GameSession.EnterWorld` at `10347a9`. f13270 sets `worldEntered +71`, sends **C5**, and flushes held sends. This is an outbound local milestone, not a received inventory-completion acknowledgement. Later UI delays are not list waits. |

`Connection.Receive` f9920 dispatches its observers/handlers before resolving matching waiter completion sources. `GameServerLink.Receive` f10560 independently applies Aux grouping. Aux 0 goes to the handler or backlog immediately. Nonzero ordinary packets accumulate per Aux. **1951** flushes that group's packets, removes the group, then delivers the marker. **1952** flushes it, replaces its queue with an empty queue, and consumes the marker. Neither ends every group or counts inventory. Raw receipt of S233 can satisfy a waiter while another group's S231 remains buffered. [W f9920@`2e9303`, f10560@`30f4b7`, pivots `30f666`, `30f67a`, `30f8df`]

Consequently, source establishes **B = completion of the synchronous backlog replay**, not `B = receipt of S233`. Scheduling and already-received traffic can make B later than H. The source does not order future server transmissions. A synthetic trace can satisfy all four bootstrap receipts with no S231, or hold S231 in Aux until after B. Both traces produce an incomplete client cache without violating those client waits. The supplied reference demonstrates these counterexamples; it does not claim the real server emits them.

There is also a lazy-construction interval between B and Inventory subscription installation. S231 after B is never included in StartSet, even if Inventory is constructed afterward. Ordinary mutations before subscription installation can likewise require scrutiny. The runtime comparison records **I**, the receive counter at inventory construction, so this interval is visible rather than hidden by a delay.

## Inventory rows, counts and subscriptions

`ReadInventoryList` m72849/f11971@`35de5d` reads `U16 count`, then the full declared number of entries. Each is `U8 Kind`, `U32 ItemId`, `U64 UID`, `U16 Count`; Kind 2/3 add four U8 state values, and Kind 2 adds `U8 optionCount` followed by that many `(U32 id, U32 a, U32 b)` options. There is no operation byte, category/page prefix, total-across-packets count or terminal flag in this consumed grammar. The strict reference rejects unknown kinds, truncation and unclassified trailing bytes. These strict failures are adapter policy; the original reader is more permissive. [W f11776, f11781, f11971]

The constructor's first pass over StartSet handles S231 and S233 in replay order. S231 rows of **Kind 1 or 2** enter `byUid`; Kind 3 is consumed but excluded. The Kind test is at `35ef80..35ef89`. `OrderedMap.Set` at `35efb2` replaces an existing UID. Multiple lists merge. A valid `00 00` list has zero entries and clears nothing. Sum of list counts, number of unique carried UIDs and number of inventory pieces are different quantities. There is no source basis for treating one list per item category, one nonempty list or the first empty list as the whole inventory. [W f11986, generic method-spec row16599 -> m70668 `OrderedMap.Set`]

S233's first 184 bytes contain 23 U64 equipment UIDs. Values below 2 normalize to zero. During the first pass, each slot is assigned its UID only if it is already in `byUid` at that point; otherwise it is assigned zero. A later S231 does not repair that slot. Galder is U32 at payload231. The constructor then makes a **second** StartSet pass for 518/521, which can set/clear known UID slot20. This second pass sees the completed first-pass dictionary. `SetSlot` prevents the same UID occupying another slot. [W f11980@`35e606`; f11986 `35f064..35f0e5`; f11987@`36037e`; f11988@`360438`]

After replay, Inventory subscribes to these incoming logical opcodes:

`518, 521, 561, 770, 160, 586, 590, 262, 246, 260, 843, 1152, 587, 599, 601, 261, 68, 41, 82, 85, 83, 86, 35, 151`.

The subscription section is f11986 `35f5a7..360348`. The three-opcode array at metadata `0x4e71e0` is `(262,246,260)`; the packed pair at `35fcb7` is `(599,601)`. **231 and 233 are absent.** Existing mutation grammars remain in [R14]; these subscriptions include refusals/notifications as well as mutations, so they are not all row replacements. No `RequestInventory`, `RequestAll` or full-list sender exists in the inspected Inventory/Items path. Inventory's outgoing methods equip, unequip, use, drop or pick up specific items. Warehouse owns the storage/bank requests. No full carried-baseline request was recovered from that path. [W, metadata owners `Items`, `Inventory`, `Warehouse`]

The source uses an `OrderedMap<UInt64,InvItem>`, not a count-only cache. `Inventory +12` points to it; `+16` points to a **U64[23]** slot array. Its generic implementation uses slots +12, tombstones +16, used +20, gaps +24, version +28. `OrderedMap.Set` f69436 and Remove f69438 supply this layout; the snapshot reader validates the runtime array stride, tombstones and each key against `InvItem.Uid`. Generic metadata field-offset zeros are not treated as real offsets.

## Complete S233 byte-coverage ledger

The task supplied an observed length of 278. No capture was opened here. The table accounts for **every byte through 277**, including bytes whose meaning remains unknown. The source does not require exact length 278; its separate consumers have different minimums. No unconsumed byte is promoted to padding, a count or a completion bit.

| Payload bytes | Proven consumer or unresolved meaning |
| --- | --- |
| 0..183 | 23 U64 equipment UIDs. `Items.ReadStartEquipment` f11980. |
| 184..207 | Twelve U16 stat points. `StartStatsWire.ParseStatBlock` f11454 and `Status.ReadStatBlock` f11457. |
| 208..209 | U16 Bonus. Same stat block. |
| 210..211 | U16 skill points, assigned by `SkillLink..ctor` f11276 at `33609f`; requires length >=212. |
| 212..213 | U16 TmLevel. f11457. |
| 214..215 | U16 Level. f11457. |
| 216..218 | Unconsumed/unclassified in the traced readers. The status consumer's >=217 guard is not a grammar for these bytes. |
| 219..220 | U16 zone. `Messages.ReadStartZone` f12343@`374c5a`. |
| 221..230 | Unconsumed/unclassified in the traced readers. |
| 231..234 | U32 Galder. f11980 and f11455. |
| 235..238 | U32 current HP. f11455. |
| 239..242 | U32 current MP. f11455. |
| 243..246 | U32 TmExp. `CharacterStatus.ApplyStart` f11470, with >=247 guard. |
| 247..250 | U32 Exp. f11455; aggregate `StartStats` f11456 requires >=251. |
| 251..270 | All twenty bytes are unconsumed/unclassified in the traced S233 readers. They are not a proven carried-weight scalar or inventory count. |
| 271 | Default ExplainGroup byte, `CharacterStatus +100`. Used for selectors other than 2/3, and for a sufficiently long older tail lacking selector273. A missing byte remains unknown in the strict reference. |
| 272 | ExplainGroup when selector273 ==2. |
| 273 | Selector. If length >=274, `ReadStartBody` selects body U16 from **S679** at offset53 for selector2, offset56 for selector3, otherwise offset37. It is not an inventory category. |
| 274..275 | U16 **warehouse storage-slot capacity**. `Warehouse.StartStorageSlots` m75165/f11703@`34fc3b` requires >=276. WarehouseLink f11710 repeats this read at `350748..350750` and calls `AddSlots` f11711. It is not carried-item count or carried weight. |
| 276 | ExplainGroup when selector273 ==3. |
| 277 | Unconsumed/unclassified in the traced readers. |

`ApplyStart`'s exact selector branch is `341e42..341e69`: selector2 chooses272, selector3 chooses276, all other selectors choose271. The destination is `CharacterStatus.ExplainGroup +100` at `341e6d`. The native policy must preserve absence rather than adopting the target's fallback zero. `ReadStartBody` f12345 independently uses selector273 only to choose a field in S679. [W]

This ledger came from the current method joins, all direct S233 constant candidates in Spinel assemblies, their parser callees and consumers. Byte scanning was only a locator; disassembly resolved false positives and branch tables. It does not prove that unconsumed server bytes have no meaning. There is no client-side load from the remaining bytes that supplies the requested positive contract.

A misleading alternate lead, `SetItemMessage.Complete`, belongs to `ServerNotices.ParseSetItem` f11250. That packet reads a player/object reference, an equipment set ID and a completion boolean. It does not count `Inventory.byUid`. `OfflineBotStop.Weight` is an enum reason; trade/buy overweight fields are refusals. None is a received current carried-weight scalar. [W f11250@`333f68`; metadata types12278,12595,12738]

## Carried weight and invalidation

The direct current-weight path is a **local calculation**. `CarriedWeightUi.Install` async body f38675 obtains `Items.InventoryOf` at `c0d802`, calls WeightChanged f38669 at `c0d829`, and subscribes to Inventory.OnChange at `c0d875`. Its callback f38670 reruns the calculation for ItemsChanged. WeightChanged enumerates `Inventory.All`, calls `ItemWeight.Carried` f22677 and stores the exact I64 result in its closure's `carried +8` before updating the UI. This is a concrete runtime oracle, independent of rounded percentage text. [W]

The [definitions report][R27] owns the actual item table and formulas. Independently rejoined here, `ItemWeight.Of` f22676 multiplies `ItemInfo.Weight +80` by `InvItem.Pieces`; f11786 returns Count for **wire Kind==1**, otherwise1. Current metadata names the field **Kind +24**; the earlier report's prose called that wire selector Type. This is distinct from catalog ItemInfo.Type/SubType. `Carried` sums with I64 arithmetic. Equipped items remain in the enumeration. Expiry is not a filter in this weight loop. An expired item still in byUid still contributes; expiry/use eligibility is a separate TimerItems contract. A missing definition or incomplete cache remains unknown in the adapter, even though the target helper can return zero for missing information. [W f22676@`6f3ba5`, f22677@`6f3c8b`, f11786@`3549fc`, f11565@`3481e6`]

**S282 total index6 is maximum carrying capacity.** Its indexed totals belong to CharacterStatus, not Inventory. S282 does not assert the cache is complete and does not provide carried weight. S283/buff recomputation and point/equipment contribution changes can clear cached totals. `Recompute` f11473 clears the nullable totals vector at +44. A new maximum must come from a fresh total or complete current contributions. [W f11469, f11491, f11492, f11473; R27]

| Event | Required native treatment |
| --- | --- |
| Fresh login, reconnect, changed character or changed build | Discard previous baseline proof, current weight, slot resolution and cached maxima. Obtain new session-scoped evidence. |
| Known, ordered inventory delta after a proven baseline | Apply the source mutation exactly and recompute weight from the resulting cache. Missing definition, unsupported operation/kind, truncation, overflow of the source journal or unresolved delivery makes the result unknown. |
| Unexpected S231 after an accepted initial cutoff | Invalidate the initial-cutoff policy for this session and pause. Merge rows only as observations; do not silently call the new list complete. Record a sanitized policy-deviation result. |
| Equip/unequip | Do not remove the item from carried weight merely because its slot changes. Re-evaluate maxima/contributions and derived movement mode. Require slot UIDs to resolve against the current cache. |
| Expiry | Re-evaluate use/equip eligibility and any changed contributions. Do not subtract weight on a local timer unless an authoritative inventory mutation removes the item. |
| Ordinary zone change with the same live GameSession | Source retains the same Inventory. `CompleteZoneChangeAsync` f13320 binds a replacement link at `3b1af2` without reenabling recordingStartSet or rebuilding Inventory. Resume still needs a proven ordered handoff with no lost deltas and fresh zone/resources. |
| Unproven handoff or connection interruption | Pause and invalidate continuity. An old StartSet, S7/zone event, C5 send or matching zone ID does not rebuild the inventory. Use a fresh full session if continuity cannot be established. |

No permanent false capability is proposed. Once root accepts a build-scoped cutoff from runtime evidence, each native startup can automatically compute that cutoff, settle delivery, construct the cache and calculate weight without Unity. The existing unknown-weight Start guard remains correct until that acceptance.

## Minimal future runtime comparison

This section specifies future work for root under its own account authorization. It grants this researcher no account lease. Use the same pinned build and one existing character. Serialize sessions and verify clean logout before the next boot. No combat, digging, pickups, item use, inventory rearrangement or account changes are needed.

### Trial A, one client-assisted boot with a passive native shadow

1. Re-fetch the public build descriptors and check all pins. Load `future-probe.js` as a temporary document-start script and invoke `inventory31.install()` before the game's WASM is instantiated. It intercepts instantiate/instantiateStreaming, patches **only** the exact pinned bytes, and verifies the patched SHA. If no instrumented instance is captured, stop. There is no fallback to guessed pointers or another build.
2. Feed the same game-server frames to the real native decoder/reducer as a passive shadow. It must send no commands. Number every raw **game-link** receipt from 1, exactly once at entry to f10560, before Aux buffering. Retain original ingress through dispatch. This is separate from gateway/login-server packet numbering.
3. Keep a private trace of `{opcode, aux, payloadHex}` in that raw order. For non-inventory opcodes, omit payloadHex. Retain payload bytes only for the inventory allowlist in `compare.py`. Never retain credentials/tickets or whole socket bodies for this comparison. Keep root's actual queues and pending partial-frame accounting available to verify settlement.
4. The shim records B at `3add5b/3add6b`, I at Inventory construction, and W at C5 EnterWorld. It records all S231 counts/last positions and the four startup receipt positions. Wait for normal world entry, inventoryReady and weightReady, then take one synchronous private snapshot with `inventory31.snapshot(inventory31.instance.exports)`. It reads the **actual cache**, slot UIDs, selected StartSet inventory payloads and the closure's exact carried weight. It calls no target method. The corresponding shadow snapshot must use the same final raw mark R.
5. Root supplies `native.json` with `build`, `rx`, full `rows`, `slots`, and diagnostic `candidateWeight`. The row shape is `{uid, itemId, kind, count, state, options}`. UID/itemId/options values use decimal strings to avoid JavaScript precision loss. State is four integer bytes for Kind2, otherwise empty; options are `[id,a,b]` triplets. This candidate weight must not reach the engine as known before the experiment passes.
6. Run the private comparator below. It reconstructs Aux delivery, verifies the actual StartSet membership against B, compares initial projections at H and B, requires all observed S231 at/before H, compares every live UID/field/slot at R, and compares exact I64 weight against both Unity and the pinned actual public item definitions. Counts/rounded weight alone cannot pass. Missing lists, pending Aux, repeated/missing required startup packets, unclassified rows, an unreadable cache or mismatched marks fail explicitly.

```sh
python3 /private/tmp/opentrickster-inventory31.nU6vBQ/compare.py \
  /outside-git/private/unity.json /outside-git/private/native.json \
  /outside-git/private/gs-trace.json \
  /private/tmp/opentrickster-definitions.z72itC/items.json
```

The comparator emits only a sanitized PASS/FAIL, reason, build, boundary counters and row/list counts. It does not print UID, item names, payloads or weight. Its `production_policy_accepted` result is deliberately false because root still owns acceptance. Private results remain outside Git and outside public issue comments.

This is not a quiet-time test. The observation boundary is normal world entry plus a synchronized snapshot. A timeout only bounds a failed experiment; it never creates completeness. Check root's live transport for unparsed/raw/Aux frames through H as well as the comparator's decoded trace. The shim's `rx` alone does not prove that an outer transport buffer is empty.

If H and B differ, the exact missing list ordinals and Aux groups are the next bounded finding. Do not retry with a longer delay or designate the last seen list as terminal. Inspect the finite `(H,B]` packet suffix and its containing transport callback for a repeatable source/wire milestone, then test that proposed milestone. If no such observable boundary exists, this trial has disproved H; it has not licensed automation. The full private client snapshot remains the oracle for that narrower comparison.

### Trial B, one serialized native-only boot

After clean client logout, start the same existing character with Unity absent. Use the validated H policy and the real native transport settlement checks. Compare its private complete rows/slots/weight with Trial A, accounting for any explicitly observed server-side mutation. Unexplained changes make the comparison inconclusive. Verify that Start becomes available from actual known resources and that disconnect/reconnect returns to unknown/paused until a fresh baseline is acquired. Do not initiate gameplay to test this research boundary.

A pass establishes the target build's behavior for the exercised startup, character and transport conditions. It is not a universal server guarantee. Root should record whether the trace exercised multiple lists, a zero-count list, duplicate UIDs, Kind3 rows or Aux buffering; unobserved variants remain unobserved. Synthetic coverage is not evidence that a real empty character was tested. Native policy should be keyed to the verified build, retain explicit-list evidence for the empty case, fail on drift or unexpected late lists, and never depend on Unity during ordinary operation.

## Reproduction and checks

All scratch is under `/private/tmp/opentrickster-inventory31.nU6vBQ`. The approved mixed directory is read only by exact filename. `parse.py`, `scoped.py`, `read.py`, `decompile.py` and `windows.py` are static navigation aids. `scoped.py` preserves selected function bytes while replacing unrelated bodies with unreachable stubs; offsets in its output are translated back to original file offsets. No scoped or original target module was run.

| Artifact | Purpose |
| --- | --- |
| `check.py` | Standalone primary-artifact verifier, embedded below. Rechecks all three pins, embedded metadata identity, 6 module registrations, 57 method joins, 42 fields, 44 instruction pivots, the list-opcode array and 2 generic joins. |
| `evidence.json` | Full method/token/table/body/size/SHA and field/pivot manifest emitted by check.py. |
| `reference.py` | Strict packet/tail transcription, StartSet projection, weight and Aux model; **64 synthetic assertions** including missing-list and delayed-Aux counterexamples. |
| `instrument.py` | Offline public-WASM patch builder. Adds only 18 diagnostic mutable globals/exports and stores/counters in five existing functions. No new imports or network operations. |
| `instrumentation.json`, `instrumented.wasm` | Exact splice manifest and patched bytes. WABT static validation passed. Never instantiated. |
| `future-probe.js` | Future document-start hook and bounded private memory reader. Syntax checked; patch output matches instrumented.wasm byte for byte. Target/browser compatibility remains unrun. |
| `test-probe.cjs` | Verifies patch identity, wrong-build refusal and memory-reader checks against a synthetic ArrayBuffer, without WebAssembly instantiation. |
| `compare.py`, `test-compare.py` | Future private same-stream comparator; synthetic positive case and six mismatch cases passed. No private input was used here. |

The memory reader checks the Inventory class, session/link identity, OrderedMap extents/tombstones, key/UID agreement, array strides, options, 23 U64 slot references, and unchanged map version. It reads only inventory-related structures. Its field/layout assumptions are pinned source transcriptions and synthetic checks, not a claim of live memory validation. The diagnostic copy may affect scheduling; Trial B verifies the native cutoff separately. If the hook changes observed ordering, reject that result rather than treating the instrumented client as an unmodified timing oracle.

```sh
python3 /private/tmp/opentrickster-inventory31.nU6vBQ/check.py
python3 /private/tmp/opentrickster-inventory31.nU6vBQ/reference.py
python3 /private/tmp/opentrickster-inventory31.nU6vBQ/instrument.py
/tmp/opentrickster-issue13/tools/node_modules/wabt/bin/wasm-validate --enable-all \
  /private/tmp/opentrickster-inventory31.nU6vBQ/instrumented.wasm
node --check /private/tmp/opentrickster-inventory31.nU6vBQ/future-probe.js
node /private/tmp/opentrickster-inventory31.nU6vBQ/test-probe.cjs
python3 /private/tmp/opentrickster-inventory31.nU6vBQ/test-compare.py
```

No application build, live login, action acceptance, independent reviewer or runtime inventory completeness is claimed. The report was read back, its embedded verifier rerun, its links/byte coverage reviewed, and its one-file diff checked for whitespace. Published report bytes and remote branch SHA are verified after the single docs commit; the issue resolution comment records that immutable SHA because the report cannot contain its own commit hash.

## Compact primary-source verifier

Save this block outside Git as `check.py`; Python 3.9+ is sufficient. Its optional argument is the approved public cache directory. It opens only the three named files and writes sanitized `evidence.json` beside itself. Run without `-O`. This verifies source identity and joins; it cannot prove server ordering.

```python
import gzip,hashlib,json,pathlib,struct,sys
r=pathlib.Path(__file__).parent
assets=pathlib.Path(sys.argv[1]) if len(sys.argv)>1 else pathlib.Path('/private/tmp/opentrickster-native17.0h1SYk')
pins={
'd9a4c4fd2737e740b82e1321839e352f.wasm.gz':'381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404',
'6135c4139b62e558bb4c755b266f5fa2.data.gz':'fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474',
'current-global-metadata.dat':'7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade'}
loaded=[]
for name,sha in pins.items():
 b=(assets/name).read_bytes();b=gzip.decompress(b) if b[:2]==b'\x1f\x8b' else b
 assert hashlib.sha256(b).hexdigest()==sha,'artifact drift';loaded.append(b)
w,data,meta=loaded
assert data[:16]==b'UnityWebData1.0\0'
p=20;found=[]
while p<struct.unpack_from('<I',data,16)[0]:
 at,n,l=struct.unpack_from('<III',data,p);p+=12;name=data[p:p+l];p+=l
 if name==b'Il2CppData/Metadata/global-metadata.dat':found.append((at,n));assert data[at:at+n]==meta
assert found==[(16748911,9343896)]
assert struct.unpack_from('<II',meta)==(0xfab11baf,108)
header=lambda i:struct.unpack_from('<III',meta,8+12*i)
strings,methods,fields,types=header(2)[0],header(5)[0],header(11)[0],header(19)[0]
assert header(5)[1:]==(88892*30,88892) and header(19)[1:]==(15440*66,15440)
name=lambda at:meta[strings+at:meta.index(b'\0',strings+at)].decode()
p=8
assert w[:8]==b'\0asm\x01\0\0\0'
def u():
 global p
 value=shift=0
 while True:
  c=w[p];p+=1;value|=(c&127)<<shift;shift+=7
  if c<128:return value
secs={}
while p<len(w):
 k=w[p];p+=1;n=u();secs[k]=(p,p+n);p+=n
p=secs[2][0];imports=u();assert imports==490
for _ in range(imports):
 for _ in range(2):n=u();p+=n
 assert w[p]==0;p+=1;u()
p=secs[9][0];elements=[]
for _ in range(u()):
 assert u()==0 and w[p]==0x41;p+=1;at=u();assert w[p]==11;p+=1
 a=[u() for _ in range(u())];elements.extend([None]*(at-len(elements)));elements.extend(a)
mem=bytearray(64*1024*1024);p=secs[11][0]
for _ in range(u()):
 assert u()==0 and w[p]==0x41;p+=1;at=u();assert w[p]==11;p+=1
 n=u();mem[at:at+n]=w[p:p+n];p+=n
p=secs[10][0];bodies={}
for f in range(imports,imports+u()):n=u();bodies[f]=(p,n);p+=n
u32=lambda at:struct.unpack_from('<I',mem,at)[0]
modules={'N':(0x212548,4791,0x392c70,'Spinel.Net.dll'),'G':(0x21251c,18796,0x37f5a0,'Spinel.Game.dll'),'P':(0x2125a0,16606,0x3a9600,'Spinel.UI.Pc.dll'),'C':(0x212574,17533,0x3979f0,'Spinel.UI.Custom.dll'),'E':(0x212120,1934,0x37cf10,'Spinel.Engine.Unity.dll'),'F':(0x21214c,324,0x37f050,'Spinel.Formats.dll')}
for mod,(at,n,ptr,label) in modules.items():
 assert (u32(at+4),u32(at+8))==(n,ptr)
 assert bytes(mem[u32(at):u32(at)+len(label)+1])==label.encode()+b'\0'
M=[('P', 19609, 48894, 16973815),
 ('P', 19868, 49130, 17107421),
 ('N', 70533, 9877, 3037670),
 ('N', 70581, 9926, 3055900),
 ('N', 70801, 10060, 3091638),
 ('N', 70571, 9920, 3052291),
 ('N', 70799, 10056, 3087778),
 ('N', 72043, 10550, 3206734),
 ('N', 72049, 10556, 3207699),
 ('N', 72052, 10559, 3208235),
 ('N', 72053, 10560, 3208375),
 ('N', 72059, 10564, 3209739),
 ('N', 72061, 13248, 3855493),
 ('N', 72083, 13257, 3856780),
 ('N', 72094, 13270, 3859810),
 ('N', 72120, 13259, 3858007),
 ('N', 72121, 13296, 3865298),
 ('N', 72155, 13320, 3872201),
 ('N', 72157, 13324, 3874316),
 ('N', 72836, 11786, 3492348),
 ('N', 72846, 11776, 3489292),
 ('N', 72847, 11781, 3491548),
 ('N', 72849, 11971, 3530333),
 ('N', 72858, 11980, 3532294),
 ('N', 72861, 11795, 3494494),
 ('N', 72865, 11985, 3533358),
 ('N', 72879, 11986, 3533414),
 ('N', 72897, 11987, 3539838),
 ('N', 72902, 11988, 3540024),
 ('N', 72889, 11808, 3498030),
 ('N', 72893, 11994, 3541189),
 ('N', 72918, 12017, 3546285),
 ('N', 72919, 12018, 3546662),
 ('N', 72920, 12019, 3546813),
 ('N', 72921, 12020, 3546979),
 ('N', 73352, 12343, 3624026),
 ('N', 73354, 12345, 3624194),
 ('N', 74566, 11250, 3358568),
 ('N', 74613, 11276, 3364896),
 ('N', 74847, 11455, 3407583),
 ('N', 74848, 11456, 3407724),
 ('N', 74873, 11469, 3411028),
 ('N', 74878, 11470, 3415194),
 ('N', 74883, 11473, 3416131),
 ('N', 74893, 11491, 3419639),
 ('N', 74894, 11492, 3420457),
 ('N', 74976, 11565, 3441126),
 ('N', 75165, 11703, 3472443),
 ('N', 75188, 11710, 3474205),
 ('N', 75203, 11711, 3476320),
 ('G', 41673, 22676, 7289765),
 ('G', 41674, 22677, 7289995),
 ('P', 21159, 38669, 12634867),
 ('P', 21160, 38670, 12635115),
 ('P', 21165, 38675, 12636490),
 ('C', 5398, 57614, 20804897),
 ('P', 19311, 15248, 4478239)]
rows=[]
for mod,mi,expected_fn,expected_at in M:
 at=methods+30*mi;method_name=name(struct.unpack_from('<I',meta,at)[0]);ti=struct.unpack_from('<H',meta,at+4)[0];token=struct.unpack_from('<I',meta,at+18)[0]
 cls=name(struct.unpack_from('<I',meta,types+66*ti)[0]);slot=u32(modules[mod][2]+4*((token&0xffffff)-1));fn=elements[slot];body,size=bodies[fn]
 assert (fn,body)==(expected_fn,expected_at),(mi,cls,method_name)
 rows.append(dict(method=mi,name=cls+'::'+method_name,token=hex(token),table=slot,function=fn,body=hex(body),size=size,sha256=hashlib.sha256(w[body:body+size]).hexdigest()))
F={6756: {'Weight': 80, 'Type': 24, 'SubType': 28},
 11788: {'bundles': 8, 'handler': 12, 'backlog': 16, 'Connection': 20},
 11817: {'startSet': 36,
         'gs': 40,
         'recordingStartSet': 70,
         'worldEntered': 71,
         'handoffPending': 72,
         '<Me>k__BackingField': 96},
 12069: {'Id': 8, 'A': 16, 'B': 24},
 12070: {'Uid': 8, 'ItemId': 16, 'Kind': 24, 'Count': 28, 'State': 32, 'Options': 36},
 12096: {'session': 8, 'byUid': 12, 'slots': 16, 'galderValue': 24, '<Timers>k__BackingField': 36},
 12706: {'Totals': 44, 'TmExp': 24, 'ExplainGroup': 100},
 12800: {'Tabs': 8, 'Bank': 12, 'inventory': 32},
 11282: {'Opcode': 8, 'Aux': 12, 'Payload': 16},
 2950: {'carried': 8, 'inventory': 16, 'infos': 20},
 2667: {'play': 144},
 11333: {'Data': 8, 'Offset': 12}}
for ti,expected in F.items():
 at=types+66*ti;first=struct.unpack_from('<I',meta,at+20)[0];count=struct.unpack_from('<H',meta,at+46)[0];ptr=u32(0x35d850+4*ti)
 actual={name(struct.unpack_from('<I',meta,fields+10*(first+j))[0]):struct.unpack_from('<i',mem,ptr+4*j)[0] for j in range(count)}
 for key,value in expected.items():assert actual[key]==value,(ti,key)
assert struct.unpack_from('<3I',meta,0x4e71e0)==(262,246,260)
for cell,mi in [(3334236,70668),(3334224,70672)]:
 encoded=u32(cell);assert encoded>>29==5
 offset=header(32)[0]+6*((encoded&0x1fffffff)>>1)
 method,owner=struct.unpack_from('<IH',meta,offset);assert method==mi and owner==1665
PIVOTS=[(13257, 3857752, '3a0000'),
 (13257, 3857761, '10cb67'),
 (13257, 3857768, '3a0000'),
 (13296, 3865509, '2d0046'),
 (13296, 3865515, '0440'),
 (10556, 3207792, '36020c'),
 (10556, 3207836, '360200'),
 (10556, 3208017, '10d1fe03'),
 (10560, 3208806, '419f0f'),
 (10560, 3208826, '419f0f'),
 (10560, 3209439, '419f0f'),
 (10564, 3210082, '41a705'),
 (10564, 3210152, '41e601'),
 (10564, 3210222, '41e901'),
 (10564, 3210990, '4104'),
 (10564, 3211069, '4285808080f013'),
 (10564, 3212144, '10ae8104'),
 (11986, 3534507, '41e701'),
 (11986, 3534511, '0e0300080108'),
 (11986, 3534553, '10c35d'),
 (11986, 3534720, '280218'),
 (11986, 3534770, '10bc9e04'),
 (11986, 3534948, '10cc5d'),
 (11986, 3535044, '10c09e04'),
 (11986, 3537387, '41dce0d001'),
 (11986, 3538103, '42d784808090cb00'),
 (11470, 3415379, '418f02'),
 (11470, 3415598, '419202'),
 (11470, 3415618, '2d00a102'),
 (11470, 3415625, '0e02010002'),
 (11470, 3415631, '419402'),
 (11470, 3415657, '2d0010'),
 (11470, 3415661, '360264'),
 (11703, 3472446, '2101'),
 (11710, 3475272, '419402'),
 (11710, 3475280, '2f00a202'),
 (11710, 3475290, '10bf5b'),
 (11276, 3367044, '41e901'),
 (11276, 3367061, '41d401'),
 (11276, 3367071, '2f00e201'),
 (22677, 7290480, '1094b101'),
 (22677, 7290797, '7c'),
 (13320, 3873496, '3a0048'),
 (13320, 3873522, '10cb67')]
for fn,at,raw in PIVOTS:
 start,length=bodies[fn];expected=bytes.fromhex(raw);assert start<=at<at+len(expected)<=start+length
 assert w[at:at+len(expected)]==expected,(fn,hex(at))
evidence=dict(artifacts=pins,methods=rows,fields=F,pivots=PIVOTS,scope='public static only; source joins do not prove server ordering')
(r/'evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
print('PASS: 3 hashes, embedded metadata identity, 6 modules,',len(rows),'method joins,',sum(map(len,F.values())),'field offsets,',len(PIVOTS),'instruction pivots, inventory opcode array and 2 generic joins')
```

## Selected method fingerprints

| Method | Token / table | Function / body / bytes | SHA-256 |
| --- | --- | --- | --- |
| m72049 `GameServerLink::Attach` | `0x6000624` / 25176 | f10556@`0x30f213` / 506 | `00deee98cd7164d7016b4a80ce00a337086e77f6b156ce684d78c7f22427d6c5` |
| m72059 `<LoginAsync>d__7::MoveNext` | `0x600062e` / 25186 | f10564@`0x30fa0b` / 3713 | `dfe40d0cf340e20b16875430fcbeced93c3f71d2d6b89f850e59b94b88a5d5f7` |
| m72083 `GameSession::.ctor` | `0x6000646` / 25210 | f13257@`0x3ad98c` / 999 | `77c3db983dd5063c134388e18b1d0d98be58b46c1d0ab9da8b41e7a07e582d71` |
| m72120 `GameSession::BindGameServer` | `0x600066b` / 25247 | f13259@`0x3ade57` / 180 | `a3ba358aa0bf3aaaf50d14fa65529bd9acf350ecb2195a0a5034e1189cf1b8f3` |
| m72121 `GameSession::Handle` | `0x600066c` / 25248 | f13296@`0x3afad2` / 3465 | `206400830d1fd27238ec8e54a8c94daec18eb390bf86d9765608d36ef8419d94` |
| m72155 `<CompleteZoneChangeAsync>d__77::MoveNext` | `0x600068e` / 25282 | f13320@`0x3b15c9` / 1962 | `bc3859db0266958d2774ca35c37251c3e95ded69154548c8147ca2b508bb8d50` |
| m72849 `Items::ReadInventoryList` | `0x6000944` / 25969 | f11971@`0x35de5d` / 289 | `c588079151def1a87918f2084fe58f2a1448319cc9a3963a47df6dda6e9d47a5` |
| m72858 `Items::ReadStartEquipment` | `0x600094d` / 25978 | f11980@`0x35e606` / 630 | `788350654bcf4d9f8478159a90cf47bfb73e386c0246a3d1c7d2149cdb926c18` |
| m72879 `Inventory::.ctor` | `0x6000962` / 25999 | f11986@`0x35ea66` / 6422 | `823f8d75a88855ce17b83a932b9b376fbde268087f775740ead793791f48e76c` |
| m73354 `Messages::ReadStartBody` | `0x6000b3d` / 26474 | f12345@`0x374d02` / 106 | `42041fb86506ff09476f189de7916dacddb58a7312c680e0deae1128d153fd2b` |
| m74613 `SkillLink::.ctor` | `0x6001028` / 27732 | f11276@`0x335820` / 2375 | `e391aab303331407e8d4331a953f9a345abdaf1647e98bac4d678e5027e8c436` |
| m74878 `CharacterStatus::ApplyStart` | `0x6001131` / 27997 | f11470@`0x341c9a` / 655 | `80653fea7eaefdad93a37458ce9f8658cbdd9776e16fd67e42a7f49c7c2197c8` |
| m75165 `Warehouse::StartStorageSlots` | `0x6001250` / 28284 | f11703@`0x34fc3b` / 27 | `8a81cb14ad2ab403acbd1757fc9f5242af374a063e6a414bdc92012a25aebc7a` |
| m75188 `WarehouseLink::.ctor` | `0x6001267` / 28307 | f11710@`0x35031d` / 2113 | `b6d16cf40726a1d7619020aac3db094fea0b6c8e3552043496a7ed22f506b7e0` |
| m41674 `ItemWeight::Carried` | `0x6001d70` / 12161 | f22677@`0x6f3c8b` / 817 | `3e842ffca02c51855a9366e4a2848e315dc4a72595dd04b25cb1d1953fd6d23c` |
| m21159 `<>c__DisplayClass2_0::<Install>g__WeightChanged\|0` | `0x6000e2b` / 49650 | f38669@`0xc0caf3` / 247 | `01470d418313039e391502133932d4c575d776c7e64b05aed991845bc25c9585` |

## Scratch fingerprints

| Outside-Git file | SHA-256 |
| --- | --- |
| `check.py` | `d000a88bf56a58c125e1ad4da7c4a0775e027e20e3359153a82976d7375312a9` |
| `reference.py` | `0ed6673fc812049a4dd3bd3be639247d6d4a410206e155d3c9370c83941b458a` |
| `instrument.py` | `800ba232cd4ad8352e77658b1087271de31ef5a0d3986f08685c4242c6541842` |
| `instrumentation.json` | `89d66871bf6b3bb4dfe184380ddcdde696cb0241ff16220a39c638edf127a34c` |
| `instrumented.wasm` | `1cc9f88f2d091797a791041ff14fcacd16a5a7881e4fd3cf08a93ba765f24016` |
| `future-probe.js` | `c39c406af3e86c67fee9d6f8ce32e64047bff3cffa066fd37ac3ddc9c596c378` |
| `test-probe.cjs` | `f9385edd1767ec3866cd84c4e33d88041ebd9e5a8e48ddf023938217c48cb2e4` |
| `compare.py` | `94735adfb6153290e8dad591189e9e546a604617c0aa8f268523e6655a4852d8` |
| `test-compare.py` | `d156bb9adbff2892932f3f71bdf59b28323a22a998878d48300967b7495a60f3` |

## Primary and supporting sources

[W]: https://tr-play.playall.in.th/Build/d9a4c4fd2737e740b82e1321839e352f.wasm.gz
[D]: https://tr-play.playall.in.th/Build/6135c4139b62e558bb4c755b266f5fa2.data.gz
[R17]: https://github.com/oDestroyeRo/opentrickster/blob/df5fe0a5e8e45072845538fc74967a1757b6d9d7/docs/research/current-build-protocol-revalidation.md
[R14]: https://github.com/oDestroyeRo/opentrickster/blob/cb587e235b113aa26b04db018c31f1e26510f5bd/docs/research/target-state-completion.md
[R27]: https://github.com/oDestroyeRo/opentrickster/blob/07bd3daf8f6dbbd6bcef3730269d0e6b2d7412c1/docs/research/target-game-definitions.md
[R28]: https://github.com/oDestroyeRo/opentrickster/blob/6f93a75a23849475c7f88d5f3b099d98a4a6c33e/docs/research/assisted-input-control.md
[R29]: https://github.com/oDestroyeRo/opentrickster/blob/efccf7992b77966804e9078d4643a565aa3b6340/docs/research/gun-combat-control.md
