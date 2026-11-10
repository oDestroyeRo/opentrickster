'use strict';
// Replays synthetic inbound frames through the generated page bridge on one game socket,
// after its login, and prints every message the bridge posts. The script comes from Rust.
const vm = require('node:vm');
const input = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
const cfg = JSON.parse(input.script.match(/const cfg = (.*);/)[1]);
const messages = [];
class Socket {
  static CONNECTING=0; static OPEN=1; static CLOSING=2; static CLOSED=3;
  constructor(url){this.url=url;this.readyState=0;this.bufferedAmount=0;this.handlers={};}
  addEventListener(name,fn){(this.handlers[name]??=[]).push(fn);}
  send(_){if(this.readyState!==1)throw Error('not open');}
  close(){}
  fire(name,value){for(const fn of this.handlers[name]??[])fn(value);}
}
const context=vm.createContext({WebSocket:Socket,ArrayBuffer,Uint8Array,URL,DOMException,performance:{now:()=>0},location:{origin:cfg.origin,href:cfg.origin+'/'},ipc:{postMessage:s=>messages.push(JSON.parse(s))},btoa:s=>Buffer.from(s,'binary').toString('base64'),setTimeout:()=>0,clearTimeout:()=>{}});
vm.runInContext(input.script,context);
const gs=new context.WebSocket(cfg.origin.replace('https:','wss:')+'/net?host=127.0.0.1&port=1234&b='+cfg.build);
gs.readyState=1;
const login=new Uint8Array(20);login[0]=14;login[2]=cfg.login&255;login[3]=cfg.login>>>8;
gs.send(login);
for(const frame of input.frames)gs.fire('message',{data:new Uint8Array(Buffer.from(frame,'base64'))});
process.stdout.write(JSON.stringify(messages));
