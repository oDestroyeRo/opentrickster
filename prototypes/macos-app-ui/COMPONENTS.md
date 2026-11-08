# Component specifications

This file specifies the components of the [macOS app UI prototype](README.md) for a Rust GPUI build. Token names refer to [TOKENS.md](TOKENS.md). Sizes are logical pixels. On 2026-11-08 the owner chose layout A with its panel-hidden view. [`DESIGN.md` on `main`](https://github.com/oDestroyeRo/opentrickster/blob/main/DESIGN.md) carries the confirmed specifications for that layout. This file keeps the specifications for all three prototype layouts.

## Rules for every component

1. Nothing that GPUI draws overlaps the game rectangle. This rule covers tooltips, banners, messages, popovers, and focus rings. The [#4 report](https://github.com/oDestroyeRo/opentrickster/blob/89d5993882982e959fc078f6690c8562abd7cb44/docs/research/macos-gpui-embedding.md) says the native webview sits outside GPUI clipping and paint order.
2. A control above the game rectangle needs a visible label, because a GPUI tooltip there would open over the game. Native macOS tooltips may draw over the webview, but #11 has not verified that.
3. Every status has an icon and a text label as well as a color.
4. Text uses the `text.*` tokens. Status colors appear only on icons, tinted fills, meter fills, and strips.
5. Numbers that update in place use tabular figures.
6. Every line height is at least 1.5 times its font size. Thai stacks vowels and tone marks above and below the baseline. Measured in Chrome with IBM Plex Sans Thai, the cluster ปู่ needs 1.46 times its size to fit inside a line box that centers it. `build-tokens.mjs` fails on a tighter type token, and `shoot.mjs` fails on tighter rendered text.
7. When Reduce Motion is on, spinners, the reconnect pattern, and the loading sweep stop. The pulse beat becomes a fade across the whole strip, and the sheet appears without sliding.

## GPUI APIs at the pinned revision

The notes below name these GPUI APIs. Each one exists at Zed revision `b5cd310b3b54dfc659fcc06453e6358843dfe216`, which the #4 report pins. Nobody has built this app against them yet.

| API | Source file | Use |
| --- | --- | --- |
| `rgb`, `rgba` | [`color.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/color.rs) | Color tokens, from the GPUI column in TOKENS.md |
| `Styled::text_size`, `Styled::line_height`, `Styled::font_features` | [`styled.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/styled.rs) | The type scale and tabular figures |
| `FontFeatures` | [`text_system/font_features.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/text_system/font_features.rs) | OpenType tag and value pairs, such as `("tnum", 1)` |
| `TextSystem::add_fonts` | [`text_system.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/text_system.rs) | Bundled IBM Plex Sans, IBM Plex Sans Thai, and IBM Plex Mono |
| `actions!` | [`action.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/action.rs) | The `Start`, `Resume`, `Pause`, `Stop`, and `EndRun` actions |
| `KeyBinding::new` | [`keymap/binding.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/keymap/binding.rs) | Shortcuts in a dashboard key context |
| `App::bind_keys`, `App::set_menus` | [`app.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/app.rs) | Shortcut registration and the menu bar |
| `Menu`, `MenuItem::action` | [`platform/app_menu.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/platform/app_menu.rs) | The Run menu |
| `FocusHandle` | [`window.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/window.rs) | Dashboard keyboard focus |
| `track_focus`, `key_context`, `on_action` | [`elements/div.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/elements/div.rs) | Focus tracking and action handlers |
| `uniform_list` | [`elements/uniform_list.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/elements/uniform_list.rs) | The log |
| `canvas` | [`elements/canvas.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/elements/canvas.rs) | Layout bounds for the game rectangle |
| `Context::observe_window_bounds` | [`app/context.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/app/context.rs) | Window bounds changes |
| AccessKit integration | [`_accessibility.rs`](https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/_accessibility.rs) | Roles and announcements for screen readers |

## Window and toolbar

The window has a unified toolbar 52 px tall (`size.toolbar`) with the traffic lights at the left. A 3 px pulse strip (`size.pulse`) runs under the toolbar at full width. The window background is `surface.base`, and the toolbar and panels use `surface.panel`.

| Layout | Toolbar content, left to right |
| --- | --- |
| A | App name, mode switch, keyboard badge, panel toggle |
| A, panel hidden | Status pill, HP, MP, and weight mini meters, run controls, keyboard badge, panel toggle |
| B | App name, mode switch, status pill, run controls, keyboard badge |
| C | Status pill, activity chip during a run, HP, MP, and weight mini meters, run time during a run, mode switch, icon-only run controls, keyboard badge, drawer button |

A mini meter is 92 px wide. It shows a label and a percentage over a 4 px track and takes the same tones as the full meter. The mini meters appear only once a character is in the world. They hide below a 1232 px window, the width that A's panel-hidden toolbar needs in its widest state. If the toolbar still runs out of room, the status pill's detail ends in an ellipsis.

In layout A, in both views, and in layout B, every toolbar control has a visible label. Layout C's icon-only run controls and drawer button depend on tooltips, so layout C breaks rule 2 while the drawer is closed.

## Menu bar

The prototype has no menu bar. The proposal adds a Run menu with **Start** or **Resume** (⌘R), **Pause** (⌘P), and **Stop** or **End run** (⌘.), plus a View menu for the tabs (⌘1 to ⌘3) and **Hide Panel** or **Show Panel** (⌘\\), which toggles the A panel or the C drawer. Menu key equivalents are the candidate way to keep these shortcuts working while the webview has keyboard focus. Issue #11 must verify that. In GPUI, `App::set_menus` installs the menus, and `MenuItem::action` ties each item to the same action that the buttons dispatch.

## Mode switch

The mode switch is a two-segment control with **Client-assisted** and **Clientless**, each with an icon. The selected segment uses `surface.selected` and `text.primary` with a 1 px `line.control` outline.

While a run exists, a click does not switch. The layout's message area says "Mode switch is off during a run. Stop the run first." While a run is held during a disconnect, the message ends with "End the run first." With no run, a click opens the system sheet. In the `client-absent` and `offline` phases, a click switches at once, because no game or session is open.

## Status pill

The status pill shows an icon, a phase label, and an optional detail in `text.secondary`, such as "Paused, Out of HP potions". Its background is the tone's subtle color, and the icon takes the tone color. The README lists the label for each phase.

| Size | Height | Type | Where |
| --- | --- | --- | --- |
| Default | 28 px | `body`, semibold label | B and C toolbars, and the A toolbar while the panel is hidden |
| Large | 34 px | `title`, semibold label | Top of the A rail |

The pill has the status role and announces each change politely. In GPUI, map it to an AccessKit status announcement. The exact AccessKit mapping is unverified.

## Run controls

The run controls are a primary button and a stop button. While the run is running or pausing, **Pause** takes the primary button's place.

| Button | Label | Shortcut | Enabled when |
| --- | --- | --- | --- |
| Primary | Start, or Resume when paused | ⌘R | The session accepts `Start` or `Resume`, and no preflight check blocks it |
| Pause | Pause | ⌘P | The session accepts `Pause` |
| Stop | Stop, or End run while reconnecting or disconnected | ⌘. | The session accepts `Stop`, or accepts `EndRun` while a run is held |

The enabled state comes from the session transition table, the same table that applies the event. A button never looks active while the session would ignore it. Each button shows its shortcut after the label in `caption` type. In layout C, the buttons are icon only. A disabled primary button uses `surface.raised` with `text.muted`, so it does not read as active. The stop button's icon uses `status.critical`.

## Blocked reason and refused actions

In layout A, when Start or Resume is unavailable and no alert explains why, a line under the run controls says so, for example "Resume is unavailable. HP potions: None in the bag." It has a warning icon in `status.warning` and text in `text.secondary`. While A's panel is hidden, and in layout C, the ticker shows the same line.

A refused action shows a short message for 2.2 seconds, for example "Resume is locked until the bag has HP potions again." or "Can't pause while the status is Ready." The message has an info icon and the status role. It appears in the layout's own status area, never in a floating toast. That area is under the run controls in A, the ticker while A's panel is hidden, the right end of the dock tab row in B, and the ticker in C.

## Pulse strip

The pulse strip is a 3 px line under the toolbar that shows the session's state at a glance.

| Pattern | Phases | Look |
| --- | --- | --- |
| Live | `starting`, `running` | A tone line. One beat crosses it for each result the game confirms, over `motion.pulse`. |
| Hatch | `pausing`, `paused` | Diagonal stripes in the tone color |
| March | `reconnecting` | Dashes in the tone color that move to the right |
| Solid | `disconnected`, `error` | A solid line in the tone color |
| Sweep | `client-loading`, `connecting` | A segment in the tone color that sweeps across |
| Flat | All other phases | `line.subtle` |

## Alert banner

An alert banner has an icon, a title, a body, an optional unlock line, and optional actions. Its background is the tone's subtle color with a 3 px left strip in the tone color. Its radius is `radius.panel`. The body uses `small` type in `text.secondary`.

- Layout A shows the banner in the rail under the run controls, and in the ticker while the panel is hidden.
- Layout B shows it at the top of the dock overview.
- Layout C shows the same content in the ticker.

Pause reasons, reconnecting, disconnected, and halts create banners in that place. A failed sign-in shows its banner inside the sign-in form instead, next to the fields it concerns. A `client-failed` halt in client-assisted mode has no banner, because the game pane shows the failure and its **Reload game** button.

A banner never repeats a run control. When a pause has a blocking check, the unlock line has a lock icon and says what unlocks Resume, for example "Resume unlocks when the bag has HP potions again." For a halt, the first action is the primary button. Buttons with a dashed outline exist only in the prototype. They stand in for actions the user takes in the game.

The banner has the alert role. In GPUI, map it to an AccessKit alert. That mapping is unverified.

## Ticker

The ticker is a bottom bar, 34 px tall, in layout C and in layout A while the panel is hidden.

- With no alert, it shows the latest confirmed result and its time on the left and the status line on the right. During an error, it shows the status label instead of the latest result.
- When Start or Resume is unavailable and no alert explains why, it shows the blocked reason with a warning icon.
- With an alert, it shows the alert icon, title, and body, truncated with an ellipsis, and then the alert's actions. In layout C only, the actions start with **Resume** while paused, or with **End run** while a run is held during a disconnect. These labeled buttons exist because C's toolbar controls are icon only. Layout A's toolbar keeps labeled run controls, so A's ticker repeats none of them.
- A refused action's message replaces the whole ticker for 2.2 seconds.

## Status line

The status line shows the game build and the age of the game state. Each layout shows it exactly once. It sits in the rail footer in A, on the right side of the ticker while A's panel is hidden and in C, and at the right end of the dock tab row in B. In the ticker, it hides while the ticker shows an alert.

The build label reads "Build f5e972fa" in `text.muted`, with the build in IBM Plex Mono. During a `build-changed` halt, it reads "Build 7c21e0b4 not supported" in `text.primary` with an octagon icon in `status.critical`.

| Freshness chip | Icon | Tone | When |
| --- | --- | --- | --- |
| No character yet | Person | Neutral | No character is in the world yet |
| Live, 0.2 s old | Dot | Good | State updates arrive |
| State N s old | Clock | Warning | State updates are late |
| No live state | Unplugged cable | Serious | Reconnecting, disconnected, or after a halt that stops state reads |

## Meter

A meter has a header row and a track. The header row holds the label in semibold `text.primary`, an optional state, and the value as "current / max" in tabular figures, with "/ max" in `text.muted`. A state has an octagon icon when critical and a warning icon otherwise. The track is 6 px tall with `radius.meter`.

| Tone | Fill | Track |
| --- | --- | --- |
| Neutral | `meter.fill` | `meter.track` |
| Warning, critical, or accent | The tone color | The tone's subtle color |

Threshold ticks are 2 px marks in `meter.tick`, each with a title such as "HP potion at 45%". The track has the meter role with its current, minimum, and maximum values.

## Vitals

Vitals stack these meters, followed by the HP potion and MP potion counts.

| Meter | Warning | Critical | Tick |
| --- | --- | --- | --- |
| HP | At or below the HP potion line, state "Below potion line" | At or below 20%, state "Low", or "Fainted" at 0 | The HP potion line |
| MP | At or below the MP potion line, no state | None | The MP potion line |
| Weight | Within 10 percentage points of the weight limit | At or above the weight limit, state "At limit" | The weight limit |
| The drill, digging only | At or below twice the durability limit | At or below the durability limit, state "At limit" | The durability limit |

The weight limit is a percentage of the maximum weight. The drill meter's label is the drill's name.

The A rail, the B dock, and the C drawer use a compact form that leaves out Galder. The full form adds Galder and appears on the clientless board while A's panel is hidden or C's drawer is closed.

## Run totals

Run totals are four tiles, each with a label in `small` type and `text.secondary` and a value in semibold `heading` type. A secondary part of a value, such as "3 empty" or "of 5", uses `small` type in `text.muted`.

| Tile | Combat | Digging |
| --- | --- | --- |
| 1 | Time | Time |
| 2 | Kills | Finds, with the empty count |
| 3 | EXP per hour | TM per hour |
| 4 | "Goal" and the goal item, with "N of M", or Galder with no goal | "Goal" and the goal item, with "N of M", or Galder with no goal |

A caption under the tiles says "Counts include only results the game confirmed. The game confirmed X of Y actions sent." The section title is "This run" during a run and "Last run" after it. Before any run, the B dock column says "No run yet."

## Activity card

The combat card shows "Fighting" while the run is starting, running, or pausing, and "Last target" otherwise. Then it shows the target name with the skill and its slot, or "moving into range", a target HP meter in percent, the number of monsters in range, and the map.

The digging card shows "Digging" or "Last dig", the current step with the spot coordinates, a five-step stepper, a depth meter in the accent tone as "N of M", and the map. The steps are Pick spot, Move, Detect, Dig, and Collect. The stepper marks done steps in `meter.fill` and the current step in `accent.default` with a semibold label.

Both cards drop to 55% opacity when the run is not live. On the clientless board, the target or step name uses `display` type, and the digging card leaves out the map, because the character header shows it.

## Preflight checklist

The preflight checklist appears before Start. Each row has an icon, a bold label, and a detail in `text.secondary`.

| Level | Icon | Meaning |
| --- | --- | --- |
| OK | Check in `status.good` | The check passes. |
| Block | Cross in `status.critical` | Start stays locked. The row carries the phrase that says what unlocks it. |
| Unknown | Info in `text.muted` | The app cannot check this yet. |

| Activity | Rows |
| --- | --- |
| Both | Character in the world, Supported game build, Map allows automation (Unknown) |
| Combat | Targets, HP potions, and Character when the character has fainted |
| Digging | Drill equipped, Drill durability |
| Both, last | Bag weight |

## Last-minute lanes

The lanes show the last 60 seconds in two rows.

- The Sent row has a 9 px hollow circle, outlined in `text.muted`, for each action sent.
- The Confirmed row has a 9 px filled circle in `accent.default` for each confirmed result and an 11 px cross in `status.critical` for each rejected action.

The axis reads "1 min ago", "45 s", "30 s", "15 s", and "Now". Hovering a mark shows its time and text in a tooltip above the Confirmed row. The lanes appear under the game in layout A while the panel shows, and on the clientless board. Under the game, the tooltip stays outside the game rectangle. On the board, the caption reads "Each mark is one action or one result". While the game state is not live, the lanes keep the last known marks, and the caption reads "Last known, no live game state".

## Log

Each log row is 24 px tall (`size.log.row`) and holds the time in tabular figures, an icon, and one line of `small` text that ends in an ellipsis when it is too long. Rows are newest first.

| Kind | Icon | Text |
| --- | --- | --- |
| Sent | Arrow in `text.muted` | `text.secondary` |
| Confirmed | Check in `status.good` | `text.primary` |
| Warning | Warning in `status.warning` | `text.primary` |
| Error | Octagon in `status.critical` | `text.primary` |
| Info | Dot in `accent.default` | `text.primary` |

A segmented filter above the log offers **All**, **Confirmed**, and **Problems**. Confirmed keeps confirmed results, and Problems keeps warnings and errors. A hint beside the filter says "Arrows mark actions sent. Checks mark results the game confirmed." An empty log says "Nothing here yet." The log has the log role. In GPUI, `uniform_list` fits the log because every row has the same height.

## Tabs

The tab row has three tabs. Each tab has an icon, a label, and its shortcut in `caption` type and `text.muted`. Tabs are 36 px tall. An unselected tab uses `text.secondary`. The selected tab uses `text.primary` with a 2 px underline in `accent.default`.

| Tab | Layout A | Layout B | Layout C |
| --- | --- | --- | --- |
| First (⌘1) | Run | Overview | Details |
| Second (⌘2) | Setup | Setup | Setup |
| Third (⌘3) | Log | Log | Log |

In layout A, ⌘1 to ⌘3 also show a hidden panel.

## Setup form

The setup form starts with the activity choice, a segmented control for **Combat** and **Digging** that stays locked until the run ends.

| Activity | Section | Fields |
| --- | --- | --- |
| Combat | Where | Map, movement boundary |
| Combat | Targets | Only these or all except these, a target list, skip monsters another player is fighting |
| Combat | Attacks and recovery | Skill order, HP potion threshold, MP potion threshold |
| Combat | Loot and limits | Pick-up rule, goal, bag weight limit, kills without a drop |
| Digging | Where | Map, spot selection, search radius |
| Digging | Drill | The equipped drill, durability limit, what to do when attacked |
| Digging | Limits | Goal, bag weight limit |
| Both | Alerts | Pause with a reason, lost connection, goal reached, Dock badge |

The Attacks and recovery section says "The run pauses when HP potions run out. This is always on." While a run is starting or running, a lock bar says "Pause the run to change setup. Changes apply when you resume.", and the fields are disabled. Layouts A and C show the form in one column. Layout B gives each section its own column across the dock, five for combat and four for digging.

## Keyboard badge

The keyboard badge appears in client-assisted mode only. It says "Keys go to Game" or "Keys go to Dashboard" and shows ⌃Tab in `text.muted`. A click switches the keyboard owner. It is 28 px tall (`size.control`) with a `line.subtle` border. When the game owns the keyboard, the badge uses an `accent.default` border and an `accent.subtle` fill.

## Panel toggle

The panel toggle is the last button in layout A's toolbar. It has the sidebar icon and reads **Hide panel** while the panel shows and **Show panel** while it is hidden, followed by ⌘\\ in `caption` type. It reports whether the panel is expanded. Hiding the panel also hides the strip under the game, so the game fills the window below the toolbar and above the ticker.

## Game pane

The game pane is a 16:9 rectangle inside a stage with 3 px of padding. The stage background is `surface.letterbox` in both themes. GPUI draws nothing inside the rectangle while the webview is present. While the game owns the keyboard, GPUI draws a 2 px focus ring in `accent.default` (`size.focus.ring`) in the stage padding, outside the rectangle. The proposed minimum is 960 × 540 (`size.game.min.width` and `size.game.min.height`).

| Phase | The rectangle shows |
| --- | --- |
| `client-absent` | A GPUI panel, "The game client is not loaded", with **Load game**. No webview exists yet. |
| `client-loading`, `awaiting-login` | The game's own loading and login screens |
| `ready` and later | The game |
| `error` with `client-failed` | A GPUI panel with the failure kind, "The game client stopped", the cause, and **Reload game**. The webview is gone. |

In GPUI, the #4 report derives the rectangle from element layout bounds, through a `canvas` callback or a custom element, and from `Context::observe_window_bounds`.

## Clientless board

In clientless mode, the board takes the game pane's place.

1. The character header shows the name's first letter in a circle, the name in `display` type, the map, the coordinates, the Galder, and the Level and TM meters in the accent tone. When the board is narrower than 900 px, the two meters move to a second row.
2. The activity card follows in its large form. Before a run, a Ready card with the activity, the map, and the preflight checklist takes its place.
3. A Session block shows the connection and the account, in layouts A and C.
4. The last-minute lanes span the full width.
5. Vitals and run totals appear while A's panel is hidden or C's drawer is closed.
6. The log, with its filter, fills the remaining height.

## Connect panel

The connect panel replaces the board before a character is in the world.

- When offline, it shows "Connect without the game client", a short explanation, **Game ID** and **Password** fields, a "Store the password in macOS Keychain" checkbox, and **Sign in**. When the service rejects the ID or password, a "Sign-in failed" alert appears above the fields.
- While connecting, it shows four steps: Signing in, Choose a world, Choose a character, and Entering the world. The world list appears at the second step and the character list at the third. The first and fourth steps show a spinner. **Cancel** returns to the offline form.

## System sheet

The mode switch confirms with a native AppKit sheet, not a GPUI overlay. The sheet has an icon, "Switch to clientless mode?" or "Switch to client-assisted mode?", a body that says what happens to the game and the character, **Cancel**, and **Switch** as the default button. AppKit draws the sheet in its own window, so it may appear over the game rectangle. Issue #11 must verify that it does.
