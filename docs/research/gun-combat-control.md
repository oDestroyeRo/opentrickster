# Normal gun combat control

Issue [#29](https://github.com/oDestroyeRo/opentrickster/issues/29). Public-source inspection on **2026-11-08 UTC**, build **8502ebbf2fc9033d2fba50d02fd92002**. The bounded normal-gun source contract is recovered. **Server acceptance and live existing-character state remain untested.** This report supplies the control and attribution contract for #21; actual public definition rows belong to #27, and client-assisted input integration to #28.

A configured gun changes a normal monster click before `CombatLink.Attack`. The gun branch consumes the click even while reloading or unable to shoot. A native adapter must not substitute melee. A successful local send, a ready gauge, an attack-end notification, a target losing HP, and an own server-originated gun strike are different observations. Only the last supplies the attacker identity needed to attribute a shot.

Only this Markdown file is changed in the checkout. The starting branch was `research/gun-combat-control`, clean at `5e419ff89629b5fc3e45b730b43a247bd7f4163e`. AGENTS.md, its tracker/domain pointers, GLOSSARY.md, architecture/MVP contracts, and the research skill were read. Primary-source research was performed directly under the explicit no-subagent scope. Static decoding and synthetic transcriptions did not instantiate target WASM/JavaScript, access credentials/account data/private captures, log in, play, or open a game socket. Ordinary movement 24 and Attack 288+29 were not re-investigated.

## Artifact and locator contract

Cookie-free public GETs of [configuration](https://tr-play.playall.in.th/spinel-config.json), [version](https://tr-play.playall.in.th/version), and the [page](https://tr-play.playall.in.th/) agree on the build and asset references. A Python default-user-agent request returned 403; public `curl -q` requests with a browser user agent succeeded. Config SHA-256 is `084900013e6edde0ce0eca286dc7ac884a86da2ece1bae361ce0671852092602`; version is `9ce11c71ef039ffbe6bede54dfcf5b27afb887d44b2a79f57f959638e2e771b0`. The fetched HTML hash is recorded in the outside-Git evidence manifest because the page can change without replacing these assets.

The three approved cache files were read individually, without listing or exploring their mixed scratch directory. Both `.gz` files already contained decoded bytes. Their magic and these full hashes passed. Parsing the UnityWebData directory finds metadata at offset **16748911**, length **9343896**, byte-for-byte equal to the approved standalone metadata.

| Primary artifact | Decoded bytes | SHA-256 |
| --- | ---: | --- |
| [Current WASM][W] | 43136343 | `381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404` |
| [Current Unity data][D] | 27724487 | `fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474` |
| `Il2CppData/Metadata/global-metadata.dat` within [D] | 9343896 | `7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade` |

Below, `m` denotes a zero-based metadata method, `f` a WASM function index, and `@` an original decoded WASM file offset. Object offsets are linear-memory runtime offsets, not wire positions. These are current metadata→module→element-slot→body joins, not copied offsets from older builds. The full joined rows, tokens, element slots, original-body sizes/SHA-256 and field offsets are in `evidence.json` outside Git.

Metadata format is **108**, method rows **30 bytes** with name U32@0, declaring type U16@4 and token U32@18; type rows are **66 bytes**, field rows **10 bytes**. There are **490** imported functions. Module prefixes `(address,count,methodPointers)` are Net `(0x212548,4791,0x392c70)`, Game `(0x21251c,18796,0x37f5a0)`, UI.Pc `(0x2125a0,16606,0x3a9600)`. The token RID minus one indexes a module's pointer array, yielding an element slot; the element section gives the function, and the code section gives its original body. Registration type pointers `0xd75a0` and field-offset pointers `0x35d850` validate owning types and the fields below. [W], [D]

| Owner | Current join |
| --- | --- |
| `GunData.Build`, `Long` | m40780/40781 → f22073@`6c6b81`, f22074@`6c6fa0` |
| `GunFlowing..ctor`, `Wear` | m40837/40843 → f22096@`6c7630`, f22097@`6c8842` |
| `GunWorn`, `HoldsGun`, slot initializer | m40841/40842/40883 → f22104@`6c9243`, f22105@`6c92d4`, f22142@`6cb0be` |
| `OnKey`, `GunOn`, `MyAppear` | m40844/40846/40845 → f22108@`6c9591`, f22109@`6c95ad`, f22111@`6c98fe` |
| `Special`, `SpecialOn`, `Unload` | m40849/40850/40851 → f22099@`6c8b80`, f22101@`6c8e1a`, f22113@`6c9b6c` |
| `Update`, `Tick`, `MayShow` | m40852/40853/40854 → f22114@`6c9bb3`, f22115@`6c9d19`, f22117@`6ca13a` |
| `Click`, `Shoot` | m40857/40858 → f22122@`6ca3a7`, f22123@`6ca3c6` |
| `Fired`, `TakeBunch`, `ShotAnswered`, `Hit` | m40859/40860/40861/40862 → f22124@`6ca573`, f22125@`6ca61a`, f22127@`6ca850`, f22128@`6ca86a` |
| `Gun.Crc`, `Shot`, `ReadKey` | m72623/72624/72625 → f11831@`35756a`, f11832@`357589`, f11833@`3575e3` |
| `Combat.ParseSwing`, `ParseDamage`, `ReadDamage`, `ParseMessage` | m71371/71372/71375/71379 → f13206@`3aa31a`, f13207@`3aa5c1`, f13208@`3aa647`, f13214@`3aaaaf` |
| `CombatLink.Receive`, `BundleEnd`, `Flush` | m71395/71396/71397 → f13230@`3abfe7`, f13231@`3ac47f`, f13232@`3ac590` |
| `CombatModel.WorldClick`, `Begin`, `ApplyMessage` | m37914/37904/37897 → f17422@`53dfb7`, f17408@`53ba12`, f17402@`53ac1c` |
| UI gun installer and tick callback | m25097/25093 → f46644@`f4b3cf`, f46638@`f4a62f` |


Selected original body fingerprints supplement the joins above. All are byte ranges in the pinned decoded WASM, including local declarations and the final end instruction.

| Function | Body length | SHA-256 |
| --- | ---: | --- |
| f22105 | 614 | `3f0fc813b866cf9fa110a49956c2ecac16984d12b127d58b2b3cb91899860d72` |
| f22115 | 456 | `ddf61f3d909dd99d34cd2c2047a9f96973998a167bfcd7451fba16342e2fd2f6` |
| f22123 | 427 | `59a8337b00214947a04b04793224d7866000a63aaf77e329248f69a5ce5e6b95` |
| f11831 | 30 | `6dcbf85444a0383346b827c7555c8749e0979b27e8ceae970dd4cc17bc2aca02` |
| f11832 | 89 | `21d30c791e936c978358415f734b0ae547db96031564eda61a796f74910a3269` |
| f17408 | 3791 | `cecfd6f0dc443551f1f1822f055f32f00cf599b362d22f97effbd39484f09b1b` |

## Equipment, definitions and initial state

`CombatModel.WorldClick` handles the primary button, rejects own fainted state, requires a present **Type 3** monster pick and excludes `fading`. Its gun callback receives the picked actor ID and the current `moveMode`. A true callback return exits before ordinary targeting/Attack. The UI installer binds `GunFlowing.Click` into `CombatModel.GunClick +272` at `0xf4c5ba..0xf4c5e8`. `Click` tests `GunWorn`, calls `Shoot` when true, then returns that same gun-presence boolean independently of whether a packet was sent. Gun interception also happens before the ordinary same-target suppression and before assigning `CombatModel.Target`. Track the pending gun target explicitly. [f17422@`53e0a0..53e101`; f22122; f46644]

`HoldsGun` scans **slots [4,9,1,22]** in that order. For each in-bounds nonzero equipped UID, it obtains `Inventory.Get(uid)`, resolves `InvItem.ItemId` through the item-info lookup, and recognizes **ItemInfo.SubType == 54**. This test does not additionally require Type 2. The array is current compiled public configuration, four I32s copied by f22142 from fieldRef207, metadata field53533 at `0x4d16d0`, not a guessed slot toggle. Missing item definitions cannot establish that a character is unarmed. [f22105@`6c94db..6c9517`; f22142]

The wear-event handler has a narrower test, **Type == 2 and SubType == 54**. Wearing a matching item resets Armed, Ready, State, Load and Power, then sends **logical 511 with an empty payload** to request the gun key. Removing it hides/resets local gun state. The ordinary gun profile still comes specifically from **inventory slot 1**: UID → inventory item → ItemId → `GunData.Params[ItemId]`. `GunOn` requires that actual item and parameter row; sets MinRange, MaxRange and `MidRange = ((MinRange + MaxRange) * 3) >> 2`; outside camp it sets Param, Armed=true and State to 1 when ammo is loaded, otherwise 0. Slot scanning and profile selection are distinct contracts. A native implementation should block an unresolved/mismatched profile instead of guessing another equipped item. [f22097@`6c8931..6c8a28`; f22109@`6c967e..6c9825`]

`GunData.Build` resolves table **`Item_GunParam2`** and builds the first row for each ID. `Long` obtains `Zone.Field` → `Zone.JsNumber`, returns zero for nonfinite values, otherwise truncates toward zero to I64 (the six non-ID fields are stored as low32 bits); fields absent/unusable in real definitions must be surfaced by the adapter rather than replaced with fabricated zeros. The precise contract sent to #27 is:

| Column | Runtime field/type | Consumer |
| --- | --- | --- |
| `ID` | `GunParam.Id +8`, I64 key | Actual equipped `InvItem.ItemId` lookup |
| `GunType` | +16, I32 | Loaded profile; not used by the inspected normal CRC/tick |
| `GaugeSpeed` | +20, I32 | Reload progression, CRC |
| `PowRange` | +24, I32 | Power progression, strict critical upper bound, CRC |
| `PowStartPoint` | +28, I32 | Strict critical lower bound, CRC |
| `MinAttackRange` | +32, I32 | MinRange and MidRange |
| `MaxAttackRange` | +36, I32 | MaxRange and MidRange |

The checked table-name literal is **7666**, columns **6720,6284,5823,11258,11259,9268,9095**, resolved from compiled usage cells and adjacent metadata literal offsets. Stores at `0x6c6e45..0x6c6ec7` match the registered GunParam fields. The report does not claim to have fetched the selected character's item row or equipment. [f22073; D type6621]

**Key.** GunFlowing registers incoming **512** at `0x6c84ac`; callback m40893/f22148@`6cb4d4` calls `Gun.ReadKey`, then `OnKey`. `ReadKey` reads a U16 when at least two bytes exist. The client helper returns zero on a missing/short body, but that fallback is not evidence of a valid zero key. Preserve unknown on malformed input. A received zero is distinguishable from missing input. `OnKey` stores Key+82 and calls GunOn when not Armed. This key is received session state, not a compiled constant. No key bytes were captured or published. [f11833; f22108; f22148]

**Equipment and ammo.** The constructor scans StartSet **233** through `Items.ReadStartEquipment`. At **slot 20**, a resolved item of **ItemInfo.Type 29** sets `Loaded = InvItem.Uid`, **Bunch=1**, State=1. Other equipped items go through Wear. It also replays StartSet **518/521**. Live **518** has a leading U64 inventory UID and calls `SpecialOn`; Type29 loads the same way. Live **521** with a resolved Type29 UID unloads. A special item of Type16, or Type11/SubType60, clears loaded gun ammo. Bunch=1 is the recovered normal initialization, not a fixture fallback for unknown state. The inspected normal flow never chooses an arbitrary inventory item, buys ammo, or equips it merely to satisfy readiness. [f22096@`6c7aa2..6c7c3d`, `6c82f1..6c8314`; f22099; f22101]

Live **82/85** carry U64 UID plus U8 slot. Gun callbacks maintain previous-worn UIDs for slots0..22, look up the actual items, and run Wear on changes. Own appearance **8/9** is filtered by first U16 actor ID against Me before processing player equipment. The constructor and these subscriptions explain key acquisition when an equipped gun is recognized; **512 is not read as a StartSet baseline by this constructor**. Register the key listener before any 511 request so a coalesced response cannot be lost. [f22146/22147@`6cb361`/`6cb448`; f22111; f22096@`6c8458..6c852f`]

**Learned skills and active effects.** StartSet **271** begins U16 count, followed by five-byte records `(U16 skill,U8 ignored,U16 ignored)`. Presence of skill **3010** sets LoadsWhileMoving; **3011** sets KeepsLoadOnHit. Live **523** uses first U16 learned skill plus the current SkillLink level lookup. An incomplete learned-skill baseline is unknown, not proof that both are absent. [m40863/40864 → f22098@`6c8a74`, f22129@`6ca885`]

LoadSpeed starts at **f32 1.0** in the actual constructor. `ESAFastLoad.Init/InitState` reads an **F32 effect value v**, sends `f32(1 + f32(100-v)/100)` through IEsaHost.SetLoadSpeed slot63, then the UI host forwards to GunFlowing.SetLoadSpeed. OnEnd restores1. The effect's server-supplied value and presence are inputs; the default is justified only for a known baseline without that active modifier. This report does not synthesize active effect parameters from a skill label. [f16980@`50e959`, f16982@`50ec94`, f16983@`50ed64`; host f45412@`ee08ea`; f22131@`6ca9b2`]

Additional consumed definitions are **ItemInfo.Type/SubType/FlagsEx** and **ZoneInfo.Type**. `MayShow` allows a known zone Type other than **0,1,5**. An absent zone resolves to the disallowed zero branch in the client; a native adapter should retain absence separately. ItemInfo.FlagsEx bit64 controls local ammo consumption after firing. [f22117; f22125]

## Clock, readiness, shot and reload

GameplayScreen.Update uses Unity's **unscaledDeltaTime × 1000**. Advance delivers milliseconds to OnUpdate listeners. The gun UI callback keeps fractional milliseconds: `total = deltaMs + residue; dt = trunc(total); residue = total-dt`, then calls `GunFlowing.Update(dt, ownAvatar.Moving)`. `Update` calls Tick only when GunWorn and Armed. These are elapsed frame milliseconds, not absolute wall time, a system heartbeat, or a server tick. [m19448/19449 → f38536@`bfbf51`, f38537@`bfbf6d`, delivery `bfd5bd..bfd5cf`; f46638@`f4a67d..f4a732`]

Relevant `GunFlowing` fields, independently registered at type6629, are Armed+44, Ready+45, State+48, Load(f32)+52, Power(f32)+56, Param+60, Loaded(U64 UID)+64, Bunch(I32)+72, LoadSpeed(f32)+76, LoadsWhileMoving+80, KeepsLoadOnHit+81, Key(U16)+82, Min/Mid/MaxRange+84/+88/+92. State names below describe numeric branches; they are not invented enum names. [D; W f22115]

| State | Tick behavior |
| ---: | --- |
| 0 | Ready=false. No automatic action. |
| 1 | If zone permits, clear Load/Power and enter2; the current dt is not applied to loading. Otherwise enter0/hide. |
| 2 | Ready=false. Compute reload candidate below. Moving without3010 clears Load and Percent. Otherwise update Percent and enter3 at the threshold, clearing Power. Ready remains false for this tick. |
| 3 | Ready=true. Advance Power below. At stored Power≥200 enter4. |
| 4 | Ready=true. No automatic shot. |
| 5 | Ready=false while waiting. No local timeout or resend is implemented here. |

The exact arithmetic matters at transitions. Let `F(x)` round to IEEE binary32; all stored Load/Power/LoadSpeed values are binary32. The reload candidate is double precision:

```text
candidate = oldLoad + double(LoadSpeed) * 0x1.555556p-3 * dtMs * GaugeSpeed / 100
Load = F(candidate)
Percent = min(100f, F(F(Load * 100f) / 200f))
enter state3 when candidate >= 0x1.8fffffp+7
```

The source constant is the promoted binary32 approximation of one sixth, not an exact rational. State2 movement resets happen before the threshold check. For state3:

```text
candidate = oldPower + (double(PowRange) / 1200) * dtMs * 100 / 100
Power = 200f if candidate > 0x1.900001p+7 else F(candidate)
enter state4 when Power >= 200f
```

Do not process several states using one oversized tick. The switch runs one branch per update. The normal noncritical shot does not require waiting for the critical window; it requires Ready and actual valid input state. Selecting when to click is an application policy. [f22115@`6c9dc5..6c9edf`]

`Shoot(target,moveMode)` checks Ready. When true, it calculates **critical = Power > PowStartPoint AND Power < PowRange**, with strict inequalities. It sets **State=5 and Ready=false before sending**. It passes actual Param fields, current received Key, Bunch, target ID and the caller's moveMode. It does not look up, move to, or substitute a different target. Its nonready branch emits local explanations based on loaded ammo presence/count, without a shot. [f22123@`6ca403..6ca4f4`]

```text
crc = (((((PowRange << 3) XOR (GaugeSpeed << 7) XOR PowStartPoint) << 5)
        XOR Key) OR critical) AND 0xffff
logical477 payload = U16LE target, U16LE crc, U16LE Bunch, U8 moveMode
```

This is **seven bytes**, before normal framing/opcode mapping. The low bit is ORed, not forcibly overwritten. Preserve I32 bitwise semantics and the final U16 mask. Do not hard-code the current movement mode, force critical, send system5511 as combat, or send melee when the gun path does not emit. [f11831@`35756b..357586`; f11832@`3575ba..3575de`]

Reload starts on **own received firing**, not on `Send` completion. `Fired` requires a nonzero Loaded UID, obtains its actual inventory item and runs TakeBunch. Unless ItemInfo.FlagsEx has bit64, it calls `Inventory.Consume(uid,Bunch)` locally. Separately it tests **old Count−Bunch > 0** even when bit64 skipped consumption. If ammo is absent/exhausted it clears Loaded, resets Bunch=1 and State=0; otherwise it clears Load/Power and enters2. This local decrement is a client projection, not proof of a separate authoritative inventory delta. Do not double-decrement when later server inventory data replaces the count. There is no recovered universal gun cooldown duration. [f22124; f22125@`6ca756..6ca7c3`]

Own damage calls `Hit`; without learned3011 this restarts State2 and clears Load/Power. With3011 it preserves them. Readiness is refreshed by the next Tick, so a caller must not mistake an old Ready flag after a reset for a new authoritative outcome. [f22128; UI callback m25088/f46633@`f4a2ff` compares damaged actor with session.Me.Id]

## Authoritative relationship and outcome attribution

All opcodes here are incoming logical opcodes, not their mapped wire numbers. The source's actor association is explicit:

1. **31,485,1438** enter `CombatLink.Receive`'s Strike branch and parse a Swing. With nonzero Aux, the Strike is collected under that Aux; recognized subsequent messages with the same Aux are appended to its Hits. Incoming1951 invokes BundleEnd, which removes and emits that collected Strike; Flush is the source's microtask fallback. Aux0 emits immediately. Preserve envelope Aux and collection boundaries in the native parser. Do not correlate by receive-time proximity. [f13230@`3ac0ee..3ac298`, `3ac31e..3ac464`; f13231; f13232]
2. For normal gun Swing.Kind **2**, wire fields are `U16 Attacker, U16 Kind, U16 Target, U16 FromX, U16 FromY, U16 ToX, U16 ToY`. If at least two bytes remain, read **I16 Amount**; if a byte remains afterward, read **U8 Result**. Thus a one-byte tail is Result without Amount. Kind3 is a separate hammer/multitarget grammar, not normal gun. Preserve absent optional values as unknown in the adapter even though allocated source fields start at zero. [f13206@`3aa378..3aa5b1`; D type11548]
3. `CombatModel.Begin` tests **Swing.Kind==2 and Swing.Attacker==session.Me.Id** before invoking GunFired. UI callback f46632 invokes GunFlowing.Fired. Other actors' gun swings cannot trigger this callback. This proves the own-firing relationship independently of damage to the selected monster. Matching the one outstanding native request also requires **Swing.Target == pendingTarget**, the same session/zone epoch and no intervening ambiguous manual action. The target and epoch restrictions are conservative adapter correlation policy; the protocol provides no request nonce. [f17408@`53c49b..53c555`; installer f46644@`f4c4fb..f4c56b`; callback f46632@`f4a2f0`]
4. **42 DamageEvent** begins **U16 Target** followed by **I32 Amount**, discarded U32, U32 KindBits, U32 Element, U32 Flags, U32 Extra, optional U16 Guard when at least two bytes remain. Kind is `KindBits & 127`. The mandatory body is26 bytes. There is **no attacker ID** in this message. DamageEvent.Target is the recipient, not the shooter. [f13207/13208; D types11549/11555]
5. **43 Faint** provides U16 Id and optional U8 Kind. It identifies who fainted, without a killer. **286 MonsterHp** identifies a monster and absolute HP; it does not identify who damaged it. A Faint or target damage can be attributed to an own strike only when preserved as a hit of that same complete, unambiguous own Strike bundle and the recipient matches the target. Standalone damage/fainting, another actor's Strike, and death after an old own shot must remain unattributed. A same-bundle Faint establishes a source-associated death, not a recovered loot, EXP-credit, or exclusive kill-ownership rule. [f13214 dispatch `3aab89`, `3aaba3`, Faint `3aac51..3aacc0`; f13230]

`CombatModel.lastAttacker` and deathRecords are client-maintained associations. Begin writes target→attacker from Swing; `LastAttackerOf`, `TryGetLastAttacker` and `SetDeathRecord` reuse/update these local maps. They do not add a killer field to a standalone Faint. Using the last locally remembered attacker to claim exclusive server kill credit would exceed the evidence. Damage subtypes, unknown flags, Swing.Result codes and the optional Swing.Amount are retained raw here; none is relabeled as success/failure without a recovered discriminator. [f17408@`53bd3e..53bd73`; m37909/37910/37911 → f17417@`53d9a8`, f17418@`53da1d`, f17419@`53db68`]

**Attack end and non-outcomes.** Incoming **32** reads U16 actor ID. The gun callback requires own ID and GunWorn, then ShotAnswered changes5→4. CombatModel separately clears its own Target, sets attack FSM2→0 when applicable, and removes attackTargets[Id]. Incoming **310/525** call the same ShotAnswered while Armed and ignore payload in these gun callbacks. Those packets can release the local waiting phase, but neither target success nor a specific refusal reason is established. A send receipt, this phase release, gauge animation, absolute target HP, or an EXP update alone must not confirm the native attack. [f22151@`6cb50e`, f22152@`6cb601`; registrations `6c85a6`, `6c85cf`, `6c85f9`; f22127; f17402@`53b06e..53b0dc`]

## Cancellation and quiescence

No gun-specific cancellation send occurs in the inspected `Reset`, `GunOff`, `Unload`, `Hit`, drill-visibility or camp-reset functions. Reset/GunOff clear local phases; Unload clears the local ammo selection; DrillBegin can hide the gauge without itself stopping Tick. `CombatModel.AttackCancelled` clears Target and changes local attack FSM2 to0. `UnhandledClick(0)` clears Target. These are not server cancellation acknowledgements. [f22106@`6c953b`, f22107@`6c9553`, f22113, f22134@`6cacb6`, f22139@`6caecb`; f17362@`53735f`, f17424@`53e3f8`]

The source-backed native control policy is consequently:

- Serialize intents and latch a pending shot before emitting477. Pause/Stop disables further emission immediately; stop callback/timer ownership before reporting local quiescence.
- Keep the outstanding shot separate from the gun's gauge/state. An own matching strike resolves firing; correlated Hits resolve observed damage/fainting; own32 establishes observed attack-state end. Do not replay an unanswered shot merely because State4 or a timeout permits another local click.
- Drain already-received bundles and late outcomes. If a sent shot lacks an attributable terminal observation, report **uncertain** and close/retire the session according to integration policy. Closing establishes that this adapter will send no more; it does not prove that an already-sent action was undone on the server.
- Reconnect, zone/actor replacement, equipment changes and manual interference invalidate pending correlations and timing baselines. Require fresh actual state before Resume. No persistent replay identifier or server undo guarantee is recovered here.

This is an implementable bounded pause contract, not evidence that the target protocol offers a stronger cancel operation. Client-assisted input owners must also stop their own held/repeated input; #28 owns that boundary.

## Reproduction and verification

Scratch is **`/private/tmp/opentrickster-gun29.znrIPu`**. `parse.py` reconstructs current metadata, linear memory, module/element/code joins from only the approved public artifacts copied there. `scoped.py` produces a non-executed disassembly copy, keeping selected original function bytes/indices and stubbing unselected bodies. WABT **1.0.39** at `/tmp/opentrickster-issue13/tools/node_modules/wabt/bin/wasm-objdump` decodes it; offsets are translated back to original bodies. `read.py` annotates calls and metadata usages; string bytes are suppressed except explicitly selected public table/column names. `branches.py` resolves branch-table nesting. `verify-source.py` checks the pinned originals, not an executed target.

```sh
python3 /private/tmp/opentrickster-gun29.znrIPu/parse.py
python3 /private/tmp/opentrickster-gun29.znrIPu/scoped.py
python3 /private/tmp/opentrickster-gun29.znrIPu/verify-source.py
python3 /private/tmp/opentrickster-gun29.znrIPu/reference.py
```

Observed source checks passed **3 artifact hashes**, embedded metadata identity, **3 named module joins**, **66 method joins**, **48 field offsets**, **25 pivotal instruction locations**, table column/slot identities and deployment coherence. The independent Python behavioral transcription passed **38 synthetic checks** covering strict critical boundaries, reload transition/rounding, movement and hit skills, no duplicate shot while waiting, ammo exhaustion/bit64 behavior, wrong actor/target, unbundled damage, and short/optional swing tails. These counts identify checked assertions, not full-client equivalence. Full untouched WASM validation with `wasm-validate --enable-all` also passed. No product build or live game test belongs to this research-only change.

The compact arithmetic/serialization lever below is standalone Python3. Inputs in its two final assertions are invented. Actual parameters, key, loaded ammo, effective LoadSpeed, learned flags, zone, own identity and pending target must come from their respective decoded sources.

```python
import struct

def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]

def gun_crc(gauge_speed, pow_range, pow_start, key, critical):
    return (((((pow_range << 3) ^ (gauge_speed << 7) ^ pow_start) << 5)
             ^ key) | int(critical)) & 65535

def shot(target, gauge_speed, pow_range, pow_start, key, power, bunch, move_mode):
    critical = pow_start < power < pow_range
    return struct.pack('<HHHB', target,
        gun_crc(gauge_speed, pow_range, pow_start, key, critical), bunch, move_mode)

def tick(state, load, power, ready, dt_ms, moving, moving_skill,
         gauge_speed, pow_range, load_speed, zone_type):
    if state in (0, 5):
        ready = False
    elif state == 1:
        state = 0 if zone_type in (0, 1, 5) else 2
        if state == 2:
            load = power = 0.0
    elif state == 2:
        ready = False
        value = load + f32(load_speed) * float.fromhex('0x1.555556p-3') * dt_ms * gauge_speed / 100
        load = f32(value)
        if moving and not moving_skill:
            load = 0.0
        elif value >= float.fromhex('0x1.8fffffp+7'):
            power, state = 0.0, 3
    elif state == 3:
        ready = True
        value = power + (pow_range / 1200.0) * dt_ms * 100 / 100
        power = 200.0 if value > float.fromhex('0x1.900001p+7') else f32(value)
        if power >= 200.0:
            state = 4
    elif state == 4:
        ready = True
    return state, load, power, ready

assert gun_crc(3, 5, 7, 0x1234, True) == 0x27d5
assert shot(123, 60, 80, 20, 0x1234, 20, 1, 2) == struct.pack(
    '<HHHB', 123, gun_crc(60, 80, 20, 0x1234, False), 1, 2)
```

The complete `reference.py` additionally models fractional milliseconds, source click interception, ShotAnswered, own-hit reset, own-fire ammo/reload, and typed own/bundle attribution. It intentionally adds no automatic retries, fabricated runtime values or network access. `evidence.json` is the machine-readable method/field/pivot manifest; `verify-source.py` is the checker. These final hashes bind the local handoff.

| Outside-Git file | SHA-256 |
| --- | --- |
| `parse.py` | `3c9148bed93382821e69ae2ff9c60f4ee783bb35abeeb67754d62e48aff8c052` |
| `scoped.py` | `c36af076b61451ceff87a3e55441d1dbf329d4c71623fafab77af6e95554426d` |
| `selected.json` | `aadacd16cbf9d6ae658d2a1edc4b5deee4641dbb3796da43ed02199b6500e023` |
| `read.py` | `4648b01d41acabadef42344bf9c37cff90f159141a0e458b408619478c793d3f` |
| `verify-source.py` | `1b167fa2063cbc3580b180b89e87008b7e40d674ce14ccd48f1bf1ecd2b0964c` |
| `reference.py` | `99fc4894193d928d9cbc24d6ace3511f4c62137587e42f1b30d7d8324ea33c52` |
| `evidence.json` | `f53bc633cd433aafaddfd847d299015fbeb1a4b521aca82ae709c5bdbd727c20` |


## Remaining integration prerequisites

The recovered source contract is sufficient to implement a normal gun controller. Runtime readiness still requires a current-build session, actual complete equipment/inventory and ammo selection, the slot1 gun's real `Item_GunParam2` row, item/zone definitions from #27, learned3010/3011 state, effective active-effect LoadSpeed, received512 key, own actor identity, fresh world/target state, current moveMode and a continuous elapsed-time baseline. Unknown prerequisites block the gun capability; they do not justify substituting1/0 or melee.

Integration must prove one bounded own477 shot and matching server-originated Kind2 strike plus relevant same-bundle target outcome, distinguish another player's damage/death, apply authoritative inventory updates without double consumption, and establish pause/session cleanup with an in-flight shot. It must measure actual server response/acceptance and timeout behavior. Source-derived range fields are not proof of server range acceptance, a fixed cooldown, ammo compatibility policy, key lifetime, exactly-once processing, kill/loot credit or a cancel guarantee. No broader implementation ticket is resolved by this report.

The previous [current-build revalidation][R18], [state completion][R14] and [manual digging][R26] reports were comparison leads. Their local bytes matched the named immutable Git blobs; all gun contracts above were checked against current W/D. Ordinary protocol/framing reuse stays under R18. Public binary formats and disassembly are interpreted with [Cpp2IL metadata definitions][C] and [WABT][B]; target-specific facts come from the pinned target bytes.

[W]: https://tr-play.playall.in.th/Build/d9a4c4fd2737e740b82e1321839e352f.wasm.gz
[D]: https://tr-play.playall.in.th/Build/6135c4139b62e558bb4c755b266f5fa2.data.gz
[R18]: https://github.com/oDestroyeRo/opentrickster/blob/df5fe0a5e8e45072845538fc74967a1757b6d9d7/docs/research/current-build-protocol-revalidation.md
[R14]: https://github.com/oDestroyeRo/opentrickster/blob/cb587e235b113aa26b04db018c31f1e26510f5bd/docs/research/target-state-completion.md
[R26]: https://github.com/oDestroyeRo/opentrickster/blob/b1a12a97420fff84f15cc1e3cf8b5070560ed7b9/docs/research/manual-dig-control.md
[C]: https://github.com/SamboyCoding/Cpp2IL/tree/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL
[B]: https://github.com/WebAssembly/wabt/releases/tag/1.0.39
