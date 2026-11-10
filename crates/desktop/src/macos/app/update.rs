use super::*;
use crate::{macos::updater::Handoff, update};
use std::time::Instant;

impl Desktop {
    pub(super) fn drive_updater(&mut self) {
        let halted = matches!(
            self.snapshot().phase,
            Phase::Error(engine::Halt::BuildChanged)
        );
        let build_changed = halted && !self.build_halt;
        self.build_halt = halted;
        let world = self.occupancy();
        for handoff in self.updater.poll(Instant::now(), world, build_changed) {
            self.hand_off(handoff);
        }
    }

    pub fn restart_to_update(&mut self, cx: &mut Context<Self>) {
        let world = self.occupancy();
        if let Some(handoff) = self.updater.restart(Instant::now(), world) {
            self.hand_off(handoff);
        }
        cx.notify();
    }

    pub fn update_notice(&self) -> Option<update::Notice> {
        update::notice(self.updater.state(), self.occupancy())
    }

    pub fn update_in_flight(&self) -> bool {
        self.updater.state().is_updating()
    }

    fn occupancy(&self) -> update::Occupancy {
        update::Occupancy {
            open: self.adapter.is_some(),
            held: matches!(
                self.snapshot().phase,
                Phase::Disconnected | Phase::Reconnecting(_)
            ),
            quitting: self.quit_requested,
        }
    }

    fn hand_off(&mut self, handoff: Handoff) {
        match handoff {
            Handoff::Close => self.request_quit(),
            Handoff::Install(staged) => match self.updater.install(&staged) {
                // The normal quit path takes over from here, and the relauncher waits for it.
                Ok(()) => self.request_quit(),
                Err(failure) => {
                    // A restart click began the quit. The game is closed, but the app stays.
                    self.quit_requested = false;
                    self.updater.install_failed(failure, Instant::now());
                }
            },
        }
    }
}
