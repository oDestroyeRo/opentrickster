# Desktop verification

The executable is `opentrickster` in the `opentrickster-desktop` package.
All UI dependencies are restricted to macOS. The desktop library's geometry and
text-boundary tests also run without GPUI on other platforms.

Run the native account-free fixture:

```sh
cargo run -p opentrickster-desktop -- --verify-embedding
```

Run the official game to its title screen without signing in:

```sh
cargo run -p opentrickster-desktop -- --verify-embedding --target
```

Exercise the real engine, public profile preparation, passive page bridge and
correlated resource-close fence with no account:

```sh
cargo run -p opentrickster-desktop -- --verify-application
```

Add `--quit-via native` to exercise AppKit `terminate:` while the game child is
open, or `--quit-via command` for the real CmdQ menu key equivalent. The report
is written from the actual application-will-quit callback after the close fence.
The title may own zero sockets. To additionally prove a real socket handshake,
run the local fixture against the built binary:

```sh
python3 crates/desktop/verification/verify_quit.py \
  target/debug/opentrickster /tmp/opentrickster-quit-proof --route native
```

Repeat with `--route command` and a different empty directory. The fixture opens
one loopback WebSocket through the same document-start wrapper, receives its
ordinary close frame, delays its close reply by 400 ms, and checks actual TCP
closure. It sends no game or authentication messages. `loopback.json` separately
records that evidence. `--public-cache <prior-output>/public-cache` may reuse a
previous public profile download; no account data belongs in that directory.

On the verified macOS/WebKit environment, the HTTPS page's insecure loopback
socket constructs and emits an error, but never opens or emits a close event.
The server never accepts it, and the app correctly holds the quit fence. This
case does **not** prove socket cleanup. Add `--expect-held` to verify that negative
case: the native report must fail its cleanup deadline while retaining both
parent and child, and the fixture driver must observe the process still alive
before terminating its own account-free test process. This path does not weaken
WebKit's security settings. A real nonzero-socket successful close remains a
separate integration proof.

`--verification-dir <empty-directory>` selects the output directory. The runner
writes `report.json` and a native WKWebView snapshot as `game.tiff`. A failed
check returns a nonzero exit status. Existing verification files are preserved.
The fixture says "Offline embedding verification" inside the view. Neither path
opens an account session or proves gameplay.

The native checks exercise parenting, WebGL2, page/host IPC, window resize,
minimum-size layout, focus transfer, an actual menu key equivalent while the
webview has focus, native sheet presentation/cancellation, snapshot capture,
and child-before-parent cleanup. The menu event runs outside GPUI's update
borrow because AppKit synchronously calls GPUI's menu validator.

The normal app uses the engine's commands, effects, and immutable snapshot.
The explicit verification path is the only path allowed to construct an offline
snapshot. Run controls and setup guards come from the engine. An unsupported
action never becomes a sent or confirmed action.

The embedded page is restricted to the official HTTPS document and one document
per webview resource. App-level Reload creates a new resource. Every native
callback retains its resource generation. A private per-document guard fences
main-frame IPC. The page policy observes trusted manual input and keeps
fullscreen requests inside the application by expanding the game pane.

## Integration gates

Final integration checks require the merged engine and network crates. The
desktop consumes Net's generated, guarded document-start observer, including its
page-local GameServer selection and safe-frame filter. Raw IPC and scripts are
never logged. Title-screen verification proves no authenticated baseline,
combat, repeated digging, or native gateway acceptance.

Assisted actions require runtime validation of the read-only roots, UI routing,
live bindings/manual dig settings, camera and hit geometry, native input and
actual outbound matching specified in [the source report](https://github.com/oDestroyeRo/opentrickster/blob/6f93a75a23849475c7f88d5f3b099d98a4a6c33e/docs/research/assisted-input-control.md).
The pure projection tests prove only its algebra. No action capability is granted
from those tests. Quiescence also requires the combined local/controller/received
predicate; releasing a button cannot prove it. Until that runtime proof exists,
assisted action delivery remains unavailable and a drain deadline closes the
resource rather than inventing an idle barrier.

Goal choices use only received stacks in the engine snapshot. Unknown inventory
remains distinct from observed-empty inventory; labels stay explicitly unknown
until a received catalog resolves them.
Notifications request macOS permission only when a real alert is emitted from a
bundled app. An unavailable or denied notification keeps the alert in the panel.
Optional password storage uses macOS Keychain and is never required to connect.

## Implementation decisions

The engine owns lifecycle. Desktop resources use an exclusive adapter enum and
one main-thread inbox. Background profile preparation and native cleanup report
completion through a bounded queue. A mode switch waits for the native sheet's
response and actual cleanup before another adapter opens.

The single retained native credential capability contains only Net's opaque,
zeroizing normalized ID and derived key. The password input clears at handoff.
A fresh native session receives a capability clone only after the old resource's
actual Closed/join fence. Explicit disconnect, mode switch, quit and authorization
failure clear the retained capability. Keychain loading is an explicit action.

For embedded cleanup, Net's correlated ack must prove actual close events for
all page-owned sockets. Desktop then loads `about:blank`, waits for native
navigation completion, detaches the child and forwards Closed. A timeout or
failed close leaves the resource fence held. Wry 0.57.1 retains its detached
WKWebView; detaching alone is not Unity or socket cleanup.

GPUI 0.2.2's quit observers run before it clears its windows and are subsequently
polled for only 100 ms. The audited native module installs the earlier AppKit
`applicationShouldTerminate:` gate on the pinned GPUI delegate. It cancels the
initial native quit without reentering GPUI, lets the normal UI pump finish the
same close fence, then retries termination. An incompatible delegate fails
before any resource opens. CmdQ and window close also await this fence.

The Model the Domain principle kept adapter ownership in one sum type. The Prove
It Works principle required native event and snapshot checks beyond compilation.
