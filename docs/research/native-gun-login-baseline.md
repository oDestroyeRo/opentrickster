# Native gun login baseline

Account-free static research on build **ea1a68bd89958fae94179f1e0835ffb5**, observed **2026-11-10 UTC**. It answers what the client's `GunFlowing.LoadSpeed` is after a normal login, and why a native session never proved it. It extends the [gun controller sources](native-gun-controller.md) and narrows one rule of the [own effect lifecycle](native-own-effect-lifecycle.md) report for the login window.

Evidence classes follow the [migration report](build-ea1a68bd-migration.md). **Source** means static decoding of the current public artifacts. **Synthetic** means checks on invented fixtures. No live acceptance is claimed here.

## Symptom

Root ran a private diagnostic build of the packaged app in clientless mode. The character wore a gun. During login the own player appeared only on s2c 7, with a player body and no FastLoad in its effect list. Foreign players appeared on 8, also without FastLoad. No own 8 or 9 arrived in about a minute idle. `LoadSpeed` never left its default, so `gun_prerequisite` returned `GunEffectState` for the whole session. `Support.basic_attack` was false at Baseline, and the engine refused Start with `Blocker::NeedsCapability(BasicAttack)`.

Net took the effect baseline only from an own 8 or 9. The own effect lifecycle report proved that the client attaches effects from StartSet 8/9 and skips an own 7. It did not ask what the client's scalar is when no own 8/9 arrives.

## Provenance and limits

Bodies were read from an outside-Git `tools/build-migrate` cache of the public build. Its WASM (`6396dfe9….wasm.gz`, SHA-256 `8780db44…`) and Unity data (`9c68dd2a….data.gz`, SHA-256 `a3b60bc7…`) equal the migration report's pins. Bodies were printed as normalized tokens through `buildmigrate.derive`, and their original bytes were hashed. Throwaway scripts outside Git decoded every defined body without error, indexed its direct calls and stores, and searched metadata usage cells. No target WASM or JavaScript was executed. No game socket was opened, no account was used, and no credential, capture or character value was read.

Caller facts below are direct calls, interface dispatch constants and metadata usage cells. A method with no usage cell cannot be the target of a delegate.

## Joins

`m` is the metadata method row and `f` the WASM function at its original body start. Sizes and SHA-256 values cover the original body bytes.

| Method | m | f | Bytes | SHA-256 |
| --- | ---: | --- | ---: | --- |
| `GunFlowing..ctor` | 40856 | f24374@`7789c5` | 4624 | `cc34d3ae369b8d1cafe7974181cb313fd17f5aa66c77629d874b2cc4d6a3b2a1` |
| `GunFlowing.set_LoadSpeed` | 40822 | f24365@`77896c` | 9 | `837c1bb8c1401ab312b9a3da00a753c5379c0beff3838ad9ebf6cf89da0ec0a8` |
| `GunFlowing.SetLoadSpeed` | 40884 | f24409@`77bd47` | 10 | `1e68aa5a7bb1f9c808b077d9a8ca3203f24991db6fa222def0bf523e4348ff6d` |
| `GunFlowing.MyAppear` | 40864 | f24389@`77ac93` | 580 | `ca6af3f661bb5f61de91260703d2dd1af4d95ce087092c9f694b1c6664690fe6` |
| `SkillEsaView/Host.SetLoadSpeed` | 31160 | f45443@`ee1c22` | 196 | `a9c213c680d97b4f18e9ac3741f0f432cf63cb00268228259e52e544acff5657` |
| `ESAFastLoad.Init` | 52334 | f16107@`4d0fa8` | 791 | `f2763314ac3984760cbe28df255d87070f1e4ef7248447bbd4d53be1932f83a5` |
| `ESAFastLoad.InitState` | 52335 | f16109@`4d12e3` | 206 | `f526d6dce43ac3c6b4945c366841af33614cbb4c6ad458b145e25d804a4ec929` |
| `ESAFastLoad.OnEnd` | 52337 | f16110@`4d13b3` | 170 | `5094326c91a4acedef4952f91e377068f4c33fd4ba12ae29a9460cc329ba9698` |
| `EsaRegistry/<>c.<R17>b__18_17` | 50047 | f16786@`505645` | 52 | `1167f04741e54ff64cbabce63a3578c969b880f9e71ed0d105bf6a1609e21605` |
| `EsaRegistry.Create(int,int)` | 49707 | f16447@`500e48` | 245 | `cce880a32e664346834e276c54bfe0939f183895d734246fa94c3361db4acaac` |
| `EsaRunner.Play` | 50222 | f16933@`50cb4a` | 1100 | `2c04cb7fb68f1166ba57da62075232f40b49422dd09c04ed87d2f3493246e0b1` |
| `EsaRunner.StartState` | 50224 | f16941@`50d8c0` | 17 | `61af548fa7c90774bd5b6c29a43b52707e38b4e2fc15ad4a5acd781d44af5858` |
| `EsaRunner.BeginState` | 50225 | f16942@`50d8d3` | 984 | `c793c9aefa66285d2d8af809aece6419743b491eaac61ddaeb8fda7043a1140a` |
| `EsaRunner.StartStates` | 50226 | f16943@`50dcad` | 1567 | `c1297127df28c016ddfbf1a0efe789aa7ea3c71d36b91f57e1d43db70fc83e93` |
| `EsaRunner.OnAppear` | 50231 | f16948@`50e982` | 216 | `91084b2fac60702e58b02503ed4a2aa0ed165fa54020a908306ca8f34f27a01a` |
| `SkillEsaView/Host.State` | 31168 | f45451@`ee23c1` | 159 | `74dcf094be3c1f1a71dffb7562cffaf0e2bfa8c6afe1615d2bfb5df426c3dcfb` |
| `SkillEsaView.Install` | 31048 | f45327@`ed8c65` | 4797 | `6b199b08a3cb2b4b26526581a9bd04bc5a0522d2e007f628ca0178b6e2109e46` |
| `SkillEsaView` install `b__7` | 31301 | f43939@`e440f3` | 78 | `d9a139bb8c8c79f5d46af5b24604d0784b8e5193e523f4561792877e4abf7594` |
| `GameSession.Handle` | 72143 | f20915@`65de1f` | 3472 | `5c9189446438e84bee0dba8d5fe232096d65abfee90a655e14b3b4b37632b627` |
| `GameSession.EnterWorld` | 72116 | f20891@`65c8bd` | 73 | `96b79fdffb6a6b39cd8c55528f16ff3fa5a4dac3ecbfc5e57edbeec75fc2e861` |
| `GunFlowingUi/<Install>d__17.MoveNext` | 25111 | f46675@`f4c731` | 5670 | `632df752ba87a63f3d03c0ae31cd6701fd80f33cce9f1ebcb890c7ea3d8598d4` |
| `SkillsFeature/<Install>d__3.MoveNext` | 31781 | f44386@`e73c25` | 6126 | `2e39819a129edfc04da9601d8182b385402d9b5d48df5622a8c40e91656a26da` |
| `PlayFeatures..cctor` | 28867 | f37436@`b8c21b` | 11532 | `1dea9b81602597ad90a33bbea9542a530207b397621973253609cac69b1adde5` |
| `GameplayScreen.InstallFeatures` | 19367 | f38485@`bf4632` | 440 | `0ffb2690aa9056ca23c735f66bc7f177df33234281ddf296ec2cfe0ae95513ff` |
| `GameplayScreen/<Enter>d__283.MoveNext` | 19623 | f48925@`103138e` | 20370 | `84f6ecf8fb9ac91994f07b24ab7ffc4f6f6601d08ac33843dd6bf8f4556c906d` |

The `GunFlowing..ctor`, `SetLoadSpeed` and `MyAppear` hashes equal the [gun controller report](native-gun-controller.md), whose `check` run also matched `ESAFastLoad.Init`, `InitState`, `OnEnd` and `SkillEsaView.Install`.

## Source facts

### The scalar and every writer

`<LoadSpeed>k__BackingField` is ordinal 18 of `GunFlowing` (type definition 6632), an F32 at +76. `Bunch` is ordinal 17, an I32 at +72. The property getter m40821 is public and its setter m40822 is private.

A census of the 116 bodies of `GunFlowing` and its nested types lists every store whose bytes cover +76..+79:

- The constructor f24374 pushes `i64.const 0x3f80000000000001` at `778b18` and stores it at +72 at `778b22`. This one store writes `Bunch = 1` and `LoadSpeed = 1.0f`.
- `SetLoadSpeed(double)` f24409 converts its argument to F32 at `77bd4c` and stores it at +76 at `77bd4d`.
- `set_LoadSpeed(float)` f24365 stores at +76 at `778971`. Its body is folded with an unrelated UI setter, and no body in the module calls it directly.

The other bulk and SIMD stores in these bodies copy into array data, write `TrembleDx/Dy` at +136, or write statics. None covers +76, so `Reset`, `Wear` and `MyAppear` never write the scalar.

`SetLoadSpeed` has one direct caller, `SkillEsaView/Host.SetLoadSpeed` f45443 at `ee1ce0`. No metadata usage cell names `SetLoadSpeed` or `set_LoadSpeed`, so no body builds a delegate to either. The same search finds the constructor's loads of the `MyAppear` cell, which register its packet handlers. `Host.SetLoadSpeed` gets the controller through `PlayServices.Get<GunFlowing>()` (`ee1cca..ee1cd2`) and skips the call when the result is null (`ee1cd6..ee1cd8`). A write before the controller is provided is dropped.

The host method implements `IEsaHost.SetLoadSpeed` m50406, interface slot 63. Of the 1232 bodies that load the `IEsaHost` type cell, only `ESAFastLoad.Init` f16107, `InitState` f16109 and `OnEnd` f16110 use the slot-63 dispatch constants: 696 on the fast path and 63 on the fallback, at `4d111c`/`4d1127`, `4d1390`/`4d139b` and `4d1431`/`4d143c`. `OnEnd` passes 1.0 at `4d1446`. The constant pattern was checked on a known call: `IPlayContext` slot 38 gives 496 in f45443.

Between login and the first shot, the scalar is therefore 1.0 from construction. It changes only when a FastLoad starts or ends after the controller is provided.

### What the client does with an own 7

`GameSession.Handle` f20915 dispatches on `opcode - 6` with the `br_table` at `65e19a`. Entries 1 to 3, which are opcodes 7, 8 and 9, all continue after the block that ends at `65e2f6`. That path parses the body with `Messages.ParseAppear` at `65e312` and emits `GameEvent/Appear` with the opcode at +16 (`65e36d`). For an own 7, its `Warp` flag is 0 (`65e326..65e35d`). An own 7 has the grammar of 8 and 9.

Two consumers could apply an appearance to gun state. Neither takes an own 7:

- **`MyAppear`.** The `GunFlowing` constructor registers it for 8 at `77989b` and for 9 at `7798c4`, and for no other opcode.
- **The effect runner.** `SkillEsaView.Install` f45327 replays StartSet and skips a frame when `(Opcode & -2) != 8` (`ed9c92..ed9c9d`), so it never parses a StartSet 7. Its live handler `b__7` f43939 forwards every `GameEvent/Appear` to `EsaRunner.OnAppear` at `e4413a`. `OnAppear` f16948 returns before `StartStates` when the opcode is 7 (`50e9ae..50e9b2`) and the actor is `IEsaHost.get_MyId` (slot 74, dispatched at `50ea15`, compared at `50ea3c..50ea3f`). Every other parsed appearance reaches `StartStates` at `50ea50`.

An own 7's effect list never reaches the effect runner, in StartSet or live, so it cannot write `LoadSpeed`. Client source does not show whether the server fills it with the complete own attached list, and the scalar does not depend on it.

### How a FastLoad can exist

`ESAFastLoad` is allocated only by the registry callback f16786. `EsaRegistry.Create(int,int)` f16447 has two callers: `EsaRunner.Play` f16933 at `50cc0e`, which is the 224/292 skill-hit path, and `BeginState` f16942 at `50d957`. `BeginState` has two callers. `StartState` f16941 calls it at `50d8c9` and is called only by `Host.State` f45451 at `ee2459`, whose one caller is the 307 handler. `StartStates` f16943 calls it at `50e170` and is called only by `OnAppear` at `50ea50`. None of these methods is virtual. The only metadata usage cell among them names `EsaRunner.Play`, which `SkillEsaView.Install` loads at `ed8cb4` and `ed8e74` to build its skill-hit delegate.

A FastLoad instance therefore needs a 224/292 start, a 307 start, or an appearance other than an own 7 whose effect list names type 1476. Net already reads every one of these.

### Construction order

The `GunFlowing` constructor has one call site, `GunFlowingUi/<Install>d__17` f46675 at `f4d039`, after awaits on its data and asset loads. `PlayServices.Provide<GunFlowing>` follows at `f4d106`. `SkillEsaView.Install` has one call site, `SkillsFeature/<Install>d__3` f44386 at `e75161`, after it awaits the combat model at `e73c7c` and `e73f76`.

Both installers are in `PlayFeatures.Core`, the static field at +0 stored at `b8d857`. `GameplayScreen.InstallFeatures` f38485 starts every Core installer in one loop (`InstallFeature` at `bf475e`) and returns `WhenAll` of those tasks at `bf47e5`. `GameplayScreen/<Enter>d__283` f48925 awaits that task (`1035b99..1035bf5`) before it calls `GameSession.EnterWorld` at `1035c3e`, which sends C5.

Both installs therefore finish before C5. Source does not order the effect runner's StartSet replay against `Provide<GunFlowing>`, because each installer first awaits its own loads. A FastLoad named before C5 can write the scalar or be dropped by the null test in f45443. `LateHooks` starts recording at `InstallFeatures` (`bf4696`) and is fed by `GameSession.Handle`. It was not examined, and the result below does not depend on it.

## Result

After a normal login, the client's `LoadSpeed` at C5 is exactly **1.0** when no frame before C5 named FastLoad. That excludes an own 8/9, a foreign 7/8/9, a 224/292 or a 307 that names type 1476, and any start too short to name its type. No FastLoad instance exists then, so only a FastLoad that starts later can change the scalar. An own 7 does not change the value, whatever its list holds.

When a frame before C5 named FastLoad, source does not determine the value at C5. The replay may have run before or after `Provide<GunFlowing>`. A complete own 8 or 9 after C5 settles it again by the own effect lifecycle rule: `StartStates` ends the actor's old attached states, whose `OnEnd` writes 1.0 for a FastLoad, then starts the new list with `InitState`.

## Native model

`LoadSpeed` in [`crates/net/src/gun.rs`](../../crates/net/src/gun.rs) follows the constructor:

1. `construct_gun`, which Net calls right after C5, stores 1. With no own list before it, no own FastLoad is attached.
2. Before construction, an own 8 or 9 that lists a FastLoad leaves the speed unknown, and no own 8 or 9 before construction clears that. Only a complete own 8 or 9 after construction settles it.
3. Every other rule is unchanged. A foreign, detached or unreadable FastLoad leaves the speed unknown for the session, and so does an own 7 that lists one, although the client ignores that list. An own 224/292 FastLoad leaves it unknown until the next complete own 8 or 9 after construction. So do an own 7, own faint, own removal and an end naming the tracked instance while an own FastLoad is tracked.

Synthetic tests in `gun.rs`, `gun_tests.rs` and the session loopback cover the own-7 login and each case before construction.

## Not established

- Whether the server ever sends an own 8 or 9 during or after login. Root saw none in about a minute idle.
- Whether the client replays pre-install 224/292 or 307 frames through `LateHooks`, and the relative timing of the Core installers in a real client.
- Zone changes. The own effect lifecycle report ends every running effect on zone cleanup. Net keeps `LoadSpeed` across a 233 zone change and a 437, and this report did not trace either.
