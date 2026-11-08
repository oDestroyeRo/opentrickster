# MVP architecture

OpenTrickster has one deterministic automation engine, one active game adapter,
and a macOS application. The engine decides what should happen. The adapter
performs an action and reports what was observed. The application displays the
engine's snapshot and forwards operator commands.

Implementation is authorized by [Deliver the two-mode OpenTrickster MVP](https://github.com/oDestroyeRo/opentrickster/issues/19).
The visual reference is [DESIGN.md](../DESIGN.md).

## Usage

Buttons and menu items send the same command through the desktop controller.
The runtime stamps a command with monotonic time and a request key, calls
`Engine::handle`, executes the returned effects, and publishes the returned
snapshot. Adapter callbacks and timer deadlines use that same input path.

```text
button or menu -> controller -> Engine::handle -> session effect -> adapter
game observation -----------> Engine::handle -> snapshot -------> dashboard
```

The engine also accepts these inputs without a window or a network. Its tests
exercise actual decisions and observable effects using synthetic domain facts.

## Ownership

| Workspace member | Owns | Public boundary |
| --- | --- | --- |
| `crates/engine` | Lifecycle, setup, world state, activity planning, pending action, deadlines, run counts and control availability | Domain commands, observations, effects and snapshots |
| `crates/net` | Build profiles, codecs, authentication, transport accounting, protocol state and raw-to-domain identity mapping | Native session and passive game observer |
| `crates/desktop` | GPUI, native child webview, controller, one concrete adapter resource, menus, forms, persistence and alerts | Application executable |

The runtime executes effects in order. It does not have a second Ready/Running
state machine. A closed adapter sum permits either a native session or an
embedded client, never both. Session keys fence late callbacks. A mode change
closes the previous adapter before opening another one, and is unavailable
while a run exists.

Clientless mode gives the native adapter ownership of all of its connections,
authentication values, gateway byte counters and cleanup. Client-assisted mode
leaves those responsibilities with Unity. Its observer mirrors the game stream
and its action backend uses normal game input. Raw socket injection is excluded:
it would bypass the game's retained-byte accounting. A different gameplay bridge
requires source and runtime proof before replacing that backend.

## Action and evidence contract

- Reserve one logical action before executing its effect. Action and session
  keys are local capabilities, not invented server request identifiers.
- Receipt states distinguish not sent, submitted, actually sent and uncertain.
  An adapter never sends the same action key twice.
- Preserve an observation's original ingress mark through Aux buffering. A
  successful send supplies a mark from the same adapter ordering domain. Only
  relevant, attributable evidence after that mark can confirm an action.
- Preserve its original receive time too. Queue draining must not make old game
  state fresh. The engine's logical clock stays monotonic while freshness and
  entity stamps use the observation's original time. Runtime submissions use
  current application time, including time spent waiting in an adapter queue.
- Buffer a bounded number of relevant early observations until the send receipt
  arrives. Overflow or uncertain attribution pauses automation.
- Deduplicate stable evidence keys, including out-of-order delivery. A latest
  sequence number alone cannot deduplicate independently flushed Aux groups.
- Keep dig feedback, ground items, pickup feedback and inventory mutations
  distinct. Inventory goals use received inventory changes, not captions.
- Inventory list rows use `Fact::InventorySnapshot`: merge received rows without
  clearing absent rows, asserting completeness or granting goal credit. Ordinary
  mutations retain their original ingress in a bounded source-order journal.
  `Fact::InventorySettled` proves that every inventory frame through its mark
  has been delivered, including partial/raw/Aux queues. Only settled deltas after
  the run's send boundary earn credit. A goal waits for unsettled inventory;
  metadata alone cannot refresh state. Late mutations behind a proven frontier
  or journal overflow invalidate inventory until the session is reconstructed.
- Pause and Stop drain only the current action. A tagged quiescence barrier
  establishes idle state; an old barrier cannot clear a later action. If idle
  cannot be established within the drain deadline, close the adapter.
- Deadlines never authorize replay. Recovery with a held run returns paused.
  A fresh baseline and verified idle state are required before Resume.
- Keep unknown resources, maxima, inventory and capabilities explicit. Unknown
  is not zero, an empty inventory, a valid target or a completed action.
- Physical gameplay input fences queued gestures and quarantines ambiguous work
  before the engine pauses with a manual-input reason. Host work carries page,
  session, layout and expiry fences. A native mode-switch sheet obtains the
  operator's choice before submitting the confirmed switch command.

The adapter validates bytes, origins, build profiles and raw identities at its
boundary. Wire types and platform objects do not enter the engine. Domain labels
may appear on the local dashboard; credentials, tickets, raw identifiers, packet
bodies and whole snapshots never enter diagnostic logs or committed fixtures.

## Host evidence

A temporary native program compiled and ran with published `gpui = 0.2.2`
(`runtime_shaders`, no default features) and `wry = 0.57.1` (`os-webview`) on
arm64 macOS 27.3 with Rust 1.99.1 and Apple Command Line Tools. It verified that
the child webview's parent is GPUI's borrowed NSView, rendered animated WebGL2,
exchanged page/host messages, and booted the real target to its title screen.
Window resizing through GPUI updated the child rectangle. A native WKWebView
snapshot was inspected. This proves the hosting path; gameplay input, menu
routing, fullscreen and graceful teardown still require application checks.

## Synthesis decision

The reducer/effects candidate is the base. Compared with the cohesive session
actor candidate, it exposes a small deterministic testing boundary and gives
independent engine, network and desktop workers explicit ownership. The runtime
still offers a small controller/snapshot facade to the UI.

The actor candidate contributes original-ingress watermarks, buffering early
outcomes until an actual-send receipt, explicit uncertain delivery, and safe
typed logs. Both candidates agree on one adapter, normal assisted input and no
raw socket injection. An independent comparison also selected the reducer base.
Its additional corrections are explicit manual-input arbitration, native mode
confirmation, inventory knowledge and concrete resume blockers. Both candidate
reducers can be tested without asynchronous IO; the choice is about module and
worker boundaries, rather than whether an actor can contain a pure reducer.

We accept an explicit effect boundary in exchange for deterministic policy
tests. We accept conservative pauses where attribution or quiescence cannot be
established. These are implementation constraints, not permission to omit either
operating mode from MVP completion.

## Implementation reconciliation

The foundation PR replaces the temporary type scaffold with a working reducer
and public-boundary trace tests. It adds typed manual input, explicit inventory
knowledge, concrete blockers, local capability getters and a received inventory
projection. Queued source-time preservation was reproduced with a failing trace
and corrected before integration. Independent review also reproduced old-map
facts, contradictory terminal evidence, reordered inventory updates and lifecycle
edge cases. Corrections fence map-scoped facts, retain Stop through resource
closure, reset successful reconnect budgets and settle inventory in source order.
The inventory snapshot/frontier facts are additive adapter contracts. These
changes preserve the original ownership and effect boundaries.

Shared signature changes must be reconciled here and communicated to other
implementers before integration. A completed MVP contains no placeholder
adapters or synthetic results presented as live gameplay.
