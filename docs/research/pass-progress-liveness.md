# Game-link liveness and season-pass progress (32572)

Build **`ea1a68bd89958fae94179f1e0835ffb5`**, 2026-11-10. This report is static research with synthetic checks. It contains no credentials, payloads, UIDs, item names or character values.

**Result.** An idle character in a quiet area receives no change to its state. Net then reported no state fact, so the engine called the state stale 15 s after the baseline and refused Start, although the game link was healthy. The link keeps delivering frames while idle. Logical 32572 is the server's season-pass progress push. The client parses it into a `PassProgressPush` for its pass window. It carries no character state.

Net now reports `Fact::StateCurrent` when a frame release applies a received game frame after the baseline. The engine counts that as a fresh observation of the state already reported. It adds and changes no value. The page bridge now forwards 32572 as a bare 6-byte header, so the client-assisted observer also sees these pushes.

## Observed idle stream

One live idle session before this change was recorded outside Git, as opcodes and lengths only:

- Frames never stopped. The longest gap was about 1.1 s.
- 32572 arrived every 1.0 to 1.1 s with 19 bytes, and once with 75.
- The other frames were 66 (3 bytes), 463 (6), 464 (21 to 35) and 63 (3).
- No 2028 arrived on the game link. The client answers 2028 only on the login link ([Login link](native-login-link.md)).

`page::INCOMING` listed none of these opcodes, so the page bridge forwarded none of them.

## Static evidence

Every method below is on build ea1a68bd and is identified by its function index. Offsets are WASM code offsets. Each hash is the SHA-256 of the function body in the code section.

| Function | Method | Body | SHA-256 |
| --- | --- | --- | --- |
| f10935 | `PassLink::.ctor(IGameSession)` | `0x3266df`, 1235 bytes | `827a2927fa7343babe76dabfd61ef32e1b167cb45249daa643b3d103e8ddf5ff` |
| f10944 | `PassLink::<.ctor>b__18_4(NetFrame)` | `0x327420`, 40 bytes | `27a762237d2250cfbbb3b6c2d58cd45d91a6130950411a15a3d7c55e8cce6803` |
| f10913 | `PassWire::ParseProgress(Byte[])` | `0x325273`, 641 bytes | `247407a320a1c070f3e9404d4165c076b45e265f173f0a77991314aa459fe05e` |
| f10931 | `PassLink::add_Progress(Action<PassProgressPush>)` | `0x326497`, 144 bytes | `01db5571d82f7c526b5b6562f817820c19efa528b93b0b2f6747d83a681495fa` |
| f22314 | `PassFeature::.ctor(IGameSession, …)` | `0x6d278c`, 726 bytes | `cb994af170f77f16e92ed3844dccfec29b331f5e83ae124094d08d31d0a5c7e8` |
| f22376 | `PassFeature::OnProgress(PassProgressPush)` | `0x6d87c0`, 696 bytes | `e313252e6b927cbeb245894f021c4068724411b4f069e250b450b5e8a6a98cc7` |

- **Only `PassLink` subscribes 32572.**
  - The code section holds `i32.const 32572` in one place, `PassLink::.ctor` f10935 at `0x326adc`. A scan of every constant from 32500 to 32700 puts 32571, 32573 and 32574 in the same constructor. Other feature links own the neighboring opcodes, for example `PassHello::.ctor` f10920 owns 32570.
  - The constructor passes each opcode and an `Action<NetFrame>` to an interface call on its `IGameSession`. For 32572 that call is the `call_indirect` at `0x326aec`. The lookup fallback at `0x326ace` passes slot 2, and slot 2 of `IGameSession` (method 72758) is `OnPacket(Int32, Action<NetFrame>)`. The constructor keeps each returned `Action` in its `cleanups` list.
  - The handlers are `b__18_2` for 32574, `b__18_3` for 32571, `b__18_4` f10944 for 32572 and `b__18_5` f10945 for 32573. `b__18_5` calls `PassWire::ParseClaim` f10914.
  - No metadata field default data holds 32572. That is where RVA arrays, such as `Combat.Opcodes`, are stored. Static memory holds one aligned I32 32572, at `0x39bee0`. It lies inside a run of 2890 consecutive integers, 32210 to 35099, and no code constant addresses that run. It is not an opcode list.
- **The handler only parses the push and raises `Progress`.** `b__18_4` f10944 passes `NetFrame.Payload` to `ParseProgress` at `0x32742a`. `NetFrame` is a value type, so `Payload` sits at offset 8 of the unboxed struct. When the result is not null and `PassLink.Progress` (offset 40) has a subscriber, it invokes that delegate. It does nothing else.
- **`ParseProgress` f10913 reads 12 + 7 × n bytes.** With a `PacketReader` it reads, in order:
  1. U16 `SeasonId` at `0x32532b`;
  2. U32 `Xp` at `0x325337`;
  3. U8 `Level` at `0x325344`;
  4. U32 `TodayXp` at `0x325350`;
  5. U8 task count n at `0x32535f`, then for each task a U16, a U32 and a U8, at `0x325378`, `0x325381` and `0x325391`. Each task becomes an `(Int32, Int64, Boolean)` in `Tasks`.

  The observed 19-byte push holds one task, and the 75-byte push holds nine.
- **Only the pass feature consumes it.** `PassFeature::.ctor` f22314 constructs the `PassLink` at `0x6d2991` and subscribes `PassFeature::OnProgress` f22376 through `add_Progress` f10931 at `0x6d2a1d`. These are the only direct calls to either method. `OnProgress` walks the push's tasks and calls `PassFeature::Update(Double)` f22363 and `PassState::Task(Int32)` f10897. Its other callees map to no metadata method. None of these is an input to Net's decoder.

To reprint the joins and the read sequence, set `CACHE` to a `tools/build-migrate` cache of this build and run this from the repository root:

```sh
python3 -I - "$CACHE" <<'EOF'
import bisect, sys
sys.path.insert(0, "tools/build-migrate")
from buildmigrate import derive, wasm
b = derive.Build(sys.argv[1])
m = b.module
code_start, code_end = m.sections[10]
starts = [start for start, _ in m.bodies]
needle = b"\x41\xbc\xfe\x01"  # i32.const 32572 (SLEB128)
hits = {}
at = m.data.find(needle, code_start)
while 0 <= at < code_end:
    function = bisect.bisect_right(starts, at) - 1 + len(m.imported_functions)
    hits.setdefault(function, set()).add(at)
    at = m.data.find(needle, at + 1)
for function, offsets in sorted(hits.items()):
    _, code = wasm.decode(m, function)
    real = [hex(x.offset) for x in code if x.op == "i32.const" and x.imm == (32572,) and x.offset in offsets]
    if real:
        print("f%d" % function, real, [b.rt.method_name(k) for k in b.rt.function_methods.get(function, [])])
parse = "[Spinel.Net.dll]Spinel.Net.Game.PassWire::ParseProgress(System.Byte[])"
for name in ("[Spinel.Net.dll]Spinel.Net.Game.PassLink::.ctor([Spinel.Net.dll]Spinel.Net.Game.IGameSession)",
             "[Spinel.Net.dll]Spinel.Net.Game.PassLink::<.ctor>b__18_4([Spinel.Net.dll]Spinel.Net.NetFrame)",
             parse,
             "[Spinel.Game.dll]Spinel.Game.Play.PassFeature::OnProgress([Spinel.Net.dll]Spinel.Net.Game.PassProgressPush)"):
    r = b.join_record(name)
    print("f%d %s/%d %s" % (r["function"], r["body"], r["bytes"], r["sha256"]))
_, code = wasm.decode(m, b.function_of(parse)[1][1])
for x in code:
    if x.op == "call":
        names = [b.rt.method_name(k) for k in b.rt.function_methods.get(x.imm[0], [])]
        if names and "PacketReader::" in names[0]:
            print(hex(x.offset), names[0])
EOF
```

## Why an applied frame shows current state

- Net applies received game frames in ingress order. `OrderedIngress::release` releases a mark only when every earlier frame is complete. It holds the queue behind a partial frame and behind an incomplete Aux group. So when a release applies a received frame, every frame through the settled mark has been applied, and the facts from those frames are emitted before the settlement event.
- After the baseline, Net's state therefore matches the client's state after the same frames. A frame that changes nothing, such as 32572, still shows that the link is delivering and that no earlier frame is held.
- What the frame means is irrelevant: any applied received game frame counts. Net uses 32572 only on the page path, where the bridge forwards nothing else while idle.
- A zone change ends the session as uncertain. A lost carried-inventory cache makes weight unknown, and the engine keeps that as missing state. The engine accepts `Facts` only while online, so a lost, reconnecting, closing or closed session never refreshes.

## Change

- **Engine.** New `Fact::StateCurrent` (`crates/engine/src/model.rs`). `Book::apply` treats it as a state observation: it advances `own.last_state_at` and sets no value. Missing or unknown state stays missing or unknown. Per-field ingress stamps are untouched, so it cannot reorder or overwrite a newer fact.
- **Native session** (`crates/net/src/session.rs`). A settlement event (revision namespace 2) carries `StateCurrent` when its release applied at least one received game frame.
- **Client-assisted observer** (`crates/net/src/observer.rs`). The same rule applies after the baseline. The page frontier also advances on the app's own sends, on other sockets, and on game frames the page does not forward. A settlement without an applied forwarded frame carries no `StateCurrent`.
- **Page bridge** (`crates/net/src/page.rs`). 32572 joins `INCOMING` and `HEADER_ONLY`, so only its 6-byte header crosses IPC, with the length zeroed. `decode::UNDECODED_INVENTORY` now lists its ten inventory inputs itself, so 32572 never invalidates carried weight.

## Verification

- **Engine traces** (`crates/engine/tests/traces.rs`):
  - `a_live_idle_stream_keeps_state_fresh_until_it_falls_silent`: 40 s of liveness events 1 s apart keeps Start allowed and a run `Running`. After 15 s of silence the run pauses as `Uncertain` with `NeedsFreshObservation(WorldState)`. A new liveness event allows Resume.
  - `liveness_never_refreshes_a_lost_reconnecting_or_closing_session`: no `StateCurrent` refreshes state while the session is lost, reconnecting, closing or closed, or when it comes from an old session.
  - `liveness_adds_no_value_and_never_overrides_newer_facts`: weight stays missing and Start stays refused. A late HP fact is still rejected, and a newer MP fact is still accepted. A queued `StateCurrent` with an older time does not move `last_state_at` back.
- **Native loopback** (`loopback_authentication_selection_startup_and_cancellation`): a synthetic 19-byte 32572 after world entry produces a settlement with `StateCurrent` after the baseline.
- **Observer** (`only_an_applied_inbound_game_frame_marks_assisted_state_current`): a frontier that passes only a send and another socket's frame settles without `StateCurrent`. A header-only 32572 settles with it.
- **Page bridge** (`crates/net/tests/page_bridge.cjs`): a synthetic 32572 with a body crosses IPC as 6 bytes with length 0.
- **Mutations.** Each of these mutations fails its test: removing the engine arm fails the idle and value traces; removing either adapter's `StateCurrent` fails its adapter test; adding `StateCurrent` to every observer settlement fails the observer's negative case.

## Limits

- The push cadence was observed, and the server does not promise it. If pass progress stops, for example outside a season, the client-assisted path has no forwarded idle frame and goes stale after 15 s. The native path still counts 66, 463, 464 and 63. A general fix would have the bridge's frontier report the newest inbound mark on the selected game socket.
- The engine closes a session once its evidence dedupe map holds more than 4096 ranges (`EVIDENCE_RANGES`). Net's revision keys are `mark << 2 | namespace` and are rarely adjacent, so nearly every `Facts` event adds a range. A synthetic trace of settlements alone closes the session at the 4096th. At one settlement a second, that is about 68 minutes. More frames make it sooner. The limit predates this change, because settlements were already sent on every frontier advance. It now bounds how long an idle session stays Ready.
- Entity and drop freshness (15 s per entity) is unchanged. It filters combat targets and never blocks Start.
- The native transport sends a probe after 5 s without any gateway message. It fails with `Timeout` if the probe gets no answer in 5 s more. Total silence therefore ends a native session before the engine's 15 s staleness.
- No live session ran after this change.
