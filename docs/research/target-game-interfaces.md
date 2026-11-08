# Target game interfaces for the two-mode MVP

## Overview

The goal is an implementation-ready specification for a macOS Trickster bot that controls one live character for combat and digging. Client-assisted mode must show the game inside the application. Clientless mode must operate without the game client and show the information dashboard. This report answers the bounded public-interface question in [issue #3](https://github.com/oDestroyeRo/opentrickster/issues/3) under [the planning map](https://github.com/oDestroyeRo/opentrickster/issues/2). It does not implement either mode.

Public artifacts establish a Unity Web client, a custom WebSocket bridge, browser-side session cleanup, and concrete places to investigate gameplay serialization. They do **not** yet establish an authenticated clientless session or a callable gameplay API for the embedded client. Both remain candidates requiring target-version proof. The evidence is stronger than the landing page alone, but it is insufficient to choose a complete bot transport. [T1], [T3], [T4]

Research date is 2026-11-08. Direct target retrievals ran between 13:57 and 14:01 UTC. The starting repository SHA was `6b8162f1f7a42b2bd9dd02c1b086be360914bb8d`. The actual branch is `research/target-game-interfaces`. The ticket resolution records the final report commit and immutable blob link. A document cannot contain its own Git commit SHA without changing that SHA.

## Key concepts and evidence levels

- **Verified target artifact** means a public response or code artifact served by this target. Static code presence does not prove the server accepts the operation or that a feature is enabled.
- **Platform fact** means behavior documented by Unity or Apple. It does not prove this game works inside a particular embedded browser or GPUI application.
- **Inference** means a proposed explanation or integration choice derived from those sources.
- **Unknown** means the inspected evidence does not settle the question. It does not mean unsupported.

No original-game protocol or emulator is used as compatibility evidence. Strings that mention older clients are target-artifact observations only. Quest automation remains outside this MVP under the parent scope. Quest gameplay research belongs to the separate research assignment. [Planning map](https://github.com/oDestroyeRo/opentrickster/issues/2)

## Where the evidence lives

### Public artifact inventory

All target downloads used `User-Agent: Mozilla/5.0`, no account, and no authentication cookies. GET requests fetched static resources and the read-only version endpoint. HEAD requests checked the four build resources before the larger downloads. No WebSocket was opened. No login, log submission, session release, gameplay action, or encrypted asset decryption was attempted.

| Evidence | Public location and observed identity | Inspection |
| --- | --- | --- |
| T1 | [Target HTML], [T1], title `Trickster Online - Spinel Project`, company `Playall`, product `Spinel`, product version `1.0` | Full HTML and inline JavaScript, 27,217 received bytes. |
| T2 | [Unity loader], [T2], filename `877598568ac7f0dc41389b0d6e411b8b.loader.js` | Full loader, 27,914 received bytes. |
| T3 | [JavaScript framework], [T3], filename `4937d6d47bade23f56e5ad4b5bf2e2b9.framework.js.gz` | Full framework, 386,859 received bytes. Custom bridge functions and browser integration inspected. |
| T4 | [Unity data package], [T4], filename `9b2286992122d3c1cc081492dc31fee5.data.gz` | 27,239,767 received bytes. Inspected package directory, boot configuration, assembly list, initialization list, and IL2CPP metadata. Did not extract game pictures or other media. |
| T5 | [WebAssembly module], [T5], filename `657890b2bd8548390dd7c7fad76093be.wasm.gz` | 42,317,618 received bytes. Inspected header, bridge import names, and version strings. No execution or full gameplay disassembly. |
| T6 | [Deployment version endpoint], [T6] | JSON reported build `f5e972fa2be429602c20d6168013a20f`. The [matching-build request], [T7] returned the same build and `outdated: false`. |
| T8 | [Owner website], [T8], title `Trickster Rebuild` | Public HTML advertises web play, account registration, and email verification. Its referenced `/static/index-KlvPLZcC.js` returned HTTP 404 during this run, so website form logic was not inspected. |

The `.gz` suffix does not describe the bytes received in these requests. The framework body was JavaScript text, the data body began with `UnityWebData1.0` plus a null byte, and the WASM body began with `00 61 73 6d 01 00 00 00`. Download caps were 512 KiB for HTML, 8 MiB for each small build resource, 32 MiB for data, and 48 MiB for WASM. All inspected files stayed under those caps. Scratch copies remain outside the repository under `/tmp/opentrickster-issue3`. [T2], [T3], [T4], [T5]

The data package's `data.unity3d` UnityFS header identifies Unity `6000.6.5f1`. WASM also contains `6000.6.5f1` and revision `3ff58d469c8a`. `boot.config` contains Unity build GUID `6fd3df5ebd114128a9ba3af6721d9861`. The IL2CPP metadata header reports format `108`. These identities, the deployment build, the filename hashes, and product version `1.0` are distinct values. None is established as the game wire-protocol version. [T1], [T4], [T5], [T6]

The loader's own data-unpacking logic established the package layout. The metadata file is at package offset `16459759`, length `9148328`. String-name inspection and bounded literal-table decoding supplied the names below. They do not supply method bodies, signatures, object addresses, or proven call paths. [T2], [T4]

### Content fingerprints

These are SHA-256 values of the received bodies, not assertions about the meaning of hashed filenames.

| Artifact | SHA-256 |
| --- | --- |
| T1 HTML | `ff4125fd514fe67edfdc84600080c02af22b44e08059b09d5853062b10c8c31b` |
| T2 loader | `de82b056302e1bfe1c0524e88b377535a508a9538fcc6a4f9533806c03d3548d` |
| T3 framework | `bd2fce60698f3cd563cc1d2a635afaf7045a4a6d78e9edd3f478f89c2e2993ca` |
| T4 data | `f1ab3ef3d2b6b04b28104bac55b25c007e78a24242a56c354b640428daf3053d` |
| T5 WASM | `77a05d62cc5cc6fd5d1a1f5b4f4d25d8c0d1f9e8ea4740c08326f8ec83e2b446` |
| T6 version JSON | `d739dca71d6be9e29a6b608c6a4e9062f5f206c5d0a3de4327908402dd66e07a` |

## How the inspected client works

### Boot and visible client

Verified target code creates `#unity-canvas`, calls `createUnityInstance` with the three build resources, and publishes the successful result as `window.spinelInstance`. The page also exposes `window.spinelShell` and `window.spinelSetSharpness`. The canvas receives keyboard focus after initialization. The inspected HTML supplies the canvas and loading shell. It does not supply a DOM tree of monsters, gameplay buttons, or inventory entries. [T1]

The page saves display preferences in `localStorage` and enables Unity's `autoSyncPersistentDataPath`. Its comment describes persistent PlayerPrefs including a saved ID. This is evidence of browser persistence, not evidence of a saved password, login token schema, or account authentication protocol. [T1]

### Transport and authentication

Verified target framework function `_SpinelSocketBridgeUrl` constructs a URL using the page's scheme, host, containing folder, and `net`. At the current HTTPS root this evaluates to `wss://tr-play.playall.in.th/net`. `_SpinelSocketOpen` constructs a browser `WebSocket` from its supplied URL and selects `arraybuffer` binary delivery. The WASM module imports both functions. This establishes a target-specific transport implementation and a derived bridge address. It does not establish a successful handshake, authentication policy, or backend server topology. [T3], [T5]

Inbound binary messages are copied into the WASM heap and delivered with socket ID, pointer, and length. Text messages use a separate callback. Outbound `_SpinelSocketSend` copies heap bytes into the WebSocket, while `_SpinelSocketSendText` sends text. Both return failure when the socket is absent or not open. Their success return follows `send`, not a game-server acknowledgement. No packet boundary, opcode layout, field encoding, heartbeat interval, or application-level encryption follows from this JavaScript alone. [T3]

The constructor passes a URL without an explicit subprotocol argument. No username/password transformation is visible in this bridge. Browser cookie participation, URL credentials, any initial text handshake, and authentication inside subsequent messages remain unknown. The login UI and networking metadata contain `PregameLoginModel`, `GameServerTicket`, `GameServerLink`, `LoginAsync`, `SendHandoffLogin`, `SecondPasswordHint`, and `SecondPasswordFails`. These names are investigation leads, not verified required login steps. [T3], [T4]

Target metadata includes a diagnostic about requesting a LoginServer ticket and diagnostics for FirstLogin refusal and GameServer login refusal. Those strings suggest a staged login and handoff. They do not establish request bodies, required server selection, secondary-password policy, ticket lifetime, or the relationship to website registration and email verification. The website's registration claim cannot substitute for the game's authentication contract. [T4], [T8]

`OpcodeMap`, `FromSeed`, `wireOpcode`, and `logical` occur in target metadata. Their coexistence is a reason to investigate opcode translation before using fixed numeric IDs. It is not proof of a particular mapping algorithm. Diagnostic literals that mention packet IDs are not sufficient to encode those packets. [T4]

### Session lifecycle

Verified browser code stores a list under session-storage key `spinel.links`. `_SpinelLinkRemember` removes an old token and retains a new one. Release code sends a token to relative `net/release` through a beacon during unload, or a keepalive POST fallback. On framework initialization, stored tokens are copied into an inherited list, submitted for release, and removed from storage. This is browser behavior in the shipped artifact. Whether the service accepts a token, rotates it, or releases a character is untested. No token was obtained or transmitted in this research. [T3]

The socket bridge reports visibility changes, page show, and online events to WASM. It invokes a tick callback every second. A non-persisted `pagehide` calls cleanup, attempts token release, and closes sockets. `_SpinelUpdateReload` also calls cleanup before reload. The update watcher polls relative `version?b=<own-build>` while visible and calls WASM when it sees a different valid build marked outdated. The active-build JSON endpoint was verified separately. The one-second callback is not proof of a one-second server heartbeat. [T3], [T6], [T7]

Metadata contains `GatewayLink`, `Unconfirmed`, `gatewayReceived`, and `Resumed`, plus diagnostics about confirmed byte positions, retained bytes, retransmission, and a missing gateway session. These support the inference that the transport has resumable delivery beyond a raw socket. The replay window, acknowledgement encoding, retry limits, ordering, deduplication, and treatment of non-idempotent actions remain unknown. A clientless design must recover that contract before resending combat or digging commands after a disconnect. [T4]

### Inbound state and outbound actions

The following names occur in the downloaded target metadata. This is evidence that the build contains those concepts. It is **not** evidence that a host application can call them through `SendMessage` or that their state is authoritative. Names are intentionally not assigned to classes without a decoded type table and call-path proof. [T4]

| Needed area | Verified names or other artifact evidence | Missing contract |
| --- | --- | --- |
| World and character selection | `WorldEntry`, `CharacterEntry`, `CharacterList`, `CharacterStatusWire`, `CharacterBusy` | Required login sequence, stable IDs, selection request, busy-character behavior, single-session policy. |
| Position and movement | `WalkTo`, `StopWalking`, `OnWorldClick`, `ZoneLoaded` | Coordinate system, map identifiers, walkability data, movement encoding, correction events, map-transition acknowledgement. `MoveRequest` alone is ambiguous and is not treated as the avatar movement packet. |
| Character and monster state | `MonsterHpOf`, `MonsterMaxHpOf`, `UpdateMonsterHp`, `SetMyHp`, `SetPlayerHp`, `FaintMe`, `Revived` | Snapshot and delta schemas, entity identity and lifetime, units, authoritative HP, death and revive handling, event ordering. |
| Combat | `Attack`, `normalAttack`, `OnSkillHits`, `SkillUsed`, `OnItemUse` | Target selection, reach and line-of-sight rules, attack or skill payloads, cooldowns, acceptance, damage results, cancellation, rejection. |
| Digging | `PressDig`, `Preconditions`, `CancelDrilling`, `DrillLife`, `OnAutoSpotTaken`, `OnAutoSpotRefused`, `CarriedWeight` | Equipment requirements, tile or point encoding, start and cancel requests, timing or gauge interaction, durability, empty spot, occupied spot, inventory and weight results. |
| Existing automation leads | `AiWindow` appears in the initialization list. `AutoDrill` and AI diagnostic strings appear in metadata. | Whether these features are enabled, what they do on this server, whether they can be externally controlled, and whether they expose reusable state. Do not assume a ready-made bot API. |

The bridge's binary and text receive callbacks are possible places to observe traffic before WASM decodes it. They do not expose normalized game state. Nothing inspected establishes the complete observation set needed by the bot, including inventory, equipment, loot, map geometry, target ownership, and action results. [T3], [T4]

## Observation and control candidates

### Embedded client

Verified page-level hooks cover loading progress, canvas sizing, sharpness, fullscreen requests, and `spinelShell.onState` display settings. The page sends `SetBrowserFullScreen` to the `WebShell` object. Framework diagnostics also publish `window.spinelProbe` and `window.spinelMemoryReport` by parsing JSON supplied by WASM. Their actual payload schemas and availability were not observed. These are useful diagnostic leads, not a verified character-state feed. [T1], [T3]

Unity documents `SendMessage` as a call to a method on a named GameObject. It does not expose arbitrary C# methods, and it accepts no argument or a single string or number. Consequently, finding `WalkTo` or `PressDig` in metadata does not make `spinelInstance.SendMessage` a proven movement or digging API. The target's confirmed example is a display-shell method. [U1], [T1], [T4]

Apple documents `WKWebView` as an in-app web view and `WKUserScript` as a mechanism for script injection, including a selected content world. Therefore, an embedded web view that loads the real HTTPS target origin is a plausible investigation path. A script in the page world could be evaluated as an observation point around the target's browser APIs. This is a platform-backed candidate, not a validated target hook. Nothing in this research proves WebGL execution, trusted input behavior, WASM memory use, or GPUI composition on macOS. Those require the embedding compatibility work. [A1], [A2], [T1], [T3]

Input automation is another unproven candidate. The framework checks `event.isTrusted` for some gesture-dependent text and link behavior. Synthetic DOM events cannot be assumed equivalent to actual user input across the client. Gameplay observation through pixels also requires its own accuracy and recovery proof. No screen-recognition or input-control prototype ran here. [T3]

### Clientless session

Inference from the shipped bridge is that a native implementation would need the target's gateway contract as well as the game serialization. Copying the WebSocket URL is insufficient. A complete investigation must account for initialization, build negotiation, authentication, ticket handoff, opcode translation, framing, retained bytes, acknowledgements, reconnect, and release. Some answers may be recoverable by further static analysis of these exact public binaries. This report does not claim that credentials are necessary for every remaining discovery. [T3], [T4], [T5]

A hidden Unity canvas still runs the game client. It does not demonstrate the requested clientless mode. A second connection to the same character is also not established as a valid handoff strategy. The parent requires one account and one character, while this target's simultaneous-login and ownership rules are unknown. [Planning map](https://github.com/oDestroyeRo/opentrickster/issues/2) [T4]

Unity's generic web networking documentation explains browser restrictions on direct TCP and UDP access and the use of browser networking facilities. It does not identify this game's backend as Unity Netcode or establish native TCP compatibility. The custom bridge artifact is the relevant target evidence. [U2], [T3]

## Findings for the decision tickets

1. Keep an embedded web-client path available for the in-app game requirement. There is a concrete canvas, loader, and instance handle to investigate. Do not describe an external browser window as meeting that requirement. GPUI integration and target execution remain separate proofs. [T1], [A1]
2. Treat embedded observation and embedded control as separate questions. Display callbacks are verified. Gameplay state and commands need a target-matched adapter or demonstrated input and observation method. `SendMessage` alone does not close either gap. [T1], [T3], [U1]
3. Keep clientless feasibility unresolved until the bridge and game contracts are established. Record the deployment build and artifact fingerprints in any subsequent compatibility proof. Do not select another emulator's packet definitions as the target contract. [T3], [T4], [T6]
4. Specify a single session owner and an explicit mode transition in the MVP design. This is a recommendation, not a verified server guarantee. Reloading or disposing the embedded client triggers release attempts, so mode switching needs observed lifecycle behavior before it can promise continuity. [T3]
5. Require authoritative action outcomes and session-loss handling in both modes. A successful JavaScript send is not an accepted attack or dig. Resume diagnostics leave retransmission semantics unresolved. [T3], [T4]

## Gotchas, remaining gaps, and next investigation

No inspected source supplies a complete target-matched protocol specification, gameplay SDK, or public source repository. Exact-name web searches for the target title, host, `Spinel.Net`, and `SpinelSocketBridgeUrl` did not locate one. This describes this search, not global nonexistence. The owner website's asset failure limited investigation of its authentication UI. The web fetch tool could not open the target hosts, but bounded direct HTTP with a browser user agent succeeded.

The smallest next public investigation is a source-version-matched analysis of `GatewayLink`, `OpcodeMap`, `PacketReader`, `PacketWriter`, login handoff, and the movement, combat, and drill call paths in T4 and T5. Produce field layouts, byte examples, and call-path evidence. Resolve whether the gateway bytes carry whole game messages or stream fragments. If binary analysis cannot establish a field, record that field rather than borrowing a format.

Track a separate access and verification task before runtime work. Its minimum input is an existing operator-controlled test account and explicit authorization for a bounded live-session exercise. Do not ask for credentials in public issues. Alternatively, target-matched source or an existing protocol specification could settle static contracts before gameplay access. This research made no signup, credential request, or third-party contact.

The runtime task needs the following acceptance evidence:

| Unsettled question | Smallest concrete proof |
| --- | --- |
| Authentication and session ownership | One redacted trace from login through character selection, including ticket handoff, failure codes, and a second-login policy observed only under separately approved scope. |
| Gateway and lifecycle | Timestamped normal logout, reload, network interruption, successful resume or explicit failure, and build mismatch. Establish acknowledgement and retransmission behavior without duplicating gameplay actions. |
| Observation | Match a decoded position, HP, monster identity, inventory change, and drill state against the visible client for the same character and build. |
| Movement, combat, and digging | Individually trace one approved move, attack, dig start, dig result, and cancellation to their outbound request and authoritative inbound result. Record refusal and resource-exhaustion behavior. |
| Embedded control | Demonstrate the selected observation and control method while the live target is visible inside the macOS application. Identify the exact callable interface or native input route. |
| Clientless behavior | Reproduce the authenticated state and approved actions with the Unity runtime absent. Show the dashboard from decoded live state, then stop and release the session. |

These are follow-up task requirements, not actions authorized or performed by issue #3. Their results are needed before implementation-ready decisions can claim either complete operating mode. The bounded public research question is answered with those unknowns explicit.

## Validation of this report

- Verified the authenticated developer and issue #3 assignee both equal `oDestroyeRo` before research. Verified repository `oDestroyeRo/opentrickster`, starting SHA, and the clean isolated branch.
- Read the parent Notes, assigned ticket, tracker instructions, and domain instructions. No glossary or ADR existed in the starting tree.
- Checked response status, bounded received bodies, and computed content hashes for the target artifacts. The data directory entries were checked against package bounds. Bridge function names were cross-checked against WASM imports. Unity version evidence was cross-checked between the package header and WASM.
- Inspected the complete target HTML and custom framework bridge functions. Metadata names and literals were treated as static leads. No target JavaScript or WASM was executed.
- No browser session, authenticated capture, live gameplay check, embedded-app test, or clientless test ran. No game capability is declared supported or unsupported from an unrun check.
- Repository validation is document readback, source and evidence-level review, whitespace checking, and confirmation that the sole change is this report. No bot code, dependency, asset, glossary, or other repository change belongs to this branch.

## Sources

Target artifacts are primary evidence retrieved on 2026-11-08. Their public URLs may change or disappear. Use the fingerprints above to detect drift. Unity pages below document version 6000.0 platform behavior and are not presented as an exact 6000.6.5f1 compatibility certification. Apple pages are current unversioned documentation, retrieved on the same date.

[T1]: https://tr-play.playall.in.th/
[T2]: https://tr-play.playall.in.th/Build/877598568ac7f0dc41389b0d6e411b8b.loader.js
[T3]: https://tr-play.playall.in.th/Build/4937d6d47bade23f56e5ad4b5bf2e2b9.framework.js.gz
[T4]: https://tr-play.playall.in.th/Build/9b2286992122d3c1cc081492dc31fee5.data.gz
[T5]: https://tr-play.playall.in.th/Build/657890b2bd8548390dd7c7fad76093be.wasm.gz
[T6]: https://tr-play.playall.in.th/version
[T7]: https://tr-play.playall.in.th/version?b=f5e972fa2be429602c20d6168013a20f
[T8]: https://tr.playall.in.th/
[U1]: https://docs.unity.com/en-us/engine/6000.0/manual/platform-specific/webgl/develop/interactingwithbrowserscripting/web-interacting-browser-unity-to-js
[U2]: https://docs.unity.com/en-us/engine/6000.0/manual/platform-specific/webgl/develop/networking
[A1]: https://developer.apple.com/documentation/webkit/wkwebview
[A2]: https://developer.apple.com/documentation/webkit/wkuserscript
