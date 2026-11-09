# Native inventory trials on build ea1a68bd

This report records the runtime comparison that the [inventory baseline report](native-inventory-baseline.md) specifies, run on 2026-11-09 against build **`ea1a68bd89958fae94179f1e0835ffb5`**. It used one authorized existing character, with the sessions run one at a time. No gameplay, item use, inventory change or account change was sent. Credentials, UIDs, item names, payloads and values stay in private outside-Git files. Only sanitized results appear here.

**Result.** Both trials passed for the observed boots. **H** is the last raw receipt among the first accepted S5, S679, S230 and S233. At H, the native cache equals both the client's actual replay boundary and a later native-only boot, and so does the exact carried weight. `INVENTORY_CUTOFF_BUILD` is now set to this build.

## Trial A: client-assisted boot with a passive native shadow

- **Code:** production Net at `604fb63`. A private copy added trial-only dump hooks and switched the cutoff on. The bridge copy forwarded every inbound game-link frame.
- **Probe:** the ea1a68bd probe (`future-probe.js` SHA-256 `f9754a265ba1c5523c0c39f114f3cf22ad775c7a12880a020f64c82e0fd2f6d2`). It patched only its pinned bytes and captured the instrumented instance. The client reached world entry with `inventoryReady`, `weightReady` and one link change. One synchronous snapshot was taken at raw mark R = 191.
- **Comparator:** `compare.py` (`aba323c3acba0dd4a93c781631ec21879428f378c9253024a4524956577c9eae`) returned `PASS_FOR_OBSERVED_BOOT_ONLY`:
  - H = B = I = W = 21;
  - one non-empty list frame;
  - 67 rows equal by UID and field;
  - all 23 slots equal;
  - the exact I64 carried weight equals Unity's value, the public-definition sum and Net's candidate.
- **Production facts:** the `Weight` fact from the production cutoff path equals that sum, and `HpPotions` was reported.

## Trial B: native-only boot

The cutoff was switched on in the same private copy, with the live-frame fixes from #46. A hook wrote the native cache once, at the baseline. Compared with Trial A, it has:

- the same 67 rows,
- the same 23 slots, with no missing references,
- the same carried weight.

The production `Weight` fact equals the candidate sum, and `HpPotions` was reported.

The session did not stay open. About 0.1 s after the native world-entry C5, the gateway sent `g server`, while the browser client stays connected. That is outside the inventory cutoff. The [login-link report](native-login-link.md) records the cause and the fix. Start availability and reconnect behavior could therefore not be exercised in this trial. Start would also still be blocked by unknown HP and weight maxima (#47).

## Coverage and limits

| Variant | Observed |
|---|---|
| Startup lists | One non-empty list at or before H. A zero-count list was not exercised. |
| Row kinds | Wire Kind 1 and Kind 2 rows. No Kind 3 row, and no duplicate UID. |
| Aux buffering | No Aux-grouped frame in 390 inbound frames. The `ordered.rs` overlap limit in `crates/net/README.md` was not exercised. |
| Post-H inventory traffic | No decoded delta, late 231 or undecoded Inventory opcode during about one minute in the world. |

A pass establishes this build's behavior for the exercised startup, character and transport conditions. It is not a server guarantee. Any unobserved variant fails closed under the README's invalidation rules. A pin migration resets the cutoff.
