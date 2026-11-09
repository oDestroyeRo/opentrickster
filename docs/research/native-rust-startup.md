# Production Rust startup verification

On 2026-11-09 UTC, the production Rust adapter completed one bounded native
sign-in on build `8502ebbf2fc9033d2fba50d02fd92002`. It selected the existing
character and received its initial state without Unity or a browser.
This verifies one part of [#21](https://github.com/oDestroyeRo/opentrickster/issues/21).
Combat, digging, inventory completeness, and application integration remain open.

## Exact code and observation boundary

The test used unchanged Engine and Net files from commit
[`504e6c611191d993ffd199d249e19631492b6030`](https://github.com/oDestroyeRo/opentrickster/commit/504e6c611191d993ffd199d249e19631492b6030).
A separate Rust executable called the public `VerifiedProfile`, `NetCredentials`,
and `NativeSession` APIs. Every copied Engine and Net file matched that revision.
The executable did not replace the transport, authentication, or decoder.

Public asset validation passed before credential loading. The actor repeated
its normal fresh deployment check before opening the authentication sequence.
Credentials came from the already-authorized private file. The executable
moved them directly into the opaque credential capability and zeroed its input
buffer. It printed no identity, credential, ticket, or packet body.

Selection required exactly one advertised world and one existing non-busy
character. Ambiguous selection, refusal, loss, or a startup timeout ended the
test without a retry. No gameplay action was submitted.

## Observed result

The sanitized report records:

- One authentication sequence and zero retries.
- Existing-character selection and a received baseline.
- A private equality check between the selected label and received own label.
- Received position, current HP, and current MP.
- Unknown carried weight throughout the observation.
- Runtime join followed by one definitive `Closed` observation.
- Normal process exit.

The earlier [standalone startup probe](native-startup-current-build.md) used a
different implementation. This test verifies the production Rust boundary.
Neither test proves server logout, a complete inventory, or accepted gameplay.

## World-entry correction

The initial adapter omitted the empty outbound C5 world-entry frame after
accepted S5 and baselines S679, S230, and S233. The pinned
[inventory source report](native-inventory-baseline.md) identifies the normal
`GameSession.EnterWorld` send. The current source join is f13270 at `0x3ae562`.

The loopback regression timed out waiting for that frame before the fix.
After the fix, it received logical opcode 5 with an empty payload, then
confirmed cancellation and socket closure. Gateway text controls are allowed
while the test waits. The production live test above includes this correction.
C5 is a local milestone and supplies no inventory-completeness evidence.

## Retained evidence and limits

The outside-Git test directory is
`/var/folders/n9/1xyxxrwx4l3g6sfc89h47f9m0000gn/T/opentrickster-rust-startup-proof-vhf04qt9`.
It contains the probe, exact source copy, public preflight, regression logs,
sanitized result, and source/readback checks. Its one-attempt latch is consumed.
Reproduction with an account requires a new bounded session, not a rerun of
that consumed test directory.

The sanitized report SHA-256 is
`8c26f5e5c6a6eda9e162d53e1c74545a637f31fb2aa45406e4dda79a48239a2d`.
The probe source SHA-256 is
`4eccedc6ac305d52f46261379bdb753503a4e9b089b032031a18aa9e8fd2c02d`.
Credential-material scans passed against the probe, report, and runtime log.
The process was confirmed absent after exit. The result file has mode `0600`.

No movement, attack, dig, potion, or pickup was sent. The test did not establish
live inventory readiness, gameplay outcomes, reconnect continuity, or embedded
input and nonzero-socket cleanup. Independent specification and desktop reviews
remain unavailable after the review provider reached its usage limit.
