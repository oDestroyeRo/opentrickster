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

/// A synthetic game session that exercises every input of derived maxima, with invented
/// IDs and values only.
pub(crate) struct MaximaSession {
    pub(crate) definitions: std::sync::Arc<crate::definitions::GameDefinitions>,
    /// Inbound frames in raw order as `(opcode, aux, payload)`.
    pub(crate) frames: Vec<(u16, u16, Vec<u8>)>,
    /// Index of the 233 at H, the last StartSet frame.
    pub(crate) h: usize,
}
pub(crate) fn maxima_session() -> MaximaSession {
    const SET: u32 = 7;
    const OWN: u16 = 321;
    // Kind 2 rows worn in slots 0 and 5, a Kind 1 stack and the StartSet 518's row.
    let [armor, ring, stack, special] = [1001u64, 1002, 1003, 1004];
    let mut definitions = definitions(&[
        (100, Def::Plain { weight: 3 }),
        (101, Def::Plain { weight: 1 }),
        (102, Def::HpPotion { weight: 1 }),
        (103, Def::Plain { weight: 2 }),
    ]);
    let data = std::sync::Arc::get_mut(&mut definitions).unwrap();
    // Each level row differs, so every stat block moves the maxima.
    data.player_levels = (0..crate::resources::PLAYER_LEVELS)
        .map(|i| 1000.0 + (i / 36 * 10 + i / 3 % 12) as f64)
        .collect();
    for (id, values) in [
        (100, [(9, 50.0), (3, 20.0), (6, 7.0)]),
        (101, [(9, 30.0), (3, 0.0), (6, 2.0)]),
    ] {
        let list = data.items.get_mut(&id).unwrap().stat_plus.as_mut().unwrap();
        for (stat, value) in values {
            list[stat] = value;
        }
    }
    let mut bonus = [0; 12];
    bonus[9] = 100;
    bonus[3] = 10;
    bonus[6] = 5;
    data.set_bonuses = Some([(i64::from(SET), bonus)].into());

    let body = |kind: u8, id: u32, uid: u64, options: Option<&[[u32; 3]]>| {
        let mut b = id.to_le_bytes().to_vec();
        b.extend(uid.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        if kind != 1 {
            b.extend([0; 4]);
        }
        if let Some(options) = options {
            b.push(options.len() as u8);
            options
                .iter()
                .flatten()
                .for_each(|v| b.extend(v.to_le_bytes()));
        }
        b
    };
    // An option whose property is HP (10009), with its value in the high half.
    let hp_option = |value: u32| [[5, value << 16 | 10009, 0]];
    let indexed = |stat: u8, value: i32| {
        let mut b = vec![1, stat];
        b.extend(value.to_le_bytes());
        b
    };
    let block = |offset: usize, points: u16| {
        let mut b = vec![0; offset + 33];
        for s in 0..12 {
            b[offset + 2 * s..offset + 2 * s + 2].copy_from_slice(&points.to_le_bytes());
        }
        b
    };
    let aura = |hp: i32| {
        let mut b = vec![0; 60];
        b[12 + 4 * 9..12 + 4 * 10].copy_from_slice(&hp.to_le_bytes());
        b[12 + 4 * 3..12 + 4 * 4].copy_from_slice(&5i32.to_le_bytes());
        b
    };
    let set_item = |complete: u8| {
        let mut b = u32::from(OWN).to_le_bytes().to_vec();
        b.extend(SET.to_le_bytes());
        b.push(complete);
        b
    };
    let slot = |uid: u64, slot: u8| {
        let mut b = uid.to_le_bytes().to_vec();
        b.push(slot);
        b
    };
    let mut profile = vec![0; 69];
    profile[17] = b'S';
    let mut clock = vec![0; 10];
    clock[4..6].copy_from_slice(&OWN.to_le_bytes());
    let mut list = 4u16.to_le_bytes().to_vec();
    for (kind, id, uid, options) in [
        (2, 100, armor, &hp_option(70)[..]),
        (2, 101, ring, &[][..]),
        (1, 102, stack, &[][..]),
        (2, 103, special, &[][..]),
    ] {
        list.push(kind);
        list.extend(body(kind, id, uid, (kind == 2).then_some(options)));
    }
    let mut stats = block(184, 4);
    stats.resize(251, 0);
    stats[..8].copy_from_slice(&armor.to_le_bytes());
    stats[40..48].copy_from_slice(&ring.to_le_bytes());
    stats[235..239].copy_from_slice(&500u32.to_le_bytes());
    stats[239..243].copy_from_slice(&300u32.to_le_bytes());
    let mut changed = vec![2, 2];
    changed.extend(body(2, 100, armor, Some(&hp_option(90))));
    let mut imprint = vec![0; 20];
    imprint.extend([2, 1]);
    imprint.extend(body(1, 102, stack, None));
    let mut points = vec![0; 2];
    points.extend(&block(0, 20)[..24]);
    let mut listed = vec![1];
    listed.extend([0; 24]);
    let frames = vec![
        (5, 0, vec![0; 4]),
        (679, 0, profile),
        (1070, 0, aura(15)),
        (231, 0, list),
        (518, 0, special.to_le_bytes().to_vec()),
        (230, 0, clock),
        (233, 0, stats),
        // Live from here. The login soul frames arrive right after H and keep the maxima.
        (1489, 0, vec![0; 6]),
        (1487, 0, vec![0; 38]),
        // The page drops 999.
        (999, 0, vec![1; 8]),
        (784, 0, indexed(9, 2000)),
        (283, 0, indexed(9, 40)),
        // Its recompute would clear the pending override, which Net cannot prove ran.
        (1386, 0, vec![0]),
        (109, 0, points),
        (629, 0, block(0, 8)),
        (634, 0, block(0, 12).into_iter().chain([0; 4]).collect()),
        (649, 0, block(69, 16)),
        (1070, 0, aura(25)),
        (1065, 0, vec![0; 8]),
        (1071, 0, vec![0; 4]),
        (837, 0, set_item(1)),
        (837, 5, set_item(0)),
        (1951, 5, Vec::new()),
        (85, 0, slot(ring, 5)),
        (82, 0, slot(ring, 5)),
        (561, 0, changed),
        (82, 0, slot(armor, 0)),
        (1386, 0, listed),
        (1386, 0, vec![0]),
        (1387, 0, imprint),
        (1386, 0, vec![0]),
        (1389, 0, vec![0; 4]),
        (282, 0, indexed(9, 777)),
        (283, 0, vec![0]),
        // A trade frame: `GameParts` is unknown for the rest of the session.
        (94, 0, vec![0; 12]),
        (1489, 0, vec![0; 6]),
        (1487, 0, vec![0; 38]),
    ];
    MaximaSession {
        definitions,
        frames,
        h: 6,
    }
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
