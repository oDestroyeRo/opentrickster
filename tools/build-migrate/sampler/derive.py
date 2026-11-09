#!/usr/bin/env python3
"""Derive the assisted sampler's pins for one Unity WebGL build.

    python3 -I derive.py --build ID WASM DATA [--expect OLD.json] [--out NEW.json]

WASM and DATA are the public Build/*.wasm.gz and Build/*.data.gz assets; gzip
is detected by magic. Only bytes are parsed. Target code is never instantiated
or run. Every pin is found by a stable name (type, field, method or metadata
usage identity), never by reusing an address from an earlier build.
"""
import argparse
import gzip
import hashlib
import json
import re
import struct
import sys

# ---------------------------------------------------------------------------
# What crates/net/src/sampler.js and crates/net/tests/sampler.cjs depend on.
# ---------------------------------------------------------------------------
TYPES = """
System.String System.Boolean System.Int32 System.Int64 System.Single System.UInt64 System.Object
System.Nullable`1 System.Collections.Generic.Dictionary`2/Entry UnityEngine.Vector2 UnityEngine.Vector2Int
System.Collections.Generic.List`1 System.Collections.Generic.Dictionary`2
System.Collections.Generic.IReadOnlyDictionary`2 System.Collections.Generic.Queue`1 System.Func`1 System.Action
System.Type System.RuntimeType System.Delegate System.MulticastDelegate
UnityEngine.InputSystem.Controls.AnyKeyControl UnityEngine.InputSystem.Controls.ButtonControl
UnityEngine.InputSystem.Controls.DiscreteButtonControl UnityEngine.InputSystem.Controls.KeyControl
UnityEngine.InputSystem.InputControl`1 UnityEngine.InputSystem.InputSystem UnityEngine.InputSystem.InputManager
UnityEngine.InputSystem.InputAction UnityEngine.InputSystem.InputActionMap UnityEngine.InputSystem.InputActionState
UnityEngine.InputSystem.Keyboard UnityEngine.EventSystems.EventSystem
Spinel.UI.Pc.Screens.GameFlow Spinel.Engine.Unity.SpinelEngine Spinel.UI.Pc.Screens.GameplayScreen
Spinel.UI.Pc.Screens.GameplayScreen/LoadedZone Spinel.Engine.Unity.Stage Spinel.Engine.StageLayout
Spinel.Engine.Unity.GameInput Spinel.Engine.Unity.World.MapView Spinel.Engine.Unity.World.ActorView
Spinel.Game.World.Point Spinel.Game.World.WorldActor Spinel.Game.World.ZoneInfo Spinel.Game.World.WarpRegion
Spinel.Net.Game.GameSession Spinel.Net.Game.Inventory Spinel.Net.Game.InvItem Spinel.Net.OrderedMap`2
Spinel.UI.Pc.Screens.PlayServices Spinel.Game.Play.CombatModel Spinel.Game.Play.CombatModel/Aging
Spinel.Game.Play.CombatModel/WaitingMove Spinel.Game.Play.DrillFeature Spinel.Game.Play.DigAssist
Spinel.Game.Play.GunFlowing Spinel.Game.Play.AvatarStatusFlags Spinel.Game.Play.SkillsApi Spinel.Game.Play.AvatarFsm
Spinel.Game.Play.HotkeyMap Spinel.Game.Play.KeyBinding Spinel.Game.Play.RunToggle Spinel.Game.Play.PlayerStats
Spinel.Game.Play.StatValues Spinel.Game.Play.ItemInfo Spinel.Game.Play.DrillData Spinel.Game.Play.DrillParam
Spinel.Game.Play.AutoDrill Spinel.UI.Pc.Hud.Hud Spinel.UI.Pc.Hud.HudCover Spinel.UI.Pc.UiSkins
Spinel.UI.Pc.Play.DigKeySettings Spinel.UI.Pc.Play.CarriedWeightUi/<>c__DisplayClass2_0
Spinel.UI.Pc.Screens.GroundItems Spinel.UI.Pc.Screens.GroundItems/Entry Spinel.UI.Pc.EditBox
Spinel.UI.Pc.UiPressOrigin
""".split()

# Fields by name. "s:" marks a static field; <Name>k__BackingField is written Name.
FIELDS = {
    'Spinel.UI.Pc.Screens.GameFlow': 'Engine Screen s:Current',
    'Spinel.Engine.Unity.SpinelEngine': 'Stage Input s:Instance',
    'Spinel.UI.Pc.Screens.GameplayScreen': 'steering steerWarping play zone run avatar actors groundItems ground '
                                           'hoveredId services hud route ready changingZone loggingOut localScene '
                                           'campScene held pointer cameraX cameraY worldCovered artAwaited '
                                           'probedAt probedTime overUi',
    'Spinel.Engine.Unity.Stage': 'hostOverride hostRect Current panPixels initialized',
    'Spinel.Engine.StageLayout': 'UiScale WorldZoom PxPerDp ViewportWidth ViewportHeight WorldWidth WorldHeight',
    'Spinel.Engine.Unity.GameInput': 'Click RightClick',
    'Spinel.Game.World.WorldActor': 'path Id Type X Y Unseen Hidden Limited HitLeft HitTop HitWidth HitHeight',
    'Spinel.Net.Game.GameSession': 'targetPending sendArmed heldStop',
    'Spinel.UI.Pc.Screens.GameplayScreen/LoadedZone': 'Info Map Warps',
    'Spinel.Game.World.ZoneInfo': 'Id Type Option SubType',
    'Spinel.Engine.Unity.World.MapView': 'Width Height',
    'Spinel.UI.Pc.Screens.PlayServices': 'items',
    'System.Type': '_impl',
    'System.RuntimeType': 'type_info',
    'Spinel.Game.Play.SkillsApi': 'target shownTarget combatPlaysEsa UiWait heldCast',
    'Spinel.Game.Play.CombatModel': 'session Target Fainted attackTargets Fsm striking waitingMoves fading',
    'Spinel.Game.Play.DrillFeature': 'Data Inventory session fsm Flags inDrillState drill param State Gauge Held '
                                     'Crazy AutoMode Auto AutoFlagOn',
    'Spinel.Game.Play.DigAssist': 'drill State',
    'Spinel.Game.Play.GunFlowing': 'Inventory session infos Armed Ready State',
    'Spinel.Game.Play.AvatarFsm': 'State',
    'Spinel.UI.Pc.Hud.Hud': 'keyMask Stats Keys Waiting',
    'Spinel.Game.Play.HotkeyMap': 'Bindings',
    'Spinel.Game.Play.KeyBinding': 'Code Modifier Exclusive',
    'UnityEngine.InputSystem.Keyboard': 'shiftKey ctrlKey altKey m_Keys s:current',
    'UnityEngine.InputSystem.Controls.ButtonControl': 'm_LastUpdateWasPress needsToCheckFramePress pressPoint '
                                                      's:s_GlobalDefaultButtonPressPoint',
    'UnityEngine.InputSystem.InputControl': 'm_CachedValueIsStale',
    'UnityEngine.InputSystem.InputSystem': 's:s_Manager',
    'UnityEngine.InputSystem.InputManager': 'm_ReadValueCachingFeatureEnabled',
    'UnityEngine.InputSystem.InputAction': 'm_ActionIndexInState m_ActionMap',
    'UnityEngine.InputSystem.InputActionMap': 'm_State',
    'UnityEngine.InputSystem.InputActionState': 'memory m_InProcessControlStateChange m_Suppressed',
    'UnityEngine.InputSystem.InputActionState/UnmanagedMemory': 'actionCount actionStates',
    'UnityEngine.InputSystem.InputActionState/TriggerState': 'm_Phase m_Flags',
    'Spinel.Game.Play.RunToggle': 'On Heavy Ready',
    'System.Delegate': 'method_ptr m_target',
    'System.MulticastDelegate': 'delegates',
    'Spinel.UI.Pc.Play.CarriedWeightUi/<>c__DisplayClass2_0': 'carried hud ctx run',
    'Spinel.Game.Play.PlayerStats': 'current',
    'Spinel.Game.Play.StatValues': 'MaxWeight',
    'Spinel.Game.Play.AvatarStatusFlags': 'Value',
    'Spinel.Net.Game.Inventory': 'byUid slots',
    'Spinel.Net.Game.InvItem': 'Uid ItemId Count',
    'Spinel.Game.Play.ItemInfo': 'Id Type SubType',
    'Spinel.Game.Play.DrillData': 'Params AutoPets',
    'Spinel.Game.Play.DrillParam': 'Id Grade GroundNature DigDepth BarEffArea BarSpeed ScoopMass DrillLife',
    'Spinel.UI.Pc.UiSkins': 's:chosen s:started',
    'Spinel.UI.Pc.Play.DigKeySettings': 's:semiAuto',
    'Spinel.Engine.Unity.World.ActorView': 'Actor',
    'Spinel.Game.World.WarpRegion': 'X1 Y1 X2 Y2',
    'Spinel.UI.Pc.Screens.GroundItems': 'hover',
    'Spinel.UI.Pc.Screens.GroundItems/HoverDraw': 'Entry',
    'Spinel.UI.Pc.EditBox': 's:SelectedText s:Focused',
    'Spinel.UI.Pc.Hud.HudCover': 's:covers',
    'UnityEngine.EventSystems.EventSystem': 'm_CurrentSelected m_HasFocus m_SelectionGuard s:m_EventSystems',
    'Spinel.UI.Pc.UiPressOrigin': 'left right s:Current',
    'Spinel.Game.Play.AutoDrill': 'State',
}

# Metadata usage cells the sampler dereferences ("!X" is a generic parameter name).
CELLS = [
    ('TypeInfo', 'Spinel.UI.Pc.Screens.GameFlow'),
    ('TypeInfo', 'Spinel.Engine.Unity.SpinelEngine'),
    ('TypeInfo', 'UnityEngine.InputSystem.InputSystem'),
    ('TypeInfo', 'UnityEngine.InputSystem.Controls.ButtonControl'),
    ('TypeInfo', 'UnityEngine.InputSystem.Keyboard'),
    ('TypeInfo', 'Spinel.UI.Pc.UiSkins'),
    ('TypeInfo', 'Spinel.UI.Pc.Play.DigKeySettings'),
    ('TypeInfo', 'Spinel.UI.Pc.EditBox'),
    ('TypeInfo', 'Spinel.UI.Pc.Hud.HudCover'),
    ('TypeInfo', 'UnityEngine.EventSystems.EventSystem'),
    ('TypeInfo', 'Spinel.UI.Pc.UiPressOrigin'),
    ('MethodRef', 'mscorlib.dll|System.Nullable`1::get_HasValue()<vt:UnityEngine.Vector2Int>'),
    ('MethodRef', 'mscorlib.dll|System.Nullable`1::get_Value()<vt:UnityEngine.Vector2Int>'),
    ('MethodRef', 'mscorlib.dll|System.Nullable`1::GetValueOrDefault()<vt:UnityEngine.Vector2>'),
    ('MethodRef', 'mscorlib.dll|System.Nullable`1::get_HasValue()<vt:UnityEngine.Vector2>'),
    ('MethodRef', 'mscorlib.dll|System.Nullable`1::GetValueOrDefault()<i4>'),
    ('MethodRef', 'mscorlib.dll|System.Nullable`1::get_HasValue()<i4>'),
    ('MethodRef', 'mscorlib.dll|System.Nullable`1::GetValueOrDefault()<bool>'),
    ('MethodRef', 'mscorlib.dll|System.Nullable`1::get_HasValue()<bool>'),
    ('MethodRef', 'mscorlib.dll|System.Collections.Generic.Dictionary`2::TryGetValue(!TKey,!TValue&)'
                  '<string,System.Collections.Generic.List`1<Spinel.Game.Play.KeyBinding>>'),
    ('MethodRef', 'mscorlib.dll|System.Collections.Generic.Dictionary`2::TryGetValue(!TKey,!TValue&)'
                  '<System.Type,object>'),
    ('MethodRef', 'mscorlib.dll|System.Collections.Generic.Dictionary`2::ContainsKey(!TKey)<i8,i4>'),
    ('MethodRef', 'Unity.InputSystem.dll|UnityEngine.InputSystem.InputControl`1::get_value()<r4>'),
    ('MethodRef', 'Spinel.Net.dll|Spinel.Net.OrderedMap`2::Get(!TKey)<u8,Spinel.Net.Game.InvItem>'),
    ('MethodRef', 'Spinel.Net.dll|Spinel.Net.OrderedMap`2::Get(!TKey)<i4,Spinel.Engine.Unity.World.ActorView>'),
    ('MethodRef', 'Spinel.Net.dll|Spinel.Net.OrderedMap`2::ContainsKey(!TKey)<i4,Spinel.Game.Play.CombatModel/Aging>'),
]

# Function-table slots compared with Delegate.method_ptr.
SLOTS = ['Spinel.UI.Pc.dll|Spinel.UI.Pc.Play.CarriedWeightUi/<>c__DisplayClass2_0::<Install>b__2()']

# Class RGCTX slots (byte offsets) read through MethodInfo+28.
RGCTX = {
    'System.Nullable`1': (0, 8, 12),
    'System.Collections.Generic.Dictionary`2': (36, 40, 44, 48, 52),
    'UnityEngine.InputSystem.InputControl`1': (4, 16, 28),
    'Spinel.Net.OrderedMap`2': (52, 84),
}

# Managed methods whose source the sampler transcribes; every overload is fingerprinted.
METHODS = {
    'Spinel.Engine.Unity.GameInput': '.ctor Poll get_PointerPosition',
    'Spinel.Engine.Unity.SpinelEngine': 'get_Instance',
    'Spinel.Engine.Unity.Stage': 'MapToScreen ScreenToHost ScreenToMap',
    'Spinel.Game.Play.AutoDrill': 'Tick',
    'Spinel.Game.Play.CombatModel': 'ApplyMessage AttackCancelled OnFsmLeft UnhandledClick WorldClick',
    'Spinel.Game.Play.DigAssist': 'DigKey get_Active get_Busy',
    'Spinel.Game.Play.DrillFeature': '.ctor AvatarUpdate Begin CreateAsync EndDig EndLocal OnAccepted OnFsm OnSpot '
                                     'Preconditions Press PressDig Release Result RightClick Step Update '
                                     'get_MarkerPercent get_MoveMode get_MyPoint get_ProgressPercent get_WornDrill',
    'Spinel.Game.Play.DrillFeature/<CreateAsync>d__123': 'MoveNext',
    'Spinel.Game.Play.GunFlowing': 'HoldsGun Info MayShow get_GunWorn',
    'Spinel.Game.Play.HotkeyMap': 'MakeCatalog Of Rebuild',
    'Spinel.Game.Play.ItemWeight': 'Heavy',
    'Spinel.Game.Play.KeyBindings': 'Matches',
    'Spinel.Game.Play.MyViewWindow': 'get_OkCancelShown',
    'Spinel.Game.Play.RunToggle': 'get_Mode',
    'Spinel.Game.World.ObjectPick': 'Pick',
    'Spinel.Game.World.Portals': 'RegionAt',
    'Spinel.Game.World.WorldActor': 'HitTest get_Moving',
    'Spinel.Net.Game.CombatLink': 'Attack',
    'Spinel.Net.Game.GameSession': 'ApplyHeldStop ApplyMove Handle OwnMoveEchoed RequestHeldMove RequestMove SendMove',
    'Spinel.Net.Game.Inventory': '.ctor Get',
    'Spinel.UI.Pc.Hud.Hud': 'OnKey',
    'Spinel.UI.Pc.Hud.HudCover': 'get_Covered',
    'Spinel.UI.Pc.Play.CarriedWeightUi/<>c__DisplayClass2_0': '<Install>b__2',
    'Spinel.UI.Pc.Play.CarriedWeightUi/<Install>d__2': 'MoveNext',
    'Spinel.UI.Pc.Play.DigKeySettings': 'get_SemiAuto',
    'Spinel.UI.Pc.Play.DrillUi/<>c__DisplayClass3_0': '<Install>b__16 <Install>b__17',
    'Spinel.UI.Pc.Play.DrillUi/<Install>d__3': 'MoveNext',
    'Spinel.UI.Pc.Play.DrillView': 'Click Update',
    'Spinel.UI.Pc.Screens.GameFlow': 'get_Current',
    'Spinel.UI.Pc.Screens.GameScreen': 'get_Stage',
    'Spinel.UI.Pc.Screens.GameplayScreen': 'ActorAt Advance Appear CameraAxis ClickAt ClickWalk HoveredActor MapPoint '
                                           'MapPointer PickTargeting PressTaken PressWalks Rejoin '
                                           'Spinel.UI.Pc.Screens.IPlayContext.get_Hovered UpdateCamera '
                                           'UpdateNameTags get_MoveMode',
    'Spinel.UI.Pc.UiPressOrigin': 'get_LeftOnUi get_RightOnUi',
    'Spinel.UI.Pc.UiSkins': 'get_Current',
    'UnityEngine.InputSystem.Controls.ButtonControl': 'get_isPressed',
    'UnityEngine.InputSystem.InputAction': 'IsPressed',
    'UnityEngine.InputSystem.InputActionState/TriggerState': 'get_isPressed',
    'UnityEngine.InputSystem.InputControl': 'get_currentStatePtr',
    'UnityEngine.InputSystem.Keyboard': 'get_Item get_aKey get_leftAppleKey get_leftCommandKey get_leftMetaKey '
                                        'get_leftWindowsKey get_rightAppleKey get_rightCommandKey get_rightMetaKey '
                                        'get_rightWindowsKey',
    'System.Object': 'GetType',
}

# ---------------------------------------------------------------------------
# Global metadata (format 108) and WebAssembly parsing.
# ---------------------------------------------------------------------------
SECTIONS = ('stringLiteral stringLiteralData string events properties methods parameterDefaultValues '
            'fieldDefaultValues fieldAndParameterDefaultValueData fieldMarshaledSizes parameters fields '
            'genericParameters genericParameterConstraints genericContainers nestedTypes interfaces '
            'vtableMethods interfaceOffsets typeDefinitions typeInlineArrays images assemblies fieldRefs '
            'referencedAssemblies attributeData attributeDataRange unresolvedVirtualCallParameterTypes '
            'unresolvedVirtualCallParameterRanges windowsRuntimeTypeNames windowsRuntimeStrings '
            'exportedTypeDefinitions methodSpecsOnGenericType genericMethodSpecsOnType methodSpecs '
            'genericMethodFunctionsDefinitions genericMethodFunctionsDefinitionsWithAdjustor invokerIndices '
            'rgctxRanges rgctxValues staticConstructorTypeIndices').split()
PRIMITIVE = {1: 'void', 2: 'bool', 3: 'char', 4: 'i1', 5: 'u1', 6: 'i2', 7: 'u2', 8: 'i4', 9: 'u4', 10: 'i8',
             11: 'u8', 12: 'r4', 13: 'r8', 14: 'string', 22: 'typedbyref', 24: 'iptr', 25: 'uptr', 28: 'object'}
USAGE = {1: 'TypeInfo', 2: 'MethodDef', 4: 'String', 5: 'MethodRef', 6: 'FieldRva'}
ADDRESS_MIN = 0x100000  # Constants at or above this are static-data addresses, not semantics.
BLOCK_TYPES = {0x40, 0x7f, 0x7e, 0x7d, 0x7c, 0x7b, 0x70, 0x6f}  # Otherwise a block type is a type index.
GENERATED = [(re.compile(p), r) for p, r in [(r'DisplayClass\d+_\d+', 'DisplayClass#'), (r'd__\d+', 'd__#'),
                                            (r'b__\d+(_\d+)?', 'b__#'), (r'\|\d+_\d+', '|#'),
                                            (r'<>9__\d+_\d+', '<>9__#')]]


def canon(name):
    for pattern, replacement in GENERATED:
        name = pattern.sub(replacement, name)
    return name


def uleb(b, p):
    value = shift = 0
    while True:
        x = b[p]
        p += 1
        value |= (x & 127) << shift
        shift += 7
        if x < 128:
            return value, p


def sleb(b, p):
    value = shift = 0
    while True:
        x = b[p]
        p += 1
        value |= (x & 127) << shift
        shift += 7
        if x < 128:
            return (value - (1 << shift) if x & 64 else value), p


def decoded(path):
    raw = open(path, 'rb').read()
    return gzip.decompress(raw) if raw[:2] == b'\x1f\x8b' else raw


def bundle_entry(data, name):
    if data[:16] != b'UnityWebData1.0\0':
        raise SystemExit('data asset is not UnityWebData1.0')
    end, p = struct.unpack_from('<I', data, 16)[0], 20
    while p < end:
        at, size, length = struct.unpack_from('<III', data, p)
        if data[p + 12:p + 12 + length].decode() == name:
            return data[at:at + size]
        p += 12 + length
    raise SystemExit(f'{name} is absent from the data asset')


def unity_version(data):
    at = data.find(b'UnityFS\0')
    if at < 0:
        raise SystemExit('no UnityFS archive in the data asset')
    return data[at + 12:at + 64].split(b'\0')[1].decode()


class Build:
    def __init__(self, wasm, metadata):
        self.wasm, self.meta = wasm, metadata
        self._metadata()
        self._webassembly()
        self._registration()
        self._modules()

    # -- metadata --
    def _metadata(self):
        b = self.meta
        if struct.unpack_from('<II', b) != (0xfab11baf, 108):
            raise SystemExit('global-metadata is not format 108')
        h = self.h = {n: struct.unpack_from('<III', b, 8 + 12 * i) for i, n in enumerate(SECTIONS)}
        w = self.w = lambda n: 'B' if h[n][2] < 255 else 'H' if h[n][2] < 65535 else 'I'
        tw = self.tw = {2: 'H', 4: 'I'}[h['interfaceOffsets'][1] // h['interfaceOffsets'][2] - 4]

        def rows(name, fmt, keys):
            offset, size, count = h[name]
            record = struct.Struct('<' + fmt)
            if record.size * count != size:
                raise SystemExit(f'unexpected {name} record size')
            return [dict(zip(keys.split(), record.unpack_from(b, offset + i * record.size))) for i in range(count)]
        self.rows = rows
        self.types = rows('typeDefinitions', 'II' + tw * 3 + w('genericContainers') + 'I' + w('fields') +
                          w('methods') + w('events') + w('properties') + w('nestedTypes') + w('interfaceOffsets') +
                          'I' + w('interfaceOffsets') + 'H' * 8 + 'II',
                          'name namespace byval declaring parent generic flags fieldStart methodStart eventStart '
                          'propertyStart nestedStart interfacesStart vtableStart interfaceOffsetsStart methodCount '
                          'propertyCount fieldCount eventCount nestedCount vtableCount interfacesCount '
                          'interfaceOffsetsCount bitfield token')
        self.methods = rows('methods', 'I' + w('typeDefinitions') + tw + 'I' + w('parameters') +
                            w('genericContainers') + 'IHHHH',
                            'name declaring returnType returnToken parameterStart generic token flags iflags slot '
                            'parameterCount')
        self.fields = rows('fields', 'I' + tw + 'I', 'name type token')
        self.params = rows('parameters', 'II' + tw, 'name token type')
        self.generic_params = rows('genericParameters', w('genericContainers') + 'I' +
                                   w('genericParameterConstraints') + 'HHH',
                                   'owner name constraintsStart constraintsCount num flags')
        self.images = rows('images', 'II' + w('typeDefinitions') + 'I' + w('typeDefinitions') + 'I' +
                           w('methods') + 'IIIIII' + w('typeDefinitions') + 'I',
                           'name assembly typeStart typeCount exportedStart exportedCount entrypoint token '
                           'customStart customCount invokerStart rgctxStart rgctxCount staticStart staticCount')
        for image in self.images:
            image['name'] = self.string(image['name'])
            for t in self.types[image['typeStart']:image['typeStart'] + image['typeCount']]:
                t['image'] = image['name']
        for t in self.types:
            t['name'], t['namespace'] = self.string(t['name']), self.string(t['namespace'])
        for m in self.methods:
            m['name'] = self.string(m['name'])

    def string(self, index):
        at = self.h['string'][0] + index
        return self.meta[at:self.meta.index(b'\0', at)].decode()

    # -- WebAssembly --
    def _webassembly(self):
        b = self.wasm
        if b[:8] != b'\0asm\1\0\0\0':
            raise SystemExit('WASM asset is not a version 1 module')
        sections, p = {}, 8
        while p < len(b):
            size, q = uleb(b, p + 1)
            sections.setdefault(b[p], (q, q + size))
            p = q + size
        count, p = uleb(b, sections[1][0])
        self.signatures = []
        for _ in range(count):
            n, p = uleb(b, p + 1)
            params = b[p:p + n]
            n, p = uleb(b, p + n)
            self.signatures.append((params.hex(), b[p:p + n].hex()))
            p += n
        count, p = uleb(b, sections[2][0])
        self.imports = []
        for _ in range(count):
            n, p = uleb(b, p)
            module = b[p:p + n].decode()
            n, p = uleb(b, p + n)
            field, kind = b[p:p + n].decode(), b[p + n]
            p += n + 1
            if kind == 0:
                _, p = uleb(b, p)
                self.imports.append(module + '.' + field)
            elif kind in (1, 2):
                p += kind == 1
                flags, p = uleb(b, p)
                _, p = uleb(b, p)
                if flags & 1:
                    _, p = uleb(b, p)
            else:
                p += 2
        count, p = uleb(b, sections[9][0])
        flags, p = uleb(b, p)
        if count != 1 or flags != 0 or b[p] != 0x41:
            raise SystemExit('unexpected element section')
        start, p = sleb(b, p + 1)
        n, p = uleb(b, p + 1)
        self.table = [None] * start
        for _ in range(n):
            f, p = uleb(b, p)
            self.table.append(f)
        segments = []
        count, p = uleb(b, sections[11][0])
        for _ in range(count):
            flags, p = uleb(b, p)
            if flags != 0 or b[p] != 0x41:
                raise SystemExit('unexpected data segment')
            at, p = sleb(b, p + 1)
            size, p = uleb(b, p + 1)
            segments.append((at, b[p:p + size]))
            p += size
        self.data_start = min(at for at, _ in segments)
        self.data_end = max(at + len(d) for at, d in segments)
        self.mem = bytearray(self.data_end)
        for at, d in segments:
            self.mem[at:at + len(d)] = d
        count, p = uleb(b, sections[10][0])
        self.bodies = {}
        for f in range(len(self.imports), len(self.imports) + count):
            size, p = uleb(b, p)
            self.bodies[f] = (p, size)
            p += size

    def u32(self, a):
        return struct.unpack_from('<I', self.mem, a)[0]

    def i32(self, a):
        return struct.unpack_from('<i', self.mem, a)[0]

    # -- registrations (located structurally, never by address) --
    def _registration(self):
        n = len(self.types)
        found, pattern, q = [], struct.pack('<I', n), self.data_start
        while (q := self.mem.find(pattern, q)) >= 0:
            if q % 4 == 0 and q >= 24 and self.u32(q + 8) == n and \
                    all(0 < self.u32(q + k) < self.data_end for k in (4, 12)):
                found.append(q - 24)
            q += 1
        if len(found) != 1:
            raise SystemExit(f'expected one metadata registration, found {len(found)}')
        r = [self.u32(found[0] + 4 * i) for i in range(10)]
        self.generic_insts, self.types_count, self.type_array = r[3], r[4], r[5]
        self.field_offsets, self.type_sizes = r[7], r[9]

    def _modules(self):
        """Join each image's code-generation module to its method-pointer table."""
        rids = {}
        for m in self.methods:
            image = self.types[m['declaring']].get('image')
            rids[image] = max(rids.get(image, 0), m['token'] & 0xffffff)
        modules = {}
        for image in self.images:
            name, found = image['name'], []
            at = self.mem.find(name.encode() + b'\0', self.data_start)
            while at >= 0:
                q, pattern = self.data_start, struct.pack('<I', at)
                while (q := self.mem.find(pattern, q)) >= 0:
                    if q % 4 == 0:
                        _, count, table = struct.unpack_from('<III', self.mem, q)
                        if count == rids.get(name) and 0 < table < self.data_end - 4 * count and \
                                all(self.u32(table + 4 * i) < len(self.table) for i in range(count)):
                            found.append(table)
                    q += 1
                at = self.mem.find(name.encode() + b'\0', at + 1)
            if len(found) > 1:
                raise SystemExit(f'ambiguous code module for {name}')
            if found:
                modules[name] = found[0]
        self.slot_of, self.methods_at = {}, {}
        for i, m in enumerate(self.methods):
            table = modules.get(self.types[m['declaring']].get('image'))
            if table is None:
                continue
            slot = self.u32(table + 4 * ((m['token'] & 0xffffff) - 1))
            if slot:
                self.slot_of[i] = slot
                self.methods_at.setdefault(self.table[slot], []).append(i)

    # -- names --
    def il2type(self, index):
        a = self.u32(self.type_array + 4 * index)
        return a, self.u32(a), self.u32(a + 4)

    def typedef_of(self, index):
        if index >= self.types_count:
            return None
        _, data, bits = self.il2type(index)
        return data if (bits >> 16) & 0xff in (17, 18) else None

    def generic_definition(self, index):
        """Type index of a class or generic instance -> its type definition."""
        _, data, bits = self.il2type(index)
        if (bits >> 16) & 0xff == 21:
            definition = self.u32(data)
            data, bits = self.u32(definition), self.u32(definition + 4)
        if (bits >> 16) & 0xff not in (17, 18):
            raise SystemExit(f'type {index} has no definition')
        return data

    def fullname(self, ti):
        t, names = self.types[ti], []
        while True:
            names.append(t['name'])
            outer = self.typedef_of(t['declaring'])
            if outer is None:
                break
            t = self.types[outer]
        return (t['namespace'] + '.' if t['namespace'] else '') + '/'.join(reversed(names))

    def render(self, a, depth=0):
        data, bits = self.u32(a), self.u32(a + 4)
        kind, suffix = (bits >> 16) & 0xff, '&' if bits >> 29 & 1 else ''
        if depth > 6:
            return '...'
        if kind in PRIMITIVE:
            return PRIMITIVE[kind] + suffix
        if kind in (17, 18):
            return ('vt:' if kind == 17 else '') + self.fullname(data) + suffix
        if kind in (15, 29):
            return self.render(data, depth + 1) + ('*' if kind == 15 else '[]') + suffix
        if kind == 20:
            return self.render(self.u32(data), depth + 1) + '[,]' + suffix
        if kind == 21:
            return self.render(self.u32(data), depth + 1) + '<' + ','.join(
                self.inst(self.u32(data + 4), depth + 1)) + '>' + suffix
        if kind in (19, 30):
            return ('!' if kind == 19 else '!!') + self.string(self.generic_params[data]['name']) + suffix
        return f'kind{kind}:{data}{suffix}'

    def inst(self, g, depth=0):
        count, argv = self.u32(g), self.u32(g + 4)
        return [self.render(self.u32(argv + 4 * i), depth) for i in range(count)]

    def method_name(self, mi):
        m = self.methods[mi]
        params = [self.render(self.il2type(self.params[m['parameterStart'] + j]['type'])[0])
                  for j in range(m['parameterCount'])]
        return f"{self.types[m['declaring']].get('image')}|{self.fullname(m['declaring'])}::{m['name']}" \
               f"({','.join(params)})"

    def method_spec(self, i):
        """MethodRef/RGCTX method index -> generic method instance name."""
        n1, n2 = self.h['methodSpecsOnGenericType'][2], self.h['genericMethodSpecsOnType'][2]
        if i < n1 + n2:
            table, j, fmt = ('methodSpecsOnGenericType', i, '<IH') if i < n1 else ('genericMethodSpecsOnType',
                                                                                  i - n1, '<IH')
            mi, g = struct.unpack_from(fmt, self.meta, self.h[table][0] + 6 * j)
            on_type, on_method = (g, None) if i < n1 else (None, g)
        else:
            mi, on_type, on_method = struct.unpack_from('<IHH', self.meta, self.h['methodSpecs'][0] + 8 * (i - n1 - n2))
            on_type = None if on_type == 0xffff else on_type
            on_method = None if on_method == 0xffff else on_method
        name = self.method_name(mi)
        if on_type is not None:
            name += '<' + ','.join(self.inst(self.u32(self.generic_insts + 4 * on_type))) + '>'
        if on_method is not None:
            name += '[' + ','.join(self.inst(self.u32(self.generic_insts + 4 * on_method))) + ']'
        return name

    def usage(self, value):
        """Decode one metadata usage word, or None."""
        kind, index = value >> 29, (value & 0x1fffffff) >> 1
        if not value & 1 or kind not in USAGE:
            return None
        try:
            if kind == 1 and index < self.types_count:
                return 'TypeInfo', self.render(self.il2type(index)[0])
            if kind == 2 and index < len(self.methods):
                return 'MethodDef', self.method_name(index)
            if kind == 4 and index < self.h['stringLiteral'][2]:
                base, size = self.h['stringLiteralData'][:2]
                lo, = struct.unpack_from('<I', self.meta, self.h['stringLiteral'][0] + 4 * index)
                hi = size if index + 1 == self.h['stringLiteral'][2] else \
                    struct.unpack_from('<I', self.meta, self.h['stringLiteral'][0] + 4 * index + 4)[0]
                return 'String', self.meta[base + lo:base + hi].decode('utf-8', 'replace')
            if kind == 5 and index < self.h['methodSpecsOnGenericType'][2] + self.h['genericMethodSpecsOnType'][2]:
                return 'MethodRef', self.method_spec(index)
            if kind == 6 and index < self.h['fieldRefs'][2]:
                row = struct.Struct('<' + self.tw + 'I')
                ti, fi = row.unpack_from(self.meta, self.h['fieldRefs'][0] + row.size * index)
                owner = self.typedef_of(ti)
                field = self.string(self.fields[self.types[owner]['fieldStart'] + fi]['name'])
                return 'FieldRva', self.render(self.il2type(ti)[0]) + '.' + field
        except (IndexError, KeyError, TypeError, ValueError, struct.error):
            return None  # Not a usage word after all.
        return None

    def cell(self, address):
        if address % 4 or address < 1024 or address + 4 > self.data_end:
            return None
        return self.usage(self.u32(address))


def short(name):
    m = re.fullmatch(r'<(.+)>k__BackingField', name)
    return m.group(1) if m else name


# ---------------------------------------------------------------------------
# Body fingerprints: renumbering- and relocation-insensitive instruction text.
# ---------------------------------------------------------------------------
def instructions(code):
    """Yield (offset, opcode, immediates) for one function body; the local declarations come first."""
    p, groups = 0, []
    n, p = uleb(code, 0)
    for _ in range(n):
        count, p = uleb(code, p)
        groups.append((count, code[p]))
        p += 1
    yield p, 'locals', tuple(groups)
    depth = 1
    while depth:
        at, op, p = p, code[p], p + 1
        if op in (0x02, 0x03, 0x04, 0x06):
            depth += 1
            if code[p] in BLOCK_TYPES:
                imm, p = (code[p],), p + 1
            else:
                v, p = sleb(code, p)
                imm = (v,)
        elif op in (0x0b, 0x18):
            depth -= 1
            imm = ()
            if op == 0x18:
                v, p = uleb(code, p)
                imm = (v,)
        elif op in (0x07, 0x08, 0x09, 0x0c, 0x0d, 0x10, 0x12, 0xd2) or 0x20 <= op <= 0x26:
            v, p = uleb(code, p)
            imm = (v,)
        elif op == 0x0e:
            n, p = uleb(code, p)
            labels = []
            for _ in range(n + 1):
                v, p = uleb(code, p)
                labels.append(v)
            imm = tuple(labels)
        elif op in (0x11, 0x13):
            t, p = uleb(code, p)
            table, p = uleb(code, p)
            imm = (t, table)
        elif op == 0x1c:
            n, p = uleb(code, p)
            imm, p = tuple(code[p:p + n]), p + n
        elif 0x28 <= op <= 0x3e:
            a, p = uleb(code, p)
            o, p = uleb(code, p)
            imm = ('mem', a, o)
        elif op in (0x3f, 0x40, 0xd0):
            imm, p = (code[p],), p + 1
        elif op in (0x41, 0x42):
            v, p = sleb(code, p)
            imm = (v,)
        elif op in (0x43, 0x44):
            n = 4 if op == 0x43 else 8
            imm, p = (code[p:p + n].hex(),), p + n
        elif op == 0xfc:
            sub, p = uleb(code, p)
            imm = (sub,)
            if sub in (8, 9, 12, 13, 14, 15, 16, 17):
                v, p = uleb(code, p)
                imm += (v,)
                if sub in (12, 14):
                    v, p = uleb(code, p)
                    imm += (v,)
            if sub in (8, 10, 11):
                n = 2 if sub == 10 else 1
                imm, p = imm + tuple(code[p:p + n]), p + n
        elif op == 0xfd:
            sub, p = uleb(code, p)
            imm = (sub,)
            if sub <= 0x0b or sub in (0x5c, 0x5d) or 0x54 <= sub <= 0x5b:
                a, p = uleb(code, p)
                o, p = uleb(code, p)
                imm = (sub, 'mem', a, o)
                if 0x54 <= sub <= 0x5b:
                    imm, p = imm + (code[p],), p + 1
            elif sub in (0x0c, 0x0d):
                imm, p = (sub, code[p:p + 16].hex()), p + 16
            elif 0x15 <= sub <= 0x22:
                imm, p = (sub, code[p]), p + 1
        elif op in (0x00, 0x01, 0x05, 0x0f, 0x19, 0x1a, 0x1b, 0xd1) or 0x45 <= op <= 0xc4:
            imm = ()
        else:
            raise SystemExit(f'unknown opcode {op:#x}')
        yield at, op, imm
    if p != len(code):
        raise SystemExit('function body has trailing bytes')


def static_operand(build, value, address):
    """A constant that names a metadata cell keeps that name; other data addresses stay anonymous."""
    usage = build.cell(value)
    if usage:
        return usage[0], canon(usage[1])
    return ('address',) if address else (value,)


def fingerprint(build, fn):
    at, size = build.bodies[fn]
    out = []
    for _, op, imm in instructions(build.wasm[at:at + size]):
        if op in (0x10, 0x12, 0xd2):
            f = imm[0]
            if f < len(build.imports):
                imm = ('import', build.imports[f])
            elif f in build.methods_at:
                imm = ('managed',) + tuple(sorted(canon(build.method_name(m)) for m in build.methods_at[f]))
            else:
                imm = ('native',)
        elif op in (0x11, 0x13):
            imm = (build.signatures[imm[0]], imm[1])
        elif op == 0x41 and imm[0] >= 1024:
            imm = static_operand(build, imm[0], imm[0] >= ADDRESS_MIN)
        elif 'mem' in imm:
            i = imm.index('mem') + 2  # memarg offset
            if imm[i] >= ADDRESS_MIN:
                imm = imm[:i] + static_operand(build, imm[i], True) + imm[i + 1:]
        out.append(f'{op} {imm!r}')
    return hashlib.sha256('\n'.join(out).encode()).hexdigest()


# ---------------------------------------------------------------------------
# Derivation.
# ---------------------------------------------------------------------------
def unique(found, what):
    if len(found) != 1:
        raise SystemExit(f'{what}: expected one match, found {len(found)}')
    return found[0]


def rgctx_slots(build, ti):
    """Runtime slots of a class RGCTX: constrained (5,6) and field (7,8) pairs share one slot."""
    t = build.types[ti]
    image = next(i for i in build.images if i['name'] == t['image'])
    for r in range(image['rgctxStart'], image['rgctxStart'] + image['rgctxCount']):
        token, start, count = struct.unpack_from('<III', build.meta, build.h['rgctxRanges'][0] + 12 * r)
        if token != t['token']:
            continue
        values = [struct.unpack_from('<BI', build.meta, build.h['rgctxValues'][0] + 5 * j)
                  for j in range(start, start + count)]
        slots, i = [], 0
        while i < len(values):
            kind, data = values[i]
            if kind in (1, 2, 4):
                slots.append(f"{'type class method array'.split()[kind - 1]} {build.render(build.il2type(data)[0])}")
            elif kind == 3:
                slots.append('method ' + build.method_spec(data))
            elif kind in (5, 7):
                pair_kind, pair = values[i + 1]
                if pair_kind != kind + 1:
                    raise SystemExit('unpaired RGCTX entry')
                owner = build.render(build.il2type(data)[0])
                if kind == 7:
                    first = build.types[build.generic_definition(data)]['fieldStart']
                    slots.append(f"field {owner}.{build.string(build.fields[first + pair]['name'])}")
                else:
                    slots.append(f'constrained {owner} {build.usage(pair)}')
                i += 1
            else:
                raise SystemExit(f'unknown RGCTX kind {kind}')
            i += 1
        return slots
    raise SystemExit(f'no RGCTX for {build.fullname(ti)}')


def derive(build, build_id, unity, artifacts):
    names = {}
    for i in range(len(build.types)):
        name = build.fullname(i)
        names.setdefault(name, []).append(i)
    tid = {name: unique(names.get(name, []), name) for name in TYPES}
    out = dict(build=build_id, unity=unity, artifacts=artifacts, typedefMax=len(build.types) - 1, types=tid)
    kinds, sizes = {}, {}
    for name, ti in tid.items():
        _, data, bits = build.il2type(build.types[ti]['byval'])
        if data != ti:
            raise SystemExit(f'{name}: byval type does not name its definition')
        if (bits >> 16) & 0xff != 18:
            kinds[name] = (bits >> 16) & 0xff
        sizes[name] = build.u32(build.u32(build.type_sizes + 4 * ti))
    out['typeKinds'], out['instanceSizes'] = kinds, sizes
    fields = {}
    for owner, spec in FIELDS.items():
        ti = unique(names.get(owner, []), owner)
        t, offsets = build.types[ti], build.u32(build.field_offsets + 4 * ti)
        have = {}
        for j in range(t['fieldCount']):
            f = build.fields[t['fieldStart'] + j]
            attrs = build.il2type(f['type'])[2] & 0xffff
            if attrs & 0x40:  # literal: no storage
                continue
            key = ('s:' if attrs & 0x10 else '') + short(build.string(f['name']))
            have[key] = build.i32(offsets + 4 * j)
        for name in spec.split():
            if name not in have:
                raise SystemExit(f'{owner}: field {name} is absent')
            fields[f"{owner}{'::' if name.startswith('s:') else '.'}{name.removeprefix('s:')}"] = have[name]
    out['fields'] = fields
    # Cells: candidate usage values by name, then one scan of initialized data.
    wanted = {}
    by_def = {}
    for i in range(build.types_count):
        a, data, bits = build.il2type(i)
        if (bits >> 16) & 0xff in (17, 18) and not bits >> 29 & 1:
            by_def.setdefault(data, []).append(i)
    for kind, name in CELLS:
        if kind == 'TypeInfo':
            for i in by_def.get(tid[name], []):
                wanted[(1 << 29) | (i << 1) | 1] = (kind, name)
    refs = {name for kind, name in CELLS if kind == 'MethodRef'}
    for i in range(build.h['methodSpecsOnGenericType'][2] + build.h['genericMethodSpecsOnType'][2]):
        name = build.method_spec(i)
        if name in refs:
            wanted[(5 << 29) | (i << 1) | 1] = ('MethodRef', name)
    hits = {}
    start = build.data_start - build.data_start % 4
    for k, (v,) in enumerate(struct.iter_unpack('<I', build.mem[start:build.data_end - (build.data_end - start) % 4])):
        if v in wanted:
            hits.setdefault(wanted[v], []).append(start + 4 * k)
    out['cells'] = {f'{kind} {name}': unique(hits.get((kind, name), []), f'{kind} {name}') for kind, name in CELLS}
    by_name = {}
    for mi in build.slot_of:
        by_name.setdefault(build.method_name(mi), []).append(mi)
    out['slots'] = {name: build.slot_of[unique(by_name.get(name, []), name)] for name in SLOTS}
    out['rgctx'] = {}
    for owner, offsets in RGCTX.items():
        slots = rgctx_slots(build, unique(names.get(owner, []), owner))
        out['rgctx'][owner] = {'+%d' % o: slots[o // 4] for o in offsets}
        out['rgctx'][owner]['sha256'] = hashlib.sha256('\n'.join(slots).encode()).hexdigest()
    out['methods'] = {}
    for owner, spec in METHODS.items():
        ti = unique(names.get(owner, []), owner)
        t = build.types[ti]
        for wanted_name in spec.split():
            found = [t['methodStart'] + j for j in range(t['methodCount'])
                     if build.methods[t['methodStart'] + j]['name'] == wanted_name and
                     t['methodStart'] + j in build.slot_of]
            if not found:
                raise SystemExit(f'{owner}::{wanted_name} has no compiled body')
            for mi in found:
                out['methods'][build.method_name(mi)] = fingerprint(build, build.table[build.slot_of[mi]])
    return out


# Sections that hold build-local indices and addresses. Every other difference is a
# layout, generic-context, code or toolchain change that needs a semantic review.
MOVES = ('typedefMax', 'types', 'cells', 'slots')


def compare(old, new):
    moved, changed = [], []
    for section in sorted((set(old) | set(new)) - {'build', 'artifacts'}):
        a, b = old.get(section), new.get(section)
        rows = [(key, a.get(key), b.get(key)) for key in sorted(set(a) | set(b)) if a.get(key) != b.get(key)] \
            if isinstance(a, dict) and isinstance(b, dict) else [('', a, b)] if a != b else []
        for key, before, after in rows:
            (moved if section in MOVES else changed).append(f'{section} {key}: {before!r} -> {after!r}')
    return moved, changed


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--build', required=True, help='public build id from the version descriptor')
    parser.add_argument('wasm')
    parser.add_argument('data')
    parser.add_argument('--expect', help='pins JSON from the previously supported build')
    parser.add_argument('--out', help='write the derived pins JSON here')
    args = parser.parse_args()
    wasm, data = decoded(args.wasm), decoded(args.data)
    metadata = bundle_entry(data, 'Il2CppData/Metadata/global-metadata.dat')
    artifacts = {k: hashlib.sha256(v).hexdigest() for k, v in (('wasm', wasm), ('data', data), ('metadata', metadata))}
    pins = derive(Build(wasm, metadata), args.build, unity_version(data), artifacts)
    text = json.dumps(pins, indent=1, sort_keys=True) + '\n'
    if args.out:
        open(args.out, 'w').write(text)
    else:
        sys.stdout.write(text)
    if args.expect:
        moved, changed = compare(json.load(open(args.expect)), pins)
        for label, rows in (('MOVED', moved), ('CHANGED', changed)):
            for line in rows:
                print(label, line, file=sys.stderr)
        print(f'{len(moved)} pin(s) moved and {len(changed)} changed since {args.expect}', file=sys.stderr)
        return 1 if changed else 0
    return 0


if __name__ == '__main__':
    sys.exit(main())
