# Native network boundary

This crate implements the production gateway, existing-character authentication,
state decoder, public catalog resolver, and passive embedded input boundary for
build `ea1a68bd89958fae94179f1e0835ffb5`. It is a partial delivery of #21.
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
It then sends the normal empty C5 world-entry frame. Like the client, it keeps
the LS link open and pumped until the GS link closes, answers 2028 only on the
LS link, and drops the LS link's other frames; see the
[login-link report](../../docs/research/native-login-link.md). This local
milestone does not prove inventory completeness or an accepted gameplay action.
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
reported only when no action is outstanding. If a window closes while a
`Quiesce` is pending, idle stays unproven: the adapter reports no further
`Quiescent`, and the engine's 2 s `DRAIN_MS` closes the session.

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
neutralizes the document and detaches WK before forwarding engine Closed. The
one exception is a terminated WebContent process: no page script remains to
send an ACK, so Desktop reports `ClientFailed`, releases owned input, detaches
the view and then forwards Closed. WebKit's networking process closes that
page's sockets shortly afterwards, so a quick switch to clientless mode can
still see the character as in use (`Occupied`) and must be retried.
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
dig feedback. Parsers are exact, with four exceptions seen in live ea1a68bd
traffic: a type-3 appearance and a removal on 15 may each carry one unread
trailing byte, a kind-2/3 ground item may carry its four unread state bytes,
and a damage event on 42 may carry any tail after its first 26 bytes, because
the client's `Combat.ParseDamage` has no end check
([damage-event tail](../../docs/research/damage-event-tail.md)). Like the
client's `EsBlock.Read`, an effect type outside the compiled
47-entry table has an empty body; it still marks the actor as affected, so that
actor is never targeted. Like `ReadPlayerBody`, player mask bit 128 and
equipment-mask bits outside the slot tables carry no fields. One live
appearance rejected by the former strict checks had ended the session. Actor
incarnations fence raw-ID reuse. Wire Kind, Count, state, full option triplets
and 23 raw slot UIDs remain private. Stack pieces use Count
only for wire Kind 1. Catalog Type/SubType remain separate.

231 is merge-only `InventorySnapshot`, never Reset or a grant. Missing and empty
lists stay distinct from complete inventory. Missing equipment references stay
unresolved at their original replay point.

Carried weight and the HP potion count are gated by `INVENTORY_CUTOFF_BUILD` in
`src/decode.rs`. It names build `ea1a68bd89958fae94179f1e0835ffb5`, accepted
after Trial A and Trial B passed ([trials](../../docs/research/native-inventory-trials.md),
procedure in the [inventory baseline report](../../docs/research/native-inventory-baseline.md)).
For any other build, both stay unknown and no `Weight` or `HpPotions` fact is
emitted, so a pin migration turns the cutoff off until the trials pass again.
When accepted:

- H is the last raw receipt among the first accepted 5, 679, 230 and 233. The
  decoded cache counts as complete once delivery has settled through H, and only
  if a 231 list arrived at or before H. An empty list then means an empty bag.
- Weight is the I64 sum of `ItemInfo.Weight` x pieces (Count for wire Kind 1,
  otherwise 1), reported as `u32`. Equipped and expiring items keep their weight.
  The maximum is S282 total 6, or unknown when absent or exceeded.
- `HpPotions` counts Kind-1 pieces that `potion_prerequisite` classifies no
  further than the unproven per-use state. Instance options, overrides, expiry
  and normal-form restrictions exclude a stack.
- Each later decoded delta, a 233 or a change of total 6 re-emits both facts.
- A missing definition, an unrepresentable sum or a delta before H reports
  nothing for the session. After a report, a late 231, an undecoded Inventory
  opcode, a row naming an uncached UID (including a 68 for a new UID) or a missing
  definition fails the session with `Uncertain`. The engine has no fact that
  forgets a value, so the session reports `Lost`; only a fresh startup after an
  engine reconnect reports them again.
- The page bridge forwards the undecoded Inventory opcodes header-only
  (`page::HEADER_ONLY`), so the assisted observer sees them and invalidates
  without any item data crossing IPC.
- Known limit: `ordered.rs` releases a buffered Aux group and later Aux-0
  packets in raw order, while the client applies Aux-0 packets at once. If both
  touch the same UID before the group flushes, the cache can diverge silently.
  The ea1a68bd trials saw no Aux-grouped frame, so this case is unexercised.

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
| Carried weight | The H cutoff is accepted for ea1a68bd: Trial A matched the client's cache and weight exactly, and a native-only boot matched Trial A. The maxima remain unknown without S282: Net models every packet-fed `CharacterStatus.Total` term, but `GameParts` is not provable from packets. See [derived maxima](../../docs/research/derived-resource-maxima.md) (#47). |
| Runtime acceptance | One production Rust startup and joined cleanup passed. Gameplay, inventory readiness, and Desktop integration remain open. See the [runtime report](../../docs/research/native-rust-startup.md). |

These gaps remain visible in support/readiness and cannot be called a completed
MVP. S282 totals become unknown on source-backed recompute events. HP, MP and
weight maxima are otherwise unknown, because their `GameParts` term is unproven. Potion
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
fingerprinted WASM. The [build `ea1a68bd` migration](../../docs/research/build-ea1a68bd-migration.md)
proved their normalized bodies unchanged from `8502ebbf`. Current qualified method rows
and original body hashes are:

| Method | Metadata row | Function | Original body | SHA-256 |
| --- | ---: | ---: | --- | --- |
| `GameSession.SendMove` | 72131 | 20903 | `0x65d730`, 231 bytes | `afc2879c67969afcb3a8fe98d0de800907e7ac6b8ed2008ec80305fbbc7280b7` |
| `CombatLink.Attack` | 71411 | 10814 | `0x31f22c`, 386 bytes | `48ca4eaf9704d4f735b03a03720ad3b867ffcc44d9ee7c8019e66d8c9c12ec93` |
| `GunFlowing.HoldsGun` | 40861 | 24383 | `0x77a669`, 614 bytes | `10abc2bbde9fb343a8a2bd8210c5e3e54140ffedc869f1650f4790800a8fd7cb` |
| `GunFlowing.Shoot` | 40877 | 24401 | `0x77b75b`, 427 bytes | `b0582e31b320f45c43fdc9b3e026b61bbb255196d8a465b22d3ad1beb754d370` |
| `Gun.Crc` | 72645 | 12914 | `0x398d39`, 30 bytes | `6dcbf85444a0383346b827c7555c8749e0979b27e8ceae970dd4cc17bc2aca02` |
| `Gun.Shot` | 72646 | 12915 | `0x398d58`, 89 bytes | `b8299b57e7f6a6920fb671275255d40ab233808be2a491877661842a93ed4726` |

`SendMove` writes four U16 values and a U8 mode, then sends logical 24 at
`0x65d7c5`. The domain uses integer coordinates, so fractional origin rounding
is not introduced. `Attack` sends 288 at `0x31f28f`, then 29 at `0x31f341`.
`CombatModel.WorldClick` calls that function at `0x54ab21` after its gun
interception. `HoldsGun` traverses configured equipment slots and resolves an
item metadata subtype, comparing it with 54 at `0x77a8a2` through `0x77a8a7`.
Earlier builds' wire opcodes are never used.

Account-free checks are reproducible with `cargo test -p opentrickster-net` and
`cargo clippy -p opentrickster-net --all-targets -- -D warnings`. The ignored
`verified_public_cache` test additionally requires `OPENTRICKSTER_PUBLIC_CACHE`
to name an outside-Git cache with the pinned public files. It reads the cache
without making a network request or reading credentials. When the deployment
changes, [`tools/build-migrate`](../../tools/build-migrate/README.md) re-derives
every pin from public artifacts and checks these Rust literals against them. Loopback WebSocket
tests prove local transport and handshake behavior with synthetic identities;
they do not establish target-server acceptance.
