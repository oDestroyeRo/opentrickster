# MVP behavior and acceptance

The application controls one existing character in either client-assisted or
clientless mode. Combat and digging use the same engine and configuration.
Client-assisted mode embeds the real game inside the macOS window. Clientless
mode runs without Unity and displays received game state in the native dashboard.

## Defaults

- Combat uses basic attacks on eligible configured monsters. Prefer the nearest
  valid target inside the current map's configured boundary. Skip targets known
  or suspected to be engaged by another player. Do not chase beyond the boundary.
- Digging repeats normal manual-point digging in a bounded area. A drill cycle
  includes its normal client-side progression and server result; sending only a
  start request is insufficient.
- Configure the area before starting. Coordinates use the target protocol's
  game-coordinate counts, with map identity included in the boundary.
- Keep one logical action outstanding. Movement, attacking, digging, pickup and
  resource use require an implemented, evidence-backed adapter capability.
  Without a pickup capability, drops stay on the ground and never block a run,
  but a combat goal cannot start because only pickups count toward it.
- Defaults are 15 seconds for state freshness, 2 seconds for a send receipt,
  5 seconds for ordinary action confirmation, 30 seconds for a dig cycle and
  2 seconds for establishing quiescence. Confirmation and dig windows start at
  the actual send, not at the ticket's issue. These are conservative application
  defaults, not server guarantees; supported normal actions may require measured
  adjustments recorded with their evidence.
- State is stale when 15 seconds pass with no received state change and no
  received game frame that the adapter has applied in order after the baseline.
  An idle character on a live game link therefore stays fresh. A silent link,
  an unfinished baseline, a disconnect, a reconnect or a closing session never
  refreshes state.
- An expired quiescence window closes the session only while an action of ours
  may be in flight or a Stop is draining. Otherwise the app keeps the session,
  repeats the same quiescence request in fresh 2-second windows, and holds the
  next action until the game is idle.
- Stop a run after three consecutive correlated rejected actions. A confirmed
  action resets the rejection streak.
- Pause on fainting, uncertain state, resource or drill limits, a reached goal,
  manual gameplay input and after reconnect. Every pause carries a reason and
  an observable condition for resuming.
- Pause permits setup edits that apply on Resume. Activity stays fixed until
  the run ends. Mode changes require ending the run and closing the session.
- Never purchase items, revive a character, accept/turn in quests, take over a
  busy session or replay an uncertain gameplay action.

The seven confirmed policies, themes, layout and shortcuts remain governed by
[DESIGN.md](../DESIGN.md). Proposed thresholds and activity details in that file
are not fabricated game mechanics. Show unsupported or unobserved values as
unknown, and expose the actual condition blocking an action.

## Completion evidence

Completion requires appropriate automated tests, reviewed and merged production
PRs, and a runnable packaged macOS app. Runtime acceptance must establish:

1. The real target boots in the embedded game pane, with correct placement,
   resizing, focus, controls and child-before-parent cleanup.
2. Native authentication selects an existing character and receives its actual
   initial state without running the game client.
3. Each adapter performs a bounded monster attack and a completed dig, with
   relevant server state and inventory outcomes distinguished from local sends.
4. Start, Pause, Resume, Stop, setup locking, stale-state handling, rejection
   limits, reconnect pause and mode/session ownership work through the app.
5. Credentials remain in memory for the current session only, outside Git,
   logs, verification artifacts and disk; game values remain in the local session.

Synthetic traces prove engine/parser behavior only. An attractive dashboard,
successful compilation or a logged-in visual game alone does not prove the MVP.
