{
  // Build 8502ebbf, source joins in issues 25, 27, 28 and 29. No target calls.
  const samplerBuild = "8502ebbf2fc9033d2fba50d02fd92002";
  const typeKinds = {8914:14,8959:2,9020:8,9021:10,9064:12,9092:11,9200:28,5590:17,9043:17,9982:17,13048:17,13049:17};
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
      const id = integer(data, 0, 15439);
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
    function object(p, id, size) {
      extent(p, size);
      const c = klass(pointer(p), id);
      if (u32(c + 128) < size || u32(c + 128) > 65536) unavailable("type");
      return p;
    }
    function root(cell, id, offset, size) {
      const c = klass(pointer(cell), id), statics = pointer(c + 76);
      return object(pointer(statics + offset), id, size);
    }
    function statics(cell, id) { return pointer(klass(pointer(cell), id) + 76); }
    function context(method) {
      extent(method, 40);
      return extent(pointer(method + 28), 56);
    }
    function methodCell(cell, wrapper = false) {
      const m = pointer(cell);
      return wrapper ? pointer(context(m) + 52) : m;
    }
    function array(p, stride, limit, element) {
      extent(p, 16);
      const c = pointer(p), count = integer(i32(p + 12), 0, limit);
      extent(c, 192);
      if (u32(c + 140) !== stride || u32(p + 8) !== 0) unavailable("generic");
      if (byte(c + 22) !== 29 || !sameType(type(pointer(c + 16)),type(pointer(c + 32) + 16))) unavailable("generic");
      if (element !== undefined) klass(pointer(c + 32), element);
      extent(p, 16 + count * stride);
      return {p, count, data:p + 16};
    }
    function list(p, element, stride = 4, limit = 512) {
      extent(p, 24);
      klass(pointer(p), 10003, [element]);
      const count = integer(i32(p + 12), 0, limit), a = array(pointer(p + 8), stride, limit, element);
      if (count > a.count) unavailable("generic");
      i32(p + 16);
      return {count, data:a.data};
    }
    function virtualTry(p) {
      const c = pointer(p), signature = type(c + 16);
      if (signature.id !== 9988 || signature.args.length !== 2) unavailable("generic");
      const n = integer(u16(c + 182), 1, 64), table = pointer(c + 72);
      extent(table, 12 * n);
      let found = 0;
      for (let i = 0; i < n; i++) {
        const row = table + i * 12, iface = pointer(row), t = type(iface + 16);
        if (t.id !== 9997 || t.args.length !== 2 || !t.args.every((a, j) => sameType(a, signature.args[j]))) continue;
        if (found) unavailable("generic");
        const slot = integer(i32(row + 4), 0, 256) + 1;
        found = pointer(c + 192 + slot * 8 + 4);
      }
      if (!found) unavailable("generic");
      return found;
    }
    function dictionary(p, keyType, valueType, limit = 512, method = null) {
      extent(p, 40);
      const c = klass(pointer(p), 9988), t = type(c + 16);
      if (t.args.length !== 2 || t.args[0].id !== keyType || (valueType !== null && t.args[1].id !== valueType)) unavailable("generic");
      const m = method || virtualTry(p), ctx = context(m);
      if (pointer(m + 16) !== c || !sameType(type(pointer(ctx + 40) + 16), t.args[0]) || !sameType(type(pointer(ctx + 48) + 16), t.args[1])) unavailable("generic");
      const hash = integer(u32(ctx + 36), 0, 120), key = integer(u32(ctx + 44), 0, 120), value = integer(u32(ctx + 52), 0, 120);
      const width = t => {
        if ([14,18,28].includes(t.kind)) return 4;
        const n = {8959:1,9020:4,9021:8,9064:4,9092:8}[t.id];
        if (!n || t.args.length) unavailable("generic");
        return n;
      };
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
      const entry = klass(pointer(pointer(ep) + 32),9982), entryType = type(entry + 16);
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
    function nullable(p, valueCell, hasCell, size, valueType, width, readValue) {
      const vm = methodCell(valueCell), hm = methodCell(hasCell), vc = context(vm), hc = context(hm);
      const owner = klass(pointer(vm+16),9043,[valueType]);
      if (pointer(hm+16) !== owner || u32(owner+132) !== size || u32(klass(pointer(vc),valueType)+132) !== width) unavailable("generic");
      const vo = integer(u32(vc + 8), 0, size - 1), ho = integer(u32(hc + 12), 0, size - 1);
      if (vo+width > size || (ho >= vo && ho < vo+width)) unavailable("generic");
      extent(p, size, 1);
      return bool(p + ho) ? readValue(p + vo) : null;
    }
    function hostOverride(p) {
      const method = methodCell(3333272), c = klass(pointer(method+16),9043,[13049]);
      if (u32(c+132)!==12) unavailable("generic");
      return bool(p+integer(u32(context(method)+12),0,11));
    }
    const vec = p => [f32(p), f32(p + 4)];
    const nullablePoint = p => nullable(p, 3333252, 3333256, 12, 13048, 8, vec);
    const nullableId = p => nullable(p, 3332368, 3332376, 8, 9020, 4, q => integer(i32(q), 1, 65535));
    function text(p, max = 16) {
      object(p, 8914, 12);
      const n = integer(i32(p + 8), 0, max);
      extent(p, 12 + n * 2 + 2);
      if (u16(p + 12 + n * 2) !== 0) unavailable("shape");
      return String.fromCharCode(...Array.from({length:n}, (_, i) => u16(p + 12 + i * 2)));
    }
    function selectedText(p, allowed) {
      object(p, 8914, 12);
      const n = integer(i32(p + 8), 0, 128);
      return allowed.filter(s => s.length === n).some(s => {
        extent(p, 12 + n * 2 + 2);
        return u16(p + 12 + n * 2) === 0 && Array.from(s).every((ch,i) => u16(p + 12 + i * 2) === ch.charCodeAt(0));
      });
    }
    function bindings(p) {
      object(p, 6741, 28);
      const rows = dictionary(pointer(p + 20), 8914, 10003, 256, methodCell(3298216));
      const result = {dig:null,run:null};
      for (const row of rows) {
        const k = pointer(row.key);
        const name = selectedText(k,["Dig"]) ? "dig" : selectedText(k,["Run"]) ? "run" : null;
        if (!name) continue;
        if (result[name]) unavailable("binding");
        const a = list(pointer(row.value),6892,4,8);
        if (a.count !== 1) unavailable("binding");
        const b = object(pointer(a.data),6892,20), code = text(pointer(b + 8));
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
      if (![10577,10580,10583,10588].includes(id)) unavailable("type");
      if (bool(p + 241)) return bool(p + 240);
      // ReadValue's initialized float specialization; stale caches must remain unknown.
      if (bool(p + 104)) unavailable("input");
      const method = methodCell(3309104), ctx = context(method), off = integer(u32(ctx + 28),164,224);
      klass(pointer(method+16),10272,[9064]);
      if (u32(klass(pointer(ctx+4),9064)+132) !== 4) unavailable("generic");
      if (bool(p + integer(u32(ctx + 16),164,224))) unavailable("input");
      const manager = object(pointer(statics(3249368,10203) + 4),10327,752);
      if (!bool(manager + 736)) unavailable("input");
      let threshold = f32(p + 244);
      if (threshold <= 0) threshold = f32(statics(3242296,10580));
      finite(threshold,0.000001,1);
      return f32(p + off) >= threshold;
    }
    function actionPressed(p) {
      object(p,10211,236);
      const map = object(pointer(p + 112),10227,168), state = object(pointer(map + 48),10245,120);
      if (bool(state + 109) || bool(state + 110)) unavailable("input");
      const n = integer(i32(state + 40),1,256), i = integer(i32(p + 108),0,n-1), a = pointer(state + 60);
      extent(a,n * 68);
      const phase = integer(byte(a + i * 68),1,4);
      if (phase === 1 && (byte(a + i * 68 + 1) & 64)) unavailable("input");
      return !!(byte(a + i * 68 + 1) & 64);
    }
    function idSet(p, keyType = 9020, valueType = null, method = null) {
      const out = new Set();
      for (const row of dictionary(p,keyType,valueType,512,method)) {
        const k = integer(i32(row.key),1,65535);
        if (out.has(k)) unavailable("generic");
        out.add(k);
      }
      return out;
    }
    const flow = root(0x318aa4,2619,4,132), engine = root(0x31ad74,13662,0,44);
    if (pointer(flow + 16) !== engine) unavailable("changing");
    const screen = object(pointer(flow + 84),2667,656), stage = object(pointer(engine + 20),13683,392);
    const input = object(pointer(engine + 28),13641,128), own = object(pointer(screen + 236),5465,344);
    const ownId = integer(i32(own + 40),1,65535);
    const session = object(pointer(screen + 144),11817,140);
    const zone = object(pointer(screen + 148),2620,40), zoneInfo = object(pointer(zone + 8),5619,60), map = object(pointer(zone + 16),13771,224);
    if (!bool(screen + 400) || [401,402,403,404,600,601].some(o => bool(screen + o))) unavailable("scene");
    if (hostOverride(stage+44) || i32(stage + 280) !== 0 || !bool(stage + 332)) unavailable("layout");
    const services = object(pointer(screen + 316),2731,16), serviceRows = dictionary(pointer(services + 8),9086,9200,256,methodCell(0x325c6c));
    const wanted = new Map([[6101,null],[6256,null],[6215,null],[6629,null],[5810,null],[8024,null]]);
    for (const row of serviceRows) {
      const p = pointer(row.value), id = type(pointer(p) + 16).id;
      if (!wanted.has(id)) continue;
      if (wanted.get(id)) unavailable("generic");
      const key = object(pointer(row.key),9155,24);
      if (!sameType(type(pointer(key + 12)),type(pointer(p) + 16))) unavailable("generic");
      wanted.set(id,p);
    }
    if ([...wanted.values()].some(p => !p)) unavailable("cold");
    const skills = object(wanted.get(8024),8024,208);
    if (u32(skills+52) || u32(skills+56) || u32(skills+188) || bool(skills+124) || bool(skills+125)) unavailable("routing");
    const combat = object(wanted.get(6101),6101,276), drill = object(wanted.get(6256),6256,232), assist = object(wanted.get(6215),6215,60), gun = object(wanted.get(6629),6629,176);
    const fsm = object(pointer(drill + 24),5807,24), fsmState = integer(i32(fsm + 8),0,64);
    if (pointer(combat + 112) !== fsm || pointer(assist + 8) !== drill || pointer(combat + 20) !== session || pointer(drill + 20) !== session || pointer(gun + 20) !== session) unavailable("changing");
    const hud = object(pointer(screen + 324),5355,160), binding = bindings(pointer(hud + 80));
    const keyboard = object(pointer(statics(3249944,10296)),10296,320), keys = array(pointer(keyboard + 304),4,256,10588);
    function keyButton(code) { const index = 14 + code.charCodeAt(3) - 65; if (index >= keys.count) unavailable("input"); return button(pointer(keys.data + index * 4)); }
    const modifiers = [272,276,280].map(o => button(pointer(keyboard + o)));
    // f34070/f34071 read Key 57/58; get_Item f34014 indexes by Key - 1.
    if (keys.count < 58) unavailable("input");
    modifiers.push(button(pointer(keys.data + 56 * 4)) || button(pointer(keys.data + 57 * 4)));
    const run = object(pointer(screen + 200),7915,32);
    let mode = 1;
    if (bool(run + 8)) {
      const heavy = u32(run + 20);
      if (!heavy) mode = 2;
      else {
        extent(heavy,64);
        if (u32(heavy + 8) !== 49652 || u32(heavy + 60) !== 0) unavailable("mode");
        const state = object(pointer(heavy + 16),2950,56);
        if (pointer(state + 24) !== hud || pointer(state + 28) !== screen || pointer(state + 40) !== run) unavailable("mode");
        const stats = object(pointer(hud + 76),7412,28), values = object(pointer(stats + 8),7409,96);
        const max = Math.trunc(f64(values + 56)), attrs = u32(object(wanted.get(5810),5810,16) + 8);
        if (max <= 0 || max > 2147483647) unavailable("mode");
        const carried = Number(BigInt.asIntN(32,u64(state + 8))), ratio = Math.trunc(Math.imul(carried,100) / max);
        mode = (attrs & 17) && ratio > 89 ? 1 : 2;
      }
    }
    if (!bool(run + 24)) unavailable("mode");
    const inv = object(pointer(gun + 16),12096,44);
    if (pointer(drill + 16) !== inv) unavailable("changing");
    const slots = array(pointer(inv + 16),8,64,9092);
    if (slots.count < 23) unavailable("cold");
    const needed = new Map();
    for (const slot of [1,4,7,8,9,22]) { const uid = u64(slots.data + slot * 8); if (uid !== 0n) needed.set(uid,null); }
    for (const row of dictionary(pointer(inv + 12),9092,12070,4096,methodCell(3334228,true))) {
      const uid = u64(row.key);
      if (!needed.has(uid)) continue;
      if (needed.get(uid)) unavailable("generic");
      const item = object(pointer(row.value),12070,40);
      if (u64(item + 8) !== uid) unavailable("generic");
      needed.set(uid,{id:u64(item + 16),count:i32(item + 28),p:item});
    }
    if ([...needed.values()].some(v => !v)) unavailable("cold");
    const infoRows = dictionary(pointer(gun + 28),9021,6756,32768), infos = new Map();
    const itemIds = new Set([...needed.values()].map(v => v.id));
    for (const row of infoRows) {
      const id = u64(row.key);
      if (!itemIds.has(id)) continue;
      if (infos.has(id)) unavailable("generic");
      const info = object(pointer(row.value),6756,260);
      if (u64(info + 8) !== id) unavailable("generic");
      infos.set(id,{type:i32(info + 24),subtype:i32(info + 28)});
    }
    if ([...itemIds].some(id => !infos.has(id))) unavailable("cold");
    const itemAt = slot => needed.get(u64(slots.data + slot * 8)) || null;
    const gunWorn = [4,9,1,22].some(s => { const item = itemAt(s); return item && infos.get(item.id).subtype === 54; });
    const pet = itemAt(8), data = object(pointer(drill + 12),6225,36);
    const wornTool = itemAt(7);
    let usableTool = false, equippedParam = null;
    const drillParameters = p => {
      object(p,6222,64);
      return {id:u64(p+8),grade:i32(p+16),nature:i32(p+20),depth:i32(p+24),area:i32(p+28),speed:i32(p+32),mass:i32(p+36),life:i32(p+40)};
    };
    if (wornTool && infos.get(wornTool.id).type === 6 && wornTool.count > 0) {
      const rows = dictionary(pointer(data+8),9021,6222,512);
      const matching = rows.filter(row => u64(row.key) === wornTool.id);
      if (matching.length !== 1) unavailable("cold");
      const p = object(pointer(matching[0].value),6222,64);
      equippedParam = drillParameters(p);
      if (equippedParam.id!==wornTool.id) unavailable("generic");
      usableTool = equippedParam.depth>0 && equippedParam.area>0 && equippedParam.area<=200 && equippedParam.speed>0 && equippedParam.mass>0;
    }
    let automaticPet = false;
    if (pet) automaticPet = dictionary(pointer(data + 32),9021,null,512,methodCell(3296664)).some(row => u64(row.key) === pet.id) && (u32(zoneInfo+40)&1)===0;
    const skins = statics(0x31b460,2483);
    if (!bool(skins+4)) unavailable("cold");
    const skin = integer(i32(skins),0,1);
    const semiAuto = nullable(statics(0x318020,3183),3331940,3331948,2,8959,1,bool);
    if (semiAuto === null) unavailable("binding");
    const manual = !(skin === 1 && semiAuto) && !automaticPet;
    const actors = [], actorIds = new Set([ownId]);
    function actor(p) {
      object(p,5465,344);
      return {raw:integer(i32(p + 40),1,65535),type:integer(i32(p + 44),0,64),x:f64(p + 56),y:f64(p + 64),hit:[i32(p + 176),i32(p + 180),integer(i32(p + 184),0,65535),integer(i32(p + 188),0,65535)],hidden:bool(p + 160) || bool(p + 161) || bool(p + 162)};
    }
    actors.push(actor(own));
    for (const row of dictionary(pointer(screen + 244),9020,13740,512,methodCell(3333612,true))) {
      const a = actor(pointer(object(pointer(row.value),13740,376) + 96));
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
    const warps = list(pointer(zone + 24),5543,4,256), portals = [];
    for (let i = 0; i < warps.count; i++) { const p = object(pointer(warps.data + i*4),5543,40); portals.push([i32(p+20),i32(p+24),i32(p+28),i32(p+32)]); }
    const ground = object(pointer(screen + 252),2685,120), groundHover = u32(ground + 68) !== 0;
    if (groundHover) object(pointer(ground+68),2668,72);
    const edit = statics(3245684,2374);
    if (u32(edit) !== 0 || i32(hud + 68) !== 0 || bool(hud + 128)) unavailable("routing");
    const covers = pointer(statics(3248164,5356));
    if (dictionary(covers,9200,8932,256).length) unavailable("routing");
    const eventSystems = list(pointer(statics(3246052,14177)),14177,4,8);
    if (eventSystems.count !== 1) unavailable("routing");
    const es = object(pointer(eventSystems.data),14177,52);
    if (u32(es + 36) || !bool(es + 40) || bool(es + 41)) unavailable("routing");
    const origins = root(3257428,2480,0,40);
    if (bool(origins + 32) || bool(origins + 33)) unavailable("routing");
    const attackTargets = idSet(pointer(combat + 104),9020,9020), waiting = idSet(pointer(combat + 128)), striking = idSet(pointer(combat + 124));
    const fadingMap = pointer(combat + 136);
    klass(pointer(fadingMap),11317,[9020,6080]);
    const fading = idSet(pointer(fadingMap+8),9020,9020,pointer(context(methodCell(3333828))+84));
    for (const a of actors) a.fading = fading.has(a.raw);
    const param = u32(drill + 96), tool = u32(drill + 92);
    if (fsmState === 5 && (!param || !tool)) unavailable("cold");
    const automatic = u32(drill+196);
    if (tool && (!wornTool || object(tool,12070,40) !== wornTool.p)) unavailable("changing");
    const activeParam = param ? drillParameters(param) : null;
    if (fsmState === 5 && (!usableTool || !equippedParam || Object.keys(equippedParam).some(key => activeParam[key] !== equippedParam[key]))) unavailable("changing");
    const dig = {fsm:fsmState,flags:u32(drill+60),stage:integer(i32(drill+100),0,7),held:bool(drill+152),inState:bool(drill+84),gauge:f32(drill+104),area:activeParam?.area ?? null,assist:integer(i32(assist+48),0,4),autoMode:bool(drill+192),autoFlag:bool(drill+200),automatic:automatic ? integer(i32(object(automatic,5779,56)+24),0,16)!==0 : false,crazy:bool(drill+154),manual,automaticPet,mapAllowed:![1,5,6].includes(i32(zoneInfo+24)) && i32(zoneInfo+44)!==14,toolUsable:usableTool};
    const zoneId = integer(i32(zoneInfo+8),1,65535);
    const equipment = [1,4,7,8,9,22].map(slot => u64(slots.data+slot*8).toString());
    const geometryKey = [buffer,flow,engine,screen,session,zone,zoneId,own,ownId,fsm,JSON.stringify([geometry,binding,mode,skin,semiAuto,equipment])];
    const geometryChanged = !previousGeometry || geometryKey.some((v,i) => v !== previousGeometry[i]);
    // Cached world/pick facts need a later MapPoint probe under the new geometry.
    const probePosition = vec(screen+620), probeTime = f64(screen+632);
    if (probeTime < 0) unavailable("routing");
    const resetProbe = geometryChanged || (previousProbeTime !== null && probeTime < previousProbeTime);
    const nextFloor = resetProbe ? probeTime : probeFloor;
    const nextPointerReady = !resetProbe && (pointerGeometryReady || probeTime > nextFloor);
    const pointerCurrent = nextPointerReady && local && Math.abs(probePosition[0]-(geometry.host[0]+local[0]*Math.fround(geometry.zoom)))<=0.25 && Math.abs(probePosition[1]-(geometry.host[1]+geometry.host[3]-local[1]*Math.fround(geometry.zoom)))<=0.25;
    const nextRevision = sampleRevision + (geometryChanged ? 1 : 0);
    const snapshot = {version:1,status:"ready",revision:nextRevision,zone:zoneId,geometry,pointer:{world:pointerCurrent?pointerWorld:null,picked:pointerCurrent?picked:null,groundHover,overUi:bool(screen+640)},binding,input:{left:actionPressed(pointer(input+16)),right:actionPressed(pointer(input+20)),digHeld:keyButton(binding.dig),runHeld:keyButton(binding.run),modifiers,mode},motion:{own:ownId,fsm:fsmState,path:list(pointer(own+12),5590,16,1024).count,route:u32(screen+372)?list(pointer(screen+372),5590,16,1024).count:0,steering:bool(screen+108)||bool(screen+116),screenHeld:held!==null,targetPending:bool(session+88),heldStop:u32(session+132)!==0,sendArmed:bool(session+89)},combat:{target:nullableId(combat+60),ownAttack:attackTargets.has(ownId),waiting:waiting.has(ownId)||striking.has(ownId),fainted:bool(combat+68),gunWorn,gunArmed:bool(gun+44),gunReady:bool(gun+45),gunState:integer(i32(gun+48),0,5)},dig,actors,portals};
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
