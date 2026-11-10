use opentrickster_engine::*;
use std::num::{NonZeroU16, NonZeroU32};

fn label() -> DisplayText {
    DisplayText::checked("Synthetic fixture".into()).unwrap_or_else(|_| panic!("fixture label"))
}

#[test]
fn queued_observation_keeps_its_original_freshness() {
    let mut h = Harness::new(false);
    h.wake(20_000);
    let original_at = At(1_000);
    let delayed = h.engine.handle(Input::Observed {
        at: original_at,
        observation: Observation {
            session: h.session,
            event: Event::Facts {
                revision: EvidenceKey(900),
                ingress: IngressMark(900),
                facts: vec![Fact::Hp(gauge(100, 100))],
            },
        },
    });

    assert_eq!(delayed.snapshot.own.last_state_at, Some(original_at));
    assert!(!delayed.snapshot.allowed.contains(&Control::Start));
    assert!(
        delayed
            .snapshot
            .blockers
            .contains(&Blocker::NeedsFreshObservation(StateKind::WorldState,))
    );
    assert!(acts(&delayed).is_empty());
}
fn gauge(current: u32, maximum: u32) -> Gauge {
    Gauge {
        current,
        maximum: NonZeroU32::new(maximum),
    }
}
fn center() -> Point {
    Point { x: 100, y: 100 }
}
fn entity(key: u64, point: Point, engaged: Option<bool>) -> Fact {
    Fact::Entity(Entity {
        key: EntityKey(key),
        species: SpeciesKey(1),
        label: label(),
        position: point,
        alive: true,
        hp: Some(gauge(10, 10)),
        engaged_by_other: engaged,
    })
}
fn support() -> Support {
    Support {
        build: label(),
        move_to: true,
        basic_attack: true,
        dig_cycle: true,
        pickup: true,
        hp_potion: false,
        quiesce: true,
    }
}
fn setup(dig: bool, goal: Option<Goal>) -> Setup {
    Setup::checked(SetupDraft {
        activity: if dig {
            Activity::Digging { min_drill_life: 1 }
        } else {
            Activity::Combat {
                targets: Targets::Only(vec![SpeciesKey(1)]),
            }
        },
        boundary: Boundary::checked(MapKey(1), center(), NonZeroU16::new(10).unwrap())
            .unwrap_or_else(|_| panic!("area")),
        hp_at: Percent::checked(30).unwrap_or_else(|_| panic!("hp")),
        weight_limit: Percent::checked(90).unwrap_or_else(|_| panic!("weight")),
        goal,
    })
    .unwrap_or_else(|_| panic!("setup"))
}
fn baseline() -> Vec<Fact> {
    vec![
        Fact::Position {
            map: MapKey(1),
            point: center(),
        },
        Fact::Hp(gauge(100, 100)),
        Fact::Weight(gauge(10, 100)),
        Fact::HpPotions(5),
        Fact::Inventory(Inventory::Reset(vec![])),
        Fact::Drill(Drill::Equipped {
            life: 100,
            depth: 10,
        }),
        Fact::InventorySettled(IngressMark(0)),
    ]
}
fn acts(effects: &Effects) -> Vec<&ActionTicket> {
    effects
        .io
        .iter()
        .filter_map(|effect| match effect {
            Effect::Session {
                op: SessionOp::Act(ticket),
                ..
            } => Some(ticket),
            _ => None,
        })
        .collect()
}
fn barrier(effects: &Effects) -> Option<BarrierKey> {
    effects.io.iter().find_map(|effect| match effect {
        Effect::Session {
            op: SessionOp::Quiesce(key),
            ..
        } => Some(*key),
        _ => None,
    })
}
fn closes(effects: &Effects) -> usize {
    effects
        .io
        .iter()
        .filter(|effect| {
            matches!(
                effect,
                Effect::Session {
                    op: SessionOp::Close,
                    ..
                }
            )
        })
        .count()
}
struct Harness {
    engine: Engine,
    session: SessionKey,
    request: u64,
    now: u64,
    revision: u64,
    ingress: u64,
}
impl Harness {
    fn new(dig: bool) -> Self {
        Self::with(dig, support(), baseline(), None)
    }
    fn with(dig: bool, support: Support, facts: Vec<Fact>, goal: Option<Goal>) -> Self {
        let mut engine = Engine::new(Mode::Clientless);
        let effects = engine.handle(Input::User {
            at: At(0),
            request: RequestKey(1),
            command: Command::Open,
        });
        let session = effects
            .io
            .iter()
            .find_map(|effect| match effect {
                Effect::Open { session, .. } => Some(*session),
                _ => None,
            })
            .unwrap();
        let mut h = Self {
            engine,
            session,
            request: 1,
            now: 0,
            revision: 1,
            ingress: 10,
        };
        h.command(Command::Configure(setup(dig, goal)));
        let effects = h.event(Event::Baseline {
            support,
            revision: EvidenceKey(1),
            facts,
            resumed: false,
        });
        if let Some(key) = barrier(&effects) {
            h.event(Event::Quiescent(key));
        }
        h
    }
    fn command(&mut self, command: Command) -> Effects {
        self.request += 1;
        self.now += 1;
        self.engine.handle(Input::User {
            at: At(self.now),
            request: RequestKey(self.request),
            command,
        })
    }
    fn event(&mut self, event: Event) -> Effects {
        self.now += 1;
        self.engine.handle(Input::Observed {
            at: At(self.now),
            observation: Observation {
                session: self.session,
                event,
            },
        })
    }
    fn facts(&mut self, mut facts: Vec<Fact>) -> Effects {
        self.revision += 1;
        self.ingress += 1;
        facts.push(Fact::InventorySettled(IngressMark(self.ingress)));
        self.event(Event::Facts {
            revision: EvidenceKey(self.revision),
            ingress: IngressMark(self.ingress),
            facts,
        })
    }
    /// An adapter settlement for a frame that carried no state, `delta` ms after the last input.
    fn live(&mut self, delta: u64) -> Effects {
        self.now += delta;
        self.revision += 1;
        self.ingress += 1;
        self.event(Event::Facts {
            revision: EvidenceKey(self.revision),
            ingress: IngressMark(self.ingress),
            facts: vec![
                Fact::InventorySettled(IngressMark(self.ingress)),
                Fact::StateCurrent,
            ],
        })
    }
    fn sent(&mut self, id: ActionKey) -> Effects {
        self.ingress += 1;
        self.event(Event::Receipt {
            action: id,
            delivery: Delivery::Sent(IngressMark(self.ingress)),
        })
    }
    fn idle(&mut self, effects: &Effects) -> Effects {
        self.event(Event::Quiescent(
            barrier(effects).expect("quiescence required"),
        ))
    }
    fn start_attack(&mut self) -> ActionKey {
        self.facts(vec![entity(1, center(), Some(false))]);
        let effects = self.command(Command::Start);
        let ticket = acts(&effects)[0];
        assert_eq!(ticket.action, Action::BasicAttack(EntityKey(1)));
        ticket.id
    }
    fn wake(&mut self, delta: u64) -> Effects {
        self.now += delta;
        self.engine.handle(Input::Wake(At(self.now)))
    }
}

#[test]
fn combat_attacks_counts_once_and_loots_with_separate_inventory() {
    let mut h = Harness::new(false);
    let target = Point { x: 103, y: 100 };
    h.facts(vec![entity(1, target, Some(false))]);
    let effects = h.command(Command::Start);
    let attack = acts(&effects)[0];
    // The game approaches a clicked monster itself; the run does not walk onto it first.
    assert_eq!(attack.action, Action::BasicAttack(EntityKey(1)));
    h.sent(attack.id);
    let kill = h.facts(vec![
        Fact::Position {
            map: MapKey(1),
            point: target,
        },
        Fact::Outcome(Outcome::OwnKill(EntityKey(1))),
    ]);
    assert_eq!(kill.snapshot.totals.kills, 1);
    let duplicate = h.facts(vec![Fact::Outcome(Outcome::OwnKill(EntityKey(1)))]);
    assert_eq!(duplicate.snapshot.totals.kills, 1);
    h.facts(vec![Fact::Drop(Drop {
        key: DropKey(1),
        kind: ItemKind(3),
        position: target,
        quantity: 2,
    })]);
    let next = h.idle(&kill);
    let id = acts(&next)[0].id;
    assert_eq!(acts(&next)[0].action, Action::Pickup(DropKey(1)));
    h.sent(id);
    let pickup = h.facts(vec![
        Fact::Outcome(Outcome::OwnPickup(DropKey(1))),
        Fact::DropGone(DropKey(1)),
        Fact::Inventory(Inventory::Put(Stack {
            key: StackKey(1),
            kind: ItemKind(3),
            quantity: 2,
        })),
    ]);
    assert_eq!(pickup.snapshot.totals.confirmed, 2);
    h.idle(&pickup);
    let stopped = h.command(Command::Stop);
    assert_eq!(stopped.snapshot.phase, Phase::Ready);
    assert_eq!(stopped.snapshot.totals.kills, 1);
    assert_eq!(stopped.snapshot.totals.sent, 2);
}

#[test]
fn an_adjusted_move_confirms_at_the_server_end_and_digs_there() {
    let mut h = Harness::with(true, support(), baseline(), None);
    let start = h.command(Command::Start);
    let id = acts(&start)[0].id;
    assert_eq!(acts(&start)[0].action, Action::DigAt(center()));
    h.sent(id);
    h.facts(vec![Fact::DigAccepted, Fact::DigSpot(center())]);
    let first = h.facts(vec![Fact::Outcome(Outcome::DigEnded(DigEnd::Empty))]);
    let movement = h.idle(&first);
    let ticket = acts(&movement)[0];
    let Action::MoveTo(target) = ticket.action else {
        panic!("move to the next bounded point");
    };
    h.sent(ticket.id);
    // The server's echo ends the walk 4 px short of the target it was asked for.
    let end = Point {
        x: target.x - 4,
        y: target.y,
    };
    let echoed = h.facts(vec![Fact::Position {
        map: MapKey(1),
        point: end,
    }]);
    assert!(acts(&echoed).is_empty());
    assert_eq!(echoed.snapshot.totals.confirmed, 1);
    let arrival = h.facts(vec![Fact::Outcome(Outcome::Arrived { target, at: end })]);
    assert_eq!(arrival.snapshot.totals.confirmed, 2);
    let dig = h.idle(&arrival);
    assert_eq!(
        acts(&dig)[0].action,
        Action::DigAt(end),
        "dig where the server ended the walk instead of walking to the same target again"
    );
}

#[test]
fn repeated_dig_cycles_move_and_goal_uses_inventory_not_caption() {
    let mut h = Harness::with(
        true,
        support(),
        baseline(),
        Some(Goal {
            item: ItemKind(7),
            gained: 2,
        }),
    );
    let start = h.command(Command::Start);
    let id = acts(&start)[0].id;
    assert_eq!(acts(&start)[0].action, Action::DigAt(center()));
    h.sent(id);
    let progress = h.facts(vec![Fact::DigAccepted, Fact::DigSpot(center())]);
    assert_eq!(progress.snapshot.totals.confirmed, 0);
    let first = h.facts(vec![Fact::Outcome(Outcome::DigEnded(
        DigEnd::ItemReported(ItemKind(7)),
    ))]);
    assert_eq!(first.snapshot.totals.digs, 1);
    assert_eq!(first.snapshot.totals.goal_gained, 0);
    let movement = h.idle(&first);
    let ticket = acts(&movement)[0];
    let id = ticket.id;
    let Action::MoveTo(point) = ticket.action else {
        panic!("move to next bounded point");
    };
    assert_eq!(point, Point { x: 110, y: 100 });
    h.sent(id);
    let arrival = h.facts(vec![
        Fact::Position {
            map: MapKey(1),
            point,
        },
        Fact::Outcome(Outcome::Arrived {
            target: point,
            at: point,
        }),
    ]);
    let dig = h.idle(&arrival);
    let id = acts(&dig)[0].id;
    assert_eq!(acts(&dig)[0].action, Action::DigAt(point));
    h.sent(id);
    let result = h.facts(vec![
        Fact::Outcome(Outcome::DigEnded(DigEnd::Empty)),
        Fact::Inventory(Inventory::Put(Stack {
            key: StackKey(1),
            kind: ItemKind(7),
            quantity: 2,
        })),
    ]);
    assert_eq!(result.snapshot.totals.digs, 2);
    assert_eq!(result.snapshot.totals.empty, 1);
    assert_eq!(result.snapshot.totals.goal_gained, 2);
    assert!(acts(&result).is_empty());
    let paused = h.idle(&result);
    assert_eq!(paused.snapshot.phase, Phase::Paused(Pause::Goal));
    assert!(!paused.snapshot.allowed.contains(&Control::Resume));
}

#[test]
fn pending_action_blocks_commands_receipts_and_ticks_from_emitting_another() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    for _ in 0..5 {
        assert!(acts(&h.command(Command::Start)).is_empty());
        assert!(
            acts(&h.event(Event::Receipt {
                action: id,
                delivery: Delivery::Submitted
            }))
            .is_empty()
        );
        assert!(acts(&h.wake(1)).is_empty());
    }
    assert_eq!(h.engine.snapshot().totals.sent, 0);
}

#[test]
fn early_result_waits_for_sent_and_checks_original_ingress() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    let early = h.facts(vec![Fact::Outcome(Outcome::OwnKill(EntityKey(1)))]);
    assert_eq!(early.snapshot.totals.kills, 0);
    let result = h.event(Event::Receipt {
        action: id,
        delivery: Delivery::Sent(IngressMark(10)),
    });
    assert_eq!(result.snapshot.totals.kills, 1);
    assert!(barrier(&result).is_some());
    let duplicate = h.event(Event::Receipt {
        action: id,
        delivery: Delivery::Sent(IngressMark(10)),
    });
    assert_eq!(duplicate.snapshot.totals.sent, 1);
    assert_eq!(duplicate.snapshot.totals.confirmed, 1);
}

#[test]
fn pre_send_aux_result_cannot_confirm_or_increment_digs() {
    let mut h = Harness::new(true);
    let start = h.command(Command::Start);
    let id = acts(&start)[0].id;
    h.event(Event::Facts {
        revision: EvidenceKey(4),
        ingress: IngressMark(8),
        facts: vec![Fact::Outcome(Outcome::DigEnded(DigEnd::Empty))],
    });
    let sent = h.event(Event::Receipt {
        action: id,
        delivery: Delivery::Sent(IngressMark(10)),
    });
    assert_eq!(sent.snapshot.totals.digs, 0);
    assert_eq!(sent.snapshot.totals.confirmed, 0);
    let result = h.event(Event::Facts {
        revision: EvidenceKey(3),
        ingress: IngressMark(11),
        facts: vec![Fact::Outcome(Outcome::DigEnded(DigEnd::Empty))],
    });
    assert_eq!(result.snapshot.totals.digs, 1);
    let duplicate = h.event(Event::Facts {
        revision: EvidenceKey(3),
        ingress: IngressMark(11),
        facts: vec![Fact::Outcome(Outcome::DigEnded(DigEnd::Empty))],
    });
    assert_eq!(duplicate.snapshot.totals.digs, 1);
}

#[test]
fn out_of_order_aux_evidence_is_not_discarded_and_state_cannot_roll_back() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.sent(id);
    h.event(Event::Facts {
        revision: EvidenceKey(100),
        ingress: IngressMark(100),
        facts: vec![Fact::Hp(gauge(90, 100))],
    });
    let result = h.event(Event::Facts {
        revision: EvidenceKey(4),
        ingress: IngressMark(50),
        facts: vec![
            Fact::Hp(gauge(1, 100)),
            Fact::Outcome(Outcome::OwnHit(EntityKey(1))),
        ],
    });
    assert_eq!(result.snapshot.totals.confirmed, 1);
    assert_eq!(result.snapshot.own.vitals.unwrap().hp.unwrap().current, 90);
}

#[test]
fn kill_counts_per_incarnation_not_repeated_outcome() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.sent(id);
    let kill = h.facts(vec![Fact::Outcome(Outcome::OwnKill(EntityKey(1)))]);
    h.idle(&kill);
    let respawn = h.facts(vec![entity(1, center(), Some(false))]);
    let id = acts(&respawn)[0].id;
    h.sent(id);
    let second = h.facts(vec![Fact::Outcome(Outcome::OwnKill(EntityKey(1)))]);
    assert_eq!(second.snapshot.totals.kills, 2);
}

#[test]
fn three_correlated_rejections_halt_and_only_confirmation_resets_streak() {
    let mut h = Harness::new(false);
    let mut id = h.start_attack();
    for index in 0..3 {
        h.sent(id);
        let irrelevant = h.facts(vec![Fact::Outcome(Outcome::Rejected {
            subject: Subject::Target(EntityKey(9)),
            reason: Rejection::Other,
        })]);
        assert!(barrier(&irrelevant).is_none());
        let result = h.facts(vec![Fact::Outcome(Outcome::Rejected {
            subject: Subject::Target(EntityKey(1)),
            reason: Rejection::InvalidTarget,
        })]);
        if index == 2 {
            assert_eq!(closes(&result), 1);
            let closed = h.event(Event::Closed);
            assert_eq!(closed.snapshot.phase, Phase::Error(Halt::ActionsRejected));
        } else {
            let next = h.idle(&result);
            id = acts(&next)[0].id;
        }
    }
}

#[test]
fn confirmed_action_resets_rejections() {
    let mut h = Harness::new(false);
    let mut id = h.start_attack();
    for index in 0..5 {
        h.sent(id);
        let outcome = if index == 2 {
            Outcome::OwnHit(EntityKey(1))
        } else {
            Outcome::Rejected {
                subject: Subject::Target(EntityKey(1)),
                reason: Rejection::Other,
            }
        };
        let result = h.facts(vec![Fact::Outcome(outcome)]);
        assert_eq!(closes(&result), 0);
        let next = h.idle(&result);
        id = acts(&next)[0].id;
    }
}

#[test]
fn pause_and_stop_are_idempotent_and_require_current_barrier() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.sent(id);
    let pause = h.command(Command::Pause);
    assert_eq!(pause.snapshot.phase, Phase::Pausing);
    assert!(h.command(Command::Pause).io.is_empty());
    let stop = h.command(Command::Stop);
    assert_eq!(stop.snapshot.phase, Phase::Stopping);
    assert!(h.command(Command::Stop).io.is_empty());
    let stopped = h.idle(&pause);
    assert_eq!(stopped.snapshot.phase, Phase::Ready);
    assert!(acts(&stopped).is_empty());
    let next = h.command(Command::Start);
    assert_eq!(acts(&next).len(), 1);
    let old = h.event(Event::Quiescent(barrier(&pause).unwrap()));
    assert!(acts(&old).is_empty());
    assert_eq!(old.snapshot.phase, Phase::Running);
}

#[test]
fn drain_timeout_closes_instead_of_claiming_idle() {
    let mut h = Harness::new(true);
    h.command(Command::Start);
    let pause = h.command(Command::Pause);
    assert!(barrier(&pause).is_some());
    let timeout = h.wake(2_000);
    assert_eq!(closes(&timeout), 1);
    assert_eq!(timeout.snapshot.phase, Phase::Closing);
    assert!(!timeout.snapshot.allowed.contains(&Control::Resume));
    let old = h.idle(&pause);
    assert_eq!(old.snapshot.phase, Phase::Closing);
}

#[test]
fn missing_send_receipt_quarantines_and_never_replays() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    let timeout = h.wake(2_000);
    assert!(barrier(&timeout).is_some());
    let late = h.sent(id);
    assert!(acts(&late).is_empty());
    assert_eq!(late.snapshot.totals.sent, 0);
    let paused = h.idle(&timeout);
    assert_eq!(paused.snapshot.phase, Phase::Paused(Pause::Uncertain));
}

#[test]
fn uncertain_delivery_discards_early_results() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.facts(vec![Fact::Outcome(Outcome::OwnKill(EntityKey(1)))]);
    let uncertain = h.event(Event::Receipt {
        action: id,
        delivery: Delivery::Unknown,
    });
    let late = h.sent(id);
    assert_eq!(late.snapshot.totals.kills, 0);
    assert_eq!(
        h.idle(&uncertain).snapshot.phase,
        Phase::Paused(Pause::Uncertain)
    );
}

#[test]
fn early_buffer_overflow_pauses_without_counting() {
    let mut h = Harness::new(true);
    let start = h.command(Command::Start);
    let id = acts(&start)[0].id;
    let mut last = start;
    for _ in 0..33 {
        last = h.facts(vec![Fact::Outcome(Outcome::DigEnded(DigEnd::Empty))]);
    }
    assert!(barrier(&last).is_some());
    let late = h.sent(id);
    assert_eq!(late.snapshot.totals.digs, 0);
}

#[test]
fn stale_domain_state_pauses_and_progress_does_not_refresh_it() {
    let mut h = Harness::new(false);
    h.command(Command::Start);
    h.wake(14_000);
    h.event(Event::Progress(ConnectStep::EnteringWorld));
    let stale = h.wake(1_000);
    assert_eq!(stale.snapshot.phase, Phase::Paused(Pause::Uncertain));
    assert!(!stale.snapshot.allowed.contains(&Control::Resume));
}

#[test]
fn a_live_idle_stream_keeps_state_fresh_until_it_falls_silent() {
    let stale = Blocker::NeedsFreshObservation(StateKind::WorldState);
    let mut h = Harness::new(false);
    let mut idle = h.live(0);
    for _ in 0..40 {
        idle = h.live(1_000);
    }
    assert_eq!(idle.snapshot.own.last_state_at, Some(At(h.now)));
    assert!(!idle.snapshot.blockers.contains(&stale));
    assert!(idle.snapshot.allowed.contains(&Control::Start));
    assert_eq!(h.command(Command::Start).snapshot.phase, Phase::Running);
    for _ in 0..40 {
        idle = h.live(1_000);
        assert_eq!(idle.snapshot.phase, Phase::Running);
        assert_eq!(idle.wake_at, Some(At(h.now + 15_000)));
    }
    assert_eq!(h.wake(14_999).snapshot.phase, Phase::Running);
    let silent = h.wake(1);
    assert_eq!(silent.snapshot.phase, Phase::Paused(Pause::Uncertain));
    assert!(silent.snapshot.blockers.contains(&stale));
    assert!(!silent.snapshot.allowed.contains(&Control::Resume));
    let resumed = h.live(1_000);
    assert!(!resumed.snapshot.blockers.contains(&stale));
    assert!(resumed.snapshot.allowed.contains(&Control::Resume));
    assert_eq!(resumed.snapshot.phase, Phase::Paused(Pause::Uncertain));
}

#[test]
fn liveness_never_refreshes_a_lost_reconnecting_or_closing_session() {
    let mut h = Harness::new(false);
    assert_eq!(h.live(1_000).snapshot.own.last_state_at, Some(At(h.now)));
    let old = h.session;
    assert_eq!(h.event(Event::Lost).snapshot.own.last_state_at, None);
    let lost = h.live(1_000);
    assert_eq!(lost.snapshot.phase, Phase::Disconnected);
    assert_eq!(lost.snapshot.own.last_state_at, None);
    assert_eq!(closes(&h.command(Command::Reconnect)), 1);
    assert_eq!(h.live(1_000).snapshot.own.last_state_at, None);
    let closed = h.event(Event::Closed);
    h.session = closed
        .io
        .iter()
        .find_map(|effect| match effect {
            Effect::Open { session, .. } => Some(*session),
            _ => None,
        })
        .unwrap();
    let opening = h.live(1_000);
    assert_eq!(opening.snapshot.phase, Phase::Reconnecting(1));
    assert_eq!(opening.snapshot.own.last_state_at, None);
    let fenced = h.engine.handle(Input::Observed {
        at: At(h.now),
        observation: Observation {
            session: old,
            event: Event::Facts {
                revision: EvidenceKey(900),
                ingress: IngressMark(900),
                facts: vec![Fact::StateCurrent],
            },
        },
    });
    assert_eq!(fenced.snapshot.own.last_state_at, None);
    let baseline = h.event(Event::Baseline {
        support: support(),
        revision: EvidenceKey(1),
        facts: baseline(),
        resumed: true,
    });
    let reported = baseline.snapshot.own.last_state_at;
    assert_eq!(reported, Some(At(h.now)));
    h.idle(&baseline);
    assert_eq!(closes(&h.command(Command::SwitchMode(Mode::Assisted))), 1);
    let closing = h.live(1_000);
    assert_eq!(closing.snapshot.phase, Phase::Closing);
    assert_eq!(closing.snapshot.own.last_state_at, reported);
    h.event(Event::Closed);
    let offline = h.live(1_000);
    assert_eq!(offline.snapshot.own.last_state_at, None);
    assert!(
        offline
            .snapshot
            .blockers
            .contains(&Blocker::NeedsConnection)
    );
}

#[test]
fn liveness_adds_no_value_and_never_overrides_newer_facts() {
    let without_weight = baseline()
        .into_iter()
        .filter(|fact| !matches!(fact, Fact::Weight(_)))
        .collect();
    let mut h = Harness::with(false, support(), without_weight, None);
    let hp = |effects: &Effects| {
        effects
            .snapshot
            .own
            .vitals
            .as_ref()
            .and_then(|vitals| vitals.hp.as_ref())
            .map(|hp| hp.current)
    };
    let mp = |effects: &Effects| {
        effects
            .snapshot
            .own
            .vitals
            .as_ref()
            .and_then(|vitals| vitals.mp.as_ref())
            .map(|mp| mp.current)
    };
    let fact_at = |h: &mut Harness, ingress: u64, fact: Fact| {
        h.revision += 1;
        h.event(Event::Facts {
            revision: EvidenceKey(h.revision),
            ingress: IngressMark(ingress),
            facts: vec![fact],
        })
    };
    fact_at(&mut h, 20, Fact::Hp(gauge(80, 100)));
    fact_at(&mut h, 21, Fact::Mp(gauge(30, 100)));
    h.ingress = 29;
    let live = h.live(30_000);
    assert_eq!(live.snapshot.own.last_state_at, Some(At(h.now)));
    assert!(
        live.snapshot
            .blockers
            .contains(&Blocker::NeedsObservation(StateKind::Weight))
    );
    assert!(!live.snapshot.allowed.contains(&Control::Start));
    assert_eq!(hp(&live), Some(80));
    let late = fact_at(&mut h, 19, Fact::Hp(gauge(50, 100)));
    assert_eq!(hp(&late), Some(80));
    let grouped = fact_at(&mut h, 25, Fact::Mp(gauge(40, 100)));
    assert_eq!(mp(&grouped), Some(40));
    let current = grouped.snapshot.own.last_state_at;
    assert_eq!(current, Some(At(h.now)));
    let queued = h.engine.handle(Input::Observed {
        at: At(1_000),
        observation: Observation {
            session: h.session,
            event: Event::Facts {
                revision: EvidenceKey(900),
                ingress: IngressMark(900),
                facts: vec![Fact::StateCurrent],
            },
        },
    });
    assert_eq!(queued.snapshot.own.last_state_at, current);
}

#[test]
fn unknown_and_missing_maxima_or_inventory_block_start_and_controls() {
    for absent in 0..3 {
        let mut facts = baseline();
        match absent {
            0 => facts.retain(|fact| !matches!(fact, Fact::Inventory(_))),
            1 => facts.retain(|fact| !matches!(fact, Fact::Weight(_))),
            _ => {
                facts.retain(|fact| !matches!(fact, Fact::Hp(_)));
                facts.push(Fact::Hp(gauge(100, 0)));
            }
        }
        let mut h = Harness::with(false, support(), facts, None);
        assert!(!h.engine.snapshot().allowed.contains(&Control::Start));
        let start = h.command(Command::Start);
        assert!(acts(&start).is_empty());
        assert_eq!(start.snapshot.blocked, Some(Refusal::MissingState));
    }
}

#[test]
fn wrong_map_outside_boundary_and_engaged_targets_are_never_attacked() {
    let mut h = Harness::new(false);
    h.facts(vec![
        entity(1, center(), Some(true)),
        entity(2, center(), None),
        entity(3, Point { x: 200, y: 100 }, Some(false)),
    ]);
    assert!(acts(&h.command(Command::Start)).is_empty());
    let wrong = h.facts(vec![Fact::Position {
        map: MapKey(2),
        point: center(),
    }]);
    assert_eq!(wrong.snapshot.phase, Phase::Paused(Pause::Uncertain));
    assert!(wrong.snapshot.entities.is_empty());
    let mut h = Harness::new(false);
    h.command(Command::Start);
    let outside = h.facts(vec![Fact::Position {
        map: MapKey(1),
        point: Point { x: 111, y: 100 },
    }]);
    assert_eq!(outside.snapshot.phase, Phase::Paused(Pause::Uncertain));
}

#[test]
fn fainting_weight_hp_and_drill_pause_safely() {
    let cases = [
        (Fact::Hp(gauge(0, 100)), Pause::Fainted),
        (Fact::Hp(gauge(20, 100)), Pause::HpPotions),
        (Fact::Weight(gauge(90, 100)), Pause::Weight),
        (Fact::Drill(Drill::Absent), Pause::Drill),
        (
            Fact::Drill(Drill::Equipped { life: 10, depth: 0 }),
            Pause::Depth,
        ),
    ];
    for (fact, reason) in cases {
        let mut h = Harness::new(true);
        h.command(Command::Start);
        let paused = h.facts(vec![fact]);
        assert_eq!(paused.snapshot.phase, Phase::Pausing);
        let final_state = h.idle(&paused);
        assert_eq!(final_state.snapshot.phase, Phase::Paused(reason));
        assert!(!final_state.snapshot.allowed.contains(&Control::Resume));
    }
}

#[test]
fn paused_setup_edits_keep_activity_locked_and_run_identity() {
    let mut h = Harness::new(false);
    h.command(Command::Start);
    let run = h.engine.run_key();
    h.command(Command::Pause);
    assert_eq!(h.engine.snapshot().phase, Phase::Paused(Pause::User));
    let change = h.command(Command::Configure(setup(true, None)));
    assert_eq!(change.snapshot.blocked, Some(Refusal::WrongPhase));
    let edit = h.command(Command::Configure(setup(
        false,
        Some(Goal {
            item: ItemKind(5),
            gained: 5,
        }),
    )));
    assert!(edit.snapshot.setup.unwrap().draft().goal.is_some());
    assert_eq!(h.engine.run_key(), run);
    assert!(
        h.command(Command::Resume)
            .snapshot
            .allowed
            .contains(&Control::Pause)
    );
}

#[test]
fn reconnect_requires_closed_fresh_baseline_and_idle_then_stays_paused() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.sent(id);
    let run = h.engine.run_key();
    let lost = h.event(Event::Lost);
    assert_eq!(lost.snapshot.phase, Phase::Disconnected);
    let reconnect = h.command(Command::Reconnect);
    assert_eq!(closes(&reconnect), 1);
    let closed = h.event(Event::Closed);
    let old = h.session;
    h.session = closed
        .io
        .iter()
        .find_map(|effect| match effect {
            Effect::Open { session, .. } => Some(*session),
            _ => None,
        })
        .unwrap();
    assert_ne!(h.session, old);
    let stale = h.engine.handle(Input::Observed {
        at: At(h.now),
        observation: Observation {
            session: old,
            event: Event::Facts {
                revision: EvidenceKey(99),
                ingress: IngressMark(99),
                facts: vec![Fact::Outcome(Outcome::OwnKill(EntityKey(1)))],
            },
        },
    });
    assert_eq!(stale.snapshot.totals.kills, 0);
    let baseline = h.event(Event::Baseline {
        support: support(),
        revision: EvidenceKey(1),
        facts: baseline(),
        resumed: true,
    });
    assert_eq!(baseline.snapshot.phase, Phase::Paused(Pause::Reconnected));
    assert!(acts(&baseline).is_empty());
    assert!(!baseline.snapshot.allowed.contains(&Control::Resume));
    let idle = h.idle(&baseline);
    assert!(idle.snapshot.allowed.contains(&Control::Resume));
    assert!(acts(&idle).is_empty());
    assert_eq!(h.engine.run_key(), run);
}

#[test]
fn mode_switch_waits_for_old_closed_and_refuses_held_run() {
    let mut h = Harness::new(false);
    h.command(Command::Start);
    let refused = h.command(Command::SwitchMode(Mode::Assisted));
    assert_eq!(refused.snapshot.mode, Mode::Clientless);
    assert_eq!(closes(&refused), 0);
    h.command(Command::Stop);
    let switch = h.command(Command::SwitchMode(Mode::Assisted));
    assert_eq!(closes(&switch), 1);
    assert_eq!(switch.snapshot.mode, Mode::Clientless);
    let closed = h.event(Event::Closed);
    assert_eq!(closed.snapshot.mode, Mode::Assisted);
    assert_eq!(closed.snapshot.phase, Phase::ClientAbsent);
}

#[test]
fn unsupported_action_capabilities_do_not_emit_actions() {
    for cap in 0..3 {
        let mut capabilities = support();
        match cap {
            0 => capabilities.basic_attack = false,
            1 => capabilities.dig_cycle = false,
            _ => capabilities.move_to = false,
        }
        let mut h = Harness::with(cap == 1, capabilities, baseline(), None);
        // Combat attacks in place, so a drop away from the character is what needs a move.
        h.facts(vec![
            entity(1, Point { x: 101, y: 100 }, Some(false)),
            Fact::Drop(Drop {
                key: DropKey(1),
                kind: ItemKind(1),
                position: Point { x: 101, y: 100 },
                quantity: 1,
            }),
        ]);
        let start = h.command(Command::Start);
        assert!(acts(&start).is_empty());
        assert_eq!(start.snapshot.blocked, Some(Refusal::Unsupported));
    }
}

#[test]
fn dig_cycle_follows_a_client_dig_feature_that_arrives_after_baseline() {
    let mut capabilities = support();
    capabilities.dig_cycle = false;
    let mut h = Harness::with(true, capabilities, baseline(), None);
    let missing = Blocker::NeedsCapability(Capability::DigCycle);
    assert!(h.engine.snapshot().blockers.contains(&missing));
    let arrived = h.event(Event::DigCycle(true));
    assert!(!arrived.snapshot.blockers.contains(&missing));
    let start = h.command(Command::Start);
    assert_eq!(acts(&start)[0].action, Action::DigAt(center()));
    let gone = h.event(Event::DigCycle(false));
    assert!(gone.snapshot.blockers.contains(&missing));
    assert_eq!(gone.snapshot.blocked, Some(Refusal::Unsupported));
}

#[test]
fn request_replays_and_backwards_time_do_not_duplicate_effects_or_elapsed() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    let request = h.request;
    let replay = h.engine.handle(Input::User {
        at: At(0),
        request: RequestKey(request),
        command: Command::Stop,
    });
    assert_eq!(replay.snapshot.phase, Phase::Running);
    assert!(replay.io.is_empty());
    h.sent(id);
    let before = h.engine.snapshot().totals.elapsed_ms;
    let backwards = h.engine.handle(Input::Wake(At(0)));
    assert_eq!(backwards.snapshot.totals.elapsed_ms, before);
}

#[test]
fn display_configuration_and_logs_are_bounded() {
    assert!(Percent::checked(101).is_err());
    assert!(DisplayText::checked("secret\nbody".into()).is_err());
    assert!(DisplayText::checked("x".repeat(257)).is_err());
    assert!(
        Boundary::checked(
            MapKey(1),
            Point { x: i32::MAX, y: 0 },
            NonZeroU16::new(1).unwrap()
        )
        .is_err()
    );
    assert!(
        Boundary::checked(MapKey(1), Point { x: 0, y: 0 }, NonZeroU16::new(1).unwrap()).is_err()
    );
    let mut h = Harness::new(false);
    for _ in 0..250 {
        h.command(Command::Resume);
    }
    let snapshot = h.engine.snapshot();
    assert_eq!(snapshot.log.len(), 200);
    assert!(snapshot.log.windows(2).all(|pair| pair[0].at <= pair[1].at));
    assert!(
        Setup::checked(SetupDraft {
            activity: Activity::Combat {
                targets: Targets::Only(vec![])
            },
            boundary: Boundary::checked(MapKey(1), center(), NonZeroU16::new(1).unwrap())
                .unwrap_or_else(|_| panic!("area")),
            hp_at: Percent::checked(30).unwrap_or_else(|_| panic!("hp")),
            weight_limit: Percent::checked(90).unwrap_or_else(|_| panic!("weight")),
            goal: None
        })
        .is_err()
    );
    assert!(setup(true, None).draft().boundary.parts().2.get() > 0);
}

#[test]
fn manual_input_before_send_quarantines_early_results_and_does_not_refresh_state() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.facts(vec![Fact::Outcome(Outcome::OwnKill(EntityKey(1)))]);
    let state_at = h.engine.snapshot().own.last_state_at;
    let manual = h.event(Event::ManualInput);
    assert_eq!(manual.snapshot.phase, Phase::Pausing);
    assert_eq!(manual.snapshot.own.last_state_at, state_at);
    let late = h.sent(id);
    assert_eq!(late.snapshot.totals.kills, 0);
    assert_eq!(late.snapshot.totals.sent, 0);
    let paused = h.idle(&manual);
    assert_eq!(paused.snapshot.phase, Phase::Paused(Pause::ManualInput));
    assert!(acts(&paused).is_empty());
}

#[test]
fn manual_input_during_sent_action_prevents_later_automation_attribution() {
    let mut h = Harness::new(true);
    let start = h.command(Command::Start);
    let id = acts(&start)[0].id;
    h.sent(id);
    let manual = h.event(Event::ManualInput);
    let result = h.facts(vec![Fact::Outcome(Outcome::DigEnded(DigEnd::Empty))]);
    assert_eq!(result.snapshot.totals.digs, 0);
    assert_eq!(
        h.idle(&manual).snapshot.phase,
        Phase::Paused(Pause::ManualInput)
    );
}

#[test]
fn a_slow_idle_proof_without_a_run_keeps_the_session_and_asks_again() {
    let mut h = Harness::new(false);
    let manual = h.event(Event::ManualInput);
    let key = barrier(&manual).expect("quiescence requested");
    for _ in 0..3 {
        let expired = h.wake(DRAIN_MS);
        assert_eq!(closes(&expired), 0);
        assert_eq!(barrier(&expired), Some(key));
        assert_eq!(expired.snapshot.phase, Phase::Ready);
        assert!(
            expired
                .snapshot
                .blockers
                .contains(&Blocker::NeedsQuiescence)
        );
    }
    let settled = h.event(Event::Quiescent(key));
    assert!(
        !settled
            .snapshot
            .blockers
            .contains(&Blocker::NeedsQuiescence)
    );
    assert!(settled.snapshot.allowed.contains(&Control::Start));
}

#[test]
fn manual_play_during_a_run_without_a_pending_action_keeps_the_session() {
    let mut h = Harness::new(false);
    let start = h.command(Command::Start);
    assert!(acts(&start).is_empty());
    let manual = h.event(Event::ManualInput);
    let key = barrier(&manual).expect("quiescence requested");
    let expired = h.wake(DRAIN_MS);
    assert_eq!(closes(&expired), 0);
    assert_eq!(barrier(&expired), Some(key));
    let settled = h.event(Event::Quiescent(key));
    assert_eq!(settled.snapshot.phase, Phase::Paused(Pause::ManualInput));
}

#[test]
fn manual_input_while_idle_still_requires_new_quiescence() {
    let mut h = Harness::new(false);
    h.command(Command::Start);
    let manual = h.event(Event::ManualInput);
    assert!(barrier(&manual).is_some());
    assert!(!manual.snapshot.allowed.contains(&Control::Resume));
    assert_eq!(
        h.idle(&manual).snapshot.phase,
        Phase::Paused(Pause::ManualInput)
    );
}

#[test]
fn a_map_change_during_a_run_pauses_it_without_closing_the_session() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.sent(id);
    let moved = h.facts(vec![
        Fact::MapChanged,
        Fact::Position {
            map: MapKey(2),
            point: center(),
        },
    ]);
    assert_eq!(moved.snapshot.phase, Phase::Pausing);
    assert!(moved.snapshot.entities.is_empty());
    assert!(acts(&moved).is_empty());
    let key = barrier(&moved).expect("quiescence requested");
    let late = h.facts(vec![Fact::Outcome(Outcome::OwnKill(EntityKey(1)))]);
    assert_eq!(late.snapshot.totals.kills, 0);
    let slow = h.wake(DRAIN_MS);
    assert_eq!(closes(&slow), 0);
    assert_eq!(barrier(&slow), Some(key));
    let paused = h.event(Event::Quiescent(key));
    assert_eq!(paused.snapshot.phase, Phase::Paused(Pause::MapChanged));
    assert!(acts(&paused).is_empty());
    assert!(!paused.snapshot.allowed.contains(&Control::Resume));
    assert!(
        paused
            .snapshot
            .blockers
            .contains(&Blocker::NeedsBoundaryMap(MapKey(1)))
    );
}

#[test]
fn a_map_change_outside_a_run_keeps_the_session_ready_and_forgets_the_old_map() {
    let mut h = Harness::new(false);
    h.facts(vec![entity(1, center(), Some(false))]);
    let old = h.ingress;
    let moved = h.facts(vec![
        Fact::MapChanged,
        Fact::Position {
            map: MapKey(1),
            point: center(),
        },
    ]);
    assert_eq!(moved.snapshot.phase, Phase::Ready);
    assert!(moved.snapshot.entities.is_empty());
    assert!(barrier(&moved).is_none());
    assert_eq!(closes(&moved), 0);
    assert!(moved.snapshot.allowed.contains(&Control::Start));
    h.revision += 1;
    let delayed = h.event(Event::Facts {
        revision: EvidenceKey(h.revision),
        ingress: IngressMark(old),
        facts: vec![entity(1, center(), Some(false))],
    });
    assert!(delayed.snapshot.entities.is_empty());
}

#[test]
fn inventory_knowledge_is_explicit_and_invalidated_by_lost_continuity() {
    let mut facts = baseline();
    facts.retain(|fact| !matches!(fact, Fact::Inventory(_)));
    let mut h = Harness::with(false, support(), facts, None);
    assert_eq!(
        h.engine.snapshot().own.inventory,
        InventoryKnowledge::Unknown
    );
    assert!(
        h.engine
            .snapshot()
            .blockers
            .contains(&Blocker::NeedsObservation(StateKind::Inventory))
    );
    let observed = h.facts(vec![Fact::Inventory(Inventory::Put(Stack {
        key: StackKey(1),
        kind: ItemKind(1),
        quantity: 3,
    }))]);
    assert_eq!(
        observed.snapshot.own.inventory,
        InventoryKnowledge::Observed
    );
    assert!(observed.snapshot.allowed.contains(&Control::Start));
    let lost = h.event(Event::Lost);
    assert_eq!(lost.snapshot.own.inventory, InventoryKnowledge::Unknown);
    assert!(
        lost.snapshot
            .blockers
            .contains(&Blocker::NeedsObservation(StateKind::Inventory))
    );
}

#[test]
fn partial_inventory_only_credits_known_stack_deltas() {
    let mut facts = baseline();
    facts.retain(|fact| !matches!(fact, Fact::Inventory(_)));
    facts.push(Fact::Inventory(Inventory::Put(Stack {
        key: StackKey(1),
        kind: ItemKind(7),
        quantity: 2,
    })));
    let mut h = Harness::with(
        true,
        support(),
        facts,
        Some(Goal {
            item: ItemKind(7),
            gained: 10,
        }),
    );
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    let unknown_stack = h.facts(vec![Fact::Inventory(Inventory::Put(Stack {
        key: StackKey(2),
        kind: ItemKind(7),
        quantity: 9,
    }))]);
    assert_eq!(unknown_stack.snapshot.totals.goal_gained, 0);
    let known_delta = h.facts(vec![Fact::Inventory(Inventory::Quantity(StackKey(1), 5))]);
    assert_eq!(known_delta.snapshot.totals.goal_gained, 3);
}

#[test]
fn inventory_after_completed_action_is_credited_without_relabeling_dig_report() {
    let mut h = Harness::with(
        true,
        support(),
        baseline(),
        Some(Goal {
            item: ItemKind(7),
            gained: 1,
        }),
    );
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    let dig = h.facts(vec![Fact::Outcome(Outcome::DigEnded(
        DigEnd::ItemReported(ItemKind(7)),
    ))]);
    let next = h.idle(&dig);
    assert!(matches!(acts(&next)[0].action, Action::MoveTo(_)));
    let inventory = h.facts(vec![Fact::Inventory(Inventory::Put(Stack {
        key: StackKey(1),
        kind: ItemKind(7),
        quantity: 1,
    }))]);
    assert_eq!(inventory.snapshot.totals.goal_gained, 1);
    assert_eq!(inventory.snapshot.totals.digs, 1);
    assert_eq!(inventory.snapshot.phase, Phase::Pausing);
}

#[test]
fn concrete_blockers_name_resources_and_resume_unlock_conditions() {
    let mut h = Harness::new(true);
    h.command(Command::Start);
    let paused = h.facts(vec![
        Fact::Hp(gauge(10, 100)),
        Fact::Weight(gauge(95, 100)),
        Fact::Drill(Drill::Equipped { life: 0, depth: 0 }),
    ]);
    for blocker in [
        Blocker::NeedsHpAbove { percent: 30 },
        Blocker::NeedsWeightBelow { percent: 90 },
        Blocker::NeedsDrillLife { minimum: 1 },
        Blocker::NeedsDrillDepth,
        Blocker::NeedsQuiescence,
        Blocker::NeedsActionDrain,
    ] {
        assert!(paused.snapshot.blockers.contains(&blocker));
    }
    h.idle(&paused);
    let restored = h.facts(vec![
        Fact::Hp(gauge(100, 100)),
        Fact::Weight(gauge(0, 100)),
        Fact::Drill(Drill::Equipped {
            life: 100,
            depth: 1,
        }),
    ]);
    assert!(restored.snapshot.blockers.is_empty());
    assert!(restored.snapshot.allowed.contains(&Control::Resume));
    let resume = h.command(Command::Resume);
    assert_eq!(acts(&resume).len(), 1);
}

#[test]
fn missing_quiescence_capability_has_specific_preflight_blocker() {
    let mut capabilities = support();
    capabilities.quiesce = false;
    let mut h = Harness::with(true, capabilities, baseline(), None);
    assert!(
        h.engine
            .snapshot()
            .blockers
            .contains(&Blocker::NeedsCapability(Capability::Quiescence))
    );
    assert_eq!(
        h.command(Command::Start).snapshot.blocked,
        Some(Refusal::Unsupported)
    );
}

#[test]
fn ordinary_confirmation_and_dig_deadlines_never_authorize_replay() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.sent(id);
    let timeout = h.wake(5_000);
    assert!(barrier(&timeout).is_some());
    assert!(acts(&timeout).is_empty());
    assert_eq!(
        h.idle(&timeout).snapshot.phase,
        Phase::Paused(Pause::Uncertain)
    );
    let mut h = Harness::new(true);
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    for _ in 0..2 {
        h.wake(10_000);
        h.facts(vec![Fact::Hp(gauge(100, 100))]);
    }
    let timeout = h.wake(10_000);
    assert!(barrier(&timeout).is_some());
    assert!(acts(&timeout).is_empty());
    assert_eq!(timeout.snapshot.totals.digs, 0);
}

#[test]
fn backwards_wrap_and_clock_exhaustion_cannot_leave_an_action_dispatchable() {
    let mut h = Harness::new(false);
    h.start_attack();
    let exhausted = h.engine.handle(Input::Wake(At(u64::MAX)));
    assert_eq!(closes(&exhausted), 1);
    assert_eq!(exhausted.snapshot.phase, Phase::Closing);
    assert!(acts(&exhausted).is_empty());
    let wrapped = h.engine.handle(Input::Wake(At(1)));
    assert!(acts(&wrapped).is_empty());
    assert!(!wrapped.snapshot.allowed.contains(&Control::Resume));
}

#[test]
fn duplicate_results_inside_one_batch_count_one_dig_cycle() {
    let mut h = Harness::new(true);
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    let result = h.facts(vec![
        Fact::Outcome(Outcome::DigEnded(DigEnd::Empty)),
        Fact::Outcome(Outcome::DigEnded(DigEnd::Empty)),
    ]);
    assert_eq!(result.snapshot.totals.digs, 1);
    assert_eq!(result.snapshot.totals.empty, 1);
    assert_eq!(result.snapshot.totals.confirmed, 1);
}

#[test]
fn reconnect_attempts_are_bounded_and_never_replay_held_actions() {
    let mut h = Harness::new(false);
    h.start_attack();
    let run = h.engine.run_key();
    h.event(Event::Lost);
    for attempt in 1..=5 {
        let reconnect = h.command(Command::Reconnect);
        let opening = if closes(&reconnect) > 0 {
            h.event(Event::Closed)
        } else {
            reconnect
        };
        h.session = opening
            .io
            .iter()
            .find_map(|effect| match effect {
                Effect::Open { session, .. } => Some(*session),
                _ => None,
            })
            .unwrap();
        assert_eq!(opening.snapshot.phase, Phase::Reconnecting(attempt));
        assert!(acts(&opening).is_empty());
        let failed = h.event(Event::Failed(Halt::ConnectFailed));
        assert_eq!(closes(&failed), 1);
        h.event(Event::Closed);
        assert_eq!(h.engine.run_key(), run);
    }
    let refused = h.command(Command::Reconnect);
    assert!(refused.io.is_empty());
    assert!(!refused.snapshot.allowed.contains(&Control::Reconnect));
    assert_eq!(refused.snapshot.phase, Phase::Disconnected);
    assert!(refused.snapshot.allowed.contains(&Control::Stop));
}

#[test]
fn an_assisted_reconnect_reports_its_wait_for_the_game_login() {
    let opened = |effects: &Effects| {
        effects
            .io
            .iter()
            .find_map(|effect| match effect {
                Effect::Open { session, .. } => Some(*session),
                _ => None,
            })
            .unwrap()
    };
    let mut h = Harness::new(false);
    h.command(Command::SwitchMode(Mode::Assisted));
    h.event(Event::Closed);
    h.session = opened(&h.command(Command::Open));
    h.event(Event::Progress(ConnectStep::AwaitingGameLogin));
    let online = h.event(Event::Baseline {
        support: support(),
        revision: EvidenceKey(1),
        facts: baseline(),
        resumed: false,
    });
    assert_eq!(online.snapshot.connect_step, None);
    if let Some(key) = barrier(&online) {
        h.event(Event::Quiescent(key));
    }
    h.start_attack();
    h.event(Event::Lost);
    let reconnect = h.command(Command::Reconnect);
    let opening = if closes(&reconnect) > 0 {
        h.event(Event::Closed)
    } else {
        reconnect
    };
    h.session = opened(&opening);
    assert_eq!(opening.snapshot.phase, Phase::Reconnecting(1));
    assert_eq!(
        opening.snapshot.connect_step,
        Some(ConnectStep::LoadingGame(0))
    );
    let waiting = h.event(Event::Progress(ConnectStep::AwaitingGameLogin));
    assert_eq!(waiting.snapshot.phase, Phase::Reconnecting(1));
    assert_eq!(
        waiting.snapshot.connect_step,
        Some(ConnectStep::AwaitingGameLogin)
    );
    let recovered = h.event(Event::Baseline {
        support: support(),
        revision: EvidenceKey(1),
        facts: baseline(),
        resumed: false,
    });
    assert_eq!(recovered.snapshot.connect_step, None);
}

#[test]
fn failed_clientless_reconnect_with_held_run_offers_reconnect_or_end_run() {
    let mut h = Harness::new(false);
    h.start_attack();
    let run = h.engine.run_key();
    h.event(Event::Lost);
    assert_eq!(closes(&h.command(Command::Reconnect)), 1);
    let opening = h.event(Event::Closed);
    h.session = opening
        .io
        .iter()
        .find_map(|effect| match effect {
            Effect::Open { session, mode } => {
                assert_eq!(*mode, Mode::Clientless);
                Some(*session)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(opening.snapshot.phase, Phase::Reconnecting(1));
    assert_eq!(closes(&h.event(Event::Failed(Halt::ConnectFailed))), 1);
    let failed = h.event(Event::Closed);
    assert_eq!(failed.snapshot.phase, Phase::Disconnected);
    assert!(failed.snapshot.allowed.contains(&Control::Reconnect));
    assert!(failed.snapshot.allowed.contains(&Control::Stop));
    assert!(!failed.snapshot.allowed.contains(&Control::Open));
    assert_eq!(h.engine.run_key(), run);

    let ended = h.command(Command::Stop);
    assert_eq!(ended.snapshot.phase, Phase::Offline);
    assert!(ended.snapshot.allowed.contains(&Control::Open));
    assert_eq!(h.engine.run_key(), None);
}

#[test]
fn stop_after_finish_and_duplicate_request_are_noops() {
    let mut h = Harness::new(false);
    h.command(Command::Start);
    h.command(Command::Stop);
    let stopped = h.command(Command::Stop);
    assert!(stopped.io.is_empty());
    assert!(
        !stopped
            .snapshot
            .log
            .iter()
            .any(|entry| matches!(entry.event, SafeLog::Refused(_)))
    );
}

#[test]
fn busy_character_and_unknown_choices_are_refused() {
    let mut h = Harness::new(false);
    h.command(Command::SwitchMode(Mode::Assisted));
    h.event(Event::Closed);
    let open = h.command(Command::Open);
    h.session = open
        .io
        .iter()
        .find_map(|effect| match effect {
            Effect::Open { session, .. } => Some(*session),
            _ => None,
        })
        .unwrap();
    h.event(Event::Characters(vec![CharacterChoice {
        key: CharacterKey(1),
        label: label(),
        busy: true,
    }]));
    let busy = h.command(Command::ChooseCharacter(CharacterKey(1)));
    assert_eq!(busy.snapshot.blocked, Some(Refusal::Busy));
    assert!(busy.io.is_empty());
    let unknown = h.command(Command::ChooseCharacter(CharacterKey(2)));
    assert_eq!(unknown.snapshot.blocked, Some(Refusal::MissingState));
}

#[test]
fn active_setup_is_locked_and_paused_goal_edits_follow_selected_item() {
    let mut h = Harness::with(
        true,
        support(),
        baseline(),
        Some(Goal {
            item: ItemKind(7),
            gained: 10,
        }),
    );
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    let active = h.command(Command::Configure(setup(true, None)));
    assert_eq!(active.snapshot.blocked, Some(Refusal::WrongPhase));
    h.facts(vec![Fact::Inventory(Inventory::Put(Stack {
        key: StackKey(1),
        kind: ItemKind(7),
        quantity: 2,
    }))]);
    let pause = h.command(Command::Pause);
    h.idle(&pause);
    let edit = h.command(Command::Configure(setup(
        true,
        Some(Goal {
            item: ItemKind(9),
            gained: 1,
        }),
    )));
    assert_eq!(edit.snapshot.totals.goal_gained, 0);
}

#[test]
fn combat_selects_nearest_eligible_configured_target_deterministically() {
    let mut h = Harness::new(false);
    let wrong_species = Fact::Entity(Entity {
        key: EntityKey(1),
        species: SpeciesKey(2),
        label: label(),
        position: center(),
        alive: true,
        hp: None,
        engaged_by_other: Some(false),
    });
    h.facts(vec![
        wrong_species,
        entity(4, Point { x: 104, y: 100 }, Some(false)),
        entity(3, Point { x: 102, y: 100 }, Some(false)),
        entity(2, Point { x: 102, y: 100 }, Some(false)),
    ]);
    let start = h.command(Command::Start);
    assert_eq!(acts(&start)[0].action, Action::BasicAttack(EntityKey(2)));
}

#[test]
fn a_running_or_draining_run_reports_no_blocked_control() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    assert_eq!(h.engine.snapshot().phase, Phase::Running);
    assert_eq!(h.engine.snapshot().blocked, None);
    h.sent(id);
    let draining = h.command(Command::Pause);
    assert_eq!(draining.snapshot.phase, Phase::Pausing);
    assert_eq!(draining.snapshot.blocked, None);
    // A paused run still reports what blocks Resume.
    let mut h = Harness::new(false);
    h.command(Command::Start);
    let paused = h.facts(vec![Fact::HpPotions(0)]);
    assert_eq!(paused.snapshot.phase, Phase::Paused(Pause::HpPotions));
    assert!(paused.snapshot.blocked.is_some());
}

#[test]
fn combat_requires_observed_hp_potions_and_restocks_unlock_resume() {
    let mut h = Harness::new(false);
    h.command(Command::Start);
    let paused = h.facts(vec![Fact::HpPotions(0)]);
    assert_eq!(paused.snapshot.phase, Phase::Paused(Pause::HpPotions));
    assert!(paused.snapshot.blockers.contains(&Blocker::NeedsHpPotions));
    assert!(!paused.snapshot.allowed.contains(&Control::Resume));
    let stocked = h.facts(vec![Fact::HpPotions(1)]);
    assert!(stocked.snapshot.allowed.contains(&Control::Resume));
    let mut facts = baseline();
    facts.retain(|fact| !matches!(fact, Fact::HpPotions(_)));
    let mut unknown = Harness::with(false, support(), facts, None);
    let refused = unknown.command(Command::Start);
    assert!(
        refused
            .snapshot
            .blockers
            .contains(&Blocker::NeedsObservation(StateKind::HpPotions))
    );
    assert!(acts(&refused).is_empty());
}

#[test]
fn pre_send_aux_inventory_is_not_goal_credit() {
    let mut h = Harness::with(
        true,
        support(),
        baseline(),
        Some(Goal {
            item: ItemKind(7),
            gained: 1,
        }),
    );
    let start = h.command(Command::Start);
    let id = acts(&start)[0].id;
    h.event(Event::Facts {
        revision: EvidenceKey(9),
        ingress: IngressMark(5),
        facts: vec![Fact::Inventory(Inventory::Put(Stack {
            key: StackKey(1),
            kind: ItemKind(7),
            quantity: 1,
        }))],
    });
    let sent = h.event(Event::Receipt {
        action: id,
        delivery: Delivery::Sent(IngressMark(10)),
    });
    assert_eq!(sent.snapshot.totals.goal_gained, 0);
}

#[test]
fn conflicting_action_results_quarantine_without_counting_a_kill() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.sent(id);
    let conflict = h.facts(vec![
        Fact::Outcome(Outcome::OwnKill(EntityKey(1))),
        Fact::Outcome(Outcome::Rejected {
            subject: Subject::Target(EntityKey(1)),
            reason: Rejection::Other,
        }),
    ]);
    assert_eq!(conflict.snapshot.totals.kills, 0);
    assert_eq!(conflict.snapshot.totals.confirmed, 0);
    assert_eq!(
        h.idle(&conflict).snapshot.phase,
        Phase::Paused(Pause::Uncertain)
    );
}

#[test]
fn result_after_rejection_does_not_count_a_kill() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.sent(id);
    let rejected = h.facts(vec![Fact::Outcome(Outcome::Rejected {
        subject: Subject::Target(EntityKey(1)),
        reason: Rejection::Other,
    })]);
    let late = h.facts(vec![Fact::Outcome(Outcome::OwnKill(EntityKey(1)))]);
    assert_eq!(late.snapshot.totals.kills, 0);
    assert_eq!(late.snapshot.totals.confirmed, 0);
    assert_eq!(
        h.idle(&rejected).snapshot.phase,
        Phase::Paused(Pause::Uncertain)
    );
}

#[test]
fn server_resource_rejection_pauses_instead_of_repeating_the_action() {
    let mut h = Harness::new(true);
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    let rejected = h.facts(vec![Fact::Outcome(Outcome::Rejected {
        subject: Subject::Dig,
        reason: Rejection::Resources,
    })]);
    let paused = h.idle(&rejected);
    assert_eq!(paused.snapshot.phase, Phase::Paused(Pause::Uncertain));
    assert!(acts(&paused).is_empty());
}

#[test]
fn required_movement_capability_uses_the_same_preflight_as_start() {
    let mut capabilities = support();
    capabilities.move_to = false;
    let mut h = Harness::with(false, capabilities, baseline(), None);
    let state = h.facts(vec![Fact::Drop(Drop {
        key: DropKey(1),
        kind: ItemKind(1),
        position: Point { x: 101, y: 100 },
        quantity: 1,
    })]);
    assert!(
        state
            .snapshot
            .blockers
            .contains(&Blocker::NeedsCapability(Capability::Movement))
    );
    assert!(!state.snapshot.allowed.contains(&Control::Start));
    assert!(acts(&h.command(Command::Start)).is_empty());
}

#[test]
fn unsupported_pickup_leaves_drops_and_keeps_attacking() {
    let mut capabilities = support();
    capabilities.pickup = false;
    let mut h = Harness::with(false, capabilities, baseline(), None);
    let state = h.facts(vec![
        Fact::Drop(Drop {
            key: DropKey(1),
            kind: ItemKind(1),
            position: center(),
            quantity: 1,
        }),
        entity(1, Point { x: 101, y: 100 }, Some(false)),
    ]);
    assert!(
        !state
            .snapshot
            .blockers
            .contains(&Blocker::NeedsCapability(Capability::Pickup))
    );
    let started = h.command(Command::Start);
    let ticket = acts(&started)[0];
    assert!(matches!(
        ticket.action,
        Action::BasicAttack(EntityKey(1)) | Action::MoveTo(Point { x: 101, y: 100 })
    ));
}

#[test]
fn combat_goal_without_pickup_cannot_start() {
    let mut capabilities = support();
    capabilities.pickup = false;
    let goal = Some(Goal {
        item: ItemKind(1),
        gained: 1,
    });
    let mut h = Harness::with(false, capabilities, baseline(), goal);
    let state = h.facts(vec![entity(1, Point { x: 101, y: 100 }, Some(false))]);
    assert!(
        state
            .snapshot
            .blockers
            .contains(&Blocker::NeedsCapability(Capability::Pickup))
    );
    assert!(acts(&h.command(Command::Start)).is_empty());
}

#[test]
fn manual_input_supersedes_an_older_quiescence_barrier() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.sent(id);
    let pause = h.command(Command::Pause);
    let manual = h.event(Event::ManualInput);
    assert_ne!(barrier(&pause), barrier(&manual));
    let old = h.idle(&pause);
    assert_eq!(old.snapshot.phase, Phase::Pausing);
    assert!(!old.snapshot.allowed.contains(&Control::Resume));
    assert_eq!(
        h.idle(&manual).snapshot.phase,
        Phase::Paused(Pause::ManualInput)
    );
}

#[test]
fn closed_session_invalidates_inventory_knowledge() {
    let mut h = Harness::new(false);
    assert_eq!(
        h.engine.snapshot().own.inventory,
        InventoryKnowledge::Observed
    );
    let closed = h.event(Event::Closed);
    assert_eq!(closed.snapshot.own.inventory, InventoryKnowledge::Unknown);
}

#[test]
fn low_hp_uses_only_an_adapter_verified_nonempty_potion_stack_and_waits_for_result() {
    let mut capabilities = support();
    capabilities.hp_potion = true;
    let mut facts = baseline();
    facts.push(Fact::Hp(gauge(20, 100)));
    facts.push(Fact::Inventory(Inventory::Put(Stack {
        key: StackKey(9),
        kind: ItemKind(81),
        quantity: 2,
    })));
    facts.push(Fact::HpPotionStack(Some(StackKey(9))));
    let mut h = Harness::with(false, capabilities, facts, None);
    let start = h.command(Command::Start);
    let id = acts(&start)[0].id;
    assert_eq!(acts(&start)[0].action, Action::HpPotion(StackKey(9)));
    h.sent(id);
    let result = h.facts(vec![
        Fact::Outcome(Outcome::PotionUsed(StackKey(9))),
        Fact::Hp(gauge(90, 100)),
        Fact::Inventory(Inventory::Quantity(StackKey(9), 1)),
    ]);
    assert_eq!(result.snapshot.totals.confirmed, 1);
    assert!(acts(&result).is_empty());
    let idle = h.idle(&result);
    assert!(acts(&idle).is_empty());
    assert_eq!(idle.snapshot.own.vitals.unwrap().hp.unwrap().current, 90);
}

#[test]
fn missing_or_empty_verified_potion_stack_blocks_resource_use() {
    for empty in [false, true] {
        let mut capabilities = support();
        capabilities.hp_potion = true;
        let mut facts = baseline();
        facts.push(Fact::Hp(gauge(20, 100)));
        if empty {
            facts.push(Fact::Inventory(Inventory::Put(Stack {
                key: StackKey(9),
                kind: ItemKind(81),
                quantity: 0,
            })));
            facts.push(Fact::HpPotionStack(Some(StackKey(9))));
        }
        let mut h = Harness::with(false, capabilities, facts, None);
        let start = h.command(Command::Start);
        assert!(acts(&start).is_empty());
        assert!(!start.snapshot.allowed.contains(&Control::Start));
        assert!(start.snapshot.blockers.contains(&if empty {
            Blocker::NeedsHpPotions
        } else {
            Blocker::NeedsObservation(StateKind::HpPotionStack)
        }));
    }
}

#[test]
fn review_stop_timeout_ends_the_requested_run() {
    let mut h = Harness::new(true);
    h.command(Command::Start);
    h.command(Command::Stop);
    let timeout = h.wake(2_000);
    assert_eq!(closes(&timeout), 1);
    let closed = h.event(Event::Closed);
    assert_eq!(closed.snapshot.phase, Phase::Offline);
    assert_eq!(
        h.engine.run_key(),
        None,
        "Stop must end the run after its adapter closes"
    );
    assert!(closed.snapshot.allowed.contains(&Control::SwitchMode));
}

#[test]
fn review_successful_reconnect_resets_the_failure_budget() {
    let mut h = Harness::new(false);
    h.command(Command::Start);
    for recovery in 1..=6 {
        h.event(Event::Lost);
        let reconnect = h.command(Command::Reconnect);
        let closed = if closes(&reconnect) > 0 {
            h.event(Event::Closed)
        } else {
            reconnect
        };
        let new_session = closed.io.iter().find_map(|effect| match effect {
            Effect::Open { session, .. } => Some(*session),
            _ => None,
        });
        assert!(
            new_session.is_some(),
            "successful prior recoveries must not consume the failure budget; recovery {recovery}"
        );
        h.session = new_session.unwrap();
        let baseline = h.event(Event::Baseline {
            support: support(),
            revision: EvidenceKey(1),
            facts: baseline(),
            resumed: true,
        });
        h.idle(&baseline);
        let resumed = h.command(Command::Resume);
        assert_eq!(resumed.snapshot.phase, Phase::Running);
    }
}

#[test]
fn review_quantity_before_put_cannot_leave_a_known_empty_bag() {
    let mut h = Harness::new(true);
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    h.engine.handle(Input::Observed {
        at: At(30),
        observation: Observation {
            session: h.session,
            event: Event::Facts {
                revision: EvidenceKey(3),
                ingress: IngressMark(30),
                facts: vec![Fact::Inventory(Inventory::Quantity(StackKey(7), 2))],
            },
        },
    });
    let late_put = h.engine.handle(Input::Observed {
        at: At(20),
        observation: Observation {
            session: h.session,
            event: Event::Facts {
                revision: EvidenceKey(2),
                ingress: IngressMark(20),
                facts: vec![Fact::Inventory(Inventory::Put(Stack {
                    key: StackKey(7),
                    kind: ItemKind(7),
                    quantity: 1,
                }))],
            },
        },
    });
    let reconstructed = late_put
        .snapshot
        .inventory
        .iter()
        .any(|stack| stack.key == StackKey(7) && stack.quantity == 2);
    assert!(
        reconstructed,
        "the delayed identity must reconstruct the newer quantity"
    );
    assert_eq!(
        late_put.snapshot.own.inventory,
        InventoryKnowledge::Observed
    );
}

#[test]
fn review_confirmed_then_rejected_must_quarantine_the_action() {
    let mut h = Harness::new(false);
    let id = h.start_attack();
    h.sent(id);
    let confirmed = h.facts(vec![Fact::Outcome(Outcome::OwnHit(EntityKey(1)))]);
    h.facts(vec![Fact::Outcome(Outcome::Rejected {
        subject: Subject::Target(EntityKey(1)),
        reason: Rejection::Other,
    })]);
    let idle = h.idle(&confirmed);
    let actions: Vec<_> = acts(&idle).iter().map(|ticket| ticket.action).collect();
    assert_eq!(
        idle.snapshot.phase,
        Phase::Paused(Pause::Uncertain),
        "contradictory evidence schedules {actions:?}"
    );
    assert!(
        actions.is_empty(),
        "contradictory evidence must not schedule the next attack"
    );
}

#[test]
fn review_paused_snapshot_does_not_advertise_an_ignored_pause() {
    let mut h = Harness::new(false);
    h.command(Command::Start);
    let paused = h.command(Command::Pause);
    assert_eq!(paused.snapshot.phase, Phase::Paused(Pause::User));
    assert!(!paused.snapshot.allowed.contains(&Control::Pause));
}

#[test]
fn review_old_map_entity_is_not_eligible_in_the_new_map() {
    let mut h = Harness::new(false);
    h.engine.handle(Input::Observed {
        at: At(100),
        observation: Observation {
            session: h.session,
            event: Event::Facts {
                revision: EvidenceKey(3),
                ingress: IngressMark(100),
                facts: vec![Fact::Position {
                    map: MapKey(2),
                    point: center(),
                }],
            },
        },
    });
    let mut draft = setup(false, None).draft().clone();
    draft.boundary = Boundary::checked(MapKey(2), center(), NonZeroU16::new(10).unwrap())
        .unwrap_or_else(|_| panic!("area"));
    h.now = 100;
    h.command(Command::Configure(
        Setup::checked(draft).unwrap_or_else(|_| panic!("setup")),
    ));
    h.engine.handle(Input::Observed {
        at: At(90),
        observation: Observation {
            session: h.session,
            event: Event::Facts {
                revision: EvidenceKey(2),
                ingress: IngressMark(90),
                facts: vec![
                    entity(7, center(), Some(false)),
                    Fact::Drop(Drop {
                        key: DropKey(7),
                        kind: ItemKind(7),
                        position: center(),
                        quantity: 1,
                    }),
                ],
            },
        },
    });
    let started = h.command(Command::Start);
    let actions: Vec<_> = acts(&started).iter().map(|ticket| ticket.action).collect();
    assert!(
        actions.is_empty(),
        "map-1 observation must not authorize an attack in map 2: {actions:?}"
    );
}

#[test]
fn inventory_snapshot_rows_never_clear_or_grant() {
    let mut facts = baseline();
    facts.push(Fact::Inventory(Inventory::Put(Stack {
        key: StackKey(7),
        kind: ItemKind(7),
        quantity: 1,
    })));
    facts.push(Fact::Inventory(Inventory::Put(Stack {
        key: StackKey(8),
        kind: ItemKind(8),
        quantity: 3,
    })));
    let mut h = Harness::with(
        true,
        support(),
        facts,
        Some(Goal {
            item: ItemKind(7),
            gained: 1,
        }),
    );
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    let snapshot = h.facts(vec![Fact::InventorySnapshot(vec![Stack {
        key: StackKey(7),
        kind: ItemKind(7),
        quantity: 9,
    }])]);
    assert_eq!(snapshot.snapshot.totals.goal_gained, 0);
    assert_eq!(snapshot.snapshot.inventory.len(), 2);
    let delta = h.facts(vec![Fact::Inventory(Inventory::Quantity(StackKey(7), 10))]);
    assert_eq!(delta.snapshot.totals.goal_gained, 1);
    assert_eq!(delta.snapshot.phase, Phase::Pausing);
}

#[test]
fn settlement_metadata_does_not_refresh_state() {
    let mut h = Harness::new(false);
    let original = h
        .facts(vec![Fact::InventorySnapshot(vec![])])
        .snapshot
        .own
        .last_state_at;
    h.wake(20_000);
    let metadata = h.event(Event::Facts {
        revision: EvidenceKey(900),
        ingress: IngressMark(900),
        facts: vec![Fact::InventorySettled(IngressMark(900))],
    });
    assert_eq!(metadata.snapshot.own.last_state_at, original);
    assert!(
        metadata
            .snapshot
            .blockers
            .contains(&Blocker::NeedsFreshObservation(StateKind::WorldState))
    );
}

#[test]
fn inventory_mutation_behind_a_proven_frontier_invalidates_knowledge() {
    let mut h = Harness::new(false);
    h.facts(vec![Fact::Inventory(Inventory::Put(Stack {
        key: StackKey(7),
        kind: ItemKind(7),
        quantity: 2,
    }))]);
    let late = h.event(Event::Facts {
        revision: EvidenceKey(900),
        ingress: IngressMark(5),
        facts: vec![Fact::Inventory(Inventory::Quantity(StackKey(7), 99))],
    });
    assert_eq!(late.snapshot.own.inventory, InventoryKnowledge::Unknown);
    assert_eq!(late.snapshot.inventory[0].quantity, 2);
    assert!(!late.snapshot.allowed.contains(&Control::Start));
}

#[test]
fn inventory_gap_waits_before_the_next_cycle() {
    let mut facts = baseline();
    facts.push(Fact::Inventory(Inventory::Put(Stack {
        key: StackKey(7),
        kind: ItemKind(7),
        quantity: 1,
    })));
    let mut h = Harness::with(
        true,
        support(),
        facts,
        Some(Goal {
            item: ItemKind(7),
            gained: 10,
        }),
    );
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    h.event(Event::Facts {
        revision: EvidenceKey(3),
        ingress: IngressMark(30),
        facts: vec![Fact::Inventory(Inventory::Quantity(StackKey(7), 2))],
    });
    let result = h.event(Event::Facts {
        revision: EvidenceKey(2),
        ingress: IngressMark(20),
        facts: vec![
            Fact::Outcome(Outcome::DigEnded(DigEnd::Empty)),
            Fact::InventorySettled(IngressMark(20)),
        ],
    });
    let waiting = h.idle(&result);
    assert_eq!(waiting.snapshot.phase, Phase::Running);
    assert!(acts(&waiting).is_empty());
    assert!(
        waiting
            .snapshot
            .blockers
            .contains(&Blocker::NeedsObservation(StateKind::Inventory))
    );
    let settled = h.event(Event::Facts {
        revision: EvidenceKey(4),
        ingress: IngressMark(40),
        facts: vec![Fact::InventorySettled(IngressMark(30))],
    });
    assert_eq!(settled.snapshot.totals.goal_gained, 1);
    assert_eq!(acts(&settled).len(), 1);
}

#[test]
fn waiting_for_goal_inventory_keeps_verified_potion_use_available() {
    let mut facts = baseline();
    facts.extend([
        Fact::Inventory(Inventory::Put(Stack {
            key: StackKey(7),
            kind: ItemKind(7),
            quantity: 2,
        })),
        Fact::HpPotionStack(Some(StackKey(7))),
    ]);
    let mut capabilities = support();
    capabilities.hp_potion = true;
    let mut h = Harness::with(
        true,
        capabilities,
        facts,
        Some(Goal {
            item: ItemKind(9),
            gained: 10,
        }),
    );
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    h.event(Event::Facts {
        revision: EvidenceKey(3),
        ingress: IngressMark(30),
        facts: vec![Fact::Inventory(Inventory::Put(Stack {
            key: StackKey(9),
            kind: ItemKind(9),
            quantity: 1,
        }))],
    });
    let result = h.event(Event::Facts {
        revision: EvidenceKey(2),
        ingress: IngressMark(20),
        facts: vec![
            Fact::Hp(gauge(20, 100)),
            Fact::Outcome(Outcome::DigEnded(DigEnd::Empty)),
            Fact::InventorySettled(IngressMark(20)),
        ],
    });
    let heal = h.idle(&result);
    assert_eq!(acts(&heal).len(), 1);
    assert_eq!(acts(&heal)[0].action, Action::HpPotion(StackKey(7)));
    assert_eq!(heal.snapshot.totals.goal_gained, 0);
}

#[test]
fn stop_on_connection_loss_waits_for_resource_closure() {
    let mut h = Harness::new(true);
    h.command(Command::Start);
    h.command(Command::Stop);
    let lost = h.event(Event::Lost);
    assert_eq!(lost.snapshot.phase, Phase::Closing);
    assert!(h.engine.run_key().is_some());
    assert_eq!(closes(&lost), 1);
    let closed = h.event(Event::Closed);
    assert_eq!(closed.snapshot.phase, Phase::Offline);
    assert!(h.engine.run_key().is_none());
}

#[test]
fn conflicting_completed_dig_results_pause_without_another_cycle() {
    let mut h = Harness::new(true);
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    let first = h.facts(vec![Fact::Outcome(Outcome::DigEnded(DigEnd::Empty))]);
    h.facts(vec![Fact::Outcome(Outcome::DigEnded(
        DigEnd::ItemReported(ItemKind(7)),
    ))]);
    let paused = h.idle(&first);
    assert_eq!(paused.snapshot.phase, Phase::Paused(Pause::Uncertain));
    assert_eq!(paused.snapshot.totals.digs, 1);
    assert!(acts(&paused).is_empty());
}

#[test]
fn excessive_unsettled_inventory_invalidates_knowledge() {
    let mut h = Harness::new(false);
    for mark in 100..=356 {
        h.event(Event::Facts {
            revision: EvidenceKey(mark),
            ingress: IngressMark(mark),
            facts: vec![Fact::Inventory(Inventory::Put(Stack {
                key: StackKey(7),
                kind: ItemKind(7),
                quantity: mark as u32,
            }))],
        });
    }
    let snapshot = h.engine.snapshot();
    assert_eq!(snapshot.own.inventory, InventoryKnowledge::Unknown);
    assert!(!snapshot.allowed.contains(&Control::Start));
}

#[test]
fn unsettled_inventory_ages_independently_of_other_state() {
    let mut h = Harness::new(true);
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    h.now = 20;
    h.event(Event::Facts {
        revision: EvidenceKey(2),
        ingress: IngressMark(20),
        facts: vec![Fact::Inventory(Inventory::Put(Stack {
            key: StackKey(7),
            kind: ItemKind(7),
            quantity: 1,
        }))],
    });
    h.now = 14_000;
    let fresh_hp = h.event(Event::Facts {
        revision: EvidenceKey(3),
        ingress: IngressMark(30),
        facts: vec![Fact::Hp(gauge(100, 100))],
    });
    assert_eq!(fresh_hp.wake_at, Some(At(15_021)));
    h.now = 15_021;
    let expired = h.wake(0);
    assert!(
        expired
            .snapshot
            .blockers
            .contains(&Blocker::NeedsFreshObservation(StateKind::Inventory))
    );
    assert_eq!(expired.snapshot.phase, Phase::Pausing);
}

#[test]
fn review_delayed_pre_send_inventory_is_not_post_send_goal_credit() {
    let mut facts = baseline();
    facts.push(Fact::Inventory(Inventory::Put(Stack {
        key: StackKey(7),
        kind: ItemKind(7),
        quantity: 1,
    })));
    let mut h = Harness::with(
        true,
        support(),
        facts,
        Some(Goal {
            item: ItemKind(7),
            gained: 1,
        }),
    );
    let start = h.command(Command::Start);
    let id = acts(&start)[0].id;
    h.engine.handle(Input::Observed {
        at: At(10),
        observation: Observation {
            session: h.session,
            event: Event::Receipt {
                action: id,
                delivery: Delivery::Sent(IngressMark(10)),
            },
        },
    });
    let waiting = h.engine.handle(Input::Observed {
        at: At(20),
        observation: Observation {
            session: h.session,
            event: Event::Facts {
                revision: EvidenceKey(3),
                ingress: IngressMark(20),
                facts: vec![Fact::Inventory(Inventory::Quantity(StackKey(7), 3))],
            },
        },
    });
    assert_eq!(waiting.snapshot.totals.goal_gained, 0);
    assert_eq!(waiting.snapshot.phase, Phase::Running);
    let late = h.engine.handle(Input::Observed {
        at: At(5),
        observation: Observation {
            session: h.session,
            event: Event::Facts {
                revision: EvidenceKey(2),
                ingress: IngressMark(5),
                facts: vec![Fact::Inventory(Inventory::Quantity(StackKey(7), 2))],
            },
        },
    });
    assert_eq!(
        late.snapshot
            .inventory
            .iter()
            .find(|stack| stack.key == StackKey(7))
            .unwrap()
            .quantity,
        3
    );
    assert_eq!(late.snapshot.totals.goal_gained, 0);
    let settled = h.event(Event::Facts {
        revision: EvidenceKey(4),
        ingress: IngressMark(30),
        facts: vec![Fact::InventorySettled(IngressMark(20))],
    });
    assert_eq!(settled.snapshot.phase, Phase::Pausing);
    assert_eq!(
        settled.snapshot.totals.goal_gained, 1,
        "only the change from 2 before send to 3 after send belongs to the run goal"
    );
}

fn stack(key: u64, kind: u64, quantity: u32) -> Stack {
    Stack {
        key: StackKey(key),
        kind: ItemKind(kind),
        quantity,
    }
}

fn goal_harness(stacks: Vec<Stack>, goal: u32) -> Harness {
    let mut facts = baseline();
    facts.push(Fact::Inventory(Inventory::Reset(stacks)));
    Harness::with(
        true,
        support(),
        facts,
        Some(Goal {
            item: ItemKind(7),
            gained: goal,
        }),
    )
}

fn raw(h: &mut Harness, mark: u64, facts: Vec<Fact>) -> Effects {
    h.revision += 1;
    h.now = h.now.max(mark);
    h.engine.handle(Input::Observed {
        at: At(mark),
        observation: Observation {
            session: h.session,
            event: Event::Facts {
                revision: EvidenceKey(h.revision),
                ingress: IngressMark(mark),
                facts,
            },
        },
    })
}

#[test]
fn same_source_stack_transfer_has_zero_goal_gain() {
    let mut h = goal_harness(vec![stack(7, 7, 2), stack(8, 7, 1)], 1);
    let start = h.command(Command::Start);
    h.sent(acts(&start)[0].id);
    let transferred = h.facts(vec![
        Fact::Inventory(Inventory::Quantity(StackKey(7), 1)),
        Fact::Inventory(Inventory::Quantity(StackKey(8), 2)),
    ]);
    let quantity: u32 = transferred
        .snapshot
        .inventory
        .iter()
        .filter(|s| s.kind == ItemKind(7))
        .map(|s| s.quantity)
        .sum();
    assert_eq!(quantity, 3);
    assert_eq!(
        transferred.snapshot.totals.goal_gained, 0,
        "moving existing quantity between stacks must not satisfy a gain goal; phase {:?}",
        transferred.snapshot.phase
    );
}

#[test]
fn stop_repeat_during_timeout_closure_keeps_run_until_closed() {
    let mut h = Harness::new(true);
    h.command(Command::Start);
    let run = h.engine.run_key();
    h.command(Command::Stop);
    let closing = h.wake(2_000);
    assert_eq!(closing.snapshot.phase, Phase::Closing);
    assert_eq!(closes(&closing), 1);
    assert_eq!(h.engine.run_key(), run);
    let repeat = h.command(Command::Stop);
    assert_eq!(repeat.snapshot.phase, Phase::Closing);
    assert_eq!(
        h.engine.run_key(),
        run,
        "repeated Stop must not acknowledge resource closure"
    );
    h.event(Event::Closed);
    assert!(h.engine.run_key().is_none());
}

#[test]
fn reused_stack_does_not_inherit_potion_verification() {
    for replacement in [7, 8] {
        let mut facts = baseline();
        facts.extend([
            Fact::Inventory(Inventory::Put(stack(7, 7, 2))),
            Fact::HpPotionStack(Some(StackKey(7))),
        ]);
        let mut capabilities = support();
        capabilities.hp_potion = true;
        let mut h = Harness::with(
            true,
            capabilities,
            facts,
            Some(Goal {
                item: ItemKind(9),
                gained: 1,
            }),
        );
        let start = h.command(Command::Start);
        h.sent(acts(&start)[0].id);
        let changed = h.facts(vec![
            Fact::Inventory(Inventory::Remove(StackKey(7))),
            Fact::Inventory(Inventory::Put(stack(7, replacement, 1))),
            Fact::Hp(gauge(20, 100)),
            Fact::Outcome(Outcome::DigEnded(DigEnd::Empty)),
        ]);
        let next = h.idle(&changed);
        assert!(
            !acts(&next)
                .iter()
                .any(|t| matches!(t.action, Action::HpPotion(_))),
            "replacement stack was never verified as a potion"
        );
    }
}

#[test]
fn pending_replacement_does_not_inherit_potion_verification() {
    for replacement in [7, 8] {
        let mut facts = baseline();
        facts.extend([
            Fact::Inventory(Inventory::Put(stack(7, 7, 2))),
            Fact::HpPotionStack(Some(StackKey(7))),
        ]);
        let mut capabilities = support();
        capabilities.hp_potion = true;
        let mut h = Harness::with(
            true,
            capabilities,
            facts,
            Some(Goal {
                item: ItemKind(9),
                gained: 1,
            }),
        );
        let start = h.command(Command::Start);
        h.sent(acts(&start)[0].id);
        raw(
            &mut h,
            30,
            vec![
                Fact::Inventory(Inventory::Remove(StackKey(7))),
                Fact::Inventory(Inventory::Put(stack(7, replacement, 1))),
            ],
        );
        let confirmed = raw(
            &mut h,
            20,
            vec![
                Fact::Hp(gauge(20, 100)),
                Fact::Outcome(Outcome::DigEnded(DigEnd::Empty)),
                Fact::InventorySettled(IngressMark(20)),
            ],
        );
        let next = h.idle(&confirmed);
        assert!(
            next.snapshot
                .blockers
                .contains(&Blocker::NeedsObservation(StateKind::Inventory))
        );
        assert!(
            !acts(&next)
                .iter()
                .any(|t| matches!(t.action, Action::HpPotion(_))),
            "unsettled replacement kind must not inherit old potion verification"
        );
    }
}

#[test]
fn unknown_potion_proof_needs_a_fresh_verified_identity() {
    let mut facts = baseline();
    facts.extend([
        Fact::Hp(gauge(20, 100)),
        Fact::HpPotionStack(Some(StackKey(7))),
    ]);
    let mut capabilities = support();
    capabilities.hp_potion = true;
    let mut h = Harness::with(false, capabilities, facts, None);
    h.facts(vec![Fact::Inventory(Inventory::Put(stack(7, 81, 2)))]);
    assert!(acts(&h.command(Command::Start)).is_empty());
    h.facts(vec![Fact::HpPotionStack(Some(StackKey(7)))]);
    assert_eq!(
        acts(&h.command(Command::Start))[0].action,
        Action::HpPotion(StackKey(7))
    );
}

#[test]
fn replacement_can_use_a_new_potion_proof_in_the_same_event() {
    let mut facts = baseline();
    facts.extend([
        Fact::Inventory(Inventory::Put(stack(7, 81, 2))),
        Fact::HpPotionStack(Some(StackKey(7))),
    ]);
    let mut capabilities = support();
    capabilities.hp_potion = true;
    let mut h = Harness::with(false, capabilities, facts, None);
    h.facts(vec![
        Fact::Inventory(Inventory::Remove(StackKey(7))),
        Fact::Inventory(Inventory::Put(stack(7, 82, 1))),
        Fact::HpPotionStack(Some(StackKey(7))),
        Fact::Hp(gauge(20, 100)),
    ]);
    assert_eq!(
        acts(&h.command(Command::Start))[0].action,
        Action::HpPotion(StackKey(7))
    );
}
