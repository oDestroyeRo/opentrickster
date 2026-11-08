# Trickster combat, digging, and quest progression research

The goal is an implementation-ready MVP specification for a macOS Trickster bot that controls one account and one character, automates monster combat and digging, and supports two modes. Client-assisted mode displays the game inside the app. Clientless mode controls a live character while displaying the bot dashboard. Quest research informs routes, but automatic quest acceptance, completion, and turn-in remain outside the MVP. These are requirements from [planning map #2][map], not conclusions about available game interfaces.

This report answers [research issue #5][ticket]. It identifies a documented gameplay model and a conditional early route. Public evidence does **not** establish the live target's complete quest graph, rates, action acknowledgements, or every interruption rule. Those gaps need target-specific evidence before the final specification can promise unattended progression.

## Research record and evidence boundaries

- Research date is 2026-11-08. Source retrievals and the unsuccessful historical archive request occurred on that date.
- Starting commit is `6b8162f1f7a42b2bd9dd02c1b086be360914bb8d`.
- Actual research branch is `docs/research-gameplay-progression`.
- The working tree was clean before research. This report is the only repository change. The final commit SHA and immutable report URL belong in the issue resolution comment, avoiding a self-referential commit hash inside this file.
- Research used unauthenticated public HTTP reads, web search, and the repository's GitHub issue records. No account was created, game session entered, character changed, or gameplay request sent. No game binaries or artwork were copied into the repository.

The labels below distinguish source strength.

| Label | Meaning |
| --- | --- |
| Target artifact | Directly inspected public files served by the target or its linked portal. This proves their contents, not live gameplay execution. |
| Target announcement | The target operator's dated, non-sample news. This proves an operator statement about this service, not an independently reproduced result. |
| Target guide | Owner-published Rebuild instructions. Controls and names have a stated client-file basis; mechanics and route advice have explicit classic/community caveats. |
| Original account | A dated player's firsthand account of the original PC game. It is neither publisher documentation nor target compatibility evidence. |
| Inference / proposal | A planning consequence or suggested bot behavior, not a discovered game rule. |
| Unknown | Public evidence inspected here does not resolve the question. This does not mean unsupported. |

### Establishing the correct service

**Target artifact.** The portal's public [content response][content] names `https://tr-play.playall.in.th/` as `gameStart.url`. Its [served JavaScript][bundle] defines the guide, journey, and class-guide routes. This connects those documents to the requested target without relying on a similar server name.

**Target artifact.** The [target HTML][target] creates a Unity canvas and identifies company `Playall`, product `Spinel`, and `productVersion` `1.0`. It references the build files recorded in the source register. That version string is not a gameplay ruleset version, protocol compatibility guarantee, or proof that an original Windows client can connect.

**Target announcement.** [CBT announcement #13][n13], dated November 3, announces November 7 at 15:00 Thailand time. [CBT missions #16][n16], dated November 5, gives a November 7–10 test period. The content response's English opening notice still says delayed while its Thai notice says CBT is open. Treat availability as time-sensitive; no successful login was tested.

**Target guide.** The [beginner guide][basics] says hotkeys, chat commands, and item/NPC names were checked against Rebuild's Thai game files. It attributes game-system material to classic Korean-service references including namu.wiki, arca.live, and Trickster Atlas, and warns that Rebuild values may differ. The [journey sources chapter][journey-sources] similarly separates client/team material from advice gathered across player communities. Those secondary sources are not promoted to verified publisher evidence in this report.

**Excluded compatibility evidence.** The inspected [playtrickster.com play link][other-play] redirects to `play.playtrickster.com`, whose HTML has a different client bootstrap. Its [November 8 patch notes][other-patch] therefore do not establish this Unity target's rules. No link proving a shared game service was found. Likewise, emulator schemas and other private-server rates are not used as target facts.

## Relevant character and resource model

The following table summarizes the **target guide**, not independently measured formulas. The guide's [stats chapter][stats] groups twelve stats into Power, Magic, Sense, and Charm. It describes automatic growth through the creation-time Build Graph and four bonus points per base level. The [skills chapter][skills] treats TM as a separate skill progression track, with a point per TM level and TM earned through hunting, digging, and skill use.

| Values | Documented purpose | Consequence for the specification, an inference |
| --- | --- | --- |
| HP / maximum HP, DP, HV | Health, physical defense, and evasion. [Stats][stats] | Health, fainting, and defensive status must be observable independently of attack commands. |
| MP / maximum MP, MA, MD | Mana, magic power, and magic defense. [Stats][stats] | A spell rotation needs MP availability and recovery policy. |
| AP, AC, DX | Attack, accuracy including gun-damage relevance, and attack speed. [Stats][stats] | Melee, gun, and spell profiles cannot share an assumed damage formula or cadence. |
| WT / maximum capacity | Carrying capacity. [Stats][stats], [combat][combat] | Track carried weight and capacity as well as item slots. |
| DA, LK | Detection/digging, and luck/critical relevance. [Stats][stats] | Record both, but do not hardcode an unverified detection probability or critical formula. |
| Base level/EXP, TM level/EXP, unspent points | Two distinct progression tracks. [Stats][stats], [skills][skills] | Display separate counters. The MVP need not allocate stats or learn skills automatically. |
| Galder, potions, equipped weapon/drill/pet, consumable quantities | Currency, recovery, and activity prerequisites. [Combat][combat], [digging][dig], [equipment][gear] | A run configuration needs a resource budget and a clear reason when it cannot continue. |

**Target announcement.** The [November 7 skill patch][n19] establishes target-specific changes to cast delays, cooldowns, MP costs, areas, and debuff durations. Examples are Mana Arrow's announced 1-second cast delay, Power Shot's 0.5-second delay, and Rain of Knives consuming 2–5 throwing weapons. These are specific parameters with a date, not a complete skill table. The same announcement makes copying an old skill database without reconciliation unsafe.

**Target guide.** The [skills chapter][skills] describes skill cards from the character type's Paradise Skill Master, skill levels up to 10, and mastering through an NPC with materials and more TM points. The [class guide][classes] describes passive, one-time, and timed skills and their prerequisites. **Unknown.** The chosen character, learned skills, actual per-level costs, range units, cooldown groups, cast cancellation, recovery timing, ammunition consumption, and status-effect constraints still need client or runtime confirmation. Do not infer that every class supports the same combat profile.

## Combat, movement, and loot

### Documented interaction

These are **target-guide** statements from [PC controls][controls], [combat][combat], and [travel][travel].

| Interaction | Published behavior | Unresolved target detail |
| --- | --- | --- |
| Movement | Ground clicks move; holding continues movement. `R` toggles running. | Collision grid, movement speed, path validity, coordinate units, and an authoritative arrival event. |
| Monster selection | Clicking a monster walks toward it and attacks. `Tab` selects the next target; the Thai guide specifies nearby monsters. | Target IDs, claimed targets, attack range, line of sight, aggro radius, leashing, and target loss. |
| Skills and items | `F1` through `F8` use quick slots, with five pages. An optional setting finds a nearby target for skill keys. Hotkeys are rebindable. | Exact cast sequence for each skill and whether target selection itself starts an attack. |
| Loot | Click ground items or their names with `Alt`; `Z` picks a nearby item. | Ownership reservation, pickup range, expiry, stacking, partial pickup, and error messages. |
| Navigation | Map-edge portals, town teleporters, and return items connect maps. The large map can navigate to NPCs and monsters. Some destinations require gate registration. | Current map graph, portal conditions, teleport costs for the chosen route, and transition acknowledgement. |
| Movement blocked | The FAQ mentions sitting and open settings/shop/craft windows as reasons walking can fail. | All modal windows, immobilizing effects, and how a controller detects them. |

**Target guide.** [Combat][combat] says the built-in AUTO prioritizes skills, fills gaps with normal attacks, collects loot, and uses configured potions through the AI window `B`. It says AUTO stops on fainting, exhausted potions, or a full bag/weight. It also describes a daily drop-time allowance after which monsters give EXP/TM but no drops, and maps that forbid bots. The inspected docs do not give the allowance amount, reset semantics, map list, or whether these rules also govern an external controller.

**Inference.** Built-in AUTO is evidence of an existing player workflow worth evaluating. It is not evidence of an external control API, permission for a third-party bot, clientless availability, or suitable observability. The integration decision must establish those separately. The bot must not show a run as healthy merely because attacks continue while the requested quest item can no longer drop.

### Proposed combat loop for the decision tickets

This is a **proposal**, derived from the documented interactions above and their missing acknowledgements. It is not implemented behavior.

1. Verify the selected character, map, resource budget, activity permissions, and current observations. Pause if identity or state is uncertain.
2. Choose a configured monster in the allowed area. Exclude invalid or disallowed targets according to a target policy established during validation.
3. Approach until the selected attack is valid. Require observed progress; stop or choose another target when progress times out.
4. Execute the configured basic attack or learned skill. Observe target existence, HP, cast result, cooldown, MP, and consumable use before continuing.
5. Recover HP/MP according to the user's thresholds. Reassess when enemies, status effects, or resources change.
6. Confirm the combat outcome. Collect only configured items while preserving required quest materials and capacity.
7. Update observed inventory and EXP/TM, then select another target or pause with a concrete reason.

**Proposal.** The minimum end states are target defeated, target disappeared, target unreachable, character fainted, resource exhausted, capacity reached, action rejected, manual pause, map transition, and disconnected/observation lost. After uncertain outcomes, reobserve before retrying. A timeout alone must not increment kills, consumed potions, loot, or quest progress. These distinctions follow from the separate action and outcome observations the [documented loop][combat] requires; exact server events remain unknown.

**Original account, not target fact.** W1N9Z3R0's original-PC [FAQ v1.3][original], last dated June 10, 2008, describes aggressive/passive monsters, restrictions on attacking another player's engaged monster, base EXP per successful hit, and TM on kills with a Charm exception. It describes digging as awarding base and TM EXP even without a find. The author explicitly marks quest information obsolete after the August 2007 Revolution patch. These claims justify investigation questions, not copied target rules. In particular, current EXP attribution, kill ownership, aggro, and loot reservation are unknown here.

## Digging prerequisites, interaction, and outcomes

All mechanics in the next table are **target-guide** descriptions, with the provenance caveat above. The [digging chapter][dig] distinguishes semi-auto single-spot digging from pet-assisted automatic spot selection.

| Stage | Published behavior | Specification consequence, an inference |
| --- | --- | --- |
| Equip | Obtain a Basic Drill from Driller Marky or buy a drill from Item Girl. Equip it through the bag's Drill tab. | Refuse a digging run without an eligible equipped drill. Exact level/stat/terrain eligibility must come from inspected item data. |
| Choose area | Fields and dungeons allow digging; towns and plazas do not. `/Depth` reports map depth. | The route needs map permission, terrain, and depth data; a drill's grade alone is insufficient. |
| Begin | `D` enters digging. Select a ground point; the character approaches, digs, handles the gauge, and picks up a find. | Separate point selection, arrival, probing, active digging, and collection. |
| Semi-auto | Enabled by default. After the attempt, the game asks for another point. | One press is not evidence of an endless autonomous digging loop. |
| Manual gauge | With semi-auto disabled, hold the gauge in its allowed band. Overheating resets depth; right-click cancels. | Manual gauge control has timing and reset failure cases. Whether the integration can observe these is unresolved. |
| Detection | The character may announce an item or an empty spot before digging. A detected empty spot stops without drill wear. DA affects digging. | Distinguish an empty probe from a completed empty dig. The DA probability is not established. |
| Wear | The guide says each press uses one drill life, or two for mismatched soil. It lists terrain types in drill descriptions. | Observe durability changes; do not translate "press" into a packet or gauge tick without evidence. |
| Depth failure | A low-grade-drill message means a deeper drill is needed. | Pause for configuration/equipment correction; repeating the same point is not a remedy. |
| Finish | A find or an empty result ends the attempt. Digging supplies items and TM EXP. | Require result and inventory/EXP evidence. Base EXP and reward timing remain unverified for this target. |
| Cancel | `D`, the mobile dig button, or `Esc` stops semi-auto digging. Hiding its banner only hides the banner. | The app needs a real stop acknowledgement, separate from hiding game UI. |

**Target guide.** The same [chapter][dig] describes Crazy Drilling as an unlocked passive, associated with stress buildup and a short burst. It names a level-25 Paradise chain through Indiana John, Explorer Reina, and Driller Marky, and `/Stress` as a diagnostic command. It describes Driller Boy pets as allowing spot-free digging, with no Crazy Drilling in that mode. Exact stress thresholds, buff duration, skill prerequisites, durability exhaustion behavior, and interruption by damage are unknown here.

**Target announcement.** [MyShop article #32][n32], dated November 6, advertises Baby Driller Boy 30 as a level-30 auto-dig pet with a 72-hour duration and Flicker Drill X as level 30, grade 7, depth 300, and durability 1,800. It says Driller Boy 100 cannot use Surprise Spots or Crazy Drill. The article retains launch-date placeholders, so it does not prove current purchase availability. These are optional item-specific descriptions, not prerequisites for the MVP and not a recommendation to buy anything.

**Proposal.** Model digging as prepare, select point, approach, detect, dig, collect, and reobserve. Preserve distinct outcomes for empty probe, empty dig, item found, insufficient depth, overheating, exhausted drill, blocked movement, capacity failure, hostile interruption, cancellation, map change, and session loss. Only some are documented outcomes above; exhausted-drill and hostile-interruption semantics specifically require observation. A safe default for an unclassified interruption is to pause with the last confirmed state.

## Equipment, inventory, recovery, and failure limits

**Target guide.** [Combat][combat] describes loss of running above 90% carried weight, inability to buy/trade/pick up at full capacity, and a limit of 300 item kinds. It describes shop HP/MP potions as heavier than G potions. Treat those numbers as documented expectations pending target checks. Slot counting, stack maxima, capacity arithmetic, potion cooldowns, and whether equipment weight counts are unknown.

**Target guide.** [Items][items] distinguishes character Storage from account Bank and documents level gates and fees. Its storage description says no weight limit, while [MyShop #32][n32] advertises a permit that raises storage weight as well as slots. This unresolved wording is a reason to inspect the actual storage window before designing automatic restocking. Automatic selling, banking, shopping, mail claims, and equipment upgrades are not assumed MVP scope.

**Target guide.** [Equipment][gear] says equipment has minimum levels and sometimes stat requirements. It describes refining failures as leaving broken, unequippable gear and repair through Blacksmith Marx. **Target announcement.** [Equipment article #41][n41], dated November 8, documents monster equipment drops, random bonus options, seven scroll slots per piece, and crafting from hunting materials. The relationship between these newer systems and the older guide's refining/compounding details is not fully specified. The MVP can observe equipped item identity and usability without automating those upgrades.

**Target guide.** At zero HP the character faints. [Combat][combat] describes no-EXP-loss revival through level 20, later restart to town, and item-based revival with map restrictions. It gives no exact post-level-20 loss formula or authoritative respawn location rule. **Proposal.** Pause on fainting and let the user revive in the MVP unless the behavior decision explicitly adds a verified recovery flow. Never assume a previous run's location or target survives death.

**Unknown target failures.** Remaining cases include monster disappearance during casting, silence/stun/sleep, damage during digging, competing pickups, vanished loot, exhausted ammunition, item expiry, insufficient Galder, map restrictions, disconnects, maintenance, reconnect, and character/session replacement. The [portal content][content] publishes a maintenance window, but no live disconnection behavior was tested. Each case needs an observed result or an explicit pause policy, rather than invented server semantics.

## How combat and digging feed quests and leveling

**Target guide.** [Quests][quests] presents Normal, Monster, Party, Story, Job, and Key tabs. It recommends following region Key Quests and Episodes, and describes objectives that collect monster drops or dug items. The [journey][journey] places hunting fields, digging, TM acquisition, and manual NPC progression alongside each other. These sources do not provide a complete target quest database or EXP reward table.

**Target guide.** The quest helper's prose first summarizes automatic walking, hunting, and return, then explicitly says the player presses Accept/Complete. It documents repeat counts, pausing on manual movement, and Resume. This is evidence of a helper workflow, not proof that every quest or digging objective can execute automatically. It also does not extend the [map's scope][map] to quest automation.

**Proposal for the MVP.** Let the user accept quests and select the combat/digging objective. The bot can then gather configured items or hunt configured monsters, show confirmed inventory and available quest counters, and stop at the requested quantity. Reserve quest materials from any future disposal policy. The user reviews completion and turns in the quest. Read-only quest display is distinct from automated acceptance, scripted completion, turn-in, and reward claiming, all excluded from this MVP.

### Conditional early-game flow

This is the exact sequence supported by the **target guide's published route**, not a validated live walkthrough. Individual steps are conditional on the NPC/quest actually being available. Missing steps and quantities are left missing. See [levels 1–15][early] and [levels 15–35][desert].

| Order and gate | Documented step | What combat/digging can supply | Missing before a verified route |
| --- | --- | --- | --- |
| Start, Blooming Cora | Begin EP0 registration with Megalo Crew Sera, then Bunny Maid. | The guide places early hunting in Coral Beach Fields 1–3. | Quest IDs, dialogue choices, required kills/items, rewards, and the complete connecting quest chain. |
| Obtain a drill | Get the Basic Drill from Driller Marky. The early guide describes depth 20. | Equip and perform field digging. | Target item ID, durability, actual availability, terrain/depth data, and item-generation behavior. |
| Level 13 or above | Driller Marky's test requests two Tanning Oil dug near him. The digging chapter gives a pet and Drilling for Dummies as rewards. | Gather two confirmed Tanning Oil; the user submits them. | Exact NPC coordinates/map assignment and quest ID. The early route subsequently names Don Giuvanni at Gate of Desert Beach but does not fully specify the transition or prerequisites. |
| Follow EP0 chapter 2 | Continue Don Giuvanni's Training at Gate of Desert Beach. | Only objectives verified in the current quest window should become bot tasks. | Full item/kill requirements, intermediate NPCs, gates, and rewards. The guide is insufficient to script the chapter. |
| Level 15 | Receive the Teleport Guide by mail; the next band uses Paradise as a base. The Star Gazer quest is named at Desert Beach Field 2. | Continue hunting Desert Beach Fields 1–4 while the user handles quests. | Actual mail contents, claim steps, route portals, and The Star Gazer requirements. No guaranteed level gain is documented. |
| Level 25 | The first region Key Quest begins with Sandman in Paradise and needs five Desert Stickers. | The guide says repeating a town quest five times yields one sticker. | Which eligible town quests, exact item requirements, per-quest repetition limits, and prerequisite completion flags. This is not evidence that any arbitrary town quest gives a sticker. |
| Desert Beach Key Quest | Its next stages name a Legendary Recipe from Sand Demon in Desert Beach Field 3, then a Glyph Tablet dug in the Pyramid Dungeon for The Miracle of Sand. | Hunt the named monster and dig only after verifying the active objective. | Item quantities, drop chances, eligible dungeon floors, depth, and exact stage prerequisites. The guide names Adventurer's Book and Weapon Ticket A as outcomes but omits a full reward ledger. |
| Level 25, optional digging progression | Indiana John, Explorer Reina, and Driller Marky form the documented Crazy Drilling chain in Paradise. | Gather only materials specified by the observed quest. | Intermediate quests, item quantities, costs, and unlock confirmation. It is not required merely to perform basic digging. |

**Finding.** The sources support a useful early route skeleton and a concrete two-item digging objective. They do not support a complete target-specific level-1-to-35 recipe or a guarantee that these quests alone reach those levels. Base/TM multipliers, reward totals, monster/drop tables, spawn timing, and exact completion flags remain unknown.

**Later progression context, target-guide claim.** The [levels 35–60 chapter][relics] describes a level-60/TM-50 first-job goal, with a Growth Badge from Kaboom and three 100K Galder Checks. This explains why keeping TM and money alongside base level matters. It is an investigation lead beyond the early MVP route, not a verified requirement to encode now. No boss hunting, party management, or job-change automation is added to scope.

## Findings usable by the MVP decisions

These are **proposals for map #2's decision work**, supported by the evidence above. They do not resolve another ticket.

| Decision | Evidence and implication |
| --- | --- |
| Activity scope | [Combat][combat] and [digging][dig] have different prerequisites, resource limits, and outcomes. Specify separate activity configurations and stop reasons. |
| Digging baseline | The documented default is assisted single-spot digging. Pet-driven automatic spot selection is optional and item-dependent. Decide whether the bot selects spots itself or uses a verified built-in behavior. Neither route is proven available to a clientless controller. |
| Combat baseline | The [dated skill patch][n19] changes timings and costs. Select an initial character and explicit learned-skill profile; avoid promising all nine characters from generic formulas. |
| Dashboard | Expose live character/map identity, activity/state, target, HP/MP, EXP and TM separately, capacity, potions/ammunition, drill eligibility/durability, confirmed item counts, and last stop/error. These derive from the documented resources and failure cases. |
| Quest boundary | Follow the [map's exclusion][map]. Show objectives and gather resources only within the chosen combat/digging task. Manual quest progression remains visible as a pause/handoff. |
| Both application modes | The same observed character and activity outcomes must back the dashboard in both modes. A visible game canvas proves neither clientless actions nor state access; that belongs to integration research. |
| Verification | Require a recorded target build and evidence for activity start, progress, result, and stop. Do not count submitted inputs as successful actions. |

## Remaining gaps and smallest next investigation

The bounded public-documentation question is answered, including its unknowns. Implementation readiness still depends on the following **follow-up decision/access task** being tracked by the map owner.

**Proposed task.** Validate one early Rebuild character profile, combat loop, digging loop, and manual quest handoff against a pinned target build. Decide the initial character and recovery/loot policies before finalizing the behavior specification. Obtain target-specific evidence through owner-provided data or a separately authorized operator session. This report does not authorize that session.

The smallest next step that requires no game action is to reconcile available target quest/item/monster/map/skill definitions, if supplied, with the published November 7 patch and the two-Tanning-Oil objective. Request only the required export or a sanitized recording through the root's access decision, not credentials. Needed fields are listed below.

| Gap | Minimum evidence or observation |
| --- | --- |
| Quest route | Registration and Driller Marky quest IDs, prerequisite flags, map/NPC IDs, exact item counts, consumption, EXP/TM rewards, repeat limits, and user-controlled Accept/Complete transitions. |
| Combat | One supported attack/skill's range, MP and consumable cost, cast/cooldown acknowledgements, target death/despawn, aggro/claim policy, loot reservation, and pickup result. |
| Digging | One drill's eligibility, terrain, depth, durability, normal empty result, successful find, damage interruption, wear exhaustion, capacity failure, and acknowledged cancellation. |
| Resource limits | Actual weight and slot rules, stack limits, potion timing, ammunition consumption, equipment usability, and storage inconsistency. |
| Recovery | HP-zero handling, EXP loss, respawn destination, disconnect/maintenance, and resynchronization without duplicate action accounting. |
| Built-in helpers | AUTO availability, prohibited maps, drop allowance/reset, user overrides, and whether helper state is observable through each proposed integration. |
| Rates and leveling | Base/TM/quest multipliers and real per-objective reward deltas. No ETA or guaranteed leveling rate is defensible before these are known. |

If an operator session is later authorized, a minimal check is one allowed monster kill with one collected drop, one empty and one successful dig, deliberate Stop, and manual acceptance/turn-in of the verified two-item objective. Record build, map/character IDs, resource and inventory deltas, and quest state before/after. Failure cases can come from owner test data or a safe test environment rather than risking a live character. That would turn the route skeleton into reproducible acceptance criteria without adding quest automation.

## Source register and reproducibility

The source links below were read, not inferred from search snippets alone, except the inaccessible historical publisher page explicitly listed as a gap. Target HTML, portal JavaScript, and JSON were retrieved with public `curl` GET requests using a normal browser user-agent. Some `web.open` requests could not fetch the same target pages. No authenticated API, content mutation, client binary, or game session was used.

| Source | Version/date and inspected locator | Use and limits |
| --- | --- | --- |
| [Target HTML][target] | Retrieved 2026-11-08; `Spinel` `1.0`. Loader `877598568ac7f0dc41389b0d6e411b8b.loader.js`; data `9b2286992122d3c1cc081492dc31fee5.data.gz`; framework `4937d6d47bade23f56e5ad4b5bf2e2b9.framework.js.gz`; wasm `657890b2bd8548390dd7c7fad76093be.wasm.gz`. | Only HTML inspected. Referenced game binaries were not downloaded. HTML SHA-256 `44c56cd767ce2cb5dd1ae8cce995f60011c9dce4ff12b146a9c9c1ab39fe8216`. |
| [Target portal][portal] and [content JSON][content] | Retrieved 2026-11-08. Content links the requested game URL, news IDs, notices, and maintenance. | JSON SHA-256 `b7b3f3187e8d2afd14dc9404ea95fd0a5abd40fa24d22e7c927eaba355c211b3`. Dynamic content has no immutable revision or update timestamp. |
| [Portal JavaScript][bundle] | Filename `index-KlvPLZcC.js`; retrieved 2026-11-08. SHA-256 `09633e9d8b9b9c641a3ed55407c0333f4b894bd28d06948aacdacf31a254a49d`. | Source for client-rendered guide prose and route definitions. No bundled code was executed during research. No guide publication date is provided. |
| [Beginner guide][basics] | Bundle chapter IDs `controls-pc`, `stats`, `skills`, `dig`, `quests`, `combat`, `travel`, `items`, `gear`, `faq`. | Owner guide with explicit provenance caveats. Its stated Thai-client cross-check was not independently reproduced. |
| [Journey guide][journey] | Chapter IDs `lv-1-15`, `lv-15-35`, `lv-35-60`, `sources`. | Owner-published mixed client/community route. Exact quantities are retained only where stated. |
| [Class guide][classes] | Retrieved 2026-11-08. | Skill categories and consumable leads. Classic/player advice needs target reconciliation. |
| [CBT announcement #13][n13], [JSON][a13] | November 3, 2026; `isSample=false`. | Dated planned opening, not observed live availability. |
| [CBT missions #16][n16], [JSON][a16] | November 5, 2026; `isSample=false`. | CBT period and event context. No event rewards were claimed. |
| [Skill balance #19][n19], [JSON][a19] | November 7, 2026; `isSample=false`. | Target-specific announced deltas. No skill was cast. |
| [Equipment #41][n41], [JSON][a41] | November 8, 2026; `isSample=false`. | Current equipment-system description. Does not resolve every older guide interaction. |
| [MyShop #32][n32], [JSON][a32] | November 6, 2026; `isSample=false`, with visible date placeholders. | Drill/pet descriptions; purchase and live availability not tested. |
| [Character article #9][n9], [JSON][a9] | October 22, 2026; `isSample=false`. | Explicitly says job/skill details derive from original Trickster and may change in Rebuild. It is not live proof of every class. |
| [W1N9Z3R0 FAQ][original] | v1.3, June 10, 2008; battle, EXP, drill, and version-history sections. | Clearly firsthand original-PC account with mixed credited inputs. Not publisher evidence. Its own post-Revolution obsolescence warning disqualifies its quest recipes here. |
| [Historical Ntreev drill guide][archive] | Archive URL points to July 7, 2010. | Fetch failed. Not used to establish any mechanic; original-publisher verification remains missing. |

The report was reviewed against the issue and map, checked for its sole-file scope, source references, evidence labels, and unfilled route requirements. Gameplay, timing, integration capability, and leveling rates were not tested. After committing, delivery validation must read the committed report, compare its bytes with the working file, and confirm the same SHA on the pushed branch.

[map]: https://github.com/oDestroyeRo/opentrickster/issues/2
[ticket]: https://github.com/oDestroyeRo/opentrickster/issues/5
[target]: https://tr-play.playall.in.th/
[portal]: https://tr.playall.in.th/
[content]: https://tr-api.playall.in.th/api/content
[bundle]: https://tr.playall.in.th/static/index-KlvPLZcC.js
[basics]: https://tr.playall.in.th/guide
[controls]: https://tr.playall.in.th/guide#controls-pc
[stats]: https://tr.playall.in.th/guide#stats
[skills]: https://tr.playall.in.th/guide#skills
[dig]: https://tr.playall.in.th/guide#dig
[quests]: https://tr.playall.in.th/guide#quests
[combat]: https://tr.playall.in.th/guide#combat
[travel]: https://tr.playall.in.th/guide#travel
[items]: https://tr.playall.in.th/guide#items
[gear]: https://tr.playall.in.th/guide#gear
[journey]: https://tr.playall.in.th/guide/journey
[journey-sources]: https://tr.playall.in.th/guide/journey#sources
[early]: https://tr.playall.in.th/guide/journey#lv-1-15
[desert]: https://tr.playall.in.th/guide/journey#lv-15-35
[relics]: https://tr.playall.in.th/guide/journey#lv-35-60
[classes]: https://tr.playall.in.th/guide/classes
[n13]: https://tr.playall.in.th/news/13
[a13]: https://tr-api.playall.in.th/api/news/13
[n16]: https://tr.playall.in.th/news/16
[a16]: https://tr-api.playall.in.th/api/news/16
[n19]: https://tr.playall.in.th/news/19
[a19]: https://tr-api.playall.in.th/api/news/19
[n41]: https://tr.playall.in.th/news/41
[a41]: https://tr-api.playall.in.th/api/news/41
[n32]: https://tr.playall.in.th/news/32
[a32]: https://tr-api.playall.in.th/api/news/32
[n9]: https://tr.playall.in.th/news/9
[a9]: https://tr-api.playall.in.th/api/news/9
[original]: https://www.supercheats.com/pc/walkthroughs/trickster-walkthrough01.txt
[archive]: https://web.archive.org/web/20100707132915/http://trickster.ntreev.net/gameguide/cg_drill.aspx
[other-play]: https://playtrickster.com/play
[other-patch]: https://playtrickster.com/news/patch-note-november-8-2026
