(() => {
  "use strict";
  const cfg = __OPENTRICKSTER_SETTINGS__;
  const Native = globalThis.WebSocket;
  if (typeof Native !== "function" || globalThis.__opentricksterNetCloseV1) return;
  const records = new Map();
  const bySocket = new WeakMap();
  const nativeSend = Native.prototype.send;
  const requests = new Set();
  const incoming = new Set(cfg.incoming);
  const outgoing = new Set(cfg.outgoing);
  const now = () => performance.now();
  let failed = false, closing = false, sequence = 0, ingress = 0, lastFrontier = 0, next = 0, queue = [], queuedBytes = 0, timer = null;
  function post(kind, value) {
    const body = Object.assign({channel:"opentrickster.net.v1",epoch:cfg.epoch,guard:cfg.guard,sequence:++sequence,kind},value);
    try {globalThis.ipc.postMessage(JSON.stringify(body));}
    catch (_) {failed=true;for(const record of records.values())record.capture=false;}
  }
  function fault(code) {
    if(failed)return;failed=true;
    queue = []; queuedBytes = 0;
    for (const record of records.values()) record.capture = false;
    post("fault",{code});
  }
  function frontier() {
    if(failed || closing)return;
    let through=ingress;
    for(const record of records.values()) {
      for(const stream of [record.rx,record.tx]) {
        const pending=stream?.pending();
        if(pending)through=Math.min(through,pending-1);
      }
    }
    for(const entry of queue)through=Math.min(through,entry.ingress-1);
    if(through>lastFrontier){lastFrontier=through;post("frontier",{ingress:through,ageMs:0});}
  }
  function encode(bytes) {
    let s = "";
    for (let i=0; i<bytes.length; i++) s += String.fromCharCode(bytes[i]);
    return btoa(s);
  }
  function flush() {
    timer = null;
    if (closing) { queue = []; queuedBytes = 0; return; }
    while (queue.length) {
      const entry = queue[0];
      if (entry.direction === "outbound" && entry.record.socket.bufferedAmount !== 0) {
        if (entry.record.closed || entry.record.socket.readyState !== Native.OPEN) { fault("send-uncertain"); return; }
        if (now()-entry.at > 2000) { fault("send-uncertain"); return; }
        timer = setTimeout(flush,16); return;
      }
      queue.shift(); queuedBytes -= entry.bytes.length;
      post("frame",{connection:entry.record.id,direction:entry.direction,ingress:entry.ingress,data:encode(entry.bytes),ageMs:Math.max(0,Math.ceil(now()-entry.at))});
    }
    frontier();
  }
  function frame(record,direction,bytes,at,mark) {
    if (closing || !record.capture) return;
    if (queue.length >= 256 || queuedBytes+bytes.length > 1048576) { fault("capacity"); return; }
    queue.push({record,direction,bytes,at,ingress:mark}); queuedBytes += bytes.length;
    if (timer === null) flush();
  }
  function reader(record,direction) {
    const header = new Uint8Array(6);
    let h = 0, remaining = 0, size = 0, wire = 0, body = null, offset = 0, at = 0, mark = 0, keep = false;
    function finish() {
      if (keep && body) frame(record,direction,body,at,mark);
      h=0; remaining=0; body=null; offset=0; keep=false;
    }
    const push = input => {
      if (!record.capture || closing || typeof input === "string") return;
      let bytes;
      if (input instanceof ArrayBuffer) bytes = new Uint8Array(input);
      else if (ArrayBuffer.isView(input)) bytes = new Uint8Array(input.buffer,input.byteOffset,input.byteLength);
      else { fault("binary-type"); return; }
      if (bytes.length > 1048576) { fault("capacity"); return; }
      let p=0;
      while (p<bytes.length) {
        if (h<6) {
          if (h===0) {at=now();mark=++ingress;if(!Number.isSafeInteger(ingress)){fault("capacity");return;}}
          while (h<6 && p<bytes.length) header[h++]=bytes[p++];
          if (h<6) return;
          size=header[0]+header[1]*256; wire=header[2]+header[3]*256;
          if (size>32745) {fault("malformed");return;}
          remaining=size;offset=0;
          if (direction==="outbound" && wire===cfg.login && size===14 && header[4]===0 && header[5]===0) {
            if (record.selected) {fault("unknown-flow");return;}
            record.selected=true;
            post("game-login",{connection:record.id,ingress:mark,header:Array.from(header),ageMs:Math.max(0,Math.ceil(now()-at))});
          }
          keep=record.selected && (direction==="inbound" ? incoming : outgoing).has(wire);
          if (keep) {
            const headerOnly=direction==="inbound" && (wire===cfg.accepted || wire===cfg.refused || cfg.markers.includes(wire));
            body=new Uint8Array(6+(headerOnly?0:size));body.set(header);
            if (headerOnly) {body[0]=0;body[1]=0;}
          }
          if (remaining===0) {finish();continue;}
        }
        const take=Math.min(remaining,bytes.length-p);
        if (keep && body.length>6) {
          if (direction==="inbound" && wire===cfg.profile) {
            for (let i=0;i<take;i++) {
              const index=offset+i;
              if ((index>=17 && index<=38) || index===53 || index===54 || index===56 || index===57) body[6+index]=bytes[p+i];
            }
          } else body.set(bytes.subarray(p,p+take),6+offset);
        }
        p+=take;offset+=take;remaining-=take;
        if (remaining===0) finish();
      }
      frontier();
    };
    push.pending=()=>h>0?mark:null;
    return push;
  }
  function acknowledge() {
    if (!closing || Array.from(records.values()).some(record=>!record.closed)) return;
    for (const request of requests) post("closed",{request,sockets:records.size,remaining:0});
  }
  function closeRecord(record) {
    if (record.closed) return;
    try {record.socket.close();}
    catch (_) {
      if (record.socket.readyState===Native.CONNECTING) record.socket.addEventListener("open",()=>{if(closing)closeRecord(record);},{once:true});
      else post("close-failed",{code:"socket-close"});
    }
  }
  const ProxySocket = new Proxy(Native,{construct(target,args,newTarget) {
    if (closing) throw new DOMException("Document network is closing","InvalidStateError");
    if (records.size>=64 || Array.from(records.values()).filter(record=>!record.closed).length>=8) {fault("capacity");throw new DOMException("Document network capacity","InvalidStateError");}
    const socket=Reflect.construct(target,args,newTarget);
    const record={id:++next,socket,closed:false,capture:false,selected:false,rx:null,tx:null};
    records.set(record.id,record);
    try {
      const url=new URL(String(args[0]),location.href);
      if(location.origin===cfg.origin && url.protocol==="wss:" && url.host===new URL(cfg.origin).host && url.pathname==="/net") {
        if(url.searchParams.get("b")!==cfg.build)fault("build-changed");
        else if(url.searchParams.has("resume"))fault("unknown-flow");
      }
      record.capture=!failed && location.origin===cfg.origin && url.protocol==="wss:" && url.host===new URL(cfg.origin).host && url.pathname==="/net" && url.searchParams.get("b")===cfg.build && !url.searchParams.has("resume") && url.searchParams.has("host") && url.searchParams.has("port");
    } catch (_) {}
    record.rx=reader(record,"inbound");record.tx=reader(record,"outbound");
    bySocket.set(socket,record);
    socket.addEventListener("message",event=>record.rx(event.data));
    socket.addEventListener("close",()=>{
      record.closed=true;
      post("socket-closed",{connection:record.id});
      acknowledge();
    });
    post("socket",{connection:record.id});
    if(closing)closeRecord(record);
    return socket;
  }});
  Native.prototype.send=function(data) {
    const record=bySocket.get(this);
    const open=this.readyState===Native.OPEN;
    const result=Reflect.apply(nativeSend,this,[data]);
    if(record && open && this.readyState===Native.OPEN) {
      try {record.tx(data);} catch (_) {fault("malformed");}
    }
    return result;
  };
  globalThis.WebSocket=ProxySocket;
  Object.defineProperty(globalThis,"__opentricksterNetCloseV1",{configurable:false,writable:false,value:(epoch,guard,request)=>{
    if(epoch!==cfg.epoch || guard!==cfg.guard || typeof request!=="string" || !/^[0-9a-f]{1,16}$/.test(request) || request==="0")return;
    if(!requests.has(request) && requests.size>=16){post("close-failed",{code:"capacity"});return;}
    requests.add(request);closing=true;queue=[];queuedBytes=0;
    if(timer!==null){clearTimeout(timer);timer=null;}
    for(const record of records.values())closeRecord(record);
    acknowledge();
  }});
})();
