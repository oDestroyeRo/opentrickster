# Native network boundary

This crate implements the production gateway, existing-character authentication,
state decoder, public catalog resolver, and passive embedded input boundary for
build `8502ebbf2fc9033d2fba50d02fd92002`. It is a partial delivery of #21.
The remaining native action paths and live acceptance belong to #21 and #23.

## Runtime ownership

`VerifiedProfile::load_cache` and `fetch_public` block and run off GPUI. They
validate the official deployment, public asset fingerprints, compiled marker
and client-version fields. The Rust catalog resolver validates the encrypted
public responses, IIDX EOF/read-set/runtime/overlay stamps, XTEA table hashes,
row counts and first-duplicate gun selection. Cached assets and extracted keys
stay outside Git. Static definitions never become received character state.

`NativeSession::open` starts one private Tokio runtime thread. It registers
startup waits before FL, LS and GS sends, consumes coalesced 2014 and 3 in raw
order, and requires accepted 5 plus all 679/230/233 before its baseline.
It then sends the normal empty C5 world-entry frame. This local milestone does
not prove inventory completeness or an accepted gameplay action.
`submit_at` supplies the engine clock at actual enqueue time. `submit(Act)`
refuses. `try_next_at` preserves original receive age across every queue.

`NetCredentials` is an opaque cloneable secret capability. It retains only
normalized ID and derived key in zeroizing owners. The password is zeroed in
`new`, and every clone zeroes on drop. Desktop retains one selected-account
capability, clones only after the previous native resource closed and joined,
and clears it on disconnect, mode change, quit or failed authorization.
There is no raw getter, Debug or Serialize implementation.

The native actor revalidates fresh config/version/page references before each
physical authentication attempt. Occupied 60013, gateway busy, refusal, any
second-password state other than 0 or a nonzero failure count stops it. Optional
absent character status remains Unknown; `CharacterChoice.busy=false` means no
positive busy signal. Only security state 0 has a recovered client path: its
explicitly approved decline uses UID-only 2012 without changing settings.
There is no takeover, release-held retry or automatic uncertain gameplay replay.
Only engine recovery effects authorize another physical attempt, up to five.

A ticket's `not_after` bounds only the send receipt. After the actual send, the
engine's `ACTION_MS` (5 s) or `DIG_MS` (30 s) window bounds completion. The
native adapter starts its window at the send, so it closes no later than the
engine's. When it closes without a correlated outcome, the adapter cancels a dig
with the normal 48 request, expires the pending action and attributes no later
evidence to it. `Quiesce` also cancels an active native dig. `Quiescent` is
reported only when no action is outstanding; otherwise the engine's 2 s
`DRAIN_MS` closes the session.

`close` waits up to 2.5 seconds and retains its worker on timeout. A finished
worker can join without a done signal, including after panic. Native `Closed`
requires the join and means local resource cleanup, never server logout. Only
this actor's own token can reach the bounded release request.

## Framing and provenance

The codec uses exact current directional maps, six-byte LE headers and a 32745
payload limit. Reassembly reserves the original ingress mark and Instant at
the first byte, including a partial header. Raw authentication waits precede
Aux grouping. Independent Aux groups preserve original marks; 1952's private
boundary resets attribution without producing a domain fact. Buffers, queues,
actors, items and frame groups have explicit bounds and fail closed.

Evidence keys use separate namespaces for packet facts, baselines, settlement
metadata and delayed correlation. `InventorySettled(mark)` follows all inventory
facts through that original mark and remains behind raw, partial and Aux work.
A latest observed mark is never a complete-through frontier. S282 resource
composites retain the older current HP/MP timestamp. A new maximum cannot
freshen or fabricate a change to an older current value.

Retention includes frame headers and partial ACK boundaries. Stale ordinary
ACKs are ignored; ACKs beyond sent bytes fail. Native liveness uses the current
visible-client policy. No gateway resume or uncertain outbound replay occurs.

## Passive page and normal input

`VerifiedProfile::observer_script` generates one document-start wrapper from the
verified maps. It recognizes the GS login from its six-byte header and never
copies that authentication body. Only allowlisted GS state/actions cross IPC.
FL/LS authentication bytes and text tokens never cross. Host epoch, guard,
connection generation, sequence and first-byte ordinal fence callbacks.

The same wrapper owns the guarded sampler and close request. It refuses new
sockets during closure and closes every owned socket, including unknown roles.
An ACK requires their actual close events. CLOSED state, an error, a timeout or
an unupgraded preconnect does not manufacture an ACK. Desktop additionally
neutralizes the document and detaches WK before forwarding engine Closed.
Observer-local `close` proves only decoder cleanup.

The sampler reads only pinned heap classes/fields with extent, RGCTX, version
and consistency checks. Its snapshots and Candidates stay private. It neither
calls target gameplay methods nor writes memory or sockets. Desktop dispatches
ordinary native input from the typed `InputPlan`.

The locked public methods are in `src/observer.rs`:

- `sample_script(request, layout_epoch)` requests one guarded source sample.
- `input_plan(now, fences, ticket)` prepares one event for one logical action.
- `input_dispatched(step_key, original_instant)` records actual native dispatch.
- `continue_input(now, fences, action)` returns the next event or None to wait.
- `active_input()` becomes None only when the controller has fully drained.
- `quiesce(now, fences, barrier)` permits owned ups and normal right-click drain.
- `emergency_released(action, original_instant)` records actual host neutralization.

Every plan contains exactly one event. Every dispatch invalidates the previous
sample AND its outstanding request. A newly requested post-dispatch sample must
prove the relevant Held/controller transition. A matching actual send can release
an ordinary mouse press sooner. RAF and delay are never receipts. Initial dig
selection is an ordinary click with a <=250ms release bound. Only actual gauge
Charging and Tracking holds are exempt. Held controls still require fresh samples.
Physical ASCII `Character` bindings mean DOM KeyA..KeyZ, never layout text.

Manual dig preserves one armed ticket across key targeting,44,45/own371, gauge
holds,49,50 and sampled releases. Normal right-down/up clears active digging or
remaining targeting. Source input-up,46,stage7 and silence are not idle proof.
Quiescent requires a new source idle sample, original ingress drained through
that sample, no unresolved action and no queued/held input. Planning NotReady,
Busy and Refused are transient errors and do not poison the decoder. Missing
receipt means owned emergency release plus uncertainty/drain, never replay.

## Received state and remaining scope

The decoder handles own profile/zone/resources, full typed entity appearance
and effects, moves, HP, faint/despawn, inventories/equipment, ground items and
dig feedback. Actor incarnations fence raw-ID reuse. Wire Kind, Count, state,
full option triplets and 23 raw slot UIDs remain private. Stack pieces use Count
only for wire Kind 1. Catalog Type/SubType remain separate.

231 is merge-only `InventorySnapshot`, never Reset or a grant. Missing and empty
lists stay distinct from complete inventory. Missing equipment references stay
unresolved at their original replay point. Current carried weight remains
unknown. H=max raw accepted 5/679/230/233 is UNACCEPTED until Root compares the
actual Rust decoder with Unity's synchronous cache boundary and weight oracle.

Native code sends24, ordinary288+29 and the full timed44->45/371->49->50 dig
exchange, including real48 cancellation counters. Native movement policy is
explicit WALK1. Assisted mode uses observed RunToggle/Heavy; it never inherits
that policy. Own hit/kill association requires one complete unambiguous own
Strike bundle with the matching recipient, action floor and incarnation.
Unattributed HP falls, captions and faint alone cannot award outcomes.

| Remaining part of #21/#23 | Current boundary |
| --- | --- |
| Sampler review | Root applied the geometry, type, and active-parameter corrections. Independent specification review and live correspondence remain open. |
| Native gun controller | Serializer and source-backed profile/key/skill/ammo prerequisites exist. Tick/CRC/effect-lifetime controller integration remains unfinished. A gun never falls back to melee. |
| HP potion | Catalog classification and precise missing instance/expiry/use-state reasons exist. Normal87 use and result correlation remain unfinished; no fake usable-potion fact. |
| Pickup | Ground state and own captions decode. Normal 33 send and inventory attribution remain unfinished; captions do not grant items. |
| Carried weight | Source31 gives a bounded comparison recipe; no accepted complete native inventory cutoff yet. |
| Runtime acceptance | One production Rust startup and joined cleanup passed. Gameplay, inventory readiness, and Desktop integration remain open. See the [runtime report](../../docs/research/native-rust-startup.md). |

These gaps remain visible in support/readiness and cannot be called a completed
MVP. S282 totals become unknown on source-backed recompute events. Potion
restoration arithmetic, gun defaults and full-inventory claims are not invented.

The catalog and input contracts use the immutable
[definition resolver](https://github.com/oDestroyeRo/opentrickster/blob/07bd3daf8f6dbbd6bcef3730269d0e6b2d7412c1/docs/research/target-game-definitions.md),
[normal input report](https://github.com/oDestroyeRo/opentrickster/blob/6f93a75a23849475c7f88d5f3b099d98a4a6c33e/docs/research/assisted-input-control.md),
[gun report](https://github.com/oDestroyeRo/opentrickster/blob/efccf7992b77966804e9078d4643a565aa3b6340/docs/research/gun-combat-control.md),
[own effect report](https://github.com/oDestroyeRo/opentrickster/blob/1614063ecbfa92836050eec8a545d0ae2b4fb663/docs/research/native-own-effect-lifecycle.md)
and [inventory baseline report](https://github.com/oDestroyeRo/opentrickster/blob/ce0cf202159c669832087a204db435d998ec2518/docs/research/native-inventory-baseline.md).

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
