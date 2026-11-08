use super::*;

impl Desktop {
    pub(super) fn execute(&mut self, effect: Effect, window: &mut Window, _cx: &mut Context<Self>) {
        match effect {
            Effect::Open { session, mode } => {
                if self.adapter.is_some() || self.quit_requested {
                    self.observe(session, Event::Failed(engine::Halt::ClientFailed));
                    return;
                }
                let credentials = if mode == Mode::Clientless {
                    self.credentials.as_ref().cloned()
                } else {
                    None
                };
                if mode == Mode::Clientless && credentials.is_none() {
                    self.observe(session, Event::Failed(engine::Halt::ConnectFailed));
                    return;
                }
                let Some(cache) = self.cache.clone() else {
                    self.net_failure(session, NetError::Io);
                    return;
                };
                let cancel = Arc::new(AtomicBool::new(false));
                self.background
                    .prepare(session, mode, credentials, cancel.clone(), cache);
                self.adapter = Some(Adapter::Preparing { session, cancel });
            }
            Effect::Session {
                session,
                op: SessionOp::Close,
            } => self.close_adapter(session),
            Effect::Session { session, op } => {
                let now = self.controller().map(|c| c.now()).unwrap_or(engine::At(0));
                let mut receipt = None;
                let mut failure = None;
                match self
                    .adapter
                    .as_mut()
                    .filter(|adapter| adapter.session() == session)
                {
                    Some(Adapter::Native { native, .. }) => {
                        if let Err(error) = native.submit_at(now, op) {
                            failure = Some(error);
                        }
                    }
                    Some(Adapter::Embedded { observer, .. }) => match op {
                        SessionOp::Act(ticket) => {
                            let action = ticket.id;
                            // No camera mapping or gameplay input is inferred from hosting proof.
                            let _ = observer;
                            receipt = Some(Event::Receipt {
                                action,
                                delivery: engine::Delivery::NotSent,
                            });
                        }
                        SessionOp::Quiesce(_) => {}
                        _ => failure = Some(NetError::Unsupported),
                    },
                    _ => failure = Some(NetError::WrongPhase),
                }
                if let Some(event) = receipt {
                    self.observe(session, event);
                }
                if let Some(error) = failure {
                    self.net_failure(session, error);
                }
            }
            Effect::Notify { notice, .. } => {
                let enabled = match notice {
                    engine::Notice::Paused(_) => self.form.alerts[0],
                    engine::Notice::ConnectionLost => self.form.alerts[1],
                    engine::Notice::GoalReached => self.form.alerts[2],
                };
                if enabled {
                    if self.form.alerts[3] {
                        macos_native::badge(true);
                    }
                    let (title, body) = match &notice {
                        engine::Notice::Paused(reason) => {
                            ("Run paused", crate::macos::view::pause_label(reason))
                        }
                        engine::Notice::ConnectionLost => {
                            ("Connection lost", "Check the session before resuming.")
                        }
                        engine::Notice::GoalReached => {
                            ("Goal reached", "Check the character in the game.")
                        }
                    };
                    self.alerts.notify(title, body);
                }
            }
        }
        let _ = window;
    }

    pub(super) fn close_adapter(&mut self, session: SessionKey) {
        if self
            .adapter
            .as_ref()
            .is_some_and(|adapter| adapter.session() != session)
        {
            return;
        }
        match self.adapter.take() {
            Some(Adapter::Preparing { cancel, .. }) => {
                cancel.store(true, Ordering::Release);
                self.adapter = Some(Adapter::Preparing { session, cancel });
            }
            Some(Adapter::Native { native, .. }) => {
                self.background.close(session, native);
                self.adapter = Some(Adapter::Closing { session });
            }
            Some(Adapter::Embedded {
                host, mut observer, ..
            }) => {
                self.next_close += 1;
                let request = self.next_close;
                match observer.close_script(request) {
                    Ok(script) => {
                        let submitted = host
                            .child
                            .borrow()
                            .as_ref()
                            .is_some_and(|web| web.evaluate_script(script.as_str()).is_ok());
                        self.adapter = Some(Adapter::ClosingEmbedded {
                            session,
                            host,
                            observer,
                            stage: CloseStage::Sockets {
                                request,
                                started: std::time::Instant::now(),
                            },
                        });
                        if !submitted {
                            self.say("The game has not confirmed that its connections closed. Keep the app open and check the connection.");
                        }
                    }
                    Err(_) => {
                        self.adapter = Some(Adapter::ClosingEmbedded {
                            session,
                            host,
                            observer,
                            stage: CloseStage::Sockets {
                                request,
                                started: std::time::Instant::now(),
                            },
                        });
                        self.say("Game cleanup could not start. Keep the current session closed before switching modes.");
                    }
                }
            }
            Some(closing @ Adapter::ClosingEmbedded { .. }) => self.adapter = Some(closing),
            Some(closing @ Adapter::Closing { .. }) => self.adapter = Some(closing),
            None => self.observe(session, Event::Closed),
        }
    }

    pub(in crate::macos) fn request_quit(&mut self) {
        if self.quit_requested {
            return;
        }
        self.quit_requested = true;
        self.credentials.take();
        if let Some(session) = self.adapter.as_ref().map(Adapter::session) {
            self.close_adapter(session);
        }
    }

    pub(super) fn net_failure(&mut self, session: SessionKey, error: NetError) {
        if matches!(
            error,
            NetError::InvalidCredentials | NetError::Refused | NetError::SecurityRequired
        ) {
            self.credentials.take();
        }
        let halt = if error == NetError::BuildChanged {
            engine::Halt::BuildChanged
        } else {
            engine::Halt::ConnectFailed
        };
        self.observe(session, Event::Failed(halt));
        self.say(match error {
            NetError::Occupied | NetError::Busy => {
                "This character is in use. Choose another character or wait."
            }
            NetError::InvalidCredentials | NetError::Refused => {
                "Sign-in failed. Check the Game ID and password."
            }
            NetError::SecurityRequired => "Complete the account security step in the game.",
            NetError::BuildChanged => "The game updated. Update OpenTrickster before reconnecting.",
            NetError::Capacity | NetError::Uncertain => {
                "The game state could not be verified. Reconnect before continuing."
            }
            NetError::Timeout | NetError::Transport => {
                "The connection could not be completed. Try again when it is available."
            }
            _ => "This session operation could not be completed safely.",
        });
    }

    pub(super) fn poll_network(&mut self, window: &mut Window) {
        while let Some(completion) = self.background.next() {
            match completion {
                Completion::Prepared { session, result } => {
                    let active = matches!(&self.adapter,Some(Adapter::Preparing{session:key,cancel}) if *key==session && !cancel.load(Ordering::Acquire));
                    if !active {
                        if let Ok(Prepared::Native(native)) = result {
                            self.background.close(session, native);
                            if self
                                .adapter
                                .as_ref()
                                .is_some_and(|adapter| adapter.session() == session)
                            {
                                self.adapter = Some(Adapter::Closing { session });
                            }
                        } else if self
                            .adapter
                            .as_ref()
                            .is_some_and(|a| a.session() == session)
                        {
                            self.adapter.take();
                            self.observe(session, Event::Closed);
                        }
                        continue;
                    }
                    self.adapter.take();
                    match result {
                        Ok(Prepared::Native(native)) => {
                            self.adapter = Some(Adapter::Native { session, native })
                        }
                        Ok(Prepared::Embedded(profile)) => {
                            self.next_host += 1;
                            let result = (|| {
                                let identity = PageIdentity::fresh(HostEpoch(self.next_host))
                                    .map_err(|_| NetError::Io)?;
                                let binding = identity.binding()?;
                                let script = profile.observer_script(&binding)?;
                                let mut observer = GameObserver::new(session, profile);
                                observer.bind_page(binding)?;
                                let host = EmbeddedHost::open(
                                    window,
                                    self.inbox.clone(),
                                    HostSource::Official {
                                        probe: self.probe.is_some(),
                                    },
                                    identity,
                                    Some(&script),
                                )
                                .map_err(|_| NetError::Io)?;
                                Ok::<_, NetError>((host, observer))
                            })();
                            match result {
                                Ok((host, observer)) => {
                                    self.adapter = Some(Adapter::Embedded {
                                        session,
                                        host,
                                        observer,
                                    })
                                }
                                Err(error) => self.net_failure(session, error),
                            }
                        }
                        Err(error) => self.net_failure(session, error),
                    }
                }
                Completion::Closed { session, result } => {
                    if !self
                        .adapter
                        .as_ref()
                        .is_some_and(|adapter| adapter.session() == session)
                    {
                        continue;
                    }
                    match result {
                        Ok(()) => {
                            self.adapter.take();
                            self.observe(session, Event::Closed);
                        }
                        Err(error) => self.net_failure(session, error),
                    }
                }
            }
        }
        if self.background.overflowed()
            && let Some(session) = self.adapter.as_ref().map(Adapter::session)
        {
            self.close_adapter(session);
            self.net_failure(session, NetError::Capacity);
        }
        let now = self.controller().map(|c| c.now()).unwrap_or(engine::At(0));
        let mut observations = Vec::new();
        let mut error = None;
        match self.adapter.as_mut() {
            Some(Adapter::Native { session, native }) => {
                for _ in 0..256 {
                    match native.try_next_at(now) {
                        Ok(Some(observation)) => observations.push(observation),
                        Ok(None) => break,
                        Err(fault) => {
                            error = Some((*session, fault));
                            break;
                        }
                    }
                }
            }
            Some(Adapter::Embedded {
                session, observer, ..
            })
            | Some(Adapter::ClosingEmbedded {
                session, observer, ..
            }) => {
                for _ in 0..256 {
                    match observer.try_next_at(now) {
                        Ok(Some(observation)) if !matches!(observation.1.event, Event::Closed) => {
                            observations.push(observation)
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(fault) => {
                            error = Some((*session, fault));
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
        for (at, observation) in observations {
            if matches!(
                observation.event,
                Event::Failed(engine::Halt::SessionReplaced)
            ) {
                self.credentials.take();
            }
            if matches!(observation.event, Event::Closed)
                && matches!(&self.adapter,Some(Adapter::Native{session,..}) if *session==observation.session)
            {
                self.adapter.take();
            }
            if let Some(controller) = self.controller_mut() {
                controller.observe_at(at, observation);
            }
        }
        if let Some((session, error)) = error {
            self.net_failure(session, error);
        }
    }
    pub(super) fn ingest_page(
        &mut self,
        origin: &str,
        json: &str,
        captured_at: std::time::Instant,
    ) {
        let notice = match self.adapter.as_mut() {
            Some(Adapter::Embedded { observer, .. })
            | Some(Adapter::ClosingEmbedded { observer, .. }) => {
                observer.ingest_page(origin, json, captured_at)
            }
            _ => return,
        };
        match notice {
            Ok(Some(opentrickster_net::PageNotice::Closed { request, sockets })) => {
                if let Some(Adapter::ClosingEmbedded {
                    host,
                    stage:
                        CloseStage::Sockets {
                            request: expected, ..
                        },
                    ..
                }) = &self.adapter
                {
                    if *expected != request {
                        return;
                    }
                    if let Some(probe) = &mut self.probe {
                        probe.socket_close_seen = true;
                        probe.closed_socket_count = Some(sockets);
                    }
                    let neutralizing = host.neutralize();
                    if neutralizing {
                        if let Some(Adapter::ClosingEmbedded { stage, .. }) = &mut self.adapter {
                            *stage = CloseStage::Neutralizing;
                        }
                    } else {
                        self.say("The game connections closed, but the game view could not finish closing.");
                    }
                }
            }
            Ok(Some(opentrickster_net::PageNotice::Fault(fault))) => {
                let error = if fault == opentrickster_net::PageFault::BuildChanged {
                    NetError::BuildChanged
                } else {
                    NetError::Uncertain
                };
                if let Some(session) = self.adapter.as_ref().map(Adapter::session) {
                    if matches!(self.adapter, Some(Adapter::ClosingEmbedded { .. })) {
                        self.say("The game has not confirmed complete cleanup. Keep the app open and check the connection.");
                    } else {
                        self.net_failure(session, error);
                    }
                }
            }
            Err(_) => {
                if let Some(session) = self.adapter.as_ref().map(Adapter::session) {
                    if matches!(self.adapter, Some(Adapter::ClosingEmbedded { .. })) {
                        self.say("The game has not confirmed complete cleanup. Keep the app open and check the connection.");
                    } else {
                        self.net_failure(session, NetError::Uncertain);
                    }
                }
            }
            _ => {}
        }
    }
    pub(super) fn finish_embedded_close(&mut self) {
        if !matches!(
            self.adapter,
            Some(Adapter::ClosingEmbedded {
                stage: CloseStage::Neutralizing,
                ..
            })
        ) {
            return;
        }
        let Some(Adapter::ClosingEmbedded {
            session,
            host,
            mut observer,
            ..
        }) = self.adapter.take()
        else {
            return;
        };
        let _ = observer.close();
        host.close();
        if let Some(probe) = &mut self.probe {
            probe.document_neutralized = true;
        }
        self.observe(session, Event::Closed);
    }
}
