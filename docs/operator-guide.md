# Operator guide

This guide is for one player who runs the packaged OpenTrickster app with one
existing character. [MVP behavior](mvp.md) defines the app's rules, and
[MVP acceptance](acceptance.md) records what has been verified. Labels in
**bold** and text in quotes appear in the app exactly as written here.

> **Current state:** the app cannot run gameplay yet. The official game moved
> to a build that this app does not support ([#37]), so both modes stop at
> **Game updated**. After that is fixed, Start stays unavailable until the app
> observes carried weight, inventory and, for combat, HP potions ([#21],
> [#31]). See [Limitations](#limitations).

## Install and open the app

The app requires macOS 14 or later. Build `dist/OpenTrickster.app` with
[Build and package on macOS](macos-build.md), or unzip
`OpenTrickster-macOS-arm64.zip` from a successful macOS CI run. The CI build
runs on Apple silicon only.

The app has an ad-hoc signature and is not notarized, so macOS blocks the
first open of a downloaded copy. Try to open it once, then open
**System Settings > Privacy & Security** and click **Open Anyway** for
OpenTrickster. A copy that you built yourself opens directly.

Before it opens a game or session, the app downloads the game's public files
to `~/Library/Caches/org.opentrickster.desktop/public` and checks them against
its pinned build. These files contain no account data.

## Choose a mode

The app starts in client-assisted mode each time it opens. Only one game or
session is open at a time; to change modes, see [Switch modes](#switch-modes).

| | Client-assisted | Clientless |
| --- | --- | --- |
| Game | The real game runs in the left pane. Use it to watch or play between runs. | No game client runs. A board shows the state the app receives. |
| Sign-in | You log in inside the game. The app never reads that password. | You enter your Game ID and password in the app. |

## Sign in

### Client-assisted

1. Click **Load game** in the game pane. The status shows **Loading game**,
   then **Log in to the game**.
2. Log in inside the game and choose your character.
3. Wait for the status **Ready**.

The toolbar badge reads **Keys go to Game** or **Keys go to Dashboard**. Press
⌃Tab or click the badge to switch. A click outside the game sends keys back to
the dashboard.

### Clientless

1. Enter your **Game ID** and **Password**, then click **Sign in**.
2. Under **Choose a world**, click a world.
3. Under **Choose a character**, click your character. A character marked
   **In use** cannot be chosen. To stop signing in, click **Cancel**.
4. Wait for the board to show your character and the status **Ready**.

The app keeps your sign-in in memory for the current session only and never
writes the password to disk. It clears the sign-in when you cancel, switch
modes or quit, and when the game rejects it, so enter the password again for
each session. While a session is open, the app reuses it to reconnect.

If sign-in fails, the status shows **Sign-in failed** and the form briefly
shows the cause, for example "Sign-in failed. Check the Game ID and password."
If it says "Complete the account security step in the game.", finish that
step in the game first.

## Configure a run

Open the **Setup** tab (⌘2) after your character is in the world. Your changes
take effect only when you click **Apply setup**.

1. Under **Activity**, choose **Combat** or **Digging**, then set the fields
   for that activity, as described below.
2. Stand where the run should be centered. Under **Where**, click
   **Use current location**. The form shows "Center X, Y".
3. In **Movement radius · game-coordinate counts**, enter the radius of the
   area around that center. The run stays on this map and inside this circle.
4. Under **Limits**, set **Pause at bag weight percent** and, optionally, an
   **Item goal**.
5. Click **Apply setup**.

The fields start at 20 for the radius, 35 for the HP percent, 85 for bag
weight, 10 for drill durability and 1 for the goal count. Check each value
before you apply. If a value is not valid, a message names the problem, for
example "Enter a positive radius and valid percentage and drill limits." or
"Setup is incomplete. Choose an area and eligible targets."

### Combat

- Under **Targets**, choose **Only these** and select monsters to attack, or
  **All except** and select monsters to skip. **Only these** needs at least
  one monster. The list shows monsters received nearby, or "No received
  monsters yet." The app skips monsters engaged by another player.
- Under **Recovery**, **Use HP potion below percent** sets the HP threshold.
  The app cannot use HP potions, so the run pauses at this HP percent.
- Keep at least one HP potion in the bag. Combat does not start without one,
  and the run pauses when HP potions run out.
- Choose **No item goal**. A combat goal cannot start; see
  [Limitations](#limitations).

### Digging

- Under **Drill**, **Pause below this durability** sets the drill limit, which
  must be above 0. Equip a drill with enough depth before you start.
- Digging also pauses at the HP percent in the **Recovery** field, which only
  the Combat form shows. To check it, switch **Activity** to **Combat**, read
  or change the value, then switch back to **Digging** before you apply.

### Item goal

Goal choices come from the bag data the app receives. Item names are not
known yet, so a choice reads "Label unknown · choice N · M in bag". Enter the
count in **Stop after this many confirmed items gained**. When confirmed gains
reach the count, the run pauses with "Goal reached".

## Start and control a run

The Run tab (⌘1) shows the activity, **Vitals**, **This run** and
**Before you start**. Start is available when nothing blocks it. Otherwise,
the line under the run controls names the first blocker, for example
"Inventory is unknown. Waiting for received bag data." Counts include only
results that the game confirmed.

| Action | Button | Shortcut | What happens |
| --- | --- | --- | --- |
| Start | **Start** | ⌘R | Starts a run with the applied setup. |
| Pause | **Pause** | ⌘P | Finishes the current action, then pauses. The status shows **Pausing**, then **Paused**. |
| Resume | **Resume** | ⌘R | Continues a paused run once nothing blocks it. |
| Stop | **Stop** | ⌘. | Finishes the current action, then ends the run. The game or session stays open. |
| End run | **End run** | ⌘. | Ends a run held while the app is reconnecting or disconnected. |
| Tabs | **Run**, **Setup**, **Log** | ⌘1, ⌘2, ⌘3 | Shows the tab. A hidden panel shows again. |
| Panel | **Hide panel**, **Show panel** | ⌘\\ | Hides or shows the side panel. |
| Keyboard | Keyboard badge | ⌃Tab | Moves keys between the game and the dashboard. |

The Run and View menus have the same commands.

While a run is running, Setup says "Pause the run to change setup. Changes
apply when you resume." While it is paused, edit the form, click
**Apply setup**, then resume. The activity stays locked until the run ends.

In client-assisted mode, Start gives the game keyboard focus. During a run, do
not type in the game, and keep the pointer outside the game pane. Keys,
clicks, scrolling or pointer movement over the game pause the run as manual
input. Shortcuts with ⌘ or ⌃ do not.

## Pause reasons

A pause shows the banner **Run paused** with one of the texts below. When
something still blocks Resume, the banner adds "Resume locked ·" and the
reason. Fix the cause, then press **Resume** (⌘R).

| Banner text | Cause | To resume |
| --- | --- | --- |
| "By you" | You pressed Pause. | Resume. |
| "Manual gameplay input. Check the character before resuming." | You used the game during a run. | Wait until the app confirms that the character is idle, then resume. |
| "HP reached the potion threshold, and this session cannot use HP potions. Recover HP in the game before resuming." | HP fell to the threshold. | Recover HP above the threshold in the game. |
| "Restock HP potions in the game before resuming." | The bag has no HP potions. | Restock HP potions in the game. |
| "HP potions ran out or could not be used. Check HP and potions in the game before resuming." | HP or HP potions caused the pause, and that blocker has since cleared. | Check HP and potions in the game, then resume. |
| "Free bag space in the game before resuming." | Bag weight reached the limit. | Free bag space below the limit. |
| "Repair or replace the drill in the game." | No drill, or durability is below the limit. | Equip, repair or replace the drill. |
| "Equip a deeper drill or choose another area." | The drill is too shallow. | Equip a deeper drill or change the area. |
| "Goal reached. Check the character before resuming." | Confirmed gains reached the goal. | Change or remove the goal and apply, or stop. |
| "Revive the character in the game." | The character fainted. | Revive in the game. The app never revives. |
| "Resume requires fresh state and verified idle gameplay." | No state update for 15 seconds, or an action had an uncertain result. | Resume when the state is live again. |
| "Check the character and live state before resuming." | The session reconnected or closed during the run. | Check the character, then resume. |

If the app cannot confirm within 2 seconds of a pause or stop that the
character is idle, it closes the game or session. A paused run stays paused.

In clientless mode, no game client is open to fix these causes. Stop the run,
switch to client-assisted mode or quit, fix the cause in the game, then start
a new session.

## Halts

A halt ends the run and closes the game or session. The status shows the halt.

| Status | Meaning | What to do |
| --- | --- | --- |
| **Game updated** | The official game no longer matches the build this app supports. | Click **Dismiss**. Reloading or reconnecting fails the same way until an OpenTrickster update supports the new build. |
| **Game client stopped** | The game view ended or failed. The game pane shows the cause. | Click **Reload game**. |
| **Actions rejected** | The game rejected three actions in a row. | Check the character and Setup, then click **Dismiss**. |
| **Sign-in failed** | The game or session could not open, for example after a rejected sign-in or a network error. | Fix the cause, then click **Sign in**, or **Reload game** in client-assisted mode. |

## Connection loss

1. When the connection drops, the status shows **Disconnected** and the banner
   says "Reconnect when the connection is available. Check the character
   before resuming." A run stays paused. The app does not reconnect on its own.
2. Click **Reconnect** in the banner. The status shows **Reconnecting**. In
   client-assisted mode, the game loads again, and you log in inside it. In
   clientless mode, the app reuses this session's sign-in.
3. When the session returns, the run is paused with "Check the character and
   live state before resuming." Check the character, then resume. The app
   never resends the action that was in progress.

The app allows up to five reconnect attempts in a row. If **Reconnect** is not
offered, end the run with **End run** or **Stop** (⌘.), then sign in again.

## Switch modes

1. Stop the run. During a run, the app says "Mode switch is off during a run.
   Stop the run first."
2. Show the panel, then click **Client-assisted** or **Clientless** in the
   toolbar.
3. If a game or session is open, a sheet asks "Switch to clientless mode?" or
   "Switch to client-assisted mode?". Click **Switch** (Return) or **Cancel**
   (Escape).
4. The app closes the game or session and waits until its connections close.
   The new mode opens with nothing loaded. Sign in again.

If closing takes more than 5 seconds, the app says "Waiting for the game
connections to close. No new session can open until they do."

Right after the game view stops, the game can still see the character as in
use. If clientless sign-in then says "This character is in use. Choose another
character or wait.", wait, then sign in again.

## Quit

Quit with ⌘Q, the window's close button or **Quit** in the Dock. The app
closes the game or session first, then quits. A run in progress ends. If
cleanup takes more than 5 seconds, the app says "Waiting for the game
connections to close. The app stays open until cleanup is confirmed." Leave it
open until it quits. The app keeps no mode, setup, run or sign-in between
launches.

## Alerts

Setup lists four alerts, all on by default: **Pause with a reason**,
**Connection lost**, **Goal reached** and **Dock badge**. Notifications work
only from the packaged app, after you allow them. Otherwise alerts stay in the
panel.

## Limitations

- **Pinned game build.** The app supports only build `8502ebbf2fc9033d2fba50d02fd92002`.
  After a redeploy, it refuses to load the game and shows **Game updated**
  until its pins are migrated. The official game now runs a newer build ([#37]).
- **State readiness.** Start stays blocked until the app has received carried
  weight, inventory and, for combat, the HP potion count. Carried weight is not
  established yet ([#21], [#31]), so expect "Waiting for received character
  resources and world state."
- **No pickup.** Drops stay on the ground. A combat goal cannot start, because
  only pickups count toward it: "A required game action is not available for
  this session."
- **No HP potion use.** The run pauses at the HP threshold instead.
- **No gun combat.** With a gun equipped, combat cannot start.
- **No live proof yet.** Bounded live combat and digging in each mode are
  still open ([#23]).
- The app never buys items, revives the character, accepts or turns in
  quests, or takes over a character that is in use.

[#21]: https://github.com/oDestroyeRo/opentrickster/issues/21
[#23]: https://github.com/oDestroyeRo/opentrickster/issues/23
[#31]: https://github.com/oDestroyeRo/opentrickster/issues/31
[#37]: https://github.com/oDestroyeRo/opentrickster/issues/37
