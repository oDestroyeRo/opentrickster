# GPUI hosting for the Trickster web client on macOS

The goal is a macOS bot with the game visible inside its application window in client-assisted mode and only the information dashboard visible in clientless mode. This report answers [research ticket #4](https://github.com/oDestroyeRo/opentrickster/issues/4) under [planning map #2](https://github.com/oDestroyeRo/opentrickster/issues/2). It supplies an embedding candidate and explicit proof gates for the implementation specification. It does not establish a gameplay control interface.

Research date is 2026-11-08 UTC. The actual starting repository commit is `6b8162f1f7a42b2bd9dd02c1b086be360914bb8d`. The prepared branch is `research/gpui-game-embedding`. The tree was clean before this report. The resolution comment and research handoff record the final report commit and its immutable blob URL.

## Overview

**The documented APIs provide a credible in-window path, but the combined GPUI and target-game integration remains unproven.** GPUI exposes its macOS `NSView` through `raw-window-handle`. Apple supports adding `WKWebView` to a native view hierarchy. Wry supplies a Rust `build_as_child` path that adds its `WKWebView` to the supplied `NSView`. These are verified component contracts. They are not a successful GPUI embedding demonstration. [GPUI raw handle][g-window], [macOS raw handle][g-mac], [Apple webview][a-webview], [Wry child API][w-api], [Wry macOS implementation][w-mac].

The candidate for the decision ticket is one GPUI window with a reserved, non-overlapping game rectangle and a native `WKWebView` child in that rectangle. GPUI renders the dashboard. WebKit renders the target's web client. A small macOS adapter owns placement, native focus, messages, and destruction. This is an architectural inference from those component contracts, pending the focused proof below. A separate browser window does not satisfy the recorded product requirement. [Planning map #2](https://github.com/oDestroyeRo/opentrickster/issues/2), [GPUI window][g-window], [Apple webview][a-webview].

## Evidence classes and versions

The following labels apply throughout this report.

- **Target evidence** is public HTML, loader code, or response headers fetched from the named target. Source inspection does not imply successful execution.
- **Platform evidence** is owner-maintained GPUI or Wry source and Apple or WebKit documentation.
- **Generic Unity evidence** explains published Unity APIs for the cited manual version. It does not identify the target's Unity engine version or original Trickster behavior.
- **Inference or proposal** connects component contracts or recommends an implementation requirement. **Unknown** means that the needed evidence was not obtained.

| Component | Exact reference inspected | Limits |
| --- | --- | --- |
| GPUI and macOS backend | Zed `main` at `b5cd310b3b54dfc659fcc06453e6358843dfe216`, committed 2026-11-07 09:46:58 UTC. The manifests declare `gpui` 0.2.2 and `gpui_macos` 0.1.0. | Pin the repository revision, not just these package version strings. The README calls GPUI pre-1.0. [Manifest][g-manifest], [backend manifest][g-mac-manifest], [README][g-readme]. |
| GPUI build inputs | The same revision pins Rust 1.98.1. Its workspace uses edition 2024, `raw-window-handle` 0.6, and `objc2` 0.6. | These are inspected source requirements, not a build performed here. Current examples use `gpui_platform::application`. [Toolchain][g-toolchain], [workspace manifest][g-workspace], [example][g-example]. |
| Wry | `dev` at `e776bbb697fe444c6308d4d7f3edd8863ce0489b`, committed 2026-11-06 14:34:46 UTC. Its manifest declares 0.57.1 and Rust 1.85. | This pins the inspected source, not a dependency selection or published-release equivalence. [Manifest][w-manifest]. |
| Apple API documentation | Live documentation retrieved 2026-11-08, including its Markdown availability metadata. | SDK availability and browser-engine capability are different constraints. Apple APIs do not certify this game. [Apple webview][a-webview], [content worlds][a-world], [fullscreen][a-fullscreen]. |
| Local API check | macOS 27.3 build `26C5035r`, Apple Swift 6.4.2, macOS SDK 27.2, arm64. | Static typechecking only. No runtime behavior or older-OS compatibility was tested. |
| Unity documentation | Unity 2022.3 browser and scripting documentation, plus Unity 6000.0 input, technical limitations, and deployment documentation. | Reference versions only. No target engine version was found in the inspected HTML and loader. [Browser requirements][u-browser], [scripting][u-js], [input][u-input], [limitations][u-limits], [deployment][u-deploy]. |

No original-game or emulator evidence establishes an embedding fact in this report. No conclusion assumes compatibility with another Trickster client.

## Target-server findings

The following facts come from the [target page][t-page] and its [referenced loader][t-loader], retrieved through public HTTP GET requests on 2026-11-08. No JavaScript or gameplay was executed.

1. **Target evidence.** The page title identifies Trickster Online and Spinel Project. It creates `#unity-canvas` and calls `createUnityInstance`. The configuration names Playall, Spinel, and product version `1.0`. Product version is not Unity engine version. [Page][t-page].
2. **Target evidence.** The loader rejects a missing WebGL2 context. It also gates startup on `hasWasm2023`, whose probes include bulk memory, non-trapping conversions, sign extension, SIMD128, and Wasm exceptions. Its error message directs Safari users to 16.4 or later. `hasWasmThreads` is false in the inspected loader. These statements describe that loader, not every asset or the live server's requirements. [Loader][t-loader].
3. **Target evidence.** The page publishes `window.spinelInstance` after the Unity instance promise resolves. It defines `window.spinelShell` callbacks for display state, splash loading, and fullscreen. Its `send` helper targets `WebShell` through `SendMessage`, including `SetBrowserFullScreen`. This establishes display integration hooks. No combat, digging, character-state, or quest API was established by these artifacts. [Page][t-page].
4. **Target evidence.** The page focuses the canvas after initialization, suppresses selected browser keyboard shortcuts, uses `ResizeObserver`, and tracks device pixel ratio. It offers fixed screen sizes and a fill-window setting. Its desktop default is 1280 by 720 CSS pixels, scaled down to fit the available rectangle. The native pane must accommodate the page's own sizing logic. [Page][t-page].
5. **Target evidence.** Public HEAD requests returned 200 for the referenced data, framework, and Wasm URLs. The Wasm response declared `application/wasm`, and the framework declared `application/javascript`. These requests did not fetch those bodies or verify runtime loading. [Data URL][t-data], [framework URL][t-framework], [Wasm URL][t-wasm].

The public artifact identifiers are recorded to detect drift. CDN injection or a server update can change the page hash without changing the game.

| Artifact | Observed identifier |
| --- | --- |
| Page GET at 13:57:25 UTC | 27,217 bytes. SHA-256 `ff4125fd514fe67edfdc84600080c02af22b44e08059b09d5853062b10c8c31b`. |
| Loader GET | `Build/877598568ac7f0dc41389b0d6e411b8b.loader.js`. 27,914 bytes. SHA-256 `de82b056302e1bfe1c0524e88b377535a508a9538fcc6a4f9533806c03d3548d`. |
| Build URLs in page | Data `9b2286992122d3c1cc081492dc31fee5.data.gz`. Framework `4937d6d47bade23f56e5ad4b5bf2e2b9.framework.js.gz`. Wasm `657890b2bd8548390dd7c7fad76093be.wasm.gz`. |
| HEAD responses at 14:00:21 UTC | All three returned 200. No `Content-Encoding` header appeared in these HEAD responses. This does not establish how a browser GET negotiates or decodes the bodies. |

**Unknown.** The target has not booted in a `WKWebView` during this research. Login flow, authentication redirects, storage behavior, actual Wasm compilation, audio, game assets, network transport, and playable character rendering remain untested. Public availability of the HTML and loader does not answer those questions. [Page][t-page], [loader][t-loader].

## How the native embedding path works

### GPUI owns the window and its Metal renderer

**Platform evidence.** GPUI's macOS backend creates its own native view, adds it to the content view, and makes it first responder. Its raw window handle points to that `NSView`. Its backing-layer callback returns the renderer's layer. The Apple renderer owns a `CAMetalLayer` for window rendering. [Mac window][g-mac], [Metal renderer][g-metal].

**Platform evidence.** The public `Window` implements `raw_window_handle::HasWindowHandle` and delegates to its platform window. Its inherent `window_handle()` method instead returns GPUI's logical `AnyWindowHandle`. Code that needs the operating-system handle must use the trait contract, for example `HasWindowHandle::window_handle(window)`, or pass `&Window` to a consumer generic over that trait. Treat the returned `NSView` as borrowed from a live GPUI window. [GPUI window][g-window], [Mac window][g-mac].

**Platform evidence with a search limit.** No ready-made webview or native-child element was found in the inspected GPUI public exports, element implementations, and `PlatformWindow` interface at this revision. The existence of a raw handle is an integration entry point, not a GPUI layout, focus, clipping, or ownership contract for foreign views. [Exports][g-exports], [elements][g-elements], [platform interface][g-platform].

### WebKit owns the game view and web runtime

**Platform evidence.** Apple documents `WKWebView` as a native view that embeds interactive web content alongside native application views. AppKit `addSubview` attaches and retains a child view. WebKit manages rendering through a separate content process. This provides a normal native-view composition path without exporting game pixels to GPUI. [Apple webview][a-webview], [AppKit subviews][a-add], [content-process termination][a-terminate].

**Platform evidence.** Wry's `build_as_child` accepts `HasWindowHandle`. Its macOS implementation reads `RawWindowHandle::AppKit.ns_view` and calls `ns_view.addSubview(&webview)`. Its ordinary `build` path instead replaces the window's content view with a Wry parent. Use of the child path is therefore essential to the candidate that preserves GPUI's existing native hierarchy. [Wry API][w-api], [macOS implementation][w-mac].

**Proposal.** Let GPUI remain the application and AppKit event-loop owner. Construct the child on the main thread after the GPUI window exists. Retain one webview adapter for its lifetime. A GPUI render pass only reports current placement. It must not recreate or navigate the webview on every dashboard update. GPUI already runs `NSApplication`; Wry requires a running event loop rather than prescribing a second one. Whether callbacks compose without reentrancy defects still needs the runtime proof. [GPUI event loop][g-mac-platform], [Wry API][w-api], [Apple main-actor API][a-webview].

### The Metal texture route is a different, unproven bridge

**Platform evidence.** GPUI's `surface` element accepts `CFRetained<CVPixelBuffer>` and paints that buffer. Apple `takeSnapshot` asynchronously produces a platform-native image. The inspected public APIs do not establish a continuous `WKWebView` to `CVPixelBuffer` export. GPUI's Metal renderer alone does not supply one. [GPUI surface][g-surface], [Apple snapshot][a-snapshot], [Metal renderer][g-metal].

**Inference.** A screenshot loop would add capture, conversion, frame scheduling, and input routing work. This report does not certify its throughput, latency, WebGL capture correctness, or ability to preserve normal game interaction. A native child view avoids needing that bridge. Choose the native child candidate for the first proof. Reconsider texture composition only if a demonstrated requirement cannot be met with native view composition. [GPUI surface][g-surface], [Apple snapshot][a-snapshot].

## Runtime support and version constraints

**Platform evidence.** WebKit announced WebGL2 with a Metal backend in Safari 15 on 2021-10-26. It announced Wasm SIMD support in Safari 16.4 on 2023-03-27. Unity 2022.3 documents WebGL2, WebAssembly, and a compatible desktop browser as prerequisites. These facts establish engine capabilities, not compatibility with this target in an embedded view. [WebKit 15][k-15], [WebKit 16.4][k-164], [Unity browser requirements][u-browser].

**Target evidence and inference.** The target loader's Wasm feature gate is stronger than generic WebGL2 availability. Its Safari 16.4 guidance is a lower-bound candidate for testing, not a supported macOS minimum. Pin and test the actual macOS and system WebKit versions. Do not equate changing the user-agent string with upgrading the runtime. The target's engine patch, supported OS policy, GPU and memory requirements remain unknown. [Loader][t-loader], [Apple webview][a-webview].

**Generic Unity evidence.** Unity documents MIME and content-encoding requirements for compressed builds. Its threaded native-code option also has secure-context and isolation-header requirements. The target's `.gz` names and successful HEAD requests are insufficient to establish either correct browser delivery or a need for threading. Load the original HTTPS URL directly in the candidate webview and inspect its actual failures before proposing a local proxy, iframe, custom scheme, or copied build. [Unity deployment][u-deploy], [Unity limitations][u-limits], [target page][t-page].

**Unknown.** No distribution artifact, deployment target, signing entitlement set, or complete GPUI build was validated. GPUI's README currently requires Xcode and macOS components for its Metal build. This machine's command-line Swift check does not replace that requirement. [GPUI build instructions][g-readme].

## Input, focus, and placement contracts for the specification

The following are proposed acceptance requirements derived from the verified APIs. Their behavior in the combined application remains to be demonstrated.

- **Mouse and keyboard ownership.** The webview receives native input in the game rectangle. GPUI receives dashboard input. Do not send a physical event to both. AppKit's responder chain and GPUI's key-equivalent handling need joint verification, especially for Command shortcuts and Escape. The target page already prevents selected browser defaults. [AppKit focus][a-focus], [GPUI input handling][g-mac], [page][t-page].
- **Focus transfer.** Moving to the game requires native first-responder transfer and DOM canvas focus. Moving to the dashboard requires restoring the GPUI native view and the intended GPUI `FocusHandle`. `Window::focus` updates GPUI's logical focus, without an AppKit `makeFirstResponder` call in that method. Wry exposes `focus` and `focus_parent`, but its implementation ignores the native responder call's Boolean result. Verify the actual responder and observable typing rather than treating `Ok(())` as proof. [GPUI focus][g-window], [Wry focus implementation][w-mac], [AppKit focus][a-focus], [page][t-page].
- **Text and held keys.** Verify dashboard text entry, game chat, Thai input, IME composition, clipboard commands, modifier release, and a focus switch while a movement key is held. Generic Unity 6000.0 documentation says the game may capture all keyboard input delivered to its web page even when its canvas lacks focus. It does not establish this target's setting. Do not assume the shipped client exposes a host-callable way to change that C# setting. [Unity input][u-input], [target keyboard handlers][t-page].
- **Geometry.** Derive the game rectangle from current GPUI element bounds, including pane changes that do not resize the top-level window. `canvas` callbacks or a custom `Element` receive layout bounds. `Context::observe_window_bounds` supplies window changes. Wry child `set_bounds` converts logical or physical dimensions using the window scale factor and handles flipped AppKit coordinates. Use logical units consistently and avoid applying the backing scale twice. [GPUI canvas][g-canvas], [context observers][g-context], [Wry geometry][w-mac].
- **Retina and resizing.** Test live window resize, pane resize, display changes, and different backing scales. WebKit controls its backing resolution. The target then sizes and scales its canvas using page geometry and device pixel ratio. Preserve that logic before adding host overrides. Readability at the chosen minimum game-pane size is unknown. [Target sizing][t-page], [GPUI scale handling][g-mac], [Wry geometry][w-mac].
- **Clipping and overlays.** A foreign native child does not participate automatically in GPUI's element clipping and paint ordering. Reserve a non-overlapping rectangle for the first proof. Verify menus and dialogs over that rectangle explicitly. Hiding the child while an application modal is active is a candidate policy, not verified behavior. [GPUI element API][g-elements], [AppKit hierarchy][a-add], [Wry child attachment][w-mac].

### Fullscreen needs an explicit host policy

**Platform evidence.** Public `WKPreferences.isElementFullscreenEnabled` is available from macOS 12.3 and defaults to false. Apple states that when enabled, a page fullscreen request removes the webview from the app's view hierarchy. The pinned Wry implementation enables this preference on newer systems and uses a private fullscreen key on older systems. Its native macOS extension exposes the underlying webview handle. [Apple fullscreen][a-fullscreen], [Wry configuration][w-mac], [Wry native extension][w-api].

**Proposal.** Keep HTML element fullscreen disabled in the MVP embedding proof. If Wry is used, construct without the target URL, restore that preference through the exposed native webview API, and verify it before loading the target. An application-window fullscreen command can preserve the internal dashboard and game relationship. The target game calls `requestFullscreen` and retries rejected requests, so leaving that preference false is not by itself a complete game-settings policy. The decision ticket must choose how the in-game fullscreen setting is presented or adapted and test the resulting state synchronization. [Target fullscreen handlers][t-page], [Apple fullscreen][a-fullscreen], [Wry configuration][w-mac].

**Alternative candidate.** A direct `WKWebView` adapter through Rust Objective-C bindings or a small native shim can own only the required public Apple APIs. It avoids inheriting Wry's fullscreen defaults. This adapter is unbuilt, and its Rust ownership and callback plumbing need the same focused proof. Wry's child implementation is evidence for the native composition technique, not an obligation to adopt Wry. [Apple webview][a-webview], [Wry implementation][w-mac].

## Lifecycle and host-to-game messages

**Platform evidence.** Apple provides `WKNavigationDelegate` for navigation outcomes and a content-process termination callback. Website data stores can be persistent or nonpersistent. The target page uses local storage for display preferences and requests persistent-data synchronization in its Unity configuration. A successful page navigation is therefore distinct from a ready Unity instance, and browser data lifetime needs an explicit application policy. [Navigation and configuration][a-webview], [termination][a-terminate], [data stores][a-store], [page][t-page].

**Proposal.** Model the host's presentation state as absent, loading, ready, failed, and closing. Invalidate callbacks when the page generation changes. Distinguish network failure, navigation failure, Unity startup failure, and content-process termination in the dashboard. Stop accepting client-assisted automation commands after client loss. Release message handlers, delegates, the child view, and native references before the GPUI parent disappears. GPUI exposes release, window-close, and application-quit hooks. `stopLoading` stops navigation, not a verified live-game logout protocol. [GPUI context hooks][g-context], [GPUI app hooks][g-app], [Apple webview][a-webview], [termination][a-terminate].

**Platform evidence.** A `WKUserContentController` installs scripts and message handlers. JavaScript sends messages using `window.webkit.messageHandlers.<name>.postMessage(...)`. The host can evaluate JavaScript or call an asynchronous function with structured arguments. Content worlds separate JavaScript globals while sharing the document. Calls that access `window.spinelInstance` require the page's content world, or an explicit relay to it. These APIs are available without changing the server's source, but an injected bridge is not a server-promised game API. [User scripts][a-script], [message handlers][a-message], [content worlds][a-world], [evaluation][a-eval], [async calls][a-async], [page global][t-page].

**Proposal.** The first bridge exchanges only an initialization handshake, display state, and test messages. Include a page-generation identifier and a message kind. Validate message shape, source frame, and expected origin at the native boundary. Dispatch updates to GPUI without holding a GPUI update borrow across JavaScript completion. Keep Rust commands structured rather than concatenating unescaped values into JavaScript source. These are adapter design requirements, not observed behavior. [Apple message controller][a-controller], [async argument API][a-async], [GPUI window update API][g-window].

**Generic Unity evidence and target limit.** Unity documents `SendMessage` for calling methods on named game objects. Such a call requires the correct object and exposed method. The target's display-shell calls demonstrate that pattern only for those named hooks. They do not prove methods to enumerate monsters, inspect inventory, fight, dig, accept quests, or turn in quests. No such commands were attempted. [Unity scripting][u-js], [target shell][t-page].

**Inference.** Clientless mode cannot be specified as merely hiding this webview. The product requires a live controlled character while only the dashboard is visible. Neither native embedding nor `evaluateJavaScript` establishes the independent transport and state model that requirement needs. WebKit also documents throttling inactive content, so hidden-page timers are not an established substitute. The integration decision needs separate target transport evidence. One account and one character must remain the shared session constraint across both modes. [Planning map #2](https://github.com/oDestroyeRo/opentrickster/issues/2), [WebKit inactive content][k-power].

## What the decision tickets can use

| Candidate or capability | Evidence status | Consequence for the MVP plan |
| --- | --- | --- |
| Native `WKWebView` child alongside a GPUI dashboard | Apple and Wry support the child-view contracts. GPUI supplies the matching raw AppKit handle. The complete combination is unproven. | Retain Rust GPUI as the preferred candidate, conditional on the focused proof. [Native APIs][a-add], [Wry][w-mac], [GPUI][g-mac]. |
| Native view through Wry | Explicit child attachment, bounds, focus, and native-handle APIs exist at the pinned revision. Fullscreen defaults require attention. | Use `build_as_child`, never ordinary `build` for this candidate. Verify restored fullscreen policy and callback behavior. [Wry API][w-api], [implementation][w-mac]. |
| Direct Apple adapter | Apple's required embedding and messaging APIs exist. The API fixture typechecks. Rust integration is unbuilt. | Keep as the second implementation candidate if direct control of configuration is preferable. [Apple webview][a-webview], [controller][a-controller]. |
| WebKit executing the target game | Loader prerequisites and WebKit engine capabilities are documented. No target runtime result exists. | Keep target compatibility open. Do not label it supported or unsupported. [Loader][t-loader], [WebKit 16.4][k-164]. |
| Continuous WebKit frames inside a GPUI `surface` | GPUI accepts pixel buffers. The inspected WebKit API provides snapshots, without an established continuous pixel-buffer bridge. | Do not include this bridge in the MVP estimate as an existing capability. [Surface][g-surface], [snapshot][a-snapshot]. |
| Combat or digging over a host/game bridge | Only display-shell hooks were established. | Require separate target state and action evidence before implementation. [Target page][t-page]. |
| Clientless live control | No transport proof in this ticket. | Keep it an independent integration gate. A hidden browser does not resolve the stated requirement. [Planning map #2](https://github.com/oDestroyeRo/opentrickster/issues/2). |

Quest-based leveling research belongs to the other assigned research scope. This embedding report neither supplies quest mechanics nor changes the MVP exclusion of automatic quest acceptance, completion, and turn-in. [Planning map #2](https://github.com/oDestroyeRo/opentrickster/issues/2).

## Remaining gaps and smallest next steps

The bounded documentation question is answered. Closing this research ticket does not approve a stack choice or claim runtime compatibility. The following work must remain visible in the implementation decision and verification plan.

1. **Native integration proof, no account required.** In a temporary project pinned to the GPUI revision above, attach a single child `WKWebView` to one GPUI window. First use researcher-authored HTML with an animated WebGL2 canvas and bidirectional messages. Prove simultaneous GPUI updates, focus transfers, pane resizing, clipping policy, child-before-parent teardown, and safe handling of a parent-initiated close. Preserve the existing content view. Record macOS, SDK, GPU, source revisions, and actual failures. This is the smallest next implementation investigation. It was not performed here.
2. **Target boot proof, still no account required.** After the native proof passes, load the original target HTTPS page without changing its build. Record loader feature checks, Unity promise completion or error, visible pre-login output, real GET response headers, storage behavior, and WebKit process termination. Do not assume success from Safari alone. A WebKit failure requires a specific diagnosis before researching another embedded engine.
3. **Human access gate for character behavior.** If later verification requires sign-in or a live character, track a human-in-the-loop access task. Obtain a user-approved test account/session and exact allowed actions through the owning workflow. No signup, credential request, authentication, account mutation, or gameplay action occurred in this research. Do not treat that future gate as answered by this report.
4. **Product decisions.** Record the supported macOS and WebKit baseline, Wry versus direct native adapter, minimum game-pane size, fullscreen behavior, persistence policy, and session behavior on view loss or mode changes. Choose the baseline after evidence from the intended deployment machines. The macOS 12.3 API compilation target below is not a proposed game minimum.
5. **State and control contract.** Track target-specific read and action interfaces separately, including the clientless transport. Require evidence for combat and digging commands and state freshness. The known shell methods provide no substitute for that contract. [Target page][t-page], [loader][t-loader].

## Actual validation and reproducibility

Only this Markdown report changes the repository. Public source copies, HTTP response captures, and the compiler fixture are scratch files under `/tmp/opentrickster-issue4`. No bot application, dependency change, copied game assets, account interaction, screenshot, or benchmark was produced. The loader and HTML were inspected as public code artifacts. Large build bodies were not fetched.

The source investigation pinned revisions through `gh api`, fetched immutable raw source files, and used scoped `rg` searches and bounded reads. The inspected GPUI source covers public exports, elements, window and platform interfaces, macOS implementation, and Apple Metal rendering. Apple documentation was read through its public Markdown form where the HTML required JavaScript. An initial default Python loader request returned 403. A normal browser-user-agent GET returned 200. This is a retrieval result, not an embedded-browser test.

The temporary Swift fixture declares a main-actor function that creates a `WKWebView` configuration, installs a page-world script handler, attaches a child view, sets its frame, transfers focus, invokes JavaScript APIs, removes handlers, and detaches the view. It also declares the content-process termination delegate method. The function is never called. The corrected fixture passed the following command with exit code 0 and no warnings.

```sh
xcrun swiftc -typecheck -warnings-as-errors \
  -target arm64-apple-macos12.3 \
  /tmp/opentrickster-issue4/EmbeddingAPI.swift
```

Fixture SHA-256 is `f4b74aac2604caca14bb9a9c3bc8934fddf7afb4df825c94ef9797a84f88d60d`. The final compile uses the completion-handler spelling `callAsyncJavaScript(_:arguments:in:in:completionHandler:)`. The current SDK also offers a different asynchronous overload. This check proves API availability to that compiler and deployment target. It does not prove a GPUI build, view composition, browser rendering, game compatibility, message delivery, or live control. [Apple async signature][a-async].

Before publication, validation includes report readback, source and citation review, `git diff --check`, confirmation that the commit changes only the assigned report, committed-blob readback, and confirmation that the pushed branch names the actual report commit. The ticket comment supplies that immutable report URL and records the unresolved runtime gates.

## Primary sources

All sources were accessed on 2026-11-08. GitHub source references below are immutable. Apple and target URLs can change. The target hashes above identify the observed public artifacts.

[g-window]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/window.rs
[g-mac]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui_macos/src/window.rs
[g-metal]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui_apple/src/metal_renderer.rs
[g-manifest]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/Cargo.toml
[g-mac-manifest]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui_macos/Cargo.toml
[g-workspace]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/Cargo.toml
[g-toolchain]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/rust-toolchain.toml
[g-example]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/examples/hello_world.rs
[g-readme]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/README.md
[g-exports]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/gpui.rs
[g-elements]: https://github.com/zed-industries/zed/tree/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/elements
[g-platform]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/platform.rs
[g-surface]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/elements/surface.rs
[g-canvas]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/elements/canvas.rs
[g-context]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/app/context.rs
[g-app]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui/src/app.rs
[g-mac-platform]: https://github.com/zed-industries/zed/blob/b5cd310b3b54dfc659fcc06453e6358843dfe216/crates/gpui_macos/src/platform.rs
[w-api]: https://github.com/tauri-apps/wry/blob/e776bbb697fe444c6308d4d7f3edd8863ce0489b/src/lib.rs
[w-mac]: https://github.com/tauri-apps/wry/blob/e776bbb697fe444c6308d4d7f3edd8863ce0489b/src/wkwebview/mod.rs
[w-manifest]: https://github.com/tauri-apps/wry/blob/e776bbb697fe444c6308d4d7f3edd8863ce0489b/Cargo.toml
[a-webview]: https://developer.apple.com/documentation/webkit/wkwebview
[a-add]: https://developer.apple.com/documentation/appkit/nsview/addsubview(_:)
[a-focus]: https://developer.apple.com/documentation/appkit/nswindow/makefirstresponder(_:)
[a-fullscreen]: https://developer.apple.com/documentation/webkit/wkpreferences/iselementfullscreenenabled
[a-terminate]: https://developer.apple.com/documentation/webkit/wknavigationdelegate/webviewwebcontentprocessdidterminate(_:)
[a-store]: https://developer.apple.com/documentation/webkit/wkwebsitedatastore
[a-controller]: https://developer.apple.com/documentation/webkit/wkusercontentcontroller
[a-script]: https://developer.apple.com/documentation/webkit/wkuserscript
[a-message]: https://developer.apple.com/documentation/webkit/wkscriptmessagehandler
[a-world]: https://developer.apple.com/documentation/webkit/wkcontentworld
[a-eval]: https://developer.apple.com/documentation/webkit/wkwebview/evaluatejavascript(_:in:in:completionhandler:)
[a-async]: https://developer.apple.com/documentation/webkit/wkwebview/callasyncjavascript(_:arguments:in:in:completionhandler:)
[a-snapshot]: https://developer.apple.com/documentation/webkit/wkwebview/takesnapshot(with:completionhandler:)
[k-15]: https://webkit.org/blog/11989/new-webkit-features-in-safari-15/
[k-164]: https://webkit.org/blog/13966/webkit-features-in-safari-16-4/
[k-power]: https://webkit.org/blog/8970/how-web-content-can-affect-power-usage/
[u-browser]: https://docs.unity3d.com/2022.3/Documentation/Manual/webgl-browsercompatibility.html
[u-js]: https://docs.unity3d.com/2022.3/Documentation/Manual/web-interacting-browser-unity-to-js.html
[u-input]: https://docs.unity.com/en-us/engine/6000.0/manual/platform-specific/webgl/develop/input
[u-limits]: https://docs.unity.com/en-us/engine/6000.0/manual/platform-specific/webgl/intro/technical-overview
[u-deploy]: https://docs.unity.com/en-us/engine/6000.0/manual/platform-specific/webgl/building-distribution/deploying
[t-page]: https://tr-play.playall.in.th/
[t-loader]: https://tr-play.playall.in.th/Build/877598568ac7f0dc41389b0d6e411b8b.loader.js
[t-data]: https://tr-play.playall.in.th/Build/9b2286992122d3c1cc081492dc31fee5.data.gz
[t-framework]: https://tr-play.playall.in.th/Build/4937d6d47bade23f56e5ad4b5bf2e2b9.framework.js.gz
[t-wasm]: https://tr-play.playall.in.th/Build/657890b2bd8548390dd7c7fad76093be.wasm.gz
