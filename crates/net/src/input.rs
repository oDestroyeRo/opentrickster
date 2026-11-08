use opentrickster_engine::{ActionKey, At, SessionKey};

/// Host and layout generations must match at every normal native input step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputFences {
    pub host_epoch: u64,
    pub layout_epoch: u64,
    pub not_after: At,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKey {
    Character(char),
    Escape,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Enter,
    Tab,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NormalInput {
    PointerMove { x: f64, y: f64 },
    LeftDown,
    LeftUp,
    RightDown,
    RightUp,
    KeyDown(InputKey),
    KeyUp(InputKey),
}
/// A bounded plan from a fresh read-only sample. Never conveys a server action or request ID.
pub struct InputPlan {
    pub(crate) session: SessionKey,
    pub(crate) action: ActionKey,
    pub(crate) fences: InputFences,
    pub(crate) steps: Vec<NormalInput>,
    pub(crate) resample: bool,
}
impl InputPlan {
    pub fn session(&self) -> SessionKey {
        self.session
    }
    pub fn action(&self) -> ActionKey {
        self.action
    }
    pub fn fences(&self) -> InputFences {
        self.fences
    }
    pub fn steps(&self) -> &[NormalInput] {
        &self.steps
    }
    /// A hover-only preparation requires a new sample and plan before pressing anything.
    pub fn requires_resample(&self) -> bool {
        self.resample
    }
}
