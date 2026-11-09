"use strict";
// Invented-heap tests for src/sampler.js. Every object is laid out from the pinned build in
// tools/build-migrate/sampler/ea1a68bd.json, and every sampler read must land on a pinned field of a
// matching type. No target runtime state is encoded here.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const crypto = require("node:crypto");
const script = fs.readFileSync(path.join(__dirname, "../src/sampler.js"), "utf8");
const pins = JSON.parse(fs.readFileSync(path.join(__dirname, "../../../tools/build-migrate/sampler/ea1a68bd.json"), "utf8"));

// libil2cpp structures the sampler reads. Metadata does not describe them; each offset was read off this
// Unity version's compiled libil2cpp (docs/research/sampler-heap-reads.md).
const UNITY = "6000.6.5f1";
const HEADER = pins.runtime.typeDefinitionsAt;
const RUNTIME = {
  Il2CppGlobalMetadataHeader: {sanity: [0, "u4"], typeDefinitionsOffset: [HEADER, "u4"], typeDefinitionsSize: [HEADER + 4, "u4"], typeDefinitionsCount: [HEADER + 8, "u4"]},
  Il2CppObject: {klass: [0, "ptr"]},
  Il2CppType: {data: [0, "ptr"], type: [6, "u1"]},
  Il2CppGenericClass: {type: [0, "ptr"], class_inst: [4, "ptr"]},
  Il2CppGenericInst: {type_argc: [0, "u4"], type_argv: [4, "ptr"]},
  Il2CppClass: {"byval_arg.data": [16, "ptr"], "byval_arg.type": [22, "u1"], element_class: [32, "ptr"], interfaceOffsets: [72, "ptr"], static_fields: [76, "ptr"],
    instance_size: [128, "u4"], stack_slot_size: [132, "u4"], element_size: [140, "u4"], interface_offsets_count: [182, "u2"]},
  Il2CppRuntimeInterfaceOffsetPair: {interfaceType: [0, "ptr"], offset: [4, "i4"]},
  VirtualInvokeData: {method: [4, "ptr"]},
  Il2CppArray: {klass: [0, "ptr"], bounds: [8, "ptr"], max_length: [12, "i4"]},
  MethodInfo: {klass: [16, "ptr"], rgctx_data: [28, "ptr"]},
};
const VTABLE = 192, VTABLE_ENTRY = 8, ARRAY_DATA = 16;
const STRIDE = pins.runtime.typeDefinitions[1] / pins.runtime.typeDefinitions[2];
assert.equal(pins.unity, UNITY, "re-prove RUNTIME against the new Unity version's libil2cpp");
assert(Number.isInteger(STRIDE));
// Generic parameters of the pinned RGCTX owners, in declaration order.
const PARAMS = {"System.Nullable`1": ["T"], "UnityEngine.InputSystem.InputControl`1": ["TValue"], "System.Collections.Generic.Dictionary`2": ["TKey", "TValue"],
  "System.Collections.Generic.KeyValuePair`2": ["TKey", "TValue"], "Spinel.Net.OrderedMap`2": ["TKey", "TValue"]};

// Type text is derive.py's: i4, bool, object, vt:Name, Name<args>, X[], X*. A type spec is a name or [name, ...args].
const ACCESS = {bool: "byte", u1: "byte", i1: "byte", char: "short", u2: "short", i2: "short", i4: "int", u4: "int", r4: "float", r8: "double", i8: "long", u8: "long", iptr: "ptr", uptr: "ptr", ptr: "ptr"};
const GETTERS = {byte: ["getUint8"], short: ["getUint16"], int: ["getInt32", "getUint32"], float: ["getFloat32"], double: ["getFloat64"], long: ["getBigUint64", "getBigInt64"], ptr: ["getUint32"]};
const WIDTH = {byte: 1, short: 2, int: 4, float: 4, double: 8, long: 8, ptr: 4};
const PRIM = {"System.Boolean": "bool", "System.Char": "char", "System.Int32": "i4", "System.Int64": "i8", "System.UInt64": "u8", "System.Single": "r4", "System.Double": "r8"};
const NAMED = {...Object.fromEntries(Object.entries(PRIM).map(([name, text]) => [text, name])), object: "System.Object", string: "System.String"};
const VALUE_KINDS = new Set([2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 17]);
const inline = text => text.startsWith("vt:") && !/(\[\]|\*)$/.test(text);
const accessOf = text => ACCESS[text] ?? (inline(text) ? assert.fail(`${text} is stored inline`) : "ptr");
const nameOf = t => typeof t === "string" ? t : t[0];
const isArray = t => Array.isArray(t) && t[0] === "[]";
const key = t => typeof t === "string" ? t : isArray(t) ? `${key(t[1])}[]` : `${t[0]}<${t.slice(1).map(key).join(",")}>`;
const defKind = t => pins.typeKinds[nameOf(t)] ?? 18;
const isValue = t => !isArray(t) && VALUE_KINDS.has(defKind(t));
const render = t => PRIM[t] ?? `${isValue(t) ? "vt:" : ""}${typeof t === "string" ? t : `${t[0]}<${t.slice(1).map(render).join(",")}>`}`;
const shape = t => PRIM[t] ?? (isValue(t) ? render(t) : "object");
// derive.py lays out generic instances by argument shape. An unpinned one is opaque: no read may enter it.
const layoutOf = t => pins.generics[`${t[0]}<${t.slice(1).map(shape).join(",")}>`];
function parse(text) {
  let i = 0;
  const one = () => {
    if (text.startsWith("vt:", i)) i += 3;
    const start = i;
    while (i < text.length && !"<>,".includes(text[i])) i++;
    const name = NAMED[text.slice(start, i)] ?? text.slice(start, i);
    if (text[i] !== "<") return name;
    const t = [name];
    do { i++; t.push(one()); } while (text[i] === ",");
    assert.equal(text[i++], ">", text);
    return t;
  };
  const t = one();
  assert.equal(i, text.length, text);
  return t;
}
const bind = (t, names) => typeof t !== "string" ? t.map(a => bind(a, names)) : t[0] !== "!" ? t : names[t.slice(1)] ?? assert.fail(`unbound ${t}`);

// Pinned fields by owner. Field names hold no ".", static ones follow "::".
const OWN = new Map(), STATIC = new Map();
for (const [label, [offset, type]] of Object.entries(pins.fields)) {
  const s = label.indexOf("::"), cut = s >= 0 ? s : label.lastIndexOf("."), map = s >= 0 ? STATIC : OWN, owner = label.slice(0, cut);
  if (!map.has(owner)) map.set(owner, []);
  map.get(owner).push({name: label.slice(cut + (s >= 0 ? 2 : 1)), offset, type, credit: label});
}
const entries = fields => Object.entries(fields).map(([name, [offset, type]]) => ({name, offset, type, credit: null}));
function fieldsOf(t) {
  const own = typeof t === "string" ? OWN.get(t) ?? [] : entries(layoutOf(t)?.fields ?? {});
  return [...own, ...(pins.bases[nameOf(t)] ?? []).flatMap(b => pins.generics[b] ? entries(pins.generics[b].fields) : OWN.get(b) ?? [])];
}
function sizeOf(t) {
  if (isArray(t)) return ARRAY_DATA;
  if (typeof t !== "string") return layoutOf(t)?.size ?? 0;
  assert(pins.instanceSizes[t] !== undefined, `no pinned size for ${t}`);
  return pins.instanceSizes[t];
}
function slotSize(t) {
  if (!isValue(t)) return 4;
  assert(sizeOf(t) > 8, `no pinned size for ${key(t)}`);
  return sizeOf(t) - 8;
}
// Leaf fields with offsets relative to the object (static storage when statik). Value-type offsets in
// metadata include the 8-byte object header, so an inline value type starts 8 bytes before its offsets.
const LEAVES = new Map();
function leavesOf(t, statik = false) {
  const label = `${statik ? "static " : ""}${key(t)}`;
  if (!LEAVES.has(label)) {
    const out = [];
    const walk = (fields, base, prefix, credit) => {
      for (const f of fields) {
        const c = f.credit ? [...credit, f.credit] : credit, at = base + f.offset, name = prefix + f.name;
        if (!inline(f.type)) out.push({path: name, offset: at, type: f.type, credit: c});
        else if (pins.enums[f.type.slice(3)]) out.push({path: name, offset: at, type: pins.enums[f.type.slice(3)], credit: c});
        else walk(fieldsOf(parse(f.type)), at - 8, `${name}.`, c);
      }
    };
    walk(statik ? STATIC.get(t) ?? [] : fieldsOf(t), 0, "", []);
    LEAVES.set(label, out);
  }
  return LEAVES.get(label);
}
function typed(type, access, value, label) {
  const ok = type === "bool" ? typeof value === "boolean" : access === "long" ? typeof value === "bigint" || Number.isSafeInteger(value)
    : access === "float" || access === "double" ? typeof value === "number" : access === "ptr" ? Number.isSafeInteger(value) && value >= 0 : Number.isSafeInteger(value);
  assert(ok, `${label} is ${type}, not ${typeof value} ${value}`);
}

const COVERED = new Set();
// A 16 MiB invented heap. Objects, classes, generic contexts and metadata cells exist only where pinned.
class Heap {
  constructor() {
    this.buffer = new ArrayBuffer(16 << 20);
    this.view = new DataView(this.buffer);
    this.next = 0x500000;
    this.fields = new Map();
    this.layouts = new Map();
    this.memo = new Map();
    this.violations = [];
    // libil2cpp loads global-metadata.dat at s_GlobalMetadata and points s_GlobalMetadataHeader at it.
    this.metadata = this.alloc(HEADER + 12);
    this.struct(this.metadata, "Il2CppGlobalMetadataHeader");
    this.write(this.metadata, "int", 0xfab11baf);
    pins.runtime.typeDefinitions.forEach((v, i) => this.write(this.metadata + HEADER + 4 * i, "int", v));
    for (const label of ["s_GlobalMetadata", "s_GlobalMetadataHeader"]) this.cell(pins.runtime[label], label, this.metadata);
    for (const [label, cell] of Object.entries(pins.cells)) {
      const info = /^TypeInfo (.+)$/.exec(label), ref = /^MethodRef [^|]+\|(.+)::([^(]+)\([^)]*\)<(.+)>$/.exec(label);
      assert(info || ref, label);
      if (info) this.statics(info[1]);
      this.cell(cell, label, info ? this.cls(info[1]) : this.method(parse(`${ref[1]}<${ref[3]}>`), ref[2]));
    }
  }
  alloc(n) { const p = this.next; this.next = (p + n + 7) & ~7; assert(this.next < this.buffer.byteLength); return p; }
  once(label, make) { if (!this.memo.has(label)) this.memo.set(label, make()); return this.memo.get(label); }
  u32(p) { return this.view.getUint32(p, true); }
  declare(p, label, access, credit = []) {
    const old = this.fields.get(p);
    assert(!old || old.label === label, `${label} overlaps ${old?.label} at ${p}`);
    this.fields.set(p, {label, access, credit});
  }
  struct(p, name) {
    for (const [field, [offset, type]] of Object.entries(RUNTIME[name])) this.declare(p + offset, `${name}.${field}`, accessOf(type), [`${name}.${field}`]);
  }
  cell(p, label, value) { this.declare(p, label, "ptr", [label]); this.write(p, "ptr", value); }
  write(p, access, v) {
    const view = this.view;
    if (access === "byte") view.setUint8(p, Number(v));
    else if (access === "short") view.setUint16(p, v, true);
    else if (access === "int") v < 0 ? view.setInt32(p, v, true) : view.setUint32(p, v, true);
    else if (access === "float") view.setFloat32(p, v, true);
    else if (access === "double") view.setFloat64(p, v, true);
    else if (access === "long") view.setBigUint64(p, BigInt.asUintN(64, BigInt(v)), true);
    else if (access === "ptr") view.setUint32(p, v, true);
    else assert.fail(access);
  }
  // Deliberately malformed or unpinned bytes for refusal cases.
  raw(p, access, v) { this.write(p, access, v); }
  place(p, prefix, leaves) {
    const map = new Map();
    for (const leaf of leaves) {
      assert(!map.has(leaf.path), `${prefix}${leaf.path} is pinned twice`);
      this.declare(p + leaf.offset, prefix + leaf.path, accessOf(leaf.type), leaf.credit);
      map.set(leaf.path, leaf);
    }
    this.layouts.set(p, map);
  }
  leaf(p, field) { const leaf = this.layouts.get(p)?.get(field); assert(leaf, `${field} is not a pinned field at ${p}`); return leaf; }
  addr(p, field) { return p + this.leaf(p, field).offset; }
  set(p, field, value) {
    if (typeof field === "object") { for (const [k, v] of Object.entries(field)) this.set(p, k, v); return p; }
    if (value !== null && typeof value === "object") { for (const [k, v] of Object.entries(value)) this.set(p, `${field}.${k}`, v); return p; }
    const leaf = this.leaf(p, field), access = accessOf(leaf.type);
    typed(leaf.type, access, value, field);
    this.write(p + leaf.offset, access, value);
    return p;
  }
  get(p, field) {
    const leaf = this.leaf(p, field), access = accessOf(leaf.type), at = p + leaf.offset;
    return access === "long" ? this.view.getBigUint64(at, true) : access === "byte" ? this.view.getUint8(at) : this.u32(at);
  }
  handle(name) {
    assert(pins.types[name] !== undefined, `${name} is not a pinned type`);
    return this.metadata + pins.runtime.typeDefinitions[0] + pins.types[name] * STRIDE;
  }
  type(t) {
    return this.once(`type ${key(t)}`, () => {
      const p = this.alloc(8);
      this.struct(p, "Il2CppType");
      if (typeof t === "string") { this.write(p, "ptr", this.handle(t)); this.write(p + 6, "byte", defKind(t)); return p; }
      if (isArray(t)) { this.write(p, "ptr", this.type(t[1])); this.write(p + 6, "byte", 29); return p; }
      const g = this.alloc(16), inst = this.alloc(8), argv = this.alloc(4 * (t.length - 1));
      this.struct(g, "Il2CppGenericClass");
      this.struct(inst, "Il2CppGenericInst");
      this.write(g, "ptr", this.type(t[0]));
      this.write(g + 4, "ptr", inst);
      this.write(inst, "int", t.length - 1);
      this.write(inst + 4, "ptr", argv);
      t.slice(1).forEach((a, i) => { this.declare(argv + 4 * i, `Il2CppGenericInst.type_argv[${i}]`, "ptr", ["Il2CppGenericInst.type_argv[]"]); this.write(argv + 4 * i, "ptr", this.type(a)); });
      this.write(p, "ptr", g);
      this.write(p + 6, "byte", 21);
      return p;
    });
  }
  cls(t) {
    return this.once(`class ${key(t)}`, () => {
      const c = this.alloc(VTABLE + 2 * VTABLE_ENTRY), byval = this.type(t);
      this.struct(c, "Il2CppClass");
      this.write(c + 16, "ptr", this.u32(byval));
      this.write(c + 22, "byte", this.view.getUint8(byval + 6));
      if (isArray(t)) { this.write(c + 32, "ptr", this.cls(t[1])); this.write(c + 140, "int", slotSize(t[1])); }
      this.write(c + 128, "int", sizeOf(t));
      this.write(c + 132, "int", slotSize(t));
      return c;
    });
  }
  method(t, name) {
    return this.once(`method ${key(t)}::${name}`, () => {
      const m = this.alloc(48);
      this.struct(m, "MethodInfo");
      this.write(m + 16, "ptr", this.cls(t));
      this.write(m + 28, "ptr", this.rgctx(t));
      return m;
    });
  }
  // A class RGCTX built from derive.py's slot descriptions. Field slots hold offsets: unboxed for value types.
  rgctx(t) {
    return this.once(`rgctx ${key(t)}`, () => {
      const slots = pins.rgctx[t[0]], names = {};
      assert(slots && PARAMS[t[0]], `no pinned RGCTX for ${t[0]}`);
      PARAMS[t[0]].forEach((n, i) => { names[n] = t[i + 1]; });
      const used = Object.keys(slots).filter(k => k[0] === "+").map(k => [Number(k.slice(1)), slots[k]]);
      const p = this.alloc(Math.max(56, ...used.map(([o]) => o + 4)));
      for (const [o, text] of used) {
        const label = `rgctx ${t[0]}+${o}`, cls = /^class (.+)$/.exec(text), method = /^method [^|]+\|(.+)::([^(]+)\([^)]*\)<(.+)>$/.exec(text), field = /^field (.+)\.([^.]+)$/.exec(text);
        if (cls) { this.declare(p + o, label, "ptr", [label]); this.write(p + o, "ptr", this.cls(bind(parse(cls[1]), names))); }
        else if (method) { this.declare(p + o, label, "ptr", [label]); this.write(p + o, "ptr", this.method(bind(parse(`${method[1]}<${method[3]}>`), names), method[2])); }
        else {
          assert(field, `unknown RGCTX slot ${text}`);
          const owner = bind(parse(field[1]), names), f = fieldsOf(owner).find(f => f.name === field[2]);
          assert(f, text);
          this.declare(p + o, label, "int", [label]);
          this.write(p + o, "int", isValue(owner) ? f.offset - 8 : f.offset);
        }
      }
      return p;
    });
  }
  statics(name) {
    return this.once(`statics ${name}`, () => {
      const leaves = leavesOf(name, true), s = this.alloc(Math.max(8, ...leaves.map(l => l.offset + WIDTH[accessOf(l.type)])));
      this.place(s, `${name}::`, leaves);
      this.write(this.cls(name) + 76, "ptr", s);
      return s;
    });
  }
  root(name, field) { const o = this.object(name); this.set(this.statics(name), field, o); return o; }
  object(t) {
    const size = sizeOf(t);
    assert(size > 0, `no instance size for ${key(t)}`);
    const p = this.alloc(size);
    this.struct(p, "Il2CppObject");
    this.write(p, "ptr", this.cls(t));
    this.place(p, `${key(t)}.`, leavesOf(t));
    return p;
  }
  array(e, length) {
    const t = ["[]", e], stride = slotSize(e), p = this.alloc(ARRAY_DATA + stride * length);
    const one = isValue(e) && !PRIM[e] ? leavesOf(e).map(l => ({...l, offset: l.offset - 8})) : [{path: "", offset: 0, type: PRIM[e] ?? "object", credit: []}];
    this.struct(p, "Il2CppArray");
    this.write(p, "ptr", this.cls(t));
    this.write(p + 12, "int", length);
    this.place(p, key(t), Array.from({length}, (_, i) => one.map(l => ({...l, path: `[${i}]${l.path ? "." : ""}${l.path}`, offset: ARRAY_DATA + i * stride + l.offset}))).flat());
    return p;
  }
  list(e, values = []) {
    const items = this.array(e, values.length);
    values.forEach((v, i) => this.set(items, `[${i}]`, v));
    return this.set(this.object(["System.Collections.Generic.List`1", e]), {_items: items, _size: values.length, _version: 1});
  }
  // String stores its characters inline from _firstChar, then a NUL.
  string(s) {
    const p = this.alloc(Math.max(sizeOf("System.String"), 14 + 2 * s.length)), base = leavesOf("System.String"), first = base.find(l => l.path === "_firstChar");
    this.struct(p, "Il2CppObject");
    this.write(p, "ptr", this.cls("System.String"));
    this.place(p, "System.String.", [...base, ...Array.from({length: s.length}, (_, i) => ({...first, path: `_firstChar[${i + 1}]`, offset: first.offset + 2 * (i + 1)}))]);
    this.set(p, "_stringLength", s.length);
    [...s, "\0"].forEach((ch, i) => this.write(p + first.offset + 2 * i, "short", ch.charCodeAt(0)));
    return p;
  }
  // IReadOnlyDictionary`2 declares ContainsKey then TryGetValue, so TryGetValue is one past its interface offset.
  virtual(t) {
    return this.once(`virtual ${key(t)}`, () => {
      const c = this.cls(t), row = this.alloc(12), slot = c + VTABLE + VTABLE_ENTRY;
      this.struct(row, "Il2CppRuntimeInterfaceOffsetPair");
      this.write(row, "ptr", this.cls(["System.Collections.Generic.IReadOnlyDictionary`2", t[1], t[2]]));
      this.write(c + 72, "ptr", row);
      this.write(c + 182, "short", 1);
      this.struct(slot, "VirtualInvokeData");
      this.write(slot + 4, "ptr", this.method(t, "TryGetValue"));
    });
  }
  // A Dictionary`2; deleted rows carry hashCode -1. An unused dictionary has no entries array.
  dict(k, v, rows = [], deleted = []) {
    const t = ["System.Collections.Generic.Dictionary`2", k, v], p = this.object(t);
    this.virtual(t);
    if (!rows.length) return p;
    const a = this.array(["System.Collections.Generic.Dictionary`2/Entry", k, v], rows.length);
    rows.forEach(([key, value], i) => this.set(a, `[${i}]`, {hashCode: deleted.includes(i) ? -1 : i, next: -1, key, value}));
    return this.set(p, {_entries: a, _count: rows.length, _freeCount: deleted.length, _version: 1});
  }
  // An OrderedMap`2: KeyValuePair slots in insertion order, tombstoned by gone[] and counted in gaps.
  orderedMap(k, v, rows = [], gone = []) {
    const t = ["Spinel.Net.OrderedMap`2", k, v], slots = this.array(["System.Collections.Generic.KeyValuePair`2", k, v], rows.length), marks = this.array("System.Boolean", rows.length);
    rows.forEach(([key, value], i) => this.set(slots, `[${i}]`, {key, value}));
    gone.forEach(i => this.set(marks, `[${i}]`, true));
    return this.set(this.object(t), {slots, gone: marks, used: rows.length, gaps: gone.length, version: 1});
  }
  // InputActionState's unmanaged TriggerState array.
  triggers(n) {
    const t = "UnityEngine.InputSystem.InputActionState/TriggerState", stride = slotSize(t), p = this.alloc(n * stride);
    this.place(p, t, Array.from({length: n}, (_, i) => leavesOf(t).map(l => ({...l, path: `[${i}].${l.path}`, offset: i * stride + l.offset - 8}))).flat());
    return p;
  }
  // A DataView whose reads must each hit a pinned field of the read's type.
  recorder() {
    const heap = this;
    class RecordingView extends DataView {}
    for (const g of ["getInt8", "getUint8", "getInt16", "getUint16", "getInt32", "getUint32", "getFloat32", "getFloat64", "getBigInt64", "getBigUint64"]) {
      RecordingView.prototype[g] = function (p, little) {
        const f = heap.fields.get(p);
        if (!f) heap.violations.push(`${g}(${p}) reads no pinned field`);
        else if (!GETTERS[f.access].includes(g)) heap.violations.push(`${g}(${p}) reads ${f.label}, a ${f.access} field`);
        else f.credit.forEach(c => COVERED.add(c));
        return DataView.prototype[g].call(this, p, little);
      };
    }
    return RecordingView;
  }
  audit() { assert.deepEqual(this.violations, [], "sampler reads must hit pinned fields"); }
}

const SCREEN = "Spinel.UI.Pc.Screens.GameplayScreen", ACTOR = "Spinel.Game.World.WorldActor", VIEW = "Spinel.Engine.Unity.World.ActorView", POINT = "Spinel.Game.World.Point";
const ITEM = "Spinel.Net.Game.InvItem", INFO = "Spinel.Game.Play.ItemInfo", PARAM = "Spinel.Game.Play.DrillParam", BINDING = "Spinel.Game.Play.KeyBinding";
const KEY = "UnityEngine.InputSystem.Controls.KeyControl", BUTTON = "UnityEngine.InputSystem.Controls.ButtonControl", GROUND = "Spinel.UI.Pc.Screens.GroundItems/Entry";
const AGING = "Spinel.Game.Play.CombatModel/Aging", EDIT = "Spinel.UI.Pc.EditBox", FUNC = ["System.Func`1", "System.Boolean"];
const I32 = "System.Int32", I64 = "System.Int64", U64 = "System.UInt64", PAIRS = ["[]", ["System.Collections.Generic.KeyValuePair`2", I32, VIEW]];
const HEAVY = pins.slots["Spinel.UI.Pc.dll|Spinel.UI.Pc.Play.CarriedWeightUi/<>c__DisplayClass2_0::<Install>b__2()"];
const CELL = name => pins.cells[name] ?? assert.fail(name);
const BASE_PARAM = {Id: 8001n, DigDepth: 30, BarEffArea: 70, BarSpeed: 100, ScoopMass: 1};
const RECT = {left: 27.5, top: 41.25, width: 960, height: 540};

function fixture(primePointer = true) {
  const h = new Heap(), p = {};
  const actor = (id, type, x, y) => h.set(h.object(ACTOR), {Id: id, Type: type, X: x, Y: y, HitLeft: -10, HitTop: -10, HitWidth: 20, HitHeight: 20});
  p.flow = h.root("Spinel.UI.Pc.Screens.GameFlow", "Current");
  p.engine = h.root("Spinel.Engine.Unity.SpinelEngine", "Instance");
  p.screen = h.object(SCREEN);
  p.stage = h.object("Spinel.Engine.Unity.Stage");
  p.input = h.object("Spinel.Engine.Unity.GameInput");
  h.set(p.flow, {Engine: p.engine, Screen: p.screen});
  h.set(p.engine, {Stage: p.stage, Input: p.input});
  h.set(p.stage, {initialized: true, hostRect: {m_XMin: 8, m_YMin: 10, m_Width: 1870, m_Height: 1310},
    Current: {UiScale: 1, WorldZoom: 1.25, PxPerDp: 1, ViewportWidth: 1920, ViewportHeight: 1350, WorldWidth: 1536, WorldHeight: 1080}});
  p.own = h.set(actor(41, 1, 350, 110), "path", h.list(POINT));
  p.session = h.set(h.object("Spinel.Net.Game.GameSession"), "sendArmed", true);
  p.zoneInfo = h.set(h.object("Spinel.Game.World.ZoneInfo"), "Id", 17);
  p.map = h.set(h.object("Spinel.Engine.Unity.World.MapView"), {Width: 2000, Height: 1500});
  p.zone = h.set(h.object("Spinel.UI.Pc.Screens.GameplayScreen/LoadedZone"), {Info: p.zoneInfo, Map: p.map, Warps: h.list("Spinel.Game.World.WarpRegion")});
  h.set(p.screen, {avatar: p.own, play: p.session, zone: p.zone, ready: true, cameraX: 320, cameraY: 96});
  p.combat = h.object("Spinel.Game.Play.CombatModel");
  p.drill = h.object("Spinel.Game.Play.DrillFeature");
  p.assist = h.object("Spinel.Game.Play.DigAssist");
  p.gun = h.object("Spinel.Game.Play.GunFlowing");
  p.attributes = h.object("Spinel.Game.Play.AvatarStatusFlags");
  p.skills = h.object("Spinel.Game.Play.SkillsApi");
  // PlayServices keys services by RuntimeType; libil2cpp builds Type objects over a class's byval_arg.
  p.serviceKeys = [p.combat, p.drill, p.assist, p.gun, p.attributes, p.skills].map(service => h.set(h.object("System.RuntimeType"), "_impl.value", h.u32(service) + 16));
  p.serviceDict = h.dict("System.Type", "System.Object", p.serviceKeys.map((k, i) => [k, [p.combat, p.drill, p.assist, p.gun, p.attributes, p.skills][i]]));
  h.set(p.screen, "services", h.set(h.object("Spinel.UI.Pc.Screens.PlayServices"), "items", p.serviceDict));
  p.fsm = h.object("Spinel.Game.Play.AvatarFsm");
  h.set(p.drill, {fsm: p.fsm, session: p.session});
  h.set(p.combat, {Fsm: p.fsm, session: p.session});
  h.set(p.gun, "session", p.session);
  h.set(p.assist, "drill", p.drill);
  p.hud = h.object("Spinel.UI.Pc.Hud.Hud");
  h.set(p.screen, "hud", p.hud);
  const binding = code => h.set(h.object(BINDING), {Code: h.string(code), Exclusive: true});
  p.digBinding = binding("KeyF");
  p.runBinding = binding("KeyV");
  h.set(p.hud, "Keys", h.set(h.object("Spinel.Game.Play.HotkeyMap"), "Bindings", h.dict("System.String", ["System.Collections.Generic.List`1", BINDING],
    [[h.string("Dig"), h.list(BINDING, [p.digBinding])], [h.string("Run"), h.list(BINDING, [p.runBinding])]])));
  p.keyboard = h.root("UnityEngine.InputSystem.Keyboard", "current");
  p.keys = h.array(KEY, 128);
  for (let i = 0; i < 128; i++) h.set(p.keys, `[${i}]`, h.set(h.object(KEY), "needsToCheckFramePress", true));
  h.set(p.keyboard, "m_Keys", p.keys);
  p.modifiers = ["shiftKey", "ctrlKey", "altKey"].map(name => { const q = h.set(h.object(BUTTON), "needsToCheckFramePress", true); h.set(p.keyboard, name, q); return q; });
  p.actions = ["Click", "RightClick"].map(name => {
    const action = h.object("UnityEngine.InputSystem.InputAction"), map = h.object("UnityEngine.InputSystem.InputActionMap"), state = h.object("UnityEngine.InputSystem.InputActionState"), triggers = h.triggers(1);
    h.set(p.input, name, action);
    h.set(action, "m_ActionMap", map);
    h.set(map, "m_State", state);
    h.set(state, "memory", {actionCount: 1, actionStates: triggers});
    return h.set(triggers, "[0].m_Phase", 1);
  });
  p.run = h.set(h.object("Spinel.Game.Play.RunToggle"), "Ready", true);
  h.set(p.screen, "run", p.run);
  p.slots = h.set(h.array(U64, 23), "[7]", 501n);
  p.tool = h.set(h.object(ITEM), {Uid: 501n, ItemId: 8001n, Count: 300});
  p.inv = h.set(h.object("Spinel.Net.Game.Inventory"), {slots: p.slots, byUid: h.orderedMap(U64, ITEM, [[501n, p.tool]])});
  h.set(p.gun, "Inventory", p.inv);
  h.set(p.drill, "Inventory", p.inv);
  p.toolInfo = h.set(h.object(INFO), {Id: 8001n, Type: 6});
  h.set(p.gun, "infos", h.dict(I64, INFO, [[8001n, p.toolInfo]]));
  p.drillParam = h.set(h.object(PARAM), BASE_PARAM);
  p.drillData = h.set(h.object("Spinel.Game.Play.DrillData"), {Params: h.dict(I64, PARAM, [[8001n, p.drillParam]]), AutoPets: h.dict(I64, I32)});
  h.set(p.drill, "Data", p.drillData);
  p.skins = h.set(h.statics("Spinel.UI.Pc.UiSkins"), {chosen: 1, started: true});
  p.semi = h.set(h.statics("Spinel.UI.Pc.Play.DigKeySettings"), "semiAuto", {hasValue: true, value: false});
  p.monster = actor(73, 3, 500, 300);
  p.monsterView = h.set(h.object(VIEW), "Actor", p.monster);
  p.actors = h.orderedMap(I32, VIEW, [[73, p.monsterView]]);
  h.set(p.screen, {actors: p.actors, pointer: {hasValue: true, value: {x: 180, y: 204}}, hoveredId: {hasValue: true, value: 73}, probedAt: {x: 233, y: 1065}, probedTime: 100});
  p.hover = h.object("Spinel.UI.Pc.Screens.GroundItems/HoverDraw");
  p.ground = h.set(h.object("Spinel.UI.Pc.Screens.GroundItems"), {hover: p.hover, entries: h.dict(I32, GROUND)});
  h.set(p.screen, "ground", p.ground);
  p.edit = h.statics(EDIT);
  h.set(h.statics("Spinel.UI.Pc.Hud.HudCover"), "covers", h.dict("System.Object", FUNC));
  p.eventSystem = h.set(h.object("UnityEngine.EventSystems.EventSystem"), "m_HasFocus", true);
  h.set(h.statics("UnityEngine.EventSystems.EventSystem"), "m_EventSystems", h.list("UnityEngine.EventSystems.EventSystem", [p.eventSystem]));
  p.origins = h.root("Spinel.UI.Pc.UiPressOrigin", "Current");
  h.set(p.combat, {attackTargets: h.dict(I32, I32), waitingMoves: h.dict(I32, "Spinel.Game.Play.CombatModel/WaitingMove"),
    striking: h.dict(I32, ["System.Collections.Generic.Queue`1", "System.Action"]), fading: h.orderedMap(I32, AGING)});
  const canvas = {isConnected: true, width: 1920, height: 1350, getBoundingClientRect: () => ({...RECT})};
  const module = {HEAPU32: new Uint32Array(h.buffer), canvas};
  const messages = [];
  const context = vm.createContext({ArrayBuffer, DataView: h.recorder(), Uint32Array, spinelInstance: {Module: module}, devicePixelRatio: 2,
    document: {hasFocus: () => true, visibilityState: "visible", activeElement: canvas},
    getComputedStyle: () => ({transform: "none", visibility: "visible", display: "block", opacity: "1", pointerEvents: "auto"}),
    collect: (kind, fields, ingress) => messages.push({kind, epoch: "7", rawIngress: ingress, ...fields})});
  const control = vm.runInContext(`(function(){const cfg={build:${JSON.stringify(pins.build)},epoch:"7",guard:"synthetic-guard"};let closing=false,failed=false,ingress=19;const now=()=>100;function post(k,v){collect(k,v,ingress);}${script}\nreturn {sample:()=>globalThis.__opentricksterNetSampleV1("7","synthetic-guard","a","b"),close:()=>{closing=true},fail:()=>{failed=true},call:(...args)=>globalThis.__opentricksterNetSampleV1(...args),cfg,setIngress:n=>{ingress=n}};})()`, context);
  function sample() { control.sample(); h.audit(); assert(messages.length); return messages.at(-1); }
  if (primePointer) { sample(); h.set(p.screen, "probedTime", 100.125); sample(); messages.length = 0; }
  return {h, p, actor, module, canvas, context, control, messages, sample};
}

// Constants the sampler declares that reads alone cannot prove, such as extents smaller than an object.
function checkConstants() {
  const constant = name => {
    const m = new RegExp(`\\b${name} = (\\{[^}]*\\}|\\[[^\\]]*\\]|"[^"]*")`).exec(script);
    assert(m, `sampler.js declares ${name}`);
    return JSON.parse(JSON.stringify(vm.runInNewContext(`(${m[1]})`)));
  };
  const names = Object.fromEntries(Object.entries(pins.types).map(([name, id]) => [id, name]));
  for (const [id, size] of Object.entries(constant("typeSizes"))) assert.equal(size, pins.instanceSizes[names[id]], `typeSizes ${id} is ${names[id]}`);
  for (const [id, kind] of Object.entries(constant("typeKinds"))) assert.equal(kind, pins.typeKinds[names[id]], `typeKinds ${id} is ${names[id]}`);
  assert.deepEqual(constant("metadataCells"), [pins.runtime.s_GlobalMetadata, pins.runtime.s_GlobalMetadataHeader]);
  assert.deepEqual(constant("typeDefinitions"), pins.runtime.typeDefinitions);
  assert.equal(constant("samplerBuild"), pins.build);
}

if (process.argv.some(a => ["--snapshot", "--stale-zoom", "--fresh-zoom"].includes(a))) {
  const f = fixture();
  if (!process.argv.includes("--snapshot")) {
    f.h.set(f.p.stage, "Current.WorldZoom", 2);
    f.sample();
    if (process.argv.includes("--fresh-zoom")) f.h.set(f.p.screen, {probedAt: {x: 368, y: 912}, probedTime: 100.25});
  }
  const value = f.sample();
  assert.equal(value.snapshot.status, "ready");
  process.stdout.write(JSON.stringify(value));
} else {
  checkConstants();
  let cases = 0;
  function check(name, change, expected = "unavailable", reason) {
    const f = fixture();
    change(f);
    const s = f.sample().snapshot;
    assert.equal(s.status, expected, name);
    if (reason) assert.equal(s.reason, reason, name);
    cases++;
    return s;
  }
  const base = fixture(), hash = () => crypto.createHash("sha256").update(new Uint8Array(base.h.buffer)).digest("hex"), before = hash();
  const sample = base.sample();
  assert.equal(sample.snapshot.status, "ready");
  assert.equal(hash(), before, "sampler never writes memory");
  assert.equal(sample.rawIngress, 19);
  assert.equal(sample.snapshot.pointer.picked, 73);
  assert.equal(sample.snapshot.pointer.groundHover, false);
  assert.equal(sample.snapshot.input.mode, 1);
  assert.equal(sample.snapshot.dig.manual, true);
  assert.equal(sample.snapshot.dig.toolUsable, true);
  assert.deepEqual(Array.from(sample.snapshot.geometry.camera), [320, 96]);
  assert.equal(sample.snapshot.binding.dig, "KeyF");
  assert.equal(base.sample().snapshot.revision, 1);
  base.h.set(base.p.screen, "cameraX", 321);
  assert.equal(base.sample().snapshot.revision, 2);
  cases++;

  // Fixed reads. Each case fails against the read it replaced.
  // Il2CppType.data is a metadata handle (libil2cpp f2486), never a typedef index.
  check("typedef index is not a type handle", ({h}) => h.raw(h.cls(SCREEN) + 16, "ptr", pins.types[SCREEN]), "unavailable", "type");
  check("type handle off the typedef record stride", ({h}) => h.raw(h.cls(SCREEN) + 16, "ptr", h.handle(SCREEN) + 1), "unavailable", "type");
  check("metadata header from another build", ({h}) => h.raw(h.metadata + HEADER + 8, "int", pins.runtime.typeDefinitions[2] + 1), "unavailable", "build");
  // RuntimeType's handle is Type._impl.value at +8. A matching handle one field later must not decide identity.
  check("service key identity comes from Type._impl", ({h, p}) => {
    p.serviceKeys.forEach(k => h.raw(k + 12, "ptr", h.u32(k + 8)));
    h.set(p.serviceKeys[0], "_impl.value", h.cls(ACTOR) + 16);
  }, "unavailable", "generic");
  // OrderedMap`2 rows are slots[0, used) minus gone[] tombstones.
  const tombstoned = check("tombstoned actor row skipped", ({h, p}) => {
    h.set(p.screen, "actors", h.orderedMap(I32, VIEW, [[73, p.monsterView]], [0]));
    h.set(p.screen, "hoveredId.hasValue", false);
  }, "ready");
  assert.equal(tombstoned.actors.length, 1);
  check("tombstones must match gaps", ({h, p}) => h.set(h.get(p.actors, "gone"), "[0]", true), "unavailable", "generic");
  check("gaps need a tombstone array", ({h, p}) => h.set(p.actors, {gone: 0, gaps: 1}), "unavailable", "generic");
  const unused = check("slots past used are not rows", ({h, p, actor}) => {
    const stale = h.set(h.object(VIEW), "Actor", actor(74, 3, 600, 300));
    h.set(p.screen, "actors", h.set(h.orderedMap(I32, VIEW, [[73, p.monsterView], [74, stale]]), "used", 1));
  }, "ready");
  assert.deepEqual(Array.from(unused.actors, a => a.raw), [41, 73]);
  check("tombstoned inventory row", ({h, p}) => h.set(p.inv, "byUid", h.orderedMap(U64, ITEM, [[501n, p.tool]], [0])), "unavailable", "cold");
  // GameplayScreen.ground is +256. GroundItems.entries maps an item actor id to its view.
  const groundItem = (f, key, x, view = true) => {
    const {h, p, actor} = f, item = actor(key, 9, x, 300), entry = h.set(h.object(GROUND), "View", view ? h.set(h.object(VIEW), "Actor", item) : 0);
    h.set(p.ground, "entries", h.dict(I32, GROUND, [[key, entry]]));
    return entry;
  };
  const underPointer = check("ground item under the pointer", f => groundItem(f, 80, 500), "ready");
  assert.equal(underPointer.pointer.groundHover, true);
  assert.equal(underPointer.pointer.picked, 73);
  assert.equal(check("ground item away from the pointer", f => groundItem(f, 80, 100), "ready").pointer.groundHover, false);
  assert.equal(check("ground entry without a view", f => groundItem(f, 80, 500, false), "ready").pointer.groundHover, false);
  check("ground entry key must match its actor id", f => {
    const entry = groundItem(f, 80, 500);
    f.h.set(f.p.ground, "entries", f.h.dict(I32, GROUND, [[81, entry]]));
  }, "unavailable", "generic");
  assert.equal(check("HoverDraw.Entry still reports a hover", f => f.h.set(f.p.hover, "Entry", groundItem(f, 80, 100)), "ready").pointer.groundHover, true);
  // EditBox.Focused (static +24) takes keys ahead of the HUD.
  check("focused EditBox owns the keys", ({h, p}) => h.set(p.edit, "Focused", h.object(EDIT)), "unavailable", "routing");
  check("EditBox static ahead of Focused is not focus", ({h, p}) => h.raw(p.edit, "ptr", h.string("selected")), "ready");
  // Hud.installing (+132) diverts only a key or menu whose action is not registered yet (WaitForKey f48518).
  // That press waits in waitMenu +136 or waitAction +140, then in due +148 until Hud.Update f48500 replays it.
  check("installing HUD alone keeps registered keys", ({h, p}) => h.raw(p.hud + 132, "byte", 1), "ready");
  check("deferred key waits for its action", ({h, p}) => h.set(p.hud, "waitAction", h.string("Synthetic")), "unavailable", "routing");
  check("deferred menu key waits for its button", ({h, p}) => h.set(p.hud, "waitMenu", h.string("Synthetic")), "unavailable", "routing");
  check("deferred key queued for replay", ({h, p}) => h.set(p.hud, "due", h.object("System.Action")), "unavailable", "routing");
  check("Hud.Waiting callback is not routing state", ({h, p}) => h.raw(p.hud + 128, "ptr", h.object("System.Action")), "ready");
  // PlayFeatures.Deferred installs DrillUi after world entry. Its MoveNext f44759 provides DrillFeature, awaits
  // ItemsModel, then provides DigAssist and registers the Dig hotkey and drill click with no await between them.
  const withServices = ({h, p}, keep) => {
    const rows = [p.combat, p.drill, p.assist, p.gun, p.attributes, p.skills].map((service, i) => [p.serviceKeys[i], service]);
    h.set(h.get(p.screen, "services"), "items", h.dict("System.Type", "System.Object", rows.filter(([, service]) => keep(service))));
  };
  const noDrill = f => withServices(f, s => s !== f.p.drill && s !== f.p.assist);
  const absent = check("deferred drill install has not run", noDrill, "ready");
  assert.equal(absent.dig, null);
  assert.equal(absent.motion.fsm, 0);
  const shared = check("absent drill still reports the shared avatar state", f => { noDrill(f); f.h.set(f.p.fsm, "State", 5); }, "ready");
  assert.equal(shared.motion.fsm, 5);
  assert.equal(shared.dig, null);
  check("drill provided before its helper", f => withServices(f, s => s !== f.p.assist), "unavailable", "cold");
  check("helper without its drill", f => withServices(f, s => s !== f.p.drill), "unavailable", "changing");
  check("drill and combat disagree on the avatar state", ({h, p}) => h.set(p.drill, "fsm", h.object("Spinel.Game.Play.AvatarFsm")), "unavailable", "changing");
  for (const core of ["combat", "gun", "attributes", "skills"]) check(`core ${core} service not provided yet`, f => withServices(f, s => s !== f.p[core]), "unavailable", "cold");
  const late = fixture();
  noDrill(late);
  assert.equal(late.sample().snapshot.dig, null);
  withServices(late, s => s !== late.p.assist);
  assert.equal(late.sample().snapshot.reason, "cold");
  withServices(late, () => true);
  const arrived = late.sample().snapshot;
  assert.equal(arrived.status, "ready");
  assert.equal(arrived.dig.manual, true);
  assert.equal(arrived.dig.toolUsable, true);
  cases++;

  check("tagged flow", ({h}) => h.raw(CELL("TypeInfo Spinel.UI.Pc.Screens.GameFlow"), "ptr", 0x200086e7));
  check("null engine", ({h}) => h.raw(CELL("TypeInfo Spinel.Engine.Unity.SpinelEngine"), "ptr", 0));
  check("misaligned object", ({h, p}) => h.raw(h.addr(p.flow, "Screen"), "ptr", p.screen + 1));
  check("truncated object", ({h, p}) => h.raw(h.addr(p.flow, "Screen"), "ptr", h.buffer.byteLength - 8));
  // A Dictionary`2 is 48 bytes; one cut short by the end of memory must be refused before any read.
  check("truncated dictionary", ({h, p}) => {
    const end = h.buffer.byteLength - 44;
    new Uint8Array(h.buffer, end, 44).set(new Uint8Array(h.buffer, h.get(p.combat, "attackTargets"), 44));
    h.set(p.combat, "attackTargets", end);
  }, "unavailable", "memory");
  check("type-confused screen", ({h, p}) => h.raw(p.screen, "ptr", h.cls(ACTOR)));
  check("wrong class extent", ({h}) => h.raw(h.cls(SCREEN) + 128, "int", 16));
  check("class cannot use an array type kind", ({h}) => h.raw(h.cls(SCREEN) + 22, "byte", 29));
  check("map slots require the pair element class", ({h}) => h.raw(h.cls(PAIRS) + 32, "ptr", h.cls("System.String")));
  check("dictionary entries require the entry element class", ({h}) => h.raw(h.cls(["[]", ["System.Collections.Generic.Dictionary`2/Entry", "System.Type", "System.Object"]]) + 32, "ptr", h.cls("System.String")));
  check("dictionary entries require matching generic arguments", ({h}) => {
    const entries = h.cls(["[]", ["System.Collections.Generic.Dictionary`2/Entry", "System.Type", "System.Object"]]), other = ["System.Collections.Generic.Dictionary`2/Entry", I32, INFO];
    h.raw(entries + 16, "ptr", h.type(other));
    h.raw(entries + 32, "ptr", h.cls(other));
  }, "unavailable", "generic");
  check("reference width must equal the WASM pointer width", ({h}) => h.raw(h.cls(VIEW) + 132, "int", 1));
  check("array descriptor must be an actual vector", ({h}) => h.raw(h.cls(PAIRS) + 22, "byte", 18));
  check("cold scene", ({h, p}) => h.set(p.screen, "ready", false));
  for (const field of ["changingZone", "loggingOut", "localScene", "campScene", "worldCovered", "artAwaited"]) check(`covered/changing scene ${field}`, ({h, p}) => h.set(p.screen, field, true));
  check("pan", ({h, p}) => h.set(p.stage, "panPixels", 1));
  check("host override with zero first component", ({h, p}) => h.set(p.stage, "hostOverride.hasValue", true));
  check("nonfinite zoom", ({h, p}) => h.set(p.stage, "Current.WorldZoom", NaN));
  const services = "MethodRef mscorlib.dll|System.Collections.Generic.Dictionary`2::TryGetValue(!TKey,!TValue&)<System.Type,object>";
  check("unresolved method", ({h}) => h.raw(CELL(services), "ptr", 0xa0003fdb));
  check("unresolved context", ({h}) => h.raw(h.u32(CELL(services)) + 28, "ptr", 0));
  check("wrong specialization", ({h, p}) => h.raw(p.actors, "ptr", h.u32(p.serviceDict)));
  check("invalid slot stride", ({h}) => h.raw(h.cls(PAIRS) + 140, "int", 4));
  check("used beyond slots", ({h, p}) => h.set(p.actors, "used", 2));
  check("negative used", ({h, p}) => h.set(p.actors, "used", -1));
  check("oversized map", ({h, p}) => h.set(p.actors, "used", 100000));
  check("dictionary deleted count mismatch", ({h, p}) => h.set(h.get(p.serviceDict, "_entries"), "[0].hashCode", -1));
  check("service identity mismatch", ({h, p}) => h.set(p.serviceKeys[0], "_impl.value", h.cls(ACTOR) + 16));
  check("root mutation", f => { f.canvas.getBoundingClientRect = () => { f.h.set(f.p.flow, "Screen", f.p.own); return {...RECT}; }; });
  check("map version changed", f => { f.canvas.getBoundingClientRect = () => { f.h.set(f.p.actors, "version", 2); return {...RECT}; }; });
  check("memory growth while reading", f => { let calls = 0; Object.defineProperty(f.module, "HEAPU32", {get: () => ++calls === 1 ? new Uint32Array(f.h.buffer) : new Uint32Array(new ArrayBuffer(16 << 20))}); });
  const grown = fixture();
  grown.sample();
  const bigger = new ArrayBuffer(20 << 20);
  new Uint8Array(bigger).set(new Uint8Array(grown.h.buffer));
  grown.module.HEAPU32 = new Uint32Array(bigger);
  assert.equal(grown.sample().snapshot.status, "ready");
  cases++;
  check("malformed boolean", ({h, p}) => h.raw(h.addr(p.screen, "ready"), "byte", 2));
  check("modal UI", ({h, p}) => h.set(p.eventSystem, "m_CurrentSelected", p.screen));
  // Hud..ctor f48528 sets keyMask (+68) to 0x7FFFF and set_UiMode f48477 rewrites it from GameUiMode.Masks f21418.
  // KeyPasses f48492 tests it through KeyOn f21422, which passes any name KeyBit f21420 gives no bit, as Dig and Run.
  for (const mask of [0x7ffff, 12289, 0]) check(`HUD key mask ${mask} leaves Dig and Run routed`, ({h, p}) => h.raw(p.hud + 68, "int", mask), "ready");
  check("unknown binding", ({h, p}) => h.set(p.digBinding, "Code", h.string("Key1")));
  check("modifier binding", ({h, p}) => h.set(p.digBinding, "Modifier", 1));
  check("conflicting run/dig", ({h, p}) => h.set(p.runBinding, "Code", h.string("KeyF")));
  check("missing nullable metadata", ({h}) => h.raw(CELL("MethodRef mscorlib.dll|System.Nullable`1::GetValueOrDefault()<bool>"), "ptr", 0));
  check("unknown skin startup", ({h, p}) => h.set(p.skins, "started", false));
  check("unknown heavy callback", ({h, p}) => h.set(p.run, {On: true, Heavy: h.object(FUNC)}));
  const running = check("observed running, absent Heavy delegate", ({h, p}) => h.set(p.run, "On", true), "ready");
  assert.equal(running.input.mode, 2);
  const heavy = (f, slot, carried = 0, attrs = 0) => {
    const {h, p} = f, state = h.object("Spinel.UI.Pc.Play.CarriedWeightUi/<>c__DisplayClass2_0");
    h.set(p.run, {On: true, Heavy: h.set(h.object(FUNC), {method_ptr: slot, m_target: state})});
    h.set(state, {carried: BigInt(carried), hud: p.hud, ctx: p.screen, run: p.run});
    h.set(p.hud, "Stats", h.set(h.object("Spinel.Game.Play.PlayerStats"), "current", h.set(h.object("Spinel.Game.Play.StatValues"), "MaxWeight", 100)));
    h.set(p.attributes, "Value", attrs);
  };
  for (const [carried, attrs, expected] of [[90, 17, 1], [89, 17, 2], [90, 0, 2], [2147483647, 17, 2]]) {
    assert.equal(check("source heavy predicate", f => heavy(f, HEAVY, carried, attrs), "ready").input.mode, expected);
  }
  assert.equal(check("previous build heavy callback slot", f => heavy(f, 49652)).reason, "mode");
  const cachedButton = (f, pressPoint, cached) => {
    const {h, p} = f, manager = h.set(h.object("UnityEngine.InputSystem.InputManager"), "m_ReadValueCachingFeatureEnabled", true);
    h.set(p.modifiers[0], {needsToCheckFramePress: false, pressPoint, m_CachedValue: cached});
    h.set(h.statics("UnityEngine.InputSystem.InputSystem"), "s_Manager", manager);
    h.set(h.statics(BUTTON), "s_GlobalDefaultButtonPressPoint", 0.5);
  };
  assert.equal(check("source cached modifier value", f => cachedButton(f, 0.5, 0), "ready").input.modifiers[0], false);
  assert.equal(check("default press point", f => cachedButton(f, 0, 1), "ready").input.modifiers[0], true);
  check("stale cached modifier value", f => { cachedButton(f, 0.5, 1); f.h.set(f.p.modifiers[0], "m_CachedValueIsStale", true); }, "unavailable", "input");
  check("processors evaluated on every read", f => { cachedButton(f, 0.5, 1); f.h.set(f.p.modifiers[0], "evaluateProcessorsEveryRead", true); }, "unavailable", "input");
  // Keyboard.current holds whatever MakeCurrent f34170 stored; the client's own keyboard is the precompiled FastKeyboard.
  const fastKeyboard = ({h, p}) => {
    const k = h.set(h.object("UnityEngine.InputSystem.FastKeyboard"), {m_Keys: p.keys, shiftKey: p.modifiers[0], ctrlKey: p.modifiers[1], altKey: p.modifiers[2]});
    h.set(h.statics("UnityEngine.InputSystem.Keyboard"), "current", k);
    return k;
  };
  check("FastKeyboard is the current keyboard", fastKeyboard, "ready");
  check("current keyboard of another class", ({h}) => h.set(h.statics("UnityEngine.InputSystem.Keyboard"), "current", h.object(BUTTON)), "unavailable", "type");
  const gun = check("actual equipped gun branch", ({h, p}) => {
    const item = h.set(h.object(ITEM), {Uid: 502n, ItemId: 9000n, Count: 1}), info = h.set(h.object(INFO), {Id: 9000n, Type: 2, SubType: 54});
    h.set(p.slots, "[1]", 502n);
    h.set(p.inv, "byUid", h.orderedMap(U64, ITEM, [[501n, p.tool], [502n, item]]));
    h.set(p.gun, {infos: h.dict(I64, INFO, [[8001n, p.toolInfo], [9000n, info]]), Armed: true, Ready: true, State: 3});
  }, "ready");
  assert.equal(gun.combat.gunWorn, true);
  assert.equal(gun.combat.gunReady, true);
  check("foreign nullable specialization", ({h}) => h.raw(h.u32(CELL("MethodRef mscorlib.dll|System.Nullable`1::GetValueOrDefault()<vt:UnityEngine.Vector2>")) + 16, "ptr", h.cls(["System.Nullable`1", I32])));
  check("nullable field overlap", ({h}) => h.raw(h.rgctx(["System.Nullable`1", "UnityEngine.Vector2"]) + 12, "int", 5));
  check("unknown manual setting", ({h, p}) => h.set(p.semi, "semiAuto.hasValue", false));
  check("cold drill parameter", ({h, p}) => { h.set(p.fsm, "State", 5); h.set(p.drill, "inDrillState", true); });
  check("native page zoom", f => { f.context.visualViewport = {scale: 2, offsetLeft: 0, offsetTop: 0}; });
  check("non-canvas DOM editor", f => { f.context.document.activeElement = {tagName: "INPUT"}; });
  check("canvas changed during capture", f => { let n = 0; f.canvas.getBoundingClientRect = () => ({...RECT, width: ++n === 1 ? 960 : 900}); });
  const deleted = check("deleted dictionary row skipped", ({h, p}) => h.set(p.combat, "attackTargets", h.dict(I32, I32, [[41, 73]], [0])), "ready");
  assert.equal(deleted.combat.ownAttack, false);
  assert.equal(check("own attack row", ({h, p}) => h.set(p.combat, "attackTargets", h.dict(I32, I32, [[41, 73]])), "ready").combat.ownAttack, true);
  const occupied = check("non-own combat work is distinct", ({h, p}) => h.set(p.combat, "attackTargets", h.dict(I32, I32, [[99, 73]])), "ready");
  assert.equal(occupied.combat.ownAttack, false);
  const fade = check("actual fading target", ({h, p}) => h.set(p.combat, "fading", h.orderedMap(I32, AGING, [[73, h.object(AGING)]])), "ready");
  assert.equal(fade.actors[1].fading, true);
  check("duplicate fading key", ({h, p}) => h.set(p.combat, "fading", h.orderedMap(I32, AGING, [[73, h.object(AGING)], [73, h.object(AGING)]])), "unavailable", "generic");
  check("selected skill consumes world input", ({h, p}) => h.set(p.skills, "target", p.monster));
  check("pending held cast", ({h, p}) => h.set(p.skills, "heldCast", p.monster));
  // SkillsApi..ctor f17848 is the only writer of combatPlaysEsa (+124): CombatModel was present at install.
  check("combat ESA playback is a setting, not pending input", ({h, p}) => h.raw(p.skills + 124, "byte", 1), "ready");
  const noDig = check("source map dig refusal", ({h, p}) => h.set(p.zoneInfo, "Type", 5), "ready");
  assert.equal(noDig.dig.mapAllowed, false);
  for (const option of [0, 1]) {
    const pet = check("source automatic pet and zone gate", ({h, p}) => {
      const item = h.set(h.object(ITEM), {Uid: 503n, ItemId: 7000n, Count: 1}), info = h.set(h.object(INFO), {Id: 7000n, Type: 19});
      h.set(p.slots, "[8]", 503n);
      h.set(p.zoneInfo, "Option", option);
      h.set(p.inv, "byUid", h.orderedMap(U64, ITEM, [[501n, p.tool], [503n, item]]));
      h.set(p.gun, "infos", h.dict(I64, INFO, [[8001n, p.toolInfo], [7000n, info]]));
      h.set(p.drillData, "AutoPets", h.dict(I64, I32, [[7000n, 1]]));
    }, "ready");
    assert.equal(pet.dig.automaticPet, option === 0);
    assert.equal(pet.dig.manual, option === 1);
  }
  check("wrong visual hover", ({h, p}) => h.set(p.screen, "hoveredId.value", 41), "ready");
  const noPick = check("hidden target", ({h, p}) => h.set(p.monster, "Unseen", true), "ready");
  assert.equal(noPick.pointer.picked, null);
  const shift = check("observed shift retained", ({h, p}) => h.set(p.modifiers[0], "m_LastUpdateWasPress", true), "ready");
  assert.equal(shift.input.modifiers[0], true);
  const pressed = check("pressed action", ({h, p}) => h.set(p.actions[0], "[0]", {m_Phase: 3, m_Flags: 64}), "ready");
  assert.equal(pressed.input.left, true);
  check("pressed flag on a waiting action", ({h, p}) => h.set(p.actions[0], "[0].m_Flags", 64), "unavailable", "input");
  const semi = check("semi-auto retained", ({h, p}) => h.set(p.semi, "semiAuto.value", true), "ready");
  assert.equal(semi.dig.manual, false);
  const assist = check("helper state 2 is busy", ({h, p}) => h.set(p.assist, "State", 2), "ready");
  assert.equal(assist.dig.assist, 2);
  const automatic = check("automatic drill phase", ({h, p}) => h.set(p.drill, "Auto", h.set(h.object("Spinel.Game.Play.AutoDrill"), "State", 2)), "ready");
  assert.equal(automatic.dig.automatic, true);
  const portal = check("zone portal", ({h, p}) => h.set(p.zone, "Warps", h.list("Spinel.Game.World.WarpRegion", [h.set(h.object("Spinel.Game.World.WarpRegion"), {X1: 10, Y1: 20, X2: 30, Y2: 40})])), "ready");
  assert.deepEqual(Array.from(portal.portals, q => Array.from(q)), [[10, 20, 30, 40]]);
  const route = check("queued route", ({h, p}) => h.set(p.screen, "route", h.list(POINT, [{}, {}])), "ready");
  assert.equal(route.motion.route, 2);
  const drilling = ({h, p}, stage, param) => {
    h.set(p.fsm, "State", 5);
    h.set(p.drill, {inDrillState: true, State: stage, drill: p.tool, param});
  };
  const held = check("gauge source held", f => { drilling(f, 3, f.p.drillParam); f.h.set(f.p.drill, {Held: true, Gauge: 80}); }, "ready");
  assert.equal(held.dig.held, true);
  assert.equal(held.dig.area, 70);
  for (const [name, id, depth, area] of [["foreign parameter row", 9999n, 30, 70], ["nonprogressing active row", 8001n, 0, 70], ["different active area", 8001n, 30, 10]]) {
    check(name, f => drilling(f, 6, f.h.set(f.h.object(PARAM), {...BASE_PARAM, Id: id, DigDepth: depth, BarEffArea: area})));
  }
  check("equal validated parameter copy", f => drilling(f, 6, f.h.set(f.h.object(PARAM), BASE_PARAM)), "ready");
  check("UI cache time must be finite", ({h, p}) => h.set(p.screen, "probedTime", NaN));
  check("UI cache time must be initialized", ({h, p}) => h.set(p.screen, "probedTime", -10000));
  const staleUi = check("UI cache must match mapped pointer", ({h, p}) => h.set(p.screen, "probedAt", {x: 999, y: 999}), "ready");
  assert.equal(staleUi.pointer.world, null);
  assert.equal(staleUi.pointer.picked, null);
  // GameplayScreen+620 is the unpinned field ahead of probedAt.
  const drivers = check("background-run drivers precede the UI probe", ({h, p}) => h.raw(p.screen + 620, "ptr", p.own), "ready");
  assert.equal(drivers.pointer.picked, 73);
  const oldProbe = check("previous build probe offset is stale", ({h, p}) => { h.raw(p.screen + 620, "float", 233); h.set(p.screen, "probedAt", {x: 1065, y: 999}); }, "ready");
  assert.equal(oldProbe.pointer.world, null);
  const coldPointer = fixture(false);
  assert.equal(coldPointer.sample().snapshot.pointer.world, null);
  cases++;
  const zoomed = fixture();
  assert.equal(zoomed.sample().snapshot.pointer.picked, 73);
  zoomed.h.set(zoomed.p.stage, "Current.WorldZoom", 2);
  assert.equal(zoomed.sample().snapshot.pointer.world, null);
  assert.equal(zoomed.sample().snapshot.pointer.world, null);
  zoomed.h.set(zoomed.p.screen, {probedAt: {x: 368, y: 912}, probedTime: 100.25});
  assert.equal(zoomed.sample().snapshot.pointer.picked, 73);
  cases++;
  const resized = fixture();
  resized.canvas.getBoundingClientRect = () => ({...RECT, width: 1200});
  assert.equal(resized.sample().snapshot.pointer.world, null);
  assert.equal(resized.sample().snapshot.pointer.world, null);
  resized.h.set(resized.p.screen, "probedTime", 100.25);
  assert.equal(resized.sample().snapshot.pointer.picked, 73);
  cases++;
  const queued = check("queued destination retained", ({h, p}) => h.set(p.session, "targetPending", true), "ready");
  assert.equal(queued.motion.targetPending, true);
  const sending = check("held stop retained", ({h, p}) => h.set(p.session, "heldStop", p.own), "ready");
  assert.equal(sending.motion.heldStop, true);
  const guard = fixture();
  for (const args of [["8", "synthetic-guard", "a", "b"], ["7", "bad", "a", "b"], ["7", "synthetic-guard", "0", "b"], ["7", "synthetic-guard", "10000000000000000", "b"], ["7", "synthetic-guard", "a", "0"]]) guard.control.call(...args);
  assert.equal(guard.messages.length, 0);
  guard.control.close();
  guard.control.sample();
  assert.equal(guard.messages.length, 0);
  cases++;
  const failed = fixture();
  failed.control.fail();
  failed.control.sample();
  assert.equal(failed.messages.length, 0);
  cases++;
  const unavailable = fixture();
  unavailable.control.cfg.build = "unknown";
  const fault = unavailable.sample();
  assert.deepEqual(Object.keys(fault.snapshot).sort(), ["reason", "status", "version"]);
  assert(!JSON.stringify(fault).includes("pointer"));
  cases++;

  // Every pin is read somewhere, so a pin that no read uses cannot hide a moved field.
  const expected = [...Object.keys(pins.fields), ...Object.keys(pins.cells), "s_GlobalMetadata", "s_GlobalMetadataHeader", "Il2CppGenericInst.type_argv[]",
    ...Object.entries(RUNTIME).flatMap(([name, fields]) => Object.keys(fields).map(f => `${name}.${f}`)),
    ...Object.entries(pins.rgctx).flatMap(([owner, slots]) => Object.keys(slots).filter(k => k[0] === "+").map(k => `rgctx ${owner}${k}`))];
  assert.deepEqual(expected.filter(label => !COVERED.has(label)), [], "pins no sampler read covers");
  process.stdout.write(`PASS sampler ${cases} invented-heap cases\n`);
}
