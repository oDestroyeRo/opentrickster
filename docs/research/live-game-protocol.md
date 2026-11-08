# Live game session and protocol research

Research date: 2026-11-08. Starting repository commit: `6b8162f1f7a42b2bd9dd02c1b086be360914bb8d`. This report addresses [issue #12](https://github.com/oDestroyeRo/opentrickster/issues/12) under the [MVP planning map](https://github.com/oDestroyeRo/opentrickster/issues/2). It is a partial result. The action protocol and authoritative state remain unresolved, so the ticket should remain open.

An authorized account reached its existing character in the live game. A ground click visibly moved the character. The client entered digging mode, displayed automatic digging after a spot was selected, and returned to its normal view after Cancel. The session did not establish a completed dig result or an accepted monster attack. The browser observed text and binary WebSocket messages, but this investigation did not decode their framing, opcodes, fields, or acknowledgements. None of these observations proves clientless operation or embedding the game in a macOS application.

## Target and method

The target was [Trickster Rebuild](https://tr-play.playall.in.th/) in a fresh incognito preview browser tab. The title screen displayed product version `1.0`, revision `6ab10889`, and `2026-11-07 15:13`. The Unity console identified engine `6000.6.5f1` and revision `3ff58d469c8a`. The target's [version endpoint](https://tr-play.playall.in.th/version) reported deployment build `f5e972fa2be429602c20d6168013a20f`. These identifiers describe different layers and are not established as wire-protocol versions. The browser loaded the following build artifacts:

- [Loader `877598568ac7f0dc41389b0d6e411b8b.loader.js`](https://tr-play.playall.in.th/Build/877598568ac7f0dc41389b0d6e411b8b.loader.js).
- [Framework `4937d6d47bade23f56e5ad4b5bf2e2b9.framework.js.gz`](https://tr-play.playall.in.th/Build/4937d6d47bade23f56e5ad4b5bf2e2b9.framework.js.gz).
- [Data `9b2286992122d3c1cc081492dc31fee5.data.gz`](https://tr-play.playall.in.th/Build/9b2286992122d3c1cc081492dc31fee5.data.gz).
- [WASM `657890b2bd8548390dd7c7fad76093be.wasm.gz`](https://tr-play.playall.in.th/Build/657890b2bd8548390dd7c7fad76093be.wasm.gz), identified by the prior pinned artifact inspection, not independently hashed in this session.

The earlier [target-interface research](https://github.com/oDestroyeRo/opentrickster/blob/6d996ef87a75bd8857ee998ddf30dcd99b80c1bf/docs/research/target-game-interfaces.md) supplies artifact hashes and static transport leads. The [gameplay research](https://github.com/oDestroyeRo/opentrickster/blob/4dc02384a3eb2bcfcf9ae9a72d6797f6b29602f8/docs/research/trickster-gameplay-progression.md) distinguishes owner-guide claims from live observations. Neither report substitutes for the observations below.

The researcher verified the private credential delivery locally without printing the values. The login form was Unity-rendered and exposed no HTML input elements. A clipboard attempt did not advance the login. A harmless placeholder showed that synthetic keyboard events entered the canvas and that a click needed time to focus the password field. The private file was then read into page memory through a temporary local file input, and delayed keyboard events entered the two fields. The temporary file was outside the repository with restrictive permissions and was removed. The prior clipboard contents were restored after the clipboard attempt. No credentials, authentication payloads, session tokens, raw captures, screenshots, or game assets are included in this report.

The page's WebSocket constructor was instrumented before the successful login. It retained message objects only in page memory. The analysis returned aggregate message types, not their contents. The preview tab was closed at the end, discarding that memory. The captured traffic includes other actors and background messages, so temporal proximity alone cannot identify a game acknowledgement.

## Observations

| Step | Observed result | Limit |
| --- | --- | --- |
| Boot and login | Chromium rendered the title and login screens. A login submission advanced to server selection. The default server shown was Wonderland. | This demonstrates the visual client's login for this session only. It does not identify the authentication schema or prove an equivalent native login. |
| Character selection | The account had an existing selectable character. A first-use prompt offered an optional in-game security lock. At the user's explicit direction, the researcher selected the option not to use it and confirmed the notice. The selected existing character entered the world. | No character was created. The account and character identifiers are redacted. The prompt said the lock could be set later in game settings; its persistence and server-side representation were not tested. |
| Visible state | The world view showed character HP and MP, base and TM experience indicators, a map, other entities, and a weight indicator. The bag opened and displayed item categories, stacks, and a slot count. | These are client UI observations. No decoded snapshot or delta was matched to them, and item quantities before and after digging were not compared. Their authority and update semantics remain unknown. |
| Movement | A click on nearby ground changed the terrain's position relative to the centered character. | The UI showed movement; coordinates, arrival, server correction, and the corresponding request or acknowledgement were not decoded. |
| Dig start and progress | `D` displayed a prompt to select a digging point. Clicking nearby ground changed the prompt to an automatic-digging state. A progress display appeared and a displayed counter decreased during the activity. | The counter's wire meaning was not established. No item find, empty result, inventory delta, or completed dig was confirmed. |
| Dig cancellation | Clicking the visible Cancel button removed the automatic-digging banner and returned the normal view. | This is a UI transition, not a decoded server acknowledgement or proof of behavior after reconnection. |
| Monster attack | The researcher pressed `Tab` to select a nearby target. The client reported that no monster was available on screen. | No attack was performed or accepted. Other actors' visible combat is not evidence of this character's attack. |
| Session end | The active page held three remembered link entries in session storage immediately before the researcher closed the tab. Only the count was inspected. | The prior artifact inspection describes release attempts on page exit. This session did not observe or prove server acceptance of release, ownership transfer, logout, or reconnect. |

The observed digging prompt and cancellation are consistent with the owner guide's described `D`, spot selection, and stop workflow. The guide's expected empty or found outcomes and resource rules remain unverified for this attempt. The live character was in Desert Beach Field 3. The session did not accept, complete, or turn in a quest, and it did not test the guide's leveling route.

## Transport evidence and limits

The browser wrapper observed both text and binary WebSocket messages in each direction during login and world entry. In one in-memory sample through character selection, the observed text messages were not JSON. That classification does not reveal their format or purpose. Binary WebSocket messages are browser message boundaries; this observation does not establish that one message equals one game packet. The sample was not a complete protocol trace and no authentication bytes or token values were exported.

The earlier static inspection derives `wss://tr-play.playall.in.th/net` from the target's bridge and identifies `GatewayLink`, `OpcodeMap`, `PacketReader`, and `PacketWriter` as leads. It also describes a browser release path. Those are target artifact findings, not decoded runtime contracts. This session adds no verified opcode, field layout, byte order, packet length rule, action schema, or synthetic packet example. It cannot distinguish a client submission from server acceptance. No state-changing packet was replayed and no competing session was opened.

## Remaining work

1. On this target build, use a private, bounded capture with action markers and correlate decoded inbound events to visible position, HP, inventory, monsters, and digging state. Analyze the matching `GatewayLink`, `OpcodeMap`, `PacketReader`, and `PacketWriter` implementation. Keep authentication and tokens private. Record unknown fields rather than guessing.
2. With a suitable unclaimed monster visible, perform one basic attack and correlate the observed target and damage or rejection with the decoded exchange. Confirm ownership and target-loss behavior before considering automated combat.
3. Observe one dig through an actual empty or found result, confirm any relevant inventory and drill-resource changes, and independently correlate a cancellation with server state. Avoid assuming that a changing UI counter or a sent message is a completed result.
4. Verify normal release, reconnect, and session ownership without a competing login or replaying uncertain actions. Separately demonstrate the game inside the macOS app and a live clientless session with the game runtime absent.

The MVP remains one account and one character at a time. Client-assisted mode must show the game inside the app. Clientless mode must control the live character while only the dashboard is visible. Automatic quest acceptance, completion, and turn-in remain out of scope. No implementation decision or compatibility claim follows from this partial session.
