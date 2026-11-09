mod planning;

use crate::{
    world::{Book, Evidence},
    *,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const EARLY_LIMIT: usize = 32;
const LOG_LIMIT: usize = 200;
const EVIDENCE_RANGES: usize = 4_096;

#[derive(Clone, Copy)]
enum Connection {
    Closed,
    Opening {
        key: SessionKey,
        step: ConnectStep,
        attempt: u8,
    },
    Online(SessionKey),
    Disconnected(SessionKey),
    Closing {
        key: SessionKey,
        after: AfterClose,
    },
    Halted {
        key: Option<SessionKey>,
        reason: Halt,
    },
}
#[derive(Clone, Copy)]
enum AfterClose {
    Offline(Mode),
    Reopen,
    Halt(Halt),
}
#[derive(Clone, Copy)]
enum Stage {
    Running,
    Draining(AfterDrain),
    Paused(Pause),
}
#[derive(Clone, Copy)]
enum AfterDrain {
    Pause(Pause),
    Stop,
}

struct Run {
    key: RunKey,
    stage: Stage,
    pending: Option<Pending>,
    rejected: u8,
    totals: Totals,
    killed: BTreeSet<(SessionKey, EntityKey, u64)>,
    dig_index: u64,
    goal_floor: Option<IngressMark>,
    gains: BTreeMap<ItemKind, u64>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Resolution {
    Confirmed,
    Rejected,
}

struct Pending {
    id: ActionKey,
    action: Action,
    incarnation: u64,
    receipt_by: At,
    finish_by: At,
    sent: Option<IngressMark>,
    terminal: Option<Resolution>,
    dig_result: Option<DigEnd>,
    quarantined: bool,
    early: Vec<Evidence>,
}
struct Barrier {
    key: BarrierKey,
    deadline: At,
}

/// One lifecycle owner. All I/O is returned as effects; this type never sends or logs game data.
pub struct Engine {
    mode: Mode,
    connection: Connection,
    setup: Option<Setup>,
    book: Book,
    support: Option<Support>,
    choices: Choices,
    run: Option<Run>,
    last_run: Totals,
    barrier: Option<Barrier>,
    idle: bool,
    seen: BTreeMap<u64, u64>,
    log: VecDeque<LogEntry>,
    refusal: Option<Refusal>,
    now: At,
    last_request: Option<u64>,
    next_key: u64,
    reconnect_attempt: u8,
}

impl Engine {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            connection: Connection::Closed,
            setup: None,
            book: Book::new(),
            support: None,
            choices: Choices::None,
            run: None,
            last_run: zero_totals(),
            barrier: None,
            idle: false,
            seen: BTreeMap::new(),
            log: VecDeque::new(),
            refusal: None,
            now: At(0),
            last_request: None,
            next_key: 1,
            reconnect_attempt: 0,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let allowed = [
            Control::Open,
            Control::CancelConnect,
            Control::Choose,
            Control::Configure,
            Control::Start,
            Control::Pause,
            Control::Resume,
            Control::Stop,
            Control::Reconnect,
            Control::DismissHalt,
            Control::SwitchMode,
        ]
        .into_iter()
        .filter(|control| self.guard(*control).is_ok())
        .collect();
        let activity = match (
            self.setup.as_ref().map(|setup| &setup.draft().activity),
            self.run.as_ref(),
        ) {
            (Some(Activity::Combat { .. }), Some(run)) => ActivityView::Combat {
                target: run
                    .pending
                    .as_ref()
                    .and_then(|pending| match pending.action {
                        Action::BasicAttack(key) => self.book.entities.get(&key).cloned(),
                        _ => None,
                    }),
            },
            (Some(Activity::Digging { .. }), Some(run)) => {
                let (point, step) = match run.pending.as_ref().map(|pending| pending.action) {
                    Some(Action::MoveTo(point)) => (Some(point), DigStep::Move),
                    Some(Action::DigAt(point)) => (Some(point), DigStep::Dig),
                    Some(Action::Pickup(_)) => (None, DigStep::Collect),
                    _ => (None, DigStep::Pick),
                };
                ActivityView::Digging { point, step }
            }
            _ => ActivityView::None,
        };
        let blocked = self.refusal.or_else(|| {
            if self.run.is_some() {
                self.guard(Control::Resume).err()
            } else {
                self.guard(Control::Start).err()
            }
        });
        Snapshot {
            mode: self.mode,
            phase: self.phase(),
            allowed,
            blocked,
            blockers: self.preflight(),
            build: self.support.as_ref().map(|support| support.build.clone()),
            choices: self.choices.clone(),
            setup: self.setup.clone(),
            activity,
            own: self.book.own.clone(),
            entities: self.book.entities.values().cloned().collect(),
            inventory: self.book.inventory.stacks.values().cloned().collect(),
            totals: self
                .run
                .as_ref()
                .map_or_else(|| self.last_run.clone(), |run| run.totals.clone()),
            log: self.log.iter().cloned().collect(),
        }
    }

    pub fn handle(&mut self, input: Input) -> Effects {
        let at = match &input {
            Input::User { at, .. } | Input::Observed { at, .. } | Input::Wake(at) => *at,
        };
        // Clamp backwards clocks and cap at a value with room for every policy deadline.
        let now = At(at.0.min(u64::MAX - 60_000).max(self.now.0));
        if let Some(run) = &mut self.run
            && matches!(run.stage, Stage::Running)
        {
            run.totals.elapsed_ms = run.totals.elapsed_ms.saturating_add(now.0 - self.now.0);
        }
        self.now = now;
        let mut io = Vec::new();
        self.deadlines(&mut io);
        if self.now.0 == u64::MAX - 60_000 {
            self.uncertain(&mut io);
            self.close(AfterClose::Offline(self.mode), &mut io);
            if let Input::Observed {
                observation:
                    Observation {
                        session,
                        event: Event::Closed,
                    },
                ..
            } = input
                && self.session() == Some(session)
            {
                self.event(Event::Closed, now, &mut io);
            }
            return Effects {
                wake_at: None,
                io,
                snapshot: self.snapshot(),
            };
        }
        match input {
            Input::User {
                request, command, ..
            } => {
                // Controller keys are monotonically minted; a replay is a no-op.
                if self.last_request.is_none_or(|last| request.0 > last) {
                    self.last_request = Some(request.0);
                    self.refusal = None;
                    self.command(command, &mut io);
                }
            }
            Input::Observed { at, observation } => {
                if self.session() == Some(observation.session) {
                    self.event(observation.event, at, &mut io);
                }
            }
            Input::Wake(_) => {}
        }
        self.decide(&mut io);
        Effects {
            wake_at: self.wake_at(),
            io,
            snapshot: self.snapshot(),
        }
    }

    fn mint(&mut self) -> u64 {
        let key = self.next_key;
        self.next_key = self
            .next_key
            .checked_add(1)
            .expect("local capability space exhausted");
        key
    }

    fn session(&self) -> Option<SessionKey> {
        match self.connection {
            Connection::Closed => None,
            Connection::Opening { key, .. }
            | Connection::Online(key)
            | Connection::Disconnected(key)
            | Connection::Closing { key, .. } => Some(key),
            Connection::Halted { key, .. } => key,
        }
    }

    fn phase(&self) -> Phase {
        match self.connection {
            Connection::Closed => {
                if self.mode == Mode::Assisted {
                    Phase::ClientAbsent
                } else {
                    Phase::Offline
                }
            }
            Connection::Opening { attempt, .. } if attempt > 0 => Phase::Reconnecting(attempt),
            Connection::Opening { step, .. } => match step {
                ConnectStep::LoadingGame(_) => Phase::ClientLoading,
                ConnectStep::AwaitingGameLogin => Phase::AwaitingLogin,
                _ => Phase::Connecting,
            },
            Connection::Disconnected(_) => Phase::Disconnected,
            Connection::Closing { .. } => Phase::Closing,
            Connection::Halted { reason, .. } => Phase::Error(reason),
            Connection::Online(_) => match self.run.as_ref().map(|run| run.stage) {
                None => Phase::Ready,
                Some(Stage::Running) => Phase::Running,
                Some(Stage::Paused(reason)) => Phase::Paused(reason),
                Some(Stage::Draining(AfterDrain::Pause(_))) => Phase::Pausing,
                Some(Stage::Draining(AfterDrain::Stop)) => Phase::Stopping,
            },
        }
    }

    fn guard(&self, control: Control) -> Result<(), Refusal> {
        match control {
            Control::Open
                if matches!(self.connection, Connection::Closed) && self.run.is_none() =>
            {
                Ok(())
            }
            Control::CancelConnect if matches!(self.connection, Connection::Opening { .. }) => {
                Ok(())
            }
            Control::Choose
                if matches!(self.connection, Connection::Opening { .. })
                    && !matches!(self.choices, Choices::None) =>
            {
                Ok(())
            }
            Control::Configure
                if self
                    .run
                    .as_ref()
                    .is_none_or(|run| matches!(run.stage, Stage::Paused(_))) =>
            {
                Ok(())
            }
            Control::Start
                if self.run.is_none() && matches!(self.connection, Connection::Online(_)) =>
            {
                self.ready()
            }
            Control::Resume
                if self
                    .run
                    .as_ref()
                    .is_some_and(|run| matches!(run.stage, Stage::Paused(_)))
                    && matches!(self.connection, Connection::Online(_)) =>
            {
                self.ready()
            }
            Control::Pause
                if self
                    .run
                    .as_ref()
                    .is_some_and(|run| matches!(run.stage, Stage::Running)) =>
            {
                Ok(())
            }
            Control::Stop if self.run.is_some() => Ok(()),
            Control::Reconnect
                if (matches!(self.connection, Connection::Disconnected(_))
                    || (matches!(self.connection, Connection::Closed) && self.run.is_some()))
                    && self.reconnect_attempt < 5 =>
            {
                Ok(())
            }
            Control::DismissHalt if matches!(self.connection, Connection::Halted { .. }) => Ok(()),
            Control::SwitchMode
                if self.run.is_none() && !matches!(self.connection, Connection::Closing { .. }) =>
            {
                Ok(())
            }
            _ => Err(Refusal::WrongPhase),
        }
    }

    fn ready(&self) -> Result<(), Refusal> {
        self.preflight()
            .first()
            .map_or(Ok(()), |blocker| Err(blocker_refusal(*blocker)))
    }

    fn preflight(&self) -> Vec<Blocker> {
        let mut blockers = self.safety_blockers();
        if self
            .setup
            .as_ref()
            .is_some_and(|setup| setup.draft().goal.is_some())
            && self.book.inventory.pending()
        {
            blockers.push(Blocker::NeedsObservation(StateKind::Inventory));
        }
        if blockers.is_empty()
            && matches!(self.connection, Connection::Online(_))
            && let Err(blocker) = self.plan()
        {
            blockers.push(blocker);
        }
        if !matches!(self.connection, Connection::Online(_)) {
            blockers.push(Blocker::NeedsConnection);
        }
        if self.barrier.is_some() || !self.idle {
            blockers.push(Blocker::NeedsQuiescence);
        }
        if self.run.as_ref().is_some_and(|run| run.pending.is_some()) {
            blockers.push(Blocker::NeedsActionDrain);
        }
        blockers
    }

    fn command(&mut self, command: Command, io: &mut Vec<Effect>) {
        let control = match command {
            Command::Open => Control::Open,
            Command::CancelConnect => Control::CancelConnect,
            Command::ChooseWorld(_) | Command::ChooseCharacter(_) => Control::Choose,
            Command::Configure(_) => Control::Configure,
            Command::Start => Control::Start,
            Command::Pause => Control::Pause,
            Command::Resume => Control::Resume,
            Command::Stop => Control::Stop,
            Command::Reconnect => Control::Reconnect,
            Command::DismissHalt => Control::DismissHalt,
            Command::SwitchMode(_) => Control::SwitchMode,
        };
        if matches!(command, Command::Stop) && self.run.is_none() {
            return;
        }
        if let Err(refusal) = self.guard(control) {
            self.refuse(refusal);
            return;
        }
        match command {
            Command::Open => {
                self.reconnect_attempt = 0;
                self.open(io);
            }
            Command::CancelConnect => self.close(AfterClose::Offline(self.mode), io),
            Command::ChooseWorld(key) => {
                if matches!(&self.choices, Choices::Worlds(choices) if choices.iter().any(|choice| choice.key == key))
                {
                    self.choices = Choices::None;
                    self.emit(SessionOp::ChooseWorld(key), io);
                } else {
                    self.refuse(Refusal::MissingState);
                }
            }
            Command::ChooseCharacter(key) => match &self.choices {
                Choices::Characters(choices) => {
                    match choices.iter().find(|choice| choice.key == key) {
                        Some(choice) if choice.busy => self.refuse(Refusal::Busy),
                        Some(_) => {
                            self.choices = Choices::None;
                            self.emit(SessionOp::ChooseCharacter(key), io);
                        }
                        None => self.refuse(Refusal::MissingState),
                    }
                }
                _ => self.refuse(Refusal::WrongPhase),
            },
            Command::Configure(setup) => {
                if self.run.is_some()
                    && self.setup.as_ref().is_some_and(|old| {
                        std::mem::discriminant(&old.draft().activity)
                            != std::mem::discriminant(&setup.draft().activity)
                    })
                {
                    self.refuse(Refusal::WrongPhase);
                } else {
                    if let Some(run) = &mut self.run {
                        run.totals.goal_gained = setup
                            .draft()
                            .goal
                            .as_ref()
                            .and_then(|goal| run.gains.get(&goal.item))
                            .copied()
                            .unwrap_or(0);
                    }
                    self.setup = Some(setup);
                }
            }
            Command::Start => {
                let key = RunKey(self.mint());
                self.run = Some(Run {
                    key,
                    stage: Stage::Running,
                    pending: None,
                    rejected: 0,
                    totals: zero_totals(),
                    killed: BTreeSet::new(),
                    dig_index: 0,
                    goal_floor: None,
                    gains: BTreeMap::new(),
                });
            }
            Command::Resume => {
                if let Some(run) = &mut self.run {
                    run.stage = Stage::Running;
                }
            }
            Command::Pause => {
                if self
                    .run
                    .as_ref()
                    .is_some_and(|run| matches!(run.stage, Stage::Running))
                {
                    self.drain(AfterDrain::Pause(Pause::User), io);
                }
            }
            Command::Stop => {
                if matches!(self.connection, Connection::Online(_)) {
                    self.drain(AfterDrain::Stop, io);
                } else if matches!(self.connection, Connection::Closing { .. }) {
                    if let Some(run) = &mut self.run {
                        run.stage = Stage::Draining(AfterDrain::Stop);
                    }
                } else {
                    self.finish_run();
                }
            }
            Command::Reconnect => {
                self.reconnect_attempt += 1;
                if self.session().is_some() {
                    self.close(AfterClose::Reopen, io);
                } else {
                    self.open(io);
                }
            }
            Command::DismissHalt => {
                self.finish_run();
                if self.session().is_some() {
                    self.close(AfterClose::Offline(self.mode), io);
                } else {
                    self.connection = Connection::Closed;
                }
            }
            Command::SwitchMode(mode) => {
                if matches!(
                    self.connection,
                    Connection::Closed | Connection::Halted { key: None, .. }
                ) {
                    self.mode = mode;
                    self.connection = Connection::Closed;
                } else {
                    self.close(AfterClose::Offline(mode), io);
                }
            }
        }
    }

    /// The held run capability, useful for correlating local UI work across pauses.
    pub fn run_key(&self) -> Option<RunKey> {
        self.run.as_ref().map(|run| run.key)
    }

    fn open(&mut self, io: &mut Vec<Effect>) {
        let key = SessionKey(self.mint());
        let attempt = self.reconnect_attempt;
        self.connection = Connection::Opening {
            key,
            attempt,
            step: if self.mode == Mode::Assisted {
                ConnectStep::LoadingGame(0)
            } else {
                ConnectStep::SigningIn
            },
        };
        self.book = Book::new();
        self.support = None;
        self.choices = Choices::None;
        self.seen.clear();
        self.barrier = None;
        self.idle = false;
        io.push(Effect::Open {
            session: key,
            mode: self.mode,
        });
    }

    fn emit(&self, op: SessionOp, io: &mut Vec<Effect>) {
        if let Some(session) = self.session() {
            io.push(Effect::Session { session, op });
        }
    }

    fn close(&mut self, after: AfterClose, io: &mut Vec<Effect>) {
        if matches!(self.connection, Connection::Closing { .. }) {
            return;
        }
        if let Some(key) = self.session() {
            self.connection = Connection::Closing { key, after };
            self.barrier = None;
            self.idle = false;
            self.emit(SessionOp::Close, io);
        }
    }

    fn notify(&mut self, notice: Notice, io: &mut Vec<Effect>) {
        let key = NoticeKey(self.mint());
        io.push(Effect::Notify { key, notice });
    }

    fn record(&mut self, event: SafeLog) {
        if self.log.len() == LOG_LIMIT {
            self.log.pop_front();
        }
        self.log.push_back(LogEntry {
            at: self.now,
            event,
        });
    }

    fn refuse(&mut self, refusal: Refusal) {
        self.refusal = Some(refusal);
        self.record(SafeLog::Refused(refusal));
    }

    fn event(&mut self, event: Event, observed_at: At, io: &mut Vec<Effect>) {
        if matches!(self.connection, Connection::Closing { .. }) && !matches!(event, Event::Closed)
        {
            return;
        }
        match event {
            Event::Progress(step) => {
                if let Connection::Opening { step: old, .. } = &mut self.connection {
                    *old = step;
                }
            }
            Event::Worlds(choices) => {
                if matches!(self.connection, Connection::Opening { .. }) {
                    self.choices = Choices::Worlds(choices);
                }
            }
            Event::Characters(choices) => {
                if matches!(self.connection, Connection::Opening { .. }) {
                    self.choices = Choices::Characters(choices);
                }
            }
            Event::Baseline {
                support,
                revision,
                facts,
                resumed: _,
            } => {
                if !matches!(self.connection, Connection::Opening { .. }) {
                    return;
                }
                let Some(key) = self.session() else {
                    return;
                };
                self.seen.clear();
                if !self.remember(revision) {
                    return;
                }
                self.book = Book::new();
                self.book.apply(facts, IngressMark(0), observed_at);
                self.support = Some(support);
                self.connection = Connection::Online(key);
                self.reconnect_attempt = 0;
                self.choices = Choices::None;
                self.idle = false;
                if let Some(run) = &mut self.run {
                    run.stage = Stage::Paused(Pause::Reconnected);
                    run.pending = None;
                    run.goal_floor = None;
                }
                if self.support.as_ref().is_some_and(|support| support.quiesce) {
                    self.quiesce(io);
                }
            }
            Event::DigCycle(available) => {
                if matches!(self.connection, Connection::Online(_))
                    && let Some(support) = &mut self.support
                {
                    support.dig_cycle = available;
                }
            }
            Event::Facts {
                revision,
                ingress,
                facts,
            } => {
                if !matches!(self.connection, Connection::Online(_)) {
                    return;
                }
                if !self.remember(revision) {
                    return;
                }
                if self.seen.len() > EVIDENCE_RANGES {
                    self.uncertain(io);
                    self.close(AfterClose::Offline(self.mode), io);
                    return;
                }
                let mut evidence = self.book.apply(facts, ingress, observed_at);
                self.credit_inventory(&mut evidence);
                let Some(pending) = self.run.as_mut().and_then(|run| run.pending.as_mut()) else {
                    return;
                };
                if pending.quarantined {
                    return;
                }
                evidence
                    .outcomes
                    .retain(|outcome| outcome_matches(pending.action, *outcome));
                if !matches!(pending.action, Action::DigAt(_) | Action::Pickup(_)) {
                    evidence.gains.clear();
                }
                if pending.sent.is_none() {
                    if !evidence.outcomes.is_empty() || !evidence.gains.is_empty() {
                        if pending.early.len() == EARLY_LIMIT {
                            self.uncertain(io);
                        } else {
                            pending.early.push(evidence);
                        }
                    }
                } else {
                    self.evidence(evidence, io);
                }
            }
            Event::Receipt { action, delivery } => self.receipt(action, delivery, io),
            Event::ManualInput => {
                if !matches!(self.connection, Connection::Online(_)) {
                    return;
                }
                if let Some(pending) = self.run.as_mut().and_then(|run| run.pending.as_mut()) {
                    pending.quarantined = true;
                    pending.early.clear();
                }
                if let Some(run) = &mut self.run {
                    run.goal_floor = None;
                }
                if let Some(run) = &mut self.run
                    && !matches!(run.stage, Stage::Draining(AfterDrain::Stop))
                {
                    run.stage = Stage::Draining(AfterDrain::Pause(Pause::ManualInput));
                }
                self.barrier = None;
                self.idle = false;
                self.quiesce(io);
            }
            Event::Quiescent(key) => {
                if !self
                    .barrier
                    .as_ref()
                    .is_some_and(|barrier| barrier.key == key)
                    || !matches!(self.connection, Connection::Online(_))
                {
                    return;
                }
                self.barrier = None;
                self.idle = true;
                if let Some(run) = &mut self.run {
                    run.pending = None;
                    match run.stage {
                        Stage::Draining(AfterDrain::Stop) => self.finish_run(),
                        Stage::Draining(AfterDrain::Pause(reason)) => {
                            self.pause_finished(reason, io)
                        }
                        _ => {}
                    }
                }
            }
            Event::Lost => {
                if !matches!(
                    self.connection,
                    Connection::Online(_) | Connection::Opening { .. }
                ) {
                    return;
                }
                if let Some(key) = self.session() {
                    self.connection = Connection::Disconnected(key);
                    self.book.invalidate();
                    self.barrier = None;
                    self.idle = false;
                    if let Some(run) = &mut self.run {
                        if !matches!(run.stage, Stage::Draining(AfterDrain::Stop)) {
                            run.stage = Stage::Paused(Pause::Reconnected);
                        }
                        run.goal_floor = None;
                        if let Some(pending) = &mut run.pending {
                            pending.quarantined = true;
                            pending.early.clear();
                        }
                    }
                    self.notify(Notice::ConnectionLost, io);
                    if self
                        .run
                        .as_ref()
                        .is_some_and(|run| matches!(run.stage, Stage::Draining(AfterDrain::Stop)))
                    {
                        self.close(AfterClose::Offline(self.mode), io);
                    }
                }
            }
            Event::Closed => {
                self.book.invalidate();
                let after = match self.connection {
                    Connection::Closing { after, .. } => after,
                    _ => AfterClose::Offline(self.mode),
                };
                self.barrier = None;
                self.idle = false;
                self.support = None;
                self.choices = Choices::None;
                if self
                    .run
                    .as_ref()
                    .is_some_and(|run| matches!(run.stage, Stage::Draining(AfterDrain::Stop)))
                {
                    self.finish_run();
                } else if let Some(run) = &mut self.run {
                    run.pending = None;
                    run.stage = Stage::Paused(Pause::Reconnected);
                }
                self.connection = Connection::Closed;
                match after {
                    AfterClose::Offline(mode) => {
                        self.mode = mode;
                    }
                    AfterClose::Reopen => self.open(io),
                    AfterClose::Halt(reason) => {
                        self.connection = Connection::Halted { key: None, reason };
                    }
                }
            }
            Event::Failed(Halt::ConnectFailed)
                if matches!(self.connection, Connection::Opening { attempt: 1.., .. }) =>
            {
                self.close(AfterClose::Offline(self.mode), io);
            }
            Event::Failed(reason) => self.halt(reason, io),
        }
    }

    fn remember(&mut self, key: EvidenceKey) -> bool {
        if self
            .seen
            .range(..=key.0)
            .next_back()
            .is_some_and(|(_, end)| *end >= key.0)
        {
            return false;
        }
        let left = self
            .seen
            .range(..key.0)
            .next_back()
            .map(|(&start, &end)| (start, end));
        let right = self
            .seen
            .range(key.0..)
            .next()
            .map(|(&start, &end)| (start, end));
        let mut start = key.0;
        let mut end = key.0;
        if let Some((old_start, old_end)) = left
            && old_end.checked_add(1) == Some(key.0)
        {
            start = old_start;
            self.seen.remove(&old_start);
        }
        if let Some((old_start, old_end)) = right
            && key.0.checked_add(1) == Some(old_start)
        {
            end = old_end;
            self.seen.remove(&old_start);
        }
        self.seen.insert(start, end);
        true
    }

    fn receipt(&mut self, action: ActionKey, delivery: Delivery, io: &mut Vec<Effect>) {
        if !matches!(self.connection, Connection::Online(_)) {
            return;
        }
        let Some(run) = &mut self.run else {
            return;
        };
        let Some(pending) = &mut run.pending else {
            return;
        };
        if pending.id != action || pending.quarantined {
            return;
        }
        match delivery {
            Delivery::Submitted => {}
            Delivery::Sent(mark) => {
                if let Some(old) = pending.sent {
                    if old != mark {
                        self.uncertain(io);
                    }
                    return;
                }
                pending.sent = Some(mark);
                pending.finish_by =
                    At(self
                        .now
                        .0
                        .saturating_add(if matches!(pending.action, Action::DigAt(_)) {
                            DIG_MS
                        } else {
                            ACTION_MS
                        }));
                run.totals.sent = run.totals.sent.saturating_add(1);
                if matches!(pending.action, Action::DigAt(_) | Action::Pickup(_)) {
                    run.goal_floor = Some(run.goal_floor.map_or(mark, |old| old.min(mark)));
                }
                let early = std::mem::take(&mut pending.early);
                self.record(SafeLog::Sent);
                for evidence in early {
                    self.evidence(evidence, io);
                }
            }
            Delivery::NotSent => {
                if pending.sent.is_some() {
                    self.uncertain(io);
                } else {
                    pending.quarantined = true;
                    pending.early.clear();
                    self.drain(AfterDrain::Pause(Pause::Uncertain), io);
                }
            }
            Delivery::Unknown => self.uncertain(io),
        }
    }

    fn credit_inventory(&mut self, evidence: &mut Evidence) {
        let Some(run) = &mut self.run else {
            return;
        };
        let Some(floor) = run.goal_floor else {
            return;
        };
        for gain in std::mem::take(&mut evidence.gains) {
            if gain.ingress > floor {
                let total = run.gains.entry(gain.kind).or_insert(0);
                *total = total.saturating_add(gain.quantity);
            }
        }
        run.totals.goal_gained = self
            .setup
            .as_ref()
            .and_then(|setup| setup.draft().goal.as_ref())
            .and_then(|goal| run.gains.get(&goal.item))
            .copied()
            .unwrap_or(0);
    }

    fn evidence(&mut self, mut evidence: Evidence, io: &mut Vec<Effect>) {
        self.credit_inventory(&mut evidence);
        let Some(session) = self.session() else {
            return;
        };
        let Some(run) = &mut self.run else {
            return;
        };
        let Some(pending) = &mut run.pending else {
            return;
        };
        let Some(sent) = pending.sent else {
            return;
        };
        if pending.quarantined || evidence.ingress <= sent {
            return;
        }
        let mut confirmed = false;
        let mut rejected = None;
        let mut killed = None;
        let mut dig_report = None;
        let mut picked = None;
        let mut arrived = None;
        for outcome in evidence.outcomes {
            match (pending.action, outcome) {
                (Action::MoveTo(expected), Outcome::Arrived(actual)) if expected == actual => {
                    confirmed = true;
                    arrived = Some(actual);
                }
                (Action::BasicAttack(expected), Outcome::OwnHit(actual))
                    if expected == actual
                        && self.book.incarnation(actual) == pending.incarnation =>
                {
                    confirmed = true
                }
                (Action::BasicAttack(expected), Outcome::OwnKill(actual))
                    if expected == actual
                        && self.book.incarnation(actual) == pending.incarnation =>
                {
                    killed = Some(actual);
                    confirmed = true;
                }
                (Action::DigAt(_), Outcome::DigEnded(end)) => {
                    if dig_report
                        .or(pending.dig_result)
                        .is_some_and(|old| old != end)
                    {
                        self.uncertain(io);
                        return;
                    }
                    dig_report = Some(end);
                    confirmed = true;
                }
                (Action::Pickup(expected), Outcome::OwnPickup(actual)) if expected == actual => {
                    confirmed = true;
                    picked = Some(actual);
                }
                (Action::HpPotion(expected), Outcome::PotionUsed(actual)) if expected == actual => {
                    confirmed = true
                }
                (action, Outcome::Rejected { subject, reason })
                    if subject_matches(action, subject) =>
                {
                    rejected = Some(reason)
                }
                _ => {}
            }
        }
        if (rejected.is_some() && (confirmed || pending.terminal == Some(Resolution::Confirmed)))
            || (confirmed && pending.terminal == Some(Resolution::Rejected))
        {
            self.uncertain(io);
            return;
        }
        if pending.terminal == Some(Resolution::Rejected) {
            if confirmed {
                self.uncertain(io);
            }
            return;
        }
        if let Some(key) = killed
            && run.killed.insert((session, key, pending.incarnation))
        {
            run.totals.kills = run.totals.kills.saturating_add(1);
            self.book.mark_killed(key, evidence.ingress, evidence.at);
        }
        if let Some(key) = picked {
            self.book.mark_picked(key, evidence.ingress, evidence.at);
        }
        if let Some(point) = arrived {
            self.book.arrived(point, evidence.ingress, evidence.at);
        }
        if pending.terminal.is_some() {
            return;
        }
        if let Some(reason) = rejected {
            pending.terminal = Some(Resolution::Rejected);
            run.rejected = run.rejected.saturating_add(1);
            let halt = run.rejected >= 3;
            self.record(SafeLog::Rejected(reason));
            if halt {
                self.halt(Halt::ActionsRejected, io);
            } else if reason == Rejection::Resources {
                self.drain(AfterDrain::Pause(Pause::Uncertain), io);
            } else {
                self.quiesce(io);
            }
        } else if confirmed {
            if let Some(end) = dig_report {
                pending.dig_result = Some(end);
                run.totals.digs = run.totals.digs.saturating_add(1);
                run.dig_index = run.dig_index.wrapping_add(1);
                match end {
                    DigEnd::Empty => run.totals.empty = run.totals.empty.saturating_add(1),
                    DigEnd::ItemReported(_) => {
                        run.totals.finds = run.totals.finds.saturating_add(1)
                    }
                }
            }
            pending.terminal = Some(Resolution::Confirmed);
            run.rejected = 0;
            run.totals.confirmed = run.totals.confirmed.saturating_add(1);
            self.record(SafeLog::Confirmed);
            self.quiesce(io);
        }
    }

    fn quiesce(&mut self, io: &mut Vec<Effect>) {
        if self.barrier.is_some() {
            return;
        }
        self.idle = false;
        if !self.support.as_ref().is_some_and(|support| support.quiesce) {
            self.close(AfterClose::Offline(self.mode), io);
            return;
        }
        let key = BarrierKey(self.mint());
        self.barrier = Some(Barrier {
            key,
            deadline: At(self.now.0.saturating_add(DRAIN_MS)),
        });
        self.emit(SessionOp::Quiesce(key), io);
    }

    fn drain(&mut self, after: AfterDrain, io: &mut Vec<Effect>) {
        let Some(run) = &mut self.run else {
            return;
        };
        if matches!(run.stage, Stage::Draining(AfterDrain::Stop)) {
            return;
        }
        if matches!(run.stage, Stage::Draining(_)) && matches!(after, AfterDrain::Pause(_)) {
            return;
        }
        run.stage = Stage::Draining(after);
        if run.pending.is_none() && self.idle && self.barrier.is_none() {
            match after {
                AfterDrain::Stop => self.finish_run(),
                AfterDrain::Pause(reason) => self.pause_finished(reason, io),
            }
        } else {
            self.quiesce(io);
        }
    }

    fn pause_finished(&mut self, reason: Pause, io: &mut Vec<Effect>) {
        if let Some(run) = &mut self.run {
            run.stage = Stage::Paused(reason);
        }
        self.record(SafeLog::Paused(reason));
        if reason == Pause::Goal {
            self.notify(Notice::GoalReached, io);
        } else {
            self.notify(Notice::Paused(reason), io);
        }
    }

    fn uncertain(&mut self, io: &mut Vec<Effect>) {
        if let Some(run) = &mut self.run {
            run.goal_floor = None;
        }
        if let Some(pending) = self.run.as_mut().and_then(|run| run.pending.as_mut()) {
            pending.quarantined = true;
            pending.early.clear();
        }
        self.drain(AfterDrain::Pause(Pause::Uncertain), io);
    }

    fn finish_run(&mut self) {
        if let Some(run) = self.run.take() {
            self.last_run = run.totals;
        }
    }

    fn halt(&mut self, reason: Halt, io: &mut Vec<Effect>) {
        self.record(SafeLog::Halted(reason));
        self.finish_run();
        if let Some(key) = self.session() {
            self.connection = Connection::Halted {
                key: Some(key),
                reason,
            };
            self.close(AfterClose::Halt(reason), io);
        } else {
            self.connection = Connection::Halted { key: None, reason };
        }
    }

    fn deadlines(&mut self, io: &mut Vec<Effect>) {
        if !matches!(self.connection, Connection::Online(_)) {
            return;
        }
        if self
            .barrier
            .as_ref()
            .is_some_and(|barrier| self.now >= barrier.deadline)
        {
            // With no action of ours in flight, a slow idle proof only holds the next action.
            // Keep the same barrier and ask again instead of dropping the player's session.
            if self.run.as_ref().is_none_or(|run| {
                run.pending.is_none() && !matches!(run.stage, Stage::Draining(AfterDrain::Stop))
            }) && let Some(barrier) = &mut self.barrier
            {
                barrier.deadline = At(self.now.0.saturating_add(DRAIN_MS));
                let key = barrier.key;
                self.emit(SessionOp::Quiesce(key), io);
                return;
            }
            if let Some(run) = &mut self.run
                && !matches!(run.stage, Stage::Draining(AfterDrain::Stop))
            {
                run.stage = Stage::Paused(Pause::Uncertain);
            }
            self.close(AfterClose::Offline(self.mode), io);
            return;
        }
        let expired = self
            .run
            .as_ref()
            .and_then(|run| run.pending.as_ref())
            .is_some_and(|pending| {
                pending.terminal.is_none()
                    && !pending.quarantined
                    && self.now
                        >= if pending.sent.is_none() {
                            pending.receipt_by
                        } else {
                            pending.finish_by
                        }
            });
        if expired {
            self.uncertain(io);
        }
    }

    fn decide(&mut self, io: &mut Vec<Effect>) {
        if !matches!(self.connection, Connection::Online(_))
            || !self
                .run
                .as_ref()
                .is_some_and(|run| matches!(run.stage, Stage::Running))
        {
            return;
        }
        if self.setup.is_none() {
            self.uncertain(io);
            return;
        }
        if let Some(blocker) = self.safety_blockers().first().copied() {
            self.refusal = Some(blocker_refusal(blocker));
            self.drain(AfterDrain::Pause(blocker_pause(blocker)), io);
            return;
        }
        if !self.idle
            || self.barrier.is_some()
            || self.run.as_ref().is_some_and(|run| run.pending.is_some())
        {
            return;
        }
        let planned = self.plan();
        if self
            .setup
            .as_ref()
            .is_some_and(|setup| setup.draft().goal.is_some())
            && self.book.inventory.pending()
            && !matches!(planned, Ok(Some(Action::HpPotion(_))))
        {
            return;
        }
        let action = match planned {
            Ok(Some(action)) => action,
            Ok(None) => return,
            Err(blocker) => {
                self.refusal = Some(blocker_refusal(blocker));
                self.drain(AfterDrain::Pause(Pause::Uncertain), io);
                return;
            }
        };
        let id = ActionKey(self.mint());
        let receipt_by = At(self.now.0.saturating_add(RECEIPT_MS));
        let incarnation = match action {
            Action::BasicAttack(key) => self.book.incarnation(key),
            _ => 0,
        };
        if let Some(run) = &mut self.run {
            run.pending = Some(Pending {
                id,
                action,
                incarnation,
                receipt_by,
                finish_by: receipt_by,
                sent: None,
                terminal: None,
                dig_result: None,
                quarantined: false,
                early: Vec::new(),
            });
        }
        self.idle = false;
        self.emit(
            SessionOp::Act(ActionTicket {
                id,
                action,
                not_after: receipt_by,
            }),
            io,
        );
    }

    fn wake_at(&self) -> Option<At> {
        if !matches!(self.connection, Connection::Online(_)) {
            return None;
        }
        let mut deadlines = Vec::new();
        if let Some(barrier) = &self.barrier {
            deadlines.push(barrier.deadline);
        }
        if let Some(run) = &self.run {
            if let Some(pending) = &run.pending
                && pending.terminal.is_none()
                && !pending.quarantined
            {
                deadlines.push(if pending.sent.is_none() {
                    pending.receipt_by
                } else {
                    pending.finish_by
                });
            }
            if matches!(run.stage, Stage::Running)
                && let Some(fresh_until) = self.book.fresh_until()
            {
                deadlines.push(fresh_until);
            }
        }
        deadlines
            .into_iter()
            .filter(|deadline| *deadline > self.now)
            .min()
    }
}

fn subject_matches(action: Action, subject: Subject) -> bool {
    match (action, subject) {
        (Action::MoveTo(_), Subject::Movement) | (Action::DigAt(_), Subject::Dig) => true,
        (Action::BasicAttack(a), Subject::Target(b)) => a == b,
        (Action::Pickup(a), Subject::Drop(b)) => a == b,
        (Action::HpPotion(a), Subject::Potion(b)) => a == b,
        _ => false,
    }
}
fn zero_totals() -> Totals {
    Totals {
        sent: 0,
        confirmed: 0,
        kills: 0,
        digs: 0,
        finds: 0,
        empty: 0,
        goal_gained: 0,
        elapsed_ms: 0,
    }
}

fn blocker_refusal(blocker: Blocker) -> Refusal {
    match blocker {
        Blocker::NeedsSetup => Refusal::SetupInvalid,
        Blocker::NeedsConnection => Refusal::WrongPhase,
        Blocker::NeedsQuiescence | Blocker::NeedsActionDrain => Refusal::PendingAction,
        Blocker::NeedsCapability(_) => Refusal::Unsupported,
        Blocker::NeedsBoundaryMap(_) | Blocker::NeedsInsideBoundary => Refusal::UnsafeArea,
        _ => Refusal::MissingState,
    }
}
fn blocker_pause(blocker: Blocker) -> Pause {
    match blocker {
        Blocker::NeedsLivingCharacter => Pause::Fainted,
        Blocker::NeedsHpAbove { .. }
        | Blocker::NeedsHpPotions
        | Blocker::NeedsObservation(StateKind::HpPotionStack)
        | Blocker::NeedsCapability(Capability::HpPotion) => Pause::HpPotions,
        Blocker::NeedsWeightBelow { .. } => Pause::Weight,
        Blocker::NeedsEquippedDrill | Blocker::NeedsDrillLife { .. } => Pause::Drill,
        Blocker::NeedsDrillDepth => Pause::Depth,
        Blocker::GoalReached { .. } => Pause::Goal,
        _ => Pause::Uncertain,
    }
}

fn outcome_matches(action: Action, outcome: Outcome) -> bool {
    match (action, outcome) {
        (Action::MoveTo(expected), Outcome::Arrived(actual)) => expected == actual,
        (Action::BasicAttack(expected), Outcome::OwnHit(actual) | Outcome::OwnKill(actual)) => {
            expected == actual
        }
        (Action::DigAt(_), Outcome::DigEnded(_)) => true,
        (Action::Pickup(expected), Outcome::OwnPickup(actual)) => expected == actual,
        (Action::HpPotion(expected), Outcome::PotionUsed(actual)) => expected == actual,
        (action, Outcome::Rejected { subject, .. }) => subject_matches(action, subject),
        _ => false,
    }
}
