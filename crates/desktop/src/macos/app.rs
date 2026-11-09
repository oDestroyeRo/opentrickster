mod adapter;

use super::{
    controller::Controller,
    embedded_input::EmbeddedInput,
    host::{EmbeddedHost, HostEpoch, HostEvent, HostInbox, HostQueue, HostSource, PageIdentity},
    input::TextInput,
    macos_native,
    network::{Background, Completion, Prepared},
    verification::{self, Probe},
};
use engine::{
    Command, Control, Effect, Event, Mode, Observation, Phase, SessionKey, SessionOp, Snapshot,
};
use gpui::{prelude::*, *};
use opentrickster_engine as engine;
use opentrickster_net::{GameObserver, NativeSession, NetCredentials, NetError};
use std::{
    cell::Cell,
    collections::BTreeSet,
    num::NonZeroU16,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

actions!(
    desktop,
    [
        StartResume,
        Pause,
        Stop,
        RunTab,
        SetupTab,
        LogTab,
        TogglePanel,
        ToggleFocus,
        Quit
    ]
);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Run,
    Setup,
    Log,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LogFilter {
    All,
    Confirmed,
    Problems,
}

pub struct SetupForm {
    pub digging: bool,
    pub except: bool,
    pub species: BTreeSet<engine::SpeciesKey>,
    pub radius: Entity<TextInput>,
    pub hp: Entity<TextInput>,
    pub weight: Entity<TextInput>,
    pub drill: Entity<TextInput>,
    pub goal_kind: Option<engine::ItemKind>,
    pub goal_count: Entity<TextInput>,
    pub center: Option<(engine::MapKey, engine::Point)>,
    pub alerts: [bool; 4],
}

enum Adapter {
    Preparing {
        session: SessionKey,
        cancel: Arc<AtomicBool>,
    },
    Embedded {
        session: SessionKey,
        host: EmbeddedHost,
        observer: GameObserver,
        input: EmbeddedInput,
    },
    Native {
        session: SessionKey,
        native: NativeSession,
    },
    Closing {
        session: SessionKey,
        started: std::time::Instant,
    },
    ClosingEmbedded {
        session: SessionKey,
        host: EmbeddedHost,
        observer: GameObserver,
        input: EmbeddedInput,
        stage: CloseStage,
    },
}
impl Adapter {
    fn session(&self) -> SessionKey {
        match self {
            Self::Preparing { session, .. }
            | Self::Embedded { session, .. }
            | Self::Native { session, .. }
            | Self::Closing { session, .. }
            | Self::ClosingEmbedded { session, .. } => *session,
        }
    }
}

enum CloseStage {
    Releasing,
    Sockets {
        request: u64,
        started: std::time::Instant,
    },
    Neutralizing,
    /// The WebContent process is gone, so no socket or neutralization notice can arrive.
    Abandoned,
}

enum Runtime {
    Live(Box<Controller>),
    Verification(Box<Snapshot>),
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct MenuState {
    panel: bool,
    resume: bool,
    end: bool,
    start_allowed: bool,
    pause_allowed: bool,
    stop_allowed: bool,
}
pub struct Desktop {
    runtime: Runtime,
    adapter: Option<Adapter>,
    pub inbox: HostInbox,
    pub tab: Tab,
    pub panel: bool,
    pub game_keys: bool,
    pub focus: FocusHandle,
    pub filter: LogFilter,
    pub form: SetupForm,
    pub account: Entity<TextInput>,
    pub password: Entity<TextInput>,
    pub message: Option<(&'static str, u64)>,
    pub mode_sheet: Option<(Mode, Rc<Cell<Option<bool>>>)>,
    pub probe: Option<Probe>,
    pub ticks: u64,
    next_host: u64,
    next_close: u64,
    pub alerts: super::alerts::Alerts,
    background: Background,
    credentials: Option<NetCredentials>,
    quit_requested: bool,
    pub client_failure: Option<&'static str>,
    cache: Option<std::path::PathBuf>,
    menu_fingerprint: Cell<Option<MenuState>>,
}

impl Desktop {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        options: Option<verification::Options>,
    ) -> Self {
        let verify = options.as_ref().is_some_and(|options| !options.application);
        let cache = options
            .as_ref()
            .filter(|o| o.application)
            .map(|o| o.directory.join("public-cache"))
            .or_else(|| super::network::public_cache().ok());
        let mut app = Self {
            runtime: if verify {
                Runtime::Verification(Box::new(verification::offline_snapshot()))
            } else {
                Runtime::Live(Box::new(Controller::new()))
            },
            adapter: None,
            inbox: Rc::new(HostQueue::new()),
            tab: Tab::Run,
            panel: true,
            game_keys: false,
            focus: cx.focus_handle(),
            filter: LogFilter::All,
            form: SetupForm {
                digging: false,
                except: false,
                species: BTreeSet::new(),
                radius: cx.new(|cx| TextInput::new("20", false, cx)),
                hp: cx.new(|cx| TextInput::new("35", false, cx)),
                weight: cx.new(|cx| TextInput::new("85", false, cx)),
                drill: cx.new(|cx| TextInput::new("10", false, cx)),
                goal_kind: None,
                goal_count: cx.new(|cx| TextInput::new("1", false, cx)),
                center: None,
                alerts: [true; 4],
            },
            account: cx.new(|cx| TextInput::new("", false, cx)),
            password: cx.new(|cx| TextInput::new("", true, cx)),
            message: None,
            mode_sheet: None,
            probe: options.map(Probe::new),
            ticks: 0,
            next_host: 1,
            next_close: 0,
            alerts: super::alerts::Alerts::new(),
            background: Background::new(),
            credentials: None,
            quit_requested: false,
            client_failure: None,
            menu_fingerprint: Cell::new(None),
            cache,
        };
        app.focus.focus(window);
        app.update_menus(cx);
        if let Some(probe) = app.probe.as_mut().filter(|p| !p.options.application) {
            let source = if probe.options.input {
                HostSource::InputFixture
            } else if probe.options.target {
                HostSource::Official { probe: true }
            } else {
                HostSource::OfflineFixture
            };
            match PageIdentity::fresh(HostEpoch(1)).and_then(|identity| {
                EmbeddedHost::open(window, app.inbox.clone(), source, identity, None)
            }) {
                Ok(host) => probe.host = Some(host),
                Err(_) => probe.fail("child-create"),
            }
        }
        cx.observe_window_bounds(window, |_, _, cx| cx.notify())
            .detach();
        cx.observe_window_appearance(window, |_, _, cx| cx.notify())
            .detach();
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            weak.update(cx, |app, _| {
                app.request_quit();
                let closed = app.adapter.is_none();
                if closed {
                    macos_native::allow_termination();
                }
                closed
            })
            .unwrap_or(true)
        });
        cx.on_app_quit(|app, cx| {
            // Resource cleanup has already finished at the native termination gate.
            if let Some(mut probe) = app.probe.take() {
                probe.application_will_quit(app, cx);
                app.probe = Some(probe);
            }
            async {}
        })
        .detach();
        cx.on_release(|app, _| app.close_resources()).detach();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this
                    .update_in(cx, |app, window, cx| app.tick(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(10))
                    .await;
                if this
                    .update_in(cx, |app, window, cx| {
                        if let Some(probe) = &mut app.probe {
                            probe.input_watchdog();
                        }
                        app.pump_embedded_input(window, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        if app
            .probe
            .as_ref()
            .is_some_and(|probe| probe.options.application)
        {
            app.command(Command::Open, window, cx);
        }
        app
    }

    pub fn snapshot(&self) -> &Snapshot {
        match &self.runtime {
            Runtime::Live(controller) => &controller.snapshot,
            Runtime::Verification(snapshot) => snapshot,
        }
    }
    pub fn controller(&self) -> Option<&Controller> {
        match &self.runtime {
            Runtime::Live(controller) => Some(controller),
            _ => None,
        }
    }
    fn controller_mut(&mut self) -> Option<&mut Controller> {
        match &mut self.runtime {
            Runtime::Live(controller) => Some(controller),
            _ => None,
        }
    }
    pub fn allowed(&self, control: Control) -> bool {
        self.snapshot()
            .allowed
            .iter()
            .any(|value| std::mem::discriminant(value) == std::mem::discriminant(&control))
    }
    pub fn host(&self) -> Option<&EmbeddedHost> {
        match &self.adapter {
            Some(Adapter::Embedded { host, .. }) | Some(Adapter::ClosingEmbedded { host, .. }) => {
                Some(host)
            }
            _ => self.probe.as_ref().and_then(|p| p.host.as_ref()),
        }
    }
    pub fn close_resources(&mut self) {
        self.request_quit();
        if let Some(probe) = &mut self.probe {
            probe.host.take();
        }
        macos_native::badge(false);
    }
    pub fn resources_active(&self) -> bool {
        self.adapter.is_some()
    }
    pub fn is_quitting(&self) -> bool {
        self.quit_requested
    }
    pub fn say(&mut self, message: &'static str) {
        self.message = Some((message, self.ticks + 22));
    }
    pub fn command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        let starting = matches!(&command, Command::Start) && self.allowed(Control::Start)
            || matches!(&command, Command::Resume) && self.allowed(Control::Resume);
        if starting
            && let Some(host) = self.host()
            && let Some(web) = host.child.borrow().as_ref()
        {
            let _ = web.focus();
        }
        let revoke = match &command {
            Command::SwitchMode(_) => self.allowed(Control::SwitchMode),
            Command::CancelConnect => self.allowed(Control::CancelConnect),
            _ => false,
        };
        if revoke {
            self.credentials.take();
            self.password.update(cx, |input, cx| input.clear(cx));
        }
        if let Some(controller) = self.controller_mut() {
            controller.command(command);
            self.drain(window, cx);
        } else if let Some(probe) = &mut self.probe {
            probe.commands += 1;
        }
        cx.notify();
    }
    fn observe(&mut self, session: SessionKey, event: Event) {
        if let Some(controller) = self.controller_mut() {
            controller.observe(Observation { session, event });
        }
    }
    fn drain(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = false;
        while let Some(effects) = self.controller_mut().and_then(Controller::next) {
            changed = true;
            for effect in effects {
                self.execute(effect, window, cx);
            }
        }
        if changed {
            self.update_menus(cx);
        }
    }
    pub fn can_authenticate(&self) -> bool {
        self.allowed(Control::Open) || self.allowed(Control::DismissHalt)
    }
    pub fn load_game(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.client_failure = None;
        if self.allowed(Control::DismissHalt) {
            self.command(Command::DismissHalt, window, cx);
        }
        if self.allowed(Control::Open) {
            self.command(Command::Open, window, cx);
        } else if self.allowed(Control::Reconnect) {
            self.command(Command::Reconnect, window, cx);
        }
    }
    pub fn sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.allowed(Control::DismissHalt) {
            self.command(Command::DismissHalt, window, cx);
        }
        if !self.allowed(Control::Open) {
            return;
        }
        let id = zeroize::Zeroizing::new(self.account.read(cx).value().to_owned());
        let password =
            zeroize::Zeroizing::new(self.password.update(cx, |input, cx| input.take(cx)));
        let Ok(credentials) = NetCredentials::new((*id).clone(), (*password).clone()) else {
            self.say("Enter a valid Game ID and password.");
            return;
        };
        self.credentials = Some(credentials);
        self.command(Command::Open, window, cx);
    }
    pub fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ticks += 1;
        if macos_native::termination_requested() {
            self.request_quit();
        }
        let stalled = match &self.adapter {
            Some(Adapter::ClosingEmbedded {
                stage: CloseStage::Sockets { started, .. },
                ..
            })
            | Some(Adapter::Closing { started, .. }) => started.elapsed() > Duration::from_secs(5),
            _ => false,
        };
        if stalled {
            self.say(if self.quit_requested {
                "Waiting for the game connections to close. The app stays open until cleanup is confirmed."
            } else {
                "Waiting for the game connections to close. The next mode remains locked."
            });
        }
        if let Some(controller) = self.controller_mut() {
            controller.wake();
        }
        self.poll_network(window);
        self.game_keys = self
            .host()
            .and_then(|host| {
                host.child
                    .borrow()
                    .as_ref()
                    .map(macos_native::game_has_focus)
            })
            .unwrap_or(false);
        if self.message.is_some_and(|(_, until)| self.ticks >= until) {
            self.message = None;
        }
        if let Some((mode, result)) = &self.mode_sheet
            && let Some(approved) = result.take()
        {
            let mode = *mode;
            self.mode_sheet = None;
            if approved {
                self.command(Command::SwitchMode(mode), window, cx);
            }
        }
        self.poll_host(window, cx);
        self.drain(window, cx);
        if let Some(mut probe) = self.probe.take() {
            probe.tick(self, window, cx);
            self.probe = Some(probe);
        }
        if self.quit_requested && self.adapter.is_none() {
            macos_native::badge(false);
            macos_native::allow_termination();
            cx.quit();
        }
        cx.notify();
    }
    fn poll_host(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        let events = self.inbox.drain();
        for message in events {
            if !self.host().is_some_and(|host| host.epoch == message.epoch) {
                continue;
            }
            let source_at = self
                .controller()
                .map(|c| c.at_ingress(message.ingress))
                .unwrap_or(engine::At(0));
            let event = message.event;
            if let Some(probe) = &mut self.probe {
                probe.observe(&event);
            }
            match event {
                HostEvent::NetworkPage { origin, json } => {
                    self.ingest_page(&origin, &json, message.ingress)
                }
                HostEvent::Neutralized => self.finish_embedded_close(),
                HostEvent::FullscreenRequested => {
                    self.panel = false;
                    self.say("The game stays inside this window. Show panel with ⌘\\.");
                }
                HostEvent::PhysicalInput => {
                    self.game_keys = true;
                    if let Some(Adapter::Embedded { input, .. }) = &mut self.adapter {
                        input.fence();
                    }
                    if matches!(
                        self.snapshot().phase,
                        Phase::Starting
                            | Phase::Running
                            | Phase::Pausing
                            | Phase::Paused(_)
                            | Phase::Stopping
                    ) && let Some(session) = self.adapter.as_ref().map(Adapter::session)
                        && let Some(controller) = self.controller_mut()
                    {
                        controller.observe_at(
                            source_at,
                            Observation {
                                session,
                                event: Event::ManualInput,
                            },
                        );
                    }
                }
                HostEvent::PageReady => {
                    if let Some(Adapter::Embedded { session, .. }) = &self.adapter {
                        self.observe(
                            *session,
                            Event::Progress(engine::ConnectStep::AwaitingGameLogin),
                        );
                    }
                }
                HostEvent::ProcessFailed => {
                    self.client_failed(true, "The game's web content process ended.")
                }
                HostEvent::PlacementFailed => {
                    self.client_failed(false, "The game view could not be placed in the window.")
                }
                HostEvent::Overflow => self.client_failed(
                    false,
                    "The game sent more page messages than the app could check.",
                ),
                HostEvent::Message(_) => {}
            }
        }
    }
    pub fn switch_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        if self.snapshot().mode == mode {
            return;
        }
        if !self.allowed(Control::SwitchMode) {
            self.say("Mode switch is off during a run. Stop the run first.");
            return;
        }
        if self.adapter.is_none() {
            self.command(Command::SwitchMode(mode), window, cx);
            return;
        }
        if self.mode_sheet.is_some() {
            return;
        }
        let result = Rc::new(Cell::new(None));
        if macos_native::confirm_mode(window, mode == Mode::Clientless, result.clone()) {
            self.mode_sheet = Some((mode, result));
        } else {
            self.say("The mode confirmation could not open.");
        }
    }
    pub fn toggle_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.game_keys {
            macos_native::focus_dashboard(window);
            self.focus.focus(window);
            self.game_keys = false;
        } else if let Some(host) = self.host() {
            let focused = host
                .child
                .borrow()
                .as_ref()
                .is_some_and(|web| web.focus().is_ok());
            self.game_keys = focused;
        }
        cx.notify();
    }
    pub fn apply_setup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((map, center)) = self.form.center else {
            self.say("Choose the current location after entering the world.");
            return;
        };
        let radius = self
            .form
            .radius
            .read(cx)
            .value()
            .parse::<u16>()
            .ok()
            .and_then(NonZeroU16::new);
        let hp = self
            .form
            .hp
            .read(cx)
            .value()
            .parse::<u8>()
            .ok()
            .and_then(|p| engine::Percent::checked(p).ok());
        let weight = self
            .form
            .weight
            .read(cx)
            .value()
            .parse::<u8>()
            .ok()
            .and_then(|p| engine::Percent::checked(p).ok());
        let life = self.form.drill.read(cx).value().parse::<u32>().ok();
        let (Some(radius), Some(hp_at), Some(weight_limit), Some(life)) =
            (radius, hp, weight, life)
        else {
            self.say("Enter a positive radius and valid percentage and drill limits.");
            return;
        };
        let Ok(boundary) = engine::Boundary::checked(map, center, radius) else {
            self.say("This movement boundary is not valid.");
            return;
        };
        let targets = self.form.species.iter().copied().collect();
        let activity = if self.form.digging {
            engine::Activity::Digging {
                min_drill_life: life,
            }
        } else {
            engine::Activity::Combat {
                targets: if self.form.except {
                    engine::Targets::Except(targets)
                } else {
                    engine::Targets::Only(targets)
                },
            }
        };
        let goal = if let Some(item) = self.form.goal_kind {
            let Ok(gained) = self.form.goal_count.read(cx).value().parse::<u32>() else {
                self.say("Enter a positive item goal.");
                return;
            };
            if gained == 0 {
                self.say("The item goal must be at least one.");
                return;
            }
            Some(engine::Goal { item, gained })
        } else {
            None
        };
        match engine::Setup::checked(engine::SetupDraft {
            activity,
            boundary,
            hp_at,
            weight_limit,
            goal,
        }) {
            Ok(setup) => self.command(Command::Configure(setup), window, cx),
            Err(_) => self.say("Setup is incomplete. Choose an area and eligible targets."),
        }
    }
    fn start_resume(&mut self, _: &StartResume, window: &mut Window, cx: &mut Context<Self>) {
        self.command(
            if matches!(self.snapshot().phase, Phase::Paused(_)) {
                Command::Resume
            } else {
                Command::Start
            },
            window,
            cx,
        );
    }
    fn pause(&mut self, _: &Pause, window: &mut Window, cx: &mut Context<Self>) {
        self.command(Command::Pause, window, cx);
    }
    fn stop(&mut self, _: &Stop, window: &mut Window, cx: &mut Context<Self>) {
        self.command(Command::Stop, window, cx);
    }
    fn run_tab(&mut self, _: &RunTab, _: &mut Window, cx: &mut Context<Self>) {
        self.tab = Tab::Run;
        self.panel = true;
        cx.notify();
    }
    fn setup_tab(&mut self, _: &SetupTab, _: &mut Window, cx: &mut Context<Self>) {
        self.tab = Tab::Setup;
        self.panel = true;
        cx.notify();
    }
    fn log_tab(&mut self, _: &LogTab, _: &mut Window, cx: &mut Context<Self>) {
        self.tab = Tab::Log;
        self.panel = true;
        cx.notify();
    }
    fn toggle_panel(&mut self, _: &TogglePanel, _: &mut Window, cx: &mut Context<Self>) {
        self.panel = !self.panel;
        cx.notify();
    }
    fn quit_action(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<Self>) {
        self.request_quit();
        cx.notify();
    }
    fn focus_action(&mut self, _: &ToggleFocus, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_focus(window, cx);
    }
    pub fn update_menus(&self, cx: &mut App) {
        let resume = matches!(self.snapshot().phase, Phase::Paused(_));
        let end = matches!(
            self.snapshot().phase,
            Phase::Reconnecting(_) | Phase::Disconnected
        );
        let fingerprint = MenuState {
            panel: self.panel,
            resume,
            end,
            start_allowed: self.allowed(Control::Start) || self.allowed(Control::Resume),
            pause_allowed: self.allowed(Control::Pause),
            stop_allowed: self.allowed(Control::Stop),
        };
        if self.menu_fingerprint.replace(Some(fingerprint)) == Some(fingerprint) {
            return;
        }
        cx.set_menus(vec![
            Menu {
                name: "OpenTrickster".into(),
                items: vec![MenuItem::action("Quit OpenTrickster", Quit)],
            },
            Menu {
                name: "Run".into(),
                items: vec![
                    MenuItem::action(if resume { "Resume" } else { "Start" }, StartResume),
                    MenuItem::action("Pause", Pause),
                    MenuItem::action(if end { "End run" } else { "Stop" }, Stop),
                ],
            },
            Menu {
                name: "View".into(),
                items: vec![
                    MenuItem::action("Run", RunTab),
                    MenuItem::action("Setup", SetupTab),
                    MenuItem::action("Log", LogTab),
                    MenuItem::separator(),
                    MenuItem::action(
                        if self.panel {
                            "Hide panel"
                        } else {
                            "Show panel"
                        },
                        TogglePanel,
                    ),
                    MenuItem::action("Switch keyboard focus", ToggleFocus),
                ],
            },
        ]);
    }
}

pub fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-r", StartResume, None),
        KeyBinding::new("cmd-p", Pause, None),
        KeyBinding::new("cmd-.", Stop, None),
        KeyBinding::new("cmd-1", RunTab, None),
        KeyBinding::new("cmd-2", SetupTab, None),
        KeyBinding::new("cmd-3", LogTab, None),
        KeyBinding::new("cmd-\\", TogglePanel, None),
        KeyBinding::new("ctrl-tab", ToggleFocus, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
}

impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.update_menus(cx);
        let allow_start = self.allowed(Control::Start) || self.allowed(Control::Resume);
        let allow_pause = self.allowed(Control::Pause);
        let allow_stop = self.allowed(Control::Stop);
        let root = self.render_view(window, cx);
        root.track_focus(&self.focus)
            .key_context("Desktop")
            .when(allow_start, |root| {
                root.on_action(cx.listener(Self::start_resume))
            })
            .when(allow_pause, |root| root.on_action(cx.listener(Self::pause)))
            .when(allow_stop, |root| root.on_action(cx.listener(Self::stop)))
            .on_action(cx.listener(Self::run_tab))
            .on_action(cx.listener(Self::setup_tab))
            .on_action(cx.listener(Self::log_tab))
            .on_action(cx.listener(Self::toggle_panel))
            .on_action(cx.listener(Self::focus_action))
            .on_action(cx.listener(Self::quit_action))
            .on_key_down(cx.listener(|app, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "tab"
                    && !event.keystroke.modifiers.control
                    && !event.keystroke.modifiers.platform
                {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev();
                    } else {
                        window.focus_next();
                    }
                    cx.stop_propagation();
                }
                let _ = app;
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, event: &MouseDownEvent, window, _| {
                    let in_game = app
                        .host()
                        .and_then(|host| *host.placement.borrow())
                        .is_some_and(|r| {
                            let x: f32 = event.position.x.into();
                            let y: f32 = event.position.y.into();
                            x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
                        });
                    if !in_game && app.game_keys {
                        macos_native::focus_dashboard(window);
                        app.game_keys = false;
                    }
                }),
            )
    }
}
