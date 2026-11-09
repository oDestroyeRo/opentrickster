# Implementation evidence

The static reports are unchanged copies of their published research commits. Statements
about a report's original branch, single-file scope and checks describe that
publication. They do not certify the production adapters or the complete MVP.

The current implementation profile targets build
`ea1a68bd89958fae94179f1e0835ffb5`. The reports below were written for
`8502ebbf2fc9033d2fba50d02fd92002`; the [build `ea1a68bd` migration](build-ea1a68bd-migration.md)
records which of their joins were re-proved and which pins moved. The
[sampler migration](build-ea1a68bd-sampler-migration.md) does the same for the
client-assisted sampler pins. Validate asset fingerprints before using the
recovered contracts. Older builds and prototype
labels are not interchangeable.

| Contract | Report | Original commit | Evidence and limit |
| --- | --- | --- | --- |
| Framing, opcode maps, authentication/bootstrap compatibility | [Current-build revalidation](current-build-protocol-revalidation.md) | `df5fe0a5` | Pinned static joins and synthetic checks; no live acceptance in this report. |
| Migration to build `ea1a68bd` | [Build ea1a68bd migration](build-ea1a68bd-migration.md) | This change | Rerunnable static re-derivation of every Net pin and normalized body comparison; no live acceptance. |
| Client-assisted sampler on build `ea1a68bd` | [Sampler migration](build-ea1a68bd-sampler-migration.md) | This change | Rerunnable static re-derivation of every sampler pin and an uninstantiated Trial A probe; synthetic checks only. Records five pre-existing sampler read defects. |
| Client-assisted sampler heap reads | [Sampler heap reads](sampler-heap-reads.md) | This change | Fixes those five reads and an earlier type-handle read from metadata and compiled code; the invented heap checks every read against the pins. Later sections fix the deferred drill install and six live refusals, and audit the other reads. Synthetic checks only; no live `ready`. |
| Own/entity state, effects, inventory and action-result layouts | [State completion](target-state-completion.md) | `cb587e23` | Current and prior build joins; receipt, result and inventory mutation remain distinct. |
| Combat frame tails on 31, 32, 36, 37, 43, 485 and 1438 | [Combat frame tails](combat-frame-tails.md) | This change | `Combat.ParseMessage` and `ParseSwing` never check the end: `U16At` reads a missing byte as zero, `I32At` needs 4 bytes, and a strike needs 8 or 14. Net now matches, except a grouped 43 under 2 bytes. Static and synthetic checks only; no live session after the fix. |
| Native inventory startup and carried weight | [Inventory baseline](native-inventory-baseline.md) | `ce0cf202` | Exact replay/cache/weight oracle and comparison procedure. |
| Runtime inventory trials on `ea1a68bd` | [Inventory trials](native-inventory-trials.md) | This change | Trial A matched the client's cache and weight at H = B; a native-only boot matched Trial A. Observed boots only; no Aux groups or empty list exercised. |
| Clientless authentication and initial state | [Native startup](native-startup-current-build.md) | `5de55dbf` | One standalone script reached the existing character without Unity. This is not production Rust adapter or gameplay proof. |
| Production Rust authentication and initial state | [Rust startup verification](native-rust-startup.md) | Tested code `504e6c61` | One native sequence, matching own label, position, HP, MP, and joined cleanup. No gameplay or complete-inventory proof. |
| Game-link liveness while idle (32572) | [Pass-progress liveness](pass-progress-liveness.md) | This change | 32572 is season-pass progress, subscribed only by `PassLink` and consumed only by the pass feature. Net reports `StateCurrent` when a release applies a received game frame, and the page forwards 32572 as a bare header. Static and synthetic only; no live session after the fix. |
| Native login link through the game session | [Login link](native-login-link.md) | This change | Client keeps the login-server link open and answers 2028 only there; one native boot stayed connected 60 s idle after the fix. No reconnect or gameplay proof. |
| Damage-event tail on 42 | [Damage-event tail](damage-event-tail.md) | This change | `Combat.ParseDamage` reads 26 bytes and an optional `Guard`, with no end check; Net accepts any tail after 26 bytes. Static and synthetic checks only; no live session after the fix. |
| Full manual dig | [Manual dig control](manual-dig-control.md) | `b1a12a97` | Complete timed controller and serializers; actual tool/spot state and server acceptance require runtime evidence. |
| Normal gun attack and attribution | [Gun combat control](gun-combat-control.md) | `efccf799` | Equipment, timing, request and own-result paths; live effect baselines and gameplay remain separate. |
| Own effect baseline and gun reload-speed lifetime | [Own effect lifecycle](native-own-effect-lifecycle.md) | `1614063e` | Positive own appearance baseline, attached/detached lifetimes and conservative recovery; actual effects, reload continuity and accepted shots still need runtime evidence. |
| Resource maxima without S282 on `ea1a68bd` | [Derived maxima](derived-resource-maxima.md) | This change | Static `CharacterStatus.Total` and `GameParts` input map. Net derives the installed steady state while every input is proven, and never on the page path; server enforcement is unprovable. Live oracle prepared outside Git, not run. |
| Native gun controller on `ea1a68bd` | [Gun controller sources](native-gun-controller.md) | This change | Current joins and body hashes for every method the controller relies on, plus StartSet, wear, damage, camp and strike-timing facts the gun report did not state. Static and synthetic only; server acceptance of 477 and live attribution are untested. |
| Actual public definitions and resource formulas | [Game definitions](target-game-definitions.md) | `07bd3daf` | Decoded item/drill/gun/zone tables and checks; catalog rows do not establish current character state or inventory completeness. |
| Normal embedded input and private read-only state | [Assisted input](assisted-input-control.md) | `6f93a75a` | Source-backed bindings, projection, controller and drain predicates; native delivery and live correspondence still need verification. |
| Gameplay and quest progression | [Gameplay progression](trickster-gameplay-progression.md) | `4dc02384` | Target guides with explicit classic-game caveats. Quest automation is outside the MVP. |

Earlier framing/authentication/layout reports remain linked by immutable GitHub
commit URLs inside these reports. Reproduction code uses public artifacts and
synthetic fixtures; do not commit downloaded game assets, compiled access-bearing
keys, credentials, private captures or actual account values.

The [assisted sampler #30](https://github.com/oDestroyeRo/opentrickster/issues/30)
remains under review and integration. The bounded source investigations for
[inventory/weight #31](https://github.com/oDestroyeRo/opentrickster/issues/31) and
[own effect lifecycle #32](https://github.com/oDestroyeRo/opentrickster/issues/32)
are complete; inventory cutoff acceptance, production integration and live validation remain under
[#21](https://github.com/oDestroyeRo/opentrickster/issues/21) and
[#23](https://github.com/oDestroyeRo/opentrickster/issues/23).
The production adapters, desktop, live integration and package remain under
[MVP delivery #19](https://github.com/oDestroyeRo/opentrickster/issues/19).

[MVP acceptance](../mvp.md) requires real combat and completed digging in both
modes, reviewed merged code and a verified macOS app. Static recovery, synthetic
traces and an embedded title screen each prove a narrower boundary.
