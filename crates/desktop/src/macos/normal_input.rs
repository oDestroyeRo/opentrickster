use opentrickster_engine::At;
use opentrickster_net::{InputKey, NormalInput};
use std::time::{Duration, Instant};

const ORDINARY_HOLD: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Control {
    Left,
    Right,
    Key(InputKey),
}
impl Control {
    fn release(self) -> NormalInput {
        match self {
            Self::Left => NormalInput::LeftUp,
            Self::Right => NormalInput::RightUp,
            Self::Key(key) => NormalInput::KeyUp(key),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Context {
    pub host: u64,
    pub layout: u64,
    pub physical: u64,
    pub focused: bool,
}

pub(super) trait Target {
    fn context(&self) -> Context;
    fn physical_held(&self, control: Control) -> bool;
    fn dispatch(&mut self, input: NormalInput, cleanup: bool) -> Result<Instant, ()>;
}

pub(super) struct Plan<'a, S> {
    pub session: u64,
    pub action: u64,
    pub step: S,
    pub host: u64,
    pub layout: u64,
    pub not_after: At,
    pub release_by: Option<At>,
    pub steps: &'a [NormalInput],
}
struct Owned<S> {
    control: Control,
    action: u64,
    _step: S,
    deadline: Option<Instant>,
}

/// Owns release obligations independently of the logical ticket's lifetime.
pub(super) struct Executor<S> {
    session: u64,
    context: Context,
    action: Option<u64>,
    seen: Vec<S>,
    owned: Vec<Owned<S>>,
    cancelled: bool,
    uncertain: bool,
    last_dispatch: Option<Instant>,
}
impl<S: Copy + Eq> Executor<S> {
    pub fn new(session: u64, context: Context) -> Self {
        Self {
            session,
            context,
            action: None,
            seen: Vec::new(),
            owned: Vec::new(),
            cancelled: false,
            uncertain: false,
            last_dispatch: None,
        }
    }

    pub fn begin(&mut self, action: u64, context: Context) -> Result<(), ()> {
        if self.action.is_some() || !self.owned.is_empty() || context.host != self.context.host {
            return Err(());
        }
        self.action = Some(action);
        self.context = context;
        self.cancelled = false;
        self.uncertain = false;
        Ok(())
    }

    pub fn execute(
        &mut self,
        plan: Plan<'_, S>,
        now: At,
        target: &mut impl Target,
    ) -> Result<Instant, ()> {
        self.last_dispatch = None;
        if self.cancelled
            || plan.session != self.session
            || self.action != Some(plan.action)
            || plan.host != self.context.host
            || plan.layout != self.context.layout
            || now >= plan.not_after
            || self.seen.contains(&plan.step)
            || self.seen.len() >= 4096
            || plan.steps.is_empty()
            || plan
                .steps
                .iter()
                .filter(|s| transition(**s).is_some())
                .count()
                > 1
            || target.context() != self.context
            || !self.context.focused
        {
            return Err(());
        }
        // Validate the entire plan before sending even its hover preparation.
        for input in plan.steps {
            if let Some((control, down)) = transition(*input) {
                if target.physical_held(control)
                    || if down {
                        self.owned.iter().any(|o| o.control == control)
                    } else {
                        !self
                            .owned
                            .iter()
                            .any(|o| o.control == control && o.action == plan.action)
                    }
                {
                    return Err(());
                }
            } else if !matches!(input, NormalInput::PointerMove { x, y } if x.is_finite() && y.is_finite())
            {
                return Err(());
            }
        }
        self.seen.push(plan.step);
        let mut dispatched = None;
        for input in plan.steps {
            if target.context() != self.context {
                self.cancel();
                return Err(());
            }
            let transition = transition(*input);
            if transition.is_some_and(|(control, _)| target.physical_held(control)) {
                self.cancel();
                return Err(());
            }
            // Reserve before calling native code. A failure may have partially delivered.
            if let Some((control, true)) = transition {
                let deadline = plan.release_by.map(|release| {
                    Instant::now()
                        + ORDINARY_HOLD.min(Duration::from_millis(release.0.saturating_sub(now.0)))
                });
                self.owned.push(Owned {
                    control,
                    action: plan.action,
                    _step: plan.step,
                    deadline,
                });
            }
            match target.dispatch(*input, false) {
                Ok(at) => {
                    dispatched = Some(at);
                    self.last_dispatch = Some(at);
                    if let Some((control, false)) = transition {
                        self.owned.retain(|owned| owned.control != control);
                    }
                }
                Err(()) => {
                    self.cancel();
                    return Err(());
                }
            }
        }
        if target.context() != self.context {
            self.cancel();
            return Err(());
        }
        dispatched.ok_or(())
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.uncertain = true;
    }

    /// A watchdog expiry only creates a release obligation; it is never a receipt.
    pub fn cleanup(&mut self, now: Instant, target: &mut impl Target) -> Vec<Instant> {
        if self.action.is_some() && target.context() != self.context {
            self.cancel();
        }
        if self
            .owned
            .iter()
            .any(|owned| owned.deadline.is_some_and(|at| now >= at))
        {
            self.cancel();
        }
        if !self.cancelled {
            return Vec::new();
        }
        let mut dispatched = Vec::new();
        self.owned.retain(|owned| {
            if target.context().host != self.context.host || target.physical_held(owned.control) {
                return true;
            }
            match target.dispatch(owned.control.release(), true) {
                Ok(at) => {
                    dispatched.push(at);
                    false
                }
                Err(()) => true,
            }
        });
        dispatched
    }

    pub fn controls_released(&self) -> bool {
        self.owned.is_empty()
    }
    pub fn uncertain(&self) -> bool {
        self.uncertain
    }
    pub fn last_dispatch(&self) -> Option<Instant> {
        self.last_dispatch
    }
    pub fn finish(&mut self) -> Result<(), ()> {
        if !self.controls_released() {
            return Err(());
        }
        self.action = None;
        self.cancelled = false;
        self.uncertain = false;
        Ok(())
    }
}

fn canonical(key: InputKey) -> Option<InputKey> {
    key.dom_code()?;
    match key {
        InputKey::Character(c) if c.is_ascii_alphabetic() => {
            Some(InputKey::Character(c.to_ascii_uppercase()))
        }
        InputKey::Character(_) => None,
        other => Some(other),
    }
}
fn transition(input: NormalInput) -> Option<(Control, bool)> {
    match input {
        NormalInput::LeftDown => Some((Control::Left, true)),
        NormalInput::LeftUp => Some((Control::Left, false)),
        NormalInput::RightDown => Some((Control::Right, true)),
        NormalInput::RightUp => Some((Control::Right, false)),
        NormalInput::KeyDown(key) => canonical(key).map(|key| (Control::Key(key), true)),
        NormalInput::KeyUp(key) => canonical(key).map(|key| (Control::Key(key), false)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        context: Context,
        held: Option<Control>,
        events: Vec<(NormalInput, bool)>,
        fail: bool,
        race: bool,
    }
    impl Default for Fixture {
        fn default() -> Self {
            Self {
                context: Context {
                    host: 3,
                    layout: 8,
                    physical: 0,
                    focused: true,
                },
                held: None,
                events: Vec::new(),
                fail: false,
                race: false,
            }
        }
    }
    impl Target for Fixture {
        fn context(&self) -> Context {
            self.context
        }
        fn physical_held(&self, control: Control) -> bool {
            self.held == Some(control)
        }
        fn dispatch(&mut self, input: NormalInput, cleanup: bool) -> Result<Instant, ()> {
            if self.fail {
                return Err(());
            }
            self.events.push((input, cleanup));
            if self.race {
                self.context.physical += 1;
            }
            Ok(Instant::now())
        }
    }
    fn plan(step: u64, steps: &[NormalInput]) -> Plan<'_, u64> {
        Plan {
            session: 1,
            action: 2,
            step,
            host: 3,
            layout: 8,
            not_after: At(1000),
            release_by: Some(At(250)),
            steps,
        }
    }
    fn executor(fixture: &Fixture) -> Executor<u64> {
        let mut executor = Executor::new(1, fixture.context);
        executor.begin(2, fixture.context).unwrap();
        executor
    }

    #[test]
    fn native_delivery_is_separate_from_duplicate_and_release_receipts() {
        let mut target = Fixture::default();
        let mut executor = executor(&target);
        executor
            .execute(
                plan(
                    1,
                    &[
                        NormalInput::PointerMove { x: 40., y: 50. },
                        NormalInput::LeftDown,
                    ],
                ),
                At(0),
                &mut target,
            )
            .unwrap();
        assert!(
            executor
                .execute(plan(1, &[NormalInput::LeftDown]), At(1), &mut target)
                .is_err()
        );
        assert!(
            executor
                .execute(plan(2, &[NormalInput::LeftDown]), At(1), &mut target)
                .is_err()
        );
        assert!(
            executor
                .execute(plan(3, &[NormalInput::RightUp]), At(1), &mut target)
                .is_err()
        );
        assert!(executor.finish().is_err());
        executor
            .execute(plan(4, &[NormalInput::LeftUp]), At(2), &mut target)
            .unwrap();
        assert!(
            executor
                .execute(plan(5, &[NormalInput::LeftUp]), At(3), &mut target)
                .is_err()
        );
        assert_eq!(
            target.events,
            vec![
                (NormalInput::PointerMove { x: 40., y: 50. }, false),
                (NormalInput::LeftDown, false),
                (NormalInput::LeftUp, false)
            ]
        );
        assert!(executor.finish().is_ok());
    }

    #[test]
    fn stale_session_host_layout_action_step_and_deadline_send_nothing() {
        let mut target = Fixture::default();
        let mut executor = executor(&target);
        for field in 0..6 {
            let mut p = plan(field, &[NormalInput::LeftDown]);
            match field {
                0 => p.session += 1,
                1 => p.host += 1,
                2 => p.layout += 1,
                3 => p.action += 1,
                4 => p.not_after = At(0),
                _ => target.context.focused = false,
            }
            assert!(executor.execute(p, At(0), &mut target).is_err());
        }
        assert!(target.events.is_empty());
    }

    #[test]
    fn two_transitions_or_invalid_character_are_refused_before_hover() {
        let mut target = Fixture::default();
        let mut executor = executor(&target);
        assert!(
            executor
                .execute(
                    plan(
                        1,
                        &[
                            NormalInput::PointerMove { x: 1., y: 2. },
                            NormalInput::LeftDown,
                            NormalInput::LeftUp
                        ]
                    ),
                    At(0),
                    &mut target
                )
                .is_err()
        );
        assert!(
            executor
                .execute(
                    plan(2, &[NormalInput::KeyDown(InputKey::Character('ก'))]),
                    At(0),
                    &mut target
                )
                .is_err()
        );
        assert!(target.events.is_empty());
    }

    #[test]
    fn focus_or_physical_input_between_hover_and_press_fences_the_press() {
        let mut target = Fixture {
            race: true,
            ..Fixture::default()
        };
        let mut executor = executor(&target);
        assert!(
            executor
                .execute(
                    plan(
                        1,
                        &[
                            NormalInput::PointerMove { x: 1., y: 2. },
                            NormalInput::LeftDown
                        ]
                    ),
                    At(0),
                    &mut target
                )
                .is_err()
        );
        assert_eq!(
            target.events,
            vec![(NormalInput::PointerMove { x: 1., y: 2. }, false)]
        );
        assert!(executor.uncertain());
    }

    #[test]
    fn cancellation_preserves_tokens_until_release_succeeds_without_releasing_user_controls() {
        let mut target = Fixture::default();
        let mut executor = executor(&target);
        executor
            .execute(
                plan(1, &[NormalInput::KeyDown(InputKey::Character('d'))]),
                At(0),
                &mut target,
            )
            .unwrap();
        target.held = Some(Control::Key(InputKey::Character('D')));
        target.context.focused = false;
        executor.cancel();
        assert!(executor.cleanup(Instant::now(), &mut target).is_empty());
        assert!(!executor.controls_released());
        assert!(executor.finish().is_err());
        target.held = None;
        target.fail = true;
        assert!(executor.cleanup(Instant::now(), &mut target).is_empty());
        assert!(!executor.controls_released());
        target.fail = false;
        assert_eq!(executor.cleanup(Instant::now(), &mut target).len(), 1);
        assert!(executor.controls_released());
        assert!(executor.cleanup(Instant::now(), &mut target).is_empty());
        assert_eq!(
            target.events.last(),
            Some(&(NormalInput::KeyUp(InputKey::Character('D')), true))
        );
    }

    #[test]
    fn watchdog_releases_only_its_control_and_marks_uncertain() {
        let mut target = Fixture {
            held: Some(Control::Right),
            ..Fixture::default()
        };
        let mut executor = executor(&target);
        executor
            .execute(plan(1, &[NormalInput::LeftDown]), At(0), &mut target)
            .unwrap();
        assert!(executor.cleanup(Instant::now(), &mut target).is_empty());
        assert_eq!(
            executor
                .cleanup(Instant::now() + Duration::from_millis(250), &mut target)
                .len(),
            1
        );
        assert!(executor.uncertain());
        assert_eq!(
            target.events,
            vec![(NormalInput::LeftDown, false), (NormalInput::LeftUp, true)]
        );
    }

    #[test]
    fn gauge_hold_has_no_ordinary_watchdog_but_still_releases_on_fence_change() {
        let mut target = Fixture::default();
        let mut executor = executor(&target);
        let mut gauge = plan(1, &[NormalInput::LeftDown]);
        gauge.release_by = None;
        executor.execute(gauge, At(0), &mut target).unwrap();
        assert!(
            executor
                .cleanup(Instant::now() + Duration::from_secs(2), &mut target)
                .is_empty()
        );
        target.context.layout += 1;
        assert_eq!(executor.cleanup(Instant::now(), &mut target).len(), 1);
        assert!(executor.uncertain());
    }

    #[test]
    fn a_dispatch_failure_or_wrong_host_never_forgets_release_ownership() {
        let mut target = Fixture {
            fail: true,
            ..Fixture::default()
        };
        let mut executor = executor(&target);
        assert!(
            executor
                .execute(plan(1, &[NormalInput::LeftDown]), At(0), &mut target)
                .is_err()
        );
        assert!(!executor.controls_released());
        target.fail = false;
        target.context.host += 1;
        assert!(executor.cleanup(Instant::now(), &mut target).is_empty());
        assert!(!executor.controls_released());
        target.context.host -= 1;
        assert_eq!(executor.cleanup(Instant::now(), &mut target).len(), 1);
    }
    #[test]
    fn only_finished_cleanup_can_admit_a_new_action_after_cancellation() {
        let mut target = Fixture::default();
        let mut executor = executor(&target);
        executor
            .execute(plan(1, &[NormalInput::LeftDown]), At(0), &mut target)
            .unwrap();
        executor.cancel();
        assert!(executor.finish().is_err());
        assert!(executor.begin(3, target.context).is_err());
        assert_eq!(executor.cleanup(Instant::now(), &mut target).len(), 1);
        executor.finish().unwrap();
        assert!(!executor.uncertain());
        executor.begin(3, target.context).unwrap();
        let mut next = plan(2, &[NormalInput::RightDown]);
        next.action = 3;
        executor.execute(next, At(1), &mut target).unwrap();
        assert_eq!(target.events.last(), Some(&(NormalInput::RightDown, false)));
    }
}
