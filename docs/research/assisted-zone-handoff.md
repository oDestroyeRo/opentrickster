# Assisted zone handoff

Build **`ea1a68bd89958fae94179f1e0835ffb5`**, 2026-11-10. This report is static research with synthetic checks. It contains no credentials, payloads, UIDs, hosts, keys or character values.

**Result.** A map change keeps the same `GameSession`. The server sends s2c 154 on the current game link. The client loads the new map, closes the old link, opens a replacement link to the host and port named in 154, and logs in there with the 13-byte c2s 156. The client does not report that close as a loss. It keeps its inventory, status and gun controller, and it clears the old map's actors, ground items and effects. Only s2c 154 carries the new zone ID. The private fields of 154 and 156 (UID, host, port, key, option) must never cross the page boundary.

The live evidence is in [issue #62](https://github.com/oDestroyeRo/opentrickster/issues/62): logical opcodes and frame sizes from one warp, with no payloads.

## Wire layouts

Every method below is on build ea1a68bd and is identified by its function index. Offsets are WASM code offsets. `PacketReader` methods throw `PacketException` when a read passes the end.

| Frame | Size | Layout | Source |
| --- | --- | --- | --- |
| s2c 154 `Handoff` | 27 + host bytes (live: 42) | U32 unused, U64 `Uid`, C-string `Host`, U32 `Port`, U32 `Key`, U16 `Zone`, U16 x, U16 y, U8 `Option`. No end check. | `Messages.ParseHandoff` f11873 `0x3593db` |
| c2s 156 handoff login | 13 | U64 `Uid`, U32 `Key`, U8 `Option`. Header bytes 4–5 are zero. | `GameServerLink.SendHandoffLogin` f11693 `0x34f20a` |
| s2c 437 `EnterZone` | 6 read (live: 11) | U16 own actor ID, U16 x, U16 y. No end check, so the live tail is ignored. | `Messages.ParseEnterZone` f11871 `0x3592b0` |
| s2c 155 | Not checked | Raises `HandoffRefused`. | `GameSession.Handle` f20915 `0x65e88c` |
| s2c 153 | 6 (live) | No reader. The client ignores it. | `GameSession.Handle` f20915 |

- **154.** `ParseHandoff` reads the U32 at `0x359422`, `Uid` at `0x359437`, `Host` at `0x359443`, `Port` at `0x35944f`, `Key` at `0x35945c`, `Zone` at `0x359469`, the position at `0x35948c` and `0x359494`, and `Option` at `0x3594a5`. It stores them in `HandoffMessage` (+8 `Uid`, +16 `Host`, +24 `Port`, +32 `Key`, +40 `Zone`, +44 `Pos`, +52 `Option`).
- **C-strings end at the first zero byte.** `PacketReader.Cstr` f9854 `0x2e411d` finds the terminator with `Array.IndexOf(Data, 0, Offset)`, decodes the bytes before it as CP874 and moves past the terminator. A missing terminator throws (`0x2e4208`). So a 154 with an empty host is 28 bytes.
- **156.** `SendHandoffLogin` writes U64 at `0x34f24d`, U32 at `0x34f254` and U8 at `0x34f25c`, then calls `Connection.Send(156, bytes)` f9841 at `0x34f266`. `Connection.Send` encodes through `IFrameCodec.Encode(Int32, Byte[])` (`call_indirect` at `0x2e2e51`), which takes no Aux, so header bytes 4–5 are zero, as on c2s 4. Its only caller is `GameSession/<>c__DisplayClass77_0` f20937 `0x65f7b7`, which passes the handoff's `Uid`, `Key` and `Option` at `0x65f7d3`.
- **437.** `ParseEnterZone` reads U16 at `0x359304` and the position at `0x359327` and `0x35932f`. The 437 case at `0x65e947` copies the U16 into the own `Me.Id` and the position into `Me.Pos`, then raises `EnterZone` at `0x65e96c`. The zone does not change here.
- **153.** `GameSession.Handle` has no 153 case: in the 58–184 range its default branch accepts only 184 (`0x65e5bf`). No function registers a packet handler with the constant 153.

## Client flow

1. **154 arrives on the current link.** The 154 case of `GameSession.Handle` (`0x65e784`) sets `Me.Zone` and `Me.Pos` from the handoff. It clears the held stop (+132), sets the move-sent time (+112) to NaN, writes 256 to +88 (target pending off, send armed on), stores the message in `pendingHandoff` (+64) and sets `handoffPending` (+72) at `0x65e7d7`. Then it raises `ZoneChange`.
2. **The UI loads the new map first.** `GameplayScreen.OnEvent` f38512 `0xbf6dd4` sends `ZoneChange` to `ChangeZoneAfter` f38519 (`0xbf77d4`). `GameplayScreen/<ChangeZone>d__341.MoveNext` f48915 `0x102d498` does the following, in order:
   - disposes the actor views (`ActorView.Dispose` f32559 at `0x102d89a`);
   - clears ground items (`GroundItems.Clear` f48917 at `0x102d9f1`), effects (`EffectEngine.Clear` f13735 at `0x102da10`) and portal cakes (f38312 at `0x102da2b`);
   - places the own actor and stops its action (`0x102db08`, `StopAction` f26866 at `0x102db24`);
   - loads the zone (`Zone.ZoneInfoAsync` at `0x102e152`, `LoadZone` f38504 at `0x102e8d4`) and places the own actor again (`0x102f38e`);
   - calls `GameSession.CompleteZoneChangeAsync` f20911 at `0x102f56e`. A false result or an exception logs a warning and logs out (`Logout` f38520 at `0x102f89f` and `0x102fa54`).

   Until then the old link stays open, and `Handle` keeps applying its frames.
3. **The client swaps links.** `<CompleteZoneChangeAsync>d__77.MoveNext` f20939 `0x65f923`:
   - takes `pendingHandoff` and clears it (`0x65f9ec`–`0x65f9ff`). With no handoff it returns false (`0x65fa0b`).
   - sets `closing` (+68) at `0x65fa36`, closes the old link (`GameServerLink.Close` f11697 at `0x65fa4d`), and clears `closing` at `0x65fa56`.
   - opens the replacement link to the handoff's host and port (`GameServerLink.OpenAsync` f11690 at `0x65faea`). On `ConnectionClosedException` (`0x65fc04`) it tries once more (`0x65fd29`).
   - tracks the new connection (`Session.Track` f10049 at `0x65fdfa`), resets the same send state as step 1, stores the new link in +40 (`0x65fe2b`), clears `handoffPending` (`0x65fe32`) and binds it (`BindGameServer` f20880 at `0x65fe4c`).
   - sends c2s 156 through `TrySend` f20912 (`0x65fe91`, then `0x65fed6`). When neither send succeeds it throws `HandoffLoginFailedException` (`0x65ff06`).
4. **The old link's close is not a loss.** `GameServerLink.Close` reaches `Connection.Close` f9922 `0x2e8da3`, which closes the transport and runs `Connection.Shutdown` f9923 synchronously, while `closing` is still set. `BindGameServer` installs the close listener at `0x65c234`. That listener, f20938 `0x65f7d9`, returns unless the closed link is the current one and `closing` is clear. Only then does it set `gsLost` (+69) and raise `Closed`.
5. **The server enters the zone on the new link.** The live trace shows feature frames, then 437, then the own and nearby actors' appearances and the client's usual post-entry requests, including c2s 511.

## What the client keeps

- **Session state.** `CompleteZoneChangeAsync` does not touch `recordingStartSet` (+70), `worldEntered` (+71), the start set (+36) or the inventory, and it sends no C5. The `Handle` prologue records start-set frames only while `handoffPending` is clear and `recordingStartSet` is set (`0x65dee9`–`0x65df24`). The [inventory baseline](native-inventory-baseline.md#carried-weight-and-invalidation) reaches the same conclusion for the inventory. The live replacement link carries no 231 inventory list and no 233 own state.
- **Login-only frames stay login-only.** `Handle` parses 1329 only while `worldEntered` is clear (`0x65ea38`).
- **The gun controller.** The `GunFlowing` constructor f24374 reads the live zone through a `Func<ZoneInfo>`. `GunFlowing.Reset` f24384 has no direct caller. The own appearance on the new link runs `MyAppear`, which requests a new key (c2s 511, answered by s2c 512).
- **The sampled zone is the loaded map.** The sampler reads `GameplayScreen` +148 (`LoadedZone`), then `ZoneInfo` at +8 and its `Id` at +8 ([`sampler.js`](../../crates/net/src/sampler.js)). After `LoadZone`, that ID is the handoff's `Zone`. 437 carries the own actor ID, not a zone.

The client clears the old map's actor views, ground items and effects (step 2). The server then sends appearances for the new map.

## Entry frames on the new link

Besides 437, the new link starts with feature frames that Net does not forward: 977 and 983 (`FortuneLink`), 887 and 886 (`ExpBonusUi`), 676 (`MyViewRecords`), 1228 (`MailLink`), 1263 (`GmAbility`), 604 (`PkCountModel` and `PlayerHpModel`) and 1002 (`CharacterStatus`). Of the frames in the live entry sequence, the page forwards 437, 283, 8, 9, 36, 37 and 512, which Net already decodes.

## Change

Net follows the handoff in client-assisted mode only.

- **Page bridge.** From the selected game link, the bridge forwards s2c 154 as a fixed 34-byte frame: the header, 21 zero bytes in place of the U32, `Uid`, an empty `Host`, `Port` and `Key`, then `Zone` and the position, then a zero `Option`. A 154 with no host terminator, or with fewer than 15 bytes after it, faults the binding as `malformed`. After a forwarded 154, the next socket that sends a 13-byte c2s 156 with zero header bytes 4–5 becomes the selected game link. The bridge posts that header as a `game-handoff` message, queued behind earlier frames, and never forwards the 156 payload. A 156 with no 154 before it, or on the selected link itself, faults the binding as `unknown-flow`. The host rejects a 154 frame that is not in the redacted shape.
- **Decoder.** In client-assisted mode, 154 does to Net's state what `ChangeZone` does to the client's. It clears actors, drops, combat correlation, the pending action, the walk and the dig, then sets the zone and the position. It emits `MapChanged`, the map label when the definitions have one, and `Position`. Until the 437 on the new link arrives, the decoder reports no sampling identity, so no input plan or idle proof uses the old actor ID. That 437 sets the new own actor ID and the position. A 437 outside a handoff still makes the physical state uncertain.
- **Observer.** A 154 on the active link announces a handoff, so the announcing link's close is not a loss. The replacement's `game-handoff` must arrive within 10 seconds of that close (`HANDOFF_WINDOW`). The replacement then becomes the active link, with the same decoder and no Aux groups, because the client's new link starts with none. An action armed when the 154 is applied gets an `Unknown` receipt. The session ends as `Lost` when no replacement logs in within the window. It fails as `Uncertain` on a `game-handoff` without a 154, and on a frame from the old link after the switch.
- **Engine.** `MapChanged` drops the old map's entities and drops and raises the map floor, so a delayed frame from the old map cannot bring them back. A run pauses with `Pause::MapChanged`. The run drops its pending action instead of quarantining it, so a slow idle proof during the map load asks again instead of closing the session. Outside a run, the phase does not change.
- **Clientless mode.** The native decoder does not follow a handoff. A 154 makes the physical state uncertain, and the session ends as `Lost`.

## Verification

All checks are synthetic. No live session ran after the change.

- [`page_bridge.cjs`](../../crates/net/tests/page_bridge.cjs) runs the real bridge script. A 154 with synthetic secrets crosses as the redacted frame. The replacement socket's 156 is selected and posted after the frames queued before it. No secret reaches any page message. Isolated pages check the `unknown-flow` and `malformed` faults.
- Observer tests in [`observer.rs`](../../crates/net/src/observer.rs) cover a handoff onto the replacement link, a replacement login before the old link closes, the timeout, an unfinished Aux group on the old link, and flows the client never produces.
- `loopback_zone_handoff_ends_the_native_session_without_following_it` in [`session.rs`](../../crates/net/src/session.rs) checks the clientless fail-safe.
- Two traces in [`traces.rs`](../../crates/engine/tests/traces.rs) cover a map change during a run and outside a run.

Reverting any one part of the change made at least one of these tests fail.

## Limits

This report is static. It does not show how long the map load or the link swap takes, whether the server sends 155 instead of 154 or after it, or what the client does with a 154 that arrives during login. The live trace covers one warp between two maps.

These handoff shapes still end the session:

- A 154 before Baseline.
- A refused handoff (155), a replacement link that does not open, or a failed 156. No replacement logs in, so the session ends as `Lost` after the window.
- A `socket-closed` notice that overtakes the queued 154. The bridge posts socket notices at once but queues frames behind slow sends, so a stalled send queue can turn a handoff into `Lost`.
- A page load can open at most 64 WebSockets: the bridge's record cap, the host's connection IDs 1–64 and the observer's retired-link cap. Each map change opens one, so after about 60 map changes in one page load the bridge refuses the client's next WebSocket and faults the session with `capacity`.
