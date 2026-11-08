# Native own-effect lifecycle

Issue [#32](https://github.com/oDestroyeRo/opentrickster/issues/32), part of [#21](https://github.com/oDestroyeRo/opentrickster/issues/21). This report recovers received effect state needed to maintain native gun reload speed. Gun serialization, definitions, and shot attribution remain in [gun control][R29]. Inspection uses public build **8502ebbf2fc9033d2fba50d02fd92002**. It is static, with no account activity or target execution. Actual character effects and shot acceptance remain [#23](https://github.com/oDestroyeRo/opentrickster/issues/23).

A complete received **own type-1 appearance on logical 8 or 9** supplies the positive attached-effect baseline. The normal installer replays only StartSet 8/9 into this path. An explicitly empty EsBlock proves that this attached list is empty. FastLoad is **ESA type 1476**, with one F32 value. Its U16 instance key is separate from the gun key. Learned skills 271, key 512, elapsed time, and missing effect traffic do not supply an active-effect baseline. [W] (f45296, f20632, f20627, f10388, f17633)

The inspected FastLoad path has no in-place upsert or TTL. Received starts append instances. Appearances replace one actor's attached list. A separate received 307 channel creates detached local state that survives that replacement. Duplicate keys, overlapping FastLoads, malformed input, and unresolved detached state cannot become a default speed.

The checkout started clean on research/native-effect-lifecycle at **fe3458b6d7ef3f191dfcaaa46d7ee75df30b5b23**, with its tree verified identical to merged main **62ae793384aa61db96faa3ea1fbf855d3fd76ba4**. Only this report changes. Root owns integration and merges.

## Current primary artifacts

Cookie-free public GETs of the [page](https://tr-play.playall.in.th/), [configuration](https://tr-play.playall.in.th/spinel-config.json), and [version](https://tr-play.playall.in.th/version) were repeated during this investigation. Config and version identify the build above. The page names the pinned files. Config SHA-256 is 084900013e6edde0ce0eca286dc7ac884a86da2ece1bae361ce0671852092602; version is 9ce11c71ef039ffbe6bede54dfcf5b27afb887d44b2a79f57f959638e2e771b0; fetched HTML is ccb9859c7a626b0d5e419d7ebe2a1a30b652293be27c70773fc6793ac6c4101d.

Only the three explicitly permitted files were opened in the mixed native17 directory. Both .gz names held decoded bytes, identified by magic before hashing. The UnityWebData directory locates metadata at offset **16748911**, size **9343896**, identical to the permitted standalone metadata.

| Artifact | Decoded bytes | SHA-256 |
| --- | ---: | --- |
| [WASM][W] | 43136343 | 381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404 |
| [Unity data][D] | 27724487 | fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474 |
| Embedded Il2CppData/Metadata/global-metadata.dat | 9343896 | 7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade |

Metadata version is 108. Method rows are 30 bytes, with name U32 at 0, declaring type U16 at 4, and token U32 at 18. Type rows are 66 bytes, field rows 10 bytes, and WASM has 490 imported functions. A token RID minus one indexes the owning module's pointer array, yielding an element slot, then function index, then original code body. No target module was instantiated.

| Module | Registration | Method count | Pointer array |
| --- | --- | ---: | --- |
| Net | 0x212548 | 4791 | 0x392c70 |
| Game | 0x21251c | 18796 | 0x37f5a0 |
| UI.Pc | 0x2125a0 | 16606 | 0x3a9600 |

Below, m is a zero-based metadata method, f is an original WASM function, and @ is an original decoded WASM file offset. Object offsets are not packet positions. Field joins use type pointers at 0xd75a0 and field-offset pointers at 0x35d850. The appendix and evidence.json retain method tokens, element slots, body lengths/hashes, fields, and instruction pivots.

## Received grammar and dispatch

These are incoming logical GameServer opcodes after current directional mapping and frame decoding. Multi-byte values are little-endian. The reference requires complete reads. The target sometimes substitutes zero for short input or ignores extra bytes; these fallbacks do not establish received zero values.

| Opcode | Payload | Timing-relevant dispatch |
| --- | --- | --- |
| 224, 292 | Fourteen-byte header below, then effect body | CombatLink.Receive creates SkillHits; SkillsApi.ClaimSkillHits parses the header and calls installed EsaRunner.Play. Raw 224 also reaches SkillLink.Receive for Aux=0 and length>=14. Apply one source event once. |
| 307 | U16 instance, U16 ESA type, remaining body | Host.State constructs a header and calls StartState/BeginState. No actor field. Type 1476 consumes one F32. |
| 280 | U16 owner actor, U16 instance | Combat.ParseMessage creates StateEnd; the UI combat callback invokes OnStateEnd. Require all four bytes. |
| 15 | U16 key prefix | The ESA subscription sends Message(key,15,null). Inherited FastLoad OnMessage queues ending. This opcode also participates in actor/object removal elsewhere; equal numbers do not make actor IDs and effect keys interchangeable. |
| 16 | U16 key prefix | Sends local message 30019. The base handler returns true without ending FastLoad. |
| 893, 921, 924, 1395, 1396 | U16 instance, remaining class-specific body | InstancePacket strips the prefix and invokes the class handler. FastLoad inherits the base handler and ignores these updates. They are not replacement FastLoad floats. |
| 1424 | U16 object actor, remaining class-specific body | Separate ObjectPacket route. It is not a generic FastLoad update. |

[W] (f11277@336b5c–336b8a, f11272, f13230@3ac178–3ac21a, f23769@75a4a3–75a4b4, f45420, f43906/43907/43909–43916, f20648–20650, f18039)

The 224/292 header is:

~~~text
0  U16 Invoker
2  U16 Target
4  U16 Instance
6  U16 Esa
8  I8  V14
9  I8  EsClass
10 U16 Skill
12 U16 V04
14 effect payload through frame end
~~~

ParseEsaHeader reads bytes 8/9 as U8 then sign-extends them. Invoker is the invoking actor; Target is the recipient used for attachment. Instance is the effect lifetime key. Esa selects the factory; Skill participates in its override lookup. V14 and V04 remain uninterpreted. **V04 is not a proved duration.** Proc is a separate local header field populated by the combat path, with no byte in this header. [D] (type 12605; W: f11272, f23769)

Aux=0 and completed nonzero-Aux bundles are different delivery paths to the same header. Preserve original ingress, connection, zone, and actor incarnation through bundling. Pending or malformed relevant bundles make timing unusable. This report does not replace the existing Aux dispatcher with arrival-order guesses. Do not apply raw and grouped forms twice. [W] (f13230–f13232; R18, R29)

### Snapshot shape and positive emptiness

The owning appearance parser and full PlayerBody are documented in [state completion][R14]; their current method joins were rechecked. The common prefix contains U16 actor/type/kind, U8 hair/job, I16 level, U32 flags, four U16 coordinates, Cstr name, U8 state/angle/speed, U32 object state, EsBlock, discarded U32, and the typed body. A positive own baseline requires a complete type-1 PlayerBody and known own identity. An ID with a partial tail is insufficient. The reference below parses the complete own-player form.

EsBlock.Read consumes U8 presence. Zero returns an explicitly empty list. Otherwise it reads eight groups 0–7, each with U8 entry count. Each entry is:

~~~text
U16 Esa, U16 Instance, U8 V14, U16 Skill, U16 V04, descriptor body
~~~

Snapshot V14 remains unsigned. The group becomes Header.EsClass in StartStates. Descriptor 0 consumes U8, 1 consumes U16, 2/3/4 consume four bytes, and 5 consumes bytes through NUL. Descriptor 3 for type 1476 becomes F32 in FastLoad's reader. Descriptor 3 on type 1487 belongs to ESASettleDownDo; a float body alone does not identify gun speed. The complete 47-key table is ES in the executable reference. Current constructor immediates and copied RVA arrays were re-extracted. [W] (f10388, f10391@30614b–306174; D: 0x4e6f48, 0x4e7450, 0x4e7380, 0x4e73c0)

Type 1425 overrides the table: U16 n, n U32s, U32 m, m U16s, then one extra U16 iff n!=0. The strict decoder bounds counts by available bytes. For an ESA absent from the descriptor table, the target consumes zero body bytes. This is not a length-delimited unknown extension. The reference quarantines unclassified snapshot types outside the 47 shapes and special 1425. It is deliberately narrower than the full factory registry. A native implementation can widen its known-zero-body set with a current factory/consumer proof.

SkillEsaView.Install enumerates StartSet, tests (opcode & ~1)==8 at 0xed8959–ed8966, parses appearances, and calls OnAppear at 0xed8a91. This is its only initial effect replay. Live 307 is registered separately at 0xed8188–ed818f; it is not replayed from StartSet here. OnAppear skips **opcode 7 when actor==MyId**. Complete own 8/9 is therefore the proved normal startup baseline, including an explicit empty form. No initial-empty effect baseline on 271/512 is assumed. [W] (f45296, f20632)

## Type identity, value, and lifetime

EsaRegistry.R17 stores key 1476 at 0x55c14e using callback m50027. That callback allocates ESAFastLoad, type 8683, through usage cell 3244592. EsBlock key 1476 has descriptor [3]. Create(esa,skill) checks SkillLooks before the ordinary registry. Its current special pair is (1079,3012), not a FastLoad override. The ordinary table initializes R00 through R20 in order. [W] (f17633, f17978, f17639, f17640, f17637)

Both Init(reader,invoker,target) and InitState(reader,owner) read one little-endian F32 value v. Let F round to IEEE binary32:

~~~text
LoadSpeed = F(1.0 + double(F(100.0f - v)) / 100.0)
~~~

Division and addition are binary64. This corrects the compressed arithmetic description in R29. Subtraction and the stored result are binary32. IEsaHost.SetLoadSpeed is slot 63; its UI implementation forwards to GunFlowing.SetLoadSpeed, storing F32 at +76. There is no clamp in the setter. The reference rejects nonfinite or nonpositive results as adapter policy instead of substituting 1. The real constructor starts at 1; FastLoad.OnEnd writes 1. [W] (f16980@50ea4f–50eaee, f16982@50ecc2–50ed5c, f16983, f45412, f22131)

FastLoad's current vtable binds Init m52314, InitState m52315, OnEnd m52317. Update, Tick, and OnMessage are inherited. The class adds no fields. Base State starts at -1; FastLoad never installs a timed state. No received TTL, local expiry, refresh packet, or duration inferred from V04 is justified. The Esa interface-call census finds slot 63 only in these three methods. This is a reload-speed consumer proof, not a classification of unrelated stat effects as speed. [D] (type 8683, vtable 48710, 13 entries; W: f18038, f18061, f16980/16982/16983)

### Attached and detached state

| Event | Current client behavior | Native consequence |
| --- | --- | --- |
| 224/292 start | Create effect, use nonzero Instance as Id, Persistent=(Instance!=0), run Init, append on success. Attach to recipient inventory if recipient exists with actor Type 1/3 and EsClass is neither -1 nor 6. | Associate by recipient, never invoker. Preserve attachment knowledge. |
| Complete 8/9 | StartStates removes that actor's old inventory, runs EndNow on old entries, then creates persistent states with InitState and attaches them. | Stage and replace that actor's attached list after the complete parse. This is a proved way a value changes. |
| 307 | Header contains Instance/Esa; remaining fields are CLR defaults. BeginState uses owner 0, Persistent=true, and appends to running without AttachState. | Local detached channel with no actor field. Owner 0 is a constructed value, not a received actor or substitute for Me. Preserve across own appearances. |
| Repeated live instance | Start/BeginState has no existing-instance lookup or upsert. | Deduplicate repeated delivery by ingress. Distinct events reusing an active ID invalidate timing. |
| Zero instance | Generate a local ID in 32768–65535, wrapping after 65535. BeginState marks it persistent; live Start does not. | Generated identity is not a recovered server key. The strict reference blocks zero instead of fabricating a wire-correlatable lifetime. |

[W] (f20617@649437–6494bd, f20619, f20626, f20627, f20620, f18037, f20624)

A new FastLoad writes the scalar; it does not multiply or add. Ending any FastLoad writes 1 even when another remains. Overlapping FastLoads therefore lack a sound stacking/recomputation rule. Quarantine timing instead of selecting the newest float and guessing what an older end means.

### Ending, persistence, and reset

For 280, OnStateEnd(owner,instance) defers the pair while Play is in progress. Otherwise EndState searches running, then removals, for the first Persistent && Id==instance. **It does not compare StateOwner with packet owner.** Owner lookup chooses immediate EndNow versus queued End. Both paths call RunEnd, virtual OnEnd, and detachment. Native code needs additional owner/incarnation validation before reducing a known own lifetime. [W] (f20621, f20623, f20661, f20628, f20646, f18045)

For 15, Message searches by runtime Id without a Persistent check, skips already-ending effects, then invokes OnMessage. FastLoad inherits: message 15 marks Ending and queues removal; 30019 returns true without ending. The UI maps S16 to 30019. The generic instance channels above do not update FastLoad. [W] (f20648@64cf7f–64cf91, f18039, f43906, f43907)

Own faint 43 reaches OnFaint(actor,flags). Flags bit 1024 skips this cleanup. Otherwise it ends that actor's attached effects whose EsClass is 0 or 1. FaintEnds=[0,1]; NotState=[-1,6]. Unknown flags/class do not prove that an effect ended. Detached 307 is outside that inventory. Advance separately checks actor existence and calls OnOwnerGone, removing that inventory and queuing attached effects for ending. Disappearance is not a received empty baseline for the next incarnation. [W] (f43916, f20630, f20631, f20644, f20660)

Zone cleanup queues every running effect, drains removals, clears inventories/visual state, and resets carry. UI disposal also calls OnZone. FastLoad.OnEnd can write 1 during cleanup. This is local cleanup, not evidence that the next zone/session has no server effect. Native zone changes, reconnects, own-actor replacement, and incomplete ingress invalidate timing proof. A fresh complete own 8/9 is required before absence is asserted again. [W] (f43905, f43917, f20659)

### Client-global side effect

The public code has a material ownership limitation. FastLoad.Init checks Find(target), but not target==MyId. InitState does not check owner. Host.SetLoadSpeed obtains local GunFlowing through IPlayContext and writes it without checking runner scope. Host.Find can return other actors. Thus a foreign FastLoad can affect the client's local scalar. Static evidence does not establish whether the server sends such combinations to nearby clients. [W] (f16980, f16982, f45412, f45423, f45321)

Do not convert another actor's F32 into authoritative own state. Keep its recipient separate and invalidate timing when this known global side effect conflicts with own evidence. An own appearance cannot repair an unaccounted foreign or detached FastLoad. The reference keeps global ambiguity until scope reset. This is conservative native policy, not a source-level own-recipient guard.

## Reduction contract and safe recovery

Wire vocabulary stays private to Net. The reference uses this compact internal shape:

~~~text
Scope = connection generation + zone generation + own actor incarnation
Effect = ESA type + instance + group/class + V14 + skill + V04 + raw body
Lifetime = Scope + instance + local generation + recipient channel
           + original ingress + attached/persistent flags + timing value
Knowledge = Unknown(reason) | Known(received baseline and lifetimes)
~~~

There is **no wire generation or nonce** in these messages. Capture a lifetime generation when queuing local callbacks; an old callback must not bind to whatever now occupies that U16 key. Preserve original receive ordering and deduplicate ingress. An unexplained unique event behind the reduced frontier invalidates proof. A later raw packet with a recycled key cannot identify a hidden older generation; do not invent one.

An explicit empty own 8/9 establishes speed 1 only with a complete appearance, matching own incarnation, settled source ordering, and no detached/global uncertainty. The adapter must observe initialization or reset and maintain complete relevant ingress from that boundary. Attaching midway to a session does not prove the absence of earlier detached 307 state; even an empty own snapshot cannot repair that missing history. One valid FastLoad establishes its value under the same conditions. A known unrelated effect does not become speed. Unknown state or descriptors do not mean absence.

A **280 or S15 ending allows speed 1** only when all conditions hold:

1. A positive own baseline exists in the current scope. Start followed by end before baseline does not prove absence of other effects.
2. The end resolves to one current tracked lifetime after its start. Queued callbacks match its generation. For 280 validate the received owner against a known lifetime recipient. A detached 307 lifetime has no received owner to compare; it requires a unique globally tracked key in the current scope. For S15 the instance resolves without namespace collision.
3. Relevant Aux work and queued endings are settled. No duplicate active key, foreign/global conflict, malformed value, unknown lifetime, or unclassified detached effect remains.
4. No FastLoad remains. With overlapping FastLoads the unconditional OnEnd write is not a sound recomputation rule.

A fully decoded, uniquely tracked 307 FastLoad can be retired by a matched 280 or S15. With an existing valid own baseline and no other blockers, the reference returns speed 1. An unknown 307 cannot be repaired by own appearance because it was not in that inventory. The reference preserves detached/collision/global taint through own snapshots and requires a scope reset plus fresh baseline. This includes unattached live starts with class -1 or 6. More permissive recovery needs additional proof.

A complete new own snapshot can repair uncertainty confined to its old attached list, such as an unsupported attached start. Duplicate IDs and overlapping FastLoads remain quarantined in this conservative reference. It preserves detached state. Ending one unknown/duplicate lifetime is not a blanket clear of uncertainty. Zone/reconnect resets to Unknown; the next complete baseline can establish a value.

**Known current speed is only one gun input.** If an unknown interval made Load/Power evolution untrackable, recovering speed does not recover those accumulators or imply Ready. Reconstruct normal gun state through proved transitions before scheduling a shot. Movement/hit persistence remains the separate 271/523 contract for learned skills 3010/3011 in R29. Faint persistence above is a different rule. Equipment, ammo, key 512, definitions, frame time, pending shots, and target attribution remain separate inputs.

## Reproduction and checks

New analysis is outside Git at **/private/tmp/opentrickster-effects32.i28HEE**. Primary copies are wasm.bin, data.bin, metadata.bin; cached sources remain untouched. parse.py reconstructs metadata/module/element/body/field joins. scoped.py makes a disassembly-only copy with selected unchanged bodies and nonexecuted stubs elsewhere. WABT 1.0.39 at /tmp/opentrickster-issue13/tools/node_modules/wabt/bin/wasm-objdump decodes it; offsets translate back to original bodies. read.py annotates calls/types and suppresses arbitrary literal bytes. extract.py re-extracts all 47 descriptor shapes and RVA data.

The pure **reference.py** below is a parser/reducer. **test-reference.py** checks empty/active baselines, starts, snapshot value changes, endings, other actors, stale generations/scopes, late raw ingress, duplicate IDs, detached state, unknown types, faint persistence, no expiry, all 47 shapes, special 1425, and truncation. Values/IDs are invented fixture data, never runtime defaults. **verify-source.py** checks the originals and source joins. **negative-controls.py** verifies deliberate mismatches are rejected.

~~~sh
python3 /private/tmp/opentrickster-effects32.i28HEE/parse.py
python3 /private/tmp/opentrickster-effects32.i28HEE/scoped.py
python3 /private/tmp/opentrickster-effects32.i28HEE/extract.py
python3 /private/tmp/opentrickster-effects32.i28HEE/verify-source.py
python3 /private/tmp/opentrickster-effects32.i28HEE/test-reference.py
python3 /private/tmp/opentrickster-effects32.i28HEE/negative-controls.py
~~~

Results: **134 method joins**, **50 fields**, **60 instruction pivots**, **47 descriptors**, FastLoad's inherited vtable, the Esa slot 63 census, three artifact hashes, and embedded metadata identity passed. **369 synthetic checks** passed. Five controls rejected changed WASM, wrong FastLoad method slot, wrong LoadSpeed offset, wrong descriptor, and wrong vtable index. These are static/source and transcription checks, not live acceptance or independent review. evidence.json records original-body SHA-256 values.

The embedded reference is byte-matched and rerun after extraction. Publication checks the single-report diff, whitespace, remote SHA, and report bytes read back from that SHA. This delivery contains no application build, PR, merge, watch, credential/private-data access, target execution, or account operation.

### Executable parser and reducer

Save this block outside Git as reference.py. Python 3.9 or later is sufficient. It exposes unknown timing and never computes gun Ready. The synthetic tests are in the named scratch checker. This small reference retains observed foreign keys until scope reset, even after a possible foreign retirement, so later cross-actor reuse stays conservative. A production reducer may retire them only with its full actor/lifetime proof. It also retains ingress marks for the whole scope; a bounded ingress journal must invalidate proof on unresolved overflow.

~~~python
"""Synthetic-only contract transcription. No networking or target execution."""
from __future__ import annotations
from dataclasses import dataclass
from enum import Enum
import math, struct

ES = {
1118:(2,),1153:(0,3),1186:(4,2),1192:(1,4),1208:(0,),1248:(0,),
1267:(1,4),1295:(4,1),1309:(1,1),1316:(1,4,2),1333:(2,2),1353:(1,4,2),
1357:(4,1,2),1358:(1,4,2),1361:(2,2),1369:(1,4),1371:(1,4),1373:(1,4),
1375:(1,4),1377:(1,4),1380:(4,1),1386:(2,2),1388:(5,5),1390:(0,),1394:(2,2),
1423:(2,2,2),1476:(3,),1479:(1,2,2),1483:(2,2,2),1487:(3,),1492:(1,2,2),
1494:(0,),1524:(2,),1525:(2,),1526:(2,),1527:(2,),1529:(2,),1531:(2,),
1533:(2,2),1534:(2,),1537:(2,),1540:(2,),1541:(2,),1542:(2,),1543:(2,),
1545:(2,),1547:(2,)}
FAST=1476
class Invalid(ValueError): pass
class Reader:
    def __init__(self,b): self.b,self.p=b,0
    def take(self,n):
        if n<0 or n>len(self.b)-self.p: raise Invalid('truncated')
        at=self.p; self.p+=n; return self.b[at:self.p]
    def get(self,f): return struct.unpack('<'+f,self.take(struct.calcsize('<'+f)))[0]
    def cstr(self):
        at=self.b.find(b'\0',self.p)
        if at<0: raise Invalid('missing NUL')
        return self.take(at+1-self.p)
    def end(self):
        if self.p!=len(self.b): raise Invalid('unclassified tail')
def f32(v):
    try:return struct.unpack('<f',struct.pack('<f',v))[0]
    except OverflowError:return math.copysign(math.inf,v)
def load_speed(body):
    if len(body)!=4: raise Invalid('FastLoad requires one F32')
    v=struct.unpack('<f',body)[0]; speed=f32(1.0+float(f32(100.0-v))/100.0)
    if not math.isfinite(v) or not math.isfinite(speed) or speed<=0: raise Invalid('unusable speed')
    return speed
@dataclass(frozen=True)
class Entry:
    esa:int; instance:int; group:int; v14:int; skill:int; v04:int; body:bytes
    @property
    def supported(self): return self.esa in ES or self.esa==1425

def block(r):
    rows=[]
    if r.get('B')==0:return rows
    for group in range(8):
        for _ in range(r.get('B')):
            esa,key,v14,skill,v04=struct.unpack('<HHBHH',r.take(9)); start=r.p
            if esa==1425:
                n=r.get('H');r.take(4*n);count=r.get('I');r.take(2*count)
                if n:r.take(2)
            elif esa in ES:
                for kind in ES[esa]:
                    if kind==5:r.cstr()
                    else:r.take({0:1,1:2,2:4,3:4,4:4}[kind])
            else:raise Invalid('unclassified snapshot type')
            rows.append(Entry(esa,key,group,v14,skill,v04,r.b[start:r.p]))
    return rows
A=(1,2,4,32,64,128,256,512,1024,2048,4096,8192,16384,32768,65536,262144,524288)
B=(1,2,4,4096,8192,524288)
def appearance(b):
    r=Reader(b); actor=r.get('H');kind=r.get('H');r.take(18);r.cstr();r.take(7)
    rows=block(r);r.take(4)
    if kind!=1:raise Invalid('reference baseline requires PlayerBody')
    r.take(5);mask=r.get('B')
    if mask&128:raise Invalid('unknown PlayerBody mask')
    for bit in (1,2,4):
        if mask&bit:r.cstr()
    if mask&8:r.take(1);r.cstr()
    if mask&16:r.cstr()
    for bits in (A,B):
        flags=r.get('I')
        if flags&~sum(bits):raise Invalid('unknown equipment mask')
        r.take(4*sum(bool(flags&bit) for bit in bits))
    r.take(1);r.cstr();r.cstr();r.take(8)
    if mask&32:
        n=r.get('I')
        if n>=0x20000000:raise Invalid('overflow')
        r.take(n*4)
    r.take(4)
    if mask&64:r.cstr()
    title=r.get('H');r.take(11)
    if title in (4,6):r.cstr()
    r.take(1);r.end();return actor,rows
@dataclass(frozen=True)
class Event:
    kind:str; actor:int|None=None; entry:Entry|None=None; instance:int|None=None; invoker:int|None=None; rows:tuple=()
def decode(op,b):
    r=Reader(b)
    if op in (8,9,7):
        actor,rows=appearance(b);return Event('appearance'+str(op),actor,rows=tuple(rows))
    if op in (224,292):
        invoker,actor,key,esa,v14,group,skill,v04=struct.unpack('<HHHHbbHH',r.take(14))
        entry=Entry(esa,key,group,v14,skill,v04,r.take(len(b)-r.p))
        if esa==FAST:load_speed(entry.body)
        return Event('start',actor,entry,invoker=invoker)
    if op==307:
        key,esa=struct.unpack('<HH',r.take(4));entry=Entry(esa,key,0,0,0,0,r.take(len(b)-r.p))
        if esa==FAST:load_speed(entry.body)
        return Event('local',None,entry)
    if op==280:
        actor,key=struct.unpack('<HH',r.take(4));r.end();return Event('end',actor,instance=key)
    if op in (15,16):
        key=r.get('H');r.end();return Event('end_key' if op==15 else 'ack_key',instance=key)
    if op in (893,921,924,1395,1396):
        return Event('message',instance=r.get('H'))
    raise Invalid('unsupported channel')
class Knowledge(Enum): UNKNOWN='unknown'; KNOWN='known'
@dataclass(frozen=True)
class Lifetime:
    actor:int|None; entry:Entry; generation:int; born:int; attached:bool; persistent:bool
class OwnEffects:
    """Requires complete ordered ingress from observed scope initialization/reset."""
    def __init__(self,scope,own):
        self.scope,self.own=scope,own;self.baseline=None;self.serial=0;self.frontier=0
        self.active={};self.seen=set();self.foreign_keys=set();self.own_bad=False;self.global_bad=False;self.ready=False
    @property
    def speed(self):
        if self.baseline is None or self.own_bad or self.global_bad:return None
        fast=[v for v in self.active.values() if v.entry.esa==FAST and v.actor in (None,self.own)]
        if len(fast)>1:return None
        return load_speed(fast[0].entry.body) if fast else 1.0
    @property
    def knowledge(self):return Knowledge.UNKNOWN if self.speed is None else Knowledge.KNOWN
    def taint(self,global_scope=False):
        if global_scope:self.global_bad=True
        else:self.own_bad=True
        self.ready=False
    def apply(self,scope,mark,op,body):
        if scope!=self.scope or mark in self.seen:return
        if mark<=self.frontier:
            self.taint(True);return
        self.seen.add(mark);self.frontier=mark;self.ready=False
        try:e=decode(op,body)
        except Invalid:self.taint(True);return
        if e.kind.startswith('appearance'):
            if e.actor!=self.own:
                keys={x.instance for x in e.rows}
                if keys&self.active.keys() or any(x.esa==FAST for x in e.rows):self.taint(True)
                self.foreign_keys.update(keys)
                return
            if op==7:return
            staged={};bad=False
            for entry in e.rows:
                if entry.instance==0 or entry.instance in staged:
                    bad=True;self.taint(True);continue
                try:
                    if entry.esa==FAST:load_speed(entry.body)
                except Invalid:bad=True;continue
                self.serial+=1
                staged[entry.instance]=Lifetime(e.actor,entry,self.serial,mark,True,True)
            # StartStates replaces attached own states only. Detached S307 survives.
            self.active={k:v for k,v in self.active.items() if not(v.actor==self.own and v.attached)}
            if set(staged)&(self.active.keys()|self.foreign_keys):self.taint(True)
            for key,life in staged.items():
                if key not in self.active:self.active[key]=life
            if sum(v.entry.esa==FAST for v in self.active.values())>1:self.taint(True)
            self.baseline=mark;self.own_bad=bad
            return
        if e.kind in ('start','local'):
            x=e.entry
            if e.actor not in (None,self.own):
                if x.esa==FAST or x.instance in self.active:self.taint(True)
                self.foreign_keys.add(x.instance)
                return
            attached=e.kind=='start' and x.group not in (-1,6)
            if not x.supported:self.taint(not attached);return
            if x.instance==0 or x.instance in self.active or x.instance in self.foreign_keys:self.taint(True);return
            self.serial+=1
            self.active[x.instance]=Lifetime(e.actor,x,self.serial,mark,attached,True)
            if len([v for v in self.active.values() if v.entry.esa==FAST])>1:self.taint(True)
            return
        if e.kind in ('end','end_key'):
            life=self.active.get(e.instance)
            if life is None:return
            if e.kind=='end' and life.actor is not None and e.actor!=life.actor:self.taint(True);return
            self.remove(self.scope,e.instance,life.generation,mark)
        # S16 and generic instance messages do not update FastLoad or end it.
    def remove(self,scope,key,generation,mark):
        life=self.active.get(key)
        if scope!=self.scope or life is None or life.generation!=generation or mark<life.born:return
        del self.active[key];self.ready=False
    def faint(self,scope,actor,flags,mark):
        if scope!=self.scope or actor!=self.own:return
        if flags is None:self.taint();return
        if flags&1024:return
        for key,life in list(self.active.items()):
            if life.actor==actor and life.attached and life.entry.group in (0,1):
                self.remove(scope,key,life.generation,mark)
    def advance(self,milliseconds):
        assert milliseconds>=0  # No FastLoad duration is recovered.
    def reset(self,scope,own):self.__init__(scope,own)

~~~

## Source joins

These selected joins are independently reconstructed from the pinned current metadata, module pointers, element table, and code bodies. The checker includes additional cleanup/dispatch methods. [W][D]

| Owner | Method / token / element slot | Function @ original body | Bytes |
| --- | --- | --- | ---: |
| Esa.EsaRunner::Play | m50202 / 0x6003ec0 / 20566 | f20617@0x649254 | 1100 |
| Esa.EsaRunner::Start | m50203 / 0x6003ec1 / 20567 | f20619@0x649784 | 926 |
| Esa.EsaRunner::BeginState | m50205 / 0x6003ec3 / 20569 | f20626@0x649fdd | 984 |
| Esa.EsaRunner::StartStates | m50206 / 0x6003ec4 / 20570 | f20627@0x64a3b7 | 1567 |
| Esa.EsaRunner::AttachState | m50207 / 0x6003ec5 / 20571 | f20620@0x649b24 | 462 |
| Esa.EsaRunner::OnFaint | m50209 / 0x6003ec7 / 20573 | f20630@0x64acd8 | 359 |
| Esa.EsaRunner::OnOwnerGone | m50210 / 0x6003ec8 / 20574 | f20631@0x64ae41 | 585 |
| Esa.EsaRunner::OnAppear | m50211 / 0x6003ec9 / 20575 | f20632@0x64b08c | 216 |
| Esa.EsaRunner::RunRemovals | m50219 / 0x6003ed1 / 20583 | f20646@0x64cc21 | 682 |
| Esa.EsaRunner::EndNow | m50220 / 0x6003ed2 / 20584 | f20628@0x64a9d8 | 513 |
| Esa.EsaRunner::OnStateEnd | m50221 / 0x6003ed3 / 20585 | f20621@0x649cf4 | 349 |
| Esa.EsaRunner::EndState | m50222 / 0x6003ed4 / 20586 | f20623@0x649e8e | 276 |
| Esa.EsaRunner::Message | m50224 / 0x6003ed6 / 20588 | f20648@0x64cf05 | 732 |
| Esa.EsaRunner::InstancePacket | m50225 / 0x6003ed7 / 20589 | f20649@0x64d1e2 | 114 |
| Esa.EsaRunner::OnZone | m50232 / 0x6003ede / 20596 | f20659@0x64e06e | 668 |
| Esa.Classes.ESAFastLoad::Init | m52314 / 0x6004700 / 22541 | f16980@0x50e959 | 791 |
| Esa.Classes.ESAFastLoad::InitState | m52315 / 0x6004701 / 22542 | f16982@0x50ec94 | 206 |
| Esa.Classes.ESAFastLoad::OnEnd | m52317 / 0x6004703 / 22544 | f16983@0x50ed64 | 170 |
| EsBlock::Read | m71820 / 0x600053f / 24947 | f10388@0x3055a3 | 1011 |
| EsBlock::.cctor | m71823 / 0x6000542 / 24950 | f10391@0x305abc | 2870 |
| Esa.EsaRegistry::R17 | m49681 / 0x6003cb7 / 20046 | f17633@0x55b86a | 2305 |
| Esa.EsaRegistry::Create | m49687 / 0x6003cbd / 20052 | f17639@0x55def4 | 245 |
| Esa.EsaRegistry::.cctor | m49688 / 0x6003cbe / 20053 | f17640@0x55dfeb | 311 |
| .<>c::<R17>b__18_17 | m50027 / 0x6003e11 / 20392 | f17978@0x562752 | 52 |
| Skills::ParseEsaHeader | m74592 / 0x6001013 / 27711 | f11272@0x3354db | 302 |
| SkillLink::Receive | m74630 / 0x6001039 / 27749 | f11277@0x336169 | 3025 |
| Combat::ParseMessage | m71379 / 0x6000386 / 24506 | f13214@0x3aaaaf | 1557 |
| CombatLink::Receive | m71395 / 0x6000396 / 24522 | f13230@0x3abfe7 | 1174 |
| SkillEsaView::Install | m31034 / 0x60034be / 59427 | f45296@0xed7938 | 4787 |
| .Host::SetLoadSpeed | m31146 / 0x600352e / 59539 | f45412@0xee08ea | 196 |
| .Host::State | m31154 / 0x6003536 / 59547 | f45420@0xee108f | 159 |
| .<>c__DisplayClass2_0::<Install>b__5 | m31285 / 0x60035b9 / 59678 | f43906@0xe42d5a | 90 |
| .<>c__DisplayClass2_0::<Install>b__6 | m31286 / 0x60035ba / 59679 | f43907@0xe42db5 | 92 |
| .<>c__DisplayClass2_0::<Install>b__8 | m31288 / 0x60035bc / 59681 | f43909@0xe42e61 | 23 |
| .<>c__DisplayClass2_0::<Install>b__15 | m31295 / 0x60035c3 / 59688 | f43916@0xe42f04 | 278 |
| SkillsApi::ClaimSkillHits | m48211 / 0x60036f9 / 18613 | f23769@0x75a497 | 193 |
| Esa.EsaClass::OnMessage | m50109 / 0x6003e63 / 20473 | f18039@0x564a4e | 85 |
| Esa.EsaClass::RunEnd | m50115 / 0x6003e69 / 20479 | f18045@0x564db8 | 78 |

| Registered type | Metadata row | Selected object fields |
| --- | --- | --- |
| 12605 EsaHeader | 0x7ee22e | Invoker +8, Target +12, Instance +16, Esa +20, V14 +24, EsClass +28, Skill +32, V04 +36, Payload +40, Proc +44 |
| 11699 EsEntry | 0x7df89a | Group +8, Esa +12, Instance +16, V14 +20, Skill +24, V04 +28, Body +32 |
| 8290 EsaClass | 0x7a89b8 | Host +8, runner +12, bundle +16, Header +20, Id +24, Persistent +28, State +72, StateMs +76, Ending +82, StateOwner +84, FxInvoker +88, FxTarget +92 |
| 8301 EsaRunner | 0x7a8c8e | running +12, removals +16, inventories +24, nextId +32, playing +36, deferredEnds +40, carry +48, ScopeInvoker +56, ScopeTarget +60 |
| 11560 StateEnd | 0x7dd4c4 | Owner +8, Instance +12 |
| 6629 GunFlowing | 0x78dd7e | LoadSpeed +76, LoadsWhileMoving +80, KeepsLoadOnHit +81, Key +82 |
| 8024 SkillsApi | 0x7a4524 | EsaPlayer +72, combatPlaysEsa +124 |
| 12191 AppearMessage | 0x7e7772 | Id +8, Type +12, States +108, Player +84 |

Selected body fingerprints include local declarations and the final end instruction. They identify public compiled code, not access-bearing key values. No access-bearing key was extracted for this task.

| Body | SHA-256 |
| --- | --- |
| f10388@0x3055a3, 1011 bytes | c09376144a09dfd277186b67d10cf9e08ae066e3b679746968fab5d2dd4c143f |
| f10391@0x305abc, 2870 bytes | a908eaa4bb1618c1ed13672c7d8a4709efdd759e37eb0da4df9434c6b836aedb |
| f11272@0x3354db, 302 bytes | 274539f54a091367d02a468bacd0150b70e2650f4cebe5de38e2fb79573a289b |
| f16980@0x50e959, 791 bytes | 2fec403a85e95798572bc8f9efa6d160f01a09207dfc13043a191ca8ac64afb8 |
| f16982@0x50ec94, 206 bytes | c19882766d0af89059d65755c3606dc00fdd195d4c8d125eecdbb8162fe18a50 |
| f16983@0x50ed64, 170 bytes | ff8799fba9badda9f9ca9afe38ffe116e34bdfbb1f9876947219a34b9109a992 |
| f17633@0x55b86a, 2305 bytes | c7c422d925bd75c4e904f154bc7c186e612638cc12137d9a642e96fad9384fc2 |
| f17978@0x562752, 52 bytes | b4200823038ca5d0194964328efdb391e281696e7d976332dbf284cc0baa4b93 |
| f20627@0x64a3b7, 1567 bytes | 309575d80c7f9f8ce8f612ff7690eb6391429dc4ff66c3dc128843f294f14460 |
| f20661@0x64e381, 31 bytes | 5c0f217e2c2f0d572b5f690c604f34e8e873919e8080dd2ff54b816400734f87 |
| f45420@0xee108f, 159 bytes | 074a33fe8931fcc84e897ac6f0c669c03c8d9925d9444513907d8d0ed5727d57 |
| f45412@0xee08ea, 196 bytes | adaaa88c972ab24a20e2794ea1fc9bd7dcbef6655ba91b1d2544f647c1ed0d86 |

## Remaining runtime prerequisites

This resolves the bounded current-public-source lifecycle investigation. It does not establish the selected character's actual effect list, values, or server acceptance. Native integration still needs a complete received own baseline, correct original-ingress/Aux handling, valid owner incarnations, actual equipment/ammo/definitions/key state, and a known reload accumulator. Runtime work in #23 must correlate initial and changed effects, determine whether foreign FastLoad traffic occurs, and prove an actual accepted shot with the correct lifecycle. It must also confirm zone/reconnect behavior and retirement of any detached states observed in that session.

The source has no generation nonce, FastLoad upsert or TTL, or own-recipient guard around the UI scalar setter. Those omissions remain explicit. No broader implementation ticket is resolved by publishing this report. Research branches are evidence for Root's integration, not application changes.

Earlier reports were leads. The critical current joins in this report were decoded again from W/D. The supplied definitions and assisted-input reports did not replace that proof. No private native-probe output, secrets directory, account file, credential, or login was accessed.

[W]: https://tr-play.playall.in.th/Build/d9a4c4fd2737e740b82e1321839e352f.wasm.gz
[D]: https://tr-play.playall.in.th/Build/6135c4139b62e558bb4c755b266f5fa2.data.gz
[R18]: https://github.com/oDestroyeRo/opentrickster/blob/df5fe0a5e8e45072845538fc74967a1757b6d9d7/docs/research/current-build-protocol-revalidation.md
[R14]: https://github.com/oDestroyeRo/opentrickster/blob/cb587e235b113aa26b04db018c31f1e26510f5bd/docs/research/target-state-completion.md
[R29]: https://github.com/oDestroyeRo/opentrickster/blob/efccf7992b77966804e9078d4643a565aa3b6340/docs/research/gun-combat-control.md
