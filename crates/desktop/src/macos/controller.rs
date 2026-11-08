use opentrickster_engine::{
    At, Command, Effect, Engine, Input, Mode, Observation, RequestKey, Snapshot,
};
use std::{collections::VecDeque, time::Instant};

pub struct Controller {
    engine: Engine,
    pub snapshot: Snapshot,
    clock: Instant,
    request: u64,
    inbox: VecDeque<Input>,
}

impl Controller {
    pub fn new() -> Self {
        let engine = Engine::new(Mode::Assisted);
        let snapshot = engine.snapshot();
        Self {
            engine,
            snapshot,
            clock: Instant::now(),
            request: 0,
            inbox: VecDeque::new(),
        }
    }
    pub fn now(&self) -> At {
        At(self.clock.elapsed().as_millis().min(u64::MAX as u128) as u64)
    }
    pub fn at_ingress(&self, ingress: Instant) -> At {
        At(ingress
            .checked_duration_since(self.clock)
            .unwrap_or_default()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64)
    }
    pub fn command(&mut self, command: Command) {
        self.request += 1;
        self.inbox.push_back(Input::User {
            at: self.now(),
            request: RequestKey(self.request),
            command,
        });
    }
    pub fn observe(&mut self, observation: Observation) {
        self.observe_at(self.now(), observation);
    }
    pub fn observe_at(&mut self, at: At, observation: Observation) {
        self.inbox.push_back(Input::Observed { at, observation });
    }
    pub fn wake(&mut self) {
        self.inbox.push_back(Input::Wake(self.now()));
    }
    pub fn next(&mut self) -> Option<Vec<Effect>> {
        let effects = self.engine.handle(self.inbox.pop_front()?);
        self.snapshot = effects.snapshot;
        Some(effects.io)
    }
}
