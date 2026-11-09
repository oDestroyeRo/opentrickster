//! Pinned-build network boundary. Raw identities and authentication never leave this crate.
//!
//! Integration contract (all methods are synchronous):
//! - `NetCredentials::new(id: String, password: String) -> Result<Self, NetError>`
//! - `VerifiedProfile::load_cache(cache: &Path) -> Result<Self, NetError>`
//! - `VerifiedProfile::fetch_public(cache: &Path) -> Result<Self, NetError>`
//! - `NativeSession::open(session: SessionKey, profile: VerifiedProfile,
//!   credentials: NetCredentials) -> Result<Self, NetError>`
//! - `NativeSession::submit(&self, op: SessionOp) -> Result<(), NetError>`
//! - `NativeSession::submit_at(&self, at: At, op: SessionOp) -> Result<(), NetError>`
//!   supplies the engine clock for gameplay deadlines; `submit(Act)` refuses.
//! - `NativeSession::try_next(&mut self) -> Result<Option<Observation>, NetError>`
//! - `NativeSession::try_next_at(&mut self, now: At) -> Result<Option<(At, Observation)>, NetError>`
//!   is the canonical runtime drain and preserves original ingress age.
//! - `NativeSession::close(&mut self) -> Result<(), NetError>`
//!
//! Profile loading/fetching and bounded close must run off the GPUI thread.
//! Open starts a private runtime thread; submit and try_next never block.
//! Queues are bounded. Overflow fails closed instead of dropping observations.
//! `Closed` reports local cleanup only, never server logout.
//!
//! The passive observer has no transport or authentication ownership:
//! - `GameObserver::new(session: SessionKey, profile: VerifiedProfile) -> Self`
//! - `ObserverContext::checked(origin: &str, build: &str, connection: ConnectionKey)
//!   -> Result<Self, NetError>`
//! - `GameObserver::connect(&mut self, context: ObserverContext) -> Result<(), NetError>`
//! - `GameObserver::disconnect(&mut self, connection: ConnectionKey) -> Result<(), NetError>`
//! - `GameObserver::observe_binary(&mut self, connection: ConnectionKey,
//!   direction: ObservedDirection, bytes: &[u8]) -> Result<(), NetError>`
//! - `GameObserver::arm_action(&mut self, ticket: ActionTicket) -> Result<(), NetError>`
//! - `GameObserver::try_next(&mut self) -> Result<Option<Observation>, NetError>`
//! - `GameObserver::try_next_at(&mut self, now: At) -> Result<Option<(At, Observation)>, NetError>`
//! - `GameObserver::close(&mut self) -> Result<(), NetError>`
//!
//! The document-bound input API returns one normal native event per plan:
//! - `sample_script(&mut self, request: u64, layout_epoch: u64) -> Result<ObserverScript, NetError>`
//! - `input_plan(&mut self, now: At, fences: InputFences, ticket: ActionTicket) -> Result<InputPlan, NetError>`
//! - `input_dispatched(&mut self, step: InputStepKey, captured_at: Instant) -> Result<(), NetError>`
//! - `continue_input(&mut self, now: At, fences: InputFences, action: ActionKey) -> Result<Option<InputPlan>, NetError>`
//! - `active_input(&self) -> Option<ActionKey>`
//! - `quiesce(&mut self, now: At, fences: InputFences, barrier: BarrierKey) -> Result<Option<InputPlan>, NetError>`
//! - `emergency_released(&mut self, action: ActionKey, captured_at: Instant) -> Result<(), NetError>`
//!
//! Each actual dispatch invalidates old samples and outstanding sample requests.
//! Request a new guarded sample afterward. None waits; `active_input()==None` is done.
//! Transient planning errors never poison the passive decoder. The host fences its
//! queue before cancellation and retains token-owned releases across every failure.
mod actions;
mod auth;
mod catalog;
mod codec;
mod combat;
mod decode;
mod definitions;
mod dig;
mod input;
mod input_flow;
mod observer;
mod ordered;
mod page;
mod profile;
mod resources;
mod sampler;
mod session;
#[cfg(test)]
mod test_support;
mod transport;
mod wire;

pub use auth::NetCredentials;
pub use decode::{MissingPrerequisite, Readiness};
pub use definitions::{ItemDescriptor, RecoveryKind};
pub use input::{InputFences, InputKey, InputPlan, InputStepKey, NormalInput};
pub use observer::{ConnectionKey, GameObserver, ObservedDirection, ObserverContext};
pub use page::{ObserverScript, PageBinding, PageFault, PageNotice};
pub use profile::{BUILD, VerifiedProfile};
pub use session::NativeSession;

/// Safe to log. This enum deliberately carries no remote text, identifiers or payloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetError {
    BuildChanged,
    InvalidCredentials,
    Malformed,
    Truncated,
    Capacity,
    Unsupported,
    WrongPhase,
    StaleConnection,
    Occupied,
    Busy,
    SecurityRequired,
    Refused,
    Timeout,
    Transport,
    Cancelled,
    Closed,
    DuplicateAction,
    Uncertain,
    NotReady,
    Io,
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for NetError {}

type Result<T> = std::result::Result<T, NetError>;

struct TimedEvent {
    event: opentrickster_engine::Event,
    received: std::time::Instant,
}
impl TimedEvent {
    fn at(
        self,
        now: opentrickster_engine::At,
        session: opentrickster_engine::SessionKey,
    ) -> (opentrickster_engine::At, opentrickster_engine::Observation) {
        let age = self
            .received
            .elapsed()
            .as_nanos()
            .div_ceil(1_000_000)
            .min(u64::MAX as u128) as u64;
        (
            opentrickster_engine::At(now.0.saturating_sub(age)),
            opentrickster_engine::Observation {
                session,
                event: self.event,
            },
        )
    }
}
