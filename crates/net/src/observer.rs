use crate::{
    NetError, Result, VerifiedProfile,
    codec::{Frames, MAX_PACKETS},
    decode::Decoder,
    profile::ORIGIN,
    session::Startup,
    wire::Reader,
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
    selected: bool,
}
/// Passive decoder. It cannot open a socket, authenticate, send a frame or acknowledge bytes.
pub struct GameObserver {
    page: Option<crate::page::State>,
    page_frontier: u64,
    page_seen: std::collections::BTreeSet<u64>,
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
    seen: Vec<e::ActionKey>,
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
            seen: Vec::new(),
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
        self.page
            .as_mut()
            .ok_or(NetError::WrongPhase)?
            .close_script(request)
    }
    /// `origin` comes from the authenticated native frame callback, never from JSON.
    /// Preserve the Wry callback Instant when queuing the message for a later UI pump.
    pub fn ingest_page(
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
        let connection = crate::page::connection(&value)?;
        match value["kind"].as_str() {
            Some("socket") => {
                self.connect(ObserverContext::checked(origin, crate::BUILD, connection)?)?
            }
            Some("socket-closed") => self.disconnect(connection)?,
            Some("frontier") => {
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
                let support = crate::session::support(&startup.decoder)?;
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
        if self.seen.contains(&ticket.id) {
            return Err(NetError::DuplicateAction);
        }
        if self.seen.len() >= MAX_PACKETS {
            return self.fail(NetError::Capacity);
        }
        let decoder = self.decoder.as_ref().ok_or(NetError::WrongPhase)?;
        match ticket.action {
            e::Action::MoveTo(point) => {
                crate::actions::coordinate(point)?;
            }
            e::Action::BasicAttack(key) => {
                decoder.raw_target(key)?;
            }
            _ => return Err(NetError::Unsupported),
        }
        self.seen.push(ticket.id);
        self.armed = Some(Armed {
            ticket,
            selected: false,
        });
        Ok(())
    }
    fn actual_send(&mut self, p: &crate::codec::Packet) -> Result<()> {
        let Some(armed) = &mut self.armed else {
            return Ok(());
        };
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
                        armed.selected = true;
                    }
                    false
                } else if p.opcode == 29 {
                    let mode = r.u8()?;
                    r.end()?;
                    let (_, expected) = crate::actions::normal_attack(target, mode);
                    actual == target
                        && armed.selected
                        && expected.0.as_slice() == p.payload.as_slice()
                } else {
                    let crc = r.u16()?;
                    let bunch = r.u16()?;
                    let mode = r.u8()?;
                    r.end()?;
                    actual == target
                        && crate::actions::gun_shot(target, crc, bunch, mode)
                            .0
                            .as_slice()
                            == p.payload.as_slice()
                }
            }
            _ => false,
        };
        if matched {
            let armed = self.armed.take().ok_or(NetError::WrongPhase)?;
            self.emit(Event::Receipt {
                action: armed.ticket.id,
                delivery: e::Delivery::Sent(e::IngressMark(p.mark)),
            })?;
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
}
