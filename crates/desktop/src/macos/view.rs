mod dashboard;
mod forms;

use super::{
    app::{Desktop, LogFilter, Tab},
    host::EmbeddedHost,
    theme::Theme,
};
use crate::layout;
use gpui::{prelude::*, *};
use opentrickster_engine::{self as e, Command, Control, Mode, Phase, SafeLog};

fn label(text: impl Into<SharedString>, theme: Theme) -> Div {
    div()
        .text_size(px(12.))
        .line_height(px(18.))
        .text_color(theme.secondary)
        .child(text.into())
}
fn heading(text: impl Into<SharedString>) -> Div {
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(14.))
        .line_height(px(21.))
        .child(text.into())
}
fn card(theme: Theme) -> Div {
    div()
        .w_full()
        .p(px(16.))
        .rounded(px(10.))
        .bg(theme.raised)
        .border_1()
        .border_color(theme.line)
        .flex()
        .flex_col()
        .gap(px(12.))
}
fn button(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    enabled: bool,
    primary: bool,
    theme: Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.))
        .px(px(10.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .rounded(px(7.))
        .border_1()
        .border_color(if enabled { theme.control } else { theme.line })
        .bg(if primary && enabled {
            theme.accent
        } else {
            theme.raised
        })
        .text_color(if primary && enabled {
            theme.on_accent
        } else if enabled {
            theme.text
        } else {
            theme.muted
        })
        .when(enabled, |s| {
            let hover = if primary {
                theme.accent_hover
            } else {
                theme.hover
            };
            s.cursor_pointer().hover(move |s| s.bg(hover))
        })
        .child(text.into())
}

pub fn phase_label(phase: &Phase) -> &'static str {
    match phase {
        Phase::ClientAbsent => "Game not loaded",
        Phase::ClientLoading => "Loading game",
        Phase::AwaitingLogin => "Log in to the game",
        Phase::Offline => "Not connected",
        Phase::Connecting => "Connecting",
        Phase::Ready => "Ready",
        Phase::Starting => "Starting",
        Phase::Running => "Running",
        Phase::Pausing => "Pausing",
        Phase::Paused(_) => "Paused",
        Phase::Stopping => "Stopping",
        Phase::Reconnecting(_) => "Reconnecting",
        Phase::Disconnected => "Disconnected",
        Phase::Closing => "Closing session",
        Phase::Error(halt) => halt_label(halt),
    }
}
pub fn halt_label(halt: &e::Halt) -> &'static str {
    match halt {
        e::Halt::BuildChanged => "Game updated",
        e::Halt::ClientFailed => "Game client stopped",
        e::Halt::SessionReplaced => "Signed in elsewhere",
        e::Halt::ActionsRejected => "Actions rejected",
        e::Halt::MapForbidden => "Map blocks automation",
        e::Halt::ConnectFailed => "Sign-in failed",
    }
}
pub fn pause_label(pause: &e::Pause) -> &'static str {
    match pause {
        e::Pause::User => "By you",
        e::Pause::ManualInput => "Manual gameplay input. Check the character before resuming.",
        e::Pause::HpPotions => {
            "HP potions ran out or could not be used. Check HP and potions in the game before resuming."
        }
        e::Pause::Weight => "Free bag space in the game before resuming.",
        e::Pause::Drill => "Repair or replace the drill in the game.",
        e::Pause::Depth => "Equip a deeper drill or choose another area.",
        e::Pause::Goal => "Goal reached. Check the character before resuming.",
        e::Pause::Fainted => "Revive the character in the game.",
        e::Pause::NoDrops => "Check the loot list and map.",
        e::Pause::Uncertain => "Resume requires fresh state and verified idle gameplay.",
        e::Pause::Reconnected => "Check the character and live state before resuming.",
        e::Pause::MapChanged => {
            "The character changed maps. Check the character and the run's area before resuming."
        }
    }
}
/// Live pause text. The engine keeps an unavailable potion action as a blocker on this pause.
pub fn pause_text(pause: &e::Pause, blockers: &[e::Blocker]) -> &'static str {
    let has = |blocker| blockers.contains(&blocker);
    match pause {
        e::Pause::HpPotions if has(e::Blocker::NeedsCapability(e::Capability::HpPotion)) => {
            "HP reached the potion threshold, and this session cannot use HP potions. Recover HP in the game before resuming."
        }
        e::Pause::HpPotions if has(e::Blocker::NeedsHpPotions) => {
            "Restock HP potions in the game before resuming."
        }
        _ => pause_label(pause),
    }
}
pub fn refusal_label(refusal: &e::Refusal) -> &'static str {
    match refusal {
        e::Refusal::WrongPhase => "This action is unavailable in the current session state.",
        e::Refusal::SetupInvalid => "Choose an activity, area, targets, and valid limits in Setup.",
        e::Refusal::MissingState => {
            "Waiting for received game state. Unknown resources and inventory block automation."
        }
        e::Refusal::Unsupported => "This session cannot perform the configured activity safely.",
        e::Refusal::UnsafeArea => "Choose a valid area on the current map.",
        e::Refusal::PendingAction => "Finishing the current action before continuing.",
        e::Refusal::Busy => "The character is busy. Choose another character or wait.",
    }
}
fn log_text(event: &SafeLog) -> (&'static str, &'static str) {
    match event {
        SafeLog::Sent => ("↑", "Action sent"),
        SafeLog::Confirmed => ("✓", "Result confirmed by the game"),
        SafeLog::Rejected(_) => ("!", "The game rejected an action"),
        SafeLog::Paused(pause) => ("Ⅱ", pause_label(pause)),
        SafeLog::Halted(halt) => ("!", halt_label(halt)),
        SafeLog::Refused(refusal) => ("·", refusal_label(refusal)),
    }
}

impl Desktop {
    pub fn render_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(window);
        let width: f32 = window.viewport_size().width.into();
        let height: f32 = window.viewport_size().height.into();
        let assisted = self.snapshot().mode == Mode::Assisted;
        let mut left = div().flex_1().min_w(px(0.)).h_full().flex().flex_col();
        if assisted {
            let stage = layout::stage_height(width, height, self.panel);
            let mut game = div().h(px(stage)).w_full().flex_none().bg(rgb(0x0b0c0e));
            if let Some(host) = self.host() {
                let child = host.child.clone();
                let placement = host.placement.clone();
                let layout_epoch = host.input.borrow().layout.clone();
                let focused = self.game_keys;
                let inbox = self.inbox.clone();
                let epoch = host.epoch;
                game = game.child(
                    canvas(
                        move |bounds, _, _| {
                            if !EmbeddedHost::place(&child, &placement, &layout_epoch, bounds) {
                                super::host::enqueue(
                                    &inbox,
                                    epoch,
                                    super::host::HostEvent::PlacementFailed,
                                );
                            }
                        },
                        move |bounds, _, window, _| {
                            if focused
                                && let Some(r) = layout::game_rect(
                                    bounds.origin.x.into(),
                                    bounds.origin.y.into(),
                                    bounds.size.width.into(),
                                    bounds.size.height.into(),
                                )
                            {
                                for (x, y, w, h) in [
                                    (r.x - 2., r.y - 2., r.width + 4., 2.),
                                    (r.x - 2., r.y + r.height, r.width + 4., 2.),
                                    (r.x - 2., r.y, 2., r.height),
                                    (r.x + r.width, r.y, 2., r.height),
                                ] {
                                    window.paint_quad(fill(
                                        Bounds::new(point(px(x), px(y)), size(px(w), px(h))),
                                        theme.accent,
                                    ));
                                }
                            }
                        },
                    )
                    .size_full(),
                );
            } else {
                game = game.flex().items_center().justify_center().child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(16.))
                        .items_center()
                        .text_color(rgb(0xe3e7eb))
                        .child(heading(
                            if matches!(self.snapshot().phase, Phase::Error(_)) {
                                "The game client stopped"
                            } else {
                                "The game client is not loaded"
                            },
                        ))
                        .child(
                            div()
                                .text_size(px(12.))
                                .line_height(px(18.))
                                .text_color(rgb(0xa7adb5))
                                .child(match self.snapshot().phase {
                                    Phase::Error(e::Halt::ClientFailed) => self
                                        .client_failure
                                        .unwrap_or("The game view closed. Reload it to continue."),
                                    _ => "Sign in inside the game. The app does not read its password.",
                                }),
                        )
                        .child(
                            button(
                                "load-game",
                                if matches!(self.snapshot().phase, Phase::Error(_)) {
                                    "Reload game"
                                } else {
                                    "Load game"
                                },
                                self.allowed(Control::Open)
                                    || self.allowed(Control::Reconnect)
                                    || self.allowed(Control::DismissHalt),
                                true,
                                theme,
                            )
                            .on_click(cx.listener(|app, _, window, cx| app.load_game(window, cx))),
                        ),
                );
            }
            left = left.child(game);
            left = if self.panel {
                left.child(self.activity_strip(theme, cx))
            } else {
                left.child(self.ticker(theme))
            };
        } else {
            left = left.child(self.board(theme, cx));
            if !self.panel {
                left = left.child(self.ticker(theme));
            }
        }
        let body = div()
            .flex()
            .flex_1()
            .min_h(px(0.))
            .child(left)
            .when(self.panel, |body| body.child(self.panel_view(theme, cx)));
        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme.base)
            .text_color(theme.text)
            .font(Font {
                family: "IBM Plex Sans".into(),
                fallbacks: Some(FontFallbacks::from_fonts(vec!["IBM Plex Sans Thai".into()])),
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
                features: FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)])),
            })
            .text_size(px(13.))
            .line_height(px(20.))
            .child(self.toolbar(width, theme, cx))
            .child(self.pulse(theme))
            .child(body)
    }
    fn status(&self, theme: Theme) -> Div {
        let phase = &self.snapshot().phase;
        let (tone, subtle) = self.phase_tone(theme);
        let icon = match phase {
            Phase::Running => "▶",
            Phase::Paused(_) | Phase::Pausing => "Ⅱ",
            Phase::Error(_) | Phase::Disconnected => "!",
            _ => "○",
        };
        div()
            .h(px(34.))
            .px(px(10.))
            .rounded(px(17.))
            .bg(subtle)
            .flex()
            .items_center()
            .gap(px(8.))
            .overflow_hidden()
            .child(div().text_color(tone).child(icon))
            .child(heading(if self.probe.is_some() {
                "Offline verification"
            } else {
                phase_label(phase)
            }))
    }
    fn toolbar(&self, width: f32, theme: Theme, cx: &mut Context<Self>) -> Div {
        let mut toolbar = div()
            .h(px(layout::TOOLBAR))
            .flex_none()
            .pl(px(92.))
            .pr(px(16.))
            .flex()
            .items_center()
            .gap(px(12.))
            .bg(theme.panel);
        if self.panel {
            toolbar = toolbar.child(heading("OpenTrickster")).child(
                div()
                    .flex()
                    .gap(px(4.))
                    .child(
                        button(
                            "assisted",
                            "Client-assisted",
                            true,
                            self.snapshot().mode == Mode::Assisted,
                            theme,
                        )
                        .on_click(
                            cx.listener(|app, _, w, cx| app.switch_mode(Mode::Assisted, w, cx)),
                        ),
                    )
                    .child(
                        button(
                            "clientless",
                            "Clientless",
                            true,
                            self.snapshot().mode == Mode::Clientless,
                            theme,
                        )
                        .on_click(
                            cx.listener(|app, _, w, cx| app.switch_mode(Mode::Clientless, w, cx)),
                        ),
                    ),
            );
        } else {
            toolbar = toolbar.child(self.status(theme));
            if width >= 1232. && self.snapshot().own.vitals.is_some() {
                let vitals = self.snapshot().own.vitals.as_ref().unwrap();
                for (name, gauge) in [
                    ("HP", &vitals.hp),
                    ("MP", &vitals.mp),
                    ("Weight", &vitals.weight),
                ] {
                    toolbar = toolbar.child(div().w(px(92.)).child(self.mini_gauge(
                        name,
                        gauge.as_ref(),
                        theme,
                    )));
                }
            }
            toolbar = toolbar.child(self.run_controls(theme, cx));
        }
        toolbar
            .child(
                div()
                    .id("toolbar-drag")
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .when(self.snapshot().mode == Mode::Assisted, |bar| {
                bar.child(
                    button(
                        "focus",
                        if self.game_keys {
                            "Keys go to Game  ⌃Tab"
                        } else {
                            "Keys go to Dashboard  ⌃Tab"
                        },
                        self.host().is_some(),
                        false,
                        theme,
                    )
                    .on_click(cx.listener(|app, _, w, cx| app.toggle_focus(w, cx))),
                )
            })
            .child(
                button(
                    "panel",
                    if self.panel {
                        "Hide panel  ⌘\\"
                    } else {
                        "Show panel  ⌘\\"
                    },
                    true,
                    false,
                    theme,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.panel = !app.panel;
                    cx.notify();
                })),
            )
    }
    fn pulse(&self, theme: Theme) -> Div {
        let (tone, pattern) = match self.snapshot().phase {
            Phase::Paused(_) | Phase::Pausing => (theme.warning, 1),
            Phase::Reconnecting(_) => (theme.warning, 2),
            Phase::ClientLoading | Phase::Connecting => (theme.accent, 3),
            Phase::Running | Phase::Starting => (theme.accent, 0),
            Phase::Error(_) | Phase::Disconnected => (theme.critical, 0),
            _ => (theme.line, 0),
        };
        let progress = if super::macos_native::reduce_motion() {
            0.4
        } else {
            (self.ticks % 90) as f32 / 100.
        };
        div()
            .h(px(layout::PULSE))
            .w_full()
            .flex_none()
            .bg(theme.line)
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let width: f32 = bounds.size.width.into();
                        let draw = |x: f32, w: f32, window: &mut Window| {
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(bounds.origin.x + px(x), bounds.origin.y),
                                    size(px(w.min(width - x).max(0.)), bounds.size.height),
                                ),
                                tone,
                            ));
                        };
                        match pattern {
                            1 | 2 => {
                                let (mark, period) =
                                    if pattern == 1 { (4., 7.) } else { (12., 20.) };
                                let mut x = 0.;
                                while x < width {
                                    draw(x, mark, window);
                                    x += period;
                                }
                            }
                            3 => draw(width * progress, width * 0.15, window),
                            _ => draw(0., width, window),
                        }
                    },
                )
                .size_full(),
            )
    }
    fn run_controls(&self, theme: Theme, cx: &mut Context<Self>) -> Div {
        let pausable = self.allowed(Control::Pause);
        let resume = matches!(self.snapshot().phase, Phase::Paused(_));
        let enabled = pausable
            || self.allowed(if resume {
                Control::Resume
            } else {
                Control::Start
            });
        let stop = self.allowed(Control::Stop);
        let end = matches!(
            self.snapshot().phase,
            Phase::Reconnecting(_) | Phase::Disconnected
        );
        div()
            .flex()
            .gap(px(8.))
            .child(
                button(
                    "start-pause",
                    if pausable {
                        "Ⅱ  Pause  ⌘P"
                    } else if resume {
                        "▶  Resume  ⌘R"
                    } else {
                        "▶  Start  ⌘R"
                    },
                    enabled,
                    true,
                    theme,
                )
                .on_click(cx.listener(move |app, _, w, cx| {
                    if enabled {
                        app.command(
                            if pausable {
                                Command::Pause
                            } else if resume {
                                Command::Resume
                            } else {
                                Command::Start
                            },
                            w,
                            cx,
                        );
                    }
                })),
            )
            .child(
                button(
                    "stop",
                    if end {
                        "■  End run  ⌘."
                    } else {
                        "■  Stop  ⌘."
                    },
                    stop,
                    false,
                    theme,
                )
                .on_click(cx.listener(move |app, _, w, cx| {
                    if stop {
                        app.command(Command::Stop, w, cx);
                    }
                })),
            )
    }
    fn phase_tone(&self, theme: Theme) -> (Hsla, Hsla) {
        match self.snapshot().phase {
            Phase::Running | Phase::Starting => (theme.good, theme.good_subtle),
            Phase::Error(_) | Phase::Disconnected => (theme.critical, theme.critical_subtle),
            Phase::Paused(e::Pause::Fainted | e::Pause::Uncertain) | Phase::Reconnecting(_) => {
                (theme.serious, theme.serious_subtle)
            }
            Phase::Paused(_) | Phase::Pausing | Phase::Stopping => {
                (theme.warning, theme.warning_subtle)
            }
            _ => (theme.muted, theme.neutral_subtle),
        }
    }
    fn alert_view(&self, theme: Theme, cx: &mut Context<Self>) -> Option<Div> {
        let (title, body) = match self.snapshot().phase {
            Phase::Paused(reason) => ("Run paused", pause_text(&reason, &self.snapshot().blockers)),
            Phase::Reconnecting(_)
                if self.snapshot().connect_step == Some(e::ConnectStep::AwaitingGameLogin) =>
            {
                (
                    "Connection lost",
                    "Log in inside the embedded game and choose a character. A recovered run stays paused until you check the character.",
                )
            }
            Phase::Reconnecting(_) => (
                "Connection lost",
                "Reconnecting. A recovered run stays paused until you check the character.",
            ),
            Phase::Disconnected => (
                "Disconnected",
                "Reconnect when the connection is available. Check the character before resuming.",
            ),
            Phase::Error(e::Halt::ConnectFailed) if self.snapshot().mode == Mode::Clientless => {
                return None;
            }
            Phase::Error(e::Halt::ClientFailed) if self.snapshot().mode == Mode::Assisted => {
                return None;
            }
            Phase::Error(halt) => (
                halt_label(&halt),
                match halt {
                    e::Halt::BuildChanged => {
                        "The game updated. Reload it or reconnect before continuing."
                    }
                    e::Halt::SessionReplaced => {
                        "This account signed in elsewhere. Sign in again to continue."
                    }
                    e::Halt::ActionsRejected => {
                        "The game rejected repeated actions. Check the character and Setup."
                    }
                    e::Halt::MapForbidden => {
                        "Automation is unavailable in this map. Choose another area in the game."
                    }
                    _ => "The session stopped. Reconnect before continuing.",
                },
            ),
            _ => return None,
        };
        let (tone, subtle) = self.phase_tone(theme);
        let mut banner = div()
            .w_full()
            .p(px(12.))
            .border_l(px(3.))
            .border_color(tone)
            .rounded(px(10.))
            .bg(subtle)
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(heading(title))
            .child(label(body, theme));
        if matches!(self.snapshot().phase, Phase::Paused(_))
            && let Some(blocker) = self.snapshot().blockers.first()
        {
            banner = banner.child(label(
                format!("Resume locked · {}", blocker_text(blocker)),
                theme,
            ));
        }
        if self.allowed(Control::Reconnect) {
            banner = banner.child(
                button("alert-reconnect", "Reconnect", true, true, theme).on_click(
                    cx.listener(|app, _, window, cx| app.command(Command::Reconnect, window, cx)),
                ),
            );
        } else if self.allowed(Control::DismissHalt) {
            banner = banner.child(
                button("alert-dismiss", "Dismiss", true, true, theme).on_click(
                    cx.listener(|app, _, window, cx| app.command(Command::DismissHalt, window, cx)),
                ),
            );
        }
        Some(banner)
    }
    fn explanation(&self) -> &'static str {
        if let Some((message, _)) = self.message {
            return message;
        }
        if let Phase::Paused(pause) = &self.snapshot().phase {
            return pause_text(pause, &self.snapshot().blockers);
        }
        let now = self.controller().map(|c| c.now().0).unwrap_or(0);
        if let Some(entry) = self.snapshot().log.last()
            && now.saturating_sub(entry.at.0) <= 2200
            && let SafeLog::Refused(reason) = entry.event
        {
            return refusal_label(&reason);
        }
        match self.snapshot().phase {
            Phase::ClientAbsent => {
                return "Load the game, sign in, then choose an activity in Setup.";
            }
            Phase::AwaitingLogin => {
                return "Log in inside the embedded game and choose a character.";
            }
            Phase::Reconnecting(_)
                if self.snapshot().connect_step == Some(e::ConnectStep::AwaitingGameLogin) =>
            {
                return "Log in inside the embedded game and choose a character.";
            }
            Phase::Offline => return "Sign in to connect without the game client.",
            Phase::ClientLoading | Phase::Connecting => return "Waiting for the game session.",
            _ => {}
        }
        if let Some(blocker) = self.snapshot().blockers.first() {
            return blocker_text(blocker);
        }
        if let Some(blocked) = &self.snapshot().blocked {
            return refusal_label(blocked);
        }
        match self.snapshot().phase {
            Phase::ClientAbsent => "Load the game, sign in, then choose an activity in Setup.",
            Phase::AwaitingLogin => "Log in inside the embedded game and choose a character.",
            Phase::Offline => "Sign in to connect without the game client.",
            Phase::ClientLoading | Phase::Connecting => "Waiting for the game session.",
            Phase::Pausing | Phase::Stopping => "Finishing this action. No new action will start.",
            Phase::Reconnecting(_) => "A recovered run stays paused until you check the character.",
            Phase::Error(ref halt) => halt_label(halt),
            _ => "Only results confirmed by the game count toward this run.",
        }
    }
    fn panel_view(&self, theme: Theme, cx: &mut Context<Self>) -> Div {
        let alert = self.alert_view(theme, cx);
        let has_alert = alert.is_some();
        let tab_content = match self.tab {
            Tab::Run => self.run_tab_view(theme),
            Tab::Setup => self.setup_view(theme, cx),
            Tab::Log => self.log_view(theme, true, cx),
        };
        div()
            .w(px(layout::PANEL))
            .h_full()
            .flex_none()
            .border_l_1()
            .border_color(theme.line)
            .bg(theme.panel)
            .flex()
            .flex_col()
            .child(
                div()
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(self.status(theme))
                    .child(self.run_controls(theme, cx))
                    .when(!has_alert, |panel| {
                        panel.child(label(self.explanation(), theme))
                    })
                    .children(alert),
            )
            .child(
                div()
                    .flex()
                    .h(px(36.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(theme.line)
                    .children(
                        [
                            (Tab::Run, "Run  ⌘1"),
                            (Tab::Setup, "Setup  ⌘2"),
                            (Tab::Log, "Log  ⌘3"),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(i, (tab, name))| {
                            div()
                                .id(("tab", i))
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .border_b_2()
                                .border_color(if self.tab == tab {
                                    theme.accent
                                } else {
                                    theme.line
                                })
                                .text_color(if self.tab == tab {
                                    theme.text
                                } else {
                                    theme.secondary
                                })
                                .child(name)
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.tab = tab;
                                    cx.notify();
                                }))
                        }),
                    ),
            )
            .child(
                div()
                    .id("panel-content")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p(px(16.))
                    .child(tab_content),
            )
            .child(
                div()
                    .h(px(34.))
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(theme.line)
                    .child(label(self.live_caption(), theme)),
            )
    }
    fn live_caption(&self) -> &'static str {
        if self.probe.is_some() {
            return "Offline verification · no account";
        }
        match self.snapshot().own.last_state_at {
            Some(at)
                if self
                    .controller()
                    .is_some_and(|c| c.now().0.saturating_sub(at.0) < 15000)
                    && !matches!(
                        self.snapshot().phase,
                        Phase::Disconnected
                            | Phase::Error(_)
                            | Phase::Closing
                            | Phase::Reconnecting(_)
                    ) =>
            {
                "Received game state"
            }
            Some(_) => "Last known · no live game state",
            None => "No live state",
        }
    }
    fn ticker(&self, theme: Theme) -> Div {
        div()
            .h(px(layout::TICKER))
            .flex_none()
            .px(px(16.))
            .flex()
            .items_center()
            .bg(theme.panel)
            .text_color(theme.secondary)
            .child(self.explanation())
    }
}

fn blocker_text(blocker: &e::Blocker) -> &'static str {
    match blocker {
        e::Blocker::NeedsConnection => "Connect and enter the world before starting.",
        e::Blocker::NeedsSetup => "Choose an activity, area, targets, and limits in Setup.",
        e::Blocker::NeedsQuiescence => {
            "Waiting for the game to confirm that the character is idle."
        }
        e::Blocker::NeedsActionDrain => "Finishing the current action before continuing.",
        e::Blocker::NeedsObservation(e::StateKind::Inventory) => {
            "Inventory is unknown. Waiting for received bag data."
        }
        e::Blocker::NeedsObservation(_) => {
            "Waiting for received character resources and world state."
        }
        e::Blocker::NeedsFreshObservation(_) => {
            "State is too old. Resume requires fresh received game state."
        }
        e::Blocker::NeedsCapability(e::Capability::Quiescence) => {
            "The session cannot verify that gameplay is idle."
        }
        e::Blocker::NeedsCapability(e::Capability::DigCycle) => {
            "A complete digging cycle is not available for this session."
        }
        e::Blocker::NeedsCapability(e::Capability::HpPotion) => {
            "This session cannot use HP potions. Recover HP above the configured limit in the game."
        }
        e::Blocker::NeedsCapability(_) => {
            "A required game action is not available for this session."
        }
        e::Blocker::NeedsBoundaryMap(_) => {
            "Move to the configured map or choose the current map in Setup."
        }
        e::Blocker::NeedsInsideBoundary => "Move inside the configured area before continuing.",
        e::Blocker::NeedsHpAbove { .. } => "Recover HP above the configured limit in the game.",
        e::Blocker::NeedsWeightBelow { .. } => "Free bag space below the configured weight limit.",
        e::Blocker::NeedsHpPotions => "Restock HP potions in the game before resuming.",
        e::Blocker::NeedsLivingCharacter => "Revive the character in the game before resuming.",
        e::Blocker::NeedsEquippedDrill => "Equip a drill in the game before starting.",
        e::Blocker::NeedsDrillLife { .. } => {
            "Repair or replace the drill above the durability limit."
        }
        e::Blocker::NeedsDrillDepth => "Equip a deeper drill or choose another area.",
        e::Blocker::GoalReached { .. } => "Goal reached. Change the goal or stop the run.",
    }
}
