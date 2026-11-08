use opentrickster_engine::Mode;
use std::rc::Rc;

#[derive(Clone, Default)]
pub(super) struct Revision(Rc<()>);

impl Revision {
    pub fn invalidate(&mut self) {
        *self = Self::default();
    }

    fn matches(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

pub(super) struct CredentialLoad {
    generation: Revision,
    account: Revision,
    password: Revision,
}

impl CredentialLoad {
    pub fn new(generation: Revision, account: Revision, password: Revision) -> Self {
        Self {
            generation,
            account,
            password,
        }
    }

    pub fn accepts(&self, current: &Self, mode: Mode, quitting: bool, allowed: bool) -> bool {
        mode == Mode::Clientless
            && !quitting
            && allowed
            && self.generation.matches(&current.generation)
            && self.account.matches(&current.account)
            && self.password.matches(&current.password)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_current_clientless_request_can_load() {
        let generation = Revision::default();
        let account = Revision::default();
        let password = Revision::default();
        let request = CredentialLoad::new(generation.clone(), account.clone(), password.clone());
        let current = CredentialLoad::new(generation, account, password);
        assert!(request.accepts(&current, Mode::Clientless, false, true));
        assert!(!request.accepts(&current, Mode::Assisted, false, true));
        assert!(!request.accepts(&current, Mode::Clientless, true, true));
        assert!(!request.accepts(&current, Mode::Clientless, false, false));
    }

    #[test]
    fn revocation_and_edits_cannot_restore_an_old_request() {
        let mut generation = Revision::default();
        let mut account = Revision::default();
        let mut password = Revision::default();
        for change in 0..3 {
            let request =
                CredentialLoad::new(generation.clone(), account.clone(), password.clone());
            match change {
                0 => generation.invalidate(),
                1 => account.invalidate(),
                _ => password.invalidate(),
            }
            let current =
                CredentialLoad::new(generation.clone(), account.clone(), password.clone());
            assert!(!request.accepts(&current, Mode::Clientless, false, true));
        }
    }
}
