# Damage-event tail on opcode 42

Build **`ea1a68bd89958fae94179f1e0835ffb5`**, 2026-11-10. This report is static research with synthetic checks. It contains no credentials, payloads, UIDs, item names or character values.

**Result.** The client parses a game-link 42 with `Combat.ParseDamage`. That method reads 26 bytes, reads a 2-byte `Guard` only when at least 2 bytes remain, and never checks the end of the payload. So the client accepts any 42 of 26 bytes or more and ignores the tail. Shorter payloads throw. Net's former arm rejected every tail except one 2-byte `Guard`, so a live 30-byte 42 ended a client-assisted session with `Malformed`. Net now requires 26 bytes and accepts any tail, as the client does.

## Static evidence

Every method below is on build ea1a68bd and is identified by its function index. Offsets are WASM code offsets.

- **Only `CombatLink.Receive` handles 42.**
  - `Combat..cctor` f10808 fills `Combat.Opcodes` from a 24-entry RVA array: 31, 42, 43, 32, 36, 37, 38, 1030, 39, 286, 289, 163, 761, 763, 151, 91, 224, 292, 485, 1438, 923, 668, 719 and 280.
  - `CombatLink..ctor` f10809 subscribes `CombatLink.Receive` f10820 to each entry through `IGameSession.OnPacket`, at `0x31ebd0`.
  - No other `OnPacket` or `OnSystemPacket` caller names 42, either as a constant or in an RVA array. The other subscription methods, `GameSession.On` f20883 and `GameplayScreen.OnZone` f38462, take no opcode.
  - `GameSession.Handle` f20915 has no case for 42. Its `br_table` at `0x65e19a` sends 42 to the default.
- **42 goes to `ParseDamage`.** `CombatLink.Receive` calls `Combat.ParseMessage` f10804 at `0x31fc2d`. In `ParseMessage`, the `br_table` at `0x31e405` on `opcode - 32` sends 42 to the `Combat.ParseDamage` call at `0x31e488`. `ParseMessage` is the only caller of `ParseDamage`.
- **`ParseDamage` f10797 reads `Target`, then calls `ReadDamage`.** It makes a `PacketReader` over the payload, reads U16 `Target` at `0x31de84` into `CombatMessage/DamageEvent.Target`, and calls `Combat.ReadDamage` at `0x31deb8` for `DamageEvent.Damage`. It returns the event with no further read.
- **`ReadDamage` f10798 reads 24 bytes, then an optional `Guard`.** It reads, in order:
  1. I32 `Amount` at `0x31deec`;
  2. one U32 at `0x31def5`, which it discards;
  3. U32 `KindBits` at `0x31defd`, with `Kind = KindBits & 127`;
  4. U32 `Element` at `0x31df06`;
  5. U32 `Flags` at `0x31df0f`;
  6. U32 `Extra` at `0x31df18`;
  7. `PacketReader.get_Remaining` f9838 at `0x31df21`, which is `Data.Length - Offset`, and only when that is at least 2, U16 `Guard` at `0x31df2d`. `Guard` is 0 otherwise.

  It then returns. Neither method compares `Offset` with `Data.Length` after the last read, so any bytes after `Guard` stay unread.
- **A payload under 26 bytes throws.** `PacketReader.U16` f9827, `I32` f10035 and `U32` f9829 each call `PacketReader.Need` f9830 first. `Need` throws `PacketException` when `Offset + n > Data.Length`. `GameSession.Handle` runs each handler inside a `catch` at `0x65dff6`, logs the exception and continues, so the client drops that frame and keeps the session.
- **The fingerprints did not move.** `tools/build-migrate check` reports `MATCH` for `ParseDamage`, `ReadDamage`, `ParseMessage`, `CombatLink.Receive` and `GameSession.Handle` against their reviewed 8502ebbf fingerprints.

`ReadDamage` is also called by `ParseCounterDamage` f10799 for 923 and `ParseGuardDamage` f10800 for 668. Net decodes neither opcode.

To reprint both read sequences, set `CACHE` to a `tools/build-migrate` cache of this build and run this from the repository root:

```sh
python3 -I - "$CACHE" <<'EOF'
import sys
sys.path.insert(0, "tools/build-migrate")
from buildmigrate import derive
b = derive.Build(sys.argv[1])
for name in ("[Spinel.Net.dll]Spinel.Net.Game.Combat::ParseDamage(System.Byte[])",
             "[Spinel.Net.dll]Spinel.Net.Game.Combat::ReadDamage([Spinel.Net.dll]Spinel.Net.PacketReader)"):
    _, (_, function, start, _) = b.function_of(name)
    print("== f%d @0x%x %s" % (function, start, name))
    for offset, text in b.fingerprinter.tokens(function)[1]:
        print("%x %s" % (offset, text))
EOF
```

## State the client changes

`CombatModel.ApplyMessage` f17568 passes the event to `CombatModel.ApplyDamage` f17555 at `0x547803`. That method changes only cached client state:

- for the own actor, it calls `SetMyHp` f17545 at `0x546660` with the cached HP minus `max(0, Amount)`;
- for a monster, it calls `UpdateMonsterHp` f17544 at `0x5466bc`, starting from `MonsterMaxHp` f17540 when no HP is cached;
- for another player, it updates `PlayerHpRecord` f17546 at `0x5466f1`.

Net needs none of this state. It takes HP only from values the server reports in 233, 36, 43, 286 and 289, never from damage. The combat correlator still reads `Target` and `Amount` from 42, as before.

## Change

The 42 arm in `Decoder::apply` (`crates/net/src/decode.rs`) now requires the 26 bytes of `Target` and the damage block, and accepts any tail. It reads no `Guard`, because Net has no use for it.

## Verification

- **Synthetic.** `damage_accepts_every_length_the_client_parses` sends 42 through `Decoder::apply` with Aux 0 and with Aux 1. With Aux 1 the combat correlator also reads `Target` and `Amount`. Every length from 0 to 25 must fail. Lengths 26 to 31 and the 32745-byte payload maximum must succeed.
- **Mutations.** The test fails on the former arm, which rejects 30 bytes. It also fails with a strict end after 26 bytes, with no minimum length, and with an end check in the correlator after `Amount`.

## Limits

- Net still ends the session on a 42 shorter than 26 bytes, while the client logs that frame and drops it. That difference predates this change and applies to every decoder failure.
- In a 30-byte 42 the client reads `Guard` from bytes 26 and 27 and leaves bytes 28 and 29 unread. Their meaning is unknown.
- The live failure occurred once in two in-world sessions. No live session ran after this change.
