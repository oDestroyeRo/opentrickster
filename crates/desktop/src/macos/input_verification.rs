use super::{
    host::{EmbeddedHost, HostEvent},
    macos_native,
    normal_input::{Executor, Plan, Target},
};
use gpui::Window;
use opentrickster_engine::At;
use opentrickster_net::{InputKey, NormalInput};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(super) struct InputProbe {
    started: Instant,
    ready: bool,
    step: u64,
    executor: Option<Executor<u64>>,
    events: Vec<Value>,
    checks: Vec<(&'static str, bool)>,
    cleanup: usize,
    watchdog: Option<Instant>,
    watchdog_elapsed: Option<u128>,
}
impl InputProbe {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            ready: false,
            step: 0,
            executor: None,
            events: Vec::new(),
            checks: Vec::new(),
            cleanup: 0,
            watchdog: None,
            watchdog_elapsed: None,
        }
    }
    pub fn observe(&mut self, event: &HostEvent) {
        if let HostEvent::Message(body) = event {
            let Ok(value) = serde_json::from_str::<Value>(body) else {
                return;
            };
            if value["kind"] == "input-ready" {
                self.ready = true;
            }
            if value["kind"] == "native-input" {
                self.events.push(value);
            }
        }
    }
    pub fn cleanup(&mut self, host: &EmbeddedHost) {
        if let Some(executor) = &mut self.executor {
            let releases = executor.cleanup(Instant::now(), &mut *host.input.borrow_mut());
            self.cleanup += releases.len();
            if !releases.is_empty()
                && let Some(started) = self.watchdog.take()
            {
                self.watchdog_elapsed = Some(started.elapsed().as_millis());
            }
        }
    }
    pub fn tick(&mut self, host: &EmbeddedHost, window: &Window) -> Option<Value> {
        if self.started.elapsed() > Duration::from_secs(15) {
            return Some(
                json!({"passed":false,"failure":"native-input-timeout","account_used":false,"gameplay_proven":false,"checks":self.checks,"events":self.events}),
            );
        }
        if !self.ready {
            return None;
        }
        self.step += 1;
        let down = [
            NormalInput::PointerMove { x: 120., y: 140. },
            NormalInput::LeftDown,
        ];
        match self.step {
            1 => {
                if let Some(web) = host.child.borrow().as_ref() {
                    let _ = web.focus();
                }
            }
            3 => {
                let context = host.input.borrow().context();
                self.checks.push(("native_game_focus", context.focused));
                self.executor = Some(Executor::new(1, context));
                self.checks.push((
                    "begin_owned_action",
                    self.executor.as_mut().unwrap().begin(2, context).is_ok(),
                ));
                self.send(host, &down);
            }
            4 => self.send(host, &[NormalInput::LeftUp]),
            5 => self.send(host, &[NormalInput::KeyDown(InputKey::Character('d'))]),
            6 => self.send(host, &[NormalInput::KeyUp(InputKey::Character('D'))]),
            7 => self.send(host, &[NormalInput::RightDown]),
            8 => self.send(host, &[NormalInput::RightUp]),
            9 => {
                self.send(host, &down);
                self.watchdog = Some(Instant::now());
            }
            13 => {
                let executor = self.executor.as_mut().unwrap();
                self.checks.push((
                    "ordinary_watchdog_release",
                    executor.controls_released() && executor.uncertain() && self.cleanup == 1,
                ));
                self.checks.push((
                    "watchdog_within_250ms",
                    self.watchdog_elapsed.is_some_and(|ms| ms <= 250),
                ));
                self.checks
                    .push(("finish_after_release", executor.finish().is_ok()));
                self.checks.push((
                    "begin_next_action",
                    executor.begin(2, host.input.borrow().context()).is_ok(),
                ));
                self.send(host, &[NormalInput::KeyDown(InputKey::Character('x'))]);
            }
            14 => {
                macos_native::focus_dashboard(window);
                self.executor.as_mut().unwrap().cancel();
                self.cleanup(host);
                self.checks.push((
                    "cancel_after_focus_change_releases_original_view",
                    self.executor.as_ref().unwrap().controls_released(),
                ));
                self.send_refusal(host);
            }
            17 => {
                let sequence: Vec<_> = self
                    .events
                    .iter()
                    .filter(|e| e["event"] != "pointermove")
                    .map(|e| {
                        (
                            e["event"].as_str().unwrap_or(""),
                            e["code"].as_str(),
                            e["button"].as_i64(),
                        )
                    })
                    .collect();
                self.checks.push((
                    "native_dom_transition_sequence",
                    sequence
                        == vec![
                            ("pointerdown", None, Some(0)),
                            ("pointerup", None, Some(0)),
                            ("keydown", Some("KeyD"), None),
                            ("keyup", Some("KeyD"), None),
                            ("pointerdown", None, Some(2)),
                            ("pointerup", None, Some(2)),
                            ("pointerdown", None, Some(0)),
                            ("pointerup", None, Some(0)),
                            ("keydown", Some("KeyX"), None),
                            ("keyup", Some("KeyX"), None),
                        ],
                ));
                // A press reports its own button as held, as a physical click does.
                self.checks.push((
                    "native_press_reports_held_button",
                    self.events
                        .iter()
                        .filter(|e| e["event"] == "pointerdown" || e["event"] == "pointerup")
                        .all(|e| {
                            let held = match (e["event"].as_str(), e["button"].as_i64()) {
                                (Some("pointerdown"), Some(0)) => 1,
                                (Some("pointerdown"), Some(2)) => 2,
                                _ => 0,
                            };
                            e["buttons"].as_i64() == Some(held)
                        }),
                ));
                self.checks.push((
                    "native_dom_events_trusted",
                    !self.events.is_empty() && self.events.iter().all(|e| e["trusted"] == true),
                ));
                self.checks.push((
                    "native_pointer_hover",
                    self.events.iter().any(|e| {
                        e["event"] == "pointermove"
                            && e["x"].as_f64().is_some_and(|x| (x - 120.).abs() < 1.)
                            && e["y"].as_f64().is_some_and(|y| (y - 140.).abs() < 1.)
                    }),
                ));
                self.checks.push((
                    "css_viewport_coordinates",
                    self.events
                        .iter()
                        .filter(|e| e["event"] == "pointerdown")
                        .count()
                        == 3
                        && self
                            .events
                            .iter()
                            .filter(|e| e["event"] == "pointerdown")
                            .all(|e| {
                                e["x"].as_f64().is_some_and(|x| (x - 120.).abs() < 1.)
                                    && e["y"].as_f64().is_some_and(|y| (y - 140.).abs() < 1.)
                            }),
                ));
                self.checks.push((
                    "native_bot_events_do_not_change_physical_epoch",
                    host.input.borrow().physical_revision() == 0,
                ));
                self.checks.push((
                    "manual_event_queued_via_appkit",
                    host.input.borrow().verify_manual_event(),
                ));
            }
            19 => {
                self.checks.push((
                    "appkit_manual_input_changes_physical_epoch",
                    host.input.borrow().physical_revision() > 0,
                ));
                return Some(
                    json!({"passed":self.checks.iter().all(|(_,ok)|*ok),"verification":"offline-native-input","account_used":false,"gameplay_proven":false,"unity_receipt_proven":false,"watchdog_release_ms":self.watchdog_elapsed,"checks":self.checks,"events":self.events}),
                );
            }
            _ => {}
        }
        None
    }
    fn send(&mut self, host: &EmbeddedHost, steps: &[NormalInput]) {
        let context = host.input.borrow().context();
        let plan = Plan {
            session: 1,
            action: 2,
            step: self.step,
            host: host.epoch.0,
            layout: context.layout,
            not_after: At(1000),
            release_by: Some(At(250)),
            steps,
        };
        let sent = self
            .executor
            .as_mut()
            .unwrap()
            .execute(plan, At(0), &mut *host.input.borrow_mut())
            .is_ok();
        self.checks.push(("native_plan_dispatched", sent));
    }
    fn send_refusal(&mut self, host: &EmbeddedHost) {
        let context = host.input.borrow().context();
        let plan = Plan {
            session: 1,
            action: 2,
            step: 100,
            host: host.epoch.0,
            layout: context.layout,
            not_after: At(1000),
            release_by: Some(At(250)),
            steps: &[NormalInput::LeftDown],
        };
        let refused = self
            .executor
            .as_mut()
            .unwrap()
            .execute(plan, At(0), &mut *host.input.borrow_mut())
            .is_err();
        self.checks
            .push(("focus_loss_refuses_further_press", refused));
    }
}
