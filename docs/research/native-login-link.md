# Native login link through the game session

Build **`ea1a68bd89958fae94179f1e0835ffb5`**, 2026-11-09. This report covers one authorized existing character, one session at a time. It contains no credentials, payloads, UIDs or character values.

**Result.** The native session lost its game link about 0.1 s after world entry because it had already closed its login-server (LS) link. The real client keeps that link open and pumped for the whole game session. With the LS link kept open, one live native boot stayed connected for the full 60 s idle window after its baseline.

## Static evidence

Every method below is on build ea1a68bd and is identified by its function index.

- **The client keeps the LS link open.**
  - `Characters/<SelectCharacterAsync>d__14.MoveNext` f9882 has no `Connection.Close`.
  - `Session/<EnterWorldAsync>d__26.MoveNext` f10062 closes only the first-login link, at `0x2f2591`. It installs `On(2028)` (f10060) and `On(5509)` (f10061) on the LS link and leaves that link open.
- **Only session close ends the LS link.** `Session.Close` f10014 is the only closer of `Login`. Its callers include `GameSession.Close` f20918, which runs after the game-server link closes.
- **The client keeps serving the LS link in game.** `Session.Pump` f10048 pumps the LS link every frame, so its 2028 listener keeps answering during play.
- **2028 belongs to the LS link only.** `i32.const 2028` occurs only in the LS handler f10060 and its registration in f10062. `GameSession.Handle` f20915 has no 2028 case, so the client never answers 2028 on the game link.
- **Nothing the client sends at world entry keeps the session alive.** After C5, `GameSession.EnterWorld` f20891 flushes held feature requests (511, 28449, 1189). Later timers send 28419, 752, 28426 and 28435. None of them is a prerequisite for staying connected, and none is an anti-cheat or attestation handshake.
- **The client's LS-only system-server request is not sent.** `StartSystemServer` f20893 sends C5508 on the LS link, and its reply opens a third link. The native session does not send 5508.

## Change

`NativeSession` now does the following:

- It keeps the LS link open until the game link closes.
- It pumps the LS link during startup and in game, answers 2028 there, and drops the LS link's other frames.
- It never answers 2028 on the game link.
- It closes the game link first, then the LS link.

LS frames use their own frame order, so game-link marks are unchanged.

## Verification

- **Synthetic.** The loopback session test runs a separate LS link and game link. It asserts the following:
  - the first game-link frame after startup is C5, even though a game-link 2028 was received first;
  - the LS link still answers 2028 with 4 bytes after world entry;
  - the game link does not answer a 2028 sent after world entry;
  - both links close.

  These four mutations each fail the test: no LS reply, a game-link reply during startup or in game, and an unpumped LS link.
- **Live.** One native-only boot ran production Net at the tested commit, with the #46 decoder fixes applied privately. It reached the baseline, then stayed connected for 60 000 ms with no failure, loss or close event. It sent no gameplay, and its runtime joined on close. HP, MP and weight maxima stayed unknown (#47).

## Limits

The cause on the server side is inferred: the server did not explain the disconnect. The live result is one idle session. It does not prove reconnect behavior, LS-link loss during play, or the client-only system-server link.
