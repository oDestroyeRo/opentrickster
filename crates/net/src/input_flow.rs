use crate::{
    InputFences, InputStepKey, NetError, NormalInput, Result,
    sampler::{self, Candidate, DigPhase, Snapshot},
};
use opentrickster_engine::{self as e, Action, ActionTicket, At};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Prepare,
    KeyRelease,
    WorldRelease,
    AwaitDig,
    Charging,
    FirstCoast,
    Tracking,
    Waiting,
    Cleanup,
    Done,
}
struct Issued {
    key: InputStepKey,
    event: NormalInput,
    at: Option<Instant>,
}
pub(crate) struct Flow {
    pub(crate) ticket: ActionTicket,
    host: u64,
    layout: u64,
    revision: u64,
    zone: u16,
    phase: Phase,
    issued: Option<Issued>,
    held: Option<NormalInput>,
    pub(crate) armed: bool,
    pub(crate) submitted: bool,
    pub(crate) sent: Option<u64>,
    sent_at: Option<At>,
    accepted: bool,
    spot: bool,
    completed: bool,
    cancel_sent: bool,
    draining: bool,
    cleanup_clicks: u8,
}
impl Flow {
    pub(crate) fn new(ticket: ActionTicket, s: &Snapshot) -> Self {
        Self {
            ticket,
            host: s.host_epoch,
            layout: s.layout_epoch,
            revision: s.revision,
            zone: s.zone,
            phase: Phase::Prepare,
            issued: None,
            held: None,
            armed: false,
            submitted: false,
            sent: None,
            sent_at: None,
            accepted: false,
            spot: false,
            completed: false,
            cancel_sent: false,
            draining: false,
            cleanup_clicks: 0,
        }
    }
    pub(crate) fn issue(&mut self, key: InputStepKey, event: NormalInput) {
        self.issued = Some(Issued {
            key,
            event,
            at: None,
        });
    }
    pub(crate) fn dispatched(&mut self, key: InputStepKey, at: Instant) -> Result<bool> {
        let issued = self
            .issued
            .as_mut()
            .filter(|p| p.key == key && p.at.is_none())
            .ok_or(NetError::StaleConnection)?;
        if at > Instant::now() {
            return Err(NetError::Malformed);
        }
        issued.at = Some(at);
        match issued.event {
            NormalInput::LeftDown | NormalInput::RightDown | NormalInput::KeyDown(_) => {
                self.held = Some(issued.event)
            }
            NormalInput::LeftUp | NormalInput::RightUp | NormalInput::KeyUp(_) => self.held = None,
            NormalInput::PointerMove { .. } => {}
        }
        let first = !self.submitted && !matches!(issued.event, NormalInput::PointerMove { .. });
        self.submitted |= first;
        Ok(first)
    }
    pub(crate) fn observe_send(&mut self, opcode: u16, mark: u64) {
        if opcode == 44 {
            self.sent = Some(mark);
        }
        if matches!(opcode, 24 | 29 | 477) && !matches!(self.ticket.action, Action::DigAt(_)) {
            self.sent = Some(mark);
        }
        if opcode == 48 && self.sent.is_some_and(|floor| mark > floor) {
            self.cancel_sent = true;
            self.phase = Phase::Cleanup;
        }
        if opcode == 49 && self.sent.is_some_and(|floor| mark > floor) {
            self.phase = Phase::Cleanup;
        }
    }
    pub(crate) fn observe_facts(&mut self, mark: u64, opcode: u16, facts: &[e::Fact]) {
        if !matches!(self.ticket.action, Action::DigAt(_))
            || !self.sent.is_some_and(|floor| mark > floor)
        {
            return;
        }
        for fact in facts {
            match fact {
                e::Fact::DigAccepted => self.accepted = true,
                e::Fact::DigSpot(point) if matches!(self.ticket.action,Action::DigAt(expected) if expected==*point) => {
                    self.spot = true
                }
                _ => {}
            }
        }
        if opcode == 50 {
            self.completed = true;
            self.phase = Phase::Cleanup;
            if self
                .issued
                .as_ref()
                .is_some_and(|issued| issued.at.is_none())
            {
                self.issued = None;
            }
        }
    }
    pub(crate) fn quiesce(&mut self) {
        self.draining = true;
        self.phase = Phase::Cleanup;
        if self
            .issued
            .as_ref()
            .is_some_and(|issued| issued.at.is_none())
        {
            self.issued = None;
        }
    }
    pub(crate) fn done(&self) -> bool {
        self.phase == Phase::Done
    }
    /// Before the actual send, the ticket's receipt deadline bounds the flow. After it, the
    /// engine's completion window for the action applies, measured from when the send was seen.
    pub(crate) fn deadline(&mut self, now: At) -> u64 {
        if self.draining {
            return u64::MAX;
        }
        if self.sent.is_some() && self.sent_at.is_none() {
            self.sent_at = Some(now);
        }
        match self.sent_at {
            None => self.ticket.not_after.0,
            Some(at) => {
                at.0.saturating_add(if matches!(self.ticket.action, Action::DigAt(_)) {
                    e::DIG_MS
                } else {
                    e::ACTION_MS
                })
            }
        }
    }
    pub(crate) fn gauge_hold(&self) -> bool {
        matches!(self.phase, Phase::Charging | Phase::Tracking)
    }
    pub(crate) fn emergency_released(&mut self, at: Instant) -> Result<()> {
        if at > Instant::now() {
            return Err(NetError::Malformed);
        }
        self.held = None;
        self.issued = None;
        self.draining = true;
        self.phase = Phase::Cleanup;
        Ok(())
    }
    fn up(&self) -> Option<NormalInput> {
        self.held.and_then(|event| match event {
            NormalInput::LeftDown => Some(NormalInput::LeftUp),
            NormalInput::RightDown => Some(NormalInput::RightUp),
            NormalInput::KeyDown(k) => Some(NormalInput::KeyUp(k)),
            _ => None,
        })
    }
    fn one(event: NormalInput) -> Candidate {
        Candidate {
            steps: vec![event],
            requires_resample: false,
        }
    }
    pub(crate) fn next(
        &mut self,
        s: Option<&Snapshot>,
        now: At,
        fences: InputFences,
        session: e::SessionKey,
        target: Option<u16>,
        pending: bool,
    ) -> Result<Option<Candidate>> {
        if self.host != fences.host_epoch
            || self.layout != fences.layout_epoch
            || now >= fences.not_after
        {
            return Err(NetError::StaleConnection);
        }
        if self
            .issued
            .as_ref()
            .is_some_and(|issued| issued.at.is_none())
        {
            return Ok(None);
        }
        if self.draining
            && let Some(up) = self.up()
        {
            self.issued = None;
            return Ok(Some(Self::one(up)));
        }
        if now.0 >= self.deadline(now) {
            return Err(NetError::Timeout);
        }
        if let Some(issued) = &self.issued {
            let Some(at) = issued.at else {
                return Ok(None);
            };
            let proof = s
                .filter(|s| s.captured > at)
                .map(|s| sampler::press_sampled(s, &issued.event))
                .transpose()?
                .unwrap_or(false);
            match issued.event {
                NormalInput::PointerMove { .. } => {
                    if !s.is_some_and(|s| s.captured > at) {
                        return Ok(None);
                    }
                }
                NormalInput::LeftDown
                    if !matches!(self.phase, Phase::Charging | Phase::Tracking) =>
                {
                    if !proof && self.sent.is_none() {
                        return Ok(None);
                    }
                    self.phase = Phase::WorldRelease;
                    return Ok(Some(Self::one(NormalInput::LeftUp)));
                }
                NormalInput::LeftDown => {
                    if !proof {
                        return Ok(None);
                    }
                }
                NormalInput::RightDown => {
                    let cancelled = s.is_some_and(|s| {
                        s.dig
                            .as_ref()
                            .is_none_or(|d| d.fsm != 5 && d.flags & 4 == 0)
                    });
                    if !self.cancel_sent && !(proof && cancelled) {
                        return Ok(None);
                    }
                    return Ok(Some(Self::one(NormalInput::RightUp)));
                }
                NormalInput::KeyDown(key) => {
                    if !proof {
                        return Ok(None);
                    }
                    self.phase = Phase::KeyRelease;
                    return Ok(Some(Self::one(NormalInput::KeyUp(key))));
                }
                NormalInput::LeftUp | NormalInput::RightUp | NormalInput::KeyUp(_) => {
                    if !proof {
                        return Ok(None);
                    }
                }
            }
            self.issued = None;
        }
        let Some(s) = s else {
            return Ok(None);
        };
        if s.host_epoch != self.host
            || s.layout_epoch != self.layout
            || s.zone != self.zone
            || s.revision != self.revision
        {
            return Err(NetError::StaleConnection);
        }
        if self.completed {
            self.phase = Phase::Cleanup;
        }
        match self.phase {
            Phase::Prepare => {
                sampler::plan(s, now, fences, session, &self.ticket, target).map(Some)
            }
            Phase::KeyRelease => {
                if s.dig.as_ref().is_none_or(|d| d.flags & 4 == 0) {
                    return Ok(None);
                }
                self.phase = Phase::Prepare;
                sampler::plan(s, now, fences, session, &self.ticket, target).map(Some)
            }
            Phase::WorldRelease => {
                if matches!(self.ticket.action, Action::DigAt(_)) {
                    self.phase = Phase::AwaitDig;
                } else {
                    self.phase = Phase::Waiting;
                }
                Ok(None)
            }
            Phase::AwaitDig => {
                if !self.accepted || !self.spot {
                    return Ok(None);
                }
                self.phase = Phase::Charging;
                self.gauge(s)
            }
            Phase::Charging | Phase::Tracking => self.gauge(s),
            Phase::FirstCoast => {
                if s.dig.as_ref().is_some_and(|d| d.stage == 5) {
                    return Ok(None);
                }
                self.phase = Phase::Tracking;
                self.gauge(s)
            }
            Phase::Waiting => {
                if self.sent.is_some() && !pending && sampler::idle(s) {
                    self.phase = Phase::Done;
                }
                Ok(None)
            }
            Phase::Cleanup => {
                if sampler::idle(s)
                    && (!matches!(self.ticket.action, Action::DigAt(_))
                        || self.sent.is_none()
                        || self.completed)
                    && !pending
                {
                    self.phase = Phase::Done;
                    return Ok(None);
                }
                if !matches!(self.ticket.action, Action::DigAt(_))
                    || s.dig
                        .as_ref()
                        .is_none_or(|d| d.fsm != 5 && d.flags & 4 == 0)
                {
                    return Ok(None);
                }
                if self.cleanup_clicks >= 2 {
                    return Err(NetError::Uncertain);
                }
                if s.pointer.over_ui
                    || s.pointer.world.is_none()
                    || s.input.modifiers.iter().any(|v| *v)
                    || s.input.left
                    || s.input.right
                {
                    return Err(NetError::Refused);
                }
                self.cleanup_clicks += 1;
                self.cancel_sent = false;
                Ok(Some(Self::one(NormalInput::RightDown)))
            }
            Phase::Done => Ok(None),
        }
    }
    fn gauge(&mut self, s: &Snapshot) -> Result<Option<Candidate>> {
        let charging = self.phase == Phase::Charging;
        let desired = sampler::dig_button(
            s,
            if charging {
                DigPhase::Charging
            } else {
                DigPhase::Tracking
            },
        )?;
        if charging && s.dig.as_ref().is_some_and(|d| d.stage == 5) {
            self.phase = Phase::FirstCoast;
        }
        match desired {
            Some(held) if held != self.held.is_some() => Ok(Some(Self::one(if held {
                NormalInput::LeftDown
            } else {
                NormalInput::LeftUp
            }))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dig(s: &mut Snapshot) -> &mut crate::sampler::DigState {
        s.dig
            .as_mut()
            .expect("the invented heap provides the drill feature")
    }
    #[test]
    fn continuation_after_the_actual_send_uses_the_completion_window() {
        let value = crate::test_support::sample_value();
        let s = crate::sampler::parse_snapshot(&value, Instant::now()).unwrap();
        for (action, opcode, window) in [
            (Action::DigAt(e::Point { x: 540, y: 350 }), 44, e::DIG_MS),
            (
                Action::MoveTo(e::Point { x: 540, y: 350 }),
                24,
                e::ACTION_MS,
            ),
        ] {
            let (session, ticket) = crate::test_support::ticket(action);
            let receipt = ticket.not_after.0;
            let mut flow = Flow::new(ticket, &s);
            assert_eq!(flow.deadline(At(10)), receipt);
            flow.observe_send(opcode, 5);
            assert_eq!(flow.deadline(At(400)), 400 + window);
            assert_eq!(flow.deadline(At(receipt + 100)), 400 + window);
            assert!(400 + window > receipt);
            let fences = crate::InputFences {
                host_epoch: 7,
                layout_epoch: 11,
                not_after: At(u64::MAX),
            };
            let late = flow.next(Some(&s), At(receipt + 100), fences, session, None, false);
            assert!(!matches!(late, Err(NetError::Timeout)), "{action:?}");
            let expired = flow.next(Some(&s), At(400 + window), fences, session, None, false);
            assert!(matches!(expired, Err(NetError::Timeout)), "{action:?}");
        }
    }
    #[test]
    fn dig_keeps_one_ticket_and_uses_observed_hold_release_and_first_coast() {
        let value = crate::test_support::sample_value();
        let clock = std::cell::Cell::new(Instant::now() - std::time::Duration::from_millis(100));
        let next_capture = || {
            let at = clock.get() + std::time::Duration::from_millis(1);
            clock.set(at);
            at
        };
        let mut s = crate::sampler::parse_snapshot(&value, next_capture()).unwrap();
        let point = e::Point { x: 540, y: 350 };
        let (session, ticket) = crate::test_support::ticket(Action::DigAt(point));
        let action = ticket.id;
        let mut flow = Flow::new(ticket, &s);
        let fences = crate::InputFences {
            host_epoch: 7,
            layout_epoch: 11,
            not_after: At(1000),
        };
        let mut step = 0;
        let mut dispatch = |flow: &mut Flow, event| {
            step += 1;
            let key = InputStepKey(step);
            flow.issue(key, event);
            flow.dispatched(key, next_capture()).unwrap();
        };
        let key = flow
            .next(Some(&s), At(10), fences, session, None, false)
            .unwrap()
            .unwrap()
            .steps[0];
        assert!(matches!(key, NormalInput::KeyDown(_)));
        dispatch(&mut flow, key);
        assert!(
            flow.next(None, At(11), fences, session, None, false)
                .unwrap()
                .is_none()
        );
        s.input.dig_held = true;
        s.captured = clock.get();
        assert!(
            flow.next(Some(&s), At(12), fences, session, None, false)
                .unwrap()
                .is_none()
        );
        s.captured = next_capture();
        let up = flow
            .next(Some(&s), At(12), fences, session, None, false)
            .unwrap()
            .unwrap()
            .steps[0];
        assert!(matches!(up, NormalInput::KeyUp(_)));
        dispatch(&mut flow, up);
        s.input.dig_held = false;
        dig(&mut s).flags = 4;
        s.pointer.world = Some([540.0, 350.0]);
        s.pointer.picked = None;
        s.pointer.ground_hover = false;
        s.captured = next_capture();
        let down = flow
            .next(Some(&s), At(13), fences, session, None, false)
            .unwrap()
            .unwrap()
            .steps[0];
        assert_eq!(down, NormalInput::LeftDown);
        dispatch(&mut flow, down);
        flow.observe_send(44, 20);
        flow.observe_facts(21, 45, &[e::Fact::DigAccepted]);
        flow.observe_facts(22, 371, &[e::Fact::DigSpot(point)]);
        let up = flow
            .next(None, At(14), fences, session, None, false)
            .unwrap()
            .unwrap()
            .steps[0];
        assert_eq!(up, NormalInput::LeftUp);
        dispatch(&mut flow, up);
        s.input.left = false;
        s.motion.screen_held = false;
        dig(&mut s).fsm = 5;
        dig(&mut s).in_state = true;
        dig(&mut s).stage = 2;
        dig(&mut s).area = Some(70);
        dig(&mut s).held = false;
        s.captured = next_capture();
        assert!(
            flow.next(Some(&s), At(15), fences, session, None, false)
                .unwrap()
                .is_none()
        );
        let down = flow
            .next(Some(&s), At(16), fences, session, None, false)
            .unwrap()
            .unwrap()
            .steps[0];
        assert_eq!(down, NormalInput::LeftDown);
        dispatch(&mut flow, down);
        s.input.left = true;
        dig(&mut s).held = true;
        dig(&mut s).stage = 3;
        s.captured = next_capture();
        assert!(
            flow.next(Some(&s), At(17), fences, session, None, false)
                .unwrap()
                .is_none()
        );
        dig(&mut s).stage = 5;
        dig(&mut s).gauge = 120.0;
        s.captured = next_capture();
        let up = flow
            .next(Some(&s), At(18), fences, session, None, false)
            .unwrap()
            .unwrap()
            .steps[0];
        assert_eq!(up, NormalInput::LeftUp);
        dispatch(&mut flow, up);
        s.input.left = false;
        dig(&mut s).held = false;
        s.captured = next_capture();
        assert!(
            flow.next(Some(&s), At(19), fences, session, None, false)
                .unwrap()
                .is_none()
        );
        dig(&mut s).stage = 6;
        s.captured = next_capture();
        assert_eq!(
            flow.next(Some(&s), At(20), fences, session, None, false)
                .unwrap()
                .unwrap()
                .steps,
            [NormalInput::LeftDown]
        );
        assert_eq!(flow.ticket.id, action);
        assert_eq!(flow.sent, Some(20));
        assert!(!flow.done());
    }
}
