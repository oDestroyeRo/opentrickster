# Native network boundary

This crate implements the pinned gateway and existing-character handshake for
build `8502ebbf2fc9033d2fba50d02fd92002`. It also provides a passive decoder for
the embedded game. It does not yet complete issue #21 or the MVP.

`src/lib.rs` documents the public integration signatures. `fetch_public` and
`load_cache` block and belong off the GPUI thread. `open` starts a private Tokio
runtime thread. `submit_at` supplies the engine's monotonic clock for gameplay
deadlines. `try_next_at` returns each observation with its original ingress time
in the caller's clock domain. Runtime consumers use that method rather than
refreshing queued facts at poll time. `try_next` remains an inspection convenience. `close` has a
2.5 second local cleanup wait. A timeout preserves the worker handle, and a
later `Closed` is emitted only after the worker has exited and joined.

The native actor revalidates fresh official config, version, and page asset
references before opening each authentication attempt. A cached profile alone
does not authorize connecting to a changed deployment. Assets and compiled
marker/version fields require the current fingerprints. Public assets stay in
a caller-provided cache outside Git. Credentials, login keys, tickets, gateway
tokens, raw identity maps, and packet payloads remain private. Secret buffers
use `Zeroizing`; none of those types implements `Debug` or `Serialize`.

The gateway uses the current directional maps, a six-byte header, and byte
accounting that includes headers. A binary WebSocket message is a stream chunk.
Raw authentication waits run before Aux session grouping. Aux groups retain
original first-byte ingress marks and receive times, including incomplete headers.
Facts use even evidence keys; baselines use odd keys to avoid collisions with
frames already decoded in the same stream chunk. The native liveness policy treats its actor as visible.
It sends probes after five seconds of silence and requires a pending probe,
two ticks, and another five seconds before reporting loss. It retains at most
1 MiB, accepts stale ordinary ACKs, and rejects ACKs above bytes sent. It never
resumes a gateway or replays uncertain gameplay. Only explicit engine recovery
effects can begin another physical authentication attempt, up to five attempts.

The observer supports eight concurrent connection generations. The actual
outbound GS login selects its gameplay stream. `disconnect` retires a generation;
late callbacks cannot enter a replacement stream. An observer-only `Closed`
proves decoder cleanup. The desktop must wait for its webview and Unity resources
before forwarding adapter closure to the engine. The observer neither opens
sockets nor sends ACKs. Arming a movement or attack ticket reserves correlation;
only a matching observed send produces a `Sent` receipt. An ordinary attack
requires both target 288 and attack 29. An observed gun 477 uses its distinct
seven-byte layout. Neither a receipt, damage, faint, nor despawn awards a kill.
The host must copy each actual socket callback once and preserve its connection
generation. This API does not authenticate an arbitrary page's self-reported
origin or deduplicate duplicated host callbacks.

The decoder handles the full startup field extents, strict entity/effect/player
branches, movement, HP/MP, faint/despawn, inventory lists and deltas, equipment
references, ground items, pickup captions, and manual dig feedback. Every actor
appearance gets a new local incarnation. Missing 231 remains unknown. Multiple
231 lists merge; they never clear previously observed items. A valid empty list
records that a list was seen without asserting inventory completeness. Missing
equipment references stay unresolved at their original replay point. HP/MP
maxima, weight, potion counts, and drill metadata stay unknown.

## Remaining prerequisites

Native movement, ordinary attack, and a full timed manual dig have sender paths.
The current profile loader does not yet resolve public gameplay definitions, so
its support flags remain false and unavailable tickets receive `NotSent`. A
private `GameDefinitions` boundary accepts source-verified item type/subtype,
gun slots, drill rows, zone classifications, and movement mode. It has no fixture
defaults. The root owns the pending definition resolver.

The manual controller follows 44, accepted 45 and own 371, measured timed gauge
updates, one 49 completion request, and actual 50 result. It uses the exact
f32/f64 arithmetic and real cancellation counters. Local drill wear is not
reported as a server inventory delta. Completion and cancellation are never
replayed. An active dig can satisfy a tagged barrier only after its actual result;
no cancellation-ack grammar or general idle proof is claimed.

| Capability | Missing prerequisite |
| --- | --- |
| Native movement | A proved current movement-mode source and held-own-stop/cancellation ordering. |
| Native ordinary attack | Populate the verified item metadata subtype lookup and configured equipment-slot list needed by current `GunFlowing.HoldsGun`. A gun must not take the ordinary path. |
| Native gun attack | Current ready/gauge parameters, key, bunch/ammunition, and progression. The static CRC and serializer alone do not implement that flow. |
| Complete dig cycle | Populate verified drill parameters, item Type 6, movement mode, and zone classifications; receive the slot-7 UID and actual item Count as life. The full timed exchange is implemented and tested with synthetic fixtures. |
| HP potion | Source-backed use path and item metadata that identifies HP potions. Consumable kind alone is insufficient. |
| Pickup | Source-backed normal outgoing path and action attribution to actual inventory changes. A pickup caption is not inventory receipt. |
| Quiescence | A proved stop/cancel path and barrier evidence. Silence is not proof of idle. |
| Action outcomes | Attacker-attributed hit/kill evidence, ordered movement completion, and result-to-ticket attribution. HP reduction and faint alone are insufficient. |
| Live acceptance | Root's serialized account lease and an authorized runtime handoff. No account, game socket, or live gameplay was used for this implementation. |

An absent or unrecognized optional character-status trailer remains Unknown.
The approved normal UID-only selection is allowed without a positive busy signal.
`CharacterChoice.busy=false` never asserts idle. An advertised malformed v1 trailer,
unrecognized security state, positive failure count, occupied error 60013, and
gateway busy stop the attempt. Known security state 0 follows the explicitly
approved local decline path without a setting-change packet. The adapter does
not configure a second password, evict a session, retry release-held tokens,
or assert server logout after local cleanup.

## Evidence

The codec and auth contracts come from the
[current-build revalidation](https://github.com/oDestroyeRo/opentrickster/blob/df5fe0a5e8e45072845538fc74967a1757b6d9d7/docs/research/current-build-protocol-revalidation.md).
The manifest was independently checked against SHA-256
`f258e7ac8a99566adfd4019020a03cfc1d61e56a66d101ddbd0319a6bf756b84`
before implementation. The state parsers use the
[completed state report](https://github.com/oDestroyeRo/opentrickster/blob/cb587e235b113aa26b04db018c31f1e26510f5bd/docs/research/target-state-completion.md)
and its linked manual-state report. The timed controller uses the
[manual digging report](https://github.com/oDestroyeRo/opentrickster/blob/b1a12a97420fff84f15cc1e3cf8b5070560ed7b9/docs/research/manual-dig-control.md).
Its immutable report and seven selected current method/body joins were checked.
The independent item, empty-bottom, and drill-limit fixtures produce the same
49 bytes and callback counts in Rust. These are synthetic checks, not live digs.

The ordinary and gun serializers were independently rejoined to the current
fingerprinted WASM. Current qualified method rows and original body hashes are:

| Method | Metadata row | Function | Original body | SHA-256 |
| --- | ---: | ---: | --- | --- |
| `GameSession.SendMove` | 72109 | 13282 | `0x3af3d2`, 231 bytes | `afc0f5d09209358b022c5206c66b19c822b422c9293e59ad50ccead4ad6beb27` |
| `CombatLink.Attack` | 71389 | 13224 | `0x3ab9b0`, 386 bytes | `896c77ef2b3c6126729e9c5b43300cade7b9eb58f737d20f513b399d49ae1f7d` |
| `GunFlowing.HoldsGun` | 40842 | 22105 | `0x6c92d4`, 614 bytes | `3f0fc813b866cf9fa110a49956c2ecac16984d12b127d58b2b3cb91899860d72` |
| `GunFlowing.Shoot` | 40858 | 22123 | `0x6ca3c6`, 427 bytes | `59a8337b00214947a04b04793224d7866000a63aaf77e329248f69a5ce5e6b95` |
| `Gun.Crc` | 72623 | 11831 | `0x35756a`, 30 bytes | `6dcbf85444a0383346b827c7555c8749e0979b27e8ceae970dd4cc17bc2aca02` |
| `Gun.Shot` | 72624 | 11832 | `0x357589`, 89 bytes | `21d30c791e936c978358415f734b0ae547db96031564eda61a796f74910a3269` |

`SendMove` writes four U16 values and a U8 mode, then sends logical 24 at
`0x3af467`. The domain uses integer coordinates, so fractional origin rounding
is not introduced. `Attack` sends 288 at `0x3aba13`, then 29 at `0x3abac5`.
`CombatModel.WorldClick` calls that function at `0x53e236` after its gun
interception. `HoldsGun` traverses configured equipment slots and resolves an
item metadata subtype, comparing it with 54 at `0x6c950d` through `0x6c9512`.
The old build's wire opcodes are never used.

Account-free checks are reproducible with `cargo test -p opentrickster-net` and
`cargo clippy -p opentrickster-net --all-targets -- -D warnings`. The ignored
`verified_public_cache` test additionally requires `OPENTRICKSTER_PUBLIC_CACHE`
to name an outside-Git cache with the pinned public files. It reads the cache
without making a network request or reading credentials. Loopback WebSocket
tests prove local transport and handshake behavior with synthetic identities;
they do not establish target-server acceptance.
