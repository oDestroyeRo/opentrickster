use super::*;

impl Desktop {
    pub(super) fn gauge(&self, name: &'static str, gauge: Option<&e::Gauge>, theme: Theme) -> Div {
        let value = gauge
            .map(|g| {
                g.maximum
                    .map(|max| format!("{} / {}", g.current, max))
                    .unwrap_or(format!("{} / Unknown", g.current))
            })
            .unwrap_or("Unknown".into());
        let ratio = gauge.and_then(|g| {
            g.maximum
                .map(|max| (g.current as f32 / max.get() as f32).clamp(0., 1.))
        });
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(label(name, theme))
                    .child(value),
            )
            .child(
                div()
                    .h(px(6.))
                    .w_full()
                    .rounded(px(3.))
                    .bg(theme.line)
                    .when_some(ratio, |bar, r| {
                        bar.child(
                            div()
                                .h_full()
                                .w(relative(r))
                                .rounded(px(3.))
                                .bg(theme.accent),
                        )
                    }),
            )
    }

    pub(super) fn vitals(&self, theme: Theme) -> Div {
        let v = self.snapshot().own.vitals.as_ref();
        let drill = match &self.snapshot().own.drill {
            e::Drill::Unknown => "Unknown".into(),
            e::Drill::Absent => "Not equipped".into(),
            e::Drill::Equipped { life, depth } => format!("Life {life} · Depth {depth}"),
        };
        card(theme)
            .child(heading("Vitals"))
            .child(self.gauge("HP", v.and_then(|v| v.hp.as_ref()), theme))
            .child(self.gauge("MP", v.and_then(|v| v.mp.as_ref()), theme))
            .child(self.gauge("Weight", v.and_then(|v| v.weight.as_ref()), theme))
            .child(label(format!("Drill  {drill}"), theme))
            .child(label(
                format!(
                    "HP potions  {}",
                    v.and_then(|v| v.hp_potions)
                        .map(|n| n.to_string())
                        .unwrap_or("Unknown".into())
                ),
                theme,
            ))
    }

    pub(super) fn totals(&self, theme: Theme) -> Div {
        let t = &self.snapshot().totals;
        card(theme)
            .child(heading("This run"))
            .child(label(
                format!("{} sent · {} confirmed", t.sent, t.confirmed),
                theme,
            ))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(format!("{} kills", t.kills))
                    .child(format!("{} digs", t.digs))
                    .child(format!("{} finds", t.finds)),
            )
            .child(label(
                format!("{} empty · {} items gained", t.empty, t.goal_gained),
                theme,
            ))
    }

    fn activity_card(&self, theme: Theme) -> Div {
        let activity = match &self.snapshot().activity {
            e::ActivityView::None => "No activity running".into(),
            e::ActivityView::Combat { target } => target
                .as_ref()
                .map(|t| format!("Combat · {}", t.label.as_str()))
                .unwrap_or("Combat · Waiting for a target".into()),
            e::ActivityView::Digging { point, step } => format!(
                "Digging · {}{}",
                match step {
                    e::DigStep::Pick => "Pick spot",
                    e::DigStep::Move => "Move",
                    e::DigStep::Detect => "Detect",
                    e::DigStep::Dig => "Dig",
                    e::DigStep::Collect => "Collect",
                },
                point
                    .map(|p| format!(" ({}, {})", p.x, p.y))
                    .unwrap_or_default()
            ),
        };
        card(theme).child(heading("Activity")).child(activity)
    }

    pub(super) fn run_tab_view(&self, theme: Theme) -> Div {
        let own = &self.snapshot().own;
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(self.activity_card(theme))
            .child(self.vitals(theme))
            .child(self.totals(theme))
            .child(
                card(theme)
                    .child(heading("Before you start"))
                    .child(label(
                        if own.position.is_some() {
                            "✓ Character in the world"
                        } else {
                            "○ Waiting for a character"
                        },
                        theme,
                    ))
                    .child(label(
                        if self.snapshot().build.is_some() {
                            "✓ Supported game build"
                        } else {
                            "○ Build not verified"
                        },
                        theme,
                    ))
                    .child(label(
                        if self.snapshot().setup.is_some() {
                            "✓ Setup applied"
                        } else {
                            "! Choose an activity and area in Setup"
                        },
                        theme,
                    ))
                    .children(
                        self.snapshot()
                            .blockers
                            .iter()
                            .map(|blocker| label(blocker_text(blocker), theme)),
                    )
                    .child(label(
                        match own.inventory {
                            e::InventoryKnowledge::Unknown => {
                                "○ Inventory unknown. Waiting for received bag data."
                            }
                            e::InventoryKnowledge::Observed => "✓ Inventory data received",
                        },
                        theme,
                    ))
                    .child(label(self.explanation(), theme)),
            )
    }

    pub(super) fn activity_strip(&self, theme: Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("activity-strip")
            .flex_1()
            .min_h(px(layout::STRIP_MIN))
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .gap(px(20.))
            .child(div().flex_1().min_w(px(0.)).child(self.lanes(theme)))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .child(self.log_view(theme, false, cx)),
            )
    }

    pub(super) fn lanes(&self, theme: Theme) -> Div {
        let now = self.controller().map(|c| c.now().0).unwrap_or(0);
        let mut lanes = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(heading("Last minute"));
        for sent in [true, false] {
            let mut lane = div()
                .h(px(20.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(label(if sent { "Sent" } else { "Confirmed" }, theme));
            lane = lane.child(
                div().relative().flex_1().h(px(20.)).children(
                    self.snapshot()
                        .log
                        .iter()
                        .filter(|entry| {
                            now.saturating_sub(entry.at.0) <= 60000
                                && if sent {
                                    matches!(entry.event, SafeLog::Sent)
                                } else {
                                    matches!(entry.event, SafeLog::Confirmed | SafeLog::Rejected(_))
                                }
                        })
                        .map(|entry| {
                            let x = 1. - now.saturating_sub(entry.at.0).min(60000) as f32 / 60000.;
                            div()
                                .absolute()
                                .left(relative(x.min(0.98)))
                                .text_color(if sent { theme.muted } else { theme.accent })
                                .child(if sent {
                                    "○"
                                } else if matches!(entry.event, SafeLog::Confirmed) {
                                    "●"
                                } else {
                                    "×"
                                })
                        }),
                ),
            );
            lanes = lanes.child(lane);
        }
        lanes
            .child(label(
                "1 min ago             45 s             30 s             15 s             Now",
                theme,
            ))
            .child(label(self.live_caption(), theme))
    }

    pub(super) fn log_view(&self, theme: Theme, filters: bool, cx: &mut Context<Self>) -> Div {
        let mut log = div().flex().flex_col().gap(px(4.));
        if filters {
            log = log
                .child(
                    div().flex().gap(px(4.)).children(
                        [
                            (LogFilter::All, "All"),
                            (LogFilter::Confirmed, "Confirmed"),
                            (LogFilter::Problems, "Problems"),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(i, (filter, name))| {
                            button(("filter", i), name, true, self.filter == filter, theme)
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.filter = filter;
                                    cx.notify();
                                }))
                        }),
                    ),
                )
                .child(label(
                    "Arrows mark sent actions. Checks mark confirmed results.",
                    theme,
                ));
        }
        let entries: Vec<_> = self
            .snapshot()
            .log
            .iter()
            .rev()
            .filter(|entry| {
                !filters
                    || match self.filter {
                        LogFilter::All => true,
                        LogFilter::Confirmed => matches!(entry.event, SafeLog::Confirmed),
                        LogFilter::Problems => matches!(
                            entry.event,
                            SafeLog::Rejected(_)
                                | SafeLog::Paused(_)
                                | SafeLog::Halted(_)
                                | SafeLog::Refused(_)
                        ),
                    }
            })
            .take(if filters { 256 } else { 6 })
            .collect();
        if entries.is_empty() {
            return log.child(label("Nothing here yet.", theme));
        }
        log.children(entries.into_iter().map(|entry| {
            let (icon, text) = log_text(&entry.event);
            div()
                .h(px(24.))
                .flex()
                .gap(px(8.))
                .items_center()
                .overflow_hidden()
                .text_size(px(12.))
                .line_height(px(18.))
                .child(label(
                    format!("{:02}:{:02}", entry.at.0 / 60000, entry.at.0 / 1000 % 60),
                    theme,
                ))
                .child(div().text_color(theme.accent).child(icon))
                .child(div().truncate().child(text))
        }))
    }

    pub(super) fn board(&self, theme: Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        if self.snapshot().own.position.is_none() {
            return div()
                .id("connect-board")
                .flex_1()
                .p(px(32.))
                .overflow_y_scroll()
                .child(self.connect_view(theme, cx));
        }
        let own = &self.snapshot().own;
        let name = own
            .label
            .as_ref()
            .map(|n| n.as_str().to_owned())
            .unwrap_or("Character unknown".into());
        let profile = own
            .profile
            .as_ref()
            .map(|p| {
                format!(
                    "Level {} · TM {} · {} Galder",
                    p.level, p.tm_level, p.galder
                )
            })
            .unwrap_or("Level, TM, and Galder unknown".into());
        let location = own
            .position
            .map(|(_, p)| format!("{}, {}", p.x, p.y))
            .unwrap_or("Unknown".into());
        div()
            .id("clientless-board")
            .flex_1()
            .overflow_y_scroll()
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().text_size(px(24.)).line_height(px(36.)).child(name))
                    .child(label(
                        own.map_label
                            .as_ref()
                            .map(|l| l.as_str().to_owned())
                            .unwrap_or("Map unknown".into()),
                        theme,
                    ))
                    .child(label(profile, theme))
                    .child(div().font_family("IBM Plex Mono").child(location)),
            )
            .child(self.activity_card(theme))
            .child(
                card(theme)
                    .child(heading("Session"))
                    .child(label(phase_label(&self.snapshot().phase), theme))
                    .child(label(
                        match own.inventory {
                            e::InventoryKnowledge::Unknown => "Inventory unknown",
                            e::InventoryKnowledge::Observed => "Inventory data received",
                        },
                        theme,
                    ))
                    .child(label(self.explanation(), theme)),
            )
            .when(!self.panel, |board| {
                board.child(self.vitals(theme)).child(self.totals(theme))
            })
            .child(self.lanes(theme))
            .child(self.log_view(theme, true, cx))
    }
}
