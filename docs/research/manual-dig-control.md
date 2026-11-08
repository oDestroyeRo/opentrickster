# Manual digging control and completion

The goal is a native adapter that can finish one normal selected-point dig without Unity. This report answers the bounded source question in [#25][I25] for build `8502ebbf2fc9033d2fba50d02fd92002`. The recovered exchange is **C→S 44, S→C 45 and own 371, local timed input simulation, C→S 49, S→C 50**. Cancellation uses C→S 48. The client does not send a packet for each hold, release, scoop, or gauge-stage transition. A special odd-key/empty-spot branch cancels before simulation.

This is source evidence plus invented synthetic fixtures. Server acceptance, server timing/checksum validation, and a completed live dig are **unrun**. No game socket, account, credential, private capture, browser, or target-code execution was used. Only this Markdown file is delivered. Product code and the planning map belong to the parent task.

## Source pin and reproduction

The approved decoded public cache supplied these primary sources. `.gz` filenames here contain decoded bytes. Hashes were checked against the actual files, not inferred from filenames. A fresh public config request returned HTTP 403 during this investigation, so “current” means this pinned artifact set, not a newly verified deployment.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| [WASM][W] | 43136343 | `381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404` |
| [Unity data][D] | 27724487 | `fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474` |
| `Il2CppData/Metadata/global-metadata.dat` within data | 9343896 | `7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade` |

Metadata is at data offset **16748911**, magic `0xfab11baf`, format **108**. The [current protocol report][R18] and its supplied migration manifest were read as leads. The local report bytes matched immutable commit `df5fe0a5e8e45072845538fc74967a1757b6d9d7`. The [state-completion report][R14] matched `cb587e235b113aa26b04db018c31f1e26510f5bd`. The [earlier manual report][OLD] supplied comparison leads only.

Current joins were reconstructed from the primary bytes using the metadata/WASM procedure in [R18], including image-name/module discovery, active element and data segments, **490 imported functions**, and original code-body ranges. Methods are 30-byte metadata rows; types are 66-byte rows. Method token at row+18 gives RID; module methodPointers[RID−1] gives an element slot, which gives a WASM function. Game module is memory `0x21251c`, count **18796**, pointers `0x37f5a0`; Net is `0x212548`, **4791**, `0x392c70`; UI.Pc is `0x2125a0`, pointers `0x3a9600`. Field offsets came from registration at `0x1a4e60`, field-offset pointer array `0x35d850`.

WABT 1.0.39 statically disassembled selected unchanged bodies in a scratch copy with other bodies stubbed. Original offsets were restored and body hashes checked against the untouched WASM. No stub or original module was instantiated. In the locator table, `m` is the metadata row index, `e` the element slot, and `f@offset` the function and original WASM body offset. G/N/P denote Game/Net/UI.Pc. All method and instruction claims below refer to [W] joined with [D].

## Packets and state boundaries

Logical opcodes require the current directional mapping and six-byte framing from [R18]. All multibyte payload integers below are little-endian. Payload widths are independent of that envelope.

| Direction / logical opcode | Exact payload or callback | Local consequence |
| --- | --- | --- |
| C→S 44 | `U16 X, U16 Y, U8 MoveMode`, 5 bytes | `Press` truncates coordinates from f64 to signed i32, then writes low 16 bits. Mode writes low 8 bits. Mode 2 is not evidence of automatic digging. |
| S→C 45 | Callback reads no payload, actor ID, or request ID | `OnAccepted` sets avatar FSM 5. Entry resets counters/gauge/held and chooses the worn drill and parameter row; local stage becomes 1 Waiting. Missing drill/parameter returns avatar FSM to 0. |
| S→C 46 | Callback reads no payload | Clears flag bits `0x0c`. It is not a completion record. |
| S→C 371 | `U16 actor,X,Y; U8 branch; tail; U16 key; U8 stress` | `OnSpot` accepts only own actor while avatar FSM==5. Stores spot/key and marks information received. |
| C→S 49 | `U16 starts, U16 elapsedMs, U16 depth, U16 checksum`, 8 bytes | Sent once by `AvatarUpdate` after the depth check. Sets local stage 7 Finished. It is a completion request, not a found/empty result. |
| S→C 50 | `U8 kind, U16 value, U32 itemOrValue`, 7 bytes | Applies while avatar FSM==5, or for kinds 11…19. Kinds 2/9 give Nothing feedback; 3 gives DiggedOut/item feedback. Calls `EndDig(false)`, so no cancel send, and marks `resultMark=255`. |
| C→S 48 | `U16 starts, U16 elapsedMs`, 4 bytes | Active cancellation or the early odd-key/empty branch. Pending cancellation can be `(0,0)`. |

For 371, nonzero branch reads **I16 Unusable**, giving 12 bytes total. Zero branch reads **U8 BottomUnits, U8 ItemDepthUnits, U8 Nature**, giving 13 bytes total; both depths multiply by 5. `Unusable` defaults to zero in that branch. 47 is the other-actor form, with an Auto byte instead of own key/stress, and must not substitute for 371. The reference decoder rejects extra bytes; the source callbacks only establish consumed widths. [10325@`30291a`; callbacks 14167–14170@`3ee538`–`3ee578`]

The ordinary-result callback does not read a request correlation ID. It can handle 50 in any local stage while avatar FSM==5, so a refusal/result must not wait for an outgoing 49 first. One outstanding local dig must remain distinct from observations about other actors. Kinds 4,6,7 carry low-grade, empty-life, and non-diggable feedback. Crazy-mode kinds 11…19 and automatic mode remain separate from this normal controller. Result 3 alone does not establish inventory receipt; use the inventory reducers in [R14].

### Starting and beginning locally

`PressDig` checks equipment/map/life, arms targeting, and chooses AutoMode from the equipped pet only for the separate automatic controller. Normal `Press(at,inWarp)` requires avatar state 0…2 and targeting flag 4; it sets pending bit 8 and sends 44. During avatar state 5, `Press` only sets **Held=true**. `Release` only clears Held. `AutoHost.SetButton(code)` sets Held to exactly `code==3`; both 0 and 2 release. These are local input codes, not wire values. [21295,21297,21379; 14132@`3ed8f8`]

`OnFsm(5)` clears received-spot state and initializes local stage 1, gauge/depth/scoop/elapsed/starts to zero, Held=false, and resultMark=240. `OnSpot` must occur after that entry to survive its reset. It does not begin immediately. On an update with stage 1, information received, and `moving==false`, `AvatarUpdate` consumes the information and calls `Begin`:

1. `Unusable > 0` invokes `EndLocal`, with no 48 or 49 from this branch.
2. If `(key & 1)!=0` and `ItemDepth<=0`, `Begin` calls `EndDig(true)` and sends **48**, using the current counters. It returns before clearing the key, copying depths/stress, or entering Idle. Instruction `68e8f5 br_if 1` exits to the branch ending in the send path at `68eb77`.
3. Otherwise it clears the key low bit, sets `bottom7=Bottom*7`, `itemDepth11=ItemDepth*11`, Nature and Stress, opens the local gauge, and enters stage 2 Idle. An odd key with positive item depth also produces a local hint. The key bit is not a fabricated “found” acknowledgement. [21401@`68e740`, mask `68e9ba`, state write `68ed27`]

`EndLocal` sets avatar FSM 0, clears flags, and re-arms targeting through `PressDig` when its prerequisites pass. The reference below collapses this to a stopped local state because it models one dig, not targeting UI.

## Timed gauge algorithm

The current controller is `DrillFeature.Step` and `AvatarUpdate`. Older `Driller`/`TryInputs` leads do not require a Unity object or a separate input protocol. Current UI `DrillView.Update` calls `DrillFeature.Update`; `Update` truncates its f64 milliseconds to i32, calls `AvatarUpdate`, then the automatic controller if present. There is no fixed tick interval in these methods. A native implementation can schedule regular real-time updates and use measured elapsed milliseconds; it must retain the same discrete ordering. The 16 ms fixtures below are a synthetic schedule, not a discovered server requirement. [44745@`e9cde7`, call `e9cfb8`; 21402@`68ed34`]

State uses f32 **gauge** and **depth4**, i32 **scoopMs**, **ElapsedMs**, **starts13**, and Boolean **Held**. Parameters are `A=BarEffArea`, `B=BarSpeed`, `S=ScoopMass`; marker gauge is `200-A`. `K` is the exact f64 literal **`0x1.555556p-3`**, not mathematically exact 1/6. Gauge addition is f64 `oldGauge + dt*K*B/100`, rounded to f32 for storage. Charging first rounds dt to f32; the other stages use the i32 dt directly.

Depth decay is **per call**, independent of dt: let `d=f32(depth4*0.25)`; if `d<=f32(0.35)`, depth4=0, else `depth4=f32(f32(d+f32(-0.35))*4)`. Scoop adds dt to scoopMs; if `scoopMs>=120`, subtract 120 **once**, then `depth4=f32(depth4+f32(f32(S)*4))`. A delayed update must not be replaced with an unbounded catch-up loop. [21406@`68f779`; helpers 21410@`68fec4`, 21412@`68ff3a`, 21413@`68ff71`]

| Stage at dispatch | Update, in source order |
| --- | --- |
| 0 Off | No gauge/depth progression or elapsed increment. |
| 1 Waiting | Add dt to elapsed. Movement can defer Begin while this counter runs. |
| 2 Idle | Decay depth. If not held, no elapsed increment. If held, add 13 to starts13, wear the drill, set stage 3, and execute the Charging case in the same call, including a **second decay**. |
| 3 Charging | Decay; raise gauge. If stored f32 gauge >= marker, enter 5. Otherwise release enters 4. Add dt to elapsed. |
| 4 Falling | Decay; lower gauge. Compare the pre-storage f64 sum to `2^-150`: <= threshold sets gauge=0 and stage 2; otherwise held enters 3. Add dt to elapsed. |
| 5 Drilling | Add dt/scoop; raise gauge. If pre-storage f64 gauge >= **`0x1.8fffffp+7`**, reset stage=2, gauge=0, depth4=0. This failure check follows the scoop and takes precedence over release. Otherwise release enters 6. Add dt to elapsed. |
| 6 Coasting | Add dt/scoop; lower gauge. If stored f32 gauge < marker, enter 4. Otherwise held enters 5. Add dt to elapsed. |
| 7 Finished | No elapsed/depth/gauge progression. Wait for actual server result; do not send 49 again. |

The eight stage values are 0…7 in the listed order. Metadata enum defaults use compressed signed integers, not four-byte values. The dispatcher at `68f7c2` is `br_table 0 7 1 2 3 4 5 6 7`. Full block nesting was checked when assigning cases.

After Step, `AvatarUpdate` checks only resulting stages **2…6**. Let `d=f32(depth4*0.25)`. If `itemDepth11<=10`, reaching signed `bottom7/7` ends at that bottom; otherwise reaching unsigned `itemDepth11/11` ends at the item depth. If that selected threshold was not reached, reaching `param.DigDepth` ends at the drill limit. The selected spot threshold is tested first, not `min(spotDepth,limit)` for an overshooting frame. Clamp depth4 to four times the selected threshold, truncate its quarter to i32, serialize 49, set stage 7, reset closeTimer. An overflow reset in Step has already discarded depth before this check. [21403, `68eeeb`–`68f077`; separate matching EndCheck helper 21408]

The ordinary button policy evidenced by **AutoDrill.Tick 27438**, reached through AutoDrill.Update, is usable as a local input policy without constructing automatic mode:

- Begin phase: stage 2 presses 3; observing stage 3 enters Charge.
- Charge: hold 3 until stage 5 is observed, then release 0 and enter Drill.
- Drill: stage 7 releases 0 and waits. Otherwise press 3 when `GaugePercent <= MarkerPercent + 10`, else release with code 2. The comparison promotes both f32 percentages to f64 before adding 10. `GaugePercent=f32(f32(gauge*100)/200)`; `MarkerPercent=f32((200-A)*100.0/200)`.

Apply this policy **after** the simulation update, as the source does, so it controls the next tick. The policy does not guarantee convergence for every possible malformed or unsuitable catalog row. Missing/nonprogressing parameters require an explicit unavailable/timeout outcome, not a guessed speed or completion packet. AutoDrill.Handle **27439@`895591`** handles events/results; it was a useful lead, but the repeated gauge choices are in Tick (`894ad3`, `894c31`, `894d8c`–`894e23`). AutoHost supplies the same Held boolean. None of this requires a paid pet for a normal manual request. Automatic 785/791/790/794 requests remain separate. [14129–14132; 21315/21316; R14]

### Completion arithmetic and cancellation

For signed i32 division truncating toward zero, `starts=starts13/13`. The completion checksum is:

```text
(key ^ BarEffArea ^ (BarSpeed << 1)
     ^ ((ScoopMass ^ (depth << 3) ^ (starts << 7)) << 5)) & 0xffff
```

Use the key after Begin's bit clearing. Shifts/xors use 32-bit bit patterns; final truncation makes the low 16 bits decisive. Serialize each field's low 16 bits. **ElapsedMs is not part of the checksum.** There is no local RNG, timestamp, nonce, or additional completion field on this normal send path. The random call seen in Preconditions chooses local refusal speech. Stress is received and shown; it does not alter the normal Step or completion checksum. Do not import crazy-mode stress/bang behavior. [10323@`302867`, 10324@`302896`; 21403 call `68efef`, opcode `68f000`]

Integer additions to elapsed, scoopMs, and starts13 wrap as i32. The final fields wrap as U16, so long elapsed time is not evidence of a zero counter. The pure reference retains these rules and rejects negative/nonfinite dt at its boundary. The target conversion itself is saturating truncation, including its own nonfinite behavior. Production code need not accept pathological time input merely to imitate that conversion.

| Cancellation point | Source behavior |
| --- | --- |
| Target selection flag 4 in generic `Cancel` | Clears targeting first, no packet. This branch precedes its active-state check. |
| Request pending, outside avatar FSM 5, flag 8 | `Cancel`/`Withdraw` send 48 `(0,0)`. |
| Active Waiting (1) | RightClick/CancelDrilling, or active Cancel once not targeting, sends 48 `(starts13/13,elapsed)`. Elapsed can already be nonzero. |
| Active Idle/Charging/Falling/Drilling/Coasting (2…6) | Same 48 with actual counters. Idle retries increment starts and wear only when a new charge starts. Release alone does not cancel. |
| Finished (7), 49 already sent | RightClick/CancelDrilling use EndLocal; no additional 48. This is local cleanup, not server acceptance. |
| Inbound normal 50 | `EndDig(false)` emits local end feedback and clears targeting/pending flags; no 48. |
| Begin odd key + no item | `EndDig(true)` sends 48 before this update's Step. Usually starts=0; elapsed includes prior Waiting updates. |

EndDig(true) emits an end event and sends 48; it is not a server acknowledgement. The UI may react to that event. The adapter must stop its own simulation after requesting cancellation and preserve any later server outcome separately. There is no cancellation-ack schema established here. Result-driven close timing (`resultMark==255`, closeTimer>700 ms) must not become a 700 ms “success” assumption. [21292@`6869cc`, 21291@`68691a`, 21374@`68d55e`, 21305@`687e50`, 21304@`687d58`, 21403@`68f0ce`]

## Minimal equipment and definition inputs

`get_WornDrill` resolves equipment **slot 7** to an inventory UID, requires a present inventory item, then a template whose **ItemInfo.Type==6**. `InvItem.ItemId +16` selects `DrillData.Params[ItemId]`. Current life is **InvItem.Count +28**, sourced from the U16 Count in ItemBody; it is not inferred from the four State bytes at +32. Startup 231/233 and equipment 82 schemas and replay ordering are in [R14]. Preconditions requires Count>0 and rejects map classifications Type 1/5/6 and SubType 14 when a zone is present. The reference assumes those checks and the own-actor/FSM gates have passed. [21369@`68d12a`; 21301@`687767`]

| Definition/input | Required use and source conversion |
| --- | --- |
| `Item_DrillParam.Id` | ItemId lookup key, parsed as signed 64-bit integer. |
| `BarEffArea`, `BarSpeed`, `ScoopMass`, `DigDepth` | Exact simulation/checksum inputs. `DrillData.Build` takes signed low 16 bits of truncated numeric values. No rates are supplied by 45/371. |
| `GroundNature` | Low 32 bits of numeric value. Each new Idle→Charging charge wears 1 life when zero or equal to spot Nature, otherwise 2. `SetLife` clamps the local result at zero and emits a local Inventory.Apply notification; this sends no wear packet. |
| `Grade`, `DrillLife` | Signed low 16 bits. Grade is loaded but is not used by this normal Step/checksum. DrillLife supplies the displayed life percentage. A server result can refuse grade/life; no server threshold is proven. |
| `ItemInfo.Id`, `ItemInfo.Type`; zone Type/SubType | Equipment identity/type and the source precondition checks. Actual selected rows must be available. |

`DrillData.Long` reads a field, converts through JsNumber, maps NaN/infinity to zero, truncates toward zero, then saturates to i64. Missing values must not silently become plausible native rates. The exact required positive progression values have to come from the selected current public definition row. [21341 field writes `68a164`–`68a22a`; 21343@`68b0bc`]

The current definition access path is source-backed, without downloading an entire catalog here:

1. `Boot.MoveNext 36253` constructs GameAssets with **`libconfig_thai`** at `b0ab40`. `DataSources.CreateDefault 32577` resolves Resources `SpinelDataSettings.webDataUrl`, or fallback `pack`, relative to Application.absoluteURL.
2. `GameAssets.Table(name)` → `ReadTable` → `LibconfigSource.Open`: read logical `libconfig_thai.idx` and the first byte of `libconfig_thai.dat`, derive Libconfig.Layout and ParseIndex, then `ReadByName("Item_DrillParam")` and the named index entry's data span. DrillData.LoadAsync requests its declared table names, and Build selects this table. `MapAttr` supplies the ground/map definition branch. [30753@`97c867`; 30875@`98a9b6`; 21346@`68b3e4`]
3. `HttpDataSource.UrlAsync` probes public **`_spinel/data-server.json`** (`rangeQuery`, `encrypted`, `dataVersion`). For an encrypted deployment, `ServedPathAsync` uses the public data-key/name transformation and `.bin`, and UrlAsync adds version query `v`. These logical names must not be treated as verified cleartext URLs. The current source paths are `_spinel/data-key`, DataCipher.NameAsync, HttpDataSource.Take, and ReadRangeAsync/RangeQueryAsync. No account key or account file was accessed in this task. [32532,32524,32556,32568,32548]
4. `GameAssets.ItemInfos` first uses `ItemIndex.PathFor(libconfigBase)`, **`_spinel/item-index-libconfig_thai.bin`**, with the current libconfig runtime-key/overlay checks. Its decoded item supplies Id/Type. [30813@`984755`, PathFor call `9848f3`; 22784@`700776`; parser 22801@`701a55`]

This identifies the fetch/definition boundary and minimal consumed fields; it does not claim a freshly fetched selected-item row, a complete catalog decoder, or any live-equipped tool. That concrete row and the received inventory/spot remain prerequisites for #21's real run. Substituting fixture values is not authorized by this report.

## Current method locators

| Image / method | m / token / e | f @ body |
| --- | --- | --- |
| G `DrillFeature::get_WornDrill` | 38714 / `0x60011e0` / 9210 | 21369 @ `68d12a` |
| G `DrillFeature::OnFsm` | 38759 / `0x600120d` / 9255 | 21397 @ `68e243` |
| G `DrillFeature::ResetDriller` | 38760 / `0x600120e` / 9256 | 21398 @ `68e53d` |
| G `DrillFeature::OnSpot` | 38761 / `0x600120f` / 9257 | 21400 @ `68e65f` |
| G `DrillFeature::Begin` | 38762 / `0x6001210` / 9258 | 21401 @ `68e740` |
| G `DrillFeature::Update` | 38765 / `0x6001213` / 9261 | 21402 @ `68ed34` |
| G `DrillFeature::AvatarUpdate` | 38766 / `0x6001214` / 9262 | 21403 @ `68ed6f` |
| G `DrillFeature::Step` | 38767 / `0x6001215` / 9263 | 21406 @ `68f779` |
| G `DrillFeature::EndCheck` | 38774 / `0x600121c` / 9270 | 21408 @ `68fdfd` |
| G `DrillFeature::Press` | 38728 / `0x60011ee` / 9224 | 21297 @ `687211` |
| G `DrillFeature::Cancel` | 38725 / `0x60011eb` / 9221 | 21292 @ `6869cc` |
| G `DrillFeature::EndDig` | 38763 / `0x6001211` / 9259 | 21305 @ `687e50` |
| N `Drill::DigPayload` | 71711 / `0x60004d2` / 24838 | 10321 @ `3027cb` |
| N `Drill::CancelPayload` | 71712 / `0x60004d3` / 24839 | 10322 @ `30281d` |
| N `Drill::Crc` | 71713 / `0x60004d4` / 24840 | 10323 @ `302867` |
| N `Drill::EndPayload` | 71714 / `0x60004d5` / 24841 | 10324 @ `302896` |
| N `Drill::ParseSpot` | 71715 / `0x60004d6` / 24842 | 10325 @ `30291a` |
| N `Drill::ParseResult` | 71716 / `0x60004d7` / 24843 | 10326 @ `302a07` |
| G `DrillData::Build` | 38645 / `0x600119b` / 9141 | 21341 @ `689d3c` |
| G `<LoadAsync>d__8::MoveNext` | 38654 / `0x60011a4` / 9150 | 21349 @ `68b530` |
| G `AutoHost::SetButton` | 38807 / `0x600123d` / 9303 | 14132 @ `3ed8f8` |
| G `AutoDrill::Tick` | 36020 / `0x600075a` / 6556 | 27438 @ `894680` |
| G `AutoDrill::Handle` | 36021 / `0x600075b` / 6557 | 27439 @ `895591` |

The verified DrillFeature fields are State+100, Gauge+104, depth4+108, scoopMs+112, ElapsedMs+116, resultMark+120, starts13+124, itemDepth11+140, bottom7+144, Nature+148, Held+152, Stress+160, key+72. DrillParam stores Grade+16, GroundNature+20, DigDepth+24, BarEffArea+28, BarSpeed+32, ScoopMass+36, DrillLife+40. These object offsets describe source evidence; a native adapter uses its own data structure.

## Pure synthetic reference and proof

Save the following block outside Git as `manual-dig-check.py` and run `python3 manual-dig-check.py`. It imports only the Python standard library, performs no I/O except printing, and executes no target code. It models one normal action after the equipment/actor gates, omits visual bob/sounds/window events, and accepts the received spot as an input. `stage=0` after an early exit denotes the adapter's local stop, not an invented inbound acknowledgement. The separately tested active cancellation function only returns a packet decision.

Fixture parameters are deliberately invented: area=80, speed=100, mass=2, depth limit=100, matching-neutral nature, life=100. At 16 ms callbacks, the expected 49 payloads are `0100400c2800ec2a` for an item at 40, `0100501a6400ec66` for empty bottom 100, and `010090071400ec16` for depth limit 20 before item 40. There are 197/422/122 callbacks respectively, with elapsed fields 3136/6736/1936. The first Idle callback contributes no elapsed time. An odd-key empty spot instead cancels: after 32 ms Waiting, `48: 00002000`. These values prove the transcription under this schedule, not accepted server timing.

```python
import math
import struct
from dataclasses import dataclass, replace

def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]

def i32(x):
    return (x + 2**31) % 2**32 - 2**31

def div13(x):
    return (-1 if x < 0 else 1) * (abs(x) // 13)

def words(*xs):
    return struct.pack('<' + 'H' * len(xs), *(x & 65535 for x in xs))

@dataclass(frozen=True)
class Param:
    area: int
    speed: int
    mass: int
    limit: int
    nature: int = 0

@dataclass(frozen=True)
class Spot:
    actor: int
    x: int
    y: int
    unusable: int
    bottom: int
    item: int
    nature: int
    key: int
    stress: int

def spot_decode(b):
    actor, x, y, branch = struct.unpack_from('<HHHB', b)
    if branch:
        unusable, key, stress = struct.unpack_from('<hHB', b, 7)
        assert len(b) == 12
        return Spot(actor, x, y, unusable, 0, 0, 0, key, stress)
    bottom, item, nature, key, stress = struct.unpack_from('<BBBHB', b, 7)
    assert len(b) == 13
    return Spot(actor, x, y, 0, bottom * 5, item * 5, nature, key, stress)

def result_decode(b):
    assert len(b) == 7
    return struct.unpack('<BHI', b)

def end_payload(p, key, starts, elapsed, depth):
    crc = (p.area ^ (p.speed << 1) ^ key ^
           ((p.mass ^ (depth << 3) ^ (starts << 7)) << 5)) & 65535
    return words(starts, elapsed, depth, crc)

@dataclass(frozen=True)
class State:
    stage: int = 1
    held: bool = False
    gauge: float = 0.0
    depth4: float = 0.0
    scoop_ms: int = 0
    elapsed: int = 0
    starts13: int = 0
    life: int = 100
    begun: bool = False
    key: int = 0
    stress: int = 0

def tick(old, p, spot, dt_ms, moving=False):
    assert math.isfinite(dt_ms) and dt_ms >= 0
    dt = min(int(dt_ms), 2**31 - 1)
    s = vars(old).copy()
    if s['stage'] == 1 and spot is not None and not moving:
        if spot.unusable > 0:
            return replace(old, stage=0, held=False), None
        if spot.key & 1 and spot.item <= 0:
            return replace(old, stage=0, held=False), (48, words(div13(old.starts13), old.elapsed))
        s.update(stage=2, begun=True, key=spot.key & 65534, stress=spot.stress)
    stage = s['stage']
    marker = f32(200 - p.area)
    k = float.fromhex('0x1.555556p-3')

    def decay():
        d = f32(s['depth4'] * f32(.25))
        s['depth4'] = 0.0 if d <= f32(.35) else f32(f32(d + f32(-.35)) * 4)

    if stage == 2:
        decay()
        if s['held']:
            s['starts13'] = i32(s['starts13'] + 13)
            s['life'] = max(0, i32(s['life'] - (2 if p.nature and p.nature != spot.nature else 1)))
            s['stage'] = stage = 3
    if stage == 3:
        decay()
        s['gauge'] = f32(float(f32(dt)) * k * p.speed / 100 + s['gauge'])
        if s['gauge'] >= marker:
            s['stage'] = 5
        elif not s['held']:
            s['stage'] = 4
    elif stage == 4:
        decay()
        x = float(i32(-dt)) * k * p.speed / 100 + s['gauge']
        s['gauge'] = f32(x)
        if x <= float.fromhex('0x1p-150'):
            s.update(gauge=0.0, stage=2)
        elif s['held']:
            s['stage'] = 3
    elif stage in (5, 6):
        s['scoop_ms'] = i32(s['scoop_ms'] + dt)
        delta = dt if stage == 5 else i32(-dt)
        x = float(delta) * k * p.speed / 100 + s['gauge']
        s['gauge'] = f32(x)
        if s['scoop_ms'] >= 120:
            s['scoop_ms'] = i32(s['scoop_ms'] - 120)
            s['depth4'] = f32(f32(f32(p.mass) * 4) + s['depth4'])
        if stage == 5:
            if x >= float.fromhex('0x1.8fffffp+7'):
                s.update(stage=2, gauge=0.0, depth4=0.0)
            elif not s['held']:
                s['stage'] = 6
        elif s['gauge'] < marker:
            s['stage'] = 4
        elif s['held']:
            s['stage'] = 5
    if stage in (1, 3, 4, 5, 6):
        s['elapsed'] = i32(s['elapsed'] + dt)
    packet = None
    if 2 <= s['stage'] <= 6:
        depth = f32(s['depth4'] * .25)
        target = spot.bottom if spot.item <= 0 else spot.item
        limit = target if depth >= f32(target) else p.limit if depth >= f32(p.limit) else None
        if limit is not None:
            s.update(depth4=f32(f32(limit) * 4), stage=7)
            packet = (49, end_payload(p, s['key'], div13(s['starts13']), s['elapsed'], int(f32(s['depth4'] * .25))))
    return State(**s), packet

def cancel(s, active=True, targeting=False, pending=False):
    if targeting or active and s.stage == 7:
        return None
    if active:
        return 48, words(div13(s.starts13), s.elapsed)
    return (48, words(0, 0)) if pending else None

def normal_button(s, phase):
    if phase == 'begin':
        return (3, phase) if s.stage == 2 else (None, 'charge') if s.stage == 3 else (None, phase)
    if phase == 'charge':
        return (0, 'drill') if s.stage == 5 else (3, phase)
    if s.stage == 7:
        return 0, 'wait'
    return None, phase

def controlled_tick(s, p, spot, dt, phase):
    s, packet = tick(s, p, spot, dt)
    button, phase = normal_button(s, phase)
    if phase == 'drill' and button is None:
        gauge_percent = f32(f32(s.gauge * 100) / 200)
        marker_percent = f32((200 - p.area) * 100.0 / 200)
        button = 3 if float(gauge_percent) <= float(marker_percent) + 10 else 2
    return replace(s, held=(button == 3)) if button is not None else s, packet, phase

def verify():
    p = Param(80, 100, 2, 100)
    sp = spot_decode(bytes.fromhex('07000a00140000140803351219'))
    assert sp == Spot(7, 10, 20, 0, 100, 40, 3, 0x1235, 25)
    assert spot_decode(bytes.fromhex('07000a00140001ffff351219')).unusable == -1
    assert result_decode(bytes.fromhex('030000efcdab89')) == (3, 0, 0x89abcdef)
    assert result_decode(bytes.fromhex('02000000000000')) == (2, 0, 0)
    assert words(10, 20) + bytes([2]) == bytes.fromhex('0a00140002')
    assert end_payload(p, 0x1234, 1, 1234, 40).hex() == '0100d2042800ec2a'
    assert end_payload(p, 0x1234, 1, 1235, 40)[6:] == bytes.fromhex('ec2a')
    s = State()
    s, pkt = tick(s, p, None, 16.9)
    assert (s.elapsed, pkt) == (16, None)
    s, pkt = tick(s, p, sp, 16, moving=True)
    assert (s.stage, s.elapsed, pkt) == (1, 32, None)
    s, pkt = tick(s, p, sp, 16)
    assert (s.stage, s.elapsed, s.key, s.stress) == (2, 32, 0x1234, 25)
    assert tick(replace(s, stage=7), p, sp, 999)[1] is None
    assert tick(State(), p, replace(sp, unusable=1), 16)[0].stage == 0
    for stage in range(1, 7):
        assert cancel(replace(s, stage=stage, starts13=26, elapsed=70000)) == (48, bytes.fromhex('02007011'))
    assert cancel(replace(s, stage=7)) is None
    assert cancel(s, active=False, pending=True) == (48, b'\0' * 4)
    assert cancel(s, targeting=True) is None
    assert words(div13(-13), -1) == b'\xff' * 4
    assert i32(2147483647 + 13) == -2147483636
    assert end_payload(p, 0x1234, 1, 1234, 40) == end_payload(p, 0x1234, 65537, 66770, 65576)
    failed, _ = tick(replace(s, stage=5, held=True, gauge=199.0, depth4=100), p, sp, 16)
    assert (failed.stage, failed.gauge, failed.depth4) == (2, 0.0, 0.0)
    fall, _ = tick(replace(s, stage=4, gauge=0.1), p, sp, 16)
    assert (fall.stage, fall.gauge) == (2, 0.0)
    one, _ = tick(replace(s, stage=6, gauge=180, scoop_ms=0), p, sp, 360)
    assert (one.scoop_ms, one.depth4) == (240, 8.0)
    coast, _ = tick(replace(s, stage=6, held=True, gauge=120), p, sp, 0)
    assert coast.stage == 5
    below, _ = tick(replace(s, stage=6, held=True, gauge=f32(119.99)), p, sp, 0)
    assert below.stage == 4
    overshoot, pkt = tick(replace(s, stage=6, gauge=160, depth4=160), replace(p, limit=20), sp, 0)
    assert struct.unpack('<4H', pkt[1])[2] == 40
    decay, _ = tick(replace(s, held=True, depth4=8), p, sp, 0)
    assert decay.depth4 == f32(f32(f32(2 - f32(.35)) - f32(.35)) * 4)
    worn, _ = tick(replace(s, held=True, life=1), replace(p, nature=4), sp, 16)
    assert worn.life == 0
    assert tick(State(elapsed=32), p, replace(sp, item=0), 16)[1] == (48, words(0,32))
    for item, cap, depth, expected in [(40,100,40,'0100400c2800ec2a'),(0,100,100,'0100501a6400ec66'),(40,20,20,'010090071400ec16')]:
        s, phase, packets = State(), 'begin', []
        for frame in range(2000):
            s, packet, phase = controlled_tick(s, replace(p, limit=cap), replace(sp, item=item, key=sp.key & 65534), 16, phase)
            if packet:
                packets.append(packet)
                break
        assert len(packets) == 1 and packets[0][0] == 49
        assert packets[0][1].hex() == expected
        assert struct.unpack('<4H', packets[0][1])[2] == depth
        assert s.stage == 7 and s.starts13 == 13 and s.life == 99
        print('synthetic', 'item', item, 'limit', cap, 'frames', frame + 1, 'elapsed', s.elapsed, 'payload', packets[0][1].hex())
    print('PASS: pure manual controller, wire, cancellation and boundary fixtures; no target code or server used')

if __name__ == '__main__':
    verify()
```

Primary-source verification passed **38** metadata/module/element/body joins, **29** field offsets, all eight stage defaults, the dispatch branch table, the odd-key branch, the completion opcode, and the scoop boundary instruction bytes. The source hashes and both immutable prior-report readbacks passed. The pure reference passed normal completion, early exit, wait/movement, cancellation stages, overflow reset, decay, scoop, life, and integer/serializer boundary fixtures. No product build or live game check was appropriate to this one-file source report. Prove It Works shaped the checks: the delivered Markdown's code is extracted and run, and publication is checked against the actual remote commit/blob and issue comment.

Authoring scope: `research/manual-dig-control`, based on `78e604d73b7c59b4199f47b321884a036c7f7278`. Account-free analyses and public caches supplied by the parent were read-only. New analysis remained in `/private/tmp/opentrickster-manual25.O2r15S`. Checkout AGENTS, tracker/domain pointers, research, and Prove It Works skills were read. No helpers were spawned, as explicitly requested. Throughput checkpoint: n/a, source investigation. The commit/comment readback records the delivered revision; no PR or merge belongs to this task.

[I25]: https://github.com/oDestroyeRo/opentrickster/issues/25
[W]: https://tr-play.playall.in.th/Build/d9a4c4fd2737e740b82e1321839e352f.wasm.gz
[D]: https://tr-play.playall.in.th/Build/6135c4139b62e558bb4c755b266f5fa2.data.gz
[R18]: https://github.com/oDestroyeRo/opentrickster/blob/df5fe0a5e8e45072845538fc74967a1757b6d9d7/docs/research/current-build-protocol-revalidation.md
[R14]: https://github.com/oDestroyeRo/opentrickster/blob/cb587e235b113aa26b04db018c31f1e26510f5bd/docs/research/target-state-completion.md
[OLD]: https://github.com/oDestroyeRo/opentrickster/blob/3a6c421d903483f1c977f6006fe01476d8da3223/docs/research/target-state-message-layouts.md
