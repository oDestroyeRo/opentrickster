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
type Descriptors = Arc<Mutex<std::collections::BTreeMap<e::ItemKind, crate::ItemDescriptor>>>;
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
    descriptors: Descriptors,
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
        let descriptors = Arc::new(Mutex::new(std::collections::BTreeMap::new()));
        let actor_descriptors = descriptors.clone();
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
                                descriptors: actor_descriptors,
                                order: 0,
                                attempt: 0,
                                last_action: None,
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
            descriptors,
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
    /// Public properties for a received opaque kind; never exposes the server item ID.
    pub fn item_descriptor(&self, kind: e::ItemKind) -> Result<Option<crate::ItemDescriptor>> {
        Ok(self
            .descriptors
            .lock()
            .map_err(|_| NetError::Io)?
            .get(&kind)
            .cloned())
    }
    pub fn item_label(&self, kind: e::ItemKind) -> Result<Option<e::DisplayText>> {
        Ok(self.item_descriptor(kind)?.map(|d| d.label))
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
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            return self.join();
        }
        if self.done.recv_timeout(Duration::from_millis(2500)).is_err()
            && !self
                .worker
                .as_ref()
                .is_some_and(|worker| worker.is_finished())
        {
            return Err(NetError::Timeout);
        }
        self.join()
    }
    fn join(&mut self) -> Result<()> {
        let result = self
            .worker
            .take()
            .map_or(Ok(()), |worker| worker.join().map_err(|_| NetError::Io));
        self.cleanup_done = true;
        result
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
    descriptors: Descriptors,
    order: u64,
    attempt: u8,
    /// Engine action keys strictly increase; anything at or below this one is a replay.
    last_action: Option<e::ActionKey>,
}
impl Actor {
    fn publish_descriptors(&self, decoder: &mut Decoder) -> Result<()> {
        self.descriptors
            .lock()
            .map_err(|_| NetError::Io)?
            .extend(decoder.take_item_descriptors());
        Ok(())
    }
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
        let ticket = match self.login_server(&mut ls, &ticket).await {
            Ok(ticket) => ticket,
            Err(error) => {
                ls.close().await;
                return Err(error);
            }
        };
        let mut login = LoginLink {
            gateway: ls,
            order: 0,
        };
        let result = match self.open_gateway(&ticket.host, ticket.port).await {
            Ok(mut gs) => {
                let result = self.game_server(&mut gs, &mut login, &ticket).await;
                gs.close().await;
                result
            }
            Err(error) => Err(error),
        };
        login.gateway.close().await;
        result
    }
    /// The client answers 2028 only on the login link and keeps nothing else from it in game.
    async fn login_frame(&mut self, login: &mut LoginLink, packet: Packet) -> Result<()> {
        if packet.opcode == 2028 {
            login
                .gateway
                .send(2028, &ping_millis()?, &mut login.order)
                .await?;
        }
        Ok(())
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
                gateway.send(2028, &ping_millis()?, &mut self.order).await?;
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
                        gateway.send(2028,&ping_millis()?,&mut self.order).await?;
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
    async fn game_server(
        &mut self,
        g: &mut Gateway,
        login: &mut LoginLink,
        ticket: &GameTicket,
    ) -> Result<()> {
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
            if *self.cancel.borrow() {
                return Err(NetError::Cancelled);
            }
            let p = tokio::select! {
                _=self.cancel.changed()=>return Err(NetError::Cancelled),
                result=tokio::time::timeout_at(deadline.into(),g.receive(&mut self.order))=>result.map_err(|_|NetError::Timeout)??,
                result=login.gateway.receive(&mut login.order)=>{self.login_frame(login,result?).await?;continue;}
            };
            // The client registers no 2028 handler on the game link.
            if p.opcode == 2028 {
                continue;
            }
            startup.push_through(p, g.complete_through(self.order))?;
        }
        g.send(5, &[], &mut self.order).await?;
        let mut decoder = startup.decoder;
        self.publish_descriptors(&mut decoder)?;
        *self.readiness.lock().map_err(|_| NetError::Io)? = decoder.readiness;
        let mut baseline_facts = startup.facts;
        if startup.settled > 0 {
            baseline_facts.push(e::Fact::InventorySettled(e::IngressMark(startup.settled)));
        }
        self.emit_at(
            Event::Baseline {
                support: support(&decoder, true)?,
                revision: e::EvidenceKey((self.order << 2) | 1),
                facts: baseline_facts,
                resumed: self.attempt > 0,
            },
            startup.received.unwrap_or_else(Instant::now),
        )?;
        let mut aux = startup.aux;
        let mut ordered = startup.ordered;
        let mut dig: Option<PendingDig> = None;
        let mut barrier: Option<e::BarrierKey> = None;
        let mut action_until: Option<Instant> = None;
        // An action that expired during a drain leaves idle unproven for the rest of this session.
        let mut lost = false;
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
                            if self.last_action.is_some_and(|last|ticket.id<=last) {return Err(NetError::DuplicateAction);}
                            self.last_action=Some(ticket.id);
                            let now=queued.at.map(|at|at.0.saturating_add(queued.queued.elapsed().as_millis() as u64));
                            if now.is_none_or(|now|now>=ticket.not_after.0) {
                                self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::NotSent})?;continue;
                            }
                            if dig.is_some() || decoder.action_pending() || barrier.is_some() {self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::NotSent})?;continue;}
                            let plan:Result<Vec<(u16,Writer)>>=match ticket.action {
                                e::Action::MoveTo(point)=>decoder.move_payload(point).map(|payload|vec![(24,payload)]),
                                e::Action::BasicAttack(target)=>decoder.attack_payloads(target).map(|(selected,attack)|vec![(288,selected),(29,attack)]),
                                e::Action::DigAt(point)=>decoder.dig_inputs().and_then(|inputs| {
                                    let cycle=crate::dig::Cycle::new(inputs,point)?;let (x,y)=crate::actions::coordinate(point)?;
                                    let mut w=Writer::new();w.u16(x);w.u16(y);w.u8(inputs.mode);
                                    dig=Some(PendingDig{cycle,mark:0,until:Instant::now()+Duration::from_millis(e::DIG_MS),last_tick:Instant::now()});Ok(vec![(44,w)])
                                }),
                                _=>Err(NetError::Unsupported),
                            };
                            let plan=match plan {Ok(plan)=>plan,Err(_)=>{self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::NotSent})?;continue;}};
                            if dig.is_none(){decoder.expect_action(ticket.action,self.order.checked_add(plan.len() as u64).ok_or(NetError::Capacity)?)?;}
                            self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::Submitted})?;
                            let mut last_mark=0;
                            for (opcode,payload) in plan {
                                match g.send(opcode,&payload.0,&mut self.order).await {
                                    Ok(mark)=>last_mark=mark,
                                    Err(error)=>{self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::Unknown})?;return Err(error);}
                                }
                            }
                            // The engine's completion window starts at its Sent receipt, so ours ends no later.
                            let sent=Instant::now();
                            match &mut dig {Some(pending)=>{pending.mark=last_mark;pending.until=sent+Duration::from_millis(e::DIG_MS);action_until=None;}None=>action_until=Some(sent+Duration::from_millis(e::ACTION_MS))}
                            self.emit(Event::Receipt{action:ticket.id,delivery:e::Delivery::Sent(e::IngressMark(last_mark))})?;
                        }
                        SessionOp::Close=>return Err(NetError::Cancelled),
                        SessionOp::Quiesce(key)=>{
                            // Report idle only once nothing is outstanding; otherwise the engine's drain deadline closes the session.
                            barrier=Some(key);
                            if let Some(pending)=&mut dig && let Some((opcode,payload))=pending.cycle.cancel() {g.send(opcode,&payload.0,&mut self.order).await?;}
                            if !lost && dig.is_none() && !decoder.action_pending() && ordered.is_empty() && aux.oldest_mark().is_none() && g.complete_through(self.order)==self.order {self.emit(Event::Quiescent(key))?;barrier=None;}
                        },
                        _=>return Err(NetError::WrongPhase),
                    }
                }
                _=ticks.tick()=>{
                    let now=Instant::now();
                    if let Some(pending)=&mut dig {
                        let dt=now.duration_since(pending.last_tick).as_millis().min(u64::MAX as u128) as u64;pending.last_tick=now;
                        let expired=now>=pending.until;
                        let message=if expired {pending.cycle.cancel()}else{pending.cycle.tick(dt)?};
                        if let Some((opcode,payload))=message {g.send(opcode,&payload.0,&mut self.order).await?;}
                        // No cancellation-ack schema exists: after requesting 48, stop the local simulation.
                        if expired {lost|=barrier.is_some();dig=None;}
                    }
                    if action_until.is_some_and(|until|now>=until) {lost|=barrier.is_some()&&decoder.action_pending();decoder.expire_action();action_until=None;}
                    if let Some(key)=barrier && !lost && dig.is_none() && !decoder.action_pending() && ordered.is_empty() && aux.oldest_mark().is_none() && g.complete_through(self.order)==self.order {self.emit(Event::Quiescent(key))?;barrier=None;}
                }
                result=login.gateway.receive(&mut login.order)=>self.login_frame(login,result?).await?,
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
                        if decoder.physical_uncertain(){return Err(NetError::Uncertain);}
                        self.publish_descriptors(&mut decoder)?;
                        *self.readiness.lock().map_err(|_|NetError::Io)?=decoder.readiness;
                        if !facts.is_empty() {self.emit_at(Event::Facts{revision:e::EvidenceKey(p.mark<<2),ingress:e::IngressMark(p.mark),facts},p.received)?;}
                        let mut correlated=std::collections::BTreeMap::<u64,(Instant,Vec<e::Fact>)>::new();
                        for result in decoder.take_correlated(){correlated.entry(result.mark).or_insert_with(||(result.received,Vec::new())).1.extend(result.facts);}
                        for (mark,(received,facts)) in correlated {self.emit_at(Event::Facts{revision:e::EvidenceKey((mark<<2)|3),ingress:e::IngressMark(mark),facts},received)?;}
                        if dig_ended {dig=None;}
                        if !lost && dig.is_none() && !decoder.action_pending() && ordered.is_empty() && aux.oldest_mark().is_none() && g.complete_through(self.order)==self.order && let Some(key)=barrier.take() {self.emit_at(Event::Quiescent(key),p.received)?;}
                    }
                    if let Some(mark)=settled {
                        let mut facts=vec![e::Fact::InventorySettled(e::IngressMark(mark))];
                        facts.extend(decoder.settle_carried(mark));
                        self.emit_at(Event::Facts{revision:e::EvidenceKey((mark<<2)|2),ingress:e::IngressMark(mark),facts},proof_at)?;
                    }
                }
            }
        }
    }
}
/// The login-server link, kept open and pumped until the game link closes, as the client does.
/// Its own frame order keeps login frames out of the game link's marks.
struct LoginLink {
    gateway: Gateway,
    order: u64,
}
fn ping_millis() -> Result<[u8; 4]> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| NetError::Io)?
        .as_millis() as u32;
    Ok(millis.to_le_bytes())
}
struct PendingDig {
    cycle: crate::dig::Cycle,
    mark: u64,
    until: Instant,
    last_tick: Instant,
}
pub(crate) fn support(decoder: &Decoder, quiesce: bool) -> Result<e::Support> {
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
        quiesce,
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
    // Only state 0 has a recovered client path; 1-3 need unsupported lock handling.
    if security != 0 || fails > 0 {
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
    /// Last raw receipt among the first accepted 5, 679, 230 and 233: the cutoff H.
    cutoff: u64,
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
            cutoff: 0,
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
        let first = match p.opcode {
            5 => !std::mem::replace(&mut self.accepted, true),
            159 => return Err(NetError::Refused),
            679 => !std::mem::replace(&mut self.raw[0], true),
            230 => !std::mem::replace(&mut self.raw[1], true),
            233 => !std::mem::replace(&mut self.raw[2], true),
            _ => false,
        };
        if first {
            self.cutoff = self.cutoff.max(p.mark);
            if self.accepted && self.raw.iter().all(|b| *b) {
                self.decoder.startup_cutoff(self.cutoff);
            }
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
            for deferred in self.decoder.take_correlated() {
                self.facts.extend(deferred.facts);
            }
        }
        if let Some(mark) = settled {
            self.settled = mark;
            self.facts.extend(self.decoder.settle_carried(mark));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_second_password_state_zero_proceeds() {
        let list = |state: u32, fails: u32| {
            let mut record = vec![0; 156];
            record[..8].copy_from_slice(&42u64.to_le_bytes());
            record[8] = 5;
            record[9..18].copy_from_slice(b"Synthetic");
            let mut w = Writer::new();
            w.u8(1);
            w.raw(&record);
            w.u32(state);
            w.u32(fails);
            w.cstr("").unwrap();
            w.raw(&[0, 0, 1]);
            w.0
        };
        assert_eq!(characters(&list(0, 0), 1).map(|c| c.len()), Ok(1));
        for state in [1, 2, 3, 4] {
            assert_eq!(
                characters(&list(state, 0), 1).map(|c| c.len()),
                Err(NetError::SecurityRequired)
            );
        }
        assert_eq!(
            characters(&list(0, 1), 1).map(|c| c.len()),
            Err(NetError::SecurityRequired)
        );
    }
    fn cleanup_fixture(worker: JoinHandle<()>, done: mpsc::Receiver<()>) -> NativeSession {
        let mut engine = e::Engine::new(e::Mode::Clientless);
        let opened = engine.handle(e::Input::User {
            at: e::At(0),
            request: e::RequestKey(1),
            command: e::Command::Open,
        });
        let session = opened
            .io
            .iter()
            .find_map(|effect| match effect {
                e::Effect::Open { session, .. } => Some(*session),
                _ => None,
            })
            .unwrap();
        let (ops, _) = channel::channel(1);
        let (cancel, _) = watch::channel(false);
        let (_, events) = mpsc::sync_channel(1);
        NativeSession {
            session,
            ops,
            cancel,
            events,
            done,
            worker: Some(worker),
            fault: Arc::new(Mutex::new(None)),
            readiness: Arc::new(Mutex::new(crate::Readiness::default())),
            descriptors: Arc::new(Mutex::new(std::collections::BTreeMap::new())),
            closed_reported: false,
            cleanup_done: false,
        }
    }
    fn finished(worker: &JoinHandle<()>) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !worker.is_finished() {
            assert!(Instant::now() < deadline, "fixture worker must finish");
            thread::yield_now();
        }
    }
    #[test]
    fn close_joins_finished_workers_without_a_completion_signal_including_panic() {
        for panics in [false, true] {
            let (done, rx) = mpsc::sync_channel(1);
            let worker = thread::spawn(move || {
                drop(done);
                assert!(!panics, "synthetic worker termination");
            });
            finished(&worker);
            let mut session = cleanup_fixture(worker, rx);
            assert_eq!(
                session.close(),
                if panics { Err(NetError::Io) } else { Ok(()) }
            );
            assert!(session.worker.is_none());
            assert!(session.cleanup_done);
            session.close().unwrap();
            assert!(matches!(
                session.try_next().unwrap().map(|o| o.event),
                Some(Event::Closed)
            ));
            assert!(session.try_next().unwrap().is_none());
        }
    }
    #[test]
    fn a_close_timeout_retains_the_worker_until_actual_exit_and_join() {
        let (release, wait) = mpsc::sync_channel(1);
        let (done, rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            wait.recv().unwrap();
            let _ = done.send(());
        });
        let mut session = cleanup_fixture(worker, rx);
        assert_eq!(session.close(), Err(NetError::Timeout));
        assert!(session.worker.is_some());
        assert!(!session.cleanup_done);
        assert!(session.try_next().unwrap().is_none());
        release.send(()).unwrap();
        session.close().unwrap();
        assert!(session.worker.is_none());
        assert!(matches!(
            session.try_next().unwrap().map(|o| o.event),
            Some(Event::Closed)
        ));
    }
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
        /// The next frame other than the gateway's probe and ack text.
        async fn frame(
            ws: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        ) -> Message {
            loop {
                match ws.next().await.unwrap().unwrap() {
                    Message::Text(text) if text == "p" || text.starts_with("a ") => {}
                    message => return message,
                }
            }
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let retained = NetCredentials::new("ExampleId".into(), "ExampleOnlyA1".into()).unwrap();
            for _ in 0..2 {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let address = listener.local_addr().unwrap();
                let (cancel, cancel_rx) = watch::channel(false);
                let server = tokio::spawn(async move {
                    let (socket, _) = listener.accept().await.unwrap();
                    let mut login = tokio_tungstenite::accept_async(socket).await.unwrap();
                    let mut game = None;
                    let mut login_in = Frames::new(b"c2s");
                    let mut game_in = Frames::new(b"c2s");
                    let outgoing = OpcodeMap::current(b"s2c");
                    let mut order = 0;
                    let ping = |n: u32| outgoing.encode(2028, 0, &n.to_le_bytes()).unwrap();
                    for expected in [11501, 11505, 2000, 2012, 4] {
                        if expected == 4 {
                            let (socket, _) = listener.accept().await.unwrap();
                            game = Some(tokio_tungstenite::accept_async(socket).await.unwrap());
                        }
                        let (ws, incoming) = match game.as_mut() {
                            Some(game) => (game, &mut game_in),
                            None => (&mut login, &mut login_in),
                        };
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
                                response
                                    .extend_from_slice(&outgoing.encode(11502, 0, &w.0).unwrap());
                            }
                            11505 => {
                                assert_eq!(packets[0].payload.as_slice(), &[7, 0, 9, 0]);
                                let mut w = Writer::new();
                                w.fixed("127.0.0.1", 16).unwrap();
                                w.u16(1234);
                                response
                                    .extend_from_slice(&outgoing.encode(11506, 0, &w.0).unwrap());
                            }
                            2000 => {
                                let mut record = vec![0; 156];
                                record[..8].copy_from_slice(&42u64.to_le_bytes());
                                record[8] = 5;
                                record[9..18].copy_from_slice(b"Synthetic");
                                let mut w = Writer::new();
                                w.u8(1);
                                w.raw(&record);
                                w.u32(0);
                                w.u32(0);
                                w.cstr("").unwrap();
                                w.raw(&[0, 0, 1, 1, 0]);
                                response
                                    .extend_from_slice(&outgoing.encode(2002, 0, &w.0).unwrap());
                            }
                            2012 => {
                                response.extend_from_slice(
                                    &outgoing.encode(2014, 0, &ticket()).unwrap(),
                                );
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
                                // A game-link 2028 during startup gets no reply.
                                response.extend_from_slice(&ping(1));
                                for (op, payload) in [
                                    (5, vec![]),
                                    (679, profile),
                                    (230, clock.0.to_vec()),
                                    (233, vec![0; 251]),
                                ] {
                                    response.extend_from_slice(
                                        &outgoing.encode(op, 0, &payload).unwrap(),
                                    );
                                }
                            }
                            _ => unreachable!(),
                        }
                        ws.send(Message::Binary(response.into())).await.unwrap();
                    }
                    let mut game = game.unwrap();
                    let Message::Binary(bytes) =
                        tokio::time::timeout(Duration::from_secs(1), frame(&mut game))
                            .await
                            .expect("native must announce world entry after startup")
                    else {
                        panic!("expected world-entry frame")
                    };
                    let packets = game_in.push(&bytes, &mut order).unwrap();
                    assert_eq!(packets.len(), 1);
                    assert_eq!(packets[0].opcode, 5);
                    assert!(packets[0].payload.is_empty());
                    // After world entry, the login link is still open and alone answers 2028.
                    game.send(Message::Binary(ping(2).to_vec().into()))
                        .await
                        .unwrap();
                    login
                        .send(Message::Binary(ping(3).to_vec().into()))
                        .await
                        .unwrap();
                    let Message::Binary(bytes) = frame(&mut login).await else {
                        panic!("the login link must stay open through the game session")
                    };
                    let packets = login_in.push(&bytes, &mut order).unwrap();
                    assert_eq!(packets.len(), 1);
                    assert_eq!(packets[0].opcode, 2028);
                    assert_eq!(packets[0].payload.len(), 4);
                    assert!(
                        tokio::time::timeout(Duration::from_millis(300), frame(&mut game))
                            .await
                            .is_err(),
                        "the game link never answers 2028"
                    );
                    cancel.send_replace(true);
                    assert!(matches!(frame(&mut game).await, Message::Close(_)));
                    assert!(matches!(frame(&mut login).await, Message::Close(_)));
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
                    credentials: retained.clone(),
                    ops: rx,
                    cancel: cancel_rx,
                    events,
                    fault: Arc::new(Mutex::new(None)),
                    readiness: Arc::new(Mutex::new(crate::Readiness::default())),
                    descriptors: Arc::new(Mutex::new(std::collections::BTreeMap::new())),
                    order: 0,
                    attempt: 0,
                    last_action: None,
                };
                let mut gateway = Gateway::open_url(&format!("ws://{address}")).await.unwrap();
                let (_, _, ticket) = actor.first_login(&mut gateway).await.unwrap();
                let ticket = actor.login_server(&mut gateway, &ticket).await.unwrap();
                let mut login = LoginLink { gateway, order: 0 };
                let mut game = Gateway::open_url(&format!("ws://{address}")).await.unwrap();
                assert_eq!(
                    actor.game_server(&mut game, &mut login, &ticket).await,
                    Err(NetError::Cancelled)
                );
                game.close().await;
                login.gateway.close().await;
                drop(actor);
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
            }
        });
    }

    mod carried {
        use super::*;
        use crate::test_support::{Def, definitions};
        use e::Fact;

        const H: u64 = 5;
        fn at(mark: u64, opcode: u16, aux: u16, payload: Vec<u8>) -> Packet {
            Packet {
                opcode,
                aux,
                mark,
                received: Instant::now(),
                payload: Zeroizing::new(payload),
            }
        }
        fn body(kind: u8, id: u32, uid: u64, count: u16) -> Vec<u8> {
            let mut b = vec![kind];
            b.extend(id.to_le_bytes());
            b.extend(uid.to_le_bytes());
            b.extend(count.to_le_bytes());
            if kind != 1 {
                b.extend([0; 4]);
            }
            b
        }
        fn list(rows: &[(u8, u32, u64, u16)]) -> Vec<u8> {
            let mut b = (rows.len() as u16).to_le_bytes().to_vec();
            for &(kind, id, uid, count) in rows {
                b.extend(body(kind, id, uid, count));
                if kind == 2 {
                    b.push(0);
                }
            }
            b
        }
        fn change(op: u8, kind: u8, id: u32, uid: u64, count: u16) -> Vec<u8> {
            let mut b = vec![op];
            b.extend(body(kind, id, uid, count));
            if matches!(op, 2 | 4) && kind == 2 {
                b.push(0);
            }
            b
        }
        fn totals(rows: &[(u8, i32)]) -> Vec<u8> {
            let mut b = vec![rows.len() as u8];
            for &(stat, value) in rows {
                b.push(stat);
                b.extend(value.to_le_bytes());
            }
            b
        }
        /// Level 10, HP 80, zone 3 and the Kind-2 UID 42 equipped in slot 3.
        fn stats() -> Vec<u8> {
            let mut b = vec![0; 251];
            b[24..32].copy_from_slice(&42u64.to_le_bytes());
            b[214..216].copy_from_slice(&10u16.to_le_bytes());
            b[219..221].copy_from_slice(&3u16.to_le_bytes());
            b[235..239].copy_from_slice(&80u32.to_le_bytes());
            b
        }
        fn profile() -> Vec<u8> {
            let mut b = vec![0; 69];
            b[17] = b'S';
            b
        }
        fn clock() -> Vec<u8> {
            let mut w = Writer::new();
            w.u32(1);
            w.u16(321);
            w.u16(100);
            w.u16(200);
            w.0.to_vec()
        }
        /// 4x3 + Kind-2 (one piece) 10 + 5x2 potions + 2x1 expiring potions; Kind 3 is not cached.
        const ROWS: [(u8, u32, u64, u16); 5] = [
            (1, 7, 41, 4),
            (2, 9, 42, 300),
            (1, 8, 43, 5),
            (1, 10, 44, 2),
            (3, 9, 45, 1),
        ];
        fn startup(accepted: bool, rows: &[(u8, u32, u64, u16)]) -> Startup {
            startup_with(Some(accepted), rows)
        }
        fn startup_with(accepted: Option<bool>, rows: &[(u8, u32, u64, u16)]) -> Startup {
            let mut startup = Startup::new(1);
            if let Some(accepted) = accepted {
                startup.decoder.set_inventory_cutoff(accepted);
            }
            startup.decoder.set_definitions(Some(definitions(&[
                (7, Def::Plain { weight: 3 }),
                (8, Def::HpPotion { weight: 2 }),
                (9, Def::Plain { weight: 10 }),
                (10, Def::ExpiringHpPotion { weight: 1 }),
            ])));
            startup.decoder.use_native_walk_policy();
            for (mark, opcode, payload) in [
                (1, 5, vec![]),
                (2, 679, profile()),
                (3, 230, clock()),
                (4, 231, list(rows)),
                (H, 233, stats()),
            ] {
                startup.push(at(mark, opcode, 0, payload)).unwrap();
            }
            assert!(startup.ready());
            startup
        }
        fn carried(facts: &[Fact]) -> Vec<(u32, Option<u32>, u32)> {
            let weights = facts.iter().filter_map(|fact| match fact {
                Fact::Weight(gauge) => Some((gauge.current, gauge.maximum.map(|m| m.get()))),
                _ => None,
            });
            let potions = facts.iter().filter_map(|fact| match fact {
                Fact::HpPotions(count) => Some(*count),
                _ => None,
            });
            let pairs: Vec<_> = weights
                .zip(potions)
                .map(|((weight, maximum), potions)| (weight, maximum, potions))
                .collect();
            let reported = facts
                .iter()
                .filter(|fact| matches!(fact, Fact::Weight(_) | Fact::HpPotions(_)))
                .count();
            assert_eq!(
                reported,
                pairs.len() * 2,
                "weight and potions travel together"
            );
            pairs
        }
        fn apply(decoder: &mut Decoder, p: Packet) -> Vec<(u32, Option<u32>, u32)> {
            carried(&decoder.apply(&p).unwrap())
        }

        #[test]
        fn the_pinned_build_reports_carried_weight_by_default() {
            let startup = startup_with(None, &ROWS);
            assert!(!carried(&startup.facts).is_empty());
        }

        #[test]
        fn an_unaccepted_cutoff_reports_neither_weight_nor_potions() {
            let mut startup = startup(false, &ROWS);
            assert!(carried(&startup.facts).is_empty());
            let decoder = &mut startup.decoder;
            assert!(decoder.settle_carried(100).is_empty());
            for (mark, opcode, payload) in [
                (6, 282, totals(&[(6, 100)])),
                (7, 561, change(1, 1, 7, 41, 1)),
                (8, 231, list(&ROWS)),
                (9, 35, vec![]),
            ] {
                assert!(apply(decoder, at(mark, opcode, 0, payload)).is_empty());
            }
        }

        #[test]
        fn the_settled_cutoff_reports_exact_weight_and_usable_potions() {
            let mut startup = startup(true, &ROWS);
            // Kind 1 multiplies Count; Kind 2 is one piece; Kind 3 and the expiring
            // stack contribute nothing to the usable count.
            assert_eq!(carried(&startup.facts), [(34, None, 5)]);
            let decoder = &mut startup.decoder;
            assert_eq!(
                apply(decoder, at(6, 282, 0, totals(&[(6, 100)]))),
                [(34, Some(100), 5)]
            );
            assert!(apply(decoder, at(7, 282, 0, totals(&[(9, 100)]))).is_empty());
            // Equipping changes slots and clears totals, never carried weight.
            let mut equip = 41u64.to_le_bytes().to_vec();
            equip.push(4);
            assert_eq!(apply(decoder, at(8, 82, 0, equip)), [(34, None, 5)]);
            assert_eq!(
                apply(decoder, at(9, 282, 0, totals(&[(6, 30)]))),
                [(34, None, 5)],
                "an exceeded maximum stays unknown"
            );
        }

        #[test]
        fn each_applied_delta_recomputes_weight_and_potions() {
            let mut startup = startup(true, &ROWS);
            let decoder = &mut startup.decoder;
            for (mark, payload, expected) in [
                (6, change(1, 1, 7, 41, 1), (25, None, 5)),
                (7, change(2, 1, 8, 46, 3), (31, None, 8)),
                (8, change(3, 1, 8, 43, 0), (21, None, 3)),
                (9, change(2, 2, 9, 47, 900), (31, None, 3)),
            ] {
                assert_eq!(apply(decoder, at(mark, 561, 0, payload)), [expected]);
            }
        }

        #[test]
        fn late_lists_and_unsupported_inputs_invalidate_reported_values() {
            for (opcode, payload) in [
                (231, list(&[])),
                (35, vec![]),
                (561, change(1, 1, 7, 999, 1)),
                (561, change(2, 1, 99, 48, 1)),
                // 68 only updates an existing row, so a new UID diverges from the client.
                (68, change(2, 1, 7, 99, 1)),
            ] {
                let mut startup = startup(true, &ROWS);
                assert_eq!(
                    startup.decoder.apply(&at(6, opcode, 0, payload)).err(),
                    Some(NetError::Uncertain)
                );
            }
        }

        #[test]
        fn a_late_list_before_settlement_leaves_the_cutoff_unknown() {
            let mut startup = Startup::new(1);
            startup.decoder.set_inventory_cutoff(true);
            startup
                .decoder
                .set_definitions(Some(definitions(&[(7, Def::Plain { weight: 3 })])));
            for (mark, opcode, aux, payload) in [
                (1, 5, 0, vec![]),
                (2, 679, 0, profile()),
                (3, 230, 0, clock()),
                (4, 231, 0, list(&[(1, 7, 41, 4)])),
                (H, 233, 9, stats()),
                (6, 231, 0, list(&[])),
                (7, 1951, 9, vec![]),
            ] {
                startup.push(at(mark, opcode, aux, payload)).unwrap();
            }
            assert!(startup.ready());
            assert_eq!(startup.settled, 7);
            assert!(carried(&startup.facts).is_empty());
            assert!(apply(&mut startup.decoder, at(8, 561, 0, change(1, 1, 7, 41, 1))).is_empty());
        }

        #[test]
        fn delivery_must_settle_through_the_cutoff() {
            let mut startup = Startup::new(1);
            startup.decoder.set_inventory_cutoff(true);
            startup
                .decoder
                .set_definitions(Some(definitions(&[(7, Def::Plain { weight: 3 })])));
            // A partial frame keeps the transport frontier one receipt behind.
            for (mark, opcode, payload) in [
                (1, 5, vec![]),
                (2, 679, profile()),
                (3, 230, clock()),
                (4, 231, list(&[(1, 7, 41, 4)])),
                (H, 233, stats()),
            ] {
                startup
                    .push_through(at(mark, opcode, 0, payload), mark - 1)
                    .unwrap();
            }
            assert_eq!(startup.settled, H - 1);
            assert!(carried(&startup.facts).is_empty());
            startup.advance(H).unwrap();
            assert_eq!(carried(&startup.facts), [(12, None, 0)]);
        }

        #[test]
        fn a_missing_definition_reports_nothing() {
            let mut startup = startup(true, &[(1, 7, 41, 4), (1, 99, 48, 1)]);
            assert!(carried(&startup.facts).is_empty());
            let decoder = &mut startup.decoder;
            assert!(apply(decoder, at(6, 561, 0, change(3, 1, 99, 48, 0))).is_empty());
            assert!(apply(decoder, at(7, 282, 0, totals(&[(6, 100)]))).is_empty());
        }

        #[test]
        fn only_a_settled_cutoff_completes_an_explicit_empty_list() {
            let mut startup = Startup::new(1);
            startup.decoder.set_inventory_cutoff(true);
            startup.decoder.set_definitions(Some(definitions(&[])));
            for (mark, opcode, aux, payload) in [
                (1, 5, 0, vec![]),
                (2, 679, 0, profile()),
                (3, 230, 0, clock()),
                (4, 231, 0, list(&[])),
                (H, 233, 9, stats()),
            ] {
                startup.push(at(mark, opcode, aux, payload)).unwrap();
            }
            assert!(startup.decoder.readiness.inventory_list_seen);
            assert!(
                carried(&startup.facts).is_empty(),
                "a zero-count list alone"
            );
            startup.push(at(6, 1951, 9, vec![])).unwrap();
            assert_eq!(carried(&startup.facts), [(0, None, 0)]);
            // Without any list at or before H the bag is missing, not empty.
            let mut startup = Startup::new(1);
            startup.decoder.set_inventory_cutoff(true);
            startup.decoder.set_definitions(Some(definitions(&[])));
            for (mark, opcode, payload) in [
                (1, 5, vec![]),
                (2, 679, profile()),
                (3, 230, clock()),
                (4, 233, stats()),
            ] {
                startup.push(at(mark, opcode, 0, payload)).unwrap();
            }
            assert!(carried(&startup.facts).is_empty());
            assert!(apply(&mut startup.decoder, at(5, 231, 0, list(&[]))).is_empty());
        }

        #[test]
        fn a_delta_before_the_cutoff_is_outside_the_replayed_cache() {
            let mut startup = Startup::new(1);
            startup.decoder.set_inventory_cutoff(true);
            startup
                .decoder
                .set_definitions(Some(definitions(&[(7, Def::Plain { weight: 3 })])));
            for (mark, opcode, payload) in [
                (1, 5, vec![]),
                (2, 231, list(&[(1, 7, 41, 4)])),
                (3, 561, change(1, 1, 7, 41, 1)),
                (4, 679, profile()),
                (5, 230, clock()),
                (6, 233, stats()),
            ] {
                startup.push(at(mark, opcode, 0, payload)).unwrap();
            }
            assert!(startup.ready());
            assert!(carried(&startup.facts).is_empty());
        }

        /// The engine refuses Start until weight and potions are known; with the cutoff
        /// accepted, this synthetic native startup reaches an available Start.
        #[test]
        fn engine_start_waits_for_the_accepted_cutoff() {
            for accepted in [false, true] {
                let startup = startup(accepted, &ROWS);
                let mut engine = e::Engine::new(e::Mode::Clientless);
                let opened = engine.handle(e::Input::User {
                    at: e::At(0),
                    request: e::RequestKey(1),
                    command: e::Command::Open,
                });
                let session = opened
                    .io
                    .iter()
                    .find_map(|effect| match effect {
                        e::Effect::Open { session, .. } => Some(*session),
                        _ => None,
                    })
                    .unwrap();
                let observe = |engine: &mut e::Engine, at: u64, event: Event| {
                    engine.handle(e::Input::Observed {
                        at: e::At(at),
                        observation: Observation { session, event },
                    })
                };
                let map = startup
                    .facts
                    .iter()
                    .find_map(|fact| match fact {
                        Fact::Position { map, .. } => Some(*map),
                        _ => None,
                    })
                    .unwrap();
                let mut facts = startup.facts;
                facts.push(Fact::InventorySettled(e::IngressMark(startup.settled)));
                let mut decoder = startup.decoder;
                let online = observe(
                    &mut engine,
                    10,
                    Event::Baseline {
                        support: support(&decoder, true).unwrap(),
                        revision: e::EvidenceKey(1),
                        facts,
                        resumed: false,
                    },
                );
                let barrier = online
                    .io
                    .iter()
                    .find_map(|effect| match effect {
                        e::Effect::Session {
                            op: SessionOp::Quiesce(key),
                            ..
                        } => Some(*key),
                        _ => None,
                    })
                    .unwrap();
                observe(&mut engine, 11, Event::Quiescent(barrier));
                let setup = e::Setup::checked(e::SetupDraft {
                    activity: e::Activity::Combat {
                        targets: e::Targets::Except(Vec::new()),
                    },
                    boundary: e::Boundary::checked(
                        map,
                        e::Point { x: 100, y: 200 },
                        std::num::NonZeroU16::new(10).unwrap(),
                    )
                    .unwrap(),
                    hp_at: e::Percent::checked(30).unwrap(),
                    weight_limit: e::Percent::checked(90).unwrap(),
                    goal: None,
                })
                .unwrap_or_else(|_| panic!("fixture setup"));
                engine.handle(e::Input::User {
                    at: e::At(12),
                    request: e::RequestKey(2),
                    command: e::Command::Configure(setup),
                });
                let p = at(6, 282, 0, totals(&[(9, 100), (6, 100)]));
                let facts = decoder.apply(&p).unwrap();
                observe(
                    &mut engine,
                    13,
                    Event::Facts {
                        revision: e::EvidenceKey(6 << 2),
                        ingress: e::IngressMark(6),
                        facts,
                    },
                );
                for deferred in decoder.take_correlated() {
                    observe(
                        &mut engine,
                        13,
                        Event::Facts {
                            revision: e::EvidenceKey((deferred.mark << 2) | 3),
                            ingress: e::IngressMark(deferred.mark),
                            facts: deferred.facts,
                        },
                    );
                }
                let snapshot = engine.snapshot();
                if accepted {
                    assert_eq!(snapshot.blockers, []);
                    assert!(snapshot.allowed.contains(&e::Control::Start));
                } else {
                    assert_eq!(
                        snapshot.blockers,
                        [
                            e::Blocker::NeedsObservation(e::StateKind::Weight),
                            e::Blocker::NeedsObservation(e::StateKind::HpPotions),
                        ]
                    );
                    assert!(!snapshot.allowed.contains(&e::Control::Start));
                }
            }
        }
    }
}
