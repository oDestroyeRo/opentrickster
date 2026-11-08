use crate::{
    NetCredentials, NetError, Result, VerifiedProfile,
    codec::{AuxGroups, MAX_BUFFER, MAX_PACKETS, Packet},
    decode::{Decoder, text},
    transport::Gateway,
    wire::{Reader, Writer},
};
use opentrickster_engine::{self as e, Event, Observation, SessionOp};
use std::{
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc as channel, watch};
use zeroize::Zeroizing;

const EVENTS: usize = 256;
struct QueuedOp {
    op: SessionOp,
    at: Option<e::At>,
    queued: Instant,
}
/// Owns one private network actor. Methods never expose Tokio or socket types.
pub struct NativeSession {
    session: e::SessionKey,
    ops: channel::Sender<QueuedOp>,
    cancel: watch::Sender<bool>,
    events: mpsc::Receiver<crate::TimedEvent>,
    done: mpsc::Receiver<()>,
    worker: Option<JoinHandle<()>>,
    fault: Arc<Mutex<Option<NetError>>>,
    readiness: Arc<Mutex<crate::Readiness>>,
    closed_reported: bool,
    cleanup_done: bool,
}
impl NativeSession {
    pub fn open(
        session: e::SessionKey,
        profile: VerifiedProfile,
        credentials: NetCredentials,
    ) -> Result<Self> {
        let (ops, rx) = channel::channel(32);
        let (cancel, cancel_rx) = watch::channel(false);
        let (events, event_rx) = mpsc::sync_channel(EVENTS);
        let (done, done_rx) = mpsc::sync_channel(1);
        let fault = Arc::new(Mutex::new(None));
        let actor_fault = fault.clone();
        let readiness = Arc::new(Mutex::new(crate::Readiness::default()));
        let actor_readiness = readiness.clone();
        let worker = thread::Builder::new()
            .name("opentrickster-native".into())
            .spawn(move || {
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| NetError::Io)
                    .and_then(|rt| {
                        rt.block_on(
                            Actor {
                                profile,
                                credentials,
                                ops: rx,
                                cancel: cancel_rx,
                                events,
                                fault: actor_fault.clone(),
                                readiness: actor_readiness,
                                order: 0,
                                attempt: 0,
                                seen: Vec::new(),
                            }
                            .run(),
                        )
                    });
                if let Err(error) = result
                    && let Ok(mut fault) = actor_fault.lock()
                    && fault.is_none()
                {
                    *fault = Some(error);
                }
                let _ = done.send(());
            })
            .map_err(|_| NetError::Io)?;
        Ok(Self {
            session,
            ops,
            cancel,
            events: event_rx,
            done: done_rx,
            worker: Some(worker),
            fault,
            readiness,
            closed_reported: false,
            cleanup_done: false,
        })
    }
    pub fn submit(&self, op: SessionOp) -> Result<()> {
        if matches!(op, SessionOp::Act(_)) {
            return Err(NetError::WrongPhase);
        }
        self.enqueue(None, op)
    }
    /// Supplies the engine's monotonic clock origin for deadline checks.
    pub fn submit_at(&self, at: e::At, op: SessionOp) -> Result<()> {
        self.enqueue(Some(at), op)
    }
    fn enqueue(&self, at: Option<e::At>, op: SessionOp) -> Result<()> {
        if self.closed_reported
            || *self.cancel.borrow()
            || self.worker.as_ref().is_none_or(|w| w.is_finished())
        {
            return Err(NetError::Closed);
        }
        if matches!(op, SessionOp::Close) {
            self.cancel.send_replace(true);
            return Ok(());
        }
        self.ops
            .try_send(QueuedOp {
                op,
                at,
                queued: Instant::now(),
            })
            .map_err(|error| match error {
                channel::error::TrySendError::Full(_) => NetError::Capacity,
                channel::error::TrySendError::Closed(_) => NetError::Closed,
            })
    }
    pub fn readiness(&self) -> Result<crate::Readiness> {
        self.readiness.lock().map(|r| *r).map_err(|_| NetError::Io)
    }
    pub fn try_next(&mut self) -> Result<Option<Observation>> {
        Ok(self.next_timed()?.map(|event| Observation {
            session: self.session,
            event: event.event,
        }))
    }
    /// Returns the original ingress time in the caller's monotonic clock domain.
    pub fn try_next_at(&mut self, now: e::At) -> Result<Option<(e::At, Observation)>> {
        Ok(self.next_timed()?.map(|event| event.at(now, self.session)))
    }
    fn next_timed(&mut self) -> Result<Option<crate::TimedEvent>> {
        if let Ok(event) = self.events.try_recv() {
            return Ok(Some(event));
        }
        if let Some(error) = self.fault.lock().map_err(|_| NetError::Io)?.take() {
            return Err(error);
        }
        if self.cleanup_done || self.worker.as_ref().is_some_and(|w| w.is_finished()) {
            self.join()?;
            if !self.closed_reported {
                self.closed_reported = true;
                return Ok(Some(crate::TimedEvent {
                    event: Event::Closed,
                    received: Instant::now(),
                }));
            }
        }
        Ok(None)
    }
    pub fn close(&mut self) -> Result<()> {
        self.cancel.send_replace(true);
        if self.worker.is_none() {
            return Ok(());
        }
        self.done
            .recv_timeout(Duration::from_millis(2500))
            .map_err(|_| NetError::Timeout)?;
        self.join()
    }
    fn join(&mut self) -> Result<()> {
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| NetError::Io)?;
        }
        self.cleanup_done = true;
        Ok(())
    }
}
impl Drop for NativeSession {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
    }
}

struct Actor {
    profile: VerifiedProfile,
    credentials: NetCredentials,
    ops: channel::Receiver<QueuedOp>,
    cancel: watch::Receiver<bool>,
    events: mpsc::SyncSender<crate::TimedEvent>,
    fault: Arc<Mutex<Option<NetError>>>,
    readiness: Arc<Mutex<crate::Readiness>>,
    order: u64,
    attempt: u8,
    seen: Vec<e::ActionKey>,
}
impl Actor {
    fn emit(&self, event: Event) -> Result<()> {
        self.emit_at(event, Instant::now())
    }
    fn emit_at(&self, event: Event, received: Instant) -> Result<()> {
        self.events
            .try_send(crate::TimedEvent { event, received })
            .map_err(|_| {
                if let Ok(mut fault) = self.fault.lock() {
                    *fault = Some(NetError::Capacity);
                }
                NetError::Capacity
            })
    }
    async fn run(mut self) -> Result<()> {
        loop {
            if *self.cancel.borrow() {
                return Ok(());
            }
            self.emit(Event::Progress(e::ConnectStep::SigningIn))?;
            let preflight = tokio::select! {
                _=self.cancel.changed()=>return Ok(()),
                result=self.profile.revalidate_online()=>result,
            };
            let result = match preflight {
                Ok(()) => self.attempt().await,
                Err(error) => Err(error),
            };
            *self.readiness.lock().map_err(|_| NetError::Io)? = crate::Readiness::default();
            match result {
                Ok(()) | Err(NetError::Cancelled) => return Ok(()),
                Err(error @ (NetError::Transport | NetError::Timeout | NetError::Uncertain)) => {
                    self.emit(Event::Lost)?;
                    if self.attempt >= 5 {
                        return Err(error);
                    }
                    loop {
                        let queued = tokio::select! {
                            _=self.cancel.changed()=>return Ok(()),
                            queued=self.ops.recv()=>queued.ok_or(NetError::Closed)?,
                        };
                        match queued.op {
                            SessionOp::Reconnect { attempt }
                                if attempt > self.attempt && attempt <= 5 =>
                            {
                                self.attempt = attempt;
                                break;
                            }
                            SessionOp::Act(ticket) => self.emit(Event::Receipt {
                                action: ticket.id,
                                delivery: e::Delivery::NotSent,
                            })?,
                            SessionOp::Close => return Ok(()),
                            _ => {}
                        }
                    }
                }
                Err(error) => {
                    self.emit(Event::Failed(match error {
                        NetError::BuildChanged => e::Halt::BuildChanged,
                        _ => e::Halt::ConnectFailed,
                    }))?;
                    return Err(error);
                }
            }
        }
    }
    async fn open_gateway(&mut self, host: &str, port: u16) -> Result<Gateway> {
        if *self.cancel.borrow() {
            return Err(NetError::Cancelled);
        }
        tokio::select! {_=self.cancel.changed()=>Err(NetError::Cancelled),result=Gateway::open(host,port)=>result}
    }
    async fn attempt(&mut self) -> Result<()> {
        let host = self.profile.first_host.clone();
        let port = self.profile.first_port;
        let mut fl = self.open_gateway(&host, port).await?;
        let result = self.first_login(&mut fl).await;
        fl.close().await;
        let (host, port, ticket) = result?;
        let mut ls = self.open_gateway(&host, port).await?;
        let result = self.login_server(&mut ls, &ticket).await;
        ls.close().await;
        let ticket = result?;
        let mut gs = self.open_gateway(&ticket.host, ticket.port).await?;
        let result = self.game_server(&mut gs, &ticket).await;
        gs.close().await;
        result
    }
    async fn receive(&mut self, gateway: &mut Gateway, deadline: Instant) -> Result<Packet> {
        loop {
            if *self.cancel.borrow() {
                return Err(NetError::Cancelled);
            }
            let packet = tokio::select! {
                _=self.cancel.changed()=>return Err(NetError::Cancelled),
                result=tokio::time::timeout_at(deadline.into(),gateway.receive(&mut self.order))=>result.map_err(|_|NetError::Timeout)??,
            };
            if packet.opcode == 2028 {
                let millis = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| NetError::Io)?
                    .as_millis() as u32;
                gateway
                    .send(2028, &millis.to_le_bytes(), &mut self.order)
                    .await?;
                continue;
            }
            return Ok(packet);
        }
    }
    async fn wait(
        &mut self,
        gateway: &mut Gateway,
        expected: &[u16],
        deadline: Instant,
    ) -> Result<Packet> {
        for _ in 0..MAX_PACKETS {
            let packet = self.receive(gateway, deadline).await?;
            if expected.contains(&packet.opcode) {
                return Ok(packet);
            }
        }
        Err(NetError::Capacity)
    }
    async fn choose(&mut self, gateway: &mut Gateway, world: bool) -> Result<u64> {
        let mut unrelated = 0;
        loop {
            if *self.cancel.borrow() {
                return Err(NetError::Cancelled);
            }
            tokio::select! {
                _=self.cancel.changed()=>return Err(NetError::Cancelled),
                op=self.ops.recv()=>match op.ok_or(NetError::Closed)?.op {
                    SessionOp::ChooseWorld(key) if world=>return Ok(key.0),
                    SessionOp::ChooseCharacter(key) if !world=>return Ok(key.0),
                    SessionOp::Act(ticket)=>self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::NotSent})?,
                    SessionOp::Close=>return Err(NetError::Cancelled),
                    _=>return Err(NetError::WrongPhase),
                },
                result=gateway.receive(&mut self.order)=>{
                    let p=result?;
                    if p.opcode==2028 {
                        let ms=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_|NetError::Io)?.as_millis() as u32;
                        gateway.send(2028,&ms.to_le_bytes(),&mut self.order).await?;
                    } else {
                        if matches!(p.opcode,11503|11508|2001|2013|2|159|2002) {return Err(NetError::Refused);}
                        unrelated+=1;if unrelated>MAX_PACKETS {return Err(NetError::Capacity);}
                    }
                }
            }
        }
    }
    async fn first_login(&mut self, g: &mut Gateway) -> Result<(String, u16, Zeroizing<Vec<u8>>)> {
        let expected = [11502, 11503, 11508];
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut w = Writer::new();
        w.fixed(&self.credentials.id, 19)?;
        w.fixed(&self.profile.marker, 19)?;
        w.u64(self.credentials.bits());
        g.send(11501, &w.0, &mut self.order).await?;
        let packet = self.wait(g, &expected, deadline).await?;
        if packet.opcode == 11503 {
            return Err(refusal(&packet.payload)?);
        }
        if packet.opcode == 11508 {
            return Err(NetError::Refused);
        }
        let mut r = Reader::new(&packet.payload);
        let ticket = Zeroizing::new(r.take(12)?.to_vec());
        let count = r.u8()? as usize;
        let mut worlds = Vec::new();
        let mut choices = Vec::new();
        for n in 0..count {
            let active = r.u8()?;
            let world = r.u16()?;
            let island = r.u16()?;
            let server = r.fixed(33)?;
            let name = r.fixed(33)?;
            r.u16()?;
            r.u16()?;
            if active != 0 {
                let key = e::WorldKey((self.attempt as u64 + 1) * 256 + (n + 1) as u64);
                choices.push(e::WorldChoice {
                    key,
                    label: text(format!("{server} {name}"))?,
                });
                worlds.push((key, world, island));
            }
        }
        r.end()?;
        if choices.is_empty() {
            return Err(NetError::Refused);
        }
        self.emit(Event::Worlds(choices))?;
        let key = self.choose(g, true).await?;
        let (_, world, island) = worlds
            .iter()
            .find(|(k, _, _)| k.0 == key)
            .ok_or(NetError::WrongPhase)?;
        let expected = [11506, 11507];
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut w = Writer::new();
        w.u16(*world);
        w.u16(*island);
        g.send(11505, &w.0, &mut self.order).await?;
        let packet = self.wait(g, &expected, deadline).await?;
        if packet.opcode == 11507 {
            return Err(refusal(&packet.payload)?);
        }
        let mut r = Reader::new(&packet.payload);
        let host = r.fixed(16)?;
        let port = r.u16()?;
        r.end()?;
        crate::profile::bridge(&host, port)?;
        Ok((host, port, ticket))
    }
    async fn login_server(&mut self, g: &mut Gateway, ticket: &[u8]) -> Result<GameTicket> {
        let expected = [2002, 2001];
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut w = Writer::new();
        w.cstr(&self.credentials.id)?;
        w.cstr(&self.credentials.decimal())?;
        w.raw(&self.profile.client_version);
        w.raw(ticket);
        w.u8(20);
        g.send(2000, &w.0, &mut self.order).await?;
        let packet = self.wait(g, &expected, deadline).await?;
        if packet.opcode == 2001 {
            return Err(refusal(&packet.payload)?);
        }
        let list = characters(&packet.payload, self.attempt as u64 + 1)?;
        self.emit(Event::Characters(
            list.iter()
                .map(|c| {
                    Ok(e::CharacterChoice {
                        key: c.key,
                        label: text(c.name.clone())?,
                        busy: c.availability == Availability::Busy,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        ))?;
        let key = self.choose(g, false).await?;
        let character = list
            .iter()
            .find(|c| c.key.0 == key)
            .ok_or(NetError::WrongPhase)?;
        if character.availability == Availability::Busy {
            return Err(NetError::Occupied);
        }
        self.emit(Event::Progress(e::ConnectStep::EnteringWorld))?;
        // Selection consumes both ticket and followup from raw order, before Aux grouping.
        let mut selection = Selection::default();
        let deadline = Instant::now() + Duration::from_secs(30);
        g.send(2012, &character.uid.to_le_bytes(), &mut self.order)
            .await?;
        for _ in 0..MAX_PACKETS {
            let p = self.receive(g, deadline).await?;
            if let Some(ticket) = selection.push(&p)? {
                return Ok(ticket);
            }
        }
        Err(NetError::Capacity)
    }
    async fn game_server(&mut self, g: &mut Gateway, ticket: &GameTicket) -> Result<()> {
        let mut startup = Startup::new(self.attempt as u64 + 1);
        startup
            .decoder
            .set_definitions(self.profile.definitions.clone());
        startup.decoder.use_native_walk_policy();
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut w = Writer::new();
        w.u64(ticket.uid);
        w.u16(ticket.info);
        w.u32(ticket.crc);
        g.send(4, &w.0, &mut self.order).await?;
        while !startup.ready() {
            let p = self.receive(g, deadline).await?;
            startup.push_through(p, g.complete_through(self.order))?;
        }
        let mut decoder = startup.decoder;
        *self.readiness.lock().map_err(|_| NetError::Io)? = decoder.readiness;
        let mut baseline_facts = startup.facts;
        if startup.settled > 0 {
            baseline_facts.push(e::Fact::InventorySettled(e::IngressMark(startup.settled)));
        }
        self.emit_at(
            Event::Baseline {
                support: support(&decoder)?,
                revision: e::EvidenceKey((self.order << 2) | 1),
                facts: baseline_facts,
                resumed: self.attempt > 0,
            },
            startup.received.unwrap_or_else(Instant::now),
        )?;
        let mut aux = startup.aux;
        let mut ordered = startup.ordered;
        let mut dig: Option<PendingDig> = None;
        let mut other_pending = false;
        let mut barrier = None;
        let mut ticks = tokio::time::interval(Duration::from_millis(16));
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            if *self.cancel.borrow() {
                return Err(NetError::Cancelled);
            }
            tokio::select! {
                _=self.cancel.changed()=>return Err(NetError::Cancelled),
                op=self.ops.recv()=>{
                    let queued=op.ok_or(NetError::Closed)?;
                    match queued.op {
                        SessionOp::Act(ticket)=>{
                            if self.seen.contains(&ticket.id) {return Err(NetError::DuplicateAction);}
                            if self.seen.len()>=MAX_PACKETS {return Err(NetError::Capacity);}self.seen.push(ticket.id);
                            let now=queued.at.map(|at|at.0.saturating_add(queued.queued.elapsed().as_millis() as u64));
                            if now.is_none_or(|now|now>=ticket.not_after.0) {
                                self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::NotSent})?;continue;
                            }
                            if dig.is_some() || other_pending {self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::NotSent})?;continue;}
                            let plan:Result<Vec<(u16,Writer)>>=match ticket.action {
                                e::Action::MoveTo(point)=>decoder.move_payload(point).map(|payload|vec![(24,payload)]),
                                e::Action::BasicAttack(target)=>decoder.attack_payloads(target).map(|(selected,attack)|vec![(288,selected),(29,attack)]),
                                e::Action::DigAt(point)=>decoder.dig_inputs().and_then(|inputs| {
                                    let cycle=crate::dig::Cycle::new(inputs,point)?;let (x,y)=crate::actions::coordinate(point)?;
                                    let mut w=Writer::new();w.u16(x);w.u16(y);w.u8(inputs.mode);
                                    dig=Some(PendingDig{cycle,mark:0,not_after:ticket.not_after,clock:queued.at.ok_or(NetError::WrongPhase)?,queued:queued.queued,last_tick:Instant::now()});Ok(vec![(44,w)])
                                }),
                                _=>Err(NetError::Unsupported),
                            };
                            let plan=match plan {Ok(plan)=>plan,Err(_)=>{self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::NotSent})?;continue;}};
                            self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::Submitted})?;
                            let mut last_mark=0;
                            for (opcode,payload) in plan {
                                match g.send(opcode,&payload.0,&mut self.order).await {
                                    Ok(mark)=>last_mark=mark,
                                    Err(error)=>{self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::Unknown})?;return Err(error);}
                                }
                            }
                            if let Some(pending)=&mut dig {pending.mark=last_mark;}else{other_pending=true;}
                            self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::Sent(e::IngressMark(last_mark))})?;
                        }
                        SessionOp::Close=>return Err(NetError::Cancelled),
                        SessionOp::Quiesce(key)=>{
                            barrier=Some(key);
                            if let Some(pending)=&mut dig && let Some((opcode,payload))=pending.cycle.cancel() {g.send(opcode,&payload.0,&mut self.order).await?;}
                        },
                        _=>return Err(NetError::WrongPhase),
                    }
                }
                _=ticks.tick()=>{
                    if let Some(pending)=&mut dig {
                        let now=Instant::now();let dt=now.duration_since(pending.last_tick).as_millis().min(u64::MAX as u128) as u64;pending.last_tick=now;
                        let expired=pending.clock.0.saturating_add(pending.queued.elapsed().as_millis() as u64)>=pending.not_after.0;
                        let message=if expired {pending.cycle.cancel()}else{pending.cycle.tick(dt)?};
                        if let Some((opcode,payload))=message {g.send(opcode,&payload.0,&mut self.order).await?;}
                    }
                }
                result=g.receive(&mut self.order)=>{
                    let raw=result?;let proof_at=raw.received;
                    for packet in aux.push(raw)? {
                        if let Some(pending)=&mut dig && packet.mark>pending.mark {pending.cycle.observe(packet.opcode,&packet.payload)?;}
                        ordered.push(packet)?;
                    }
                    let (ready,settled)=ordered.release(g.complete_through(self.order),aux.oldest_mark())?;
                    for p in ready {
                        let dig_ended=dig.as_ref().is_some_and(|pending|p.mark>pending.mark && p.opcode==50 && pending.cycle.stopped());
                        let facts=decoder.apply(&p)?;
                        *self.readiness.lock().map_err(|_|NetError::Io)?=decoder.readiness;
                        if !facts.is_empty() {self.emit_at(Event::Facts{revision:e::EvidenceKey(p.mark<<2),ingress:e::IngressMark(p.mark),facts},p.received)?;}
                        if dig_ended {dig=None;if let Some(key)=barrier.take() {self.emit_at(Event::Quiescent(key),p.received)?;}}
                    }
                    if let Some(mark)=settled {
                        self.emit_at(Event::Facts{revision:e::EvidenceKey((mark<<2)|2),ingress:e::IngressMark(mark),facts:vec![e::Fact::InventorySettled(e::IngressMark(mark))]},proof_at)?;
                    }
                }
            }
        }
    }
}
struct PendingDig {
    cycle: crate::dig::Cycle,
    mark: u64,
    not_after: e::At,
    clock: e::At,
    queued: Instant,
    last_tick: Instant,
}
pub(crate) fn support(decoder: &Decoder) -> Result<e::Support> {
    Ok(e::Support {
        build: text(crate::BUILD.into())?,
        move_to: decoder.move_payload(e::Point { x: 0, y: 0 }).is_ok(),
        basic_attack: decoder.normal_weapon_mode().is_ok(),
        dig_cycle: decoder
            .dig_inputs()
            .and_then(|inputs| crate::dig::Cycle::new(inputs, e::Point { x: 0, y: 0 }))
            .is_ok(),
        pickup: false,
        hp_potion: false,
        quiesce: false,
    })
}
fn refusal(b: &[u8]) -> Result<NetError> {
    let mut r = Reader::new(b);
    let code = r.u32()?;
    Ok(match code {
        60003 => NetError::InvalidCredentials,
        60013 => NetError::Occupied,
        _ => NetError::Refused,
    })
}
struct Character {
    key: e::CharacterKey,
    uid: u64,
    slot: u8,
    name: String,
    availability: Availability,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Availability {
    Unknown,
    NoPositiveSignal,
    Busy,
}
impl Drop for Character {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.uid.zeroize();
    }
}
fn characters(b: &[u8], epoch: u64) -> Result<Vec<Character>> {
    let mut r = Reader::new(b);
    let count = r.u8()? as usize;
    let mut list = Vec::new();
    for n in 0..count {
        let b = r.take(156)?;
        let mut c = Reader::new(b);
        let uid = c.u64()?;
        let slot = c.u8()?;
        let name = c.fixed(20)?;
        list.push(Character {
            key: e::CharacterKey(epoch * 256 + (n + 1) as u64),
            uid,
            slot,
            name,
            availability: Availability::Unknown,
        });
    }
    let security = r.u32()?;
    let fails = r.u32()?;
    r.cstr()?;
    r.take(2)?;
    r.u8()?;
    if security > 3 || fails > 0 {
        return Err(NetError::SecurityRequired);
    }
    if r.remaining() == 0 {
        return Ok(list);
    }
    let version = r.u8()?;
    if version != 1 {
        return Ok(list);
    }
    let count = r.u8()? as usize;
    if r.remaining() != count * 6 {
        return Err(NetError::Malformed);
    }
    for character in &mut list {
        character.availability = Availability::NoPositiveSignal;
    }
    for _ in 0..count {
        let slot = r.u8()?;
        let kind = r.u8()?;
        r.u32()?;
        if matches!(kind, 1 | 2) {
            let character = list
                .iter_mut()
                .find(|c| c.slot == slot)
                .ok_or(NetError::Malformed)?;
            character.availability = Availability::Busy;
        }
    }
    r.end()?;
    Ok(list)
}
struct GameTicket {
    uid: u64,
    info: u16,
    crc: u32,
    host: String,
    port: u16,
}
impl Drop for GameTicket {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.uid.zeroize();
        self.info.zeroize();
        self.crc.zeroize();
    }
}
fn game_ticket(b: &[u8]) -> Result<GameTicket> {
    let mut r = Reader::new(b);
    let uid = r.u64()?;
    let info = r.u16()?;
    let crc = r.u32()?;
    let host = r.cstr()?;
    let port = u16::try_from(r.u32()?).map_err(|_| NetError::Malformed)?;
    r.take(14)?;
    r.end()?;
    crate::profile::bridge(&host, port)?;
    Ok(GameTicket {
        uid,
        info,
        crc,
        host,
        port,
    })
}
#[derive(Default)]
struct Selection {
    ticket: Option<GameTicket>,
}
impl Selection {
    fn push(&mut self, p: &Packet) -> Result<Option<GameTicket>> {
        match p.opcode {
            2014 => {
                if self.ticket.is_some() {
                    return Err(NetError::Malformed);
                }
                self.ticket = Some(game_ticket(&p.payload)?);
                Ok(None)
            }
            3 => self.ticket.take().map(Some).ok_or(NetError::WrongPhase),
            2013 => Err(refusal(&p.payload)?),
            2 => {
                let mut r = Reader::new(&p.payload);
                r.take(10)?;
                Err(refusal(r.take(4)?)?)
            }
            _ => Ok(None),
        }
    }
}
pub(crate) struct Startup {
    accepted: bool,
    raw: [bool; 3],
    pub(crate) decoder: Decoder,
    pub(crate) aux: AuxGroups,
    pub(crate) facts: Vec<e::Fact>,
    pub(crate) ordered: crate::ordered::OrderedIngress,
    pub(crate) settled: u64,
    bytes: usize,
    pub(crate) received: Option<Instant>,
}
impl Startup {
    pub(crate) fn new(epoch: u64) -> Self {
        Self {
            accepted: false,
            raw: [false; 3],
            decoder: Decoder::with_epoch(epoch),
            aux: AuxGroups::default(),
            facts: Vec::new(),
            ordered: Default::default(),
            settled: 0,
            bytes: 0,
            received: None,
        }
    }
    pub(crate) fn ready(&self) -> bool {
        self.accepted && self.raw.iter().all(|b| *b) && self.decoder.baseline()
    }
    #[cfg(test)]
    pub(crate) fn push(&mut self, p: Packet) -> Result<()> {
        let through = p.mark;
        self.push_through(p, through)
    }
    pub(crate) fn push_through(&mut self, p: Packet, complete_through: u64) -> Result<()> {
        self.received = Some(self.received.map_or(p.received, |at| at.min(p.received)));
        match p.opcode {
            5 => self.accepted = true,
            159 => return Err(NetError::Refused),
            679 => self.raw[0] = true,
            230 => self.raw[1] = true,
            233 => self.raw[2] = true,
            _ => {}
        }
        self.bytes += p.payload.len() + 6;
        if self.bytes > MAX_BUFFER || self.facts.len() > MAX_PACKETS {
            return Err(NetError::Capacity);
        }
        for packet in self.aux.push(p)? {
            self.ordered.push(packet)?;
        }
        self.advance(complete_through)
    }
    pub(crate) fn advance(&mut self, complete_through: u64) -> Result<()> {
        let (ready, settled) = self
            .ordered
            .release(complete_through, self.aux.oldest_mark())?;
        for packet in ready {
            self.facts.extend(self.decoder.apply(&packet)?);
        }
        if let Some(mark) = settled {
            self.settled = mark;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(opcode: u16, payload: Vec<u8>) -> Packet {
        Packet {
            opcode,
            aux: 0,
            mark: {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            },
            received: Instant::now(),
            payload: Zeroizing::new(payload),
        }
    }
    fn ticket() -> Vec<u8> {
        let mut w = Writer::new();
        w.u64(42);
        w.u16(7);
        w.u32(99);
        w.cstr("127.0.0.1").unwrap();
        w.u32(1234);
        w.raw(&[0; 14]);
        w.0.to_vec()
    }
    #[test]
    fn fast_selection_followup_and_all_ticket_truncations() {
        let b = ticket();
        for n in 0..b.len() {
            assert!(game_ticket(&b[..n]).is_err());
        }
        let mut selection = Selection::default();
        assert!(selection.push(&packet(2014, b)).unwrap().is_none());
        assert!(selection.push(&packet(3, vec![])).unwrap().is_some());
        assert!(Selection::default().push(&packet(3, vec![])).is_err());
    }
    #[test]
    fn acceptance_alone_never_establishes_baseline() {
        let mut startup = Startup::new(1);
        startup.push(packet(5, vec![])).unwrap();
        assert!(!startup.ready());
        startup.push(packet(233, vec![0; 251])).unwrap();
        let mut profile = vec![0; 69];
        profile[17] = b'S';
        startup.push(packet(679, profile)).unwrap();
        assert!(!startup.ready());
        startup.push(packet(230, vec![0; 10])).unwrap();
        assert!(startup.ready());
    }
    #[test]
    fn occupied_and_unknown_security_stop() {
        assert_eq!(
            refusal(&60013u32.to_le_bytes()).unwrap(),
            NetError::Occupied
        );
        assert!(characters(&[0], 1).is_err());
    }
    #[test]
    fn loopback_authentication_selection_startup_and_cancellation() {
        use crate::codec::{Frames, OpcodeMap};
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let (cancel, cancel_rx) = watch::channel(false);
            let server = tokio::spawn(async move {
                let (socket, _) = listener.accept().await.unwrap();
                let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
                let mut incoming = Frames::new(b"c2s");
                let outgoing = OpcodeMap::current(b"s2c");
                let mut order = 0;
                for expected in [11501, 11505, 2000, 2012, 4] {
                    let message = ws.next().await.unwrap().unwrap();
                    let Message::Binary(bytes) = message else {
                        panic!("expected binary")
                    };
                    let packets = incoming.push(&bytes, &mut order).unwrap();
                    assert_eq!(packets.len(), 1);
                    assert_eq!(packets[0].opcode, expected);
                    let mut response = Vec::new();
                    match expected {
                        11501 => {
                            assert_eq!(packets[0].payload.len(), 46);
                            let mut w = Writer::new();
                            w.raw(&[0; 12]);
                            w.u8(1);
                            w.u8(1);
                            w.u16(7);
                            w.u16(9);
                            w.fixed("Synthetic server", 33).unwrap();
                            w.fixed("Synthetic world", 33).unwrap();
                            w.u16(100);
                            w.u16(1);
                            response.extend_from_slice(&outgoing.encode(11502, 0, &w.0).unwrap());
                        }
                        11505 => {
                            assert_eq!(packets[0].payload.as_slice(), &[7, 0, 9, 0]);
                            let mut w = Writer::new();
                            w.fixed("127.0.0.1", 16).unwrap();
                            w.u16(1234);
                            response.extend_from_slice(&outgoing.encode(11506, 0, &w.0).unwrap());
                        }
                        2000 => {
                            let mut record = vec![0; 156];
                            record[..8].copy_from_slice(&42u64.to_le_bytes());
                            record[8] = 5;
                            record[9..18].copy_from_slice(b"Synthetic");
                            let mut w = Writer::new();
                            w.u8(1);
                            w.raw(&record);
                            w.u32(3);
                            w.u32(0);
                            w.cstr("").unwrap();
                            w.raw(&[0, 0, 1, 1, 0]);
                            response.extend_from_slice(&outgoing.encode(2002, 0, &w.0).unwrap());
                        }
                        2012 => {
                            response
                                .extend_from_slice(&outgoing.encode(2014, 0, &ticket()).unwrap());
                            response.extend_from_slice(&outgoing.encode(3, 0, &[]).unwrap());
                        }
                        4 => {
                            assert_eq!(packets[0].payload.len(), 14);
                            let mut profile = vec![0; 69];
                            profile[17..26].copy_from_slice(b"Synthetic");
                            let mut clock = Writer::new();
                            clock.u32(1);
                            clock.u16(321);
                            clock.u16(100);
                            clock.u16(200);
                            for (op, payload) in [
                                (5, vec![]),
                                (679, profile),
                                (230, clock.0.to_vec()),
                                (233, vec![0; 251]),
                            ] {
                                response
                                    .extend_from_slice(&outgoing.encode(op, 0, &payload).unwrap());
                            }
                        }
                        _ => unreachable!(),
                    }
                    ws.send(Message::Binary(response.into())).await.unwrap();
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
                cancel.send_replace(true);
                let _ = ws.next().await;
            });
            let (ops, rx) = channel::channel(32);
            let (events, event_rx) = mpsc::sync_channel(256);
            ops.try_send(QueuedOp {
                op: SessionOp::ChooseWorld(e::WorldKey(257)),
                at: None,
                queued: Instant::now(),
            })
            .unwrap();
            ops.try_send(QueuedOp {
                op: SessionOp::ChooseCharacter(e::CharacterKey(257)),
                at: None,
                queued: Instant::now(),
            })
            .unwrap();
            let mut actor = Actor {
                profile: VerifiedProfile {
                    first_host: "127.0.0.1".into(),
                    first_port: 1234,
                    marker: Zeroizing::new("synthetic".into()),
                    client_version: Zeroizing::new(vec![1, 2, 3, 4]),
                    definitions: None,
                },
                credentials: NetCredentials::new("ExampleId".into(), "ExampleOnlyA1".into())
                    .unwrap(),
                ops: rx,
                cancel: cancel_rx,
                events,
                fault: Arc::new(Mutex::new(None)),
                readiness: Arc::new(Mutex::new(crate::Readiness::default())),
                order: 0,
                attempt: 0,
                seen: vec![],
            };
            let mut gateway = Gateway::open_url(&format!("ws://{address}")).await.unwrap();
            let (_, _, ticket) = actor.first_login(&mut gateway).await.unwrap();
            let ticket = actor.login_server(&mut gateway, &ticket).await.unwrap();
            assert_eq!(
                actor.game_server(&mut gateway, &ticket).await,
                Err(NetError::Cancelled)
            );
            gateway.close().await;
            server.await.unwrap();
            let events: Vec<_> = event_rx.try_iter().map(|event| event.event).collect();
            assert!(events.iter().any(|e| matches!(e, Event::Worlds(_))));
            assert!(events.iter().any(|e| matches!(e, Event::Characters(_))));
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::Baseline { .. }))
                    .count(),
                1
            );
        });
    }
}
