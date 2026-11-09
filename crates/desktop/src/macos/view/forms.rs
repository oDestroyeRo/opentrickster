use super::*;

impl Desktop {
    pub(super) fn connect_view(&self, theme: Theme, cx: &mut Context<Self>) -> Div {
        let mut form = card(theme)
            .max_w(px(520.))
            .child(
                div()
                    .text_size(px(24.))
                    .line_height(px(36.))
                    .child("Connect without the game client"),
            )
            .child(label(
                "Choose an existing character. Only one session runs at a time.",
                theme,
            ));
        match &self.snapshot().choices {
            e::Choices::Worlds(worlds) => {
                form =
                    form.child(heading("Choose a world"))
                        .children(worlds.iter().enumerate().map(|(i, world)| {
                            let key = world.key;
                            button(
                                ("world", i),
                                world.label.as_str().to_owned(),
                                self.allowed(Control::Choose),
                                false,
                                theme,
                            )
                            .on_click(cx.listener(
                                move |app, _, w, cx| app.command(Command::ChooseWorld(key), w, cx),
                            ))
                        }));
            }
            e::Choices::Characters(characters) => {
                form = form.child(heading("Choose a character")).children(
                    characters.iter().enumerate().map(|(i, character)| {
                        let key = character.key;
                        let busy = character.busy;
                        button(
                            ("character", i),
                            format!(
                                "{}{}",
                                character.label.as_str(),
                                if busy { " · In use" } else { "" }
                            ),
                            !busy && self.allowed(Control::Choose),
                            false,
                            theme,
                        )
                        .on_click(cx.listener(move |app, _, w, cx| {
                            if !busy {
                                app.command(Command::ChooseCharacter(key), w, cx);
                            }
                        }))
                    }),
                );
            }
            e::Choices::None if self.allowed(Control::CancelConnect) => {
                form = form.child(heading("Connecting")).child(label(
                    "Waiting for the next sign-in step from the game.",
                    theme,
                ));
            }
            e::Choices::None => {
                form = form
                    .child(label("Game ID", theme))
                    .child(self.account.clone())
                    .child(label("Password", theme))
                    .child(self.password.clone())
                    .child(label("The password is used for this session only.", theme))
                    .child(
                        button("sign-in", "Sign in", self.can_authenticate(), true, theme)
                            .on_click(cx.listener(|app, _, w, cx| app.sign_in(w, cx))),
                    );
            }
        }
        if self.allowed(Control::CancelConnect) {
            form = form.child(
                button("cancel-connect", "Cancel", true, false, theme).on_click(
                    cx.listener(|app, _, w, cx| app.command(Command::CancelConnect, w, cx)),
                ),
            );
        }
        form.child(label(self.explanation(), theme))
    }

    pub(super) fn setup_view(&self, theme: Theme, cx: &mut Context<Self>) -> Div {
        let enabled = self.allowed(Control::Configure);
        let activity_locked = !self.allowed(Control::SwitchMode);
        for field in [
            &self.form.radius,
            &self.form.hp,
            &self.form.weight,
            &self.form.drill,
            &self.form.goal_count,
        ] {
            field.update(cx, |input, _| input.enable(enabled));
        }
        let mut form = div().flex().flex_col().gap(px(16.));
        if !enabled {
            form = form.child(label(
                if self.snapshot().own.position.is_none() {
                    "Connect a character before applying Setup."
                } else {
                    "Pause the run to change setup. Changes apply when you resume."
                },
                theme,
            ));
        }
        form = form
            .child(heading("Activity"))
            .child(
                div().flex().gap(px(8.)).children(
                    [(false, "Combat"), (true, "Digging")]
                        .into_iter()
                        .enumerate()
                        .map(|(i, (digging, name))| {
                            button(
                                ("activity", i),
                                name,
                                !activity_locked,
                                self.form.digging == digging,
                                theme,
                            )
                            .on_click(cx.listener(
                                move |app, _, _, cx| {
                                    if !activity_locked {
                                        app.form.digging = digging;
                                        cx.notify();
                                    }
                                },
                            ))
                        }),
                ),
            )
            .child(heading("Where"))
            .child(label(
                self.snapshot()
                    .own
                    .map_label
                    .as_ref()
                    .map(|s| s.as_str().to_owned())
                    .unwrap_or("Map unknown".into()),
                theme,
            ))
            .child(
                button(
                    "use-location",
                    "Use current location",
                    enabled && self.snapshot().own.position.is_some(),
                    false,
                    theme,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    if enabled {
                        app.form.center = app.snapshot().own.position;
                        cx.notify();
                    }
                })),
            )
            .child(label(
                self.form
                    .center
                    .map(|(_, p)| format!("Center {}, {}", p.x, p.y))
                    .unwrap_or("Choose a location in the game first.".into()),
                theme,
            ))
            .child(label("Movement radius · game-coordinate counts", theme))
            .child(self.form.radius.clone());
        if self.form.digging {
            form = form
                .child(heading("Drill"))
                .child(label("Pause below this durability", theme))
                .child(self.form.drill.clone());
        } else {
            form = form
                .child(heading("Targets"))
                .child(
                    div().flex().gap(px(4.)).children(
                        [(false, "Only these"), (true, "All except")]
                            .into_iter()
                            .enumerate()
                            .map(|(i, (except, name))| {
                                button(
                                    ("target-rule", i),
                                    name,
                                    enabled,
                                    self.form.except == except,
                                    theme,
                                )
                                .on_click(cx.listener(
                                    move |app, _, _, cx| {
                                        if enabled {
                                            app.form.except = except;
                                            cx.notify();
                                        }
                                    },
                                ))
                            }),
                    ),
                )
                .child(label(
                    "Monsters engaged by another player are skipped.",
                    theme,
                ));
            let mut seen = std::collections::BTreeSet::new();
            let entities: Vec<_> = self
                .snapshot()
                .entities
                .iter()
                .filter(|entity| seen.insert(entity.species))
                .collect();
            if entities.is_empty() {
                form = form.child(label("No received monsters yet.", theme));
            }
            form = form.children(entities.into_iter().enumerate().map(|(i, entity)| {
                let key = entity.species;
                button(
                    ("target", i),
                    entity.label.as_str().to_owned(),
                    enabled,
                    self.form.species.contains(&key),
                    theme,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    if enabled {
                        if !app.form.species.remove(&key) {
                            app.form.species.insert(key);
                        }
                        cx.notify();
                    }
                }))
            }));
        }
        // The engine applies one HP threshold to both activities.
        form = form
            .child(heading("Recovery"))
            .child(label("Use HP potion below percent", theme))
            .child(self.form.hp.clone());
        if !self.form.digging {
            form = form.child(label(
                "The run pauses when HP potions run out. This is always on.",
                theme,
            ));
        }
        if self.snapshot().mode == Mode::Assisted {
            form = form.child(label(
                "HP potion use is unavailable in client-assisted mode. The run pauses at this HP percent instead.",
                theme,
            ));
        }
        form = form
            .child(heading("Limits"))
            .child(label("Pause at bag weight percent", theme))
            .child(self.form.weight.clone())
            .child(self.goal_choices(theme, cx))
            .child(
                button("apply-setup", "Apply setup", enabled, true, theme).on_click(cx.listener(
                    move |app, _, w, cx| {
                        if enabled {
                            app.apply_setup(w, cx);
                        }
                    },
                )),
            )
            .child(heading("Alerts"));
        for (i, text) in [
            "Pause with a reason",
            "Connection lost",
            "Goal reached",
            "Dock badge",
        ]
        .into_iter()
        .enumerate()
        {
            form = form.child(
                button(
                    ("alert", i),
                    format!("{}  {text}", if self.form.alerts[i] { "☑" } else { "□" }),
                    true,
                    false,
                    theme,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.form.alerts[i] = !app.form.alerts[i];
                    cx.notify();
                })),
            );
        }
        form.child(label(self.alerts.caption(), theme))
    }
    fn goal_choices(&self, theme: Theme, cx: &mut Context<Self>) -> Div {
        let enabled = self.allowed(Control::Configure);
        let mut view = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(heading("Item goal"))
            .child(
                button(
                    "no-goal",
                    "No item goal",
                    enabled,
                    self.form.goal_kind.is_none(),
                    theme,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    if enabled {
                        app.form.goal_kind = None;
                        cx.notify();
                    }
                })),
            );
        if self.snapshot().own.inventory == e::InventoryKnowledge::Unknown {
            return view.child(label(
                "Inventory unknown. Waiting for received bag data.",
                theme,
            ));
        }
        let mut kinds = std::collections::BTreeMap::<e::ItemKind, u64>::new();
        for stack in &self.snapshot().inventory {
            *kinds.entry(stack.kind).or_default() += u64::from(stack.quantity);
        }
        if kinds.is_empty() {
            return view.child(label("The received inventory has no item choices.", theme));
        }
        view = view.child(label(
            "Item labels are unknown. Choices come from received bag data.",
            theme,
        ));
        for (i, (kind, count)) in kinds.into_iter().enumerate() {
            view = view.child(
                button(
                    ("goal-item", i),
                    format!("Label unknown · choice {} · {} in bag", i + 1, count),
                    enabled,
                    self.form.goal_kind == Some(kind),
                    theme,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    if enabled {
                        app.form.goal_kind = Some(kind);
                        cx.notify();
                    }
                })),
            );
        }
        view.child(label("Pause after this many confirmed items gained", theme))
            .child(self.form.goal_count.clone())
    }
}
