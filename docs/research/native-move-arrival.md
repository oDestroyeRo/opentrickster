# Native move arrival

Build **`ea1a68bd89958fae94179f1e0835ffb5`**, 2026-11-10. This report is static research with synthetic checks. It contains no credentials, payloads, UIDs, item names, coordinates or character values.

**Result.** The client never waits for a server stop to end a walk. The server echoes an own walk request with its own destination, which can differ from the request. The client then walks to that destination locally and ends the walk when its path empties. It sends nothing at arrival, and nothing tells it the server agrees. Net confirmed a MoveTo only on an own stop at the exact requested point, so every native MoveTo expired and paused the run.

Net now treats the first own movement frame after a MoveTo request as the server's answer. An echo (24 or 26) accepts the walk and gives the destination the server accepted. Arrival is reported when the client would stop walking:

- Natively, Net estimates the walk end from the echo, as described in [Design](#design). Packets cannot prove that end.
- On the client-assisted path, a page sample proves it. The sample must show the client's own path empty, the send re-armed, and the avatar standing on the accepted destination.

The engine's `Arrived` outcome now carries both the requested target and the accepted end. The planner treats the accepted end as the place it asked for, so it does not walk to the same target again.

## Live finding

One native dig run was recorded outside Git as opcodes, lengths and deltas only:

- DigAt at the start spot was accepted with 45, 371 and 50/51, and completed as empty.
- The next MoveTo sent c2s 24 and received one own s2c 24. The echo's destination was 4 px west of the requested point (dx = -4, dy = 0, mode 1). An earlier run ended about one 16-px cell east of its target.
- No own 25 or 27 arrived in the next 25 s. The other frames were other actors' 24/25, 32572, 63, 66, 463/464, 14, 47, 51, 592, 8, 15, 16 and 34.
- The decoder emitted `Arrived` only for an own 25/27 at exactly the target, so the 5 s action window expired and the run paused as `Uncertain`.

## Static evidence

The method index is the IL2CPP method definition index. Function numbers, offsets and hashes are WASM code-section values: each hash is the SHA-256 of the function body. Joins were produced with `tools/build-migrate` (`derive.Build`, `il2cpp.Runtime`, `fingerprint.Fingerprinter.tokens`) from the pinned public cache: `build.wasm.gz` `8780db4414df5357d9595ef233579227a1303753d00ac6318d225036dd1bb277` and `build.data.gz` `a3b60bc72c11d7e38c05ed7cd312c26f7161df31118401f0d56a9b4cba3693a3`. No target code was instantiated or run.

| Method | Function | Body | SHA-256 |
| --- | --- | --- | --- |
| 19437 `GameplayScreen::ClickWalk` | f38426 | `0xbf103c`, 282 bytes | `f69f503c0f3aadc3eae397be3e47e9f587238c64f999d09c0e30b67d1592851a` |
| 72127 `GameSession::RequestMove(Double,Double,Int32)` | f20902 | `0x65d481`, 685 bytes | `632982646973c35db6d49c1e3b5bc497f38ed8f69d047f2cbe18c39e16d185e6` |
| 72129 `GameSession::Request(MapPoint,Pos,Int32)` | f20905 | `0x65d85e`, 213 bytes | `a49dfa4502cfab6fb5d89e256552ea60a00e8f2bb814744103bc68355e6a2194` |
| 72130 `GameSession::Cell` | f20904 | `0x65d818`, 68 bytes | `c4a20271cbcd67a40bb34fe8f881704eff19c4729d7ef70cb087f8be64528a81` |
| 72131 `GameSession::SendMove` | f20903 | `0x65d730`, 231 bytes | `afc2879c67969afcb3a8fe98d0de800907e7ac6b8ed2008ec80305fbbc7280b7` |
| 72137 `GameSession::OwnMoveEchoed` | f20910 | `0x65da92`, 247 bytes | `b8bfc45bac5fcdc378b0aacd02e704167b45bca7c09095112b642b42c66c5126` |
| 72143 `GameSession::Handle` | f20915 | `0x65de1f`, 3472 bytes | `5c9189446438e84bee0dba8d5fe232096d65abfee90a655e14b3b4b37632b627` |
| 72144 `GameSession::ApplyHeldStop` | f20917 | `0x65edb2`, 37 bytes | `f67b81a1f9ef563b69597cfec1a9bdbcb7f9b0da731a96baa41de2fb8074b3f5` |
| 72145 `GameSession::ApplyMove` | f20916 | `0x65ebb1`, 512 bytes | `4e2c85e9141117d07281cd6d6587b5362bf041bd76c4c606c111fee8efcd66bc` |
| 73364 `Messages::ParseMove` | f11867 | `0x359031`, 347 bytes | `7feb1b23b1f6716ddd3f435035c081e933f7a9bcb59864459afd67786a9f10b7` |
| 73365 `Messages::ParseMoveSpeed` | f11868 | `0x35918d`, 85 bytes | `093dd73ace65d9481952540197a921eb9d88779c828159b2e80e93ea7e939cda` |
| 19400 `GameplayScreen::FollowServerMove` | f38516 | `0xbf8540`, 931 bytes | `cf905e1695e9a772910cc87cb48b4392cada25a7c4fa1ef9037ee4dfa1224e7c` |
| 34193 `WorldActor::FollowMove` | f26867 | `0x85f03f`, 2275 bytes | `457307c20d735a3047bea217d74a65b517e9da780f20fd3dc1239041ed2a592e` |
| 34188 `WorldActor::get_Moving` | f26862 | `0x85ee49`, 13 bytes | `fd188480f3a2213368e120b22895b4bd3e6346dd676c3b373c23f7f97dce54c9` |
| 34210 `WorldActor::Update` | f26885 | `0x860792`, 1496 bytes | `bd468d5a46b817b88710303def0ffd660b4737cf24a3e35ae0848cba2ae228e7` |
| 34211 `WorldActor::get_CurrentSpeed` | f26871 | `0x85fc2a`, 234 bytes | `be6a7690e487fcbae2188feb25ec74131acccf79bc17b1b48852f61e2b802613` |
| 34159 `MoveSpeeds::.ctor` | f26839 | `0x85d7f1`, 34 bytes | `8c9b8a1ddeed71ca20c39f4aca1c73fbfca4df2ef298eeb390f642ed98606a02` |
| 34475 `MovePath::Find` | f26646 | `0x84e571`, 375 bytes | `309f3a018c1b3e0f902a9948042b27f3b87a7592c6157b98b2bb97a17ee55900` |
| 34393 `ServerSteps::Between` | f26869 | `0x85fb9b`, 73 bytes | `d3281ec2f78a8d8e9197b25b742ae6e9173d15e6fdcf51f37260e2ce1682e4a8` |

Field offsets come from the metadata field layout. `GameSession`: +80 `lastTarget`, +88 `targetPending`, +89 `sendArmed`, +92 `lastMode`, +96 `Me`, +108 `MoveRefused`, +112 `moveSentAt`, +132 `heldStop`. `WorldActor`: +8 `speeds`, +12 `path`, +44 `Type`, +56 `X`, +64 `Y`, +88 `Running`, +96 `Speed`, +112 `OnArrive`, +336 `CatchUpPx`.

### The walk request does not snap

- `ClickWalk` f38426 checks its walk gate, takes the mode from `RunToggle.get_Mode` f25909, and calls `RequestMove` with the raw world point at `0xbf110f`. A held click calls `RequestHeldMove` at `0xbf1122`.
- `RequestMove` f20902 rounds both coordinates with `JsMath.Round`. It refuses a target inside the avatar's own 16 × 16 cell, using the `0.0625` constants and `Cell` f20904, and then checks `MoveRefused`. A target more than 50 px from `lastTarget` is sent at once. A nearer target is sent only while `sendArmed` is set. Otherwise the client stores it and sets `targetPending` at `0x65d717`. A send first clears `sendArmed` at `0x65d6e3`. `RequestHeldMove` (`0x65da18`) and `Request` (`0x65d8f2`) clear it the same way.
- `SendMove` f20903 writes `Round(from.X)`, `Round(from.Y)`, the target and the mode, and sends logical 24 at `0x65d7c5`/`0x65d7cb`. Nothing in the request path moves the target to a cell centre or a grid point. Snapping like the client is therefore not a fix: the client sends the rounded point, and the server picks the destination.

### The echo carries the server's start and destination

- `ParseMove` f11867 reads U16 actor. For 24 and 26 only, the `br_table` at `0x3590a9` then reads a U16 X, U16 Y start. Every form then reads U16 X, U16 Y destination and U8 mode. There is no end check. The 4 bytes Net skipped on 24/26 are the server's start (`From`).
- `GameSession::Handle` f20915 parses moves at `0x65e43f`. Any own move first flushes a held stop at `0x65e46e`. An own 25 is held in +132 at `0x65e47e`. It is then applied by `ApplyHeldStop` f20917, which `JsMicrotasks.Queue` schedules at `0x65e4ce`, so it lands before the next task. Every other move goes to `ApplyMove` at `0x65e4da`. An own 729 is ignored at `0x65e51b`.
- `ApplyMove` f20916 runs the arrive hooks and listeners. For an own move it sets `Me.Position` to the destination at `0x65eccc` and records the round trip. If a target is pending it sends that target at `0x65ed9a`. Otherwise it sets `sendArmed` at `0x65eda5`. `OwnMoveEchoed` f20910 also sets it, at `0x65db7d`. A scan of direct calls and method-pointer cells found no caller, so it may be reached through a delegate.

### The walk ends locally

- `FollowServerMove` f38516 computes the distance from the actor to the echo's start. Above 320 px (`0xbf8821`) it places the actor at the start with `Place` at `0xbf8851`. It then calls `FollowMove(start, destination, mode == 2, zone WalkMap)` at `0xbf88d5`.
- `FollowMove` f26867 builds the path. For a type-1 actor it calls `MovePath.Find` at `0x85f2d6`. A failed find clears the path and starts no walk. Other actor types get a straight line cut into 16-px steps (loop at `0x85f5be`). The actor joins the path at the nearest point (loop at `0x85f780`), then `Walk` (`0x85f8f5`) and `CutAfter` (`0x85f914`).
- `MovePath.Find` f26646 returns false when start and goal share a cell. Otherwise it runs `Smart` (`0x84e60f`), an A\* over the zone grid with a 256-node pool, or the `Stupid` fallback (`0x84e649`). When the last point lies in the goal cell, it is replaced by the exact destination at `0x84e6d9`. The cell size comes from the zone's `Grid`. A 16 × 16 cell is only the default without a grid (`WalkMap` constructor f26320 at `0x824a01`). `ServerSteps.Between` f26869 is `MovePath.Find` (`0x85fbd4`), so the client models the server's steps with the same pathfinder.
- `Update` f26885 advances `dt × CurrentSpeed / 1000` px per frame (`0x860985`–`0x86098f`), plus any catch-up. When the path empties it stamps the stop, zeroes `CatchUpPx` and invokes `OnArrive` (`0x860b78`, `0x860cf2`). `get_Moving` f26862 is `path.Count > 0`.

### Speed

- `get_CurrentSpeed` f26871 returns `Speed` when set. Otherwise it returns `speeds.Run` if `Running`, else `speeds.Walk`. The unit is px/s.
- `MoveSpeeds::.ctor` f26839 sets Walk 115 and Run 184. The avatar's `<Enter>d__283` f48925 replaces them with `NumberOr(record, "WalkSpeed", 115)` (`0x1032847`/`0x103285a`) and `NumberOr(record, "RunSpeed", 184)` (`0x1032875`/`0x1032888`). The record is a `Zone.Records(Table)` row (`0x103272c`) chosen by `<Enter>b__283_31` f38329. Literal indices are 17567 `WalkSpeed` and 12409 `RunSpeed`.
- S→C 472 is `ParseMoveSpeed` f11868: U16 actor, then U8 speed returned as `speed << 2`. Its subscriber is `OnPacket(472, <Enter>b__10)` at `0x103472a`/`0x1034731`. The handler f38368 sets the actor's `Speed` to that value at `0xbec3c7`, or clears it for zero. Before this change Net ignored 472 in its decoder's catch-all.
- ESA also has `SetActorMoveSpeed`. Its writers were not traced.

### Digging waits for the walk

`DrillView.Update` f44776 reads `avatar.Moving` at `0xe9e2aa` and passes it to `DrillFeature.Update` f20271 at `0xe9e2b0`. The [manual dig report](manual-dig-control.md) already records that `Begin` waits for `moving == false`. A dig press is allowed while walking. Only the local dig simulation waits.

### The assisted sampler already reads the walk

The sampler reads `motion.path` as the count of the own `WorldActor` path at +12, which is `get_Moving`. It reads `motion.sendArmed` from `GameSession` +89 and reports the own actor's `x`/`y` from +56/+64. Every client send clears `sendArmed`, and `ApplyMove` sets it again for an own echo. Take a sample after Net saw the echo. It proves that the client finished the walk when all of these hold: `sendArmed` is set, no target is pending, no stop is held, no route is queued, the path is empty, and the avatar stands on the echo's destination. The client walks there only after it applies the echo, and the path empties only when the walk ends. So the proof holds even if the client applies frames later than the bridge sees them.

## What is proven and what is not

Proven by the static trace:

- A server echo is the client's only acceptance of a walk. The client applies its destination and walks there.
- The server's destination can differ from the request, and the client does not snap.
- The walk ends locally when the path empties. No server stop is needed, and none arrived in the live run.
- The walk time is the path length divided by `CurrentSpeed`. The default walk speed is 115 px/s, and 472 changes it.
- The assisted sample signal described above.

Not provable from packets, so the native model below is an estimate:

- **Path length.** The client's path is an A\* over a zone grid that Net does not load. It goes through cell centres and around obstacles, so it can be longer than the straight line.
- **Speed.** A zone record can change the 115 px/s default, and ESA can set a speed. Net sees neither. Net does see own 472.
- **Frame timing.** Arrival lands on a client frame, and a throttled page runs fewer frames.
- **What the server does when an action arrives while it still thinks the character is walking.** The client itself may press dig or click a new walk while walking.

## Design

1. **Acceptance.** The first own movement frame after a MoveTo request decides it. An own 24/26 accepts the walk and records its destination and start. An own 25/27 at the target means the server stopped the character there: Net reports `Arrived` at once, as before. A 25/27 anywhere else refuses the walk. After acceptance, a 25/27 at the accepted destination also arrives. Any later own walk, or a 25/27 elsewhere, diverts it. A refused or diverted MoveTo is never confirmed, so its 5 s window expires and the run pauses as `Uncertain`. A request with no echo expires the same way. A late echo is treated as a walk the server started, not as an answer. An echo answers only the request it followed. Any new request drops that link, so an expired move's walk can never confirm the next move to the same target.
2. **Native walk end.** The decoder estimates the end from the echo, using the client's own inputs:
   - *Start:* the last own position. If the echo's start is more than 320 px from it, the client places the actor at the echo's start, so the walk begins there.
   - *Length:* L = |start − echo start| + |echo start − destination|. This is an upper bound for a straight path joined at its nearest point.
   - *Speed:* own 472 × 4 when set. Otherwise 184 px/s when the echo's mode is 2, else 115 px/s.
   - *Time:* ⌈1000 × (9/8 × L + 32) / speed⌉ ms plus two 16-ms frames, counted from the echo's receipt.

   The 9/8 factor and 32 px allowance are an estimate: they cover a path through 16-px cell centres on open ground. Nothing proves them. When the time elapses, the session tick reports `Arrived`, provided the walk was not diverted. The move confirms no earlier than the estimate. A walk that would take longer than the 5 s window pauses the run instead.
3. **Assisted walk end.** The observer ends the accepted walk on the first sample that proves the walk finished (see [The assisted sampler already reads the walk](#the-assisted-sampler-already-reads-the-walk)). If the sampled avatar stands anywhere other than the accepted destination, the walk was diverted: the move expires and the run pauses.
4. **Quiescence.** Native quiescence and new actions also wait for any estimated walk to end, including one the server started. The engine still quiesces after every confirmation, so a dig never starts during a walk. Gun loading now sees the walk end too: before this change, a native walk without a stop left the gun "moving" for the rest of the session.
5. **Engine.** `Outcome::Arrived { target, at }` confirms the MoveTo whose target matches. The book records `at` as the own position, and remembers that `at` answers `target` on this map. When the planner is about to move to a target and the character stands on that target's accepted end, it acts in place. A dig then happens where the character stands. This removes the loop of moving again to an unsnapped target that the server always ends elsewhere.

The reducer stays pure, one action stays outstanding, the 5 s window and quiescence barrier are unchanged, and stale state still blocks a run.

## Live diagnostics to collect

Record these outside Git as opcodes, lengths, booleans and deltas only:

- For each native MoveTo: the length of the own echo (11 bytes for 24/26) and of any own 25/27 (7 bytes); the destination minus the target; the echo's start minus the sent start; the echo delay; whether an own 472 arrived; whether any own 25/27 arrived within 10 s.
- The estimated walk time, and whether the next action was accepted when sent after it.
- On the client-assisted path, for the same walks: the time from the echo to the first sample with an empty path, compared with the native estimate. This is the direct check of the estimate.
- Whether a dig sent after an adjusted move produced 45 and an own 371 at the dig point.
