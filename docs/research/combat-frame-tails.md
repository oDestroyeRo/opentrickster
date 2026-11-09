# Combat frame tails on 31, 32, 36, 37, 43, 485 and 1438

Build **`ea1a68bd89958fae94179f1e0835ffb5`**, 2026-11-10. This report is static research with synthetic checks. It contains no credentials, payloads, UIDs, item names or character values.

**Result.** The client parses these game-link frames only in `CombatLink.Receive`, through `Combat.ParseMessage` and `Combat.ParseSwing`. Neither method checks the end of the payload.

- On 32 and 43, `Combat.U16At` reads a missing byte as zero, so every length parses.
- On 36 and 37, `Combat.I32At` needs 4 bytes.
- A strike on 31, 485 or 1438 needs 8 bytes when its kind is 3 and 14 bytes otherwise. Its later fields are read only when they fit.

Net was stricter in four places. It required exactly 2 bytes on 32, 2 or 3 bytes on 43, exactly 4 bytes on 36 and 37, and at most 17 bytes for a grouped strike. It was also looser on strikes: it never checked an ungrouped strike, and it accepted a grouped kind-3 strike of 4 bytes. Net now accepts and rejects the same lengths as these readers, except for a grouped 43 shorter than 2 bytes (see [Limits](#limits)).

## Static evidence

Every method below is on build ea1a68bd and is identified by its function index. Offsets are WASM code offsets.

- **Only `CombatLink.Receive` parses these opcodes.**
  - `Combat..cctor` f10808 fills `Combat.Opcodes` from a 24-entry RVA array that contains 31, 32, 36, 37, 43, 485 and 1438. `CombatLink..ctor` f10809 subscribes `CombatLink.Receive` f10820 to each entry through `IGameSession.OnPacket`, at `0x31ebd0`.
  - A stack scan of every function body found the other `OnPacket` and `OnSystemPacket` subscriptions. It covered 102,541 bodies, 973 call sites and 12 wrappers, including subscriptions whose opcodes come from arrays or parameters. The scan script is not part of this change. Two more listeners name these opcodes:
    - `AutoCombatUi/World..ctor` f37778 subscribes `World.OnOpener` f37779 to 224, 292, 31, 485 and 1438 in a loop (`0xbb54a0`), and `World.OnFaintArrived` f37782 to 43 (`0xbb5572`).
    - `GunFlowing..ctor` f24374 subscribes `<>c__DisplayClass151_0.<.ctor>b__5` f24429 to 32 through `GunFlowing.On` f24378 (`0x779941`).

    These three handlers return when the payload has fewer than 2 bytes (`0xbb5777`, `0xbb5fc5`, `0x77c8d6`). After that they read only bytes 0 and 1, so none of them throws on a short or long payload.
  - `GameSession.Handle` f20915 has no case for these opcodes. Its `br_table` at `0x65e19a` sends 31, 32, 36, 37 and 43 to the default. 485 leaves at `0x65e1fc`, and 1438 leaves through the `br_table` at `0x65e2df` and the test at `0x65ea1d`.
  - The client's status readers do not read them. `CharacterStatus..ctor` f12553 subscribes to 39, 109, 110, 245, 282, 283, 285, 629, 631, 634, 636, 649, 651, 784, 826, 934 and 1002. `StartStatsWire.StartStats` f12540 parses only the start-set 233 (`0x381931`), and only when it has at least 251 bytes (`0x381942`).
- **`CombatLink.Receive` parses every strike before it looks at Aux.** It loads Aux at `0x31f90d`. It sends 31 (`0x31f96f`), 1438 (`0x31faa2`) and 485 (`0x31faaa`) to a `StrikeEvent`, and calls `Combat.ParseSwing` at `0x31fb16` with no length check. Only then does it test Aux, at `0x31fb26`: an ungrouped strike is emitted at once and a grouped one waits for its bundle. Other opcodes go to `Combat.ParseMessage` f10804 at `0x31fc2d`.
- **`U16At` reads a missing byte as zero.** `Combat.U16At` f10802 starts from 0. It reads `data[at]` only when `Length > at` (`0x31e211`) and ORs in `data[at + 1] << 8` only when `Length > at + 1` (`0x31e228`). It never throws. At offset 0, an empty payload gives 0 and a 1-byte payload gives that byte.
- **32 and 43 parse at any length.** In `ParseMessage`, the `br_table` at `0x31e405` on `opcode - 32` selects the case. Both cases inline `U16At(data, 0)`:
  - 43 builds `CombatMessage/Faint`. It tests `Length <= 0` at `0x31e502` and `Length == 1` at `0x31e510`, reads the high byte at `0x31e514` and stores `Target` at `0x31e524`. It then stores `Length >= 3 ? data[2] : 0` (`0x31e530`, `0x31e535`, `0x31e53c`) and returns.
  - 32 builds `CombatMessage/AttackEnd` with the same tests at `0x31e577` and `0x31e585`. It reads the high byte at `0x31e589`, stores `Actor` at `0x31e599` and returns.
- **36 and 37 need 4 bytes.** `ParseMessage` calls `Combat.I32At(data, 0)` for `HpChanged` at `0x31e5cc` and for `MpChanged` at `0x31e602`. `I32At` f10803 passes `(data, at, 4)` to the `Span<byte>` constructor helper f69699 at `0x31e2a8`, then reads the span with `MemoryMarshal.Read<int>`. f69699 calls `ThrowHelper.ThrowArgumentOutOfRangeException` f72216 at `0x17e2a3b` unless `start <= Length` and `length <= Length - start`, both compared unsigned (`0x17e2a2c`, `0x17e2a34`). So a payload under 4 bytes throws, and bytes after the fourth are never read.
- **A strike needs 8 or 14 bytes.** `Combat.ParseSwing` f10796 reads the payload with a `PacketReader`:
  1. U16 `Attacker` at `0x31dbf8` and U16 `Kind` at `0x31dc03`.
  2. When `Kind == 3` (`0x31dc0a`), it calls `Messages.ReadPos` f11859 at `0x31dc92`, which reads two U16 values (`0x35806d`, `0x358078`). It then reads a U8 count at `0x31dcb1` only when a byte remains (`0x31dca5`). It reads up to that many U16 IDs at `0x31dcd4`, each only while at least 2 bytes remain (`0x31dcc8`), and keeps the first 10.
  3. For any other kind, it reads U16 `Target` at `0x31dd4a` and calls `ReadPos` at `0x31dd6f` and `0x31ddf1`. It reads a U16 at `0x31de10` only when at least 2 bytes remain (`0x31de07`), and a U8 at `0x31de2a` only when a byte remains (`0x31de1c`).

  No step compares `Offset` with `Data.Length` after the last read. `PacketReader.U16` f9827 and `PacketReader.U8` f9837 call `PacketReader.Need` f9830 first, and `Need` throws `PacketException` when `Offset + n > Data.Length`. So kind 3 needs 8 bytes, every other kind needs 14, and any tail is accepted.
- **A throw drops only that handler's work.** `GameSession.Handle` runs each handler inside a `try` at `0x65dfe7`. Its `catch` at `0x65dff6` logs the exception and continues, so the client drops that frame's combat message and keeps the session.
- **The fingerprints did not move.** `tools/build-migrate check` reports `MATCH` for `ParseMessage`, `ParseSwing`, `CombatLink.Receive` and `GameSession.Handle` against their reviewed 8502ebbf fingerprints. `U16At`, `I32At` and `ReadPos` have no reviewed fingerprint. The same run fails on three pins that none of these methods uses: the size and SHA-256 of the deployment `index.html`, and `pins.definitions`.

To reprint the read sequences, set `CACHE` to a `tools/build-migrate` cache of this build and run this from the repository root:

```sh
python3 -I - "$CACHE" <<'EOF'
import sys
sys.path.insert(0, "tools/build-migrate")
from buildmigrate import derive
b = derive.Build(sys.argv[1])
for name in ("[Spinel.Net.dll]Spinel.Net.Game.Combat::U16At(System.Byte[],System.Int32)",
             "[Spinel.Net.dll]Spinel.Net.Game.Combat::I32At(System.Byte[],System.Int32)",
             "[Spinel.Net.dll]Spinel.Net.Game.Combat::ParseMessage([Spinel.Net.dll]Spinel.Net.NetFrame)",
             "[Spinel.Net.dll]Spinel.Net.Game.Combat::ParseSwing(System.Byte[])",
             "[Spinel.Net.dll]Spinel.Net.Game.Messages::ReadPos([Spinel.Net.dll]Spinel.Net.PacketReader)"):
    _, (_, function, start, _) = b.function_of(name)
    print("== f%d @0x%x %s" % (function, start, name))
    for offset, text in b.fingerprinter.tokens(function)[1]:
        print("%x %s" % (offset, text))
EOF
```

## State the client changes

`CombatModel.ApplyMessage` f17568 applies the parsed messages:

- 36 calls `SetMyHp` f17545 at `0x5479d5` with the value.
- 37 stores the value as the cached MP, clamped between 0 and the known maximum (`Math.Min` at `0x547a1f`, `Math.Max` at `0x547a42`).
- 32 compares `Actor` with the session's own ID at `0x54797a`. For the own actor in avatar state 2, it sets state 0 with `AvatarFsm.Set` f13839 at `0x547992`. For any actor, it then removes `Actor` from a `Dictionary<int, int>` at `0x5479c3`.
- 43 calls `CombatModel.Faint` f17563 at `0x547954` with `Target` and byte 2.

Net reads only what it uses: the value on 36 and 37 for own HP and MP, the actor on 32 to end a pending own basic attack, and the target on 43 for own HP 0 and actor state. It ignores byte 2 on 43. The combat correlator reads the attacker, kind and target of grouped strikes, as before.

## Change

- In `Decoder::apply` (`crates/net/src/decode.rs`):
  - 36 and 37 read the 4-byte value and no longer check the end.
  - 32 and 43 read their actor ID with `padded_u16`, which reads like `U16At` at offset 0, and accept any length.
  - A new arm for 31, 485 and 1438 requires the 4 bytes of attacker and kind, then 4 more bytes for kind 3 or 10 more for any other kind, and accepts any tail. It applies with any Aux, as `CombatLink.Receive` does.
- In the combat correlator (`crates/net/src/combat.rs`), a grouped strike no longer ends with an end check after its optional U16 and U8.

## Verification

- **Synthetic.** The first three tests send frames through `Decoder::apply` with Aux 0 and with Aux 1. With Aux 1, strikes and 43 also pass through the combat correlator.
  - `hp_and_mp_need_four_bytes_and_accept_any_tail`: 36 and 37 must fail with 0 to 3 bytes, and succeed with 4 to 8 bytes and with the 32745-byte payload maximum.
  - `attack_end_and_faint_accept_every_length`: 32 and 43 must succeed with 0 to 4 bytes and with the maximum. A grouped 43 starts at 2 bytes (see [Limits](#limits)). After a 230 sets the own ID to 7 and a 36 sets own HP, a 1-byte 43 that holds 7 must set own HP to 0.
  - `strikes_need_the_client_minimum_and_accept_any_tail`: 31, 485 and 1438 must fail below 8 bytes for kind 3 and below 14 bytes for kind 2. They must succeed with 8 to 13 bytes for kind 3, with 14 to 19 bytes for kind 2, and with the maximum.
  - `a_grouped_strike_needs_14_bytes_and_attributes_with_any_tail` drives the correlator directly. A grouped own strike must fail with 0 to 13 bytes. With 14 to 19 bytes and with the maximum, it must still attribute exactly one own hit.
- **Mutations.** Each mutation below makes at least one test fail:

  | Mutation | First failing case |
  | --- | --- |
  | Restore the end check on 36 and 37 | `hp_and_mp…`: 36 with 5 bytes |
  | Drop the 4-byte minimum on 36 and 37 | `hp_and_mp…`: 36 with 0 bytes |
  | Restore the strict 2-byte read on 32 | `attack_end_and_faint…`: 32 with 0 bytes |
  | Restore the strict 2- or 3-byte read on 43 | `attack_end_and_faint…`: 43 with 0 bytes |
  | Restore the correlator's end check | `a_grouped_strike…` at 18 bytes, and `strikes_need…`: kind 2 with 18 bytes and Aux 1 |
  | Remove the new strike arm | `strikes_need…`: kind 3 with 0 bytes and Aux 0 |
  | Require 14 bytes for kind 3 | `strikes_need…`: kind 3 with 8 bytes |
  | Require only 8 bytes for kind 2 | `strikes_need…`: kind 2 with 8 bytes |
  | Make a short padded read return 0 | `attack_end_and_faint…`: the 1-byte own faint |
  | Make the padded read big-endian | `attack_end_and_faint…`: the 1-byte own faint, and the existing 32 check in `native_walk_and_pending_actions_require_own_source_evidence` |

## Limits

- A grouped 43 (nonzero Aux) shorter than 2 bytes still fails. The correlator in `crates/net/src/combat.rs` reads its target as a full U16, while the client parses it as a `Faint` with a padded target. This change touches only the strike end check in `combat.rs`, because another branch is changing the correlator. The fix is to read the 43 target there the way `padded_u16` does.
- Net still ends the session on a frame below these minimums, while the client logs that frame and drops it. That difference predates this change and applies to every decoder failure. The new strike arm extends it to ungrouped strikes, which Net did not check before.
- Byte 2 on 43 is the second argument of `CombatModel.Faint`, and its meaning is unknown. The meaning of any tail after the fields above is also unknown.
- This change rests on static reads and synthetic checks. No live session ran after it.
