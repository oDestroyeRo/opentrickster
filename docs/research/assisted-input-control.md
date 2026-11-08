# Assisted normal-input control

This answers the bounded public-source task in [#28][I28] for build **8502ebbf2fc9033d2fba50d02fd92002**. The current client supports normal left-click movement and monster attacks, a configurable Dig hotkey followed by normal left-button holds/releases, and right-click dig cancellation. Its input coordinates are camera-relative map pixels scaled by the stage's world zoom. Ordinary `movementMode` is **1 walking / 2 running**, derived from `RunToggle` and the actual weight predicate.

The source supplies an implementable input and observation boundary. It does **not** prove native event delivery, a live hit, server acceptance, or quiescence in a running account. Those require the bounded application check described below. No target code was executed. No account, gameplay session, private capture, credential file, or native probe output was accessed. This report is the only repository change. Decoder/disassembler/checker work stayed outside Git.

Three constraints materially affect implementation:

- In the Custom skin, the Dig key defaults to **semi-automatic** behavior. Manual mode and the live binding must be known before pressing it.
- Mouse-up does not clear the client's queued movement destination. An own movement echo can send that destination afterward.
- Normal combat's local cancel methods send no stop packet. A source-backed attack end and attributable result remain distinct. Pause drains the current action; an unprovable drain closes the adapter.

## Current public evidence

Cookie-free `curl` requests to [page][PAGE], [config][CONFIG], and [version][VERSION] succeeded during this investigation and were rechecked on 2026-11-08 at 19:33:05–19:33:18 UTC. Config/version still named the target build. The page still referenced the pinned WASM/data assets. Cached asset hashes establish coherence with that page, not a fresh download of every large asset. A separate Python HTTP attempt returned 403; the successful curl readback, rather than that failed attempt, is the deployment evidence.

| Decoded artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| [WASM][W] | 43136343 | `381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404` |
| [Unity data][D] | 27724487 | `fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474` |
| `global-metadata.dat`, data offset 16748911 | 9343896 | `7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade` |
| [Framework JS][F] | 391375 | `d159059282768082bbfc54432ab93ad68f6eaa57f84883c9e09780810597d917` |
| [Loader JS][L] | 27914 | `5a4ba6acdfc6ad420a285cc1faa54dc694892655bedd752e0f3f7dde3bae6a41` |
| [Page HTML][PAGE] | 37508 | `ccb9859c7a626b0d5e419d7ebe2a1a30b652293be27c70773fc6793ac6c4101d` |
| [Config][CONFIG] | 124 | `084900013e6edde0ce0eca286dc7ac884a86da2ece1bae361ce0671852092602` |
| [Version][VERSION] | 44 | `9ce11c71ef039ffbe6bede54dfcf5b27afb887d44b2a79f57f959638e2e771b0` |

The approved `.gz` cache files already contained decoded bytes; magic and hashes were checked before use. Metadata is format 108, methods are 30-byte rows (`nameU32@0`, `typeU16@4`, `tokenU32@18`), types 66-byte rows, fields 10-byte rows. The WASM has 490 imported functions. Join a method token's RID through its module's pointer table to an element slot and then a function. Field-offset pointers are at memory `0x35d850`.

| Module | Registration | Method count | Pointer table |
| --- | --- | ---: | --- |
| Spinel.Game | `0x21251c` | 18796 | `0x37f5a0` |
| Spinel.Net | `0x212548` | 4791 | `0x392c70` |
| Spinel.UI.Pc | `0x2125a0` | 16606 | `0x3a9600` |
| Spinel.UI.Custom | `0x212574` | 17533 | `0x3979f0` |
| Spinel.Engine.Unity | `0x212120` | 1934 | `0x37cf10` |
| Spinel.Formats | `0x21214c` | 324 | `0x37f050` |

All `fNN@offset` locators below refer to original body offsets in [W], joined to metadata in [D]. Field numbers are byte offsets from the object pointer unless explicitly called static or inline. Property backing-field names are abbreviated in prose. The appended checker independently verifies representative joins, body hashes, fields and literals. WABT 1.0.39 statically disassembled unchanged selected bodies in a copy whose other bodies were stubbed; no module was instantiated. The decompiler omits exception-handling bodies, so original disassembly was used for `Advance`, `Install`, dictionary iteration and key dispatch. Do not treat decompiler `Try(); unreachable` as actual missing gameplay behavior.

The immutable [protocol revalidation][R18], [state completion][R14], and [manual dig][R25] reports were leads. Their claimed paths relevant here were checked against these current bytes. The separately published [gun report][R29] owns gun-specific outcomes and pending-shot semantics.

## Normal controls and dispatch

`GameInput..ctor f36172@b02fd4` installs these Input System actions:

| Action | Actual binding | Use here |
| --- | --- | --- |
| Point | `<Pointer>/position` | Pointer location used by world picking |
| Click | `<Mouse>/leftButton`, also primary touch press | Movement, monster click, manual gauge button |
| RightClick | `<Mouse>/rightButton` | World button code **2**, including drill cancellation |
| MiddleClick / Scroll | `<Mouse>/middleButton` / `<Mouse>/scroll` | Camera/layout interference; do not use for actions |
| Cancel | `<Keyboard>/escape` | Release callback into layered cancel handling; not a universal stop |
| LeftShift | `<Keyboard>/leftShift` | Changes world-press behavior; require an unmodified baseline |
| Submit / navigation | Enter, numpad Enter, Tab, arrows | UI controls, not a proven walking binding |

`SpinelEngine.Update f36244` polls `GameInput.Poll f36165`. `GameplayScreen.Advance f38537@bfbf6d` tests left/right `WasPressedThisFrame` at `bfc51a`/`bfc527`, maps the pointer, and invokes `PressWalks f38407@bf06e5`. A left press becomes world button **0**; a right press becomes **2**. `PressTaken f38409@bf0c3a` runs ordered world handlers. If unconsumed, `ClickAt f38408@bf0b22` tests portal regions and then calls `ClickWalk f38396@bef827`. A portal, item, actor, modal UI, selected skill, Shift, or an armed drill can consume the same physical click. An arbitrary coordinate is not therefore a walking command.

`MapPoint f38535@bfbce0` checks the stage bounds and UI raycasts. Its `overUi` cache can survive 0.1 seconds at the same pointer; a page/canvas rectangle alone does not prove a click reaches the world. The host must establish game focus and an unobstructed world point, including Unity UI, not merely DOM visibility. Chat/text capture, message boxes, keyboard capture, context menus and game UI mode can consume keys (`Hud.OnKey f48485@10065e0`, `Hud.KeyPasses f48461@100510a`). Preserve unknown routing as unavailable.

For a bounded movement, use a normal pointer move, left-down, then left-up after the client has sampled the press. Do not depend on two native events delivered within one application turn being sampled by Unity. The held-movement branch accumulates milliseconds and resamples when **heldTime > 300**, at `bfca2b`; release before repetition and observe the real outgoing request. Source timing is not a guaranteed native click duration. Release clears `GameplayScreen.held` at `bfcbc2..bfcbce`; it does not retract a request already sent or queued.

### Actual key map

`HotkeyMap.MakeCatalog f22559@6e862d` declares default **Run = KeyR** and **Dig = KeyD**. `Attach`, `Rebuild f22567@6ea1e6`, and `Of f22573@6ec6e2` allow overrides. Runtime `Hud.Keys+80` points to the map; its `Bindings+20` is the resulting dictionary from action name to a list of `KeyBinding` objects. Read only the whitelisted `Run` and `Dig` entries. No account preference files are needed for this observation path.

`KeyBinding` fields are `Code+8`, `Modifier+12`, `Exclusive+16`. `KeyBindings.Matches f20985@66da4e` compares the code; modifier **1** requires Shift, **4** Ctrl, **7** Alt. In its other branch, `Exclusive=true` rejects any of those modifiers. The MVP can require an unmodified, nonconflicting binding and report unavailable if it cannot prove one. Do not replace an unknown map with D/R defaults. `Hud.OnKey` matches the current map before invoking registered handlers; its EH region at `1006a74..1006ab1` reads `Bindings+20` and calls `KeyBindings.Matches`.

`Install f44728@e99a19` registers action `Dig` at `e9aeed`. Its callback is **f44716@e98766**: warm the drill UI, then `DigAssist.DigKey(assist, UiSkins.Current==1, DigKeySettings.SemiAuto)`. `get_SemiAuto f44648@e92965` caches a nullable boolean and otherwise reads preference `spinel.custom.digKeySemiAuto` with default **1**. `DigKey f21298@68737f` routes Custom+SemiAuto to the helper's Start/Stop, also handling an already-active helper or qualifying automatic pet. The normal branch calls `DrillFeature.PressDig`. Consequently manual operation requires the actual skin/setting, inactive helper and no automatic pet/mode path. A normal settings-UI change can establish manual mode in a later application check; a direct preference write is not this contract.

`CancelRouter.Cancel f30954@993ce8` tries layered handlers. Escape may close a window instead of ending an action. Do not prescribe repeated Escape as a source-proven idle operation.

## Exact ordinary movement mode

The chain is `GameplayScreen.get_MoveMode f38391@beee3c` → `screen.run+200` → `RunToggle.get_Mode f19265@5c7e23`:

```text
On != true                            => 1
On == true and Heavy delegate absent  => 2
On == true and Heavy() == true        => 1
On == true and Heavy() == false       => 2
```

RunToggle stores `On+8`, saved choice `+9`, `Heavy+20`, `Ready+24`. Its default/restored value comes from settings; missing observed state is not permission to assume running. GameplayScreen.Enter f48894@102fff7 registers `Run` at `10342a8..10342ce`, via method-usage cell `0x33cfb8` and callback f38352@beb074, which calls `RunToggle.Flip f19264@5c7dcb`. Frame logic may also turn running off when heavy.

`RunFeature.Install f38675@c0d14a` installs Heavy at `c0d8b3..c0d8c5`, via callback **f38671@c0cc2d**. It uses current carried weight, status maximum weight and actor attributes. `ItemWeight.Heavy f22678@6f3fbd` computes, with source i32 arithmetic:

```text
(attrs & 17) != 0
and trunc_i32(maxWeight) > 0
and (i32(low32(carried)) * 100) / trunc_i32(maxWeight) > 89
```

Division truncates; multiply wraps as i32. The callback has a client-local fallback attribute value when its service is absent; that does not turn an unknown native weight/attribute observation into known resources. Prefer actual current input state plus the observer's emitted mode. A prior packet's mode is stale after R, equipment/status/weight changes, or a new session.

The same mode reaches manual dig: `DrillFeature.moveMode+44` is provided by callback **f44705@e97884**, method-usage cell `0x33bfc8`, supplied at `e9a2c6..e9a330` to `CreateAsync f21367`, then stored by ctor `f21366@68be09`. It invokes **IPlayContext slot 34, get_MoveMode**, hence the same GameplayScreen getter. Mode **2 is not AutoMode**.

`RequestMove f13281@3af124` rounds via `JsMath.Round f9994`, rejects movement within the same 16-pixel cell, applies movement refusal, and may retain a nearby destination until another echo. `SendMove f13282@3af3d2` emits logical **24**, payload **9 bytes**: U16 rounded fromX/fromY, U16 toX/toY, U8 mode (`3af467`). Observe that actual send; a click with no send is not success. Dig's `Press` instead truncates f64 X/Y before its U16 writer. Do not unify these coordinate conversions by guessing.

## Projection and source-observable geometry

The art is isometric, but the normal input path does **not** apply an additional diamond-grid rotation. `ScreenToMap` divides host-local screen coordinates by world zoom, and `PressTaken`/`Advance` add camera X/Y. The same map-pixel X/Y are stored on `WorldActor` and serialized. `FollowServerMove f38486@bf6cfa` converts received integer X/Y directly into `Point`, then calls `WorldActor.FollowMove`; mode==2 becomes Running. This is the relevant world/wire coordinate space, not a guessed tile transform.

Define `R = canvas.getBoundingClientRect()` in CSS viewport pixels; backing size `(Bw,Bh) = (canvas.width,canvas.height)`; `rx=Bw/R.width`, `ry=Bh/R.height`. Let `(sx,sy)` be Unity screen coordinates with bottom-left origin, `H=(hx,hy,hw,hh)` the stage's host rectangle, `(cx,cy)` the actual camera offsets, and `z` the stage's WorldZoom. Under the MVP gate of no keyboard pan and no host override:

```text
sx = (clientX - R.left) * rx
sy = Bh - (clientY - R.top) * ry
worldX = cx + (sx - hx) / z
worldY = cy + (hy + hh - sy) / z

clientX = R.left + (hx + (worldX - cx) * z) / rx
clientY = R.top + (Bh - hy - hh + (worldY - cy) * z) / ry
```

`Stage.ScreenToHost f36378@b15064` supplies the subtraction/Y flip; `ScreenToMap f36382@b1541e` divides f32 values by f32-demoted WorldZoom. `MapToScreen f36383@b15482` establishes the inverse orientation. Nonzero effective `panPixels` adds to the ScreenToHost Y result; its inverse subtracts that pan from CSS Y's numerator. HostOverride/UnderViewport alter whether pan applies. Reject those cases until explicitly supported; do not silently assume zero. MapToScreen itself omits this pan.

The first CSS-to-Unity conversion is the delivery normalization the host must verify. The framework's `fillMouseEventData` at byte 254501 records DOM client coordinates and target-relative coordinates, and its canvas-size function reads the actual backing dimensions. The native/WK event route, page zoom and Unity's final input scaling were not executed here. The stage-to-world formula is directly established by the compiled methods; the complete native→DOM→Unity chain still needs runtime correspondence at known points.

The page chooses screen sizes 1280×720, 1600×900, 1920×1080 or window, fits and centers them, and rounds dimensions to device pixels. Its Saver/Sharp/Max render ratios differ; Sharp clamps DPR to [1,2], and a query ratio may override them. UI zoom is a separate fit against a 1024×768 layout. **DPR, canvas ratio, UI scale, world zoom and native backing scale are distinct.** Read actual dimensions and Stage.Current.WorldZoom; never multiply by DPR twice. Require finite positive dimensions/zoom and a point inside the known map and unobstructed stage, with integer wire coordinates representable as U16. Reject missing geometry and wrapping/out-of-map destinations.

`GameplayScreen.UpdateCamera f38538@bfd8a8` follows and clamps around the avatar, then includes camera effects/manual pan. `CameraAxis f38533@bfbab9` uses rounded actor position, half viewport size and map bounds; small maps center with FloorDiv. At map edges the player is not centered. Use `screen.cameraX+452` and `cameraY+456` (signed integers), not a player-centered inference. Sample camera and layout together, and invalidate projected points when either changes.

For picking a monster, `ActorAt f38406@bf03f7` hit-tests avatar and actors, uses `ObjectPick.Pick f14032@3e82bf` with targeting priorities, and returns null if the winning actor is the avatar. `WorldActor.HitTest f13684@3c7e9e` rejects Unseen/Hidden/Limited or nonpositive hit width. Its half-open map rectangle is `[X+HitLeft, X+HitLeft+HitWidth)` × `[Y+HitTop, Y+HitTop+HitHeight)`. Choose an interior point with a margin, not the actor's feet. Reject overlap with any other selectable hit, including the avatar, unless the exact pick priority is modeled. Reject not-yet-loaded art/hit bounds, fading/dead targets, UI occlusion and stale actor stamps. Layout snapshots must include current hit rectangles, not just actor position.

## A source-backed private observation path

There is no demonstrated exported gameplay command API. `GameplayScreen.ClickActor`, `ClickPoint` and other methods found in metadata are internal; their existence does not authorize guessed SendMessage strings or calling them through WASM tables. `WebShell.StateOf f32989@a3e4dc` publishes screen/UI settings only.

There **is** a build-specific read-only memory path. The freshly fetched loader returns `{Module:m,...}` at byte 3329. Page line 606 assigns `window.spinelInstance = unity`. Framework `updateMemoryViews` exports `Module.HEAPU32` and `Module.HEAPF64` (byte 9738 and following). It does not export HEAPU8. A production observer can create a fresh DataView from `window.spinelInstance.Module.HEAPU32.buffer`, reading a strict whitelist without invoking any game method or writing memory. Reacquire the buffer after growth. Attachment, pointer lifetime and sampling consistency require runtime validation.

TypeInfo usage cells below initially contain tagged metadata usages, **not pointers**. Only read through them after the client itself initialized them. Check build, memory bounds/alignment, resolved class identity and all root epochs; never invoke a metadata initializer to make an absent value appear.

| Source root / allowed type | Type index | Usage cell | Read path or purpose |
| --- | ---: | --- | --- |
| GameFlow | 2619 | `0x318aa4` | Resolved class → staticFields at +76 → Current at +4; getter f38245@be6202 |
| GameplayScreen | 2667 | `0x318adc` | Flow.Screen+84 must have this class; Flow.Engine+16 must match engine root |
| SpinelEngine | 13662 | `0x31ad74` | Class → staticFields+76 → Instance+0; f36241@b09964; engine.Stage+20, Input+28 |
| Stage (Engine.Unity) | 13683 | `0x31ae00` | HostRect+56, Current inline+136, hostOverride+44, panPixels+280 |
| RunToggle | 7915 | `0x31a884` | GameplayScreen.run+200 |
| WorldActor | 5465 | `0x31b6d8` | Avatar+236; other actors through screen.actors+244 → ActorView.Actor+96 |
| CombatModel | 6101 | `0x317c74` | PlayServices value; Target+60, Fsm+112, attackTargets+104 |
| DrillFeature | 6256 | `0x31808c` | PlayServices value; fsm+24, Flags+60, inDrillState+84, param+96 |
| DigAssist | 6215 | `0x318018` | PlayServices value; State+48 and linked drill+8 |
| AvatarFsm | 5807 | `0x317748` | State+8; check shared FSM identity from feature roots |
| HotkeyMap | 6741 | `0x318fcc` | GameplayScreen.hud+324 → Hud.Keys+80 → Bindings+20 |
| UiSkins / DigKeySettings | 2483 / 3183 | `0x31b460` / `0x318020` | Static current skin and cached nullable SemiAuto; no preference-file access |

The ordinary-world baseline requires screen `ready+400=true`, `changingZone+401=false`, `loggingOut+402=false`, `localScene+403=false`, `campScene+404=false`, and no covered/loading world (`worldCovered+600`, `artAwaited+601`). Inspect actual UI routing/occlusion separately. These fields do not replace Net's fresh session/own-actor baseline.

`Stage.Current` embeds an unboxed StageLayout: UiScale double+136, WorldZoom double+144, PxPerDp double+152, viewport dimensions i32+176/+180, world dimensions i32+184/+188. Metadata's boxed value-type offsets include an eight-byte object header; subtract it when mapping this inline value. Do not apply object offsets unchanged to inline structs or nullable values.

`PlayServices.items+8` is a dictionary. `Get`'s shared body **f65640@16fad29** calls `TryGetValue f64765@16bd34f`, concrete MethodInfo usage **0x325c6c**. Once that MethodInfo has been initialized, `rgctx=U32(methodInfo+28)` supplies the specialized entry layout: hash offset `rgctx[9]`, key offset `[11]`, value offset `[13]`. Entries pointer is dictionary+12, allocated count +16, version +28; array length +12, data +16, stride `U32(array.class+140)`. Bound iteration by validated count/length, skip deleted entries whose signed hash is -1 (`Remove f64752`), and inspect only whitelisted value class pointers. Never assume generic field offsets from all-zero generic metadata tables. Reject an unresolved MethodInfo/RGCTX, invalid stride, changed version/root, duplicate service or missing expected class. `Install f44728` registers DrillFeature at `e9a4b7` and DigAssist at `e9acad`.

Other dictionaries need **their own initialized specialization**, not the service dictionary's offsets. For example, `FollowServerMove` uses MethodInfo usage 3333612 for the screen actor dictionary, then reads ActorView.Actor+96. Hotkey lookup uses usage 3298216 (`KeyBindings.Matches`); its values are binding lists. For lists, the source uses items+8, count+12, version+16, and array elements at +16; validate the element class/stride before reading. Nullable fields must use their initialized generic field layout. The SemiAuto getter itself identifies its nullable value/HasValue accessors at usages 3331940/3331948. Absence is unknown, not false.

Only scalar facts needed for input planning and drain cross into the private host contract: typed booleans/stages, opaque mapped entity references, geometry, capability/binding facts and monotonic observation marks. Do not copy the heap, enumerate arbitrary strings, or read Flow.Login/Session/Ticket/Transport/account stores. Raw actor identifiers remain within Net's identity mapping; host/UI diagnostics contain action keys and sanitized outcome classes, not raw IDs, packets, pointers or full snapshots. This extends the existing passive observation boundary; it does not create a second WebSocket wrapper or public schema.

## Selecting and attacking one monster

For a normal melee basic attack, reserve one action, validate a live type-3 monster and an unambiguous current hit rectangle, project an interior point and issue one normal unmodified left click. `PressTaken` selects a selectable actor through `SelectedTarget.Select f19302` and dispatches WorldClick. Selection alone is not proof of attack.

`CombatModel.WorldClick f17422@53dfb7` requires button0, a non-fainted player, actor type3 and a non-fading target. It calls **GunClick first**. If that returns true, no normal Attack follows. For ordinary melee it stores Target, invokes StopWalking/AttackOrdered, and calls **CombatLink.Attack f13224@3ab9b0**. This emits **288: U16 target**, then **29: U16 target + U8 mode**. Match that actual outgoing pair, exact target and current action/session ordering; do not report Sent because native events were posted. Preserve Unity's own framing, byte accounting and send path.

If a gun is worn, the left click enters the separate [#29 contract][R29]. Its early-return/no-melee-fallback behavior means a silent gun click is not a melee submission. A melee-only implementation must prove the gun branch is inactive. A gun-capable implementation retains its own pending shot correlation and resource/cooldown prerequisites. Do not infer either branch from an icon or a missing packet.

A target's Damage42 or Faint43 alone does not attribute the result to this action. Net must use a complete same-Aux Strike (31/485/1438 header with attacker, kind, target and associated hits), original ingress mark and matching own actor/target. Own AttackEnd32 means the actor's attack state ended; it carries no target result. Deduplicate results separately from action submission. An old buffered result cannot complete a later click.

## Full manual dig using ordinary input

The host drives the game's existing manual simulation. It does not simulate wire packets, substitute drill parameters or call AutoDrill/DrillFeature methods.

1. Establish an idle own actor, real equipped usable drill, known allowed map/spot, known manual Dig binding/setting, `AutoMode=false`, `AutoFlagOn=false`, `Crazy=false` and inactive DigAssist. Press/release the actual Dig hotkey. Observe targeting flag4. If the key instead starts a helper, fences invalidate the action; do not continue as if it were manual.
2. Project a valid chosen ground point outside UI, portals and competing actors. A normal left press reaches **DrillView.Click f44743@e9c892** → **DrillFeature.Press f21297@687211**. With FSM0…2 and flag4 it sets pending bit8 and sends **44: U16 X,Y,U8 mode**. Release the selecting click. Correlate the actual send; no fabricated request ID exists.
3. Await **45** and own **371** through the existing observer. OnAccepted enters FSM5. `OnFsm f21397@68e243` initializes stage1 Waiting and clears old spot/gauge/counters. `OnSpot f21400@68e65f` requires own actor and FSM5. The received spot must survive that entry reset. `AvatarUpdate f21403@68ed6f` waits for information and a nonmoving avatar before Begin.
4. `Begin f21401@68e740` rejects Unusable>0 locally. Odd key plus nonpositive item depth sends48 and stops before normal gauge progression. Otherwise it installs the real spot depths/key/stress and reaches stage2 Idle. Never force a fixture spot, tool speed, area, scoop mass or empty result into this branch.
5. Drive the left button at the same currently valid world-input location. During FSM5, Press only sets **Held+152=true**; `Release f21379@68d77f` only clears it. DrillView remembers a successful press and, in **Update f44745@e9cde7**, calls Release once Input.Click is no longer pressed. Hold through sampling; do not re-click after the FSM has exited or a layout fence changed.
6. Use observed gauge/stage to decide the next button state. The current manual-compatible policy is evidenced by **AutoDrill.Tick f27438** and its **AutoHost f14129–14132** accessors: stage2 begins holding; observe stage3; continue holding until stage5, then release. Subsequently hold when `GaugePercent <= MarkerPercent+10`, otherwise release. Stage7 releases and waits. The auto host's button codes 3/0/2 mean held/released/released; normal mouse state implements the same boolean without enabling automatic mode.
7. Observe the client's own **49: U16 starts,elapsedMs,depth,checksum**, then **50: U8 kind,U16 value,U32 itemOrValue**. Stage7 is a completion request state, not success. A refusal/result50 can arrive before49; handle it without waiting for a send that will never occur. Kinds2/9 are Nothing feedback,3 item feedback. Inventory goals require an actual attributable inventory delta, not feedback text or a ground item.
8. Release every owned control, drain the result and verify cleanup. `EndLocal f21304@687d58` sets FSM0, clears flags, then calls PressDig and can **re-arm targeting**. Clear an observed remaining targeting flag through normal world right-click and verify it cleared. Do not issue a blind second click once targeting is absent.

For the helper gate require `DigAssist.State+48 == 0`. `get_Running f21280@686494` accepts states1/2, whereas `get_Active f21285@686637` only tests state1; Active=false alone is insufficient. `get_Busy f21283@6865ba` also checks the linked drill FSM, flags and automatic controller.

Gauge f32 is +104, stage i32 +100, current param pointer+96. `get_ProgressPercent f21316@688ee6` is `f32(f32(gauge*100)/200)`; `get_MarkerPercent f21315@688eb3` is `f32((200-param.BarEffArea)*100.0/200)` with parameter BarEffArea i32+28. Missing param is unavailable even though the game's display getter has a fallback. `GameplayScreen.Update f38536@bfbf51` passes unscaledDeltaTime×1000 to Advance. `DrillFeature.Update f21402@68ed34` truncates milliseconds for the simulation. There is no proven fixed 16ms interval or guaranteed latency budget for native event delivery.

The [#25 report][R25] supplies the full gauge stages, checksum, 371 branch widths and item-definition contract. Here its normal control bridge and completion path were rechecked against current Press/Release/OnFsm/Begin/AvatarUpdate/Result and AutoHost/Tick. Use actual client state for assisted timing; keep its supplied synthetic parameters out of live state.

### Cancellation and actual quiescence

| Operation/evidence | Established effect | What it does not establish |
| --- | --- | --- |
| Mouse/key up | Releases owned native control; later client sampling can clear held input | Server idle or removal of queued destination |
| GameplayScreen.StopWalking f38441@bf1fb2 | Clears local route+372 | A movement-stop send |
| AttackCancelled f17362@53735f | Conditionally FSM2→0, clears Target | A cancel packet or rollback |
| OnFsmLeft f17363@537383 | Leaving2 clears Target and emits AttackStateLeft | Remote stop acknowledgement |
| UnhandledClick f17424@53e3f8 | Button0 clears Target | Attack end, or an innocuous “cancel click”; it can also walk |
| Incoming32, U16 actor | ApplyMessage at `53b06e..53b0dc`: own actor conditionally FSM2→0 and Target=null; removes that actor's attackTargets entry | Target outcome, damage attribution, or undo of pending477 |
| Right world click while drilling | RightClick f21291@68691a: stage7→EndLocal, other active stage→EndDig(true)→48 with actual counters | Guaranteed server cancellation, or immediate FSM0 |
| Right click while targeting | Clears observed targeting through Cancel f21292@6869cc | Cancellation of a separately pending send |
| Incoming46 callback f14168@3ee546 | Clears Flags bits0x0c | FSM exit or correlated cancel acknowledgement |
| Result50 / resultMark255 | Applies result and local end behavior; delayed cleanup may follow | Inventory receipt or proof that targeting stayed off |

`GameSession.targetPending+88` is crucial. **OwnMoveEchoed f13291@3af746** and **ApplyMove f13297@3b085d** send another Move when that flag is true; otherwise they arm the next send. **Handle f13296@3afad2**, at `3b00e9..3b0189`, holds own logical25 at `heldStop+132` and queues `ApplyHeldStop f13298@3b0a5d` as a microtask. Receipt of the packet is earlier than its application. Movement quiescence must therefore include the local queue and deferred application, not only a terminal-looking incoming packet.

A sufficient conservative idle barrier combines all of the following under the same action/session/page epochs:

- Host input queue empty; no owned buttons/keys down; no unsampled release; no physical-input ambiguity; controller admits no new action.
- Fresh local actor path empty: `WorldActor.get_Moving f13655@3c4fe7` is exactly `I32(U32(actor+12)+12) > 0`: the path list has at least one point. Also no screen held gesture, route or steering; require own FSM0 or1 for this conservative ordinary baseline. `AvatarFsm.Free f13600@3c1f69` accepts unsigned states below3, including attacking2, so Free alone is not idle.
- Session `targetPending=false`, `heldStop=null`, `sendArmed=true`; no unresolved sent move/deferred own-move evidence. An echo that flushed a queued destination requires that new send to drain too. A finite quiet timeout is not a substitute.
- Combat own FSM no longer attacking, Target absent, own attackTargets entry absent, and the controller has resolved/drained the attributable pending attack. Old AttackEnd cannot satisfy a new action. For guns, retain [#29][R29]'s pending-shot conditions even if the gauge resets or hides. Other actors' animation timers need not be globally empty.
- Drill FSM not5, `inDrillState+84=false`, `Flags & 0x0c == 0`, `Held=false`, no active automatic/crazy/helper state, and no unmatched dig result/request. **OnFsm's exit retains State+100**; demanding stage0 would reject valid cleanup, while stage7 alone would accept active work. EndLocal's re-arm must be observed and cleared.
- Net's original ingress/send marks and pending ledger agree with the local snapshot after its client update/deferred work has applied. Unknown state, stale roots, missing result or ambiguous manual interference prevents the barrier.

These are sufficient implementation gates, not a new server guarantee. No source-backed generic cancel-and-ack operation was found. An early48 or local combat reset does not prove the server undid work. If drain cannot satisfy the combined predicate by its deadline, close the adapter. `EngineClosed` requires actual socket-close evidence **and** Desktop's owned-input neutralization and WK resource detachment. Observer-local `Closed` alone is insufficient. A tagged barrier for an older action must never clear a later action.

## Private host contract

This follows [architecture.md](../architecture.md) and [DESIGN.md](../../DESIGN.md). It adds source requirements to the existing Engine/Net/Desktop boundary; names below describe private data, not a competing public API or observer schema.

| Boundary | Required data/behavior |
| --- | --- |
| Observation snapshot | Supported build; local monotonic receive time and original observer mark; page/session/world identity; geometry revision and input epoch; readiness/focus/UI-routing facts; camera/stage/canvas geometry; typed own controller state; opaque target identity+stamp+hit bounds; actual binding/manual-mode/movement-mode knowledge. Missing facts remain Unknown. |
| Reserved action | Unique local action key, exact action kind/target or point, expected current state, earliest observer mark, expiry, bounded input program. At most one logical action owns gameplay input. Manual dig's many button changes remain one logical action. |
| Gesture fence | Page instance, session key, layout/camera snapshot revision, physical-input epoch, action key, monotonic expiry and ownership token. Check before every queued event, hold transition and callback, not only when queued. A page navigation, process recreation, zone/session replacement, resize, panel toggle, DPR/render/world-zoom change, camera shift or expired snapshot invalidates projected gestures. |
| Normal dispatch | Desktop owns the WKWebView child and normal native mouse/keyboard delivery. Convert CSS viewport coordinates through the actual child-view/native coordinate system. The game receives ordinary input and retains all transport/accounting. No raw socket injection, object writes, guessed SendMessage methods or WASM calls. |
| Receipts | NotSent means no event escaped; Submitted means native delivery began; Sent requires Net's actual matching outgoing game message after the reservation/input mark. Uncertain means attribution/delivery cannot be established. Never replay the same action key. For melee, match the entire 288+29 pair; for dig, match 44 then follow its lifecycle. |
| Physical input | At the native dispatch boundary, advance the physical-input epoch and fence queued automation **before** forwarding a genuine gameplay event. Quarantine any action whose attribution may overlap, then notify Engine to pause. Recognize owned generated events with private native ownership, not merely DOM `isTrusted`. Track physical and owned downs separately. A physical takeover while an owned button is down is ambiguous and requires neutralization/drain or closure, not a forged release of a physical press. |
| Focus/menu/teardown | Focus loss, native menu/sheet, page replacement, memory/root invalidation and close stop admitting gestures. Release only owned controls with their ownership token; invalidating an action must not discard the releases needed to neutralize it. Apply menu/button commands through the single desktop controller. |
| Pause/Stop | Invalidate future action gestures; permit only the current action's explicitly fenced drain/neutralization. Publish a tagged idle barrier only after the combined predicate above. Timeout closes and awaits actual transport+host cleanup gates. Resume requires a fresh baseline and verified idle. |
| Privacy | Keep raw IDs, memory pointers, bindings/settings and packet details in the necessary private adapter scope. UI diagnostics use reason classes and local action/session capabilities; no raw IDs, keys, tickets, packet bodies, whole snapshots or account-store values. |

A snapshot that changes camera while a native click is queued cannot be fixed by blindly reprojecting and retrying under the old action key: the first click may have escaped. Return NotSent only if it demonstrably did not; otherwise Uncertain and drain. Physical input during a pending shot or dig result likewise invalidates exclusive attribution even if the next outgoing payload happens to match.

## Minimal examples and verification boundary

Invented projection fixture: CSS rectangle left27.5/top41.25/960×540; backing1920×1350; host x8/y10/height1310; zoom1.25; camera320/96. World500/300 projects to CSS **144/155.25** and round-trips exactly in the algebraic checker. The deliberately unequal backing ratios expose a hidden DPR-only assumption. This is not an observed game layout.

Invented matching cases: a submitted melee click followed by old same-target evidence is still pending; a newer complete own same-Aux Strike can resolve its outcome; own32 ends its attack state but cannot by itself count a kill. A queued move followed by mouse-up remains active if targetPending is true. A result50 followed by FSM0 and flag4 remains armed, not quiescent. These examples explain the contract and are not live fixtures.

Passed here: original asset magic/hashes and embedded metadata identity; fresh public deployment/asset-reference readback; metadata→module→element→body joins; 28 body hashes and selected fields/literals in the self-contained checker; static instruction inspection for dispatch, mode, projection, dig progression/cancel and queued movement; algebraic projection round-trip and four mode branches. A 71-method locator/hash ledger and all selected disassemblies remain in scratch. Source publication checks include report readback and whitespace.

Still required in the application, with a later expressly authorized bounded gameplay session:

1. Validate the build-pinned read-only roots, generic layouts, field lifetime, memory growth and post-update sampling. Verify only whitelisted facts leave the observer. The source path is proved; a live observer installation is unrun.
2. Prove native mouse/key down/up reaches the embedded WK child and Unity once, with no unintended hold repeat. Check keyboard layout, focus, Command shortcuts, Escape, menus and physical-input arbitration. No synthetic DOM dispatch is presumed equivalent.
3. Match native→CSS→Unity→world coordinates at known points, including resized/panel-hidden/DPR/render-ratio/world-zoom/map-edge cases. Reject pan/override/modal/overlap cases until supported. Verify layout races do not leave held input.
4. Perform one bounded move and attributable monster attack with the actual outgoing payload, subsequent own state and result; perform one full manual dig with the real setting/tool/spot, observed44/45/371/49/50 and cleanup. A gun-equipped check follows #29 separately. No fixture values may substitute for missing parameters/resources.
5. Exercise Pause/Stop before send, during queued movement, while gauge-held, waiting for result and after re-arming; verify the tagged drain predicate or actual socket-close plus neutralize/detach fallback. Late Aux observations, own32/46 without outcomes, focus loss and physical interference must not produce false completion.

These are runtime prerequisites for the implementation tickets, not unfinished account-free decoding. No server-side cancellation guarantee or elapsed-time-only idle proof is claimed.

## Reproduction lever

Save the following as an outside-Git `check-input.py` and run it with the approved WASM, data and metadata paths, in that order. It detects gzip by magic and never executes target code. Use ordinary Python with assertions enabled. Full helper locations from this investigation are `/private/tmp/opentrickster-input28-qz1tyivn/{decode.py,decompile.py,check-input.py,method-proof.tsv}`. The scratch directory also contains pinned public copies, selected `fNN.dis`/`fNN.c`, fresh HTML/JS and checker output. Only this Markdown report is published.

```sh
python3 /private/tmp/opentrickster-input28-qz1tyivn/check-input.py \
  /private/tmp/opentrickster-input28-qz1tyivn/client.wasm \
  /private/tmp/opentrickster-input28-qz1tyivn/client.data \
  /private/tmp/opentrickster-input28-qz1tyivn/metadata.dat
```

```python
# Static public-artifact checker; never instantiates the WASM module.
import gzip, hashlib, math, struct, sys
from pathlib import Path

def sha(b): return hashlib.sha256(b).hexdigest()
def read(p):
    b = Path(p).read_bytes()
    return gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b
w, data, md = map(read, sys.argv[1:4])
for b, expected in zip((w, data, md), (
    "381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404",
    "fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474",
    "7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade")):
    assert sha(b) == expected
assert w[:8] == b"\0asm\1\0\0\0"
assert data[16748911:16748911+len(md)] == md
u32 = lambda b, p: struct.unpack_from("<I", b, p)[0]
u16 = lambda b, p: struct.unpack_from("<H", b, p)[0]
assert (u32(md, 0), u32(md, 4)) == (0xfab11baf, 108)
def leb(b, p):
    v = shift = 0
    while True:
        x = b[p]; p += 1; v |= (x & 127) << shift; shift += 7
        if x < 128: return v, p
sec = {}; p = 8
while p < len(w):
    kind = w[p]; n, p = leb(w, p+1); sec[kind] = (p, p+n); p += n
imports, p = leb(w, sec[2][0]); assert imports == 490
for _ in range(imports):
    for _ in range(2): n, p = leb(w, p); p += n
    assert w[p] == 0; _, p = leb(w, p+1)
n, p = leb(w, sec[9][0]); assert n == 1
flag, p = leb(w, p); assert flag == 0 and w[p] == 65
start, p = leb(w, p+1); assert w[p] == 11
n, p = leb(w, p+1); elements = [None] * start
for _ in range(n): x, p = leb(w, p); elements.append(x)
mem = bytearray(64*1024*1024)
n, p = leb(w, sec[11][0])
for _ in range(n):
    flag, p = leb(w, p); assert flag == 0 and w[p] == 65
    at, p = leb(w, p+1); assert w[p] == 11
    size, p = leb(w, p+1); mem[at:at+size] = w[p:p+size]; p += size
n, p = leb(w, sec[10][0]); bodies = {}
for f in range(imports, imports+n):
    size, p = leb(w, p); bodies[f] = (p, size); p += size
h = [struct.unpack_from("<III", md, 8+12*i) for i in range(24)]
def name(i):
    p = h[2][0]+i; return md[p:md.index(b"\0", p)].decode()
def fields(t):
    p = h[19][0]+66*t; first = u32(md,p+20); count = u16(md,p+46)
    offsets = u32(mem,0x35d850+4*t)
    return {name(u32(md,h[11][0]+10*(first+i))):u32(mem,offsets+4*i)
            for i in range(count)}
modules = {"G":(0x21251c,18796,0x37f5a0),
           "N":(0x212548,4791,0x392c70),
           "P":(0x2125a0,16606,0x3a9600),
           "U":(0x212120,1934,0x37cf10)}
for at, count, pointers in modules.values():
    assert (u32(mem,at+4),u32(mem,at+8)) == (count,pointers)
# module, metadata row, element, function, body offset, body SHA-256
anchors = [
    ('U', 78606, 2783, 36172, 0xb02fd4, "f5e1fe161a2964eafb921f2a17bc3d1a73236d291838d3e59f5b333142c526bc"),
    ('P', 19330, 47926, 38391, 0xbeee3c, "8fd3d46b3765d929a6709eeef9658d265f1675241a286e26f64eea3716d43b5a"),
    ('G', 47592, 18015, 19265, 0x5c7e23, "155fb56a5bad463b039ed30d2f564faf6de0538aa8d7f57a95c3050f7416e224"),
    ('G', 41675, 12162, 22678, 0x6f3fbd, "a458c71456d5d5e05f0d3acfa8ae0928a7c66dfbfa24dff92f8f105430aeb840"),
    ('U', 78899, 3061, 36378, 0xb15064, "b142686e156061172344171c39a4b8ac1ea78af7451c494d724b5ef9e20418fb"),
    ('U', 78903, 3065, 36382, 0xb1541e, "363911e17f95a6eb60fa7854a570b8968ec9f7fa3c1a0a7aaddf9e5b3e3952d4"),
    ('U', 78904, 3066, 36383, 0xb15482, "7fdd9c7a1d6bc183cfd2cb7f5ef6793aaba4a0a05ac75eb42f0fccb835486794"),
    ('P', 19447, 48043, 38535, 0xbfbce0, "812542a858aa50b59602ed11333295ea7f4233aa97396bb1d4ca0f12dbeba07d"),
    ('P', 19462, 48058, 38409, 0xbf0c3a, "e97e3e8a027a94c7c5f04817014388a4a3421abf3ba4fc9b1329141bffcaf377"),
    ('G', 37914, 8417, 17422, 0x53dfb7, "1dfd55d4940e5af040acede7aef71ab7e48aa394d49c19912be61d9109ab5744"),
    ('N', 71389, 24516, 13224, 0x3ab9b0, "896c77ef2b3c6126729e9c5b43300cade7b9eb58f737d20f513b399d49ae1f7d"),
    ('N', 72109, 25236, 13282, 0x3af3d2, "afc0f5d09209358b022c5206c66b19c822b422c9293e59ad50ccead4ad6beb27"),
    ('N', 72115, 25242, 13291, 0x3af746, "e739fd270f145430feb0c4644a9c44af3492480a8829c130f1a2de9d7a6878b9"),
    ('N', 72121, 25248, 13296, 0x3afad2, "206400830d1fd27238ec8e54a8c94daec18eb390bf86d9765608d36ef8419d94"),
    ('G', 34174, 4820, 13655, 0x3c4fe7, "fd188480f3a2213368e120b22895b4bd3e6346dd676c3b373c23f7f97dce54c9"),
    ('G', 38604, 9100, 21298, 0x68737f, "946a63a8ff1d4f9c1b5120fe36af7e8eb20a3cf5fcfd12cb8a4c42b9d090144c"),
    ('P', 22396, 50863, 44716, 0xe98766, "da612f2ce2d9171b5620d9f914c690c17a205de65dfea83220f4a96123d3a40e"),
    ('P', 22325, 50792, 44648, 0xe92965, "38bd4cfe60b83a34f90d9645b66620ef3dc4a32fcd6f053539268f3c63d5e228"),
    ('P', 22419, 50886, 44743, 0xe9c892, "340b7bea4994a8bacd20ccbdd4b46f4c833a364aa1fcf6773e0d65f20ce90403"),
    ('P', 22421, 50888, 44745, 0xe9cde7, "ae324f2be93da1660ce90a2f2bc850aecef09a00e47f0748346fc58e6f8d0347"),
    ('G', 38764, 9260, 21304, 0x687d58, "dd8f9ec06529b199ba068fbe52bb5084e577ce9962ab772c3c9b1dae8c3cc3b3"),
    ('G', 38763, 9259, 21305, 0x687e50, "b3c3f1d03115bb7ed28a6bc17e10322f0f62eceb0236f3b658e35ee1e6515770"),
    ('G', 38759, 9255, 21397, 0x68e243, "c75be8f6a208c0774539711043dabfe4b11975366685e17fec17ae7edc7ff38e"),
    ('G', 38766, 9262, 21403, 0x68ed6f, "012d1d2dc23f2e55f0ef3188ce9699ac31cf65efe33a4646588dacd16e900f01"),
    ('G', 38779, 9275, 21417, 0x69006a, "703fb6c6363e20b3529c18f2c310745ff25c3f090aa20e6d4b11819cdf43c2d4"),
    ('G', 36020, 6556, 27438, 0x894680, "e6de4284b3d7be018ab5aee52da68375be034b1a1fcd1a98afe66f1f756ba86c"),
    ('P', 19186, 47782, 38245, 0xbe6202, "335952cc6c4fdfdf658611691284fe1a3a4d6f2e3d50bf0d0d72a8ea7c78f485"),
    ('U', 78704, 2866, 36241, 0xb09964, "69bfe3c5630d4dbf91c1da16ec0ceae4085354dee84d5f8da7ac28a6935673d5"),
 ]
for module,m,e,f,off,digest in anchors:
    p = h[5][0]+30*m; t = u16(md,p+4); rid = u32(md,p+18)&0xffffff
    assert u32(mem,modules[module][2]+4*(rid-1)) == e
    assert elements[e] == f and bodies[f][0] == off
    assert sha(w[off:off+bodies[f][1]]) == digest
    print(f"{name(u32(md,h[19][0]+66*t))}.{name(u32(md,p))} m{m} e{e} f{f}@{off:x} OK")
checks = {
    2619:{"<Screen>k__BackingField":84,"<Engine>k__BackingField":16},
    2667:{"run":200,"avatar":236,"services":316,"cameraX":452,"cameraY":456,"ready":400,"held":416,"route":372},
    2731:{"items":8},
    5465:{"path":12,"X":56,"Y":64,"HitLeft":176,"HitTop":180,"HitWidth":184,"HitHeight":188},
    5807:{"<State>k__BackingField":8},
    6101:{"<Target>k__BackingField":60,"Fsm":112,"attackTargets":104},
    6215:{"<State>k__BackingField":48},
    6256:{"<State>k__BackingField":100,"<Gauge>k__BackingField":104,"<Held>k__BackingField":152,"<Flags>k__BackingField":60,"<AutoMode>k__BackingField":192,"<AutoFlagOn>k__BackingField":200},
    6741:{"<Bindings>k__BackingField":20},
    6892:{"Code":8,"Modifier":12,"Exclusive":16},
    7915:{"On":8,"Heavy":20,"<Ready>k__BackingField":24},
    11817:{"targetPending":88,"sendArmed":89,"moveSentAt":112,"heldStop":132},
    13662:{"<Stage>k__BackingField":20,"<Input>k__BackingField":28},
    14434:{"WorldZoom":16,"WorldWidth":56,"WorldHeight":60},
}
for t, wanted in checks.items():
    actual = fields(t)
    assert all(actual[k] == v for k,v in wanted.items()), (t, wanted)
# Public literal usages actually referenced by GameInput's constructor.
literal_cells = {3427644: '<Keyboard>/leftShift', 3427668: '<Mouse>/leftButton', 3427676: '<Mouse>/rightButton', 3427628: '<Keyboard>/escape', 3427700: '<Pointer>/position'}
for cell,expected in literal_cells.items():
    v = u32(mem,cell); assert v>>29 == 4
    i = (v&0x1fffffff)>>1
    x,y = u32(md,h[0][0]+4*i),u32(md,h[0][0]+4*i+4)
    assert md[h[1][0]+x:h[1][0]+y].decode() == expected
# Algebra-only invented fixture: nonzero host origin and nonuniform backing ratios.
left,top,cw,ch,bw,bh = 27.5,41.25,960,540,1920,1350
hx,hy,hh,zoom,cx,cy = 8,10,1310,1.25,320,96
x,y = 500,300
sx,sy = hx+(x-cx)*zoom,hy+hh-(y-cy)*zoom
px,py = left+sx/(bw/cw),top+(bh-sy)/(bh/ch)
rx,ry = (px-left)*(bw/cw),bh-(py-top)*(bh/ch)
assert math.isclose(cx+(rx-hx)/zoom,x)
assert math.isclose(cy+(hy+hh-ry)/zoom,y)
assert (px,py) == (144.0,155.25)
# Exact ordinary mode branch once the live heavy predicate is known.
mode = lambda on,heavy: 2 if on and not heavy else 1
assert [mode(a,b) for a,b in [(False,False),(False,True),(True,False),(True,True)]] == [1,1,2,1]
print("PASS: artifact pins, method joins/bodies, field offsets, input literals; synthetic projection and mode cases")
```

[I28]: https://github.com/oDestroyeRo/opentrickster/issues/28
[PAGE]: https://tr-play.playall.in.th/
[CONFIG]: https://tr-play.playall.in.th/spinel-config.json
[VERSION]: https://tr-play.playall.in.th/version
[W]: https://tr-play.playall.in.th/Build/d9a4c4fd2737e740b82e1321839e352f.wasm.gz
[D]: https://tr-play.playall.in.th/Build/6135c4139b62e558bb4c755b266f5fa2.data.gz
[F]: https://tr-play.playall.in.th/Build/7f982dae9f02da04f526a91e415474b4.framework.js.gz
[L]: https://tr-play.playall.in.th/Build/41e1cc21f8b1ab11275e55eb6949af3c.loader.js
[R18]: https://github.com/oDestroyeRo/opentrickster/blob/df5fe0a5e8e45072845538fc74967a1757b6d9d7/docs/research/current-build-protocol-revalidation.md
[R14]: https://github.com/oDestroyeRo/opentrickster/blob/cb587e235b113aa26b04db018c31f1e26510f5bd/docs/research/target-state-completion.md
[R25]: https://github.com/oDestroyeRo/opentrickster/blob/b1a12a97420fff84f15cc1e3cf8b5070560ed7b9/docs/research/manual-dig-control.md
[R29]: https://github.com/oDestroyeRo/opentrickster/blob/efccf7992b77966804e9078d4643a565aa3b6340/docs/research/gun-combat-control.md
