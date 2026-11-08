# Target gateway framing and opcode mapping

The goal is an implementation-ready specification for a macOS Rust/GPUI bot with an embedded game mode and a real clientless mode, controlling one character for combat and digging. This report answers [Recover target-build gateway framing and opcode mapping](https://github.com/oDestroyeRo/opentrickster/issues/13), part of [Plan the two-mode Trickster bot MVP](https://github.com/oDestroyeRo/opentrickster/issues/2). Quest automation is outside the MVP.

**Result: the bounded static question is answered.** The matching compiled client establishes a six-byte gateway frame header, stream reassembly, build-specific directional opcode permutations, text session controls, byte acknowledgements, and retained-byte resumption. Movement, basic attack, and manual dig start/cancel have concrete outbound serializers. This supports an offline codec specification. It does not establish native authentication, accepted gameplay, server replay guarantees, or a working clientless bot. [W], [M], locators below.

Research date: 2026-11-08. The clean checkout started at `6b8162f1f7a42b2bd9dd02c1b086be360914bb8d` on local branch `research/gateway-framing-opcodes`. The intended published ref is `research/target-protocol-framing`. The resolution comment records the final commit and verified remote SHA, since including a document's own commit SHA would change it. Before research, the authenticated developer and the ticket's existing assignee both matched `oDestroyeRo`. No reassignment was needed.

## Evidence and limits

"Extracted" describes bytes, metadata records, or instructions in the pinned public artifacts. "Derived" describes a transcription of those instructions, including the offline model and its computed examples. "Unknown" means the available evidence does not settle a field or behavior. It does not mean unsupported.

The [public-interface report][R1] supplied the artifact cache and starting leads. The [partial live report][R2] supplied context only. Its former page-memory captures were neither sought nor read. This investigation used public static GET responses, primary tool source, static disassembly, and synthetic local checks. It opened no browser or WebSocket, read no game credentials or browser storage, and executed no target JavaScript or WASM. All binaries, downloaded tools, generated disassembly, and fixtures remain outside Git in `/tmp/opentrickster-issue13`. The public cache is `/tmp/opentrickster-issue3`.

Fresh account-free retrievals used `User-Agent: Mozilla/5.0`, without cookies, between 14:49:26 and 14:49:36 UTC. All four resources returned HTTP 200 and matched the earlier cache and report exactly. The `.gz` names yielded already decoded JavaScript, Unity data, and WASM bytes. Hashes below apply to those received bodies. [L], [J], [D], [W]

| Artifact | Received bytes | SHA-256 |
| --- | ---: | --- |
| [Loader][L] | 27,914 | `de82b056302e1bfe1c0524e88b377535a508a9538fcc6a4f9533806c03d3548d` |
| [Framework][J] | 386,859 | `bd2fce60698f3cd563cc1d2a635afaf7045a4a6d78e9edd3f478f89c2e2993ca` |
| [Data package][D] | 27,239,767 | `f1ab3ef3d2b6b04b28104bac55b25c007e78a24242a56c354b640428daf3053d` |
| [WASM][W] | 42,317,618 | `77a05d62cc5cc6fd5d1a1f5b4f4d25d8c0d1f9e8ea4740c08326f8ec83e2b446` |
| Metadata extracted from data | 9,148,328 | `b7b7920f034cbdfb87eaf69557e17ea6b1513693acb9e905e918eaedbaf1a483` |

`global-metadata.dat` occupies data-package offset `16459759`, length `9148328`. Its magic is `0xFAB11BAF` and version is `108`. The identical artifact hashes preserve the earlier Unity `6000.6.5f1` identification. Independently, `GatewayBuild.Load` and its static initializer contain deployment build `f5e972fa2be429602c20d6168013a20f`. The visible revision `6ab10889` is from the earlier live report, not a fresh UI observation. These identifiers remain separate layers. [D], [W] functions 18144 and 18146; [R1], [R2]

## How names were connected to executable code

The metadata parser follows [Cpp2IL's version-108 readers][C1] at commit `b5ad444b82267cb1e4b88b8b373c008105bdea52`. This is tool-format evidence, not another game's protocol. The metadata header has offset, byte-size, and count triples. In this build, type definitions are 66 bytes, methods 30 bytes, fields 10 bytes, parameters 10 bytes, and images 54 bytes. Their declared counts and byte lengths agree. Variable-width indices must be honored. An older fixed-width metadata layout would assign incorrect names. [C1], [C2], [C3], [C4], [M]

The chain used for every target method was:

1. Decode its metadata name, declaring type, owning image, and method token.
2. Reconstruct WASM linear memory from the active data segments, without instantiating the module.
3. Locate the `Spinel.Net.dll` code-generation module at linear-memory address `0x20ad44`. Its first three words are name pointer `0x28b35`, method count `4729`, and method-pointer-array address `0x385d90`.
4. Index that array by `(method_token & 0x00ffffff) - 1`. Each entry is a function-table slot, not a WASM code address.
5. Resolve the slot through the WASM element section, then disassemble that function with WABT 1.0.39. A few methods share native bodies because of compiler deduplication. The specific target functions below have their own metadata-backed locators. [C5], [W], [T1]

For example, `OpcodeMap.FromSeed` is metadata method `69186`, token `0x060000f6`, array slot `245`, function-table entry `23033`, WASM function `18145`, body offset `0x4c268a`. `GatewayFrames.Encode` is method `69063`, token `0x0600007b`, table entry `22953`, function `18147`, body offset `0x4c2b31`. These are method-record and table joins, not symbol-string guesses. [M], [W]

Field names were checked against the binary field-offset table, not assigned from names alone. The v108 metadata-registration record is at memory `0x19e360`; its field-offset pointer array is `0x351ee0`, with `15196` entries. `GatewayLink`'s offset row is `0x151190`. It places `received`, `sent`, `acked`, `ackSent`, `retained`, and `headOffset` at object offsets `72`, `80`, `88`, `96`, `104`, and `108`. `OpcodeMap`'s row at `0x151300` places `wire` and `logical` at `8` and `12`. [C6], [M], [W]

The distinction between transport implementations matters. `Connection` checks whether its transport implements `IGatewayTransport`. That branch constructs `GatewayFrames` with `GatewayBuild.Send` and `GatewayBuild.Receive`. The other branch constructs `FrameCodec`, which has different header/sequence machinery. `GatewayLink`'s metadata includes `IGatewayTransport`, and the `Spinel.Engine.Unity.Web.JsLinkSockets` transport factory calls its constructor. The generic `FrameCodec` is therefore not the framing specification for the inspected gateway path. [W] `Connection..ctor` 18118 at `0x4bfe5f`, especially `0x4bfff3–0x4c00dd`; web factory 34050 at `0xa6dff1`, constructor call `0xa6e12b`; [M] type 11146 interface list includes type index 18218, `IGatewayTransport`.

## Binary gateway frames

Both directions use this envelope before application payload parsing. Byte order is little-endian. The maximum payload is `32745` bytes, so the maximum complete frame is `32751` bytes. [W] `GatewayFrames.Encode` 18147, `0x4c2b5b–0x4c2bc9`; `Push` 18149, `0x4c2d31–0x4c2d94`; [WASM]

| Offset | Width | Extracted meaning |
| ---: | ---: | --- |
| 0 | 2 | Unsigned payload length `N`, excluding the six-byte header. |
| 2 | 2 | Wire opcode. Outbound uses the `c2s` permutation. Inbound uses the inverse of the `s2c` permutation. |
| 4 | 2 | `NetFrame.Aux`. Outbound `Encode` leaves these bytes zero in a new byte array. Inbound preserves the unsigned value. Its application meaning is unresolved. |
| 6 | `N` | Unmodified payload bytes. |

`Encode` allocates `N + 6`, writes the length and translated opcode, and copies the payload to offset 6. No checksum, sequence number, or additional encryption step appears in this gateway encoder. This statement concerns this envelope only; individual payloads can contain their own check fields. A null opcode map has an identity fallback, but the pinned build initializer supplies both maps. [W] 18147; 18144 at `0x4c2642–0x4c2683`.

`Push` concatenates its retained suffix with each new byte array. It waits until at least six bytes exist, rejects a declared `N >= 32746`, waits for all `N + 6` bytes, emits a `NetFrame`, and repeats. It retains any incomplete suffix. A WebSocket message can therefore contain a fragment, one frame, several frames, or the end of one frame followed by another fragment. There is no evidence for treating each WebSocket message as one game packet. [W] 18149 at `0x4c2c9d–0x4c2dfd`.

`GatewayLink.Binary` counts the complete incoming binary array length in `received` and passes the array to the `Received` delegate only for the current live socket in the Live state. It does not strip a second binary header. The framework passes browser binary messages as byte arrays and routes text to another callback. `Connection.Receive` invokes the codec's `Push`, and `Connection.Send` invokes `Encode` before calling the transport. [W] 18177 at `0x4c4e3a`, 18127 at `0x4c0ed8`, 18045 at `0x4ba5cd`; [J] `_SpinelSocketOpen`, `_SpinelSocketSend`.

`PacketWriter.U8`, `U16`, `U32`, and `U64` emit the low 1, 2, 4, and 8 bytes respectively, in little-endian order. The unsigned readers check availability, advance their offset, and decode those widths. `I16` and `I32` provide signed interpretations. The extracted movement and digging serializers below use these helpers. General string encodings and all application-specific payload variants were not exhaustively traced. [W] 18050, 18049, 18225, 18065; 18041, 18031, 18033, 18032, 18228, 18229.

## Opcode remapping

The seed is a **public compiled build constant**, not an account secret or session token. `GatewayBuild.Load` returns the build ID above and this Base64 string:

```text
NPcmWWBZO27/4nZF0wrlUh+9r1909yn/jWTSUQFCnGM=
```

The string is metadata literal `9538`, metadata byte offset `0x44a49`, length `44`. The initializer decodes it to 32 bytes and calls `FromSeed(seed, "c2s")` and `FromSeed(seed, "s2c")`. The build ID is not appended to the hash input. It is carried separately in the bridge URL. No per-session seed negotiation appears in this construction path. [W] 18144 at `0x4c25cf–0x4c2683`, 18146 at `0x4c2aed`; [M]

The following algorithm is derived directly from function 18145:

- Reject a seed shorter than 16 bytes.
- Compute SHA-256 over `seed || ASCII(direction)`, with no separator.
- Interpret the first 16 digest bytes as four little-endian unsigned 32-bit words `a, b, c, d`. The `BitConverter` helper compiles to an unaligned WASM 32-bit load.
- Initialize `wire[i] = i` for every value from 0 through 65535.
- For `i` descending from 65535 through 1, compute the unsigned result `rotl32(b * 5, 7) * 9`, wrapping both multiplications at 32 bits. Set `j = result % (i + 1)` using unsigned remainder, swap `wire[i]` and `wire[j]`, and advance the four-word generator as shown below.
- Fill the inverse table with `logical[wire[i]] = i`.
- `Wire` and `Logical` mask the supplied index to 16 bits. [W] `0x4c26cf–0x4c2770`, `0x4c28c4–0x4c28ee`, `0x4c294c–0x4c2ad0`; helper 71277 at `0x17ef3b9`; accessors 18148 and 18150; [WASM]

This is a Fisher-Yates permutation driven by the arithmetic commonly called xoshiro128**. The instruction transcription below is the contract; recognizing the generator name is not the proof.

```python
import hashlib
import struct

MASK = 0xffffffff

def rotl(x, n):
    x &= MASK
    return ((x << n) | (x >> (32 - n))) & MASK

def opcode_map(seed, direction):
    if len(seed) < 16:
        raise ValueError("short seed")
    digest = hashlib.sha256(seed + direction.encode("ascii")).digest()
    a, b, c, d = struct.unpack("<4I", digest[:16])
    wire = list(range(65536))
    for i in range(65535, 0, -1):
        j = ((rotl((b * 5) & MASK, 7) * 9) & MASK) % (i + 1)
        wire[i], wire[j] = wire[j], wire[i]
        a, b, c, d = (
            (a ^ b ^ d) & MASK,
            (b ^ a ^ c) & MASK,
            (c ^ a ^ (b << 9)) & MASK,
            rotl(b ^ d, 11),
        )
    logical = [0] * 65536
    for i, value in enumerate(wire):
        logical[value] = i
    return wire, logical
```

The tuple assignment uses the old state for all four new words. In the compiled shuffle, the random output and swap occur before the state transition. Advancing the state before computing the random output changes the map. The loop uses unsigned modulo, including its resulting distribution bias. It does not use rejection sampling. [W] `0x4c29d4–0x4c2a30`.

Derived reproducibility fingerprints are SHA-256 over the complete `wire` table serialized as `struct.pack("<65536H", *wire)`:

| Seed | Direction | Table SHA-256 |
| --- | --- | --- |
| Synthetic bytes `00 01 ... 1f` | `c2s` | `c87a5dc1ad4dc4657d8ad4ababf6bb39e83daddb12cf61c76e50e89f65e6edab` |
| Synthetic bytes `00 01 ... 1f` | `s2c` | `20a1907c6cadf8a2cf4ac1c36f5dbf36b8a086d08958eec0071ffec8a0663a8a` |
| Public build seed above | `c2s` | `9c6c4259e794f31bd9da7aedb8b6df703ab054a36c9ae12483bce4900fd9e8f3` |
| Public build seed above | `s2c` | `2b9c109efc2bb4c5b6526470e424d23602bc0e0394598604d765b248e77ebaa6` |

These are model results, not a comparison with an executed game client or a server. They are appropriate regression vectors for an independent implementation after checking the cited instructions.

## Text control and session lifecycle

The binary stream has a separate text control channel. `GatewayLink.Text` splits text on ASCII space using `String.Split` with options zero. The supported forms below are extracted from actual branches and formatting calls. Placeholders are notation, never captured values. [W] 18178 at `0x4c4f79–0x4c5380`.

| Direction | Form | Client behavior established by code |
| --- | --- | --- |
| Server to client | `k <token> <holdSeconds>` | Exactly three split fields, from the live socket. Saves or rotates the remembered token. Parses the final field with invariant `Double.TryParse` and `NumberStyles.None`, then stores `holdSeconds * 1000` as `holdMs`. No actual hold duration was observed. |
| Server to client | `a <gatewayReceived>` | Exactly two fields, from the live socket. Parses an invariant Int64 with style zero. Clears a pending probe, can record round-trip timing, and calls `Confirm` when resumption is enabled. |
| Client to server | `a <received>` | Acknowledges cumulative bytes received in binary messages. It is a transport byte count, including frame headers and any currently incomplete frame. |
| Client to server | `p` | A probe emitted by `Probe`. This is an application text message, not a WebSocket ping frame. |
| Client to server, on reconnect | `r <token> <received>` | Requests resumption using the stored token and the client's cumulative receive byte position. |
| Server to client, on reconnect | `r <gatewayReceived> <newToken>` | Exactly three fields, from the attempt socket while Resuming. Calls `Resumed` with the parsed byte position and replacement token. The field order differs from the client's `r` request. |
| Server to client | `g <reason text>` | Invalidates the token and fails the link. The implementation takes the reason from character offset 2 in the original text. `server` has a distinct close-reason branch. |
| Server to client | `v <buildId>` | Exactly two fields. Passes the ID to `BuildUpdate.Offer`. |

Unknown messages or malformed recognized forms fall through to link failure. Empty text, stale socket sources, and a closed link are filtered earlier. The numeric parser uses style `0`; do not assume signs, decimal fractions, exponents, or surrounding whitespace are accepted just because one destination field is a `double`. [W] 18178, especially `0x4c503d–0x4c5096`, `0x4c50ce–0x4c50df`, `0x4c527c–0x4c52a2`; [N1]

Initial connection URL construction is:

```text
<bridge>?host=<escaped endpoint host>&port=<endpoint port>&b=<buildId>&r=1&u=1
```

Reconnect uses:

```text
<bridge>?resume=1&u=1&b=<buildId>
```

The token is subsequently sent in the `r` text message, not placed in that reconnect URL. `BridgeUrl` omits `&b=` when the build ID is absent; it is present in this build. The bridge base derives from the page origin and directory, giving `wss://tr-play.playall.in.th/net` at the inspected root. The exact strings are proven. The server's interpretation of the flag names, endpoint allowlist, and login endpoint selection are not established here. No URL above was used to open a session. [W] 18163 at `0x4c37fe`, 18162 at `0x4c3678`, 18172 at `0x4c489b`, 18176 at `0x4c4d64–0x4c4db0`; [J] `_SpinelSocketBridgeUrl`.

The metadata's states are Idle, Connecting, Live, Lost, Resuming, and Closed. Their compressed Int32 defaults at metadata offsets `0x4cadd8` through `0x4caddd` decode to numeric values 0 through 5 in that order, matching the compiled branches. Socket open advances Connecting to Live and resolves connection completion. It does not wait for `k` before declaring the transport open. `resumable` starts true. If no token arrives within 5000 ms of opening, `Tick` disables resumption, clears retained outbound bytes, and sets `acked = sent`. A native adapter must not promise resumption merely because the socket opened. [W] 18155 at `0x4c3134–0x4c313c`, 18176 at `0x4c4cda–0x4c4d49`, 18170 at `0x4c435b–0x4c43ad`; [M]

When resumption is enabled, `Send` retains the already encoded binary arrays and increments `sent` by their byte lengths. The client fails if `sent - acked > 1048576`. It writes immediately only in Live state; bytes retained during Lost or Resuming remain for resumption. This is a client-side retention cap, not proof of the server's replay-window capacity. [W] 18165 at `0x4c39bf–0x4c3ae7`.

`Confirm(n)` accepts stale `n < acked` without decreasing `acked`, rejects `n > sent`, and otherwise removes exactly `n - acked` retained bytes. It can consume part of the first retained array and updates `headOffset`. Acknowledgements therefore need not align to frame boundaries. `Resumed(n, token)` is stricter: it requires `acked <= n <= sent`, confirms through `n`, remembers the new token, promotes the attempt socket to live, sets `ackSent = -1`, and sends `Unconfirmed()` as the remaining byte suffix. It does not rebuild action packets or reset the frame decoder. [W] 18179 at `0x4c53bf–0x4c5468`, 18180 at `0x4c54f6–0x4c56f1` and `0x4c5810–0x4c58b0`, 18182 at `0x4c595f`.

For a synthetic counter example, retain byte arrays of lengths 8 and 6, with `sent = 14`, `acked = 0`. Confirmation at byte 5 retains the last 3 bytes of the first array plus all 6 bytes of the second. Resumption at 5 sends that 9-byte suffix. A stale ordinary acknowledgement at 4 is ignored, but a resumption reply at 4 is rejected because those bytes have already been discarded. This follows the retained-byte operations, not an observed server exchange. [W] 18179, 18180, 18182.

In the token-bearing Live branch, `Tick` sends `a <received>` after at least 1000 ms if the count changed, or after 10000 ms even if unchanged. A pending probe that receives no answer within 5000 ms causes link loss. Browser scheduling can delay callbacks, so these constants are client thresholds rather than guaranteed wall-clock transmission intervals. Retry scheduling, visibility handling, and close-code distinctions exist but were not exhaustively transcribed into a complete state machine. [W] 18170 at `0x4c43b3–0x4c44b7`; [J] bridge tick setup.

Release has two paths. `GatewayLink.Close` closes its sockets and forgets a remembered token. When closing from Lost or Resuming with a token, it also calls `ILinkSockets.Release`. The framework's release implementation sends the token as the raw body of a relative `net/release` POST with `keepalive: true`; unload first tries `sendBeacon`. Browser page exit releases remembered links and closes sockets. This establishes client cleanup attempts only. HTTP response interpretation, server release guarantees, token lifetime and entropy, and character ownership after release remain unknown. [W] 18168 at `0x4c4115–0x4c4230`; [J] byte offsets `113979` for `tokens`, `114301` for `release`, `114814` for `_SpinelLinkRelease`, and `115079` for `_SpinelLinkRemember`.

A successful bridge send means that the browser accepted a local `send` call. `a` and `r` acknowledge gateway byte positions. Neither is a monster-hit, dig-completion, inventory, or movement-success acknowledgement. No server code or authoritative action result was established by this slice. [J] `_SpinelSocketSend`, `_SpinelSocketSendText`; [W] 18178–18182.

## Bounded action serializers

These are compiled outbound contracts, not a complete action API. Logical opcodes are operands passed to `Send`; the wire values below are derived using this exact public build's `c2s` map. They must be regenerated for a changed build. `IGameSession.Send` is interface slot 3. Its concrete `GameSession.Send` reaches `GameServerLink.Send`, which reaches `Connection.Send`, then the gateway codec and transport. It can defer sends during handoff, so reaching a high-level action method alone does not prove immediate transmission. [M] interface type 11857; [W] 16202 at `0x448911`, 16175 at `0x4465b5`, 18045 at `0x4ba5cd`.

| Compiled operation | Logical opcode | Derived wire opcode | Payload, in order |
| --- | --- | --- | --- |
| `GameSession.SendMove(from, to, mode)` | `0x0018` | `0xf36e` | `u16(round(from.X))`, `u16(round(from.Y))`, `u16(to.X)`, `u16(to.Y)`, `u8(mode)`. Nine bytes. |
| First send inside `CombatLink.Attack(target, moveMode)` | `0x0120` | `0x1094` | `u16(target)`. Two bytes. The gameplay meaning of this preliminary send is not assigned here. |
| Second send inside `CombatLink.Attack(target, moveMode)` | `0x001d` | `0x2fc9` | `u16(target)`, `u8(moveMode)`. Three bytes. |
| `DrillFeature.Press` through `Drill.DigPayload(x, y, mode)` | `0x002c` | `0x766c` | `u16(x)`, `u16(y)`, `u8(mode)`. Five bytes. |
| `DrillFeature.Cancel` through `Drill.CancelPayload(0, 0)` on its send branch | `0x0030` | `0x2edc` | `u16(0)`, `u16(0)`. Four bytes for this caller. The reusable helper parameters are named `starts` and `elapsedMs`. |

Movement proof is `RequestMove` 16218 calling `SendMove` 16219 at `0x449ad9`. The serializer at `0x449b45–0x449b94` calls `GameServerLink.Send`. Metadata identifies `from` as `MapPoint`, with double X/Y fields, and `to` as `Pos`, with Int32 X/Y fields. `JsMath.Round` 18195 computes `floor(value + 0.5)` before integer conversion; it is not .NET's default midpoint-to-even rounding. The serialized integer writers retain low-order bits. Coordinate units, map origin, allowed mode values, path validation, and authoritative movement correction remain unverified. [M], [W] 16218, 16219, 18195 at `0x4c62b1–0x4c62be`.

Basic-attack proof is `CombatModel.WorldClick` 23410 calling `CombatLink.Attack` at `0x6b4c97`. `Attack` 17980 constructs and submits both payloads above, in that order, at `0x4b5f28` and `0x4b5fde`. `AttackOnly` 17981 sends only the second payload, and its inspected caller is `CombatModel.AttackPlayer`; it must not be substituted for the ordinary monster-click path without further proof. Target selection validity, reach, cooldowns, damage, refusals, and attack cancellation are not settled here. [W] 23410, 17980, 17981; [M] parameter records name `target` and `moveMode`.

Digging proof is `DrillFeature.Press` 25079 calling `DigPayload` 15945 at `0x75d816` and passing opcode 44 to session Send at `0x75d827`. The payload helper writes two U16 values then one U8. `DrillFeature.Cancel` 25074 calls `CancelPayload` 15946 with zeros at `0x75cfbf`, then passes opcode 48 at `0x75cfd0`. Other callers can populate the reusable cancel fields differently. Automatic digging also has a separate `AutoSpotPayload` path; this report does not equate the earlier live auto-dig UI with the manual `Press` serializer. Dig-end checks, automatic-spot selection, accepted start/cancel results, empty/found results, drill durability, and inventory changes remain outside the recovered action contracts. [W] 25079, 15945, 25074, 15946; [R2]

## Offline checks and synthetic bytes

For synthetic seed `bytes(range(32))`, direction `c2s`, logical opcode `0x1234`, and payload `aa bb`, the model emits:

```text
02 00 45 39 00 00 aa bb
```

The length is 2; the derived wire opcode is `0x3945`; Aux is zero. This is an artificial codec fixture with no assigned gameplay meaning. Splitting it after any byte, supplying it one byte at a time, or concatenating it with another synthetic frame must preserve the decoded values. The following decoder model expresses the extracted reassembly rule. It is not an executable client or a networking tool. [W] 18147, 18149.

```python
def encode(opcode, payload, wire):
    if len(payload) > 32745:
        raise ValueError("oversize")
    return struct.pack("<HHH", len(payload), wire[opcode & 65535], 0) + payload

class Frames:
    def __init__(self, logical):
        self.logical = logical
        self.pending = b""

    def push(self, chunk):
        self.pending += chunk
        frames = []
        while len(self.pending) >= 6:
            length, wire_opcode, aux = struct.unpack_from("<HHH", self.pending)
            if length > 32745:
                raise ValueError("oversize")
            if len(self.pending) < length + 6:
                break
            frames.append((self.logical[wire_opcode], aux, self.pending[6:6+length]))
            self.pending = self.pending[6+length:]
        return frames
```

The local checks passed for all 65,536 inverse mappings in both directions for both seeds, all two-chunk splits of a two-frame synthetic stream, one-byte delivery, coalesced frames, an empty payload, a 32745-byte payload, rejection of a 32746-byte declaration, and rejection of a 15-byte seed. The fixture uses one direction's map and its inverse to test codec consistency. Production inbound decoding must use the inverse **s2c** map, not the c2s map used to encode outbound bytes.

These checks validate the derived model's consistency and failure cases. They do not independently prove equivalence to every instruction, and no target method was executed. WABT's static validation of the complete pinned WASM also passed with `--enable-all`. Static review compared the model's hash input, little-endian state reads, wrapping multiplications, rotation counts, unsigned remainder, descending loop, inverse table, header writes, and buffering branches to the exact offsets cited above.

## Unresolved fields and smallest verification

| Remaining question | Missing evidence |
| --- | --- |
| `Aux` application meaning | Trace its uses in a selected inbound handler, then correlate a decoded frame with the corresponding visible state. It is not proven to be a sequence number, result status, or acknowledgement. |
| Complete login and GameServer handoff | Target-matched analysis of the separate authentication and ticket paths, plus an independently authorized runtime correlation. This report recovers transport construction, not the authentication schema. |
| Server handling of byte acknowledgements and resumption | Server source or a bounded interruption/resumption observation showing both byte counters, retained suffixes, and resulting state. The client's intent does not prove server deduplication or exactly-once gameplay. |
| Release and ownership | A separately authorized release/reconnect check with a single session owner. A browser beacon attempt is not a release confirmation. |
| Complete retry state machine | Finish `Tick`, `Lose`, `Retry`, `Ended`, and visibility branches using the recovered symbols. This remains account-free static work; no credential is required. |
| Action results and dashboard authority | Trace inbound movement, combat, drill, HP, entity, and inventory handlers, then correlate decoded outcomes with the same character and build. |
| Automatic digging | Follow `AutoSpotPayload`, automatic-spot handlers, and completion paths. The manual dig serializer is not proof of the earlier auto-dig sequence. |
| Native clientless feasibility | An authenticated native session with Unity absent, authoritative state, one bounded accepted action, and verified shutdown. No such demonstration ran here. |

The smallest next check is an independent offline reproduction of the two public-build table fingerprints and synthetic frame above from this report's cited methods. The next runtime check, under separate authorization, is one harmless movement in an existing test session on the same fingerprinted build: privately decode its outgoing `0x0018` frame and a corresponding inbound position event, then compare the decoded coordinates with the visible client. Retain authentication and session tokens privately; publish only the field-level conclusion. Resumption and digging completion require separate checks after that correlation. No new account, secret, or live session is needed to reproduce this static report.

The bounded research ticket can close because core framing and remapping are supported. The parent specification must keep clientless operation and authoritative action outcomes unverified until the remaining evidence exists.

## Reproduction and locator reference

Use an isolated scratch directory. Fetch the four public URLs with a browser user agent and compare the received-body SHA-256 values before analysis. Extract metadata using the verified package interval. The minimal locator script below reconstructs only public static data and resolves the core methods. It is intentionally tied to this fingerprint and metadata layout. It does not load or execute the WASM module.

```python
import hashlib
import pathlib
import struct
import sys

data = pathlib.Path(sys.argv[1]).read_bytes()
wasm = pathlib.Path(sys.argv[2]).read_bytes()
assert hashlib.sha256(data).hexdigest() == "f1ab3ef3d2b6b04b28104bac55b25c007e78a24242a56c354b640428daf3053d"
assert hashlib.sha256(wasm).hexdigest() == "77a05d62cc5cc6fd5d1a1f5b4f4d25d8c0d1f9e8ea4740c08326f8ec83e2b446"
meta = data[16459759:16459759 + 9148328]
assert struct.unpack_from("<II", meta) == (0xFAB11BAF, 108)

p = 8

def u():
    global p
    value = shift = 0
    while True:
        byte = wasm[p]
        p += 1
        value |= (byte & 127) << shift
        shift += 7
        if byte < 128:
            return value

sections = {}
while p < len(wasm):
    kind = wasm[p]
    p += 1
    size = u()
    sections[kind] = p
    p += size

p = sections[2]
imports = u()
for _ in range(imports):
    for _ in range(2):
        size = u()
        p += size
    assert wasm[p] == 0
    p += 1
    u()
assert imports == 484

p = sections[9]
assert u() == 1 and u() == 0 and wasm[p] == 0x41
p += 1
start = u()
assert wasm[p] == 0x0b
p += 1
elements = [None] * start + [u() for _ in range(u())]

memory = bytearray(64 * 1024 * 1024)
p = sections[11]
for _ in range(u()):
    assert u() == 0 and wasm[p] == 0x41
    p += 1
    address = u()
    assert wasm[p] == 0x0b
    p += 1
    size = u()
    memory[address:address + size] = wasm[p:p + size]
    p += size

p = sections[10]
bodies = {}
for function in range(imports, imports + u()):
    size = u()
    bodies[function] = p
    p += size

strings = struct.unpack_from("<III", meta, 8 + 12 * 2)[0]
methods, method_bytes, method_count = struct.unpack_from("<III", meta, 8 + 12 * 5)
types, type_bytes, type_count = struct.unpack_from("<III", meta, 8 + 12 * 19)
assert method_bytes == 30 * method_count and type_bytes == 66 * type_count

def name(index):
    begin = strings + index
    return meta[begin:meta.index(b"\0", begin)].decode()

module_name, count, pointers = struct.unpack_from("<III", memory, 0x20ad44)
assert memory[module_name:module_name + 15] == b"Spinel.Net.dll\0"
assert count == 4729 and pointers == 0x385d90
owners = {"GatewayBuild", "GatewayFrames", "GatewayLink", "OpcodeMap", "PacketWriter", "PacketReader"}
for index in range(method_count):
    row = struct.unpack_from("<IHHIIHIHHHH", meta, methods + index * 30)
    type_row = struct.unpack_from("<IIHHHHIIIHHHHIHHHHHHHHHII", meta, types + row[1] * 66)
    owner = name(type_row[0])
    if owner not in owners or name(type_row[1]) != "Spinel.Net":
        continue
    token = row[6]
    slot = (token & 0xffffff) - 1
    assert slot < count
    table = struct.unpack_from("<I", memory, pointers + 4 * slot)[0]
    function = elements[table]
    print(index, f"{owner}.{name(row[0])}", hex(token), table, function, hex(bodies[function]))
```

Save that block outside the repository as `locate_core.py` and supply the received data and WASM paths. Its output includes `69186 OpcodeMap.FromSeed 0x60000f6 23033 18145 0x4c268a`. The method token's high byte is conventionally padded as `0x060000f6`; these are the same value.

For disassembly, the inspected tool was the published `wabt` package version `1.0.39`, installed under the scratch directory with install scripts disabled. Its upstream tool documentation describes `wasm-objdump` and `wasm-validate`. [T1], [T2]

```sh
npm install --prefix /tmp/opentrickster-issue13/tools --ignore-scripts --no-audit --no-fund wabt@1.0.39
/tmp/opentrickster-issue13/tools/node_modules/.bin/wasm-objdump -h /tmp/opentrickster-issue3/657890b2bd8548390dd7c7fad76093be.wasm.gz
/tmp/opentrickster-issue13/tools/node_modules/.bin/wasm-objdump -d /tmp/opentrickster-issue3/657890b2bd8548390dd7c7fad76093be.wasm.gz > /tmp/opentrickster-issue13/module.dis
/tmp/opentrickster-issue13/tools/node_modules/.bin/wasm-validate --enable-all /tmp/opentrickster-issue3/657890b2bd8548390dd7c7fad76093be.wasm.gz
```

The disassembly is about 512 MiB and belongs outside the repository. Locate a method with `rg 'func\[18145\]' /tmp/opentrickster-issue13/module.dis`, then read that function through its ending `end`. The module's Code section starts at file offset `0x6cf18`; the Data section starts at `0x24bff0e`. A WASM function-body offset is a file offset, while object/module/field addresses above refer to reconstructed linear memory. Metadata offsets refer to the extracted metadata file, not to the containing Unity package. [W]

String annotations can be reproduced without running IL2CPP. An unresolved metadata-usage placeholder word has a low-bit marker. For this v108 build, a string-literal usage has `word >> 29 == 4`, and its literal index is `(word & 0x1fffffff) >> 1`. The literal table contains four-byte offsets; adjacent offsets delimit the UTF-8 literal bytes. For example, memory `3415732` initially contains `0x80004a85`, which selects literal `9538`, the public build seed. The metadata reader's version-specific usage shift is necessary because v106.1 removed one usage kind. [C7], [M], [W]

Core type and function locators are recorded here. A function number always includes the module's 484 imported functions.

| Owner or method | Metadata index or token | WASM function and body file offset |
| --- | --- | --- |
| `GatewayBuild` type | Type 11134, token `0x02000027`, metadata offset `0x7ae2a4` | Static initializer 18144, `0x4c2596` |
| `GatewayFrames` type | Type 11135, token `0x02000028`, metadata offset `0x7ae2e6` | Encode 18147, `0x4c2b31`; Push 18149, `0x4c2c5b` |
| `GatewayLink` type | Type 11146, token `0x0200002c`, metadata offset `0x7ae5bc` | Send 18165, `0x4c397b`; Close 18168, `0x4c3f29` |
| `GatewayLink.Tick` | Method 69091, token `0x06000097` | 18170, `0x4c42cf` |
| `GatewayLink.Opened` | Method 69094, token `0x0600009a` | 18176, `0x4c4c72` |
| `GatewayLink.Binary` | Method 69095, token `0x0600009b` | 18177, `0x4c4e3a` |
| `GatewayLink.Text` | Method 69096, token `0x0600009c` | 18178, `0x4c4e97` |
| `GatewayLink.Confirm` | Method 69098, token `0x0600009e` | 18179, `0x4c538b` |
| `GatewayLink.Resumed` | Method 69100, token `0x060000a0` | 18180, `0x4c547c` |
| `GatewayLink.Unconfirmed` | Method 69099, token `0x0600009f` | 18182, `0x4c595f` |
| `OpcodeMap` type | Type 11163, token `0x02000044`, metadata offset `0x7aea1e` | FromSeed 18145, `0x4c268a`; Wire 18148, `0x4c2c44`; Logical 18150, `0x4c2e78` |
| `PacketWriter` type | Type 11164, token `0x02000045`, metadata offset `0x7aea60` | U16 18049, `0x4babff`; U32 18225, `0x4c811c`; U64 18065, `0x4bc093` |
| `PacketReader` type | Type 11166, token `0x02000047`, metadata offset `0x7aeae4` | U16 18031, `0x4b9547`; U32 18033, `0x4b96f7`; U64 18032, `0x4b961f` |
| `GameSession.SendMove` | Method 70561, token `0x06000655` | 16219, `0x449af8` |
| `CombatLink.Attack` | Method 69842, token `0x06000386` | 17980, `0x4b5ec2` |
| `Drill.DigPayload` | Method 70164, token `0x060004c8` | 15945, `0x4399ff` |
| `Drill.CancelPayload` | Method 70165, token `0x060004c9` | 15946, `0x439a56` |

## Validation and delivery scope

The evidence review checked the ticket assignment, clean starting tree, artifact fingerprints, metadata bounds and record sizes, code-module and element-table joins, binary field offsets, and cited function bodies. The standalone locator was run against the original public data and WASM. The documented Python model was checked with synthetic inputs, including failure paths. Whole-module static WASM validation passed. No application build or live gameplay test applies to this documentation-only change.

Full report readback, whitespace and citation-reference checks, and rerunning the embedded Python examples passed. The sole repository change is this report. Publication is limited to a one-file commit on a throwaway research ref, followed by committed-report readback, remote-SHA verification, and one resolution comment on the assigned ticket. The resolution comment records the actual publication result. No PR, merge, parent-map edit, other-ticket edit, or live account activity is part of this delivery.

The Prove It Works principle shaped the verification: fresh artifact hashes replaced cache assumptions, metadata/table joins replaced symbol-string guesses, and the offline example was run rather than merely described. The remaining server and gameplay claims are explicitly unrun.

## Primary sources

The target URLs are mutable. The fingerprints and offsets above identify the inspected revision. Tool source links are pinned to the inspected commit. Tool behavior is supporting evidence; the protocol claims derive from the target's own artifacts.

[L]: https://tr-play.playall.in.th/Build/877598568ac7f0dc41389b0d6e411b8b.loader.js
[J]: https://tr-play.playall.in.th/Build/4937d6d47bade23f56e5ad4b5bf2e2b9.framework.js.gz
[D]: https://tr-play.playall.in.th/Build/9b2286992122d3c1cc081492dc31fee5.data.gz
[W]: https://tr-play.playall.in.th/Build/657890b2bd8548390dd7c7fad76093be.wasm.gz
[M]: https://tr-play.playall.in.th/Build/9b2286992122d3c1cc081492dc31fee5.data.gz
[R1]: https://github.com/oDestroyeRo/opentrickster/blob/6d996ef87a75bd8857ee998ddf30dcd99b80c1bf/docs/research/target-game-interfaces.md
[R2]: https://github.com/oDestroyeRo/opentrickster/blob/136cde54986c59715da061cf7a7608cb10fbe32a/docs/research/live-game-protocol.md
[C1]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppMetadata.cs
[C2]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppGlobalMetadataHeader.cs
[C3]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppTypeDefinition.cs
[C4]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/Metadata/Il2CppMethodDefinition.cs
[C5]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/BinaryStructures/Il2CppCodeGenModule.cs
[C6]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/BinaryStructures/Il2CppMetadataRegistration.cs
[C7]: https://github.com/SamboyCoding/Cpp2IL/blob/b5ad444b82267cb1e4b88b8b373c008105bdea52/LibCpp2IL/MetadataUsage.cs
[T1]: https://github.com/WebAssembly/wabt
[T2]: https://github.com/AssemblyScript/wabt.js
[WASM]: https://webassembly.github.io/spec/core/exec/numerics.html
[N1]: https://learn.microsoft.com/en-us/dotnet/api/system.globalization.numberstyles?view=net-9.0
