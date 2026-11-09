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
    /// Physical DOM KeyA..KeyZ, case-insensitive ASCII. Never layout text insertion.
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
impl InputKey {
    /// The executor must refuse None rather than synthesize text or guess a binding.
    pub fn dom_code(self) -> Option<&'static str> {
        match self {
            Self::Character(c) if c.is_ascii_alphabetic() => Some(
                [
                    "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ",
                    "KeyK", "KeyL", "KeyM", "KeyN", "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT",
                    "KeyU", "KeyV", "KeyW", "KeyX", "KeyY", "KeyZ",
                ][c.to_ascii_uppercase() as usize - 'A' as usize],
            ),
            Self::Character(_) => None,
            Self::Escape => Some("Escape"),
            Self::Space => Some("Space"),
            Self::ArrowUp => Some("ArrowUp"),
            Self::ArrowDown => Some("ArrowDown"),
            Self::ArrowLeft => Some("ArrowLeft"),
            Self::ArrowRight => Some("ArrowRight"),
            Self::Enter => Some("Enter"),
            Self::Tab => Some("Tab"),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bindings_are_physical_ascii_letters_never_layout_text() {
        assert_eq!(InputKey::Character('d').dom_code(), Some("KeyD"));
        assert_eq!(InputKey::Character('Z').dom_code(), Some("KeyZ"));
        for c in ['ก', 'é', '1', '\0'] {
            assert_eq!(InputKey::Character(c).dom_code(), None);
        }
    }
}
/// One plan dispatch receipt. It cannot identify a server operation or raw entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputStepKey(pub(crate) u64);
/// A bounded plan from a fresh read-only sample. Never conveys a server action or request ID.
pub struct InputPlan {
    pub(crate) session: SessionKey,
    pub(crate) action: ActionKey,
    pub(crate) fences: InputFences,
    pub(crate) steps: Vec<NormalInput>,
    pub(crate) resample: bool,
    pub(crate) step: InputStepKey,
    pub(crate) release_by: Option<At>,
}
impl InputPlan {
    pub fn step_key(&self) -> InputStepKey {
        self.step
    }
    /// A cleanup deadline for an ordinary press, never evidence of Unity sampling.
    /// Gauge holds use observed gauge state instead of this ordinary-click bound.
    pub fn release_by(&self) -> Option<At> {
        self.release_by
    }
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
