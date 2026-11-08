use super::{
    app::Desktop,
    host::{ChildSlot, EmbeddedHost, HostEvent},
    macos_native,
};
use gpui::{Context, Focusable, Window, px, size};
use opentrickster_engine as e;
use serde_json::{Value, json};
use std::{
    cell::Cell,
    fs,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub struct Options {
    pub application: bool,
    pub target: bool,
    quit: Option<QuitRoute>,
    socket_port: Option<u16>,
    pub directory: PathBuf,
    pub result: Rc<Cell<Option<bool>>>,
}
#[derive(Clone, Copy)]
enum QuitRoute {
    Native,
    Command,
}
impl Options {
    pub fn parse() -> Result<Option<Self>, &'static str> {
        let mut args = std::env::args().skip(1);
        let Some(first) = args.next() else {
            return Ok(None);
        };
        if !["--verify-embedding", "--verify-application"].contains(&first.as_str()) {
            return Err(
                "Unknown option. Use --verify-embedding for account-free host verification.",
            );
        }
        let application = first == "--verify-application";
        let mut target = application;
        let mut directory = None;
        let mut quit = None;
        let mut socket_port = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--target" => target = true,
                "--quit-via" if application => {
                    quit = Some(match args.next().as_deref() {
                        Some("native") => QuitRoute::Native,
                        Some("command") => QuitRoute::Command,
                        _ => {
                            return Err(
                                "Use --quit-via native or command with --verify-application.",
                            );
                        }
                    });
                }
                "--verification-socket" if application => {
                    socket_port = Some(
                        args.next()
                            .and_then(|port| port.parse::<u16>().ok())
                            .filter(|port| *port != 0)
                            .ok_or("Use a nonzero local verification port.")?,
                    );
                }
                "--verification-dir" => {
                    directory = Some(PathBuf::from(
                        args.next().ok_or("Missing verification directory.")?,
                    ))
                }
                _ => return Err("Unknown verification option."),
            }
        }
        let directory = directory.unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "opentrickster-desktop-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            ))
        });
        fs::create_dir_all(&directory).map_err(|_| "Cannot create verification directory.")?;
        if directory.join("report.json").exists() || directory.join("game.tiff").exists() {
            return Err("Verification files already exist. Choose an empty directory.");
        }
        Ok(Some(Self {
            application,
            target,
            quit,
            socket_port,
            directory,
            result: Rc::new(Cell::new(None)),
        }))
    }
}

enum ModeProbe {
    WaitingForApproval { child: ChildSlot },
    WaitingForClose { child: ChildSlot, until: Instant },
}
struct QuitProbe {
    child: ChildSlot,
    parent: objc2::rc::Retained<objc2_app_kit::NSWindow>,
    started: Instant,
    dispatched: Rc<Cell<Option<bool>>>,
    pending_parent_preserved: bool,
}

pub struct Probe {
    pub options: Options,
    pub host: Option<EmbeddedHost>,
    start: Instant,
    failures: Vec<&'static str>,
    pub commands: u64,
    frames: u64,
    webgl: bool,
    ipc: u64,
    roundtrip: bool,
    page_key: bool,
    target_canvas: bool,
    target_instance: bool,
    snapshot: Rc<Cell<Option<bool>>>,
    sheet: Rc<Cell<Option<bool>>>,
    checks: Vec<(&'static str, bool)>,
    finished: bool,
    menu: Rc<Cell<Option<bool>>>,
    steps: u64,
    host_started: Option<Instant>,
    mode_probe: Option<ModeProbe>,
    closed_child: Option<ChildSlot>,
    pub socket_close_seen: bool,
    pub document_neutralized: bool,
    pub closed_socket_count: Option<u32>,
    quit_probe: Option<QuitProbe>,
    fixture_socket: Arc<AtomicU8>,
}
impl Probe {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            host: None,
            start: Instant::now(),
            failures: vec![],
            commands: 0,
            frames: 0,
            webgl: false,
            ipc: 0,
            roundtrip: false,
            page_key: false,
            target_canvas: false,
            target_instance: false,
            snapshot: Rc::new(Cell::new(None)),
            sheet: Rc::new(Cell::new(None)),
            checks: vec![],
            finished: false,
            menu: Rc::new(Cell::new(None)),
            steps: 0,
            host_started: None,
            mode_probe: None,
            closed_child: None,
            socket_close_seen: false,
            document_neutralized: false,
            closed_socket_count: None,
            quit_probe: None,
            fixture_socket: Arc::new(AtomicU8::new(0)),
        }
    }
    pub fn fail(&mut self, name: &'static str) {
        self.failures.push(name);
    }
    pub fn observe(&mut self, event: &HostEvent) {
        if let HostEvent::Message(body) = event {
            let Ok(value) = serde_json::from_str::<Value>(body) else {
                return;
            };
            self.ipc += 1;
            match value["kind"].as_str() {
                Some("fixture") => {
                    self.frames = value["frames"].as_u64().unwrap_or(0);
                    self.webgl = value["webgl2"].as_bool().unwrap_or(false);
                }
                Some("roundtrip") => self.roundtrip = true,
                Some("key") => self.page_key = true,
                Some("target") => {
                    self.target_canvas = value["canvas"].as_bool().unwrap_or(false);
                    self.target_instance = value["instance"].as_bool().unwrap_or(false);
                }
                _ => {}
            }
        }
        if matches!(event, HostEvent::ProcessFailed | HostEvent::Overflow) {
            self.fail("host-event");
        }
    }
    pub fn tick(&mut self, app: &mut Desktop, window: &mut Window, cx: &mut Context<Desktop>) {
        if self.finished {
            return;
        }
        if let Some(quit) = &mut self.quit_probe {
            if app.resources_active() {
                quit.pending_parent_preserved |=
                    app.is_quitting() && quit.parent.isVisible() && quit.child.borrow().is_some();
            }
            if quit.started.elapsed().as_secs() > 15 {
                self.checks.push((
                    "unconfirmed_close_preserves_parent_and_child",
                    app.resources_active()
                        && quit.parent.isVisible()
                        && quit.child.borrow().is_some()
                        && !self.socket_close_seen
                        && !self.document_neutralized,
                ));
                self.fail("application-quit-cleanup-timeout");
                self.finished = true;
                self.write_report();
            }
            self.sample_fixture_socket(app);
            return;
        }
        if let Some(mode_probe) = self.mode_probe.take() {
            match mode_probe {
                ModeProbe::WaitingForApproval { child } => {
                    self.checks.push((
                        "mode_waits_for_sheet_response",
                        matches!(app.snapshot().mode, e::Mode::Assisted)
                            && app.host().is_some()
                            && macos_native::has_sheet(window),
                    ));
                    let parent = macos_native::parent_window(window);
                    cx.spawn(async move |_, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(10))
                            .await;
                        if let Some(parent) = parent {
                            macos_native::sheet_key(&parent, true);
                        }
                    })
                    .detach();
                    self.mode_probe = Some(ModeProbe::WaitingForClose {
                        child,
                        until: Instant::now() + std::time::Duration::from_secs(15),
                    });
                }
                ModeProbe::WaitingForClose { child, until } => {
                    if !(app.snapshot().mode == e::Mode::Clientless
                        && matches!(app.snapshot().phase, e::Phase::Offline))
                        && Instant::now() < until
                    {
                        self.mode_probe = Some(ModeProbe::WaitingForClose { child, until });
                        return;
                    }
                    self.checks.push((
                        "mode_closed_before_switch",
                        child.borrow().is_none() && app.host().is_none(),
                    ));
                    self.checks.push((
                        "clientless_offline_without_auth",
                        app.snapshot().mode == e::Mode::Clientless
                            && matches!(app.snapshot().phase, e::Phase::Offline),
                    ));
                    self.checks
                        .push(("actual_socket_close_ack", self.socket_close_seen));
                    self.checks
                        .push(("native_document_neutralized", self.document_neutralized));
                    self.closed_child = Some(child);
                    self.finish(app, cx);
                }
            }
            return;
        }
        let host_present = self.host.is_some() || app.host().is_some();
        if !host_present {
            if self.start.elapsed().as_secs() > 120 {
                self.fail("application-host-timeout");
                self.finish(app, cx);
            }
            return;
        }
        self.host_started.get_or_insert_with(Instant::now);
        self.steps += 1;
        match self.steps {
            5 => {
                self.checks.push((
                    "native_child",
                    self.host
                        .as_ref()
                        .or_else(|| app.host())
                        .and_then(|h| {
                            h.child
                                .borrow()
                                .as_ref()
                                .map(|w| macos_native::is_child(w, window))
                        })
                        .unwrap_or(false),
                ));
                app.update_menus(cx);
                let names = cx.text_system().all_font_names();
                self.checks.push((
                    "bundled_font_families",
                    ["IBM Plex Sans", "IBM Plex Sans Thai", "IBM Plex Mono"]
                        .iter()
                        .all(|family| names.iter().any(|name| name == family)),
                ));
                if let Some(host) = self.host.as_ref().or_else(|| app.host())
                    && let Some(web) = host.child.borrow().as_ref()
                {
                    let _ = web.evaluate_script("if(window.hostRoundtrip)window.hostRoundtrip()");
                    let _ = web.focus();
                }
            }
            8 => {
                self.checks.push((
                    "game_focus",
                    self.host
                        .as_ref()
                        .or_else(|| app.host())
                        .and_then(|h| h.child.borrow().as_ref().map(macos_native::game_has_focus))
                        .unwrap_or(false),
                ));
                let parent = macos_native::parent_window(window);
                let target = self.options.target;
                let menu = self.menu.clone();
                cx.spawn(async move |_, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(10))
                        .await;
                    if let Some(parent) = parent {
                        if !target {
                            macos_native::page_key(&parent);
                        }
                        menu.set(Some(macos_native::menu_key(&parent, "2", 19, false)));
                    }
                })
                .detach();
            }
            10 => {
                self.checks.push((
                    "menu_shortcut_with_game_focus",
                    self.menu.get() == Some(true),
                ));
                self.checks
                    .push(("setup_tab", matches!(app.tab, super::app::Tab::Setup)));
            }
            12 => {
                window.resize(size(px(1252.), px(772.)));
            }
            13 => {
                app.form.radius.read(cx).focus_handle(cx).focus(window);
                macos_native::focus_dashboard(window);
                let parent = macos_native::parent_window(window);
                cx.spawn(async move |_, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(10))
                        .await;
                    if let Some(parent) = parent {
                        macos_native::page_key(&parent);
                    }
                })
                .detach();
            }
            15 => {
                self.checks.push((
                    "native_dashboard_text_entry",
                    macos_native::dashboard_has_focus(window)
                        && app.form.radius.read(cx).value() == "20x",
                ));
                app.form
                    .radius
                    .update(cx, |input, cx| input.set("20".into(), cx));
            }
            16 => {
                self.check_bounds(app, "resize_panel", 1252., 772., true);
                app.panel = false;
            }
            20 => {
                window.resize(size(px(972.), px(641.)));
            }
            24 => {
                self.check_bounds(app, "minimum_hidden", 972., 641., false);
                app.panel = true;
                window.resize(size(px(1400.), px(900.)));
            }
            26 if self.options.target => {
                if let Some(host) = self.host.as_ref().or_else(|| app.host())
                    && let Some(web) = host.child.borrow().as_ref()
                {
                    let _ = web.evaluate_script(
                        "document.documentElement.requestFullscreen().catch(()=>{})",
                    );
                }
            }
            28 => {
                if self.options.target {
                    self.checks
                        .push(("fullscreen_stays_in_application", !app.panel));
                    app.panel = true;
                }
                self.checks.push((
                    "dashboard_focus",
                    macos_native::focus_dashboard(window)
                        && macos_native::dashboard_has_focus(window),
                ));
                app.focus.focus(window);
            }
            30 => {
                self.checks.push((
                    "native_sheet_open",
                    macos_native::confirm_mode(window, true, self.sheet.clone()),
                ));
            }
            32 => {
                self.checks
                    .push(("native_sheet_attached", macos_native::has_sheet(window)));
                let parent = macos_native::parent_window(window);
                cx.spawn(async move |_, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(10))
                        .await;
                    if let Some(parent) = parent {
                        macos_native::sheet_key(&parent, false);
                    }
                })
                .detach();
            }
            36 => {
                self.checks
                    .push(("native_sheet_cancelled", self.sheet.get() == Some(false)));
                app.tab = super::app::Tab::Run;
            }
            38 => {
                use gpui::EntityInputHandler;
                let passed = app.password.update(cx, |input, cx| {
                    input.set("offline😀".into(), cx);
                    input.replace_and_mark_text_in_range(Some(0..0), "ปู่", Some(0..3), window, cx);
                    let selection = input.selected_text_range(false, window, cx);
                    let private = input.text_for_range(0..1, &mut None, window, cx).is_none();
                    let mut value = input.take(cx);
                    use zeroize::Zeroize;
                    value.zeroize();
                    private
                        && input.value().is_empty()
                        && selection.is_some_and(|s| s.range == (0..3))
                });
                self.checks.push(("native_secure_ime_and_clear", passed));
            }
            40 => {
                macos_native::test_appearance(window, Some(true));
                macos_native::badge(true);
            }
            42 => {
                self.checks.push((
                    "system_dark_appearance",
                    super::theme::Theme::current(window).base == gpui::rgb(0x131518).into(),
                ));
                self.checks
                    .push(("native_dock_badge", macos_native::has_badge()));
                macos_native::badge(false);
                macos_native::test_appearance(window, Some(false));
            }
            46 => {
                self.checks.push((
                    "native_light_appearance",
                    macos_native::appearance(window) == Some(gpui::WindowAppearance::Light),
                ));
                self.checks.push((
                    "system_light_appearance",
                    super::theme::Theme::current(window).base == gpui::rgb(0xe9ecef).into(),
                ));
                macos_native::test_appearance(window, None);
            }
            48 => {
                if let Some(port) = self.options.socket_port {
                    let submitted = app
                        .host()
                        .and_then(|host| {
                            host.child.borrow().as_ref().map(|web| {
                                web.evaluate_script(&format!(
                                "(()=>{{const p=globalThis.__otLocalVerification={{created:false,open:false,closed:false,error:false}};try{{const w=new WebSocket('ws://127.0.0.1:{port}/');p.created=true;w.addEventListener('open',()=>p.open=true);w.addEventListener('close',()=>p.closed=true);w.addEventListener('error',()=>p.error=true);}}catch(_){{p.error=true;}}}})()"
                                ))
                                .is_ok()
                            })
                        })
                        .unwrap_or(false);
                    self.checks
                        .push(("local_socket_creation_submitted", submitted));
                }
            }
            _ => {}
        }
        let seconds = if self.options.target { 45 } else { 6 };
        if self
            .host_started
            .is_some_and(|start| start.elapsed().as_secs() >= seconds)
            && self.snapshot.get().is_none()
            && let Some(host) = self.host.as_ref().or_else(|| app.host())
            && let Some(web) = host.child.borrow().as_ref()
        {
            macos_native::snapshot(
                web,
                self.options.directory.join("game.tiff"),
                self.snapshot.clone(),
            );
        }
        if self
            .host_started
            .is_some_and(|start| start.elapsed().as_secs() >= seconds + 2)
        {
            if self.options.application && self.snapshot.get() == Some(true) {
                let child = app.host().map(|h| h.child.clone());
                if let Some(child) = child {
                    if let Some(route) = self.options.quit {
                        if let Some(parent) = macos_native::parent_window(window) {
                            let dispatched = Rc::new(Cell::new(None));
                            self.quit_probe = Some(QuitProbe {
                                child,
                                parent: parent.clone(),
                                started: Instant::now(),
                                dispatched: dispatched.clone(),
                                pending_parent_preserved: false,
                            });
                            cx.spawn(async move |_, cx| {
                                cx.background_executor()
                                    .timer(std::time::Duration::from_millis(10))
                                    .await;
                                match route {
                                    QuitRoute::Native => {
                                        macos_native::terminate_application();
                                        dispatched.set(Some(true));
                                    }
                                    QuitRoute::Command => dispatched
                                        .set(Some(macos_native::menu_key(&parent, "q", 12, false))),
                                }
                            })
                            .detach();
                        } else {
                            self.fail("application-quit-missing-parent");
                            self.finish(app, cx);
                        }
                    } else {
                        app.switch_mode(e::Mode::Clientless, window, cx);
                        self.mode_probe = Some(ModeProbe::WaitingForApproval { child });
                    }
                } else {
                    self.fail("mode-switch-missing-host");
                    self.finish(app, cx);
                }
            } else {
                self.finish(app, cx);
            }
        }
    }
    fn finish(&mut self, app: &mut Desktop, cx: &mut Context<Desktop>) {
        self.finished = true;
        self.checks.push(("page_ipc", self.ipc > 0));
        if self.options.target {
            self.checks.push(("target_canvas", self.target_canvas));
            self.checks
                .push(("target_unity_instance", self.target_instance));
        } else {
            self.checks.extend([
                ("animated_webgl2", self.webgl && self.frames > 20),
                ("rust_page_roundtrip", self.roundtrip),
                ("native_page_key", self.page_key),
            ]);
        }
        self.checks
            .push(("native_snapshot", self.snapshot.get() == Some(true)));
        if self.options.application {
            let child = self
                .closed_child
                .take()
                .or_else(|| app.host().map(|h| h.child.clone()));
            app.request_quit();
            self.checks.push((
                "child_closed_before_parent",
                child.is_some_and(|child| child.borrow().is_none()),
            ));
            self.checks
                .push(("real_engine_runtime", app.controller().is_some()));
        } else if let Some(host) = self.host.take() {
            host.close();
            self.checks
                .push(("child_closed_before_parent", host.child.borrow().is_none()));
        }
        self.write_report();
        // The normal application path completes the resource fence before native termination.
        // Offline host verification owns no observer/session and closes its child above.
        if !self.options.application {
            macos_native::allow_termination();
        }
        cx.quit();
    }
    pub fn application_will_quit(&mut self, app: &Desktop, _: &mut gpui::App) {
        let Some(quit) = self.quit_probe.take() else {
            return;
        };
        self.checks.extend([
            (
                "native_quit_request_dispatched",
                quit.dispatched.get() == Some(true),
            ),
            (
                "parent_preserved_during_cleanup",
                quit.pending_parent_preserved,
            ),
            (
                "cleanup_outlived_gpui_100ms_budget",
                quit.started.elapsed() >= std::time::Duration::from_millis(100),
            ),
            ("actual_socket_close_ack", self.socket_close_seen),
            ("native_document_neutralized", self.document_neutralized),
            (
                "child_closed_before_native_will_terminate",
                quit.child.borrow().is_none() && !app.resources_active() && quit.parent.isVisible(),
            ),
            ("real_engine_runtime", app.controller().is_some()),
        ]);
        if matches!(self.options.quit, Some(QuitRoute::Native)) {
            self.checks.push((
                "appkit_termination_deferred_before_gpui_shutdown",
                macos_native::termination_deferrals() > 0,
            ));
        }
        self.checks.extend([
            ("page_ipc", self.ipc > 0),
            ("target_canvas", self.target_canvas),
            ("target_unity_instance", self.target_instance),
            ("native_snapshot", self.snapshot.get() == Some(true)),
        ]);
        self.finished = true;
        self.write_report();
    }
    fn write_report(&self) {
        let socket_fixture_passed = self.options.socket_port.is_none()
            || self.closed_socket_count.is_some_and(|count| count > 0);
        let passed = self.failures.is_empty() && self.checks.iter().all(|(_, passed)| *passed);
        let passed = passed && socket_fixture_passed;
        let socket_state = self.fixture_socket.load(Ordering::Acquire);
        let report = json!({"application_runtime":self.options.application,"verification":if self.options.target {"official-title-only"} else {"offline-fixture"},"quit_route":self.options.quit.map(|route| match route {QuitRoute::Native=>"native-terminate",QuitRoute::Command=>"command-q"}),"account_used":false,"passed":passed,"checks":self.checks,"failures":self.failures,"frames":self.frames,"ipc_messages":self.ipc,"embedded_resource_close_fence_passed":self.options.application&&self.socket_close_seen&&self.document_neutralized,"page_owned_socket_count":self.closed_socket_count,"local_socket_fixture_passed":socket_fixture_passed,"local_socket_state":{"constructed":socket_state&1!=0,"opened":socket_state&2!=0,"closed":socket_state&4!=0,"error":socket_state&8!=0},"account_session_tested":false,"gameplay_proven":false});
        let result = serde_json::to_vec_pretty(&report)
            .ok()
            .and_then(|bytes| fs::write(self.options.directory.join("report.json"), bytes).ok());
        eprintln!(
            "OpenTrickster verification {}: {}",
            if passed && result.is_some() {
                "passed"
            } else {
                "failed"
            },
            self.options.directory.display()
        );
        self.options.result.set(Some(passed && result.is_some()));
    }
    fn sample_fixture_socket(&self, app: &Desktop) {
        if self.options.socket_port.is_none() {
            return;
        }
        if let Some(host) = app.host()
            && let Some(web) = host.child.borrow().as_ref()
        {
            let state = self.fixture_socket.clone();
            let _ = web.evaluate_script_with_callback(
                "JSON.stringify(globalThis.__otLocalVerification||{})",
                move |value| {
                    if let Ok(value) = serde_json::from_str::<String>(&value)
                        && let Ok(value) = serde_json::from_str::<Value>(&value)
                    {
                        let bits = ["created", "open", "closed", "error"]
                            .iter()
                            .enumerate()
                            .fold(0, |bits, (index, name)| {
                                bits | (u8::from(value[*name] == true) << index)
                            });
                        state.store(bits, Ordering::Release);
                    }
                },
            );
        }
    }
    fn check_bounds(
        &mut self,
        app: &Desktop,
        name: &'static str,
        width: f32,
        height: f32,
        panel: bool,
    ) {
        let expected = crate::layout::game_rect(
            0.,
            55.,
            width - if panel { 344. } else { 0. },
            crate::layout::stage_height(width, height, panel),
        );
        let actual = self
            .host
            .as_ref()
            .or_else(|| app.host())
            .and_then(|h| h.child.borrow().as_ref().and_then(|web| web.bounds().ok()))
            .map(|bounds| {
                let p = bounds.position.to_logical::<f64>(1.);
                let s = bounds.size.to_logical::<f64>(1.);
                crate::layout::GameRect {
                    x: p.x as f32,
                    y: p.y as f32,
                    width: s.width as f32,
                    height: s.height as f32,
                }
            });
        self.checks.push((
            name,
            actual.zip(expected).is_some_and(|(a, e)| {
                (a.x - e.x).abs() < 1.
                    && (a.y - e.y).abs() < 1.
                    && (a.width - e.width).abs() < 1.
                    && (a.height - e.height).abs() < 1.
            }),
        ));
    }
}

pub fn offline_snapshot() -> e::Snapshot {
    e::Snapshot {
        mode: e::Mode::Assisted,
        phase: e::Phase::ClientAbsent,
        allowed: vec![e::Control::Configure],
        blocked: None,
        blockers: vec![],
        build: None,
        choices: e::Choices::None,
        setup: None,
        activity: e::ActivityView::None,
        own: e::OwnView {
            label: None,
            profile: None,
            map_label: None,
            position: None,
            vitals: None,
            drill: e::Drill::Unknown,
            last_state_at: None,
            inventory: e::InventoryKnowledge::Unknown,
        },
        entities: vec![],
        inventory: vec![],
        totals: e::Totals {
            sent: 0,
            confirmed: 0,
            kills: 0,
            digs: 0,
            finds: 0,
            empty: 0,
            goal_gained: 0,
            elapsed_ms: 0,
        },
        log: vec![],
    }
}
