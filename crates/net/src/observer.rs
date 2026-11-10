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
/// The client's zone change. After s2c 154, `CompleteZoneChangeAsync` closes the announcing link
/// and logs in on a replacement link with c2s 156.
#[derive(Clone, Copy)]
enum Handoff {
    /// The active link announced a handoff and is still open.
    Announced(ConnectionKey),
    /// The announcing link closed at this instant. The replacement must log in within
    /// `HANDOFF_WINDOW`.
    Waiting(std::time::Instant),
    /// The replacement is active, and the announcing link has not reported its close.
    Replaced(ConnectionKey),
}
/// The client opens the replacement right after closing the old link, retrying the open once.
const HANDOFF_WINDOW: std::time::Duration = std::time::Duration::from_secs(10);
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
    /// The dig capability last reported since an assisted Baseline.
    dig_cycle: Option<bool>,
    ordered: crate::ordered::OrderedIngress,
    definitions: Option<std::sync::Arc<crate::definitions::GameDefinitions>>,
    session: e::SessionKey,
    connections: BTreeMap<u64, Connection>,
    active: Option<ConnectionKey>,
    handoff: Option<Handoff>,
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
            dig_cycle: None,
            ordered: Default::default(),
            definitions: profile.definitions,
            session,
            connections: BTreeMap::new(),
            active: None,
            handoff: None,
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
        // The page forwards every input of derived maxima, but a decoder that predates it
        // cannot prove it saw the whole StartSet. The next game login re-baselines.
        if let Some(startup) = &mut self.startup {
            startup.decoder.lose_game_parts();
        }
        if let Some(decoder) = &mut self.decoder {
            decoder.lose_game_parts();
        }
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
            if let Some((dig, captured)) =
                self.sample.as_ref().map(|s| (s.dig.is_some(), s.captured))
                && self.dig_cycle.is_some_and(|reported| reported != dig)
            {
                self.dig_cycle = Some(dig);
                self.emit_at(Event::DigCycle(dig), captured)?;
            }
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
            Some("socket-closed") => self.close_link(connection, captured_at)?,
            Some(kind @ ("game-login" | "game-handoff")) => {
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
                if kind == "game-login" {
                    self.notice_game_login(connection, header)?;
                } else {
                    self.notice_game_handoff(connection, header)?;
                }
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
        startup.decoder.follow_handoffs();
        self.active = Some(connection);
        self.startup = Some(startup);
        Ok(())
    }
    /// Only the six-byte header of c2s 156 crosses the page boundary. Never pass its body.
    pub fn notice_game_handoff(
        &mut self,
        connection: ConnectionKey,
        header: [u8; 6],
    ) -> Result<()> {
        self.check()?;
        if !self.connections.contains_key(&connection.0) {
            return Err(NetError::StaleConnection);
        }
        let expected = crate::codec::OpcodeMap::current(b"c2s").wire_opcode(156);
        if header[..2] != 13u16.to_le_bytes()
            || header[2..4] != expected.to_le_bytes()
            || header[4..] != [0, 0]
        {
            return self.fail(NetError::Malformed);
        }
        self.expire_handoff()?;
        self.handoff = match self.handoff {
            Some(Handoff::Announced(from)) => Some(Handoff::Replaced(from)),
            Some(Handoff::Waiting(_)) => None,
            // A login after the window closed belongs to a session already reported lost.
            _ if self.decoder.is_none() && self.startup.is_none() => return Ok(()),
            _ => return self.fail(NetError::Uncertain),
        };
        self.active = Some(connection);
        // The client's replacement link starts with no Aux groups.
        self.aux = Default::default();
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
        self.close_link(key, std::time::Instant::now())
    }
    fn close_link(&mut self, key: ConnectionKey, at: std::time::Instant) -> Result<()> {
        self.check()?;
        self.connections
            .remove(&key.0)
            .ok_or(NetError::StaleConnection)?;
        self.retired.push(key);
        match self.handoff {
            Some(Handoff::Replaced(from)) if from == key => self.handoff = None,
            Some(Handoff::Announced(from)) if from == key => {
                self.active = None;
                self.handoff = Some(Handoff::Waiting(at));
            }
            _ if self.active == Some(key) => self.lose()?,
            _ => {}
        }
        Ok(())
    }
    fn lose(&mut self) -> Result<()> {
        self.handoff = None;
        self.active = None;
        self.startup = None;
        self.decoder = None;
        self.dig_cycle = None;
        self.aux = Default::default();
        self.ordered = Default::default();
        if let Some(armed) = self.armed.take() {
            self.emit(Event::Receipt {
                action: armed.ticket.id,
                delivery: e::Delivery::Unknown,
            })?;
        }
        self.emit(Event::Lost)
    }
    fn expire_handoff(&mut self) -> Result<()> {
        if matches!(self.handoff, Some(Handoff::Waiting(since)) if since.elapsed() >= HANDOFF_WINDOW)
        {
            self.lose()?;
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
        // The client closes the old link before it opens the replacement.
        if matches!(self.handoff, Some(Handoff::Replaced(from)) if from == key) {
            return self.fail(NetError::Uncertain);
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
        // Seen on arrival, so a close that overtakes the frontier still waits for the replacement.
        if packet.opcode == 154 && self.decoder.is_some() {
            self.handoff = Some(Handoff::Announced(key));
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
                let mut startup = self.startup.take().ok_or(NetError::WrongPhase)?;
                // The observed client constructs its gun controller and sends its own 511s.
                startup.decoder.construct_gun();
                startup.decoder.take_key_requests();
                let mut support = crate::session::support(&startup.decoder, false)?;
                if self.page.is_some() {
                    support.move_to = true;
                    support.basic_attack = true;
                    // The client installs its dig feature after world entry. A later sample reports it.
                    support.dig_cycle = false;
                    support.quiesce = true;
                    self.dig_cycle = Some(false);
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
            let applied = !ready.is_empty();
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
                // The old map's target or spot is gone; the replacement link cannot carry it.
                if packet.opcode == 154
                    && let Some(armed) = self.armed.take()
                {
                    events.push((
                        packet.received,
                        Event::Receipt {
                            action: armed.ticket.id,
                            delivery: e::Delivery::Unknown,
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
                let mut facts = vec![e::Fact::InventorySettled(e::IngressMark(mark))];
                facts.extend(decoder.settle_carried(mark));
                // The page frontier also passes sends, other sockets and unforwarded frames.
                // Only an applied forwarded game frame shows the game link is live.
                if applied {
                    facts.push(e::Fact::StateCurrent);
                }
                events.push((
                    proof_at,
                    Event::Facts {
                        revision: e::EvidenceKey((mark << 2) | 2),
                        ingress: e::IngressMark(mark),
                        facts,
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
                    .expect_action(armed.ticket.action, p.mark, p.opcode == 477)?;
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
    #[cfg(test)]
    pub(crate) fn maxima(&self) -> [Option<u32>; 3] {
        self.decoder
            .as_ref()
            .or_else(|| self.startup.as_ref().map(|s| &s.decoder))
            .map_or([None; 3], Decoder::maxima)
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
        if !self.closed {
            self.expire_handoff()?;
            if let Some(event) = self.events.pop_front() {
                return Ok(Some(event));
            }
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
        self.handoff = None;
        self.startup = None;
        self.decoder = None;
        self.dig_cycle = None;
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
        self.handoff = None;
        self.startup = None;
        self.decoder = None;
        self.dig_cycle = None;
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
    fn observer(definitions: &std::sync::Arc<crate::definitions::GameDefinitions>) -> GameObserver {
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
        let profile = VerifiedProfile {
            first_host: "127.0.0.1".into(),
            first_port: 1234,
            marker: zeroize::Zeroizing::new("fixture".into()),
            client_version: zeroize::Zeroizing::new(vec![0; 4]),
            definitions: Some(definitions.clone()),
        };
        GameObserver::new(session, profile)
    }
    fn binding() -> crate::PageBinding {
        crate::PageBinding::new(7, "a".repeat(32)).unwrap()
    }
    fn login_header() -> [u8; 6] {
        let login = crate::codec::OpcodeMap::current(b"c2s")
            .encode(4, 0, &[0; 14])
            .unwrap();
        login[..6].try_into().unwrap()
    }
    /// Page messages numbered and fenced the way `page_bridge.js` posts them.
    #[derive(Default)]
    struct Page {
        sequence: u64,
        raw_ingress: u64,
    }
    impl Page {
        fn post(&mut self, observer: &mut GameObserver, kind: &str, fields: serde_json::Value) {
            self.sequence += 1;
            self.raw_ingress = self
                .raw_ingress
                .max(fields["ingress"].as_u64().unwrap_or(0));
            let mut value = serde_json::json!({"channel":"opentrickster.net.v1","epoch":"7","guard":"a".repeat(32),"sequence":self.sequence,"rawIngress":self.raw_ingress,"kind":kind});
            value
                .as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            observer
                .ingest_page(ORIGIN, &value.to_string(), std::time::Instant::now())
                .unwrap();
        }
    }
    /// The messages the generated page bridge posts for these inbound frames on one game
    /// socket after its login.
    fn bridged(script: &crate::ObserverScript, frames: &[Vec<u8>]) -> Vec<serde_json::Value> {
        use base64::{Engine, engine::general_purpose::STANDARD};
        use std::{
            io::Write,
            process::{Command, Stdio},
        };
        let frames: Vec<_> = frames.iter().map(|frame| STANDARD.encode(frame)).collect();
        let input = serde_json::json!({"script":script.as_str(),"frames":frames});
        let mut child = Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/page_stream.cjs"
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Node is required for page bridge behavior checks");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    /// The page path decodes what the real bridge forwards, the native path every frame.
    /// From the first frame to the last they hold the same maxima.
    #[test]
    fn a_page_bound_before_the_game_login_derives_the_native_maxima() {
        let session = crate::test_support::maxima_session();
        let incoming = crate::codec::OpcodeMap::current(b"s2c");
        let frames: Vec<Vec<u8>> = session
            .frames
            .iter()
            .map(|(opcode, aux, payload)| incoming.encode(*opcode, *aux, payload).unwrap().to_vec())
            .collect();
        let binding = binding();
        let mut messages = bridged(&crate::page::script(&binding).unwrap(), &frames).into_iter();
        let mut paged = observer(&session.definitions);
        paged.bind_page(binding).unwrap();
        let mut native = observer(&session.definitions);
        let key = ConnectionKey(1);
        native
            .connect(ObserverContext::checked(ORIGIN, crate::BUILD, key).unwrap())
            .unwrap();
        native.notice_game_login(key, login_header()).unwrap();
        let mut seen = Vec::new();
        for (i, frame) in frames.iter().enumerate() {
            native
                .observe_binary(key, ObservedDirection::Inbound, frame)
                .unwrap();
            // The login is page ingress 1, so inbound frame i is ingress i + 2.
            for message in messages.by_ref() {
                paged
                    .ingest_page(ORIGIN, &message.to_string(), std::time::Instant::now())
                    .unwrap();
                if message["kind"] == "frontier"
                    && message["ingress"].as_u64() >= Some(i as u64 + 2)
                {
                    break;
                }
            }
            let opcode = session.frames[i].0;
            assert_eq!(
                paged.maxima(),
                native.maxima(),
                "frame {i}, opcode {opcode}"
            );
            assert_eq!(
                paged.readiness().hp_maximum_known,
                native.readiness().hp_maximum_known
            );
            seen.push(native.maxima());
        }
        assert!(messages.next().is_none());
        assert!(paged.readiness().own_baseline_seen);
        assert_eq!(seen[session.h - 1], [None; 3]);
        assert!(seen[session.h].iter().all(Option::is_some));
        // The login soul frames after H cross with their payload and keep the maxima.
        assert_eq!(session.frames[session.h + 1].0, 1489);
        assert_eq!(session.frames[session.h + 2].0, 1487);
        assert!(seen[session.h + 2].iter().all(Option::is_some));
        let mut hp: Vec<u32> = seen.iter().filter_map(|m| m[0]).collect();
        hp.sort_unstable();
        hp.dedup();
        assert!(hp.len() >= 8, "the session moves the maxima: {hp:?}");
        let unknown = seen[session.h..].iter().filter(|m| m[0].is_none()).count();
        assert!(unknown >= 6, "the session reaches the unknown paths");
        // The trade frame crossed header-only and still withdrew them.
        assert_eq!(*seen.last().unwrap(), [None; 3]);
    }
    /// A decoder that predates the page cannot prove it saw the whole StartSet, so its
    /// maxima stay unknown. The next game login re-baselines through the page.
    #[test]
    fn a_page_bound_after_the_game_login_leaves_maxima_unknown_until_the_next_login() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        use serde_json::json;
        let session = crate::test_support::maxima_session();
        let incoming = crate::codec::OpcodeMap::current(b"s2c");
        let start = &session.frames[..=session.h];
        let encode = |(opcode, aux, payload): &(u16, u16, Vec<u8>)| {
            incoming.encode(*opcode, *aux, payload).unwrap()
        };
        let key = ConnectionKey(1);
        let log_in = |observer: &mut GameObserver| {
            observer
                .connect(ObserverContext::checked(ORIGIN, crate::BUILD, key).unwrap())
                .unwrap();
            observer.notice_game_login(key, login_header()).unwrap();
        };
        let mut native = observer(&session.definitions);
        log_in(&mut native);
        for frame in start {
            native
                .observe_binary(key, ObservedDirection::Inbound, &encode(frame))
                .unwrap();
        }
        let proven = native.maxima();
        assert!(proven.iter().all(Option::is_some));
        for at_h in [true, false] {
            let mut observer = observer(&session.definitions);
            log_in(&mut observer);
            // At H the 233 has arrived, but an Aux group holds it from the startup.
            let mut frames: Vec<_> = start.iter().map(encode).collect();
            if at_h {
                let (opcode, _, payload) = &start[session.h];
                frames[session.h] = incoming.encode(*opcode, 9, payload).unwrap();
            }
            for frame in &frames {
                observer
                    .observe_binary(key, ObservedDirection::Inbound, frame)
                    .unwrap();
            }
            assert_eq!(observer.readiness().own_baseline_seen, !at_h);
            assert_eq!(observer.maxima() == proven, !at_h);
            observer.bind_page(binding()).unwrap();
            assert_eq!(observer.maxima(), [None; 3]);
            let mut page = Page::default();
            let mut ingress = frames.len() as u64;
            if at_h {
                // The page releases the held 233 with the group's marker.
                page.post(
                    &mut observer,
                    "frontier",
                    json!({"ingress":ingress,"ageMs":0}),
                );
                ingress += 1;
                let marker = incoming.encode(1951, 9, &[]).unwrap();
                page.post(&mut observer,"frame",json!({"connection":1,"direction":"inbound","ingress":ingress,"ageMs":0,"data":STANDARD.encode(marker)}));
                page.post(
                    &mut observer,
                    "frontier",
                    json!({"ingress":ingress,"ageMs":0}),
                );
                assert!(observer.readiness().own_baseline_seen);
                assert_eq!(observer.maxima(), [None; 3]);
            }
            observer.disconnect(key).unwrap();
            page.post(&mut observer, "socket", json!({"connection":2}));
            ingress += 1;
            page.post(
                &mut observer,
                "game-login",
                json!({"connection":2,"ingress":ingress,"header":login_header()}),
            );
            for (opcode, aux, payload) in start {
                ingress += 1;
                let bare = crate::page::header_only(*opcode);
                let bytes = incoming
                    .encode(*opcode, *aux, if bare { &[] } else { payload })
                    .unwrap();
                page.post(&mut observer,"frame",json!({"connection":2,"direction":"inbound","ingress":ingress,"ageMs":0,"data":STANDARD.encode(bytes)}));
            }
            page.post(
                &mut observer,
                "frontier",
                json!({"ingress":ingress,"ageMs":0}),
            );
            assert!(observer.readiness().own_baseline_seen);
            assert_eq!(observer.maxima(), proven);
        }
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
    #[test]
    fn only_an_applied_inbound_game_frame_marks_assisted_state_current() {
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
            observer
                .ingest_page(ORIGIN, &value.to_string(), std::time::Instant::now())
                .unwrap();
        };
        let frame = |connection: u64, direction: &str, ingress: u64, bytes: &[u8]| json!({"connection":connection,"direction":direction,"ingress":ingress,"ageMs":0,"data":STANDARD.encode(bytes)});
        // Facts of the last frontier settlement (revision namespace 2) queued so far.
        let settled = |observer: &mut GameObserver| {
            let mut last = None;
            while let Some(observation) = observer.try_next().unwrap() {
                if let Event::Facts {
                    revision, facts, ..
                } = observation.event
                    && revision.0 & 3 == 2
                {
                    last = Some(facts);
                }
            }
            last.unwrap()
        };
        let outgoing = crate::codec::OpcodeMap::current(b"c2s");
        let incoming = crate::codec::OpcodeMap::current(b"s2c");
        ingest(&mut observer, "socket", json!({"connection":1}));
        let login = outgoing.encode(4, 0, &[0; 14]).unwrap();
        ingest(
            &mut observer,
            "game-login",
            json!({"connection":1,"ingress":1,"header":login[..6]}),
        );
        let mut profile = vec![0; 69];
        profile[17] = b'S';
        for ((opcode, payload), ingress) in [
            (5, vec![]),
            (679, profile),
            (230, vec![0; 10]),
            (233, vec![0; 251]),
        ]
        .into_iter()
        .zip(2..)
        {
            let bytes = incoming.encode(opcode, 0, &payload).unwrap();
            ingest(&mut observer, "frame", frame(1, "inbound", ingress, &bytes));
        }
        ingest(&mut observer, "frontier", json!({"ingress":5,"ageMs":0}));
        assert!(matches!(
            observer.try_next().unwrap().unwrap().event,
            Event::Baseline { .. }
        ));
        // The page frontier also passes our own sends and other sockets' frames. Neither shows
        // that the selected game link is delivering.
        ingest(&mut observer, "socket", json!({"connection":2}));
        let send = outgoing.encode(24, 0, &[0; 9]).unwrap();
        ingest(&mut observer, "frame", frame(1, "outbound", 6, &send));
        let other = incoming.encode(36, 0, &[0; 4]).unwrap();
        ingest(&mut observer, "frame", frame(2, "inbound", 7, &other));
        ingest(&mut observer, "frontier", json!({"ingress":7,"ageMs":0}));
        let facts = settled(&mut observer);
        assert!(
            facts
                .iter()
                .any(|f| matches!(f, e::Fact::InventorySettled(e::IngressMark(7))))
        );
        assert!(!facts.iter().any(|f| matches!(f, e::Fact::StateCurrent)));
        // Season-pass progress crosses the page as its bare header and is applied in order.
        let pass = incoming.encode(32572, 0, &[]).unwrap();
        ingest(&mut observer, "frame", frame(1, "inbound", 8, &pass));
        ingest(&mut observer, "frontier", json!({"ingress":8,"ageMs":0}));
        let facts = settled(&mut observer);
        assert!(
            facts
                .iter()
                .any(|f| matches!(f, e::Fact::InventorySettled(e::IngressMark(8))))
        );
        assert!(facts.iter().any(|f| matches!(f, e::Fact::StateCurrent)));
    }
    #[test]
    fn assisted_dig_cycle_waits_for_a_sample_that_reads_the_client_dig_feature() {
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
        let sample = crate::test_support::sample_value();
        let own = sample["snapshot"]["motion"]["own"].as_u64().unwrap() as u16;
        let zone = sample["snapshot"]["zone"].as_u64().unwrap() as u16;
        let mut sequence = 0;
        let mut ingest = |observer: &mut GameObserver, kind: &str, fields: serde_json::Value| {
            sequence += 1;
            let mut value = json!({"channel":"opentrickster.net.v1","epoch":"7","guard":"a".repeat(32),"sequence":sequence,"rawIngress":fields["ingress"].as_u64().unwrap_or(0),"kind":kind});
            value
                .as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            observer
                .ingest_page(ORIGIN, &value.to_string(), std::time::Instant::now())
                .unwrap();
        };
        ingest(&mut observer, "socket", json!({"connection":1}));
        let login = crate::codec::OpcodeMap::current(b"c2s")
            .encode(4, 0, &[0; 14])
            .unwrap();
        ingest(
            &mut observer,
            "game-login",
            json!({"connection":1,"ingress":1,"header":login[..6]}),
        );
        ingest(&mut observer, "frontier", json!({"ingress":1,"ageMs":0}));
        let incoming = crate::codec::OpcodeMap::current(b"s2c");
        let mut profile = vec![0; 69];
        profile[17] = b'S';
        let mut clock = vec![0; 10];
        clock[4..6].copy_from_slice(&own.to_le_bytes());
        let mut stats = vec![0; 251];
        stats[219..221].copy_from_slice(&zone.to_le_bytes());
        for (i, (opcode, payload)) in [(5, vec![]), (679, profile), (230, clock), (233, stats)]
            .into_iter()
            .enumerate()
        {
            let bytes = incoming.encode(opcode, 0, &payload).unwrap();
            ingest(
                &mut observer,
                "frame",
                json!({"connection":1,"direction":"inbound","ingress":i+2,"ageMs":0,"data":STANDARD.encode(bytes)}),
            );
        }
        ingest(&mut observer, "frontier", json!({"ingress":5,"ageMs":0}));
        let mut drained = Vec::new();
        while let Some((_, observation)) = observer.try_next_at(e::At(10000)).unwrap() {
            drained.push(observation.event);
        }
        assert!(matches!(
            drained.as_slice(),
            [Event::Baseline { support, .. }] if support.quiesce && !support.dig_cycle
        ));
        let mut request = 9;
        let mut dig_events = |observer: &mut GameObserver, snapshot: serde_json::Value| {
            request += 1;
            observer.sample_script(request, 11).unwrap();
            let mut fields = sample.clone();
            fields["request"] = json!(format!("{request:x}"));
            fields["snapshot"] = snapshot;
            ingest(observer, "sample", fields);
            let mut reported = Vec::new();
            while let Some((_, observation)) = observer.try_next_at(e::At(10000)).unwrap() {
                if let Event::DigCycle(available) = observation.event {
                    reported.push(available);
                }
            }
            reported
        };
        let present = sample["snapshot"].clone();
        let mut absent = present.clone();
        absent["dig"] = serde_json::Value::Null;
        let cold = json!({"version":1,"status":"unavailable","reason":"cold"});
        let none: [bool; 0] = [];
        assert_eq!(dig_events(&mut observer, absent.clone()), none);
        assert_eq!(dig_events(&mut observer, present.clone()), [true]);
        assert_eq!(dig_events(&mut observer, present), none);
        assert_eq!(dig_events(&mut observer, cold), none);
        assert_eq!(dig_events(&mut observer, absent), [false]);
    }
    /// A bound page whose game link (connection 1) has reached its baseline.
    struct OnlinePage {
        observer: GameObserver,
        sequence: u64,
        raw: u64,
    }
    const OWN: u16 = 0x0102;
    const ZONE: u16 = 41;
    impl OnlinePage {
        fn online(session: e::SessionKey) -> Self {
            use zeroize::Zeroizing;
            let profile = VerifiedProfile {
                first_host: "127.0.0.1".into(),
                first_port: 1234,
                marker: Zeroizing::new("fixture".into()),
                client_version: Zeroizing::new(vec![0; 4]),
                definitions: None,
            };
            let mut observer = GameObserver::new(session, profile);
            observer
                .bind_page(crate::PageBinding::new(7, "a".repeat(32)).unwrap())
                .unwrap();
            let mut page = Self {
                observer,
                sequence: 0,
                raw: 0,
            };
            page.ingest("socket", serde_json::json!({"connection":1}))
                .unwrap();
            let login = crate::codec::OpcodeMap::current(b"c2s")
                .encode(4, 0, &[0; 14])
                .unwrap();
            page.ingest(
                "game-login",
                serde_json::json!({"connection":1,"ingress":1,"header":login[..6]}),
            )
            .unwrap();
            let mut profile = vec![0; 69];
            profile[17] = b'S';
            let mut clock = vec![0; 10];
            clock[4..6].copy_from_slice(&OWN.to_le_bytes());
            let mut stats = vec![0; 251];
            stats[219..221].copy_from_slice(&9u16.to_le_bytes());
            for (mark, (opcode, payload)) in
                [(5, vec![]), (679, profile), (230, clock), (233, stats)]
                    .into_iter()
                    .enumerate()
            {
                page.frame(1, mark as u64 + 2, opcode, &payload).unwrap();
            }
            page.frontier(5);
            assert!(matches!(page.events().as_slice(), [Event::Baseline { .. }]));
            page
        }
        fn send(
            &mut self,
            kind: &str,
            fields: serde_json::Value,
            at: std::time::Instant,
        ) -> Result<Option<crate::PageNotice>> {
            self.sequence += 1;
            self.raw = self.raw.max(fields["ingress"].as_u64().unwrap_or(0));
            let mut value = serde_json::json!({"channel":"opentrickster.net.v1","epoch":"7","guard":"a".repeat(32),"sequence":self.sequence,"rawIngress":self.raw,"kind":kind});
            value
                .as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            self.observer.ingest_page(ORIGIN, &value.to_string(), at)
        }
        fn ingest(
            &mut self,
            kind: &str,
            fields: serde_json::Value,
        ) -> Result<Option<crate::PageNotice>> {
            self.send(kind, fields, std::time::Instant::now())
        }
        fn frame(
            &mut self,
            connection: u64,
            ingress: u64,
            opcode: u16,
            payload: &[u8],
        ) -> Result<()> {
            self.grouped(connection, ingress, opcode, 0, payload)
        }
        fn grouped(
            &mut self,
            connection: u64,
            ingress: u64,
            opcode: u16,
            aux: u16,
            payload: &[u8],
        ) -> Result<()> {
            use base64::{Engine, engine::general_purpose::STANDARD};
            let bytes = crate::codec::OpcodeMap::current(b"s2c")
                .encode(opcode, aux, payload)
                .unwrap();
            self.ingest("frame", serde_json::json!({"connection":connection,"direction":"inbound","ingress":ingress,"ageMs":0,"data":STANDARD.encode(bytes)}))
                .map(|_| ())
        }
        fn frontier(&mut self, ingress: u64) {
            self.ingest("frontier", serde_json::json!({"ingress":ingress,"ageMs":0}))
                .unwrap();
        }
        fn handoff_login(&mut self, connection: u64, ingress: u64) -> Result<()> {
            let login = crate::codec::OpcodeMap::current(b"c2s")
                .encode(156, 0, &[0; 13])
                .unwrap();
            self.ingest(
                "game-handoff",
                serde_json::json!({"connection":connection,"ingress":ingress,"header":login[..6],"ageMs":0}),
            )
            .map(|_| ())
        }
        /// Every event except the settlement each frontier release reports.
        fn events(&mut self) -> Vec<Event> {
            let mut events = Vec::new();
            while let Some((_, observation)) = self.observer.try_next_at(e::At(10_000)).unwrap() {
                if !matches!(&observation.event, Event::Facts { revision, .. } if revision.0 & 3 == 2)
                {
                    events.push(observation.event);
                }
            }
            events
        }
        /// The redacted s2c 154 the page bridge forwards.
        fn handoff(&mut self, ingress: u64) {
            let mut payload = vec![0; 28];
            payload[21..23].copy_from_slice(&ZONE.to_le_bytes());
            payload[23..25].copy_from_slice(&7u16.to_le_bytes());
            payload[25..27].copy_from_slice(&9u16.to_le_bytes());
            self.frame(1, ingress, 154, &payload).unwrap();
            self.frontier(ingress);
        }
    }
    fn entry(own: u16) -> Vec<u8> {
        [own.to_le_bytes(), 7u16.to_le_bytes(), 9u16.to_le_bytes()]
            .concat()
            .into_iter()
            .chain([0; 5])
            .collect()
    }
    #[test]
    fn a_zone_handoff_follows_the_client_onto_its_replacement_link() {
        let point = e::Point { x: 7, y: 9 };
        let (session, ticket) =
            crate::test_support::ticket(e::Action::MoveTo(e::Point { x: 12, y: 34 }));
        let mut page = OnlinePage::online(session);
        page.observer.arm_action(ticket.clone()).unwrap();
        page.handoff(6);
        let moved = page.events();
        let map = match moved.as_slice() {
            [
                Event::Facts { facts, .. },
                Event::Receipt {
                    action,
                    delivery: e::Delivery::Unknown,
                },
            ] if *action == ticket.id => match facts.as_slice() {
                [e::Fact::MapChanged, e::Fact::Position { map, point: at }] if *at == point => *map,
                _ => panic!("the handoff re-baselines the map and position"),
            },
            _ => panic!("map change facts, then the unsendable action resolves"),
        };
        assert_eq!(
            page.observer.decoder.as_ref().unwrap().sampling_identity(),
            None
        );
        page.ingest("socket", serde_json::json!({"connection":2}))
            .unwrap();
        page.ingest("socket-closed", serde_json::json!({"connection":1}))
            .unwrap();
        assert!(
            page.events().is_empty(),
            "the old link's close is not a loss"
        );
        page.handoff_login(2, 7).unwrap();
        page.frame(2, 8, 437, &entry(0x0304)).unwrap();
        page.frame(2, 9, 36, &17u32.to_le_bytes()).unwrap();
        page.frontier(9);
        let entered = page.events();
        assert!(matches!(
            entered.as_slice(),
            [Event::Facts { facts: position, .. }, Event::Facts { facts: hp, .. }]
                if matches!(position.as_slice(), [e::Fact::Position { map: m, point: at }] if *m == map && *at == point)
                    && matches!(hp.as_slice(), [e::Fact::Hp(_)])
        ));
        assert_eq!(
            page.observer.decoder.as_ref().unwrap().sampling_identity(),
            Some((0x0304, ZONE))
        );
        assert!(page.observer.readiness().own_baseline_seen);
    }
    #[test]
    fn a_replacement_login_before_the_old_link_closes_keeps_the_session() {
        let (session, _) = crate::test_support::ticket(e::Action::MoveTo(e::Point { x: 1, y: 1 }));
        let mut page = OnlinePage::online(session);
        page.handoff(6);
        page.events();
        page.ingest("socket", serde_json::json!({"connection":2}))
            .unwrap();
        page.handoff_login(2, 7).unwrap();
        page.ingest("socket-closed", serde_json::json!({"connection":1}))
            .unwrap();
        page.frame(2, 8, 437, &entry(OWN)).unwrap();
        page.frontier(8);
        assert!(matches!(page.events().as_slice(), [Event::Facts { .. }]));
    }
    #[test]
    fn a_handoff_with_no_replacement_login_in_time_is_lost() {
        let (session, _) = crate::test_support::ticket(e::Action::MoveTo(e::Point { x: 1, y: 1 }));
        let mut page = OnlinePage::online(session);
        page.handoff(6);
        page.events();
        let closed = std::time::Instant::now()
            .checked_sub(HANDOFF_WINDOW)
            .unwrap();
        page.send("socket-closed", serde_json::json!({"connection":1}), closed)
            .unwrap();
        assert!(matches!(page.events().as_slice(), [Event::Lost]));
        page.ingest("socket", serde_json::json!({"connection":2}))
            .unwrap();
        page.handoff_login(2, 7).unwrap();
        page.frame(2, 8, 437, &entry(OWN)).unwrap();
        page.frontier(8);
        assert!(
            page.events().is_empty(),
            "a late replacement stays unobserved"
        );
    }
    #[test]
    fn an_unfinished_aux_group_on_the_old_link_dies_with_it() {
        let (session, _) = crate::test_support::ticket(e::Action::MoveTo(e::Point { x: 1, y: 1 }));
        let mut page = OnlinePage::online(session);
        page.grouped(1, 6, 36, 5, &17u32.to_le_bytes()).unwrap();
        page.frontier(6);
        page.handoff(7);
        assert!(
            page.events().is_empty(),
            "the 154 waits behind the open group"
        );
        page.ingest("socket", serde_json::json!({"connection":2}))
            .unwrap();
        page.ingest("socket-closed", serde_json::json!({"connection":1}))
            .unwrap();
        page.handoff_login(2, 8).unwrap();
        page.frame(2, 9, 437, &entry(OWN)).unwrap();
        page.frontier(9);
        let events = page.events();
        assert!(matches!(
            events.as_slice(),
            [Event::Facts { ingress: e::IngressMark(7), facts: moved, .. }, Event::Facts { ingress: e::IngressMark(9), .. }]
                if matches!(moved.first(), Some(e::Fact::MapChanged))
        ));
    }
    #[test]
    fn handoff_flows_the_client_never_produces_fail_safe() {
        let (session, _) = crate::test_support::ticket(e::Action::MoveTo(e::Point { x: 1, y: 1 }));
        let mut unannounced = OnlinePage::online(session);
        unannounced
            .ingest("socket", serde_json::json!({"connection":2}))
            .unwrap();
        assert_eq!(unannounced.handoff_login(2, 6), Err(NetError::Uncertain));
        assert!(unannounced.observer.try_next().is_err());
        let mut late = OnlinePage::online(session);
        late.handoff(6);
        late.events();
        late.ingest("socket", serde_json::json!({"connection":2}))
            .unwrap();
        late.handoff_login(2, 7).unwrap();
        assert_eq!(
            late.frame(1, 8, 36, &17u32.to_le_bytes()),
            Err(NetError::Uncertain),
            "the client closed the old link before it logged in on the new one"
        );
        let mut dropped = OnlinePage::online(session);
        dropped
            .ingest("socket-closed", serde_json::json!({"connection":1}))
            .unwrap();
        assert!(matches!(dropped.events().as_slice(), [Event::Lost]));
        let mut unredacted = OnlinePage::online(session);
        let mut payload = vec![0; 28];
        payload[4] = 1;
        assert_eq!(
            unredacted.frame(1, 6, 154, &payload),
            Err(NetError::Malformed),
            "only Zone and Pos of a handoff may cross the page boundary"
        );
    }
}
