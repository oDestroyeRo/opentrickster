{
  // Build ea1a68bd, source joins in issues 25, 27, 28 and 29, re-derived for issue 37. No target calls.
  // Pins come from tools/build-migrate/sampler/ea1a68bd.json; tests/sampler.cjs checks every read against it.
  const samplerBuild = "ea1a68bd89958fae94179f1e0835ffb5";
  // At runtime Il2CppType.data for classes and primitives is a handle into the loaded metadata, not an index.
  const metadataCells = [4659832,4659828], typeDefinitions = [7487400,1019502,15447];
  const typeKinds = {8917:14,8962:2,9023:8,9024:10,9067:12,9095:11,9203:28,5592:17,9046:17,9985:17,10004:17,13051:17,13052:17};
  const typeSizes = {2481:40,2621:132,2622:40,2669:656,2670:72,2673:16,2687:120,2733:16,2952:56,5357:160,5467:344,5545:40,5622:60,5782:56,5810:24,5813:16,6104:280,6218:60,6225:64,6228:36,6259:232,6632:176,6744:28,6759:264,6895:20,7412:96,7415:28,7918:32,8027:208,8917:16,9158:24,10214:236,10230:168,10248:120,10299:320,10330:752,11820:144,12073:40,12099:48,13644:128,13665:44,13686:392,13743:376,13774:224,14184:52};
  let sampleRevision = 0, previousGeometry = null, previousProbeTime = null, probeFloor = null, pointerGeometryReady = false;
  const sampleHex = value => typeof value === "string" && /^[1-9a-f][0-9a-f]{0,15}$/.test(value);
  const unavailable = code => { throw code; };
  function readClient() {
    if (cfg.build !== samplerBuild) unavailable("build");
    const module = globalThis.spinelInstance?.Module;
    const buffer = module?.HEAPU32?.buffer;
    if (!(buffer instanceof ArrayBuffer)) unavailable("cold");
    const view = new DataView(buffer), reads = new Map();
    function extent(p, n, alignment = 4) {
      if (!Number.isSafeInteger(p) || p < 1024 || p % alignment || !Number.isSafeInteger(n) || n < 0 || p > buffer.byteLength - n) unavailable("memory");
      return p;
    }
    function read(p, kind, width, align = width) {
      extent(p, width, align);
      if (reads.size >= 200000) unavailable("capacity");
      const value = view[kind](p, true);
      const key = `${kind}:${p}`;
      if (reads.has(key) && !Object.is(reads.get(key).value, value)) unavailable("changing");
      reads.set(key, {p, kind, value});
      return value;
    }
    const u32 = p => read(p, "getUint32", 4);
    const i32 = p => read(p, "getInt32", 4);
    const u16 = p => read(p, "getUint16", 2);
    const byte = p => read(p, "getUint8", 1);
    const f32 = p => finite(read(p, "getFloat32", 4));
    const f64 = p => finite(read(p, "getFloat64", 8, 4));
    const u64 = p => read(p, "getBigUint64", 8, 4);
    const bool = p => { const n = byte(p); if (n > 1) unavailable("shape"); return n === 1; };
    function finite(n, low = -1000000, high = 1000000) {
      if (!Number.isFinite(n) || n < low || n > high) unavailable("shape");
      return n;
    }
    function integer(n, low, high) {
      if (!Number.isSafeInteger(n) || n < low || n > high) unavailable("shape");
      return n;
    }
    function pointer(p) { return extent(u32(p), 4); }
    // By-value kinds from the pinned metadata. Other whitelisted objects are classes.
    const typeKind = id => typeKinds[id] ?? 18;
    function type(p, depth = 0) {
      if (depth > 4) unavailable("generic");
      extent(p, 8);
      const code = byte(p + 6), data = u32(p);
      if (code === 21) {
        extent(data, 12);
        const base = type(pointer(data),depth+1), inst = pointer(data + 4);
        const count = integer(u32(inst), 1, 4), args = pointer(inst + 4);
        extent(args, 4 * count);
        if (base.args.length || ![17,18].includes(base.kind)) unavailable("generic");
        return {id: base.id, kind:base.kind, args: Array.from({length: count}, (_, i) => type(pointer(args + i * 4),depth+1))};
      }
      const offset = data - typeBase;
      if (offset < 0 || offset % typeStride) unavailable("type");
      const id = integer(offset / typeStride, 0, typeDefinitions[2] - 1);
      if (code !== typeKind(id)) unavailable("type");
      return {id, kind:code, args: []};
    }
    function sameType(a, b) { return a.id === b.id && a.kind === b.kind && a.args.length === b.args.length && a.args.every((v, i) => sameType(v, b.args[i])); }
    function klass(p, id, args) {
      extent(p, 192);
      const t = type(p + 16);
      if (t.id !== id || (args && (args.length !== t.args.length || !args.every((v, i) => sameType({id:v,kind:typeKind(v),args:[]}, t.args[i]))))) unavailable("type");
      return p;
    }
    function object(p, id) {
      const size = typeSizes[id] ?? unavailable("type");
      extent(p, size);
      const c = klass(pointer(p), id);
      if (u32(c + 128) < size || u32(c + 128) > 65536) unavailable("type");
      return p;
    }
    function root(cell, id, offset) {
      const c = klass(pointer(cell), id), statics = pointer(c + 76);
      return object(pointer(statics + offset), id);
    }
    function statics(cell, id) { return pointer(klass(pointer(cell), id) + 76); }
    function context(method) {
      extent(method, 40);
      return extent(pointer(method + 28), 56);
    }
    const methodCell = pointer;
    function array(p, stride, limit, element) {
      extent(p, 16);
      const c = pointer(p), count = integer(i32(p + 12), 0, limit);
      extent(c, 192);
      if (u32(c + 140) !== stride || u32(p + 8) !== 0) unavailable("generic");
      if (byte(c + 22) !== 29 || !sameType(type(pointer(c + 16)),type(pointer(c + 32) + 16))) unavailable("generic");
      if (element !== undefined) klass(pointer(c + 32), element);
      extent(p, 16 + count * stride);
      return {p, c, count, data:p + 16};
    }
    function list(p, element, stride = 4, limit = 512) {
      extent(p, 24);
      klass(pointer(p), 10006, [element]);
      const count = integer(i32(p + 12), 0, limit), a = array(pointer(p + 8), stride, limit, element);
      if (count > a.count) unavailable("generic");
      i32(p + 16);
      return {count, data:a.data};
    }
    function virtualTry(p) {
      const c = pointer(p), signature = type(c + 16);
      if (signature.id !== 9991 || signature.args.length !== 2) unavailable("generic");
      const n = integer(u16(c + 182), 1, 64), table = pointer(c + 72);
      extent(table, 12 * n);
      let found = 0;
      for (let i = 0; i < n; i++) {
        const row = table + i * 12, iface = pointer(row), t = type(iface + 16);
        if (t.id !== 10000 || t.args.length !== 2 || !t.args.every((a, j) => sameType(a, signature.args[j]))) continue;
        if (found) unavailable("generic");
        const slot = integer(i32(row + 4), 0, 256) + 1;
        found = pointer(c + 192 + slot * 8 + 4);
      }
      if (!found) unavailable("generic");
      return found;
    }
    function width(t) {
      if ([14,18,28].includes(t.kind)) return 4;
      const n = {8962:1,9023:4,9024:8,9067:4,9095:8}[t.id];
      if (!n || t.args.length) unavailable("generic");
      return n;
    }
    function dictionary(p, keyType, valueType, limit = 512, method = null) {
      extent(p, 48);
      const c = klass(pointer(p), 9991), t = type(c + 16);
      if (t.args.length !== 2 || t.args[0].id !== keyType || (valueType !== null && t.args[1].id !== valueType)) unavailable("generic");
      const m = method || virtualTry(p), ctx = context(m);
      if (pointer(m + 16) !== c || !sameType(type(pointer(ctx + 40) + 16), t.args[0]) || !sameType(type(pointer(ctx + 48) + 16), t.args[1])) unavailable("generic");
      const hash = integer(u32(ctx + 36), 0, 120), key = integer(u32(ctx + 44), 0, 120), value = integer(u32(ctx + 52), 0, 120);
      const kw = width(t.args[0]), vw = width(t.args[1]);
      if (u32(pointer(ctx + 40) + 132) !== kw || u32(pointer(ctx + 48) + 132) !== vw) unavailable("generic");
      if (hash % 4 || key % Math.min(kw,4) || value % Math.min(vw,4) || key < hash + 8 || value < key + kw) unavailable("generic");
      const count = integer(i32(p + 16), 0, limit), free = integer(i32(p + 24), 0, count);
      i32(p + 28);
      const ep = u32(p + 12);
      if (!ep) { if (count || free || u32(p + 8)) unavailable("generic"); return []; }
      extent(ep, 16);
      const stride = integer(u32(pointer(ep) + 140), value + vw, 128);
      if (stride % 4) unavailable("generic");
      const entry = klass(pointer(pointer(ep) + 32),9985), entryType = type(entry + 16);
      if (entryType.args.length !== 2 || !entryType.args.every((arg,i) => sameType(arg,t.args[i])) || u32(entry + 132) !== stride) unavailable("generic");
      const a = array(ep, stride, limit);
      if (count > a.count) unavailable("generic");
      const rows = [];
      let deleted = 0;
      for (let i = 0; i < count; i++) {
        const row = a.data + stride * i, h = i32(row + hash);
        if (h === -1) { deleted++; continue; }
        if (h < 0) unavailable("generic");
        rows.push({key:row + key, value:row + value});
      }
      if (deleted !== free) unavailable("generic");
      return rows;
    }
    // OrderedMap`2 rows, as its Enumerator.MoveNext walks them: slots[0, used) minus gone[] tombstones.
    function orderedMap(p, keyType, valueType, limit, cell) {
      extent(p, 32);
      const c = klass(pointer(p), 11320, [keyType, valueType]), m = methodCell(cell), ctx = context(m);
      if (pointer(m + 16) !== c) unavailable("generic");
      const pair = klass(pointer(ctx + 12), 10004, [keyType, valueType]), getValue = pointer(ctx + 16), pctx = context(getValue);
      if (pointer(getValue + 16) !== pair || pointer(pctx + 4) !== pair) unavailable("generic");
      const kw = width(type(pair + 16).args[0]), stride = integer(u32(pair + 132), kw + 4, 64);
      if (u32(klass(pointer(pctx), keyType) + 132) !== kw || u32(klass(pointer(pctx + 12), valueType) + 132) !== 4) unavailable("generic");
      const key = integer(u32(pctx + 8), 0, stride - kw), value = integer(u32(pctx + 16), 0, stride - 4);
      if (stride % 4 || key % Math.min(kw, 4) || value % 4 || (value < key + kw && key < value + 4)) unavailable("generic");
      const slots = array(pointer(p + 12), stride, limit);
      if (pointer(slots.c + 32) !== pair) unavailable("generic");
      const used = integer(i32(p + 20), 0, slots.count), gaps = integer(i32(p + 24), 0, used), gone = u32(p + 16);
      i32(p + 28);
      const marks = gone ? array(gone, 1, 65535, 8962) : null;
      if (marks ? marks.count < used : gaps) unavailable("generic");
      const rows = [];
      let holes = 0;
      for (let i = 0; i < used; i++) {
        if (marks && bool(marks.data + i)) { holes++; continue; }
        rows.push({key:slots.data + stride * i + key, value:slots.data + stride * i + value});
      }
      if (holes !== gaps) unavailable("generic");
      return rows;
    }
    function nullable(p, valueCell, hasCell, size, valueType, width, readValue) {
      const vm = methodCell(valueCell), hm = methodCell(hasCell), vc = context(vm), hc = context(hm);
      const owner = klass(pointer(vm+16),9046,[valueType]);
      if (pointer(hm+16) !== owner || u32(owner+132) !== size || u32(klass(pointer(vc),valueType)+132) !== width) unavailable("generic");
      const vo = integer(u32(vc + 8), 0, size - 1), ho = integer(u32(hc + 12), 0, size - 1);
      if (vo+width > size || (ho >= vo && ho < vo+width)) unavailable("generic");
      extent(p, size, 1);
      return bool(p + ho) ? readValue(p + vo) : null;
    }
    function hostOverride(p) {
      const method = methodCell(3334088), c = klass(pointer(method+16),9046,[13052]);
      if (u32(c+132)!==12) unavailable("generic");
      return bool(p+integer(u32(context(method)+12),0,11));
    }
    const vec = p => [f32(p), f32(p + 4)];
    const nullablePoint = p => nullable(p, 3334068, 3334072, 12, 13051, 8, vec);
    const nullableId = p => nullable(p, 3333184, 3333192, 8, 9023, 4, q => integer(i32(q), 1, 65535));
    function text(p, max = 16) {
      object(p, 8917);
      const n = integer(i32(p + 8), 0, max);
      extent(p, 12 + n * 2 + 2);
      if (u16(p + 12 + n * 2) !== 0) unavailable("shape");
      return String.fromCharCode(...Array.from({length:n}, (_, i) => u16(p + 12 + i * 2)));
    }
    function selectedText(p, allowed) {
      object(p, 8917);
      const n = integer(i32(p + 8), 0, 128);
      return allowed.filter(s => s.length === n).some(s => {
        extent(p, 12 + n * 2 + 2);
        return u16(p + 12 + n * 2) === 0 && Array.from(s).every((ch,i) => u16(p + 12 + i * 2) === ch.charCodeAt(0));
      });
    }
    function bindings(p) {
      object(p, 6744);
      const rows = dictionary(pointer(p + 20), 8917, 10006, 256, methodCell(3299016));
      const result = {dig:null,run:null};
      for (const row of rows) {
        const k = pointer(row.key);
        const name = selectedText(k,["Dig"]) ? "dig" : selectedText(k,["Run"]) ? "run" : null;
        if (!name) continue;
        if (result[name]) unavailable("binding");
        const a = list(pointer(row.value),6895,4,8);
        if (a.count !== 1) unavailable("binding");
        const b = object(pointer(a.data),6895), code = text(pointer(b + 8));
        bool(b+16);
        if (i32(b + 12) !== 0 || !/^Key[A-Z]$/.test(code)) unavailable("binding");
        result[name] = code;
      }
      if (!result.dig || !result.run || result.dig === result.run) unavailable("binding");
      return result;
    }
    function button(p) {
      extent(p,248);
      const id = type(pointer(p) + 16).id;
      if (![10580,10583,10586,10591].includes(id)) unavailable("type");
      if (bool(p + 241)) return bool(p + 240);
      // ReadValue's initialized float specialization; stale caches must remain unknown.
      if (bool(p + 104)) unavailable("input");
      const method = methodCell(3309924), ctx = context(method), off = integer(u32(ctx + 28),164,224);
      klass(pointer(method+16),10275,[9067]);
      if (u32(klass(pointer(ctx+4),9067)+132) !== 4) unavailable("generic");
      if (bool(p + integer(u32(ctx + 16),164,224))) unavailable("input");
      const manager = object(pointer(statics(3250132,10206) + 4),10330);
      if (!bool(manager + 736)) unavailable("input");
      let threshold = f32(p + 244);
      if (threshold <= 0) threshold = f32(statics(3243056,10583));
      finite(threshold,0.000001,1);
      return f32(p + off) >= threshold;
    }
    function actionPressed(p) {
      object(p,10214);
      const map = object(pointer(p + 112),10230), state = object(pointer(map + 48),10248);
      if (bool(state + 109) || bool(state + 110)) unavailable("input");
      const n = integer(i32(state + 40),1,256), i = integer(i32(p + 108),0,n-1), a = pointer(state + 60);
      extent(a,n * 68);
      const phase = integer(byte(a + i * 68),1,4);
      if (phase === 1 && (byte(a + i * 68 + 1) & 64)) unavailable("input");
      return !!(byte(a + i * 68 + 1) & 64);
    }
    function idSet(p, keyType = 9023, valueType = null, method = null) {
      const out = new Set();
      for (const row of dictionary(p,keyType,valueType,512,method)) {
        const k = integer(i32(row.key),1,65535);
        if (out.has(k)) unavailable("generic");
        out.add(k);
      }
      return out;
    }
    // Same arithmetic as libil2cpp f2486: s_GlobalMetadata plus the s_GlobalMetadataHeader typeDefinitions triple.
    const metadata = pointer(metadataCells[0]), header = extent(pointer(metadataCells[1]), 248);
    if (u32(header) !== 0xfab11baf || typeDefinitions.some((v, i) => u32(header + 236 + 4 * i) !== v)) unavailable("build");
    const typeBase = metadata + typeDefinitions[0], typeStride = typeDefinitions[1] / typeDefinitions[2];
    const flow = root(0x318d9c,2621,4), engine = root(0x31b074,13665,0);
    if (pointer(flow + 16) !== engine) unavailable("changing");
    const screen = object(pointer(flow + 84),2669), stage = object(pointer(engine + 20),13686);
    const input = object(pointer(engine + 28),13644), own = object(pointer(screen + 236),5467);
    const ownId = integer(i32(own + 40),1,65535);
    const session = object(pointer(screen + 144),11820);
    const zone = object(pointer(screen + 148),2622), zoneInfo = object(pointer(zone + 8),5622), map = object(pointer(zone + 16),13774);
    if (!bool(screen + 400) || [401,402,403,404,600,601].some(o => bool(screen + o))) unavailable("scene");
    if (hostOverride(stage+44) || i32(stage + 280) !== 0 || !bool(stage + 332)) unavailable("layout");
    const services = object(pointer(screen + 316),2733), serviceRows = dictionary(pointer(services + 8),9089,9203,256,methodCell(0x325f98));
    const wanted = new Map([[6104,null],[6259,null],[6218,null],[6632,null],[5813,null],[8027,null]]);
    for (const row of serviceRows) {
      const p = pointer(row.value), id = type(pointer(p) + 16).id;
      if (!wanted.has(id)) continue;
      if (wanted.get(id)) unavailable("generic");
      // RuntimeType inherits Type._impl, a RuntimeTypeHandle whose value is the Il2CppType*.
      const key = object(pointer(row.key),9158);
      if (!sameType(type(pointer(key + 8)),type(pointer(p) + 16))) unavailable("generic");
      wanted.set(id,p);
    }
    if ([6104,6632,5813,8027].some(id => !wanted.get(id))) unavailable("cold");
    // The deferred DrillUi.Install provides DrillFeature, awaits ItemsModel, then provides DigAssist and registers Dig in one step.
    const drillAt = wanted.get(6259), assistAt = wanted.get(6218);
    if (drillAt && !assistAt) unavailable("cold");
    if (assistAt && !drillAt) unavailable("changing");
    const skills = object(wanted.get(8027),8027);
    if (u32(skills+52) || u32(skills+56) || u32(skills+188) || bool(skills+124) || bool(skills+125)) unavailable("routing");
    const combat = object(wanted.get(6104),6104), gun = object(wanted.get(6632),6632);
    const drill = drillAt ? object(drillAt,6259) : null, assist = assistAt ? object(assistAt,6218) : null;
    const fsm = object(pointer(combat + 112),5810), fsmState = integer(i32(fsm + 8),0,64);
    if (pointer(combat + 20) !== session || pointer(gun + 20) !== session) unavailable("changing");
    if (drill && (pointer(drill + 24) !== fsm || pointer(assist + 8) !== drill || pointer(drill + 20) !== session)) unavailable("changing");
    const hud = object(pointer(screen + 324),5357), binding = bindings(pointer(hud + 80));
    const keyboard = object(pointer(statics(3250708,10299)),10299), keys = array(pointer(keyboard + 304),4,256,10591);
    function keyButton(code) { const index = 14 + code.charCodeAt(3) - 65; if (index >= keys.count) unavailable("input"); return button(pointer(keys.data + index * 4)); }
    const modifiers = [272,276,280].map(o => button(pointer(keyboard + o)));
    // f34098/f34099 read Key 57/58; get_Item f34042 indexes by Key - 1.
    if (keys.count < 58) unavailable("input");
    modifiers.push(button(pointer(keys.data + 56 * 4)) || button(pointer(keys.data + 57 * 4)));
    const run = object(pointer(screen + 200),7918);
    let mode = 1;
    if (bool(run + 8)) {
      const heavy = u32(run + 20);
      if (!heavy) mode = 2;
      else {
        extent(heavy,64);
        if (u32(heavy + 8) !== 49699 || u32(heavy + 60) !== 0) unavailable("mode");
        const state = object(pointer(heavy + 16),2952);
        if (pointer(state + 24) !== hud || pointer(state + 28) !== screen || pointer(state + 40) !== run) unavailable("mode");
        const stats = object(pointer(hud + 76),7415), values = object(pointer(stats + 8),7412);
        const max = Math.trunc(f64(values + 56)), attrs = u32(object(wanted.get(5813),5813) + 8);
        if (max <= 0 || max > 2147483647) unavailable("mode");
        const carried = Number(BigInt.asIntN(32,u64(state + 8))), ratio = Math.trunc(Math.imul(carried,100) / max);
        mode = (attrs & 17) && ratio > 89 ? 1 : 2;
      }
    }
    if (!bool(run + 24)) unavailable("mode");
    const inv = object(pointer(gun + 16),12099);
    if (drill && pointer(drill + 16) !== inv) unavailable("changing");
    const slots = array(pointer(inv + 16),8,64,9095);
    if (slots.count < 23) unavailable("cold");
    const needed = new Map();
    for (const slot of [1,4,7,8,9,22]) { const uid = u64(slots.data + slot * 8); if (uid !== 0n) needed.set(uid,null); }
    for (const row of orderedMap(pointer(inv + 12),9095,12073,4096,3335044)) {
      const uid = u64(row.key);
      if (!needed.has(uid)) continue;
      if (needed.get(uid)) unavailable("generic");
      const item = object(pointer(row.value),12073);
      if (u64(item + 8) !== uid) unavailable("generic");
      needed.set(uid,{id:u64(item + 16),count:i32(item + 28),p:item});
    }
    if ([...needed.values()].some(v => !v)) unavailable("cold");
    const infoRows = dictionary(pointer(gun + 28),9024,6759,32768), infos = new Map();
    const itemIds = new Set([...needed.values()].map(v => v.id));
    for (const row of infoRows) {
      const id = u64(row.key);
      if (!itemIds.has(id)) continue;
      if (infos.has(id)) unavailable("generic");
      const info = object(pointer(row.value),6759);
      if (u64(info + 8) !== id) unavailable("generic");
      infos.set(id,{type:i32(info + 24),subtype:i32(info + 28)});
    }
    if ([...itemIds].some(id => !infos.has(id))) unavailable("cold");
    const itemAt = slot => needed.get(u64(slots.data + slot * 8)) || null;
    const gunWorn = [4,9,1,22].some(s => { const item = itemAt(s); return item && infos.get(item.id).subtype === 54; });
    const pet = itemAt(8), data = drill && object(pointer(drill + 12),6228);
    const wornTool = itemAt(7);
    let usableTool = false, equippedParam = null;
    const drillParameters = p => {
      object(p,6225);
      return {id:u64(p+8),grade:i32(p+16),nature:i32(p+20),depth:i32(p+24),area:i32(p+28),speed:i32(p+32),mass:i32(p+36),life:i32(p+40)};
    };
    if (drill && wornTool && infos.get(wornTool.id).type === 6 && wornTool.count > 0) {
      const rows = dictionary(pointer(data+8),9024,6225,512);
      const matching = rows.filter(row => u64(row.key) === wornTool.id);
      if (matching.length !== 1) unavailable("cold");
      const p = object(pointer(matching[0].value),6225);
      equippedParam = drillParameters(p);
      if (equippedParam.id!==wornTool.id) unavailable("generic");
      usableTool = equippedParam.depth>0 && equippedParam.area>0 && equippedParam.area<=200 && equippedParam.speed>0 && equippedParam.mass>0;
    }
    let automaticPet = false;
    if (drill && pet) automaticPet = dictionary(pointer(data + 32),9024,null,512,methodCell(3297464)).some(row => u64(row.key) === pet.id) && (u32(zoneInfo+40)&1)===0;
    const skins = statics(0x31b760,2484);
    if (!bool(skins+4)) unavailable("cold");
    const skin = integer(i32(skins),0,1);
    const semiAuto = nullable(statics(0x318318,3185),3332756,3332764,2,8962,1,bool);
    if (semiAuto === null) unavailable("binding");
    const manual = !(skin === 1 && semiAuto) && !automaticPet;
    const actors = [], actorIds = new Set([ownId]);
    function actor(p) {
      object(p,5467);
      return {raw:integer(i32(p + 40),1,65535),type:integer(i32(p + 44),0,64),x:f64(p + 56),y:f64(p + 64),hit:[i32(p + 176),i32(p + 180),integer(i32(p + 184),0,65535),integer(i32(p + 188),0,65535)],hidden:bool(p + 160) || bool(p + 161) || bool(p + 162)};
    }
    actors.push(actor(own));
    for (const row of orderedMap(pointer(screen + 244),9023,13743,512,3334428)) {
      const a = actor(pointer(object(pointer(row.value),13743) + 96));
      if (integer(i32(row.key),1,65535) !== a.raw || actorIds.has(a.raw)) unavailable("generic");
      actorIds.add(a.raw); actors.push(a);
    }
    const local = nullablePoint(screen + 440), held = nullablePoint(screen + 416), hovered = nullableId(screen + 272);
    const canvas = module.canvas;
    const doc = globalThis.document;
    if (!canvas || !canvas.isConnected || !doc?.hasFocus() || doc.visibilityState !== "visible") unavailable("layout");
    if (![canvas,doc.body,doc.documentElement].includes(doc.activeElement)) unavailable("routing");
    const viewport = globalThis.visualViewport;
    if (viewport && (viewport.scale !== 1 || viewport.offsetLeft !== 0 || viewport.offsetTop !== 0)) unavailable("layout");
    const rect = canvas.getBoundingClientRect(), style = globalThis.getComputedStyle(canvas);
    if (style.transform !== "none" || style.visibility !== "visible" || style.display === "none" || Number(style.opacity) !== 1 || style.pointerEvents === "none") unavailable("layout");
    const geometry = {rect:[rect.left,rect.top,rect.width,rect.height].map(v => finite(v)),backing:[integer(canvas.width,1,16384),integer(canvas.height,1,16384)],host:[f32(stage + 56),f32(stage + 60),f32(stage + 64),f32(stage + 68)],camera:[i32(screen + 452),i32(screen + 456)],zoom:f64(stage + 144),uiScale:f64(stage+136),pxPerDp:f64(stage+152),viewport:[integer(i32(stage+176),1,16384),integer(i32(stage+180),1,16384)],world:[integer(i32(stage+184),1,16384),integer(i32(stage+188),1,16384)],dpr:finite(globalThis.devicePixelRatio,0.1,16),map:[integer(i32(map + 76),1,65535),integer(i32(map + 80),1,65535)]};
    if (geometry.rect[2] <= 0 || geometry.rect[3] <= 0 || geometry.host[0] < 0 || geometry.host[1] < 0 || geometry.host[2] <= 0 || geometry.host[3] <= 0 || geometry.host[0]+geometry.host[2]>geometry.backing[0] || geometry.host[1]+geometry.host[3]>geometry.backing[1] || geometry.zoom <= 0 || geometry.uiScale <= 0 || geometry.pxPerDp <= 0) unavailable("layout");
    const pointerWorld = local ? [Math.fround(local[0] + geometry.camera[0]),Math.fround(local[1] + geometry.camera[1])] : null;
    const intersects = (a,p) => !a.hidden && a.hit[2] > 0 && a.hit[3] > 0 && p[0] >= a.x+a.hit[0] && p[0] < a.x+a.hit[0]+a.hit[2] && p[1] >= a.y+a.hit[1] && p[1] < a.y+a.hit[1]+a.hit[3];
    const hits = pointerWorld ? actors.filter(a => intersects(a,pointerWorld)) : [];
    const picked = hits.length === 1 && hits[0].raw !== ownId && hits[0].raw === hovered ? hovered : null;
    const warps = list(pointer(zone + 24),5545,4,256), portals = [];
    for (let i = 0; i < warps.count; i++) { const p = object(pointer(warps.data + i*4),5545); portals.push([i32(p+20),i32(p+24),i32(p+28),i32(p+32)]); }
    const ground = object(pointer(screen + 256),2687), hover = object(pointer(ground + 68),2673);
    let groundHover = u32(hover + 12) !== 0;
    if (groundHover) object(pointer(hover + 12),2670);
    // No compiled code writes HoverDraw.Entry. GroundItems.Click finds a picked item actor through entries.
    for (const row of dictionary(pointer(ground + 36),9023,2670,512)) {
      const view = u32(object(pointer(row.value),2670) + 12);
      if (!view) continue;
      const item = actor(pointer(object(view,13743) + 96));
      if (integer(i32(row.key),1,65535) !== item.raw) unavailable("generic");
      if (pointerWorld && intersects(item,pointerWorld)) groundHover = true;
    }
    // EditBox.Focused takes keys before the HUD. While features install, a key or menu without a registered action
    // waits in waitMenu/waitAction, then in due until Hud.Update replays it at a time the caller does not choose.
    const edit = statics(3246444,2375);
    if (u32(edit + 24) !== 0 || i32(hud + 68) !== 0 || u32(hud + 136) || u32(hud + 140) || u32(hud + 148)) unavailable("routing");
    const covers = pointer(statics(3248924,5358));
    if (dictionary(covers,9203,8935,256).length) unavailable("routing");
    const eventSystems = list(pointer(statics(3246812,14184)),14184,4,8);
    if (eventSystems.count !== 1) unavailable("routing");
    const es = object(pointer(eventSystems.data),14184);
    if (u32(es + 36) || !bool(es + 40) || bool(es + 41)) unavailable("routing");
    const origins = root(3258196,2481,0);
    if (bool(origins + 32) || bool(origins + 33)) unavailable("routing");
    const attackTargets = idSet(pointer(combat + 104),9023,9023), waiting = idSet(pointer(combat + 128)), striking = idSet(pointer(combat + 124));
    const fading = new Set();
    for (const row of orderedMap(pointer(combat + 136),9023,6083,512,3334644)) {
      const k = integer(i32(row.key),1,65535);
      if (fading.has(k)) unavailable("generic");
      fading.add(k);
    }
    for (const a of actors) a.fading = fading.has(a.raw);
    let dig = null;
    if (drill) {
      const param = u32(drill + 96), tool = u32(drill + 92);
      if (fsmState === 5 && (!param || !tool)) unavailable("cold");
      const automatic = u32(drill+196);
      if (tool && (!wornTool || object(tool,12073) !== wornTool.p)) unavailable("changing");
      const activeParam = param ? drillParameters(param) : null;
      if (fsmState === 5 && (!usableTool || !equippedParam || Object.keys(equippedParam).some(key => activeParam[key] !== equippedParam[key]))) unavailable("changing");
      dig = {fsm:fsmState,flags:u32(drill+60),stage:integer(i32(drill+100),0,7),held:bool(drill+152),inState:bool(drill+84),gauge:f32(drill+104),area:activeParam?.area ?? null,assist:integer(i32(assist+48),0,4),autoMode:bool(drill+192),autoFlag:bool(drill+200),automatic:automatic ? integer(i32(object(automatic,5782)+24),0,16)!==0 : false,crazy:bool(drill+154),manual,automaticPet,mapAllowed:![1,5,6].includes(i32(zoneInfo+24)) && i32(zoneInfo+44)!==14,toolUsable:usableTool};
    }
    const zoneId = integer(i32(zoneInfo+8),1,65535);
    const equipment = [1,4,7,8,9,22].map(slot => u64(slots.data+slot*8).toString());
    const geometryKey = [buffer,flow,engine,screen,session,zone,zoneId,own,ownId,fsm,JSON.stringify([geometry,binding,mode,skin,semiAuto,equipment])];
    const geometryChanged = !previousGeometry || geometryKey.some((v,i) => v !== previousGeometry[i]);
    // Cached world/pick facts need a later MapPoint probe under the new geometry.
    const probePosition = vec(screen+624), probeTime = f64(screen+632);
    if (probeTime < 0) unavailable("routing");
    const resetProbe = geometryChanged || (previousProbeTime !== null && probeTime < previousProbeTime);
    const nextFloor = resetProbe ? probeTime : probeFloor;
    const nextPointerReady = !resetProbe && (pointerGeometryReady || probeTime > nextFloor);
    const pointerCurrent = nextPointerReady && local && Math.abs(probePosition[0]-(geometry.host[0]+local[0]*Math.fround(geometry.zoom)))<=0.25 && Math.abs(probePosition[1]-(geometry.host[1]+geometry.host[3]-local[1]*Math.fround(geometry.zoom)))<=0.25;
    const nextRevision = sampleRevision + (geometryChanged ? 1 : 0);
    const snapshot = {version:1,status:"ready",revision:nextRevision,zone:zoneId,geometry,pointer:{world:pointerCurrent?pointerWorld:null,picked:pointerCurrent?picked:null,groundHover,overUi:bool(screen+640)},binding,input:{left:actionPressed(pointer(input+16)),right:actionPressed(pointer(input+20)),digHeld:keyButton(binding.dig),runHeld:keyButton(binding.run),modifiers,mode},motion:{own:ownId,fsm:fsmState,path:list(pointer(own+12),5592,16,1024).count,route:u32(screen+372)?list(pointer(screen+372),5592,16,1024).count:0,steering:bool(screen+108)||bool(screen+116),screenHeld:held!==null,targetPending:bool(session+88),heldStop:u32(session+132)!==0,sendArmed:bool(session+89)},combat:{target:nullableId(combat+60),ownAttack:attackTargets.has(ownId),waiting:waiting.has(ownId)||striking.has(ownId),fainted:bool(combat+68),gunWorn,gunArmed:bool(gun+44),gunReady:bool(gun+45),gunState:integer(i32(gun+48),0,5)},dig,actors,portals};
    if (module !== globalThis.spinelInstance?.Module || buffer !== module.HEAPU32?.buffer) unavailable("changing");
    const finalRect = canvas.getBoundingClientRect();
    if (module.canvas !== canvas || [finalRect.left,finalRect.top,finalRect.width,finalRect.height].some((v,i)=>v!==geometry.rect[i]) || canvas.width!==geometry.backing[0] || canvas.height!==geometry.backing[1] || globalThis.devicePixelRatio!==geometry.dpr) unavailable("changing");
    for (const {p,kind,value} of reads.values()) if (!Object.is(view[kind](p,true),value)) unavailable("changing");
    if (JSON.stringify(snapshot).length > 60000) unavailable("capacity");
    sampleRevision=nextRevision;previousGeometry=geometryKey;previousProbeTime=probeTime;probeFloor=nextFloor;pointerGeometryReady=nextPointerReady;
    return snapshot;
  }
  globalThis.__opentricksterNetSampleV1 = (epoch, guard, request, layout) => {
    if (closing || failed || epoch !== cfg.epoch || guard !== cfg.guard || !sampleHex(request) || !sampleHex(layout)) return;
    const capturedPerformance = now();
    let snapshot;
    try { snapshot = readClient(); }
    catch (e) { snapshot = {version:1,status:"unavailable",reason:["build","cold","memory","capacity","changing","shape","type","generic","binding","input","mode","scene","layout","routing"].includes(e)?e:"shape"}; }
    post("sample",{request,layout,ageMs:Math.max(0,Math.ceil(now()-capturedPerformance)),snapshot});
  };
}
