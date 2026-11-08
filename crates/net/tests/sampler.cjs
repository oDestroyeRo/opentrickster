"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const crypto = require("node:crypto");
const script = fs.readFileSync(path.join(__dirname,"../src/sampler.js"),"utf8");
const BUILD = "8502ebbf2fc9033d2fba50d02fd92002";

// Invented objects encode only the whitelisted fields, never a target runtime.
class Heap {
  constructor() {
    this.buffer = new ArrayBuffer(16*1024*1024);
    this.view = new DataView(this.buffer);
    this.next = 0x500000;
    this.classes = new Map();
    this.signatures = new Map();
  }
  alloc(n) { const p=this.next; this.next=(p+n+7)&~7; assert(this.next<this.buffer.byteLength); return p; }
  u32(p,n) { if(n===undefined)return this.view.getUint32(p,true); this.view.setUint32(p,n,true); }
  i32(p,n) { this.view.setInt32(p,n,true); }
  u16(p,n) { this.view.setUint16(p,n,true); }
  byte(p,n) { this.view.setUint8(p,n); }
  f32(p,n) { this.view.setFloat32(p,n,true); }
  f64(p,n) { this.view.setFloat64(p,n,true); }
  u64(p,n) { this.view.setBigUint64(p,BigInt(n),true); }
  type(id,args=[]) {
    const key=JSON.stringify([id,args]);
    if(this.signatures.has(key))return this.signatures.get(key);
    const p=this.alloc(8);this.signatures.set(key,p);
    if(!args.length){this.u32(p,id);this.byte(p+6,18);return p;}
    const g=this.alloc(12),inst=this.alloc(8),a=this.alloc(args.length*4);
    this.u32(g,this.type(id));this.u32(g+4,inst);this.u32(inst,args.length);this.u32(inst+4,a);
    args.forEach((arg,i)=>this.u32(a+4*i,Array.isArray(arg)?this.type(...arg):this.type(arg)));
    this.u32(p,g);this.byte(p+6,21);return p;
  }
  cls(id,size=256,args=[],valueSize=4) {
    const key=JSON.stringify([id,args]);
    if(this.classes.has(key)) { const c=this.classes.get(key);this.u32(c+128,Math.max(size,this.u32(c+128)));return c; }
    const c=this.alloc(2400),t=this.type(id,args);
    this.classes.set(key,c);
    new Uint8Array(this.buffer,c+16,8).set(new Uint8Array(this.buffer,t,8));
    this.u32(c+128,size);this.u32(c+132,valueSize);
    return c;
  }
  object(id,size=256,args=[]) { const p=this.alloc(size);this.u32(p,this.cls(id,size,args));return p; }
  statics(cell,id,size=16) {
    const c=this.cls(id),p=this.alloc(size);this.u32(cell,c);this.u32(c+76,p);return p;
  }
  root(cell,id,size,offset=0) { const s=this.statics(cell,id),p=this.object(id,size);this.u32(s+offset,p);return p; }
  array(id,stride,count) {
    const p=this.alloc(16+stride*count),c=this.alloc(192);
    this.u32(p,c);this.u32(c+32,this.cls(id,256,[],stride));this.u32(c+140,stride);this.i32(p+12,count);
    return p;
  }
  list(id,values=[],stride=4) {
    const p=this.object(10003,24,[id]),a=this.array(id,stride,values.length);
    this.u32(p+8,a);this.u32(p+12,values.length);this.u32(p+16,1);
    values.forEach((v,i)=>{if(stride===4)this.u32(a+16+i*4,v);});
    return p;
  }
  method(c,keyType,valueType) {
    const p=this.alloc(40),ctx=this.alloc(112);
    this.u32(p+16,c);this.u32(p+28,ctx);
    const keyClass=Array.isArray(keyType)?this.cls(keyType[0],256,keyType[1]):this.cls(keyType,256,[],[9021,9092].includes(keyType)?8:4);
    const valueClass=Array.isArray(valueType)?this.cls(valueType[0],256,valueType[1]):this.cls(valueType);
    const kw=this.u32(keyClass+132),vw=this.u32(valueClass+132);
    this.u32(ctx+36,0);this.u32(ctx+40,keyClass);this.u32(ctx+44,8);this.u32(ctx+48,valueClass);this.u32(ctx+52,8+kw);
    return {p,ctx,kw,vw};
  }
  dict(keyType,valueType,rows=[],cell=null,wrapper=false) {
    const p=this.object(9988,40,[keyType,valueType]);
    const c=this.u32(p),m=this.method(c,keyType,valueType),stride=(8+m.kw+m.vw+7)&~7;
    const a=this.array(9200,stride,rows.length),table=this.alloc(12);
    this.u32(p+12,a);this.u32(p+16,rows.length);this.u32(p+28,1);
    const iface=this.cls(9997,256,[keyType,valueType]);
    this.u16(c+182,1);this.u32(c+72,table);this.u32(table,iface);this.u32(table+4,0);this.u32(c+192+8+4,m.p);
    if(cell){
      if(wrapper){const w=this.alloc(40),ctx=this.alloc(112);this.u32(w+28,ctx);this.u32(ctx+52,m.p);this.u32(cell,w);}
      else this.u32(cell,m.p);
    }
    rows.forEach(([k,v],i)=>{
      const q=a+16+i*stride;this.i32(q,i);this.i32(q+4,-1);
      if(m.kw===8)this.u64(q+8,k);else this.u32(q+8,k);
      this.u32(q+8+m.kw,v);
    });
    return {p,a,c,method:m.p,ctx:m.ctx,kw:m.kw,vw:m.vw,stride};
  }
  string(s) {
    const p=this.object(8914,14+s.length*2);this.u32(p+8,s.length);
    [...s].forEach((ch,i)=>this.u16(p+12+i*2,ch.charCodeAt(0)));return p;
  }
  nullableMethods(value,has,vo,ho) {
    const [size,id,width] = value===3333252 ? [12,13048,8] : value===3333276 ? [12,13049,8] : value===3332368 ? [8,9020,4] : [2,8959,1];
    const owner=this.cls(9043,256,[id],size),v=this.cls(id,256,[],width);
    for(const [cell,off,field] of [[value,vo,8],[has,ho,12]]) {
      const p=this.alloc(40),ctx=this.alloc(56);this.u32(p+16,owner);this.u32(p+28,ctx);this.u32(ctx,v);this.u32(ctx+field,off);this.u32(cell,p);
    }
  }
}

function fixture() {
  const h=new Heap(), p={};
  h.nullableMethods(3333252,3333256,0,8);
  h.nullableMethods(3332368,3332376,0,4);
  h.nullableMethods(3331940,3331948,1,0);
  h.nullableMethods(3333276,3333272,0,8);
  p.flow=h.root(0x318aa4,2619,132,4);p.engine=h.root(0x31ad74,13662,44);
  h.u32(p.flow+16,p.engine);
  p.screen=h.object(2667,656);h.u32(p.flow+84,p.screen);
  p.stage=h.object(13683,392);h.u32(p.engine+20,p.stage);h.byte(p.stage+332,1);
  [8,10,1870,1310].forEach((n,i)=>h.f32(p.stage+56+4*i,n));h.f64(p.stage+144,1.25);
  h.f64(p.stage+136,1.0);h.f64(p.stage+152,1.0);[1920,1350,1536,1080].forEach((n,i)=>h.u32(p.stage+176+i*4,n));
  p.input=h.object(13641,128);h.u32(p.engine+28,p.input);
  p.own=h.object(5465,344);h.u32(p.screen+236,p.own);h.u32(p.own+40,41);h.u32(p.own+44,1);h.f64(p.own+56,350);h.f64(p.own+64,110);
  [-10,-10,20,20].forEach((n,i)=>h.i32(p.own+176+4*i,n));h.u32(p.own+12,h.list(5590,[],16));
  p.session=h.object(11817,140);h.u32(p.screen+144,p.session);h.byte(p.session+89,1);
  p.zone=h.object(2620,40);h.u32(p.screen+148,p.zone);
  p.zoneInfo=h.object(5619,60);h.u32(p.zone+8,p.zoneInfo);h.u32(p.zoneInfo+8,17);
  p.map=h.object(13771,224);h.u32(p.zone+16,p.map);h.u32(p.map+76,2000);h.u32(p.map+80,1500);
  h.u32(p.zone+24,h.list(5543));h.byte(p.screen+400,1);h.i32(p.screen+452,320);h.i32(p.screen+456,96);
  p.services=h.object(2731,16);h.u32(p.screen+316,p.services);
  p.combat=h.object(6101,276);p.drill=h.object(6256,232);p.assist=h.object(6215,60);p.gun=h.object(6629,176);p.attributes=h.object(5810,16);p.skills=h.object(8024,208);
  const services=[];
  for(const [id,v] of [[6101,p.combat],[6256,p.drill],[6215,p.assist],[6629,p.gun],[5810,p.attributes],[8024,p.skills]]) {
    const k=h.object(9155,24);h.u32(k+12,h.type(id));services.push([k,v]);
  }
  p.serviceDict=h.dict(9086,9200,services,0x325c6c);h.u32(p.services+8,p.serviceDict.p);
  p.fsm=h.object(5807,24);h.u32(p.drill+24,p.fsm);h.u32(p.combat+112,p.fsm);h.u32(p.assist+8,p.drill);
  for(const q of [p.combat,p.drill,p.gun])h.u32(q+20,p.session);
  p.hud=h.object(5355,160);h.u32(p.screen+324,p.hud);
  p.hotkeys=h.object(6741,28);h.u32(p.hud+80,p.hotkeys);
  function binding(code){const q=h.object(6892,20);h.u32(q+8,h.string(code));h.byte(q+16,1);return q;}
  p.digBinding=binding("KeyF");p.runBinding=binding("KeyV");
  p.bindingDict=h.dict(8914,[10003,[6892]],[[h.string("Dig"),h.list(6892,[p.digBinding])],[h.string("Run"),h.list(6892,[p.runBinding])]],3298216);
  h.u32(p.hotkeys+20,p.bindingDict.p);
  p.keyboard=h.root(3249944,10296,320);p.keys=h.array(10588,4,128);h.u32(p.keyboard+304,p.keys);
  p.buttons=[];
  for(let i=0;i<128;i++){const q=h.object(10588,256);h.byte(q+241,1);h.u32(p.keys+16+i*4,q);p.buttons.push(q);}
  for(const off of [272,276,280]){const q=h.object(10580,248);h.byte(q+241,1);h.u32(p.keyboard+off,q);}
  p.actions=[];
  for(const off of [16,20]) {
    const q=h.object(10211,236),map=h.object(10227,168),state=h.object(10245,120),states=h.alloc(68);
    h.u32(p.input+off,q);h.u32(q+112,map);h.u32(map+48,state);h.u32(state+40,1);h.u32(state+60,states);h.byte(states,1);p.actions.push(states);
  }
  p.run=h.object(7915,32);h.u32(p.screen+200,p.run);h.byte(p.run+24,1);
  p.inv=h.object(12096,44);h.u32(p.gun+16,p.inv);h.u32(p.drill+16,p.inv);
  p.slots=h.array(9092,8,23);h.u32(p.inv+16,p.slots);
  p.tool=h.object(12070,40);h.u64(p.tool+8,501);h.u64(p.tool+16,8001);h.u32(p.tool+28,300);h.u64(p.slots+16+7*8,501);
  p.inventory=h.dict(9092,12070,[[501,p.tool]],3334228,true);h.u32(p.inv+12,p.inventory.p);
  p.toolInfo=h.object(6756,260);h.u64(p.toolInfo+8,8001);h.u32(p.toolInfo+24,6);
  p.infos=h.dict(9021,6756,[[8001,p.toolInfo]]);h.u32(p.gun+28,p.infos.p);
  p.drillData=h.object(6225,36);h.u32(p.drill+12,p.drillData);p.petDict=h.dict(9021,9200,[],3296664);h.u32(p.drillData+32,p.petDict.p);
  p.drillParam=h.object(6222,64);h.u64(p.drillParam+8,8001);h.u32(p.drillParam+24,30);h.u32(p.drillParam+28,70);h.u32(p.drillParam+32,100);h.u32(p.drillParam+36,1);
  h.u32(p.drillData+8,h.dict(9021,6222,[[8001,p.drillParam]]).p);
  p.skins=h.statics(0x31b460,2483);h.u32(p.skins,1);h.byte(p.skins+4,1);
  p.semi=h.statics(0x318020,3183);h.byte(p.semi,1);h.byte(p.semi+1,0);
  p.monster=h.object(5465,344);h.u32(p.monster+40,73);h.u32(p.monster+44,3);h.f64(p.monster+56,500);h.f64(p.monster+64,300);
  [-10,-10,20,20].forEach((n,i)=>h.i32(p.monster+176+i*4,n));
  p.monsterView=h.object(13740,376);h.u32(p.monsterView+96,p.monster);
  p.actorDict=h.dict(9020,13740,[[73,p.monsterView]],3333612,true);h.u32(p.screen+244,p.actorDict.p);
  h.f32(p.screen+440,180);h.f32(p.screen+444,204);h.byte(p.screen+448,1);h.u32(p.screen+272,73);h.byte(p.screen+276,1);
  p.ground=h.object(2685,120);h.u32(p.screen+252,p.ground);
  h.statics(3245684,2374);
  p.covers=h.dict(9200,[8932,[8959]]);h.u32(h.statics(3248164,5356),p.covers.p);
  p.eventSystem=h.object(14177,52);h.byte(p.eventSystem+40,1);h.u32(h.statics(3246052,14177),h.list(14177,[p.eventSystem]));
  p.origins=h.root(3257428,2480,40);
  p.attackTargets=h.dict(9020,9020);h.u32(p.combat+104,p.attackTargets.p);
  p.waiting=h.dict(9020,6079);h.u32(p.combat+128,p.waiting.p);
  p.striking=h.dict(9020,[10012,[8924]]);h.u32(p.combat+124,p.striking.p);
  p.fading=h.object(11317,32,[9020,6080]);h.u32(p.combat+136,p.fading);
  p.fadingIndex=h.dict(9020,9020);h.u32(p.fading+8,p.fadingIndex.p);
  const fadeMethod=h.alloc(40),fadeCtx=h.alloc(112);h.u32(fadeMethod+28,fadeCtx);
  h.u32(fadeCtx+84,p.fadingIndex.method);h.u32(3333828,fadeMethod);
  const canvas={isConnected:true,width:1920,height:1350,getBoundingClientRect:()=>({left:27.5,top:41.25,width:960,height:540})};
  const module={HEAPU32:new Uint32Array(h.buffer),canvas};
  const messages=[];
  const context=vm.createContext({ArrayBuffer,DataView,Uint32Array,spinelInstance:{Module:module},devicePixelRatio:2,document:{hasFocus:()=>true,visibilityState:"visible",activeElement:canvas},getComputedStyle:()=>({transform:"none",visibility:"visible",display:"block",opacity:"1",pointerEvents:"auto"}),collect:(kind,fields,ingress)=>messages.push({kind,epoch:"7",rawIngress:ingress,...fields})});
  const control=vm.runInContext(`(function(){const cfg={build:${JSON.stringify(BUILD)},epoch:"7",guard:"synthetic-guard"};let closing=false,failed=false,ingress=19;const now=()=>100;function post(k,v){collect(k,v,ingress);}${script}\nreturn {sample:()=>globalThis.__opentricksterNetSampleV1("7","synthetic-guard","a","b"),close:()=>{closing=true},fail:()=>{failed=true},call:(...args)=>globalThis.__opentricksterNetSampleV1(...args),cfg,setIngress:n=>{ingress=n}};})()`,context);
  function sample(){control.sample();assert(messages.length);return messages.at(-1);}
  return {h,p,module,canvas,context,control,messages,sample};
}

if(process.argv.includes("--snapshot")) {
  const f=fixture(),value=f.sample();assert.equal(value.snapshot.status,"ready");
  process.stdout.write(JSON.stringify(value));
} else {
  let cases=0;
  function check(name,change,expected="unavailable") {
    const f=fixture();change(f);const s=f.sample().snapshot;assert.equal(s.status,expected,name);cases++;return s;
  }
  const base=fixture(),hash=()=>crypto.createHash("sha256").update(new Uint8Array(base.h.buffer)).digest("hex"),before=hash();
  const sample=base.sample();assert.equal(sample.snapshot.status,"ready");assert.equal(hash(),before,"sampler never writes memory");
  assert.equal(sample.rawIngress,19);assert.equal(sample.snapshot.pointer.picked,73);assert.equal(sample.snapshot.input.mode,1);assert.equal(sample.snapshot.dig.manual,true);assert.equal(sample.snapshot.dig.toolUsable,true);
  assert.deepEqual(Array.from(sample.snapshot.geometry.camera),[320,96]);assert.equal(sample.snapshot.binding.dig,"KeyF");
  assert.equal(base.sample().snapshot.revision,1);base.h.i32(base.p.screen+452,321);assert.equal(base.sample().snapshot.revision,2);cases++;
  check("tagged flow",f=>f.h.u32(0x318aa4,0x200086e7));
  check("null engine",f=>f.h.u32(0x31ad74,0));
  check("misaligned object",f=>f.h.u32(f.p.flow+84,f.p.screen+1));
  check("truncated object",f=>f.h.u32(f.p.flow+84,f.h.buffer.byteLength-8));
  check("type-confused screen",f=>f.h.u32(f.p.screen,f.h.cls(5465,656)));
  check("wrong class extent",f=>f.h.u32(f.h.u32(f.p.screen)+128,16));
  check("cold scene",f=>f.h.byte(f.p.screen+400,0));
  for(const o of [401,402,403,404,600,601])check("covered/changing scene "+o,f=>f.h.byte(f.p.screen+o,1));
  check("pan",f=>f.h.i32(f.p.stage+280,1));
  check("host override with zero first component",f=>f.h.byte(f.p.stage+52,1));
  check("nonfinite zoom",f=>f.h.f64(f.p.stage+144,NaN));
  check("unresolved method",f=>f.h.u32(0x325c6c,0xa0003fdb));
  check("unresolved context",f=>f.h.u32(f.h.u32(0x325c6c)+28,0));
  check("wrong specialization",f=>f.h.u32(f.p.actorDict.p,f.p.serviceDict.c));
  check("invalid entry stride",f=>f.h.u32(f.h.u32(f.p.actorDict.a)+140,4));
  check("count beyond array",f=>f.h.i32(f.p.actorDict.p+16,2));
  check("negative count",f=>f.h.i32(f.p.actorDict.p+16,-1));
  check("oversized dictionary",f=>f.h.i32(f.p.actorDict.p+16,100000));
  check("deleted count mismatch",f=>f.h.i32(f.p.actorDict.a+16,-1));
  check("service identity mismatch",f=>{const key=f.h.u32(f.p.serviceDict.a+24);f.h.u32(key+12,f.h.type(5465));});
  check("root mutation",f=>{f.canvas.getBoundingClientRect=()=>{f.h.u32(f.p.flow+84,f.p.own);return {left:27.5,top:41.25,width:960,height:540};};});
  check("dictionary version changed",f=>{f.canvas.getBoundingClientRect=()=>{f.h.u32(f.p.actorDict.p+28,2);return {left:27.5,top:41.25,width:960,height:540};};});
  check("memory growth while reading",f=>{let calls=0;Object.defineProperty(f.module,"HEAPU32",{get:()=>++calls===1?new Uint32Array(f.h.buffer):new Uint32Array(new ArrayBuffer(16*1024*1024))});});
  const grown=fixture();grown.sample();const bigger=new ArrayBuffer(20*1024*1024);new Uint8Array(bigger).set(new Uint8Array(grown.h.buffer));grown.module.HEAPU32=new Uint32Array(bigger);assert.equal(grown.sample().snapshot.status,"ready");cases++;
  check("malformed boolean",f=>f.h.byte(f.p.screen+400,2));
  check("modal UI",f=>f.h.u32(f.p.eventSystem+36,f.p.screen));
  check("key routing masked",f=>f.h.u32(f.p.hud+68,1));
  check("unknown binding",f=>f.h.u32(f.p.digBinding+8,f.h.string("Key1")));
  check("modifier binding",f=>f.h.u32(f.p.digBinding+12,1));
  check("conflicting run/dig",f=>f.h.u32(f.p.runBinding+8,f.h.string("KeyF")));
  check("missing nullable metadata",f=>f.h.u32(3331940,0));
  check("unknown skin startup",f=>f.h.byte(f.p.skins+4,0));
  check("unknown heavy callback",f=>{f.h.byte(f.p.run+8,1);f.h.u32(f.p.run+20,f.h.object(9177,64));});
  const running=check("observed running, absent Heavy delegate",f=>f.h.byte(f.p.run+8,1),"ready");assert.equal(running.input.mode,2);
  for (const [carried,attrs,expected] of [[90,17,1],[89,17,2],[90,0,2],[2147483647,17,2]]) {
    const heavy=check("source heavy predicate",f=>{
      const {h,p}=f,delegate=h.object(9177,64),state=h.object(2950,56),stats=h.object(7412,28),values=h.object(7409,96);
      h.byte(p.run+8,1);h.u32(p.run+20,delegate);h.u32(delegate+8,49652);h.u32(delegate+16,state);h.u64(state+8,carried);
      h.u32(state+24,p.hud);h.u32(state+28,p.screen);h.u32(state+40,p.run);h.u32(p.hud+76,stats);h.u32(stats+8,values);h.f64(values+56,100);h.u32(p.attributes+8,attrs);
    },"ready");assert.equal(heavy.input.mode,expected);
  }
  const cached=check("source cached modifier value",f=>{
    const {h,p}=f,q=h.u32(p.keyboard+272),method=h.alloc(40),ctx=h.alloc(56),manager=h.object(10327,752);
    h.byte(q+241,0);h.f32(q+168,0);h.f32(q+244,0.5);h.u32(method+28,ctx);h.u32(method+16,h.cls(10272,256,[9064]));h.u32(ctx+4,h.cls(9064));h.u32(ctx+16,164);h.u32(ctx+28,168);h.u32(3309104,method);
    h.u32(h.statics(3249368,10203)+4,manager);h.byte(manager+736,1);
  },"ready");assert.equal(cached.input.modifiers[0],false);
  const gun=check("actual equipped gun branch",f=>{
    const {h,p}=f,item=h.object(12070,40),info=h.object(6756,260);h.u64(item+8,502);h.u64(item+16,9000);h.u32(item+28,1);h.u64(p.slots+16+1*8,502);
    h.u64(info+8,9000);h.u32(info+24,2);h.u32(info+28,54);
    h.u32(p.inv+12,h.dict(9092,12070,[[501,p.tool],[502,item]],3334228,true).p);h.u32(p.gun+28,h.dict(9021,6756,[[8001,p.toolInfo],[9000,info]]).p);
    h.byte(p.gun+44,1);h.byte(p.gun+45,1);h.u32(p.gun+48,3);
  },"ready");assert.equal(gun.combat.gunWorn,true);assert.equal(gun.combat.gunReady,true);
  check("foreign nullable specialization",f=>f.h.u32(f.h.u32(3333252)+16,f.h.cls(9043,256,[9020],8)));
  check("nullable field overlap",f=>{const ctx=f.h.u32(f.h.u32(3333256)+28);f.h.u32(ctx+12,1);});
  check("unknown manual setting",f=>f.h.byte(f.p.semi,0));
  check("cold drill parameter",f=>{f.h.u32(f.p.fsm+8,5);f.h.byte(f.p.drill+84,1);});
  check("native page zoom",f=>{f.context.visualViewport={scale:2,offsetLeft:0,offsetTop:0};});
  check("non-canvas DOM editor",f=>{f.context.document.activeElement={tagName:"INPUT"};});
  check("canvas changed during capture",f=>{let n=0;f.canvas.getBoundingClientRect=()=>({left:27.5,top:41.25,width:++n===1?960:900,height:540});});
  const deleted=check("deleted dictionary row skipped",f=>{f.h.i32(f.p.actorDict.a+16,-1);f.h.u32(f.p.actorDict.p+24,1);f.h.byte(f.p.screen+276,0);},"ready");assert.equal(deleted.actors.length,1);
  const occupied=check("non-own combat work is distinct",f=>{const {h,p}=f;h.u32(p.combat+104,h.dict(9020,9020,[[99,73]]).p);},"ready");assert.equal(occupied.combat.ownAttack,false);
  const fade=check("actual fading target",f=>{const {h,p}=f,d=h.dict(9020,9020,[[73,0]]);h.u32(p.fading+8,d.p);const ctx=h.u32(h.u32(3333828)+28);h.u32(ctx+84,d.method);},"ready");assert.equal(fade.actors[1].fading,true);
  check("selected skill consumes world input",f=>f.h.u32(f.p.skills+52,f.p.monster));
  check("pending held cast",f=>f.h.u32(f.p.skills+188,f.p.monster));
  const noDig=check("source map dig refusal",f=>f.h.u32(f.p.zoneInfo+24,5),"ready");assert.equal(noDig.dig.mapAllowed,false);
  for (const option of [0,1]) {
    const pet=check("source automatic pet and zone gate",f=>{
      const {h,p}=f,item=h.object(12070,40),info=h.object(6756,260);h.u64(item+8,503);h.u64(item+16,7000);h.u32(item+28,1);h.u64(p.slots+16+8*8,503);
      h.u64(info+8,7000);h.u32(info+24,19);h.u32(p.zoneInfo+40,option);
      h.u32(p.inv+12,h.dict(9092,12070,[[501,p.tool],[503,item]],3334228,true).p);h.u32(p.gun+28,h.dict(9021,6756,[[8001,p.toolInfo],[7000,info]]).p);
      h.u32(p.drillData+32,h.dict(9021,9200,[[7000,p.own]],3296664).p);
    },"ready");assert.equal(pet.dig.automaticPet,option===0);assert.equal(pet.dig.manual,option===1);
  }
  check("wrong visual hover",f=>f.h.u32(f.p.screen+272,41),"ready");
  const noPick=check("hidden target",f=>f.h.byte(f.p.monster+160,1),"ready");assert.equal(noPick.pointer.picked,null);
  const shift=check("observed shift retained",f=>f.h.byte(f.h.u32(f.p.keyboard+272)+240,1),"ready");assert.equal(shift.input.modifiers[0],true);
  const semi=check("semi-auto retained",f=>f.h.byte(f.p.semi+1,1),"ready");assert.equal(semi.dig.manual,false);
  const assist=check("helper state 2 is busy",f=>f.h.u32(f.p.assist+48,2),"ready");assert.equal(assist.dig.assist,2);
  const held=check("gauge source held",f=>{f.h.u32(f.p.fsm+8,5);f.h.byte(f.p.drill+84,1);f.h.u32(f.p.drill+100,3);f.h.byte(f.p.drill+152,1);const q=f.h.object(6222,64);f.h.u32(q+28,70);f.h.u32(f.p.drill+96,q);f.h.u32(f.p.drill+92,f.p.tool);f.h.f32(f.p.drill+104,80);},"ready");assert.equal(held.dig.held,true);assert.equal(held.dig.area,70);
  const queued=check("queued destination retained",f=>f.h.byte(f.p.session+88,1),"ready");assert.equal(queued.motion.targetPending,true);
  const sending=check("held stop retained",f=>f.h.u32(f.p.session+132,f.p.own),"ready");assert.equal(sending.motion.heldStop,true);
  const guard=fixture();for(const args of [["8","synthetic-guard","a","b"],["7","bad","a","b"],["7","synthetic-guard","0","b"],["7","synthetic-guard","10000000000000000","b"],["7","synthetic-guard","a","0"]])guard.control.call(...args);assert.equal(guard.messages.length,0);guard.control.close();guard.control.sample();assert.equal(guard.messages.length,0);cases++;
  const failed=fixture();failed.control.fail();failed.control.sample();assert.equal(failed.messages.length,0);cases++;
  const unavailable=fixture();unavailable.control.cfg.build="unknown";const fault=unavailable.sample();assert.deepEqual(Object.keys(fault.snapshot).sort(),["reason","status","version"]);assert(!JSON.stringify(fault).includes("pointer"));cases++;
  process.stdout.write(`PASS sampler ${cases} invented-heap cases\n`);
}
