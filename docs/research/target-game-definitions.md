# Public game definitions for native automation

Issue [#27](https://github.com/oDestroyeRo/opentrickster/issues/27). Static public-source research, **2026-11-08 UTC**, build **8502ebbf2fc9033d2fba50d02fd92002**, data version **KIcgbcTZu7eh**.

The actual catalog is decoded: **18,118 item definitions, 191 drill rows, 613 gun rows / 611 distinct gun IDs, 696 zones, 243 potion candidates, 1,467 monster parameter rows, and 3,355 character definitions**. The supplied executable produces the private-to-net `GameDefinitions` inputs, labels, resource tables and complete item records. It verifies the item index's runtime/read-set/overlay contract against the current public libconfig and embedded overlays. These are downloaded rows, not fixtures. No production code or game assets are added to Git.

Resource readiness has a concrete received-state path: **S282 supplies indexed I32 total stats**; total indices **9, 3, 6** are maximum HP, MP, and weight. Alternatively, the recovered public level table plus complete character contributions produces those totals. Missing state remains unknown. Server acceptance, account state, gameplay and end-to-end automation were not exercised.

## Evidence and delivery boundary

Only this report is writable in the checkout. Base: `5e419ff89629b5fc3e45b730b43a247bd7f4163e`; branch: `research/target-game-definitions`. The original checkout was clean. AGENTS.md, tracker/domain pointers, MVP/architecture documents and the research skill were read. The investigation stayed sequential under the explicit no-subagent instruction.

The [current configuration](https://tr-play.playall.in.th/spinel-config.json), [version](https://tr-play.playall.in.th/version), [page](https://tr-play.playall.in.th/) and data-server descriptor were re-fetched. Config SHA-256 is `084900013e6edde0ce0eca286dc7ac884a86da2ece1bae361ce0671852092602`; version SHA-256 is `9ce11c71ef039ffbe6bede54dfcf5b27afb887d44b2a79f57f959638e2e771b0`. Both report the build above. Only the three named cache files were opened in the mixed scratch directory; their `.gz` suffixes currently contain decoded bytes. The resolver tests magic before decompression.

| Public artifact | Decoded size | SHA-256 |
| --- | ---: | --- |
| [WASM][W] | 43136343 | `381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404` |
| [Unity data][D] | 27724487 | `fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474` |
| Metadata inside data, offset 16748911 | 9343896 | `7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade` |

`m` below is a zero-based metadata method index; `f` is a WASM function index; `@` is an **original decoded WASM file offset**. Object field offsets are not wire offsets. Metadata is format 108, method rows are 30 bytes (name U32@0, type U16@4, token U32@18), type rows 66 bytes, field rows 10 bytes, and imported functions 490. Module method-pointer arrays and the element section join metadata tokens to functions. `method-proof.json` records the original body size/hash and owning metadata rows. WABT 1.0.39 was used only for static disassembly/decompilation. Its decompiler omits exception-region bodies; those were inspected in disassembly. No target code was instantiated or executed.

The earlier [protocol](https://github.com/oDestroyeRo/opentrickster/blob/df5fe0a5e8e45072845538fc74967a1757b6d9d7/docs/research/current-build-protocol-revalidation.md), [incoming state](https://github.com/oDestroyeRo/opentrickster/blob/cb587e235b113aa26b04db018c31f1e26510f5bd/docs/research/target-state-completion.md), [manual digging](https://github.com/oDestroyeRo/opentrickster/blob/b1a12a97420fff84f15cc1e3cf8b5070560ed7b9/docs/research/manual-dig-control.md), and [gun](https://github.com/oDestroyeRo/opentrickster/blob/efccf7992b77966804e9078d4643a565aa3b6340/docs/research/gun-combat-control.md) reports were leads. Their named local report bytes match those immutable commits; the catalog and formulas below were checked against the current primary assets.

## Public loader and exact fetch contract

`Boot` f36253@`b0a823` chooses `libconfig_thai` at `b0ab40`. `DataSources.CreateDefault` f32577@`a26778` loads Resources `SpinelDataSettings`, uses a nonblank `webDataUrl +16`, otherwise resolves `pack` relative to `Application.absoluteURL`. The current Unity ResourceManager has no `spineldatasettings` key. Its pinned public data therefore selects **https://tr-play.playall.in.th/pack/**. This is verified asset configuration, not a guessed CDN root. `GameAssets.ItemInfos` async body f30813@`984755` asks `ItemIndex.PathFor` f22784@`700776` for `_spinel/item-index-libconfig_thai.bin` and invokes parser f22801@`701a55`. [W][D]

The [public descriptor](https://tr-play.playall.in.th/pack/_spinel/data-server.json) is exactly `{"rangeQuery":true,"encrypted":true,"dataVersion":"KIcgbcTZu7eh"}`. The [public data-key endpoint](https://tr-play.playall.in.th/pack/_spinel/data-key) is fetched without account state; its response is 73 bytes, SHA-256 `6c06ccb0e9f57269b34a1fecb68566e6ed4754ae56e5d190a14b5358b623b19e`. Its key is used only in local memory by the embedded resolver and is not printed or embedded here.

The current methods are `ProbeInfo` async f32532@`a21bc4`, `DataCipher.NameAsync` async f32483@`a1edc4`, `FetchDataKey` async f32524@`a2100b`, `Take` async f32560@`a252e6`, and `ReadRangeAsync` f32548@`a2308d`. Some investigation leads used different async-function names/numbers; these are the checked metadata joins. The JavaScript imports are in the [current framework][J].

For normalized logical name `p`, the framework computes `nonce = HMAC-SHA256(K, UTF8("spinel-data-name\0" + p))[0:16]`, encrypts `UTF8(p)` with AES-256-CTR using that nonce, and forms unpadded base64url of `nonce || ciphertext`. The served path is `<token>.bin?v=<dataVersion>`. Response headers `X-Spinel-Enc: aes-256-ctr`, `X-Spinel-IV`, and `X-Spinel-Coding: gzip` select AES-CTR decryption followed by gzip. These are two layers: HTTP asset decryption exposes the libconfig container; XTEA subsequently exposes its XML tables. The resolver fetches complete assets, so it does not depend on an unverified range-query syntax. `rangeQuery=true` is preserved as descriptor evidence. [J; W f32524/32560/32548]

`DataPath.Normalize` f65143@`16cf116` changes backslashes to slashes, skips an initial dot and leading slashes, and removes an initial case-insensitive `data/`. The six logical names below are already normalized. No general lowercasing is applied.

| Logical name / exact public URL | Decoded bytes | Decoded SHA-256 |
| --- | ---: | --- |
| [_spinel/item-index-libconfig_thai.bin](https://tr-play.playall.in.th/pack/z5MXF5gFK7tvq6yx2NXFsJPP4xMbfaAkU5zK6I5g4h9jKI4Q7WXIsPyNkId24TDpQiSVyXs.bin?v=KIcgbcTZu7eh) | 6254728 | `1f33cb29efac45471873dc5c6d05320cd5d8a14e03202d91fa1b46ac566c3993` |
| [libconfig_thai.idx](https://tr-play.playall.in.th/pack/pu1jlKpNTi4ruj-jAV7EM5O8i6scDtCjhNx4SxOYclZCdA.bin?v=KIcgbcTZu7eh) | 259683 | `4784253c40e33ca76e6f98430cfa32487e0eb9229a55dc3226af48ed9975d89b` |
| [libconfig_thai.dat](https://tr-play.playall.in.th/pack/2Io1nClL7ktIZXXF9hl2Gfvs8LLBrOaWDzABZt7KDfSpng.bin?v=KIcgbcTZu7eh) | 155695673 | `ab16141a6491bae383f20f56ddbdc88ab0aca317904a33a2a58d8e3b9fa88223` |
| [gulim13.fnt2](https://tr-play.playall.in.th/pack/GUF1apBkOn0UaXeCyckiyYKzlsDfXOn07BPceQ.bin?v=KIcgbcTZu7eh) | 567604 | `a97da1418b88f0920a1f868e735e728fc032070cca422ab9f6f2efb0440f1b3c` |
| [gulim14.fnt2](https://tr-play.playall.in.th/pack/ROsUPWWkcT-Jtc6LuI46Zo9diTI99KbTNMF1gQ.bin?v=KIcgbcTZu7eh) | 12789 | `7d9fa9aa233cab8ec3db58d5e21b0dad3697db1b8bcb0859fe9ab9d169edf468` |
| [gulim23.fnt2](https://tr-play.playall.in.th/pack/HQ6aP4UKXb5LKpaj-TEvEoGh5pkK3_VT0u7t1Q.bin?v=KIcgbcTZu7eh) | 124744 | `0f5ba9f94371e087a7364a31c34366aa4a6f50530e31b360750a1b85c08d14e9` |

## Index, tables, and compiled configuration

The libconfig index consists of UTF-8 NUL-terminated names followed by U32LE offsets: **10,419 ordered entries, 10,251 distinct names**. Preserve order and duplicates. A table span starts at its offset and ends at the next entry. Libconfig layout byte 0 is 1; the runtime stamp's last end is **dat size minus1**, because `Open` subtracts its layout offset before `ParseIndex` f9625@`2cb8a2`. Hashing the last end as the complete dat length fails the current stamp. [W f30753@`97c867`, f9625]

Compiled XTEA key extraction is public-artifact decoding. For current layout 1, `Libconfig..cctor` f9648@`2ce8d5` references usage cell **0x342df8**, metadata field **79074**, data bytes **metadata[0x4ed2a8:0x4ed2b8]**, SHA-256 **b5b8c3c831b51613bc8701615db8ec52cda99e4610d0f452a9c984619ba9dea0**. The alternate key is cell 0x342dec, field 79071, metadata 0x4ed250, SHA-256 `3436b57e4ab900cd2b7e93025760f34266bc093b147a6bc014690af24d0ed96f`. Neither key's bytes are published. The resolver accepts only the pinned current layout and extracts the current field locally.

For a full field-reference join: usage kind 6 gives fieldRef index `(value & 0x1fffffff)>>1`; its metadata section 23 row is `(runtimeType U16, ordinal U32)`. The type pointer array at WASM memory 0xd75a0 identifies the declaring type, whose fieldStart plus ordinal gives the field index. Find that field's default-data record in metadata section7 and add its data offset to section8. The full artifact hash guards the compact fixed locator in the reproducer.

`Xtea.DecryptInPlace` f9571@`2c83a6` uses 32 rounds, delta 0x9e3779b9, initial decryption sum 0xc6ef3720, four little-endian U32 key words and big-endian U32 block words. `TableBytesInPlace` f9629@`2cbef0` removes NUL bytes and clips at `</TABLE>`. Decoded XML uses UTF-8. Required columns and row counts are checked; no absent cell becomes a fixture zero.

| Actual table | Raw dat span, end exclusive | Rows | Decoded XML SHA-256 |
| --- | --- | ---: | --- |
| Item_DrillParam | 2856617..2985689 | 191 | `f73ce98fb594e8189b94fb97d3353ac6dc4ab7bd94c16d803a0cdec8c84a7bbd` |
| Item_GunParam2 | 48100529..48258073 | 613 | `c897abd5de36dd24c873009efd79710760eef7206eeff9008613dabf9d078af8` |
| MapInfoEx | 24824097..25766385 | 696 | `f2d89e609d624ee0e367987ee9719c07ab358e0f232c00096dea6223a0c70820` |
| MapAttr | 6111593..6115841 | 17 | `87dbf35863085f0a43f9d36460f985dc4c2783efcc06547b5bd799b1c6c6eaba` |
| CharacterInfo | 12897..2398177 | 3355 | `95ee8ab045e3cdc110032ba68adf2c55ea77625c497aec38b746d9251d375713` |
| MonsterParamEx2 | 102053881..105438345 | 1467 | `fbea79e5299fc5d34e0725dec32d0c2e7cf54c5064bcd8bf34771901a49b8da5` |

**IIDX format.** Bytes 0..7 are `IIDX` and U32LE version 1; seven U64LE fields at8 are Content, Overlay, Runtime, DatSize, DatTicks, IdxSize, IdxTicks. ReadCount U32@64 precedes U64 read hashes. The current four hashes correspond exactly to `ItemParamCM2`, `ItemParamEx2`, `ItemParamEx`, `ItemParam2`. The body begins at 100. It contains a string table and 18,118 complete rows, ending exactly at byte 6254728. `Var` is Zigzag ULEB128 I64; `Int` is Var constrained to I32; strings have ordinary unsigned 7-bit byte lengths and UTF-8 bytes; references are Int, zero=null and positive=nth string. `Number` is Var: even means arithmetic shift-right1; exactly1 means a following F64LE; other odd values reject. The embedded decoder specifies the complete field order. [W f22801, f22787@`7009d9`, f21001@`67043f`, f21002@`67063a`]

**Stamp checks.** The custom hash starts at 14695981039346656037. Adding U64 `v` computes `x=((h XOR v)*1099511628211) mod 2^64`, then `h=x XOR (x>>29)`. Adding a string adds its UTF-8 byte length, each complete LE U64 chunk, then each remaining byte. Runtime hashes version 1, ordered entry count, and every name/start/end. The independently computed **4815233531925194354** matches. Relevant overlay hash is **12638152005296870472**, the hash after adding version 1 alone. `Refusal` f21000@`670397` checks Runtime then Overlay; it does **not** establish content integrity from Content or file timestamps. The resolver separately pins SHA-256 of each asset and checks source sizes. Content is 3666684648237282665; it is retained as stamp metadata, not falsely reported as recomputed. [W f20997@`670189`, f20996@`66f628`, f21000]

The public `data.unity3d` at data offset 270, length 16709784 contains Thai and English TextAssets with 127 tables each. Thai SHA-256 `ce9c8f53be329c6097212a451dc241bffe8e2a2230abbc8154bb1eed2e0d6758`, English `8fc4019b037f01107035b65c410800606a7954004daf1272a2b387f3d6378ac2`. Neither overrides any of the four IIDX source tables or six decoded tables above. `LibconfigOverlay.Apply` f36210@`b05cc0`, `Tables` f36211@`b05fc5`, and `InstallSpinel` f36212@`b062c5` own installation. New relevant overlays cause the reproducer to reject instead of silently treating them as empty. [D][W]

## Private-to-net definition contract

`game_definitions.json` contains `item_types: ItemId -> [Type,SubType]`, `drills: ItemId -> {area,speed,mass,limit,nature}`, `gun_slots: [4,9,1,22]`, `zones: zoneId -> [Type,SubType]`, and **`movement_mode: null`**, whose runtime/input origin belongs to #28. Extensions are `labels`, `zone_labels`, `drill_meta`, `gun_params`, `potions`, `normal_form_effects`, `level_tables`, `monster_params`, `character_info`, and `map_materials`. JSON object keys are decimal strings. `items.json` preserves every decoded ItemInfo field; use it for weights, effective flags and metadata. These stay in the net boundary rather than becoming engine wire types.

| Consumer | Actual source and conversion |
| --- | --- |
| Item type and label | ItemInfo `Id +8`, `Name +16`, `Type +24`, `SubType +28`; parser f22801. IIDX IDs are I64; all current IDs fit the adapter's U32 key. |
| Ammo | `FlagsEx +68`, low-byte mask 64; `Type29`. Current loaded inventory UID/count is received state. SubType54 detects guns; Wear additionally requires Type2. See #29 for flow. |
| Gun slots | f22142@`6cb0be`; metadata field 53533 at 0x4d16d0, 16 bytes, SHA-256 `f79067ab1dc0e54d3c4b8d6c007d3a06f9ba33296ce8d0e6e038b335115bf800`. |
| Drill parameters | `Item_DrillParam.Id` I64; Grade+16, GroundNature+20, DigDepth+24, BarEffArea+28, BarSpeed+32, ScoopMass+36, DrillLife+40. f21341@`689d3c`, Long f21343@`68b0bc`. |
| Drill narrowing | `JsNumber`, finite value truncated toward zero and saturated to I64; GroundNature stores low32, the other six listed parameters narrow to **signed low16**, held in I32 fields. Missing/nonfinite real inputs are rejected by this resolver rather than adopting the source helper's zero fallback. |
| Gun parameters | f22073@`6c6b81`, Long f22074@`6c6fa0`: `ID` I64; `GunType,GaugeSpeed,PowRange,PowStartPoint,MinAttackRange,MaxAttackRange` store low32 after the same finite/truncate/saturate conversion. GunParam offsets8,16,20,24,28,32,36. **First duplicate ID wins.** |
| Zone classification | **MapInfoEx**, not MapAttr. f20234@`624355` loads MapInfoEx and f20228@`623350` builds ZoneInfo: Id+8, Name+12, Type+24, SubType+44. All current zone IDs fit U16. MapAttr's 17 rows describe ground material Type/Attr/effects/sound; they are emitted separately. |

Examples from actual rows: drill 8001 has `(area70,speed100,mass1,limit30,nature1,Grade1,DrillLife300)` and weight 50. Gun 3500 has `(0,85,180,150,0,250)` in the six-column order above. Zone 17 is Type3/SubType0, “Megalopolis Forest 2 - Southwest.” Item 2001 is “Pink Potion B,” Hp200, Mp0, both recovery-rate fields 0, weight 5. These examples are checks, never fallback definitions.

The gun duplicates matter: ID3580's first row has GaugeSpeed120, PowRange180, PowStartPoint150, MaxAttackRange245; its later row has 130/200/140/275. ID52001 repeats identically. Resolving the last duplicate would change behavior. Three drill IDs 19907/19911/19912 and four gun IDs 510087/510088/510089/76458 have no current IIDX row; retain their parameter rows but require a successful actual ItemInfo join before use.

Equipped drill readiness uses **slot7 -> inventory UID -> ItemId -> Type6 -> drill parameters**. The received inventory **Count is current drill life**; `DrillLife` is maximum life. Four State bytes are not a life value. Configured gun detection scans 4/9/1/22; the actual GunParam comes specifically from **slot1**. Its Min/Max ranges are direct; MidRange is `((Min+Max)*3)>>2` with I32 arithmetic. Zone Type gates gun display; the owning gun report rejects 0/1/5. Manual digging rejects zone Type1/5/6 or SubType14, and needs the remaining terrain/spot/session inputs from its report. Catalog availability alone does not prove equipment or action readiness. [W f22105/f22109/f21341; pinned #29 and manual-dig reports]

MonsterParamEx2 includes real ID/Name/Type/Level/MaxHpLv, movement, aggression, resistance and boss fields. CharacterInfo supplies CommonName, files and attack/chase ranges for its own ID domain; `CharacterData.CharacterInfos` async f30665@`976072` owns that table. The output keeps both maps separate. `MonsterField` f17455@`54169f` and `MonsterMaxHp` f17375@`53804d` index the public monster level array at `((L-1)*12+s)*3+2` (s9 for HP). A table template key is not a live actor ID; current target identity, HP and validity still require the received actor state.

## Resources and potion use

`CombatData.Load` async f17466@`5422e9` fetches `gulim13.fnt2` (player stats), `gulim23.fnt2` (monster stats), and `gulim14.fnt2` (experience). Despite their suffixes these are whitespace-separated numeric data: 35,964 player values and 35,964 monster values = 999 levels × 12 stats × 3 columns; experience has 1,600 values. The complete arrays are in `level_tables`. [W f17459/f17461/f17466]

For player stat `s`, `L=floor(points[s]/4)+1`; `base = trunc_sat_i32(player[((L-1)*12+s)*3+2])`. L1..999 is supported by the source. `CharacterStatus.Total(s,base)` returns a present `Totals[s]` directly. Otherwise it sums `(recomputedBase[s] ?? base) + Buffs[s] + recomputedAura[s] + GameParts[s]`, with wrapping I32 addition. HP uses s9, MP s3, maxWeight s6. The implementation requires complete contribution vectors when no total exists; it does not infer zero equipment, aura, buffs or game parts from an absent packet. Equipment/set/imprint contributions are owned by `AvatarStats.Part` f18112@`568027`; they must be implemented or bypassed only by an actual authoritative total, not erased. [W f17457@`54186b`, f17456@`541779`, f17365@`53771c`, f17366@`5377d2`, f11484@`3428fa`]

**Direct totals:** CharacterStatus constructor f11469 registers **S282** with callback f11492@`343129`; binding cells/instructions are `3415e1..3415fb`. `Status.ParseIndexedValues` f11460@`34063a` reads `count U8`, then `count × (stat U8, value I32LE)`. Callback indices 0..11 populate nullable `Totals`, then notify. These totals can provide actual maxima without solving all equipment effects. The resolver's strict parser requires the full declared extent. **S283** callback f11491@`342df7` writes `Buffs`; its recompute clears totals. `Recompute` f11473@`342043`, points and equipment events likewise invalidate the applicable cached totals; do not carry stale received maxima across those changes.

Start233's twelve points are U16LE at 184..207; Level is U16@214. Current HP and MP are U32LE at 235 and 239, respectively, requiring at least 243 bytes. `ParseStartStats` f11455@`33fedf` calls those readers; missing-byte helper fallbacks must not become authoritative current values. The native transport must continue processing its HP/MP update contract and own-actor attribution. Maxima are independent from current values. A percentage is a policy calculation from known current/max values with a valid positive maximum, never a fabricated maximum or inferred heal amount. [W f11454/f11455/f11456; pinned protocol/state reports]

**Current weight:** `ItemWeight.Of` f22676@`6f3ba5` multiplies ItemInfo.Weight (I64,+80) by `InvItem.Pieces`. `Pieces` f11786@`3549fc` is Count only when **wire InvItem.Type ==1**, otherwise1. `Carried` f22677@`6f3c8b` sums across the inventory, with I64 arithmetic. Equipped entries remain part of the inventory enumeration. For example, three actual item 2001 units plus one drill 8001 weigh `3*5+50=65`, irrespective of that drill's life Count. An incomplete inventory or unknown item definition makes current weight unknown; the target helper's missing-info zero is not adopted.

**Potion metadata:** World.IsHpPotion f37798@`bb9014` requires `Flags &2` and `(Hp>0 || HpRecoveryRate>0)`; MP f37797@`bb8fd5` is analogous. No Type10-only heuristic is used. ItemInfo fields are Flags+64, FlagsEx+68, MinLevel+96, Hp+104, Mp+112, Delay+120, HpRecoveryRate+184, MpRecoveryRate+192, Effect+200; names and Use text are preserved. `ItemData.ItemFlags` f22691@`6f9413` can take ItemClass2's effective flags from inventory Extra key13. Thus static candidates are not a substitute for resolving actual item-instance flags.

`ItemsModel.CanUse` f20887@`664bb7` requires effective flags mask 2 and current Level >= **signed low16 MinLevel**. If its HasStatus dictionary exists and lookup 1 is false, effects **15,21,23,24,25,26** are refused; true bypasses that restriction. The array is `NormalFormEffects`, initialized by f20927@`669516`, metadata field 53446 at 0x4d09c8, 24 bytes, SHA-256 `65a96e5111fde179295e050214fd70276da0446bb6fec006bfc2bc87b759efc3`. Do not invert this boolean or describe it as a generic “blocked=true” flag.

`QuickSlots.UseItem` f25066@`7cd4c6` and `ItemsModel.Activate` f20890@`665045` also require the actual inventory instance, item availability, non-expiry, valid item-use/UI mode, a free avatar FSM, no trading/faint/before-use refusal, and the applicable effect/use handlers. If UsesReady is pending, non-potion-subtype use can wait through HoldUse; SubType10/11 take the direct path. The available item metadata does not justify a guessed Delay-based cooldown. `Inventory.RequestUse` f11994@`3608c5` sends logical **87 with U64 inventory UID**, not ItemId. Server feedback and inventory/current-resource updates determine success.

The examined potion consumers classify and display recovery fields; they do not prove a universal `max*rate/100 + Hp` restoration formula. `PotionEffectTable` is a one-row visual-effect table (item 2405), not recovery arithmetic. Full-resource checks in BoxesModel.PotionChecks f18322@`57ca70` warn for SubType10 HP, 11 MP, 12 both; they are not a universal local rejection. Exact server restoration/rounding, special effect semantics and dynamic expiry/use-handler state remain runtime prerequisites where applicable. An engine can choose a known eligible potion at a configured current/max threshold and await authoritative updates without inventing those formulas.

## Verification and remaining integration

The packaged resolver re-fetched all six assets and reproduced their SHA-256 values, parsed the entire IIDX with exact EOF, matched runtime and overlay stamps, decrypted and checked all six table XML hashes/counts, and generated the native definition maps. A separate checker passed **28 checks**, including actual row values, first-duplicate selection, truncation/trailing bytes, altered runtime/overlay/read-set stamps, reordered table index, newly relevant overlays, missing/nonfinite cells, signed narrowing, received-total precedence, missing resource/use baselines, stack-versus-drill-life weight, and truncated S282. Synthetic state checks are marked in the checker; none are runtime defaults.

The adapter still needs received equipment/inventory and current HP/MP; valid totals or complete stat contributions; item-use/expiry and active effects; own actor and zone; and the manual-dig and gun runtime prerequisites. For guns that includes actual key, loaded UID/count, learned 3010/3011 state and active fast-load F32 state from #29. #28 owns movement_mode. No account activity or target runtime was used to claim these inputs exist. The source-bound catalog task is complete; broader native automation and integration tickets remain separate.

Practical files are outside Git at **`/private/tmp/opentrickster-definitions.z72itC/`**:

- `catalog_resolver.py`: standalone actual fetch/decode/reference, with resource/weight/S282/use-metadata helpers.
- `catalog_check.py`, `checks.json`: independent acceptance/rejection checks and results.
- `game_definitions.json`: 18,118 item types/labels plus the maps and extensions above.
- `items.json`: full actual ItemInfo records, including flags, weight and StatPlus.
- `catalog-proof.json`, `method-proof.json`, `urls.json`: hashes, source joins, table spans and exact URLs.
- `artifact.py`, `view.py`, selected `f-*.dis`: original-offset static inspection support. No private/native probe files were read.

These local generated data files are implementation inputs, not repository assets. Only this report is published. Material contracts and the practical output paths were sent to the active #21 implementer as soon as verified.

Output SHA-256: `game_definitions.json` = `4a7c86ea510f3c2789f07c59715a0f00aae6fe10f93ddc898601b02399a27cc3`; `items.json` = `50d82abdb872c23e870207b498336bdcd3f58ac59542f9853d311243da3ce654`. A clean second output directory reproduced both byte-for-byte and passed the same 28 checks.

## Reproducer

Save the following Python block as `catalog_resolver.py` **outside Git**. Use Python 3.9+, `UnityPy==1.25.4`, `pycryptodome==3.23.0`, and curl. Do not run Python with `-O`. `--assets` must contain only the three named pinned inputs (or point to their approved directory; the code opens only those filenames). `--out` is an outside-Git output directory. The script rejects a changed build/data version, changed pin, or relevant overlay instead of quietly continuing with stale definitions.

```sh
python3 -m venv /tmp/catalog-venv
/tmp/catalog-venv/bin/pip install UnityPy==1.25.4 pycryptodome==3.23.0
/tmp/catalog-venv/bin/python /tmp/catalog_resolver.py \
  --assets /path/to/pinned-public-files --out /tmp/catalog-output --fetch
# A subsequent offline verification uses the same command without --fetch.
```

```python
import pathlib,struct,json,hashlib,xml.etree.ElementTree as ET,math
ROOT=pathlib.Path(__file__).parent
class Reader:
 def __init__(self,b,p=0):self.b=b;self.p=p
 def raw(self,n):
  assert n>=0 and self.p+n<=len(self.b),'truncated'
  v=self.b[self.p:self.p+n];self.p+=n;return v
 def u(self):
  v=0
  for s in range(0,70,7):
   c=self.raw(1)[0];v|=(c&127)<<s
   if c<128:
    assert v<2**64,'varint overflow';return v
  raise ValueError('varint too long')
 def v(self):
  u=self.u();return (u>>1)^-(u&1)
 def i(self):
  v=self.v();assert -2**31<=v<2**31;return v
 def number(self):
  v=self.v()
  if v%2==0:return v//2
  assert v==1,'invalid number tag';return struct.unpack('<d',self.raw(8))[0]
 def string(self):return self.raw(self.u()).decode('utf8')
def items(b):
 assert b[:8]==b'IIDX\x01\0\0\0','index format/version'
 n=struct.unpack_from('<i',b,64)[0];assert 0<=n<100000
 r=Reader(b,68+n*8);strings=[None]+[r.string() for _ in range(r.i())];count=r.i();out={}
 for _ in range(count):
  row={'Id':r.v()}
  for k in 'Name Use Comment IconFile GroundFile PictureFile EquipFile PartFile'.split():
   i=r.i();assert 0<=i<len(strings);row[k]=strings[i]
  for k in 'ItemClass Type SubType IconBundle Bundle PictureBundle Flags FlagsEx BodyFlags'.split():row[k]=r.i()
  for k in 'Weight Price'.split():row[k]=r.v()
  row['MinLevel']=r.i()
  for k in ['Hp','Mp']:row[k]=r.v()
  for k in 'Money RefineIndex RefineType MinStatType CompoundSlot ReformCount ExistType Effect Life FType MpCon CardNum GroupId'.split():row[k]=r.i()
  for k in 'Ncash PivotId HideHat'.split():row[k]=r.v()
  for k in 'Delay Ap MinStatLv HpRecoveryRate MpRecoveryRate'.split():row[k]=r.number()
  length=r.i();assert -1<=length<1000
  row['StatPlus']=None if length<0 else [r.number() for _ in range(length)]
  assert row['Id'] not in out;out[row['Id']]=row
 assert r.p==len(b),(r.p,len(b))
 return out
import argparse,gzip,hmac,base64,subprocess,urllib.parse,os
from Crypto.Cipher import AES
MASK=(1<<64)-1
BUILD='8502ebbf2fc9033d2fba50d02fd92002'
ORIGIN='https://tr-play.playall.in.th/'
PINS={
 'd9a4c4fd2737e740b82e1321839e352f.wasm.gz':'381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404',
 '6135c4139b62e558bb4c755b266f5fa2.data.gz':'fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474',
 'current-global-metadata.dat':'7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade'}
PUBLIC={
 '_spinel/item-index-libconfig_thai.bin':'1f33cb29efac45471873dc5c6d05320cd5d8a14e03202d91fa1b46ac566c3993',
 'libconfig_thai.idx':'4784253c40e33ca76e6f98430cfa32487e0eb9229a55dc3226af48ed9975d89b',
 'libconfig_thai.dat':'ab16141a6491bae383f20f56ddbdc88ab0aca317904a33a2a58d8e3b9fa88223',
 'gulim13.fnt2':'a97da1418b88f0920a1f868e735e728fc032070cca422ab9f6f2efb0440f1b3c',
 'gulim14.fnt2':'7d9fa9aa233cab8ec3db58d5e21b0dad3697db1b8bcb0859fe9ab9d169edf468',
 'gulim23.fnt2':'0f5ba9f94371e087a7364a31c34366aa4a6f50530e31b360750a1b85c08d14e9'}
TABLES={
 'Item_DrillParam':(191,'f73ce98fb594e8189b94fb97d3353ac6dc4ab7bd94c16d803a0cdec8c84a7bbd'),
 'Item_GunParam2':(613,'c897abd5de36dd24c873009efd79710760eef7206eeff9008613dabf9d078af8'),
 'MapInfoEx':(696,'f2d89e609d624ee0e367987ee9719c07ab358e0f232c00096dea6223a0c70820'),
 'MapAttr':(17,'87dbf35863085f0a43f9d36460f985dc4c2783efcc06547b5bd799b1c6c6eaba'),
 'CharacterInfo':(3355,'95ee8ab045e3cdc110032ba68adf2c55ea77625c497aec38b746d9251d375713'),
 'MonsterParamEx2':(1467,'fbea79e5299fc5d34e0725dec32d0c2e7cf54c5064bcd8bf34771901a49b8da5')}
def check(ok,why):
 if not ok:raise ValueError(why)
def sha(b):return hashlib.sha256(b).hexdigest()
def pinned(b,h):check(sha(b)==h,'artifact fingerprint mismatch');return b
def signed(v,bits):return ((v+(1<<(bits-1)))%(1<<bits))-(1<<(bits-1))
def long_cell(row,key):
 check(key in row and row[key].strip()!='','missing numeric column '+key)
 v=float(row[key]);check(math.isfinite(v),'unusable numeric column '+key)
 return max(-(1<<63),min((1<<63)-1,math.trunc(v)))
class Hash:
 def __init__(self):self.h=14695981039346656037
 def add(self,v):
  self.h=((self.h^(v&MASK))*1099511628211)&MASK;self.h^=self.h>>29;return self
 def string(self,s):
  b=s.encode('utf8');self.add(len(b));n=len(b)//8
  for i in range(n):self.add(struct.unpack_from('<Q',b,i*8)[0])
  for v in b[n*8:]:self.add(v)
  return self

def index_entries(b):
 p=0;out=[]
 while p<len(b):
  end=b.index(0,p);check(end+5<=len(b),'truncated table index')
  out.append((b[p:end].decode('utf8'),struct.unpack_from('<I',b,end+1)[0]));p=end+5
 check(all(out[i][1]<out[i+1][1] for i in range(len(out)-1)),'nonascending table spans')
 return out

def validate_stamp(iidx,entries,dat,idx,overlays):
 check(len(iidx)>=68 and iidx[:8]==b'IIDX\1\0\0\0','IIDX version')
 v,content,overlay,runtime,ds,dt,isz,it,n=struct.unpack_from('<I7QI',iidx,4)
 check(n<=10000 and 68+n*8<=len(iidx),'read-set bounds')
 reads=set(struct.unpack_from('<'+'Q'*n,iidx,68));check(len(reads)==n,'duplicate read hash')
 h=Hash().add(v).add(len(entries))
 for i,(name,start) in enumerate(entries):h.string(name).add(start).add(entries[i+1][1] if i+1<len(entries) else len(dat)-1)
 check(h.h==runtime,'runtime stamp mismatch')
 check(ds==len(dat) and isz==len(idx),'index source size mismatch')
 needed={'ItemParamCM2','ItemParamEx2','ItemParamEx','ItemParam2'}
 check(reads=={Hash().add(v).string(x).h for x in needed},'unexpected read-set')
 for root in overlays:
  names={t.attrib['name'] for t in root.findall('TABLE')}
  check(not names.intersection(needed|set(TABLES)),'relevant overlay needs merge')
 check(Hash().add(v).h==overlay,'overlay stamp mismatch')
 return {'content':content,'runtime':runtime,'overlay':overlay,'read_hashes':sorted(reads),'table_spans':len(entries)}

def xtea(cipher,key):
 check(len(cipher)%8==0,'unaligned XTEA table');k=struct.unpack('<4I',key);out=bytearray()
 for v0,v1 in struct.iter_unpack('>2I',cipher):
  sm=0xc6ef3720
  for _ in range(32):
   v1=(v1-((((v0<<4)^(v0>>5))+v0)^((sm+k[(sm>>11)&3])&0xffffffff)))&0xffffffff
   sm=(sm-0x9e3779b9)&0xffffffff
   v0=(v0-((((v1<<4)^(v1>>5))+v1)^((sm+k[sm&3])&0xffffffff)))&0xffffffff
  out.extend(struct.pack('>2I',v0,v1))
 return bytes(out)

def table_rows(dat,entries,key,name):
 i=next(i for i,e in enumerate(entries) if e[0]==name);start=entries[i][1];end=entries[i+1][1]
 raw=xtea(dat[start:end],key).replace(b'\0',b'');stop=raw.index(b'</TABLE>')+8;raw=raw[:stop]
 pinned(raw,TABLES[name][1]);root=ET.fromstring(raw)
 check(root.attrib['name']==name,'wrong table name')
 rows=[{c.tag:''.join(c.itertext()) for c in row} for row in root.findall('ROW')]
 check(len(rows)==int(root.attrib['RowCount'])==TABLES[name][0],'wrong table row count')
 return rows,{'start':start,'end':end,'sha256':sha(raw),'rows':len(rows)}

def curl(url):return subprocess.check_output(['curl','-q','--fail','--silent','--show-error','--max-time','60',url])
def aes_ctr(b,key,iv):return AES.new(key,AES.MODE_CTR,nonce=b'',initial_value=int.from_bytes(iv,'big')).decrypt(b)
def fetch(out):
 page=curl(ORIGIN)
 check(json.loads(curl(ORIGIN+'version'))['build']==BUILD,'build version changed')
 config=json.loads(curl(ORIGIN+'spinel-config.json'));check(config['build']==BUILD and config['dataVersion']=='KIcgbcTZu7eh','configuration changed')
 for n in list(PINS)[:2]:check(n.encode() in page,'website changed build assets')
 info=json.loads(curl(ORIGIN+'pack/_spinel/data-server.json'))
 check(info=={'rangeQuery':True,'encrypted':True,'dataVersion':'KIcgbcTZu7eh'},'data deployment changed')
 keydoc=curl(ORIGIN+'pack/_spinel/data-key')
 pinned(keydoc,'6c06ccb0e9f57269b34a1fecb68566e6ed4754ae56e5d190a14b5358b623b19e')
 key=base64.b64decode(json.loads(keydoc)['key'],validate=True);check(len(key)==32,'AES key width')
 manifest={}
 for logical,h in PUBLIC.items():
  nonce=hmac.new(key,b'spinel-data-name\0'+logical.encode(),hashlib.sha256).digest()[:16]
  token=base64.urlsafe_b64encode(nonce+aes_ctr(logical.encode(),key,nonce)).decode().rstrip('=')
  url=ORIGIN+'pack/'+token+'.bin?v='+info['dataVersion'];stem=out/logical.replace('/','_')
  body=subprocess.check_output(['curl','-q','--fail','--silent','--show-error','--max-time','60','-D',str(stem)+'.headers',url])
  headers={s.split(':',1)[0].lower():s.split(':',1)[1].strip() for s in pathlib.Path(str(stem)+'.headers').read_text().splitlines() if ':' in s}
  check(headers.get('x-spinel-enc')=='aes-256-ctr' and headers.get('x-spinel-coding')=='gzip','response encoding changed')
  decoded=gzip.decompress(aes_ctr(body,key,base64.b64decode(headers['x-spinel-iv'],validate=True)));pinned(decoded,h)
  pathlib.Path(str(stem)+'.decoded').write_bytes(decoded)
  manifest[logical]={'url':url,'encoded_sha256':sha(body),'decoded_sha256':h,'bytes':len(decoded)}
 (out/'urls.json').write_text(json.dumps(manifest,indent=2)+'\n')

def resolve(assets,out):
 artifact=[]
 for n,h in PINS.items():
  b=(assets/n).read_bytes();b=gzip.decompress(b) if b[:2]==b'\x1f\x8b' else b;artifact.append(pinned(b,h))
 wasm,data,meta=artifact;check(data[16748911:16748911+len(meta)]==meta,'metadata slice mismatch')
 # Public compiled fields: extract only after exact artifact hashes pass; never print keys.
 key=pinned(meta[0x4ed2a8:0x4ed2b8],'b5b8c3c831b51613bc8701615db8ec52cda99e4610d0f452a9c984619ba9dea0')
 normal_effects=pinned(meta[0x4d09c8:0x4d09e0],'65a96e5111fde179295e050214fd70276da0446bb6fec006bfc2bc87b759efc3')
 slots=pinned(meta[0x4d16d0:0x4d16e0],'f79067ab1dc0e54d3c4b8d6c007d3a06f9ba33296ce8d0e6e038b335115bf800')
 import UnityPy
 env=UnityPy.load(data[270:16710054]);overlays=[];resource_keys=[]
 overlay_pins={'libconfig_thai':'ce9c8f53be329c6097212a451dc241bffe8e2a2230abbc8154bb1eed2e0d6758','libconfig_eng':'8fc4019b037f01107035b65c410800606a7954004daf1272a2b387f3d6378ac2'}
 for o in env.objects:
  if o.type.name=='ResourceManager':resource_keys.extend(k for k,v in o.read_typetree()['m_Container'])
  if o.type.name=='TextAsset':
   t=o.read()
   if t.m_Name in overlay_pins:
    b=t.m_Script.encode('utf8','surrogateescape');pinned(b,overlay_pins[t.m_Name]);overlays.append(ET.fromstring(b));(out/('unity-'+t.m_Name+'.txt')).write_bytes(b)
 check(len(overlays)==2 and len(resource_keys)>0,'public overlay/resource assets missing')
 check('spineldatasettings' not in [k.lower() for k in resource_keys],'webDataUrl override requires inspection')
 public={n:pinned((out/(n.replace('/','_')+'.decoded')).read_bytes(),h) for n,h in PUBLIC.items()}
 idx=public['libconfig_thai.idx'];dat=public['libconfig_thai.dat'];check(dat[0]==1,'unknown libconfig key selection')
 entries=index_entries(idx);stamp=validate_stamp(public['_spinel/item-index-libconfig_thai.bin'],entries,dat,idx,overlays)
 infos=items(public['_spinel/item-index-libconfig_thai.bin']);check(len(infos)==18118,'wrong item count')
 tables={};proof={}
 for name in TABLES:tables[name],proof[name]=table_rows(dat,entries,key,name)
 def keyed(name,idcol='ID'):
  d={}
  for row in tables[name]:d.setdefault(long_cell(row,idcol),row)
  return d
 drills={};drill_meta={}
 for i,r in keyed('Item_DrillParam','Id').items():
  drills[i]={k:signed(long_cell(r,c),32 if k=='nature' else 16) for k,c in [('area','BarEffArea'),('speed','BarSpeed'),('mass','ScoopMass'),('limit','DigDepth'),('nature','GroundNature')]}
  drill_meta[i]={k:signed(long_cell(r,k),16) for k in ['Grade','DrillLife']}
 guns={i:{k:signed(long_cell(r,k),32) for k in ['GunType','GaugeSpeed','PowRange','PowStartPoint','MinAttackRange','MaxAttackRange']} for i,r in keyed('Item_GunParam2').items()}
 zones=keyed('MapInfoEx');check(all(0<=i<65536 for i in zones),'zone ID outside u16')
 check(all(0<=i<2**32 for i in infos),'item ID outside u32')
 levels={}
 for kind,n in [('player','gulim13.fnt2'),('monster','gulim23.fnt2'),('experience','gulim14.fnt2')]:
  values=[float(s) for s in public[n].decode('utf8').split()];check(all(math.isfinite(x) for x in values),'nonfinite level table')
  check(len(values)==(1600 if kind=='experience' else 35964),'level table extent');levels[kind]=values
 potions={i:{k:r[k] for k in ['Name','ItemClass','Type','SubType','Flags','FlagsEx','MinLevel','Hp','Mp','HpRecoveryRate','MpRecoveryRate','Delay','Effect','Use']} for i,r in infos.items() if (r['Flags']&2) and (r['Hp']>0 or r['Mp']>0 or r['HpRecoveryRate']>0 or r['MpRecoveryRate']>0)}
 result={'build':BUILD,'item_types':{i:[r['Type'],r['SubType']] for i,r in infos.items()},'labels':{i:r['Name'] for i,r in infos.items()},'drills':drills,'drill_meta':drill_meta,'gun_slots':list(struct.unpack('<4i',slots)),'gun_params':guns,'zones':{i:[signed(long_cell(r,'Type'),32),signed(long_cell(r,'SubType'),32)] for i,r in zones.items()},'zone_labels':{i:r['Name'] for i,r in zones.items()},'movement_mode':None,'normal_form_effects':list(struct.unpack('<6i',normal_effects)),'potions':potions,'level_tables':levels,'monster_params':keyed('MonsterParamEx2'),'character_info':keyed('CharacterInfo'),'map_materials':tables['MapAttr']}
 def write(n,v):(out/n).write_text(json.dumps(v,ensure_ascii=False,allow_nan=False,separators=(',',':'))+'\n')
 write('items.json',infos);write('game_definitions.json',result)
 write('catalog-proof.json',{'build':BUILD,'artifact_sha256':PINS,'public_sha256':PUBLIC,'stamps':stamp,'tables':proof,'overlays':overlay_pins,'resource_keys_count':len(resource_keys),'counts':{k:len(result[k]) for k in ['item_types','drills','gun_params','zones','potions','monster_params','character_info']},'output_sha256':{n:sha((out/n).read_bytes()) for n in ['game_definitions.json','items.json']}})
 return result

def resource_max(defs,stat,points,totals,base_overrides,buffs,aura,game_parts):
 check(stat in (3,6,9),'resource stat index')
 check(len(totals)==12,'total validity vector')
 if totals[stat] is not None:return signed(totals[stat],32)
 check(all(x is not None and len(x)==12 for x in [points,base_overrides,buffs,aura,game_parts]),'incomplete stat baseline')
 level=math.floor(points[stat]/4);check(0<=level<=998,'unsupported stat level')
 b=defs['level_tables']['player'][(level*12+stat)*3+2]
 base=max(-2**31,min(2**31-1,math.trunc(b)))
 check(all(x[stat] is not None for x in [buffs,aura,game_parts]),'unknown stat contribution')
 return signed((base if base_overrides[stat] is None else base_overrides[stat])+buffs[stat]+aura[stat]+game_parts[stat],32)

def carried_weight(infos,inventory):
 total=0
 for item in inventory:
  check(str(item['ItemId']) in infos or item['ItemId'] in infos,'unknown carried item')
  row=infos.get(str(item['ItemId']),infos.get(item['ItemId']))
  pieces=item['Count'] if item['Type']==1 else 1
  total=signed(total+signed(row['Weight']*pieces,64),64)
 return total

def potion_metadata_allowed(info,effective_flags,level,has_status1,normal_effects):
 check(isinstance(level,int) and isinstance(has_status1,bool),'unknown use baseline')
 return bool(effective_flags&2) and level>=signed(info['MinLevel'],16) and (has_status1 or info['Effect'] not in normal_effects)

def indexed_totals(payload):
 check(len(payload)>=1 and len(payload)==1+payload[0]*5,'indexed totals packet extent')
 return {i:v for i,v in struct.iter_unpack('<Bi',payload[1:]) if i<=11}

if __name__=='__main__':
 check(__debug__,'run without Python -O: decoder assertions are required')
 ap=argparse.ArgumentParser();ap.add_argument('--assets',type=pathlib.Path,required=True);ap.add_argument('--out',type=pathlib.Path,required=True);ap.add_argument('--fetch',action='store_true');args=ap.parse_args()
 args.out.mkdir(parents=True,exist_ok=True)
 if args.fetch:fetch(args.out)
 result=resolve(args.assets,args.out)
 print(json.dumps({'status':'PASS','build':BUILD,'counts':{k:len(result[k]) for k in ['item_types','drills','gun_params','zones','potions','monster_params','character_info']},'gun_slots':result['gun_slots'],'output':str(args.out/'game_definitions.json')}))
```

The checker is also portable: save this next to the resolver and its output as `catalog_check.py`, then run it with the same virtual environment.

```python
import catalog_resolver as r,json,pathlib,struct,xml.etree.ElementTree as ET
P=pathlib.Path(__file__).parent;defs=json.loads((P/'game_definitions.json').read_text());infos=json.loads((P/'items.json').read_text());passed=[]
def ok(name,v):assert v,name;passed.append(name)
def rejects(name,f):
 try:f()
 except (ValueError,AssertionError,IndexError,struct.error,UnicodeError):passed.append(name);return
 raise AssertionError('accepted '+name)
b=(P/'_spinel_item-index-libconfig_thai.bin.decoded').read_bytes();idx=(P/'libconfig_thai.idx.decoded').read_bytes();dat=(P/'libconfig_thai.dat.decoded').read_bytes();entries=r.index_entries(idx);overlays=[ET.fromstring((P/('unity-libconfig_'+lang+'.txt')).read_bytes()) for lang in ['thai','eng']]
ok('actual counts',len(infos)==18118 and len(defs['drills'])==191 and len(defs['gun_params'])==611 and len(defs['zones'])==696)
ok('actual drill 8001',defs['drills']['8001']==dict(area=70,speed=100,mass=1,limit=30,nature=1) and defs['drill_meta']['8001']==dict(Grade=1,DrillLife=300))
ok('actual gun 3500',defs['gun_params']['3500']==dict(GunType=0,GaugeSpeed=85,PowRange=180,PowStartPoint=150,MinAttackRange=0,MaxAttackRange=250))
ok('first duplicate gun3580 wins',defs['gun_params']['3580']['GaugeSpeed']==120 and defs['gun_params']['3580']['MaxAttackRange']==245)
ok('actual zone17',defs['zones']['17']==[3,0])
ok('actual potion2001',defs['potions']['2001']['Hp']==200 and defs['potions']['2001']['HpRecoveryRate']==0)
ok('actual gun slots',defs['gun_slots']==[4,9,1,22])
rejects('IIDX truncation',lambda:r.items(b[:-1]));rejects('IIDX trailing byte',lambda:r.items(b+b'\0'))
bad=bytearray(b);bad[32]^=1;rejects('runtime stamp alteration',lambda:r.validate_stamp(bad,entries,dat,idx,overlays))
bad=bytearray(b);bad[24]^=1;rejects('overlay stamp alteration',lambda:r.validate_stamp(bad,entries,dat,idx,overlays))
bad=bytearray(b);bad[68]^=1;rejects('read set alteration',lambda:r.validate_stamp(bad,entries,dat,idx,overlays))
bad=entries.copy();bad[0],bad[1]=bad[1],bad[0];rejects('reordered libconfig index',lambda:r.validate_stamp(b,bad,dat,idx,overlays))
ov=ET.fromstring('<INI><TABLE name="ItemParam2"/></INI>');rejects('relevant new overlay',lambda:r.validate_stamp(b,entries,dat,idx,[ov]))
rejects('missing numeric field',lambda:r.long_cell({},'BarSpeed'));rejects('nonfinite numeric field',lambda:r.long_cell({'BarSpeed':'NaN'},'BarSpeed'))
ok('signed narrowing and truncation',r.signed(r.long_cell({'v':'65535.9'},'v'),16)==-1)
# Synthetic state checks exercise formulas; they are never catalog/runtime defaults.
totals=[None]*12;totals[9]=1234
ok('received total overrides absent baseline',r.resource_max(defs,9,None,totals,None,None,None,None)==1234)
rejects('absent stat baseline remains unknown',lambda:r.resource_max(defs,3,None,totals,None,None,None,None))
for stat,expected in [(9,120),(3,80),(6,1200)]:ok('real table base '+str(stat),r.resource_max(defs,stat,[0]*12,[None]*12,[None]*12,[0]*12,[0]*12,[0]*12)==expected)
inv=[{'ItemId':2001,'Type':1,'Count':3},{'ItemId':8001,'Type':6,'Count':300}]
ok('stack count versus drill life weight',r.carried_weight(infos,inv)==infos['2001']['Weight']*3+infos['8001']['Weight'])
rejects('unknown inventory item',lambda:r.carried_weight(infos,[{'ItemId':4294967295,'Type':1,'Count':3}]))
pkt=bytes([3])+struct.pack('<BiBiBi',9,1234,3,500,6,9000);ok('S282 indexed signed totals',r.indexed_totals(pkt)=={9:1234,3:500,6:9000});rejects('truncated S282',lambda:r.indexed_totals(pkt[:-1]))
ok('normal form effects',defs['normal_form_effects']==[15,21,23,24,25,26])
rejects('unknown potion use baseline',lambda:r.potion_metadata_allowed(infos['2001'],2,None,None,defs['normal_form_effects']))
(P/'checks.json').write_text(json.dumps({'status':'PASS','checks':passed},indent=2)+'\n');print('PASS',len(passed),'checks')
```

## Selected original method fingerprints

All ranges include the local-declaration prefix in the original decoded WASM. Full metadata owners and additional bodies are in the local `method-proof.json`.

| Metadata method | Function / offset / bytes | Original body SHA-256 |
| --- | --- | --- |
| m84673 <<ItemInfos>b__75_0>d.MoveNext | f30813@`0x984755` / 1092 | `b16f2f1dd98f77b3aae7bf302b98529c0d98131f12472ea934b44d06917c0e3c` |
| m41818 <ReadAsync>d__9.MoveNext | f22801@`0x701a55` / 5628 | `84c2a54c9ff36fb57946c7beae506e389dc9238b11e2c9d37b042ded47d85054` |
| m42306 LibconfigIndex.Refusal | f21000@`0x670397` / 166 | `dc9af8e73568e2f2dc3f8ba7d2acc67b40d8e76700c1b3c69de8e06e86f369f4` |
| m42303 LibconfigIndex.OverlayKey | f20996@`0x66f628` / 2911 | `553b1915193f5b44e1c923e84fa8665ee3f8243cee26aa793d5936a02dfc5f29` |
| m84878 Libconfig.ParseIndex | f9625@`0x2cb8a2` / 975 | `ac3709b5f11140fbf411f7ff9058d4bbcea55af55d6c8cd175d8556ae9ee6c5a` |
| m84882 Libconfig.TableBytesInPlace | f9629@`0x2cbef0` / 544 | `85c942b85a6b0b4f78f6bfe8d5ca23c9662412226eb3e5b2584a7bb4d615ed39` |
| m85153 Xtea.DecryptInPlace | f9571@`0x2c83a6` / 604 | `38fd7f7e512d5d4f6ebb15128b67acad32ff055aa39b631aa28636e71c8bfd40` |
| m38645 DrillData.Build | f21341@`0x689d3c` / 4708 | `7e5ceffe42f9c6e955138e45c945a543d241c9072044528eb0de0a409e1cf38b` |
| m40780 GunData.Build | f22073@`0x6c6b81` / 1053 | `234e7974639485a706561f6bbec3f6f3f9b94c4a31d0bafc3dffb33918e35942` |
| m37979 LevelTables.StatBase | f17456@`0x541779` / 240 | `0e6cf01b2e70351425083549c62041e1443fbd6bdd89e944855da2523268a91a` |
| m37976 LevelTables.PlayerMax | f17366@`0x5377d2` / 69 | `d65ddfb0e6c59b7ffa5fe8fb7315e4d15d9e3eee477fde3a47968ad697ed6a41` |
| m74873 CharacterStatus..ctor | f11469@`0x340c54` / 4164 | `f53547d01b77d0eff43ff8638ff6993ae3afdbeb2802f1b89b3e7fae8882398d` |
| m74885 CharacterStatus.Total | f11484@`0x3428fa` / 367 | `288cb57645dee0f98eebb08b9153e2df4b15c65a351baf58edb55fa92dc99e29` |
| m74855 Status.ParseIndexedValues | f11460@`0x34063a` / 294 | `044ebd93bea1e75cd628a712ee3db92c1639f109d449594d68f4b32f293b9e99` |
| m74894 CharacterStatus.<.ctor>b__23_6 | f11492@`0x343129` / 719 | `354567ca11e7487e42bafeb48fb5a1589bcdc3773092cb28da8aac66eb618c51` |
| m41673 ItemWeight.Of | f22676@`0x6f3ba5` / 228 | `f337be7aed46df846863f5758ec300b9b216b469527f44391fbf93ea2bf61502` |
| m72836 InvItem.get_Pieces | f11786@`0x3549fc` / 25 | `41738dcbaa48a5204f96942d8745b768f7cee4c63ce418b6f7ac79d23be0b2ce` |
| m42174 ItemsModel.CanUse | f20887@`0x664bb7` / 272 | `1802b1dc0c0784446551a0fa7a6751ebf667c3bd20519cb98590e6825ec4f50c` |
| m20530 World.IsHpPotion | f37798@`0xbb9014` / 62 | `aa066e60609f9690ce7b8b86f0199d77535834063699ac0fdb787b1e7e1dfe2c` |

[W]: https://tr-play.playall.in.th/Build/d9a4c4fd2737e740b82e1321839e352f.wasm.gz
[D]: https://tr-play.playall.in.th/Build/6135c4139b62e558bb4c755b266f5fa2.data.gz
[J]: https://tr-play.playall.in.th/Build/7f982dae9f02da04f526a91e415474b4.framework.js.gz
