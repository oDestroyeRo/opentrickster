# Native gun controller sources

Account-free static research for the native gun controller in `crates/net`, observed **2026-11-09 UTC** on build **ea1a68bd89958fae94179f1e0835ffb5**. It extends the [gun combat report](gun-combat-control.md), which describes build 8502ebbf. This report re-derives the current joins that the controller relies on and records the source facts that report did not state. Its function indices are ea1a68bd values; the indices in the earlier report are 8502ebbf values.

Evidence classes follow the [migration report](build-ea1a68bd-migration.md): **source** means static decoding of the current public artifacts, **synthetic** means checks on invented fixtures. No live acceptance is claimed.

## Provenance and limits

The bodies were read from an outside-Git `tools/build-migrate` cache of the public build, whose assets match the migration report's pins (WASM `6396dfe9…`, Unity data `9c68dd2a…`). They were printed as normalized tokens through `buildmigrate.derive`, and their original bytes were hashed. No target WASM or JavaScript was executed. No game socket was opened, no account was used, and no credential, capture or character value was read.

`python3 -I tools/build-migrate/migrate.py check CACHE --rust crates/net/src` matched all 400 signed methods, including the gun methods signed from the earlier report's citations, and every Rust literal pin. Its 3 failures are the cached page's bytes and hash, which differ from the pinned page, and catalog definitions, which this cache does not contain. Neither is on the gun path. The bodies of `HoldsGun`, `Update`, `Tick`, `Shoot`, `Fired`, `TakeBunch`, `Hit`, `ShotAnswered`, `Gun.Crc` and `Gun.Shot` are therefore unchanged in normalized form, so that report's state table, arithmetic, CRC and 7-byte 477 layout still hold.

These methods in the table below are not in `signatures.json`, so `check` does not guard them: `<OnLearned>g__Has`, `DrillEnd`, `CampLeave`, constructor `b__0`, `b__1`, `b__8`, `b__9` and `b__10`/`b__11`, `ESAFastLoad.Speed`, `ParseCounterDamage`, `ParseGuardDamage`, `ApplyMessage`'s `b__0`, `StrikeOf`, `Later(Strike)`, and `GunFlowingUi` `b__1` and `b__7`. Their bodies were read directly for this report. `accept` covers only signed names, so adding them needs a reviewed `sign` run that cites this report for build ea1a68bd.

## Current joins

`m` is the metadata method row and `f` the WASM function at its original body start. Sizes and SHA-256 values cover the original body bytes.

| Method | m | f | Bytes | SHA-256 |
| --- | ---: | --- | ---: | --- |
| `GunFlowing..ctor` | 40856 | f24374@`7789c5` | 4624 | `cc34d3ae369b8d1cafe7974181cb313fd17f5aa66c77629d874b2cc4d6a3b2a1` |
| `GunFlowing.Wear` | 40862 | f24375@`779bd7` | 560 | `560710695b41106aac8d57f13c30afa776307835e2efe2afd49f27ba2ae98619` |
| `GunFlowing.LearnedAtLogin` | 40882 | f24376@`779e09` | 266 | `9281e9615f0f32b45d360950892d45eb03519a23baaca806d284dcdcebce1f04` |
| `GunFlowing.Special` | 40868 | f24377@`779f15` | 397 | `568740c08ec8063a45f46c415bf71338f173895ea71cc9924b72e253e3f31721` |
| `GunFlowing.SpecialOn` | 40869 | f24379@`77a1af` | 328 | `36423d752b482258625f20ea55ff18e8ebbef9de7deafccdd754406d1cebc916` |
| `GunFlowing.get_GunWorn` | 40860 | f24382@`77a5d8` | 143 | `b63c3fb8b42885fd2ef7f5440162ce7b2230200f30450d80c0c97103d5240d7f` |
| `GunFlowing.HoldsGun` | 40861 | f24383@`77a669` | 614 | `10abc2bbde9fb343a8a2bd8210c5e3e54140ffedc869f1650f4790800a8fd7cb` |
| `GunFlowing.OnKey` | 40863 | f24386@`77a926` | 26 | `c413d2c12f3fb402ada58c4063002e186c42ddbb36d8d60f04c8643902ba71dd` |
| `GunFlowing.GunOn` | 40865 | f24387@`77a942` | 644 | `4bedd296b5fd2a42944ffce5a2a3d93769a4479d59ab4070e903168ebc7fe462` |
| `GunFlowing.MyAppear` | 40864 | f24389@`77ac93` | 580 | `ca6af3f661bb5f61de91260703d2dd1af4d95ce087092c9f694b1c6664690fe6` |
| `GunFlowing.Update` | 40871 | f24392@`77af48` | 356 | `99d1aa7fdff89218c21205542061a5134690f99acc1b8cab64c475b3cd798609` |
| `GunFlowing.Tick` | 40872 | f24393@`77b0ae` | 456 | `0a671708b745d81a423af3d4f87bbe3d6dec8067dfc327a9c959bbbd9febf434` |
| `GunFlowing.MayShow` | 40873 | f24395@`77b4cf` | 258 | `96a63eaaee85c0e720fb82fdc8c732f6cf8c507a4b1569cf2b8083a5288e451f` |
| `GunFlowing.Shoot` | 40877 | f24401@`77b75b` | 427 | `b0582e31b320f45c43fdc9b3e026b61bbb255196d8a465b22d3ad1beb754d370` |
| `GunFlowing.Fired` | 40878 | f24402@`77b908` | 165 | `42ebc5e9ea5ad6414ca92cda0e2b380b9ccbca187c0fffde9cdcb3e497355689` |
| `GunFlowing.TakeBunch` | 40879 | f24403@`77b9af` | 449 | `b12b649c6cd4fbaef198abd741f690d0cc50573abe70b22f0333cd37d9f0732a` |
| `GunFlowing.ShotAnswered` | 40880 | f24405@`77bbe5` | 25 | `d2f3afbb4ad7d6cab75b36fba4d15b3a14612d7d9fb3552641f2c3f7fc864c21` |
| `GunFlowing.Hit` | 40881 | f24406@`77bbff` | 25 | `4583ae28c8454c14b34dad8aec061db27b46a708fa30557599ed02e672f4a0d4` |
| `GunFlowing.OnLearned` | 40883 | f24407@`77bc1a` | 215 | `2463e760821dc09a7118df1df09c4b8e4dc76b2e34f5c0b5e6089a6595d5cb61` |
| `GunFlowing.<OnLearned>g__Has` | 40903 | f24408@`77bcf2` | 84 | `e4bb88ac14610f763ee64c398cb46df9ecbf02adff1cb9da8a3e9ec25978b2e2` |
| `GunFlowing.SetLoadSpeed` | 40884 | f24409@`77bd47` | 10 | `1e68aa5a7bb1f9c808b077d9a8ca3203f24991db6fa222def0bf523e4348ff6d` |
| `GunFlowing.DrillBegin` | 40893 | f24412@`77c04b` | 41 | `4014a29022d8692926d615637bcd7cb7093b8af4b7a9acb7e86cc63e83fd2301` |
| `GunFlowing.DrillEnd` | 40894 | f24413@`77c075` | 68 | `bd75e4587146d7ed967e414b5816c2f83878064b0a7c522a8111ec9bf72e6118` |
| `GunFlowing.CampEnter` | 40898 | f24417@`77c260` | 62 | `e77212d3240eb21a527cbd5367dcb6632e6311e0f8f26f0902fa2f63c076a671` |
| `GunFlowing.CampLeave` | 40899 | f24418@`77c29f` | 103 | `6df253294262715129519b3849903b4d93063fb29b6b3149f461cc1639e5be1b` |
| constructor `b__0` (82) | 40910 | f24424@`77c6f6` | 229 | `442fbb1bf9e865c35f960fe6957f5ff82a53e1d86eaf6f69d758d550c7f25f79` |
| constructor `b__1` (85) | 40911 | f24425@`77c7dd` | 139 | `571b8093f8172c2f26716c51af6d7e56fbea11f064776c05e65a1e465dbad336` |
| constructor `b__2` (512) | 40912 | f24426@`77c869` | 20 | `c0bca7fbce1147a6927027d2b51913e105f06303b06a5c6266ffdb16702505c9` |
| constructor `b__5` (32) | 40915 | f24429@`77c8a3` | 242 | `19280c788eb4da018f5e643baf42970b6fe196bd852f0902c68a9386895877b6` |
| constructor `b__6`, `b__7` (310, 525) | 40916, 40917 | f24430@`77c996` | 26 | `c116442b7be245ffa65bd25414316d3bb8cd0e431aa019c251db55d9ff2ea028` |
| constructor `b__8` (410) | 40918 | f24431@`77c9b1` | 46 | `431adf7d2c6a1e9e9bcf2b4976f631b38fbab1f830613ab400c8a2d2258bc94f` |
| constructor `b__9` (377) | 40919 | f24432@`77c9e0` | 13 | `2b9334be62a8c7091df2cd5ab249052961dfd269d9374b0941de68fcf0f6efc5` |
| constructor `b__10`, `b__11` (380, 382) | 40920, 40921 | f24433@`77c9ee` | 13 | `dcdd1518f75d2f37e076ad52931e0e0cb5b33db9d56bad88aba8a8e0d02baaa7` |
| `Gun.Crc` | 72645 | f12914@`398d39` | 30 | `6dcbf85444a0383346b827c7555c8749e0979b27e8ceae970dd4cc17bc2aca02` |
| `Gun.ReadKey` | 72647 | f12916@`398db2` | 88 | `287759c7c17be8d25ef0c2776e236f05262e12020dcf694d7a0c4c7fc90b31df` |
| `ESAFastLoad.Speed` | 52336 | f16108@`4d12c0` | 33 | `a3892ad69ed31b5cbcd5436512e4e62c8c81dab7a344e0989b76ba28dcfbc1c5` |
| `Combat.ParseMessage` | 71401 | f10804@`31e32b` | 1557 | `076052359538d5468c92741bcfb83470ec6a43edc41e045557df077805bd410d` |
| `Combat.ParseDamage` | 71394 | f10797@`31de3d` | 132 | `ade189a5e0f36814b92d16dfa30e00dd6de996a597f0d9f5700ba8537ced23c4` |
| `Combat.ParseCounterDamage` | 71395 | f10799@`31df7c` | 148 | `4cfe4ee829ebae2cf45e111270ee25adaa34ca04bef1c6e292bc27263fb787f3` |
| `Combat.ParseGuardDamage` | 71396 | f10800@`31e012` | 148 | `db648b31c61195281836db5449769431fd43458e19144c459a02bdac8bed81f1` |
| `CombatModel.ApplyMessage` | 37916 | f17568@`547509` | 2668 | `9ce2a603fdff2058ffdf7e429d025e6697761b5bd780811d44f0ee15636784cf` |
| its delayed `b__0` | 37977 | f17613@`54d51c` | 28 | `ee5de84df066c0381aaa874ae2969d3036f93965c7e7919486d4cbbbd3dc163a` |
| `CombatModel.Begin` | 37923 | f17574@`5482fe` | 3790 | `cec3dfc36122e26112d8ef5a6d59db167ee2b6b1d88f1a2fbdaffcca3b3d9797` |
| `CombatModel.StrikeOf` | 37918 | f17572@`5481f5` | 158 | `70bc84e6ee86b66c5fa48c66c95a64d01ec0bc7515f53f7248ec0be2dbe20b79` |
| `CombatModel.Later(Strike)` | 37919 | f17573@`548294` | 104 | `8749263c44dd63a35213350aa391bf8032d681062b10006e969f377dd8d03adc` |
| `GunFlowingUi` install `b__1` | 25100 | f46662@`f4b583` | 205 | `a920ef2fea568b04aad10ff9448acdda6b30f62185c11320792759f36348379b` |
| `GunFlowingUi` install `b__2` | 25101 | f46663@`f4b651` | 13 | `32b16916c0c781ced3224f8ba39ac9420eae85d8a087ec02698d87c46f298228` |
| `GunFlowingUi` install `b__3` | 25102 | f46664@`f4b660` | 191 | `0aa50374a1f4ff505a4078e1762697c08e1fcc59da5492913936402f4d11f110` |
| `GunFlowingUi` install `b__7` | 25105 | f46667@`f4b834` | 180 | `7a1755fd22eec7f20d9f4f5b113c271496178398aca57252d226e3caf3954fe4` |
| `GunFlowingUi` install `b__9` | 25107 | f46669@`f4b990` | 1298 | `de7b671375c807a6c04f4a1c2e04b1a6d0e17b2750e0cde7a5ee425cd1cab9ee` |

The `Shoot`, `HoldsGun` and `Gun.Crc` hashes equal the pins in [`crates/net/README.md`](../../crates/net/README.md). `ESAFastLoad.Init`, `InitState` and `OnEnd` (f16107, f16109, f16110) and `SkillEsaView.Install` (f45327) are covered by the existing signatures and matched.

## Source facts the controller relies on

1. **Construction and key.** The constructor registers each listener through `On` (f24378); a listener exists only from construction. `Wear(id, true)` on a Type 2, SubType 54 item resets the controller and sends an empty 511. The controller is built while gameplay installs, before `GameSession.EnterWorld` sends C5 and flushes held sends (see the [inventory baseline report](native-inventory-baseline.md)), so that 511 leaves after C5. A 512 received before construction is not replayed to `b__2`.
2. **StartSet replay.** The constructor enumerates StartSet and calls `LearnedAtLogin` for every 271 in arrival order (f24374 at `779325..779334`). Each call assigns both flags from that list alone: 3011 to +81 and 3010 to +80 (f24376 at `779efe..779f0f`). The last StartSet 271 therefore wins, and without one the flags keep their constructed false values. No listener reads a live 271. The constructor also replays every StartSet 518 and 521 in arrival order through `Special`, which ignores a payload shorter than 8 bytes and a UID missing from the inventory.
3. **Worn tracking.** The controller keeps its own per-slot UID array. It starts zeroed at construction, and only live 82 and 85 maintain it. 82 (`b__0`) wears a resolved item even when its UID equals the slot's previous UID, so a repeated 82 resets the controller and sends another 511. It unwears the previous item only when that UID is nonzero, different and still in the inventory. 85 (`b__1`) clears the slot, then unwears the named item when it resolves. An 82 or 85 received before construction is therefore absent from the controller's view.
4. **Own appearance.** Live own 8 or 9 reaches `MyAppear`. With player appearance data and Attributes bit 1 clear, it wears every nonzero worn item ID again. With a gun worn this resets the controller and requests a new key.
5. **Live learning.** 523 reaches `OnLearned`, which reads the first U16 skill and sets a flag through `Has`. `Has` consults the level lookup that the UI passes to the constructor (`b__1` at f46662 calls `SkillLink.Level`). That lookup's state is not decoded here.
6. **Own damage.** The UI `Damaged` callback (`b__3`) calls `Hit` whenever the damaged actor's ID equals the session's own ID; it tests no damage value or flag. `Combat.ParseMessage` sends 42 to `ParseDamage`, 923 to `ParseCounterDamage` and 668 to `ParseGuardDamage` (branches at `31e405`, `31e45f` and `31e43b`). Counter and guard damage read U16, U16, then a damage body, and the first U16 is the actor `ApplyMessage` damages. `ApplyMessage` applies 42 and 923 at once through `ApplyDamage` (at `547803` and `54784c`). For 668 it looks the actor up and, if a further actor check passes, schedules `b__0` with `After(500 ms)` (at `54793f`); `b__0` then calls `ApplyDamage` for that actor.
7. **Releases.** 32 (`b__5`) calls `ShotAnswered` for the own actor while `GunWorn`. 310 and 525 (`b__6`, `b__7`) call it while Armed. `ShotAnswered` changes only state 5 to state 4, so none of them is an outcome.
8. **Camp.** 410 (`b__8`) with a nonempty payload whose first byte is 4 only sets InCamp (+167). Any other 410, and 377 (`b__9`), call `CampEnter`. It sets InCamp and, while the gauge is shown, clears Armed, Ready, State, Load and Power. 380 and 382 (`b__10`, `b__11`) call `CampLeave`. `GunOn` arms nothing while InCamp is set.
9. **Digging.** The UI play-event callback `b__7` calls `DrillBegin`, which hides the gauge without stopping `Tick`, and `DrillEnd`. While `GunWorn` and hidden, `DrillEnd` shows the gauge again and, when a profile is set, restarts state 1, or state 0 without loaded ammo. The same callback calls `CrazyStart` and `CrazyQuit`. Those two and `CrazyTick` (f24414–f24416) were not examined.
10. **Strike timing.** `StrikeOf` runs `Begin`, and so the own `GunFired` and `Fired`, at once unless the attacker already has queued strikes. Then it queues `Later(strike)`, which runs `Begin` when the earlier ones finish.
11. **Time and speed.** The UI frame callback `b__9` keeps the fractional-millisecond residue before `Update`, as the earlier report describes. `ESAFastLoad.Speed` computes `F(1 + double(F(100 − v)) / 100)`, and the UI forwards it to `SetLoadSpeed`.

## Not established

- The live order of 271 and own 8/9 relative to the startup receipts. Source makes the client's StartSet end at its backlog replay, which can be later than the receipt of S233.
- Whether the server validates the CRC, the critical bit, the key's lifetime or the shot rate.
- The effect timers through which an ESA damages the own avatar, beyond the effect's start.
- When the attacker's strike queue drains, which bounds how late a queued own `Fired` runs.
- What `Has` returns for a given skill level, and the `Crazy*` transitions.

The controller's choices for these gaps are listed in [`crates/net/README.md`](../../crates/net/README.md). Synthetic tests in `crates/net/src/gun.rs`, `gun_tests.rs` and `session.rs` cover the modelled transitions.
