'use strict';
const assert = require('node:assert/strict');
const vm = require('node:vm');
const input = JSON.parse(require('node:fs').readFileSync(0,'utf8'));
const cfg = JSON.parse(input.script.match(/const cfg = (.*);/)[1]);
const messages=[], sockets=[], timers=[];
let clock=100;
class Socket {
  static CONNECTING=0; static OPEN=1; static CLOSING=2; static CLOSED=3;
  constructor(url){this.url=url;this.readyState=0;this.bufferedAmount=0;this.handlers={};this.closeCalls=0;sockets.push(this);}
  addEventListener(name,fn){(this.handlers[name]??=[]).push(fn);}
  send(_){if(this.readyState!==1)throw Error('not open');}
  close(){this.closeCalls++;if(this.readyState<2)this.readyState=2;}
  fire(name,value={}){if(name==='close')this.readyState=3;for(const fn of this.handlers[name]??[])fn(value);}
}
const context=vm.createContext({WebSocket:Socket,ArrayBuffer,Uint8Array,URL,DOMException,performance:{now:()=>clock},location:{origin:cfg.origin,href:cfg.origin+'/'},ipc:{postMessage:s=>messages.push(JSON.parse(s))},btoa:s=>Buffer.from(s,'binary').toString('base64'),setTimeout:fn=>{timers.push(fn);return timers.length;},clearTimeout:id=>{timers[id-1]=null;}});
vm.runInContext(input.script,context);
const url=cfg.origin.replace('https:','wss:')+'/net?host=127.0.0.1&port=1234&b='+cfg.build+'&r=1&u=1';
const make=()=>{const s=new context.WebSocket(url);s.readyState=1;return s;};
const frame=(op,payload,aux=0)=>{const b=new Uint8Array(payload.length+6);b[0]=payload.length&255;b[1]=payload.length>>>8;b[2]=op&255;b[3]=op>>>8;b[4]=aux&255;b[5]=aux>>>8;b.set(payload,6);return b;};
const concat=(...parts)=>{const all=new Uint8Array(parts.reduce((sum,p)=>sum+p.length,0));let at=0;for(const p of parts){all.set(p,at);at+=p.length;}return all;};
const forbidden=Buffer.from('SYNTHETIC-AUTH-NEVER-FORWARD');
const fl=make();fl.send(frame(input.auth,forbidden));fl.fire('message',{data:'k SYNTHETIC-PRIVATE-TOKEN 600'});
const ls=make();ls.send(frame(input.ls,forbidden));ls.fire('message',{data:frame(input.ticket,forbidden)});
assert.equal(messages.filter(m=>m.kind==='frame'||m.kind==='game-login').length,0);
const gs=make();const auth=frame(cfg.login,Buffer.from('uid-ticket-CRC'));
gs.send(auth.subarray(0,1));clock+=40;gs.send(auth.subarray(1));
const notice=messages.find(m=>m.kind==='game-login');assert.ok(notice);assert.equal(notice.header.length,6);assert.equal(notice.header[0],14);assert.equal(notice.ageMs,40);
const profile=frame(cfg.profile,new Uint8Array(69).fill(85));
gs.fire('message',{data:concat(frame(input.ticket,forbidden),frame(cfg.accepted,forbidden),profile)});
const incoming=messages.filter(m=>m.kind==='frame'&&m.direction==='inbound');assert.equal(incoming.length,2);
const accepted=Buffer.from(incoming[0].data,'base64');assert.equal(accepted.length,6);assert.equal(accepted[0],0);
const redacted=Buffer.from(incoming[1].data,'base64').subarray(6);assert.equal(redacted.length,69);
for(let i=0;i<redacted.length;i++)assert.equal(redacted[i],i>=17&&i<=38||[53,54,56,57].includes(i)?85:0);
const inventoryInput=cfg.headerOnly[0];assert.equal(cfg.headerOnly.length,11);assert.ok(cfg.incoming.includes(input.pass)&&cfg.headerOnly.includes(input.pass));
gs.fire('message',{data:frame(inventoryInput,Buffer.from('SYNTHETIC-ITEM-ROWS'),3)});
const headerOnly=Buffer.from(messages.filter(m=>m.kind==='frame'&&m.direction==='inbound').at(-1).data,'base64');
assert.equal(headerOnly.length,6);assert.equal(headerOnly[0],0);assert.equal(headerOnly[1],0);assert.equal(headerOnly[4],3);
gs.fire('message',{data:frame(input.pass,Buffer.from('SYNTHETIC-PASS-PROGRESS'))});
const pass=Buffer.from(messages.filter(m=>m.kind==='frame'&&m.direction==='inbound').at(-1).data,'base64');
assert.equal(pass.length,6);assert.equal(pass.readUInt16LE(0),0);assert.equal(pass.readUInt16LE(2),input.pass);
assert.ok(!JSON.stringify(messages).includes(Buffer.from('SYNTHETIC-ITEM-ROWS').toString('base64')));
assert.ok(!JSON.stringify(messages).includes('SYNTHETIC-AUTH'));
assert.ok(!JSON.stringify(messages).includes('SYNTHETIC-PRIVATE-TOKEN'));
assert.ok(!messages.some(m=>m.data===Buffer.from(auth).toString('base64')));
const before=messages.length;gs.bufferedAmount=20;gs.send(frame(input.move,new Uint8Array(9)));gs.fire('message',{data:frame(input.hp,new Uint8Array(4))});assert.equal(messages.length,before);
clock+=50;gs.bufferedAmount=0;for(const timer of timers.splice(0))if(timer)timer();
assert.deepEqual(messages.slice(before).filter(m=>m.kind==='frame').map(m=>m.direction),['outbound','inbound']);assert.ok(messages.slice(before).filter(m=>m.kind==='frame').every(m=>m.ageMs>=50));
const delayed=frame(input.hp,new Uint8Array([2,0,0,0]));gs.fire('message',{data:delayed.subarray(0,1)});
const previousFrontier=messages.filter(m=>m.kind==='frontier').at(-1).ingress;
gs.send(frame(input.move,new Uint8Array(9)));
assert.equal(messages.filter(m=>m.kind==='frontier').at(-1).ingress,previousFrontier);
const send=messages.filter(m=>m.kind==='frame'&&m.direction==='outbound').at(-1);
gs.fire('message',{data:delayed.subarray(1)});
const late=messages.filter(m=>m.kind==='frame'&&m.direction==='inbound').at(-1);
assert.ok(late.ingress<send.ingress);assert.ok(messages.filter(m=>m.kind==='frontier').at(-1).ingress>=send.ingress);
const unknown=new context.WebSocket('wss://unclassified.example/unknown');
context.__opentricksterNetCloseV1(cfg.epoch,'wrong','9');assert.ok(sockets.every(s=>s.closeCalls===0));
context.__opentricksterNetCloseV1('wrong',cfg.guard,'9');assert.ok(sockets.every(s=>s.closeCalls===0));
context.__opentricksterNetCloseV1(cfg.epoch,cfg.guard,'9');assert.ok(sockets.every(s=>s.closeCalls===1));assert.equal(unknown.readyState,2);
assert.throws(()=>new context.WebSocket(url));assert.throws(()=>new context.WebSocket('wss://other.example'));
for(const socket of sockets)socket.readyState=3;
assert.equal(messages.filter(m=>m.kind==='closed').length,0);
for(const socket of sockets.slice(0,-1))socket.fire('close');assert.equal(messages.filter(m=>m.kind==='closed').length,0);
sockets.at(-1).fire('close');const ack=messages.filter(m=>m.kind==='closed');assert.equal(ack.length,1);assert.equal(ack[0].request,'9');assert.equal(ack[0].sockets,4);assert.equal(ack[0].remaining,0);
context.__opentricksterNetCloseV1(cfg.epoch,cfg.guard,'9');assert.equal(messages.filter(m=>m.kind==='closed').length,2);
assert.ok(messages.every((m,index)=>m.sequence===index+1));
process.stdout.write('PASS page privacy filtering, first-byte age, send drain, all-role actual-close acknowledgement, generation/guard/request fencing\n');
