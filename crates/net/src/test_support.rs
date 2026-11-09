use opentrickster_engine::{self as e, Action, ActionTicket, At, SessionKey};
use std::num::{NonZeroU16, NonZeroU32};
pub(crate) fn ticket(action: Action) -> (SessionKey, ActionTicket) {
    let (session, ticket, _) = engine_ticket(action);
    (session, ticket)
}
/// An engine-minted ticket and the barrier key of the drain that preceded it.
pub(crate) fn engine_ticket(action: Action) -> (SessionKey, ActionTicket, Option<e::BarrierKey>) {
    let mut engine = e::Engine::new(e::Mode::Assisted);
    let effects = engine.handle(e::Input::User {
        at: At(0),
        request: e::RequestKey(1),
        command: e::Command::Open,
    });
    let session = effects
        .io
        .iter()
        .find_map(|e| {
            if let e::Effect::Open { session, .. } = e {
                Some(*session)
            } else {
                None
            }
        })
        .expect("engine session");
    let label = || e::DisplayText::checked("Synthetic".into()).unwrap();
    let center = e::Point { x: 500, y: 300 };
    let setup = e::Setup::checked(e::SetupDraft {
        activity: e::Activity::Combat {
            targets: e::Targets::Only(vec![e::SpeciesKey(1)]),
        },
        boundary: e::Boundary::checked(e::MapKey(1), center, NonZeroU16::new(100).unwrap())
            .unwrap(),
        hp_at: e::Percent::checked(30).unwrap(),
        weight_limit: e::Percent::checked(90).unwrap(),
        goal: None,
    })
    .unwrap_or_else(|_| panic!("fixture setup"));
    engine.handle(e::Input::User {
        at: At(1),
        request: e::RequestKey(2),
        command: e::Command::Configure(setup),
    });
    let gauge = e::Gauge {
        current: 100,
        maximum: NonZeroU32::new(100),
    };
    let effects = engine.handle(e::Input::Observed {
        at: At(2),
        observation: e::Observation {
            session,
            event: e::Event::Baseline {
                support: e::Support {
                    build: label(),
                    move_to: true,
                    basic_attack: true,
                    dig_cycle: true,
                    pickup: true,
                    hp_potion: false,
                    quiesce: true,
                },
                revision: e::EvidenceKey(1),
                resumed: false,
                facts: vec![
                    e::Fact::Position {
                        map: e::MapKey(1),
                        point: center,
                    },
                    e::Fact::Hp(gauge.clone()),
                    e::Fact::Weight(e::Gauge {
                        current: 10,
                        maximum: NonZeroU32::new(100),
                    }),
                    e::Fact::HpPotions(5),
                    e::Fact::Inventory(e::Inventory::Reset(vec![])),
                    e::Fact::Drill(e::Drill::Equipped {
                        life: 100,
                        depth: 10,
                    }),
                    e::Fact::InventorySettled(e::IngressMark(0)),
                    e::Fact::Entity(e::Entity {
                        key: e::EntityKey(1),
                        species: e::SpeciesKey(1),
                        label: label(),
                        position: center,
                        alive: true,
                        hp: Some(gauge),
                        engaged_by_other: Some(false),
                    }),
                ],
            },
        },
    });
    let barrier = effects.io.iter().find_map(|effect| match effect {
        e::Effect::Session {
            op: e::SessionOp::Quiesce(k),
            ..
        } => Some(*k),
        _ => None,
    });
    if let Some(key) = barrier {
        engine.handle(e::Input::Observed {
            at: At(3),
            observation: e::Observation {
                session,
                event: e::Event::Quiescent(key),
            },
        });
    }
    let effects = engine.handle(e::Input::User {
        at: At(4),
        request: e::RequestKey(3),
        command: e::Command::Start,
    });
    let mut ticket = effects
        .io
        .into_iter()
        .find_map(|effect| match effect {
            e::Effect::Session {
                op: e::SessionOp::Act(t),
                ..
            } => Some(t),
            _ => None,
        })
        .expect("engine action");
    ticket.action = action;
    ticket.not_after = At(1000);
    (session, ticket, barrier)
}

/// Synthetic item definitions, never public catalog rows.
#[derive(Clone, Copy)]
pub(crate) enum Def {
    Plain { weight: i64 },
    HpPotion { weight: i64 },
    ExpiringHpPotion { weight: i64 },
}
pub(crate) fn definitions(
    rows: &[(u32, Def)],
) -> std::sync::Arc<crate::definitions::GameDefinitions> {
    use crate::definitions::{GameDefinitions, ItemInfo};
    use std::collections::BTreeMap;
    let info = |def: Def| {
        let (weight, potion, exist_type) = match def {
            Def::Plain { weight } => (weight, false, 0),
            Def::HpPotion { weight } => (weight, true, 0),
            Def::ExpiringHpPotion { weight } => (weight, true, 1),
        };
        ItemInfo {
            weight,
            class: 0,
            kind: 1,
            subtype: 0,
            flags: if potion { 2 } else { 0 },
            flags_ex: 0,
            min_level: 0,
            hp: if potion { 50 } else { 0 },
            mp: 0,
            hp_rate: 0.0,
            mp_rate: 0.0,
            delay: 0.0,
            effect: 0,
            exist_type,
            life: 0,
            ftype: 0,
            stat_plus: Some(vec![0.0; 12]),
        }
    };
    std::sync::Arc::new(GameDefinitions {
        item_types: rows.iter().map(|(id, _)| (*id, (1, 0))).collect(),
        items: rows.iter().map(|(id, def)| (*id, info(*def))).collect(),
        labels: BTreeMap::new(),
        drills: BTreeMap::new(),
        drill_meta: BTreeMap::new(),
        gun_params: BTreeMap::new(),
        gun_slots: vec![1],
        zones: BTreeMap::new(),
        zone_labels: BTreeMap::new(),
        normal_form_effects: Vec::new(),
        player_levels: Vec::new(),
        set_bonuses: Some(BTreeMap::new()),
    })
}

pub(crate) fn sample_value() -> serde_json::Value {
    static VALUE: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    VALUE
        .get_or_init(|| {
            let output = std::process::Command::new("node")
                .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sampler.cjs"))
                .arg("--snapshot")
                .output()
                .unwrap();
            assert!(output.status.success());
            serde_json::from_slice(&output.stdout).unwrap()
        })
        .clone()
}
