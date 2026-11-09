# MVP acceptance checklist

This checklist maps each completion requirement in
[MVP behavior](mvp.md#completion-evidence) and [MVP delivery #19][#19] to a
concrete check. Run every check against one recorded revision and one packaged
app. The [operator guide](operator-guide.md) describes the app steps.

Statuses were last updated on 2026-11-09.

## Status values

- **Passed** names its evidence: a CI run, a PR or a report.
- **Not run** means that no one has run the check at a recorded revision.
- **Blocked by #N** names the issue that must close first.

Live gameplay is blocked for two reasons:

- [#37]: the official game moved to build `ea1a68bd89958fae94179f1e0835ffb5`.
  Every pin is for `8502ebbf2fc9033d2fba50d02fd92002`, so both adapters refuse
  to load.
- [#31] Trials A and B, and [#21]: carried weight and complete inventory are
  not established yet, so Start stays unavailable.

[#23] owns the live combat and digging acceptance.

## Requirement map

| Requirement | Source | Checks |
| --- | --- | --- |
| Appropriate automated tests | mvp.md, #19 | A1 to A4 |
| Runnable packaged macOS app | mvp.md, #19 | A5, A6, L1 |
| Reviewed and merged production PRs | mvp.md, #19 | L18 |
| 1. The real target boots in the embedded pane, with placement, resizing, focus, controls and child-before-parent cleanup | mvp.md, #19 | A6 to A8, L2, L3, L16 |
| 2. Native sign-in selects an existing character and receives its initial state without the game client | mvp.md, #19 | L4 |
| 3. Each adapter performs a bounded attack and a completed dig, with server results distinguished from local sends | mvp.md, #19 | L5 to L9 |
| 4. Start, Pause, Resume, Stop, setup locking, stale state, rejection limit, reconnect pause and mode and session ownership | mvp.md, #19 | L10 to L15 |
| 5. Credentials stay in memory for the current session only, outside Git, logs, verification artifacts and disk | mvp.md, #19 | L17 |
| The pinned build refuses a changed game | [Net boundary](../crates/net/README.md) | A9 |

## Record evidence

For each check, record:

- the date in UTC, the commit SHA, and `revision` and `dirty` from
  `OpenTrickster.app/Contents/Resources/build.json`;
- the macOS version and the Mac's architecture;
- the exact command or manual steps, and the exit status;
- for a verification command, `passed`, `checks`, `failures` and
  `account_used` from its `report.json`;
- for a live check, the status labels, banner text, run counts and log rows
  that you saw.

Never record credentials, the Game ID, character UIDs or other raw
identifiers, tickets, packet bodies, page messages or private game captures.
Replace character, world and account names with placeholders. Keep
screenshots and `game.tiff` snapshots out of Git and issue comments unless
they show only the offline fixture or the title screen.

Use a fresh `CARGO_TARGET_DIR` for each acceptance build and a new, empty
directory for each verification run.

## Automated checks

Run these from the repository root. They use no account. `APP` stands for
`dist/OpenTrickster.app/Contents/MacOS/opentrickster`.

| ID | Check | Expected result | Evidence | Status |
| --- | --- | --- | --- | --- |
| A1 | `cargo fmt --all --check` | Exit 0 | CI run or command output | Passed: CI run [42581644750] on `main` at `3a34004`, and CI run [42582226234] on this branch at `6c53c3a` |
| A2 | `cargo test --workspace --locked` | Every test passes | Test counts per crate | Passed: the same runs. [PR #39] recorded 27 desktop, 75 engine and 47 Net tests |
| A3 | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Exit 0 with no warnings | CI run or command output | Passed: the same runs |
| A4 | `node crates/net/tests/sampler.cjs` | Every sampler heap case passes | Case count | Passed: the same runs. [PR #39] recorded 87 cases |
| A5 | `python3 scripts/package_macos.py --output dist/<new-name>.app` | Exit 0. The app has a verified ad-hoc signature and a `build.json` | `build.json` values | Passed: [PR #40]. CI run [42582226234] uploaded `OpenTrickster-macOS-arm64.zip` |
| A6 | `APP --verify-embedding --verification-dir <new-dir>` | Exit 0. `report.json` has `"passed": true`, `"verification": "offline-fixture"` and `"account_used": false` | `report.json` fields | Passed: packaged app, [PR #40] |
| A7 | `APP --verify-input --verification-dir <new-dir>` | Exit 0. `report.json` has `"passed": true` | `report.json` fields | Passed on the desktop branch, [PR #39]. Not run on the packaged app |
| A8 | `APP --verify-application --verification-dir <new-dir>`, then again with `--quit-via native` and with `--quit-via command` | Exit 0. `"verification": "official-title-only"`, `"embedded_resource_close_fence_passed": true` and `"account_used": false` | `report.json` fields per route | Blocked by [#37]: [PR #40] recorded exit 1 because the app refuses the new build |
| A9 | Run A8 while the official game is on a build other than the pinned one | Nonzero exit. The app does not load the changed game | Exit status and `failures` | Passed: [PR #40] recorded exit 1 while the official build is `ea1a68bd` |

A6 uses an offline fixture. It checks child parenting, WebGL2, page and host
messages, window resizing, minimum-size layout, focus transfer, a menu
shortcut while the game view has focus, the mode sheet, a snapshot and
child-before-parent cleanup. A8 loads the official title page and downloads
the public game files without signing in. It is the only automated check that
contacts the game. To prove a real socket close at quit, also run
`verify_quit.py` as described in the
[desktop verification notes](../crates/desktop/verification/README.md).

After this branch is rebased onto the desktop branch that removes Keychain
storage (`aa06928`), rerun A1 to A7 at the new head. CI run [42609769864] for
desktop head `e34ab28` was still in progress on 2026-11-09.

## Live checks

Live checks use the already authorized account, one worker at a time, with
bounded normal gameplay. Do not buy items, revive, accept or turn in quests,
or take over a character that is in use. Run each mode separately.

| ID | Steps | Expected result | Evidence | Status |
| --- | --- | --- | --- | --- |
| L1 | Open the packaged app. For a downloaded copy, allow it once in **System Settings > Privacy & Security**. | One OpenTrickster window opens in client-assisted mode with the status **Game not loaded**. Nothing downloads until you click **Load game**. | `build.json` values, whether macOS asked to allow the app | Not run. A6 ran the packaged executable directly |
| L2 | Click **Load game**. At **Log in to the game**, resize the window from large to its minimum. Press ⌘\\ twice, ⌃Tab twice, then ⌘2 and ⌘1 while the game has focus. | The game fills a 16:9 area in the left pane and follows every resize. No app element covers it. The badge alternates between **Keys go to Game** and **Keys go to Dashboard**. The shortcuts switch tabs while the game has focus. | Window sizes, observations, a title-screen screenshot | Blocked by [#37] |
| L3 | Log in inside the game and choose the character. | The status becomes **Ready**. Vitals show HP and MP. The footer reads "Received game state". Setup lists received monsters. | Status labels, which values show "Unknown", the **Before you start** rows | Blocked by [#37] |
| L4 | Switch to **Clientless**. Enter the Game ID and password, click **Sign in**, then choose a world and the character. | No game client runs. The board shows the character, map and coordinates, and the status becomes **Ready**. One sign-in, no retries. | Status labels, board fields with names replaced | Blocked by [#37]. Outside the app, the production Rust adapter completed one sign-in at build `8502ebbf` ([native startup report](research/native-rust-startup.md)) |
| L5 | In each mode, apply a valid setup, then open the Run tab. | **Before you start** shows "✓ Character in the world", "✓ Supported game build", "✓ Setup applied" and "✓ Inventory data received", with no blocker. Weight shows a current and maximum value. Start is enabled. | Every row and blocker text | Blocked by [#37], [#31], [#21] |
| L6 | Client-assisted: apply Combat with **Only these** and one monster, a small radius, **No item goal**, a weapon that is not a gun and at least one HP potion. Start. Stop after the first confirmed kill. | The log shows "Action sent", then "Result confirmed by the game". Kills increase only after confirmation. The character stays inside the boundary. Stop returns to **Ready**. | "N sent · M confirmed", kills, log rows, run time, any rejection | Blocked by [#37], [#31]. Acceptance in [#23] |
| L7 | Repeat L6 in clientless mode. | The same as L6. | The same as L6 | Blocked by [#37], [#31], [#21]. Acceptance in [#23] |
| L8 | Client-assisted: apply Digging with a drill limit and an equipped drill. Start. Stop after the first completed dig. | The activity steps through Pick spot, Move, Detect, Dig and Collect. "Result confirmed by the game" follows the send. Digs increase by one, with either a find or an empty dig. A received inventory change is recorded apart from the sent action. | Digs, finds, empty count, drill life before and after, log rows | Blocked by [#37], [#31]. Acceptance in [#23] |
| L9 | Repeat L8 in clientless mode. | The same as L8. | The same as L8 | Blocked by [#37], [#31], [#21]. Acceptance in [#23] |
| L10 | During a run, use ⌘P, ⌘R and ⌘. once each through the buttons, the shortcuts and the Run menu. Open Setup while the run is running and while it is paused. Change a value, apply it and resume. | Pause shows **Pausing**, then **Paused** with "By you". Resume continues. Stop finishes the current action, then returns to **Ready**. While running, Setup says "Pause the run to change setup. Changes apply when you resume." While paused, it is editable. The activity stays locked until the run ends. | Labels and the order of states | Blocked by [#37], [#31] |
| L11 | During a run, cut network traffic for 20 seconds, for example by turning off Wi-Fi, then restore it. | No action starts on old state. The run pauses with "Resume requires fresh state and verified idle gameplay." or goes to **Disconnected**. Resume stays locked until fresh state arrives. | Which path occurred, banner text | Blocked by [#37], [#31] |
| L12 | Do not provoke rejections. Record every rejected action during L6 to L9. | A confirmed action resets the streak. Three rejections in a row end the run with **Actions rejected**. | Log rows "The game rejected an action" | Blocked by [#37]. Engine tests in A2 cover the synthetic case |
| L13 | During a client-assisted run, click inside the game once. | The run pauses with "Manual gameplay input. Check the character before resuming." Resume unlocks after the app confirms that the character is idle. | Banner and blocker text | Blocked by [#37], [#31] |
| L14 | During a run in each mode, cut the network until the status is **Disconnected**. Restore it and click **Reconnect**. | **Disconnected**, then **Reconnecting**. The run returns paused with "Check the character and live state before resuming." Nothing is resent. In client-assisted mode, the game loads again and asks you to log in. | Labels, attempts used, log rows | Blocked by [#37], [#31] |
| L15 | During a run, click the other mode. Stop, click it again and choose **Cancel**. Click it again, choose **Switch** and sign in in the new mode. | During the run: "Mode switch is off during a run. Stop the run first." **Cancel** keeps the session. **Switch** closes the old game or session before the new mode shows nothing loaded. The new sign-in does not find the character in use. | Labels, close wait time, any "In use" result | Blocked by [#37] |
| L16 | Quit with ⌘Q while the game is loaded. Repeat with a clientless session open, and once with **Quit** in the Dock. Then run `pgrep -x opentrickster`. | The app quits only after cleanup. A wait over 5 seconds shows "Waiting for the game connections to close. The app stays open until cleanup is confirmed." `pgrep` prints nothing. | Quit route, wait time, `pgrep` output | Blocked by [#37]. A6 covers the offline cleanup |
| L17 | After L3, L4 and a quit, run the [credential check](#credential-check). Then relaunch the app and switch to **Clientless**. | No file, history or log contains the Game ID or password. No Keychain item exists. The **Password** field is empty, and the form says "The password is used for this session only." | Command outputs, which contain no secret | Blocked by [#37]. Keychain storage was removed in `aa06928` |
| L18 | Merge the reviewed desktop and packaging PRs into `main`, then rerun A1 to A9 on `main`. | Every automated check passes on `main`. A8 can pass only after [#37]. | Merge commits, CI runs | Blocked by [#39] and [#40], both open |

### Credential check

Read each secret without echoing it, so that it never appears in the
terminal, shell history or evidence. Run the block once for the Game ID and
once for the password.

```sh
read -rs OT_SECRET
grep -rlF -- "$OT_SECRET" . <verification-dirs> \
  ~/Library/Caches/org.opentrickster.desktop ~/Library/Preferences
git log --all -p | grep -cF -- "$OT_SECRET"
log show --last 2h --predicate 'process == "opentrickster"' | grep -cF -- "$OT_SECRET"
unset OT_SECRET
security find-generic-password -s org.opentrickster.desktop/game >/dev/null 2>&1; echo $?
```

Expect no file names from `grep -rlF`, `0` from both counts, and `44` from
`security`, which means that no Keychain item exists. If you launched the app
from a terminal, search its captured output too.

In client-assisted mode, you type the password into the game page, not the
app. The embedded view uses WebKit's default persistent website data, so
anything the game page saves is kept for this app. After L3, also search the
app's WebKit data, for example `~/Library/WebKit/org.opentrickster.desktop`
and `~/Library/HTTPStorages/org.opentrickster.desktop`, and record the result.

[#19]: https://github.com/oDestroyeRo/opentrickster/issues/19
[#21]: https://github.com/oDestroyeRo/opentrickster/issues/21
[#23]: https://github.com/oDestroyeRo/opentrickster/issues/23
[#31]: https://github.com/oDestroyeRo/opentrickster/issues/31
[#37]: https://github.com/oDestroyeRo/opentrickster/issues/37
[#39]: https://github.com/oDestroyeRo/opentrickster/pull/39
[#40]: https://github.com/oDestroyeRo/opentrickster/pull/40
[PR #39]: https://github.com/oDestroyeRo/opentrickster/pull/39
[PR #40]: https://github.com/oDestroyeRo/opentrickster/pull/40
[42581644750]: https://github.com/oDestroyeRo/opentrickster/actions/runs/42581644750
[42582226234]: https://github.com/oDestroyeRo/opentrickster/actions/runs/42582226234
[42609769864]: https://github.com/oDestroyeRo/opentrickster/actions/runs/42609769864
