use super::{
    host::EmbeddedHost,
    normal_input::{Context, Executor, Plan, Target},
};
use opentrickster_engine::{
    ActionTicket, At, BarrierKey, Delivery, Event, SEND_MARGIN_MS, SessionKey,
};
use opentrickster_net::{GameObserver, InputFences, InputPlan, InputStepKey, NetError};
use std::time::{Duration, Instant};

const SAMPLE_AGE: Duration = Duration::from_millis(250);
const SAMPLE_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, Copy)]
enum Gate {
    Open,
    Draining {
        barrier: BarrierKey,
        deadline: At,
        started: bool,
    },
    Closing,
}
enum Phase {
    Planning,
    Continuing,
}
struct Flight {
    ticket: ActionTicket,
    phase: Phase,
    emergency_notified: bool,
    fault_reported: bool,
    press_attempted: bool,
    admitted: bool,
    context: Context,
}

/// Local request bookkeeping does not interpret the sampler payload or infer Unity receipt.
#[derive(Default)]
struct Samples {
    next: u64,
    pending: Option<(u64, Instant)>,
    valid_at: Option<Instant>,
    requested_at: Option<Instant>,
    invalid: bool,
}
impl Samples {
    fn notice(&mut self, request: u64, available: bool, ingress: Instant) {
        if self.pending.is_none_or(|(expected, _)| expected != request) {
            return;
        }
        self.pending = None;
        self.invalid = !available;
        self.valid_at = available.then_some(ingress);
    }
    fn invalidate(&mut self) {
        self.pending = None;
        self.valid_at = None;
        self.requested_at = None;
        self.invalid = false;
    }
    fn due(&self) -> bool {
        self.pending
            .is_none_or(|(_, at)| at.elapsed() >= SAMPLE_AGE)
            && self
                .requested_at
                .is_none_or(|at| at.elapsed() >= SAMPLE_INTERVAL)
    }
    fn unsafe_to_hold(&self, dispatched: Option<Instant>, now: Instant) -> bool {
        self.invalid
            || self
                .valid_at
                .or(dispatched)
                .is_some_and(|at| now.saturating_duration_since(at) >= SAMPLE_AGE)
    }
}

pub(super) struct EmbeddedInput {
    executor: Executor<InputStepKey>,
    flight: Option<Flight>,
    gate: Gate,
    samples: Samples,
    fenced: bool,
    dispatched_at: Option<Instant>,
}
impl EmbeddedInput {
    pub fn new(session: SessionKey, host: &EmbeddedHost) -> Self {
        Self {
            executor: Executor::new(session.value(), host.input.borrow().context()),
            flight: None,
            gate: Gate::Open,
            samples: Samples::default(),
            fenced: false,
            dispatched_at: None,
        }
    }
    pub fn start(&mut self, ticket: ActionTicket, host: &EmbeddedHost) -> Result<(), NetError> {
        if !matches!(self.gate, Gate::Open) || self.flight.is_some() {
            return Err(NetError::Busy);
        }
        self.executor.finish().map_err(|_| NetError::Busy)?;
        let context = host.input.borrow().context();
        self.fenced = false;
        self.dispatched_at = None;
        self.samples.invalidate();
        self.flight = Some(Flight {
            ticket,
            phase: Phase::Planning,
            emergency_notified: false,
            fault_reported: false,
            press_attempted: false,
            admitted: false,
            context,
        });
        Ok(())
    }
    pub fn sample_notice(&mut self, request: u64, available: bool, ingress: Instant) {
        self.samples.notice(request, available, ingress);
    }
    pub fn quiesce(&mut self, barrier: BarrierKey, now: At) {
        self.gate = Gate::Draining {
            barrier,
            deadline: At(now.0.saturating_add(10_000)),
            started: false,
        };
        self.samples.invalidate();
        if let Some(flight) = &mut self.flight {
            flight.phase = Phase::Continuing;
        }
    }
    pub fn fence(&mut self) {
        self.fenced = true;
        self.executor.cancel();
        self.samples.invalidate();
    }
    pub fn close(&mut self) {
        self.gate = Gate::Closing;
        self.fence();
    }
    pub fn controls_released(&self) -> bool {
        self.executor.controls_released()
    }

    pub fn pump(
        &mut self,
        now: At,
        host: &EmbeddedHost,
        observer: &mut GameObserver,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        self.cleanup(host, observer, &mut events);
        if matches!(self.gate, Gate::Closing) {
            return events;
        }
        if let Some(flight) = &mut self.flight {
            let context = host.input.borrow().context();
            if !flight.admitted && flight.context != context {
                flight.context = context;
                self.samples.invalidate();
            }
        }

        if let Some(flight) = &self.flight {
            let source_lost = !self.executor.controls_released()
                && self
                    .samples
                    .unsafe_to_hold(self.dispatched_at, Instant::now());
            // A press planned this close to the receipt deadline could not report in time.
            let expired = matches!(flight.phase, Phase::Planning)
                && now.0.saturating_add(SEND_MARGIN_MS) >= flight.ticket.not_after.0;
            if source_lost || expired {
                self.fence();
                self.cleanup(host, observer, &mut events);
            }
        }

        let fences = self.fences(now, host);
        let plan = match self.gate {
            Gate::Draining {
                barrier,
                deadline,
                started,
            } if !started => {
                self.gate = Gate::Draining {
                    barrier,
                    deadline,
                    started: true,
                };
                observer.quiesce(now, fences, barrier)
            }
            Gate::Draining { barrier, .. } if observer.active_input().is_none() => {
                observer.quiesce(now, fences, barrier)
            }
            Gate::Draining { .. } => match observer.active_input() {
                Some(action) => observer.continue_input(now, fences, action),
                None => Ok(None),
            },
            _ => match self.flight.as_ref() {
                Some(flight)
                    if matches!(flight.phase, Phase::Planning)
                        && !self.fenced
                        && self.samples.valid_at.is_some()
                        && host.input.borrow().context().focused =>
                {
                    observer
                        .input_plan(now, fences, flight.ticket.clone())
                        .map(Some)
                }
                Some(flight) if observer.active_input() == Some(flight.ticket.id) => {
                    observer.continue_input(now, fences, flight.ticket.id)
                }
                _ => Ok(None),
            },
        };
        match plan {
            Ok(Some(plan)) if !self.fenced => self.dispatch(plan, now, host, observer, &mut events),
            Ok(Some(_)) => {}
            Ok(None) | Err(NetError::NotReady) => {}
            Err(_) => {
                self.fence();
                self.cleanup(host, observer, &mut events);
            }
        }

        if self.flight.is_some()
            && observer.active_input().is_none()
            && self.executor.controls_released()
            && (self.fenced
                || self
                    .flight
                    .as_ref()
                    .is_some_and(|flight| matches!(flight.phase, Phase::Continuing)))
        {
            let _ = self.executor.finish();
            self.flight = None;
        }
        if self.samples.due() {
            if self.samples.next == u64::MAX {
                self.fence();
                return events;
            }
            self.samples.next += 1;
            let request = self.samples.next;
            if let Ok(script) = observer.sample_script(request, host.input.borrow().layout.get()) {
                let submitted = host
                    .child
                    .borrow()
                    .as_ref()
                    .is_some_and(|web| web.evaluate_script(script.as_str()).is_ok());
                if submitted {
                    let at = Instant::now();
                    self.samples.pending = Some((request, at));
                    self.samples.requested_at = Some(at);
                } else {
                    self.fence();
                }
            } else if self.flight.is_some() {
                self.fence();
            }
        }
        events
    }

    fn fences(&self, now: At, host: &EmbeddedHost) -> InputFences {
        let deadline = match self.gate {
            Gate::Draining { deadline, .. } => deadline,
            _ => At(now.0.saturating_add(250)),
        };
        InputFences {
            host_epoch: host.epoch.0,
            layout_epoch: host.input.borrow().layout.get(),
            not_after: At(deadline.0.min(now.0.saturating_add(250))),
        }
    }

    fn dispatch(
        &mut self,
        plan: InputPlan,
        now: At,
        host: &EmbeddedHost,
        observer: &mut GameObserver,
        events: &mut Vec<Event>,
    ) {
        if plan.steps().len() != 1 {
            self.fence();
            return;
        }
        if let Some(flight) = &mut self.flight
            && !flight.admitted
        {
            if self
                .executor
                .begin(flight.ticket.id.value(), host.input.borrow().context())
                .is_err()
            {
                self.fence();
                return;
            }
            flight.admitted = true;
        }
        let fences = plan.fences();
        let command = Plan {
            session: plan.session().value(),
            action: plan.action().value(),
            step: plan.step_key(),
            host: fences.host_epoch,
            layout: fences.layout_epoch,
            not_after: fences.not_after,
            release_by: plan.release_by(),
            steps: plan.steps(),
        };
        let result = self
            .executor
            .execute(command, now, &mut *host.input.borrow_mut());
        if let Some(at) = self.executor.last_dispatch() {
            self.samples.invalidate();
            self.dispatched_at = Some(at);
            if let Some(flight) = &mut self.flight {
                flight.phase = if plan.requires_resample() {
                    Phase::Planning
                } else {
                    Phase::Continuing
                };
                flight.press_attempted |= matches!(
                    plan.steps()[0],
                    opentrickster_net::NormalInput::LeftDown
                        | opentrickster_net::NormalInput::RightDown
                        | opentrickster_net::NormalInput::KeyDown(_)
                );
            }
            if observer.input_dispatched(plan.step_key(), at).is_err() {
                self.fence();
            }
        }
        if result.is_err() || self.executor.uncertain() {
            // An attempted down can be ambiguous even when native code could not return a receipt.
            if let Some(flight) = &mut self.flight {
                flight.press_attempted |= !self.executor.controls_released();
            }
            self.fence();
            self.cleanup(host, observer, events);
        }
    }

    fn cleanup(
        &mut self,
        host: &EmbeddedHost,
        observer: &mut GameObserver,
        events: &mut Vec<Event>,
    ) {
        let releases = self
            .executor
            .cleanup(Instant::now(), &mut *host.input.borrow_mut());
        if self.executor.uncertain() {
            self.fenced = true;
        }
        if !self.fenced || !self.executor.controls_released() {
            return;
        }
        let Some(flight) = &mut self.flight else {
            return;
        };
        if !flight.emergency_notified {
            let at = releases.last().copied().unwrap_or_else(Instant::now);
            if observer.active_input() == Some(flight.ticket.id) {
                // Errors retain the flight and its resource; no idle or close is inferred.
                if observer.emergency_released(flight.ticket.id, at).is_err() {
                    return;
                }
            }
            flight.emergency_notified = true;
            self.samples.invalidate();
            self.dispatched_at = Some(at);
        }
        if !flight.fault_reported {
            flight.fault_reported = true;
            events.push(Event::Receipt {
                action: flight.ticket.id,
                delivery: if flight.press_attempted {
                    Delivery::Unknown
                } else {
                    Delivery::NotSent
                },
            });
        }
    }
    pub fn barrier_observed(&mut self, barrier: BarrierKey) -> bool {
        if !self.controls_released() {
            return false;
        }
        if matches!(self.gate, Gate::Draining { barrier: expected, .. } if expected == barrier) {
            let _ = self.executor.finish();
            self.flight = None;
            self.dispatched_at = None;
            self.gate = Gate::Open;
            self.fenced = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_pre_dispatch_callback_cannot_replace_the_new_guarded_sample() {
        let mut samples = Samples::default();
        let before = Instant::now();
        samples.pending = Some((1, before));
        samples.invalidate();
        samples.pending = Some((2, Instant::now()));
        samples.notice(1, true, Instant::now());
        assert!(samples.valid_at.is_none());
        assert_eq!(samples.pending.map(|(id, _)| id), Some(2));
        let after = Instant::now();
        samples.notice(2, true, after);
        assert_eq!(samples.valid_at, Some(after));
        assert!(samples.pending.is_none());
    }
    #[test]
    fn gauge_hold_loses_admission_on_missing_stale_or_failed_post_dispatch_sampling() {
        let mut samples = Samples::default();
        let dispatched = Instant::now();
        samples.pending = Some((2, dispatched));
        samples.notice(1, true, dispatched + Duration::from_millis(240));
        assert!(samples.unsafe_to_hold(Some(dispatched), dispatched + SAMPLE_AGE));
        let sampled = dispatched + Duration::from_millis(200);
        samples.notice(2, true, sampled);
        assert!(!samples.unsafe_to_hold(Some(dispatched), dispatched + SAMPLE_AGE));
        assert!(samples.unsafe_to_hold(Some(dispatched), sampled + SAMPLE_AGE));
        samples.pending = Some((3, sampled));
        samples.notice(3, false, sampled + Duration::from_millis(10));
        assert!(samples.unsafe_to_hold(Some(dispatched), sampled + Duration::from_millis(10)));
    }
}
