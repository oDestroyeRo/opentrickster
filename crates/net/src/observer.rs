use crate::{
    NetError, Result, VerifiedProfile, codec::Frames, decode::Decoder, profile::ORIGIN,
    session::Startup, wire::Reader,
};
use opentrickster_engine::{self as e, Event, Observation};
use std::collections::{BTreeMap, VecDeque};

/// A host-minted connection generation, never a raw game/socket identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionKey(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservedDirection {
    Inbound,
    Outbound,
}
pub struct ObserverContext {
    connection: ConnectionKey,
}
impl ObserverContext {
    pub fn checked(origin: &str, build: &str, connection: ConnectionKey) -> Result<Self> {
        if origin != ORIGIN || build != crate::BUILD || connection.0 == 0 {
            return Err(NetError::BuildChanged);
        }
        Ok(Self { connection })
    }
}
struct Connection {
    key: ConnectionKey,
    inbound: Frames,
    outbound: Frames,
}
struct Armed {
    ticket: e::ActionTicket,
    selected: Option<u64>,
    floor: u64,
    at: std::time::Instant,
}
/// Passive decoder. It cannot open a socket, authenticate, send a frame or acknowledge bytes.
pub struct GameObserver {
    page: Option<crate::page::State>,
    page_frontier: u64,
    page_seen: std::collections::BTreeSet<u64>,
    sample: Option<crate::sampler::Snapshot>,
    sample_pending: Option<(u64, u64)>,
    sample_request: u64,
    sample_after: Option<std::time::Instant>,
    input_flow: Option<crate::input_flow::Flow>,
    input_step: u64,
    barrier: Option<(e::BarrierKey, u64, u64)>,
    ordered: crate::ordered::OrderedIngress,
    definitions: Option<std::sync::Arc<crate::definitions::GameDefinitions>>,
    session: e::SessionKey,
    connections: BTreeMap<u64, Connection>,
    active: Option<ConnectionKey>,
    retired: Vec<ConnectionKey>,
    order: u64,
    epoch: u64,
    startup: Option<Startup>,
    decoder: Option<Decoder>,
    aux: crate::codec::AuxGroups,
    events: VecDeque<crate::TimedEvent>,
    armed: Option<Armed>,
    /// Engine action keys strictly increase; anything at or below this one is a replay.
    last_action: Option<e::ActionKey>,
    fault: Option<NetError>,
    closed: bool,
    closed_reported: bool,
}
impl GameObserver {
    pub fn new(session: e::SessionKey, profile: VerifiedProfile) -> Self {
        Self {
            page: None,
            page_frontier: 0,
            page_seen: Default::default(),
            sample: None,
            sample_pending: None,
            sample_request: 0,
            sample_after: None,
            input_flow: None,
            input_step: 0,
            barrier: None,
            ordered: Default::default(),
            definitions: profile.definitions,
            session,
            connections: BTreeMap::new(),
            active: None,
            retired: Vec::new(),
            order: 0,
            epoch: 0,
            startup: None,
            decoder: None,
            aux: crate::codec::AuxGroups::default(),
            events: VecDeque::new(),
            armed: None,
            last_action: None,
            fault: None,
            closed: false,
            closed_reported: false,
        }
    }
    pub fn bind_page(&mut self, binding: crate::PageBinding) -> Result<()> {
        self.check()?;
        if self.page.is_some() {
            return Err(NetError::WrongPhase);
        }
        self.page = Some(crate::page::State::new(binding));
        Ok(())
    }
    pub fn close_script(&mut self, request: u64) -> Result<crate::ObserverScript> {
        if let Some(flow) = &mut self.input_flow {
            flow.quiesce();
        }
        self.invalidate_sample(std::time::Instant::now());
        self.page
            .as_mut()
            .ok_or(NetError::WrongPhase)?
            .close_script(request)
    }
    /// Request IDs increase within one document. Run the returned guarded script normally.
    pub fn sample_script(
        &mut self,
        request: u64,
        layout_epoch: u64,
    ) -> Result<crate::ObserverScript> {
        self.check()?;
        if request <= self.sample_request {
            return Err(NetError::StaleConnection);
        }
        let script = self
            .page
            .as_ref()
            .ok_or(NetError::WrongPhase)?
            .sample_script(request, layout_epoch)?;
        self.sample_request = request;
        self.sample_pending = Some((request, layout_epoch));
        Ok(script)
    }
    pub fn active_input(&self) -> Option<e::ActionKey> {
        self.input_flow.as_ref().map(|flow| flow.ticket.id)
    }
    /// Starts one normal-input action. Hover preparation may reuse the same unsubmitted ticket.
    pub fn input_plan(
        &mut self,
        now: e::At,
        fences: crate::InputFences,
        ticket: e::ActionTicket,
    ) -> Result<crate::InputPlan> {
        self.check()?;
        if self.barrier.is_some() {
            return Err(NetError::Busy);
        }
        if !self.input_order_ready() {
            return Err(NetError::NotReady);
        }
        if let Some(flow) = &self.input_flow {
            if flow.ticket.id != ticket.id {
                return Err(NetError::Busy);
            }
            if flow.ticket.action != ticket.action || flow.ticket.not_after != ticket.not_after {
                return Err(NetError::DuplicateAction);
            }
        } else {
            if self.last_action.is_some_and(|last| ticket.id <= last) {
                return Err(NetError::DuplicateAction);
            }
            let sample = self
                .sample
                .as_ref()
                .filter(|s| s.captured.elapsed() <= std::time::Duration::from_millis(250))
                .ok_or(NetError::NotReady)?;
            self.input_flow = Some(crate::input_flow::Flow::new(ticket.clone(), sample));
        }
        self.next_input(now, fences)?.ok_or(NetError::NotReady)
    }
    /// Call once immediately after actual native dispatch, preserving that Instant.
    /// Old stored samples and outstanding sample requests can no longer acknowledge this step.
    pub fn input_dispatched(
        &mut self,
        step: crate::InputStepKey,
        captured_at: std::time::Instant,
    ) -> Result<()> {
        self.check()?;
        let flow = self.input_flow.as_mut().ok_or(NetError::StaleConnection)?;
        let action = flow.ticket.id;
        let submitted = flow.dispatched(step, captured_at)?;
        self.invalidate_sample(captured_at);
        if submitted {
            self.emit_at(
                Event::Receipt {
                    action,
                    delivery: e::Delivery::Submitted,
                },
                captured_at,
            )?;
        }
        Ok(())
    }
    /// None waits for a new receipt, gauge decision or physical drain. It never rearms a ticket.
    pub fn continue_input(
        &mut self,
        now: e::At,
        fences: crate::InputFences,
        action: e::ActionKey,
    ) -> Result<Option<crate::InputPlan>> {
        self.check()?;
        if self.active_input() != Some(action) {
            return Err(NetError::StaleConnection);
        }
        self.next_input(now, fences)
    }
    /// Notify only after every token-owned emergency up was actually dispatched.
    /// This creates no sampled-release, Sent or idle receipt. Fence admission before calling.
    pub fn emergency_released(
        &mut self,
        action: e::ActionKey,
        captured_at: std::time::Instant,
    ) -> Result<()> {
        self.check()?;
        let flow = self
            .input_flow
            .as_mut()
            .filter(|flow| flow.ticket.id == action)
            .ok_or(NetError::StaleConnection)?;
        let unknown = flow.submitted && flow.sent.is_none();
        flow.emergency_released(captured_at)?;
        self.invalidate_sample(captured_at);
        if unknown {
            self.emit_at(
                Event::Receipt {
                    action,
                    delivery: e::Delivery::Unknown,
                },
                captured_at,
            )?;
        }
        Ok(())
    }
    /// Cancels future action presses. Continue the same action for owned releases and normal
    /// right-button dig cancellation; a fresh complete idle proof emits the tagged barrier.
    pub fn quiesce(
        &mut self,
        now: e::At,
        fences: crate::InputFences,
        barrier: e::BarrierKey,
    ) -> Result<Option<crate::InputPlan>> {
        self.check()?;
        if now >= fences.not_after {
            return Err(NetError::Timeout);
        }
        if self.barrier.is_some_and(|(old, host, layout)| {
            old != barrier || host != fences.host_epoch || layout != fences.layout_epoch
        }) {
            return Err(NetError::Busy);
        }
        let first = self.barrier.is_none();
        self.barrier = Some((barrier, fences.host_epoch, fences.layout_epoch));
        if first {
            self.invalidate_sample(std::time::Instant::now());
        }
        if let Some(flow) = &mut self.input_flow {
            if first {
                flow.quiesce();
            }
            self.next_input(now, fences)
        } else {
            self.maybe_quiescent()?;
            Ok(None)
        }
    }
    fn invalidate_sample(&mut self, after: std::time::Instant) {
        self.sample = None;
        self.sample_pending = None;
        self.sample_after = Some(after);
    }
    fn input_order_ready(&self) -> bool {
        self.sample
            .as_ref()
            .is_some_and(|sample| self.page_frontier >= sample.raw_ingress)
            && self.aux.oldest_mark().is_none()
            && self.ordered.is_empty()
    }
    fn next_input(
        &mut self,
        now: e::At,
        mut fences: crate::InputFences,
    ) -> Result<Option<crate::InputPlan>> {
        let Some(flow) = &self.input_flow else {
            return Ok(None);
        };
        let target = match flow.ticket.action {
            e::Action::BasicAttack(key) => Some(
                self.decoder
                    .as_ref()
                    .ok_or(NetError::NotReady)?
                    .raw_target(key)?,
            ),
            _ => None,
        };
        let pending = self.decoder.as_ref().is_some_and(|d| d.action_pending());
        let candidate = self.input_flow.as_mut().ok_or(NetError::WrongPhase)?.next(
            self.sample.as_ref(),
            now,
            fences,
            self.session,
            target,
            pending,
        )?;
        if self.input_flow.as_ref().is_some_and(|flow| flow.done()) {
            if let Some(flow) = self.input_flow.take()
                && flow.sent.is_none()
            {
                if self
                    .armed
                    .as_ref()
                    .is_some_and(|armed| armed.ticket.id == flow.ticket.id)
                {
                    self.armed = None;
                }
                self.emit(Event::Receipt {
                    action: flow.ticket.id,
                    delivery: if flow.submitted {
                        e::Delivery::Unknown
                    } else {
                        e::Delivery::NotSent
                    },
                })?;
            }
            self.maybe_quiescent()?;
            return Ok(None);
        }
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        if candidate.steps.len() != 1 {
            return Err(NetError::Malformed);
        }
        let event = candidate.steps[0];
        if matches!(
            event,
            crate::NormalInput::LeftDown
                | crate::NormalInput::RightDown
                | crate::NormalInput::KeyDown(_)
        ) && !self.input_order_ready()
        {
            return Err(NetError::NotReady);
        }
        let needs_arm = self.input_flow.as_ref().is_some_and(|flow| !flow.armed)
            && matches!(
                event,
                crate::NormalInput::LeftDown | crate::NormalInput::KeyDown(_)
            );
        if needs_arm {
            let ticket = self
                .input_flow
                .as_ref()
                .ok_or(NetError::WrongPhase)?
                .ticket
                .clone();
            self.arm_action(ticket)?;
            self.input_flow.as_mut().ok_or(NetError::WrongPhase)?.armed = true;
        }
        let flow = self.input_flow.as_mut().ok_or(NetError::WrongPhase)?;
        let ticket_deadline = flow.deadline(now);
        let ordinary = matches!(event, crate::NormalInput::KeyDown(_))
            || matches!(event, crate::NormalInput::LeftDown) && !flow.gauge_hold()
            || matches!(event, crate::NormalInput::RightDown);
        let release_by = ordinary.then_some(e::At(
            now.0
                .saturating_add(250)
                .min(fences.not_after.0)
                .min(ticket_deadline),
        ));
        fences.not_after = e::At(fences.not_after.0.min(now.0.saturating_add(250)));
        if !matches!(
            event,
            crate::NormalInput::LeftUp | crate::NormalInput::RightUp | crate::NormalInput::KeyUp(_)
        ) {
            fences.not_after = e::At(fences.not_after.0.min(ticket_deadline));
        }
        self.input_step = self.input_step.checked_add(1).ok_or(NetError::Capacity)?;
        let step = crate::InputStepKey(self.input_step);
        let action = flow.ticket.id;
        self.input_flow
            .as_mut()
            .ok_or(NetError::WrongPhase)?
            .issue(step, event);
        Ok(Some(crate::InputPlan {
            session: self.session,
            action,
            fences,
            steps: candidate.steps,
            resample: candidate.requires_resample,
            step,
            release_by,
        }))
    }
    fn maybe_quiescent(&mut self) -> Result<()> {
        let Some((barrier, host, layout)) = self.barrier else {
            return Ok(());
        };
        let Some(sample) = &self.sample else {
            return Ok(());
        };
        if self.input_flow.is_some()
            || self.armed.is_some()
            || self.decoder.as_ref().is_none_or(|d| d.action_pending())
            || self.aux.oldest_mark().is_some()
            || !self.ordered.is_empty()
            || self.page_frontier < sample.raw_ingress
            || sample.host_epoch != host
            || sample.layout_epoch != layout
            || !crate::sampler::idle(sample)
        {
            return Ok(());
        }
        let received = sample.captured;
        self.barrier = None;
        self.emit_at(Event::Quiescent(barrier), received)
    }
    /// `origin` comes from the authenticated native frame callback, never from JSON.
    /// Preserve the Wry callback Instant when queuing the message for a later UI pump.
    pub fn ingest_page(
        &mut self,
        origin: &str,
        json: &str,
        captured_at: std::time::Instant,
    ) -> Result<Option<crate::PageNotice>> {
        let result = self.ingest_page_inner(origin, json, captured_at);
        if let Err(error) = result
            && error != NetError::StaleConnection
        {
            return self.fail(error);
        }
        result
    }
    fn ingest_page_inner(
        &mut self,
        origin: &str,
        json: &str,
        captured_at: std::time::Instant,
    ) -> Result<Option<crate::PageNotice>> {
        let value = self
            .page
            .as_mut()
            .ok_or(NetError::WrongPhase)?
            .parse(origin, json)?;
        self.order = self
            .order
            .max(self.page.as_ref().ok_or(NetError::WrongPhase)?.raw_ingress);
        match value["kind"].as_str() {
            Some("closed") => {
                let request = value["request"]
                    .as_str()
                    .and_then(|s| u64::from_str_radix(s, 16).ok())
                    .filter(|n| *n > 0)
                    .ok_or(NetError::Malformed)?;
                let sockets = value["sockets"]
                    .as_u64()
                    .filter(|n| *n <= 64)
                    .ok_or(NetError::Malformed)? as u32;
                if value["remaining"].as_u64() != Some(0)
                    || self.page.as_ref().and_then(|p| p.closing) != Some(request)
                {
                    return Err(NetError::StaleConnection);
                }
                return Ok(Some(crate::PageNotice::Closed { request, sockets }));
            }
            Some("fault" | "close-failed") => {
                let fault = crate::page::fault(&value);
                let _ = self.fail::<()>(NetError::Uncertain);
                return Ok(Some(crate::PageNotice::Fault(fault)));
            }
            _ => {}
        }
        if self.page.as_ref().is_some_and(|p| p.closing.is_some()) {
            return Ok(None);
        }
        if value["kind"] == "sample" {
            let request = value["request"]
                .as_str()
                .and_then(|s| u64::from_str_radix(s, 16).ok())
                .ok_or(NetError::Malformed)?;
            let layout = value["layout"]
                .as_str()
                .and_then(|s| u64::from_str_radix(s, 16).ok())
                .ok_or(NetError::Malformed)?;
            if self.sample_pending != Some((request, layout)) {
                return Ok(None);
            }
            self.sample_pending = None;
            let snapshot = match crate::sampler::parse_snapshot(&value, captured_at) {
                Ok(snapshot) => Some(snapshot),
                Err(NetError::Unsupported | NetError::Timeout) => None,
                Err(error) => return Err(error),
            };
            self.sample = snapshot.filter(|s| {
                s.captured.elapsed() <= std::time::Duration::from_millis(250)
                    && self.sample_after.is_none_or(|at| s.captured > at)
                    && self
                        .decoder
                        .as_ref()
                        .or_else(|| self.startup.as_ref().map(|s| &s.decoder))
                        .and_then(|d| d.sampling_identity())
                        == Some((s.motion.own, s.zone))
            });
            let available = self.sample.is_some();
            self.maybe_quiescent()?;
            return Ok(Some(crate::PageNotice::Sample { request, available }));
        }
        if value["kind"] == "frontier" {
            let through = crate::page::ordinal(&value)?;
            if through < self.page_frontier {
                return self.fail(NetError::Uncertain);
            }
            self.page_frontier = through;
            self.order = self.order.max(through);
            self.page_seen.retain(|mark| *mark > through);
            self.advance(through, captured_at)?;
            return Ok(None);
        }
        let connection = crate::page::connection(&value)?;
        match value["kind"].as_str() {
            Some("socket") => {
                self.connect(ObserverContext::checked(origin, crate::BUILD, connection)?)?
            }
            Some("socket-closed") => self.disconnect(connection)?,
            Some("game-login") => {
                let mark = crate::page::ordinal(&value)?;
                self.order = self.order.max(mark);
                let fields = value["header"]
                    .as_array()
                    .filter(|a| a.len() == 6)
                    .ok_or(NetError::Malformed)?;
                let mut header = [0; 6];
                for (out, input) in header.iter_mut().zip(fields) {
                    *out = input
                        .as_u64()
                        .and_then(|n| u8::try_from(n).ok())
                        .ok_or(NetError::Malformed)?;
                }
                self.notice_game_login(connection, header)?;
            }
            Some("frame") => {
                let captured = crate::page::captured(&value, captured_at)?;
                let (direction, packet) = crate::page::frame(&value, captured)?;
                if packet.mark <= self.page_frontier || !self.page_seen.insert(packet.mark) {
                    return self.fail(NetError::Uncertain);
                }
                if self.page_seen.len() > 256 {
                    return self.fail(NetError::Capacity);
                }
                self.order = self.order.max(packet.mark);
                self.packet(connection, direction, packet, self.page_frontier)?;
            }
            _ => return Err(NetError::Malformed),
        }
        Ok(None)
    }
    /// Only the six-byte header crosses the page boundary. Never pass GS authentication bytes.
    pub fn notice_game_login(&mut self, connection: ConnectionKey, header: [u8; 6]) -> Result<()> {
        self.check()?;
        if !self.connections.contains_key(&connection.0) {
            return Err(NetError::StaleConnection);
        }
        let expected = crate::codec::OpcodeMap::current(b"c2s").wire_opcode(4);
        if header[..2] != 14u16.to_le_bytes()
            || header[2..4] != expected.to_le_bytes()
            || header[4..] != [0, 0]
        {
            return self.fail(NetError::Malformed);
        }
        if self.active.is_some() || self.startup.is_some() || self.decoder.is_some() {
            return self.fail(NetError::WrongPhase);
        }
        self.epoch = self.epoch.checked_add(1).ok_or(NetError::Capacity)?;
        let mut startup = Startup::new(self.epoch);
        startup.decoder.set_definitions(self.definitions.clone());
        self.active = Some(connection);
        self.startup = Some(startup);
        Ok(())
    }
    pub fn connect(&mut self, context: ObserverContext) -> Result<()> {
        self.check()?;
        if self.retired.contains(&context.connection)
            || self.connections.contains_key(&context.connection.0)
        {
            return Err(NetError::StaleConnection);
        }
        if self.retired.len() >= 64 || self.connections.len() >= 8 {
            return self.fail(NetError::Capacity);
        }
        self.connections.insert(
            context.connection.0,
            Connection {
                key: context.connection,
                inbound: Frames::new(b"s2c"),
                outbound: Frames::new(b"c2s"),
            },
        );
        Ok(())
    }
    pub fn disconnect(&mut self, key: ConnectionKey) -> Result<()> {
        self.check()?;
        self.connections
            .remove(&key.0)
            .ok_or(NetError::StaleConnection)?;
        self.retired.push(key);
        if self.active == Some(key) {
            self.active = None;
            self.startup = None;
            self.decoder = None;
            self.aux = Default::default();
            self.ordered = Default::default();
            self.ordered = Default::default();
            if let Some(armed) = self.armed.take() {
                self.emit(Event::Receipt {
                    action: armed.ticket.id,
                    delivery: e::Delivery::Unknown,
                })?;
            }
            self.emit(Event::Lost)?;
        }
        Ok(())
    }
    /// Only feed bytes copied from the selected official page's actual socket events.
    /// Replaced connection callbacks must retain their original ConnectionKey.
    pub fn observe_binary(
        &mut self,
        connection: ConnectionKey,
        direction: ObservedDirection,
        bytes: &[u8],
    ) -> Result<()> {
        self.check()?;
        if self.page.is_some() {
            return Err(NetError::WrongPhase);
        }
        let result = self.observe_at(connection, direction, bytes, std::time::Instant::now());
        if let Err(error) = result {
            if error != NetError::StaleConnection {
                return self.fail(error);
            }
            return Err(error);
        }
        Ok(())
    }
    fn observe_at(
        &mut self,
        key: ConnectionKey,
        direction: ObservedDirection,
        bytes: &[u8],
        captured: std::time::Instant,
    ) -> Result<()> {
        let c = self
            .connections
            .get_mut(&key.0)
            .ok_or(NetError::StaleConnection)?;
        if c.key != key {
            return Err(NetError::StaleConnection);
        }
        let packets = match direction {
            ObservedDirection::Inbound => c.inbound.push_at(bytes, &mut self.order, captured)?,
            ObservedDirection::Outbound => c.outbound.push_at(bytes, &mut self.order, captured)?,
        };
        let through = c
            .inbound
            .pending_mark()
            .into_iter()
            .chain(c.outbound.pending_mark())
            .min()
            .map_or(self.order, |mark| mark.saturating_sub(1));
        let mut packets = packets.into_iter().peekable();
        while let Some(packet) = packets.next() {
            let safe = packets
                .peek()
                .map_or(through, |next| through.min(next.mark.saturating_sub(1)));
            self.packet(key, direction, packet, safe)?;
        }
        Ok(())
    }
    fn packet(
        &mut self,
        key: ConnectionKey,
        direction: ObservedDirection,
        packet: crate::codec::Packet,
        through: u64,
    ) -> Result<()> {
        if !self.connections.contains_key(&key.0) {
            return Err(NetError::StaleConnection);
        }
        if direction == ObservedDirection::Outbound {
            if packet.opcode == 4 {
                return self.fail(NetError::Malformed);
            }
            if self.active == Some(key) {
                self.actual_send(&packet)?;
            }
            return Ok(());
        }
        if self.active != Some(key) {
            return Ok(());
        }
        let proof_at = packet.received;
        if let Some(startup) = &mut self.startup {
            startup.push_through(packet, through)?;
        } else {
            for packet in self.aux.push(packet)? {
                self.ordered.push(packet)?;
            }
        }
        self.advance(through, proof_at)
    }
    fn advance(&mut self, through: u64, proof_at: std::time::Instant) -> Result<()> {
        if let Some(startup) = &mut self.startup {
            startup.advance(through)?;
            if startup.ready() {
                let startup = self.startup.take().ok_or(NetError::WrongPhase)?;
                let mut support = crate::session::support(&startup.decoder, false)?;
                if self.page.is_some() {
                    support.move_to = true;
                    support.basic_attack = true;
                    support.dig_cycle = true;
                    support.quiesce = true;
                }
                let mut facts = startup.facts;
                if startup.settled > 0 {
                    facts.push(e::Fact::InventorySettled(e::IngressMark(startup.settled)));
                }
                self.decoder = Some(startup.decoder);
                self.aux = startup.aux;
                self.ordered = startup.ordered;
                self.emit_at(
                    Event::Baseline {
                        support,
                        revision: e::EvidenceKey((self.order << 2) | 1),
                        facts,
                        resumed: false,
                    },
                    startup.received.unwrap_or(proof_at),
                )?;
            }
        } else if let Some(decoder) = &mut self.decoder {
            let (ready, settled) = self.ordered.release(through, self.aux.oldest_mark())?;
            let mut events = Vec::new();
            for packet in ready {
                let facts = decoder.apply(&packet)?;
                if decoder.physical_uncertain() {
                    return Err(NetError::Uncertain);
                }
                if let Some(flow) = &mut self.input_flow {
                    flow.observe_facts(packet.mark, packet.opcode, &facts);
                }
                if !facts.is_empty() {
                    events.push((
                        packet.received,
                        Event::Facts {
                            revision: e::EvidenceKey(packet.mark << 2),
                            ingress: e::IngressMark(packet.mark),
                            facts,
                        },
                    ));
                }
                let mut correlated = BTreeMap::<u64, (std::time::Instant, Vec<e::Fact>)>::new();
                for result in decoder.take_correlated() {
                    correlated
                        .entry(result.mark)
                        .or_insert_with(|| (result.received, Vec::new()))
                        .1
                        .extend(result.facts);
                }
                for (mark, (received, facts)) in correlated {
                    events.push((
                        received,
                        Event::Facts {
                            revision: e::EvidenceKey((mark << 2) | 3),
                            ingress: e::IngressMark(mark),
                            facts,
                        },
                    ));
                }
            }
            if let Some(mark) = settled {
                events.push((
                    proof_at,
                    Event::Facts {
                        revision: e::EvidenceKey((mark << 2) | 2),
                        ingress: e::IngressMark(mark),
                        facts: vec![e::Fact::InventorySettled(e::IngressMark(mark))],
                    },
                ));
            }
            for (received, event) in events {
                self.emit_at(event, received)?;
            }
        }
        self.maybe_quiescent()?;
        Ok(())
    }
    pub fn arm_action(&mut self, ticket: e::ActionTicket) -> Result<()> {
        self.check()?;
        if self.page.as_ref().is_some_and(|p| p.closing.is_some()) {
            return Err(NetError::Closed);
        }
        if self.armed.is_some() {
            return Err(NetError::Busy);
        }
        if self.last_action.is_some_and(|last| ticket.id <= last) {
            return Err(NetError::DuplicateAction);
        }
        let decoder = self.decoder.as_ref().ok_or(NetError::WrongPhase)?;
        if decoder.action_pending() {
            return Err(NetError::Busy);
        }
        match ticket.action {
            e::Action::MoveTo(point) => {
                crate::actions::coordinate(point)?;
            }
            e::Action::BasicAttack(key) => {
                decoder.raw_target(key)?;
            }
            e::Action::DigAt(point) => {
                crate::actions::coordinate(point)?;
            }
            _ => return Err(NetError::Unsupported),
        }
        self.last_action = Some(ticket.id);
        self.armed = Some(Armed {
            ticket,
            selected: None,
            floor: self.order,
            at: std::time::Instant::now(),
        });
        Ok(())
    }
    fn actual_send(&mut self, p: &crate::codec::Packet) -> Result<()> {
        if let Some(flow) = &mut self.input_flow
            && matches!(flow.ticket.action, e::Action::DigAt(_))
            && matches!(p.opcode, 48 | 49)
        {
            if p.payload.len() != if p.opcode == 48 { 4 } else { 8 } {
                return Err(NetError::Malformed);
            }
            flow.observe_send(p.opcode, p.mark);
            return Ok(());
        }
        let Some(armed) = &mut self.armed else {
            return Ok(());
        };
        if p.mark <= armed.floor || p.received <= armed.at {
            return Ok(());
        }
        let mut r = Reader::new(&p.payload);
        let matched = match (&armed.ticket.action, p.opcode) {
            (e::Action::MoveTo(point), 24) => {
                let from = e::Point {
                    x: r.u16()? as i32,
                    y: r.u16()? as i32,
                };
                let target = e::Point {
                    x: r.u16()? as i32,
                    y: r.u16()? as i32,
                };
                let mode = r.u8()?;
                r.end()?;
                if !matches!(mode, 1 | 2) {
                    return Err(NetError::Malformed);
                }
                crate::actions::movement(from, *point, mode)?.0.as_slice() == p.payload.as_slice()
                    && target == *point
            }
            (e::Action::BasicAttack(key), 288 | 29 | 477) => {
                let target = self
                    .decoder
                    .as_ref()
                    .ok_or(NetError::WrongPhase)?
                    .raw_target(*key)?;
                let actual = r.u16()?;
                if p.opcode == 288 {
                    r.end()?;
                    if actual == target {
                        armed.selected = Some(p.mark);
                    }
                    false
                } else if p.opcode == 29 {
                    let mode = r.u8()?;
                    r.end()?;
                    if !matches!(mode, 1 | 2) {
                        return Err(NetError::Malformed);
                    }
                    let (_, expected) = crate::actions::normal_attack(target, mode);
                    actual == target
                        && armed.selected.is_some_and(|mark| p.mark > mark)
                        && expected.0.as_slice() == p.payload.as_slice()
                } else {
                    let crc = r.u16()?;
                    let bunch = r.u16()?;
                    let mode = r.u8()?;
                    r.end()?;
                    if !matches!(mode, 1 | 2) {
                        return Err(NetError::Malformed);
                    }
                    actual == target
                        && crate::actions::gun_shot(target, crc, bunch, mode)
                            .0
                            .as_slice()
                            == p.payload.as_slice()
                }
            }
            (e::Action::DigAt(point), 44) => {
                let actual = e::Point {
                    x: r.u16()? as i32,
                    y: r.u16()? as i32,
                };
                let mode = r.u8()?;
                r.end()?;
                if !matches!(mode, 1 | 2) {
                    return Err(NetError::Malformed);
                }
                actual == *point
            }
            _ => false,
        };
        if matched {
            let armed = self.armed.take().ok_or(NetError::WrongPhase)?;
            if !matches!(armed.ticket.action, e::Action::DigAt(_)) {
                self.decoder
                    .as_mut()
                    .ok_or(NetError::WrongPhase)?
                    .expect_action(armed.ticket.action, p.mark)?;
            }
            if let Some(flow) = &mut self.input_flow {
                flow.observe_send(p.opcode, p.mark);
            }
            self.emit_at(
                Event::Receipt {
                    action: armed.ticket.id,
                    delivery: e::Delivery::Sent(e::IngressMark(p.mark)),
                },
                p.received,
            )?;
        }
        Ok(())
    }
    pub fn readiness(&self) -> crate::Readiness {
        self.decoder
            .as_ref()
            .map(|d| d.readiness)
            .or_else(|| self.startup.as_ref().map(|s| s.decoder.readiness))
            .unwrap_or_default()
    }
    /// Public catalog label for an already-received opaque item kind.
    pub fn item_label(&self, kind: e::ItemKind) -> Option<e::DisplayText> {
        self.decoder
            .as_ref()
            .or_else(|| self.startup.as_ref().map(|s| &s.decoder))
            .and_then(|d| d.item_label(kind))
    }
    pub fn item_descriptor(&self, kind: e::ItemKind) -> Option<crate::ItemDescriptor> {
        self.decoder
            .as_ref()
            .or_else(|| self.startup.as_ref().map(|s| &s.decoder))
            .and_then(|d| d.item_descriptor(kind))
    }
    pub fn try_next(&mut self) -> Result<Option<Observation>> {
        Ok(self.next_timed()?.map(|event| Observation {
            session: self.session,
            event: event.event,
        }))
    }
    pub fn try_next_at(&mut self, now: e::At) -> Result<Option<(e::At, Observation)>> {
        Ok(self.next_timed()?.map(|event| event.at(now, self.session)))
    }
    fn next_timed(&mut self) -> Result<Option<crate::TimedEvent>> {
        if let Some(event) = self.events.pop_front() {
            return Ok(Some(event));
        }
        if let Some(error) = self.fault.take() {
            return Err(error);
        }
        if self.closed && !self.closed_reported {
            self.closed_reported = true;
            return Ok(Some(crate::TimedEvent {
                event: Event::Closed,
                received: std::time::Instant::now(),
            }));
        }
        Ok(None)
    }
    pub fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.connections.clear();
        self.input_flow = None;
        self.sample = None;
        self.sample_pending = None;
        self.barrier = None;
        self.active = None;
        self.startup = None;
        self.decoder = None;
        self.armed = None;
        self.aux = Default::default();
        self.ordered = Default::default();
        self.closed = true;
        Ok(())
    }
    fn emit(&mut self, event: Event) -> Result<()> {
        self.emit_at(event, std::time::Instant::now())
    }
    fn emit_at(&mut self, event: Event, received: std::time::Instant) -> Result<()> {
        if self.events.len() >= 256 {
            return self.fail(NetError::Capacity);
        }
        self.events.push_back(crate::TimedEvent { event, received });
        Ok(())
    }
    fn check(&self) -> Result<()> {
        if self.closed {
            Err(NetError::Closed)
        } else if self.fault.is_some() {
            Err(NetError::Uncertain)
        } else {
            Ok(())
        }
    }
    fn fail<T>(&mut self, error: NetError) -> Result<T> {
        self.connections.clear();
        self.input_flow = None;
        self.sample = None;
        self.sample_pending = None;
        self.barrier = None;
        self.active = None;
        self.startup = None;
        self.decoder = None;
        self.armed = None;
        self.fault = Some(error);
        self.closed = true;
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origin_build_and_generation_are_required() {
        for origin in [
            "http://tr-play.playall.in.th",
            "https://tr-play.playall.in.th.attacker.example",
            "null",
        ] {
            assert!(ObserverContext::checked(origin, crate::BUILD, ConnectionKey(1)).is_err());
        }
        assert!(ObserverContext::checked(ORIGIN, "changed", ConnectionKey(1)).is_err());
        assert!(ObserverContext::checked(ORIGIN, crate::BUILD, ConnectionKey(0)).is_err());
        assert!(ObserverContext::checked(ORIGIN, crate::BUILD, ConnectionKey(1)).is_ok());
    }
    #[test]
    fn only_a_new_post_dispatch_sample_request_can_acknowledge_a_press() {
        use serde_json::json;
        use zeroize::Zeroizing;
        let mut value = crate::test_support::sample_value();
        let own = value["snapshot"]["motion"]["own"].as_u64().unwrap() as u16;
        let zone = value["snapshot"]["zone"].as_u64().unwrap() as u16;
        let mut decoder = Decoder::new();
        let packet = |opcode, payload| crate::codec::Packet {
            opcode,
            payload: Zeroizing::new(payload),
            aux: 0,
            mark: 1,
            received: std::time::Instant::now(),
        };
        let mut clock = vec![0; 10];
        clock[4..6].copy_from_slice(&own.to_le_bytes());
        decoder.apply(&packet(230, clock)).unwrap();
        let mut stats = vec![0; 251];
        stats[219..221].copy_from_slice(&zone.to_le_bytes());
        decoder.apply(&packet(233, stats)).unwrap();
        let mut monster = vec![0; 38];
        monster[..2].copy_from_slice(&73u16.to_le_bytes());
        monster[2..4].copy_from_slice(&3u16.to_le_bytes());
        monster[22] = b'M';
        let facts = decoder.apply(&packet(7, monster)).unwrap();
        let key = facts
            .iter()
            .find_map(|f| match f {
                e::Fact::Entity(actor) => Some(actor.key),
                _ => None,
            })
            .unwrap();
        let (session, ticket) = crate::test_support::ticket(e::Action::BasicAttack(key));
        let action = ticket.id;
        let profile = VerifiedProfile {
            first_host: "127.0.0.1".into(),
            first_port: 1,
            marker: Zeroizing::new("fixture".into()),
            client_version: Zeroizing::new(vec![0; 4]),
            definitions: None,
        };
        let mut observer = GameObserver::new(session, profile);
        observer
            .bind_page(crate::PageBinding::new(7, "a".repeat(32)).unwrap())
            .unwrap();
        observer.decoder = Some(decoder);
        observer.page_frontier = 19;
        value["channel"] = json!("opentrickster.net.v1");
        value["guard"] = json!("a".repeat(32));
        value["sequence"] = json!(1);
        observer.sample_script(10, 11).unwrap();
        assert!(matches!(
            observer
                .ingest_page(ORIGIN, &value.to_string(), std::time::Instant::now())
                .unwrap(),
            Some(crate::PageNotice::Sample {
                available: true,
                ..
            })
        ));
        let fences = crate::InputFences {
            host_epoch: 7,
            layout_epoch: 11,
            not_after: e::At(1000),
        };
        let plan = observer.input_plan(e::At(10), fences, ticket).unwrap();
        assert_eq!(plan.steps(), [crate::NormalInput::LeftDown]);
        observer.sample_script(11, 11).unwrap();
        observer
            .input_dispatched(plan.step_key(), std::time::Instant::now())
            .unwrap();
        value["request"] = json!("b");
        value["sequence"] = json!(2);
        value["snapshot"]["input"]["left"] = json!(true);
        value["snapshot"]["motion"]["screenHeld"] = json!(true);
        assert!(
            observer
                .ingest_page(ORIGIN, &value.to_string(), std::time::Instant::now())
                .unwrap()
                .is_none()
        );
        assert!(
            observer
                .continue_input(e::At(11), fences, action)
                .unwrap()
                .is_none()
        );
        observer.sample_script(12, 11).unwrap();
        value["request"] = json!("c");
        value["sequence"] = json!(3);
        assert!(matches!(
            observer
                .ingest_page(ORIGIN, &value.to_string(), std::time::Instant::now())
                .unwrap(),
            Some(crate::PageNotice::Sample {
                available: true,
                ..
            })
        ));
        let up = observer
            .continue_input(e::At(12), fences, action)
            .unwrap()
            .unwrap();
        assert_eq!(up.steps(), [crate::NormalInput::LeftUp]);
        assert_ne!(up.step_key(), plan.step_key());
        assert_eq!(observer.active_input(), Some(action));
    }
    #[test]
    fn socket_free_frontier_releases_baseline_and_preserves_original_age() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        use serde_json::json;
        use zeroize::Zeroizing;
        let profile = VerifiedProfile {
            first_host: "127.0.0.1".into(),
            first_port: 1234,
            marker: Zeroizing::new("fixture".into()),
            client_version: Zeroizing::new(vec![0; 4]),
            definitions: None,
        };
        let mut engine = e::Engine::new(e::Mode::Assisted);
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
        let mut observer = GameObserver::new(session, profile);
        observer
            .bind_page(crate::PageBinding::new(7, "a".repeat(32)).unwrap())
            .unwrap();
        let mut sequence = 0;
        let mut raw_ingress = 0;
        let mut ingest = |observer: &mut GameObserver, kind: &str, fields: serde_json::Value| {
            sequence += 1;
            raw_ingress = raw_ingress.max(fields["ingress"].as_u64().unwrap_or(0));
            let mut value = json!({"channel":"opentrickster.net.v1","epoch":"7","guard":"a".repeat(32),"sequence":sequence,"rawIngress":raw_ingress,"kind":kind});
            value
                .as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            observer.ingest_page(ORIGIN, &value.to_string(), std::time::Instant::now())
        };
        ingest(&mut observer, "socket", json!({"connection":1})).unwrap();
        let outgoing = crate::codec::OpcodeMap::current(b"c2s");
        let login = outgoing.encode(4, 0, &[0; 14]).unwrap();
        ingest(
            &mut observer,
            "game-login",
            json!({"connection":1,"ingress":1,"header":login[..6]}),
        )
        .unwrap();
        ingest(&mut observer, "frontier", json!({"ingress":1,"ageMs":0})).unwrap();
        let incoming = crate::codec::OpcodeMap::current(b"s2c");
        let mut profile = vec![0; 69];
        profile[17] = b'S';
        for (i, (opcode, payload)) in [
            (5, vec![]),
            (679, profile),
            (230, vec![0; 10]),
            (233, vec![0; 251]),
        ]
        .into_iter()
        .enumerate()
        {
            let bytes = incoming.encode(opcode, 0, &payload).unwrap();
            ingest(&mut observer,"frame",json!({"connection":1,"direction":"inbound","ingress":i+2,"ageMs":1000,"data":STANDARD.encode(bytes)})).unwrap();
        }
        assert!(!observer.readiness().own_baseline_seen);
        ingest(&mut observer, "frontier", json!({"ingress":5,"ageMs":0})).unwrap();
        assert!(observer.readiness().own_baseline_seen);
        let (at, baseline) = observer.try_next_at(e::At(10000)).unwrap().unwrap();
        assert!(at.0 <= 9000);
        assert!(matches!(baseline.event, Event::Baseline { .. }));
        let bytes = incoming.encode(36, 0, &17u32.to_le_bytes()).unwrap();
        ingest(&mut observer,"frame",json!({"connection":1,"direction":"inbound","ingress":6,"ageMs":1500,"data":STANDARD.encode(bytes)})).unwrap();
        ingest(&mut observer, "frontier", json!({"ingress":6,"ageMs":0})).unwrap();
        let (at, facts) = observer.try_next_at(e::At(10000)).unwrap().unwrap();
        assert!(at.0 <= 8500);
        assert!(matches!(
            facts.event,
            Event::Facts {
                ingress: e::IngressMark(6),
                ..
            }
        ));
    }
}
