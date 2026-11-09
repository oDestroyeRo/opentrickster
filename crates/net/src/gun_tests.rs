//! Synthetic gun fixtures and decoder-level tests of the native gun controller.
//! Every ID, key, profile and count here is invented; none is a public row or account value.

use crate::{
    codec::Packet,
    decode::{Decoder, MissingPrerequisite as M},
    definitions::{GameDefinitions, GunParams, ItemInfo},
    gun::{HIT_SKILL, MOVE_SKILL},
    wire::Writer,
};
use opentrickster_engine::{self as e, Fact};
use std::{collections::BTreeMap, sync::Arc, time::Instant};
use zeroize::Zeroizing;

pub(crate) const OWN: u16 = 321;
pub(crate) const MONSTER: u16 = 9;
const OTHER: u16 = 322;
const ZONE: u16 = 40;
const TOWN: u16 = 41;
const GUN: u32 = 9001;
const AMMO: u32 = 9002;
const COAT: u32 = 9004;
const DRILL: u32 = 9005;
const GUN_UID: u64 = 0x100;
const AMMO_UID: u64 = 0x200;
const COAT_UID: u64 = 0x300;
const DRILL_UID: u64 = 0x400;
pub(crate) const KEY: u16 = 0x55aa;
/// GaugeSpeed 2000 reloads in 60 ms at speed 1; PowRange 1200 adds one Power per ms.
pub(crate) const PROFILE: GunParams = GunParams {
    kind: 1,
    speed: 2000,
    range: 1200,
    start: 600,
    min_range: 0,
    max_range: 8,
};
const RELOAD_MS: f64 = 60.0;

pub(crate) fn definitions() -> Arc<GameDefinitions> {
    let info = |kind, subtype, flags_ex| ItemInfo {
        weight: 1,
        class: 0,
        kind,
        subtype,
        flags: 0,
        flags_ex,
        min_level: 0,
        hp: 0,
        mp: 0,
        hp_rate: 0.0,
        mp_rate: 0.0,
        delay: 0.0,
        effect: 0,
        exist_type: 0,
        life: 0,
        ftype: 0,
        stat_plus: Some(vec![0.0; 12]),
    };
    let rows = [
        (GUN, info(2, 54, 0)),
        (AMMO, info(29, 1, 0)),
        (COAT, info(3, 0, 0)),
        (DRILL, info(6, 0, 0)),
    ];
    Arc::new(GameDefinitions {
        item_types: rows
            .iter()
            .map(|(id, info)| (*id, (info.kind, info.subtype)))
            .collect(),
        items: rows.into_iter().collect(),
        labels: BTreeMap::new(),
        drills: [(
            DRILL,
            crate::dig::Params {
                area: 40,
                speed: 100,
                mass: 10,
                limit: 100,
                nature: 0,
            },
        )]
        .into(),
        drill_meta: [(
            DRILL,
            crate::definitions::DrillMeta {
                grade: 1,
                maximum_life: 300,
            },
        )]
        .into(),
        gun_params: [(GUN, PROFILE)].into(),
        gun_slots: vec![4, 9, 1, 22],
        zones: [(ZONE, (2, 0)), (TOWN, (1, 0))].into(),
        zone_labels: BTreeMap::new(),
        normal_form_effects: Vec::new(),
        player_levels: Vec::new(),
        set_bonuses: Some(BTreeMap::new()),
    })
}

fn stats(zone: u16, slots: &[(usize, u64)]) -> Vec<u8> {
    let mut b = vec![0; 251];
    for (slot, uid) in slots {
        b[slot * 8..slot * 8 + 8].copy_from_slice(&uid.to_le_bytes());
    }
    b[219..221].copy_from_slice(&zone.to_le_bytes());
    // A living character: HP follows the zone, ten skipped bytes and Galder.
    b[235..239].copy_from_slice(&100u32.to_le_bytes());
    b
}
fn clock() -> Vec<u8> {
    let mut w = Writer::new();
    w.u32(1);
    w.u16(OWN);
    w.u16(100);
    w.u16(200);
    w.0.to_vec()
}
fn profile() -> Vec<u8> {
    let mut b = vec![0; 69];
    b[17] = b'S';
    b
}
/// One ItemBody: wire kind 2 carries state and, in lists, an empty option table.
fn row(w: &mut Writer, kind: u8, id: u32, uid: u64, count: u16, options: bool) {
    w.u8(kind);
    w.u32(id);
    w.u64(uid);
    w.u16(count);
    if kind == 2 {
        w.raw(&[0; 4]);
        if options {
            w.u8(0);
        }
    }
}
fn list(rows: &[(u8, u32, u64, u16)]) -> Vec<u8> {
    let mut w = Writer::new();
    w.u16(rows.len() as u16);
    for &(kind, id, uid, count) in rows {
        row(&mut w, kind, id, uid, count, true);
    }
    w.0.to_vec()
}
fn learned(skills: &[u16]) -> Vec<u8> {
    let mut w = Writer::new();
    w.u16(skills.len() as u16);
    for skill in skills {
        w.u16(*skill);
        w.u8(1);
        w.u16(0);
    }
    w.0.to_vec()
}
/// An appearance prefix through the label and the EsBlock with FastLoad entries.
fn actor(w: &mut Writer, id: u16, kind: u16, fast: &[(u16, f32)]) {
    w.u16(id);
    w.u16(kind);
    w.u16(0);
    w.raw(&[0; 8]);
    w.u16(100);
    w.u16(200);
    w.raw(&[0; 4]);
    w.cstr("Synthetic").unwrap();
    w.raw(&[0; 7]);
    if fast.is_empty() {
        w.u8(0);
    } else {
        w.u8(1);
        w.u8(fast.len() as u8);
        for (instance, value) in fast {
            w.u16(crate::gun::FAST_LOAD);
            w.u16(*instance);
            w.raw(&[0; 5]);
            w.u32(value.to_bits());
        }
        w.raw(&[0; 7]);
    }
    w.u32(0);
}
/// A complete type-1 appearance whose Worn A mask holds `worn` in slot 1.
fn player(id: u16, fast: &[(u16, f32)], worn: Option<u32>, attributes: u8) -> Vec<u8> {
    let mut w = Writer::new();
    actor(&mut w, id, 1, fast);
    w.u8(attributes);
    w.u32(0);
    w.u8(0);
    w.u32(if worn.is_some() { 2 } else { 0 });
    if let Some(id) = worn {
        w.u32(id);
    }
    w.u32(0);
    w.u8(0);
    w.cstr("").unwrap();
    w.cstr("").unwrap();
    w.raw(&[0; 12]);
    w.u16(0);
    w.raw(&[0; 11]);
    w.u8(0);
    w.0.to_vec()
}
fn monster() -> Vec<u8> {
    let mut w = Writer::new();
    actor(&mut w, MONSTER, 3, &[]);
    w.u16(0);
    w.0.to_vec()
}
fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
pub(crate) fn strike(attacker: u16, kind: u16, target: u16) -> Vec<u8> {
    words(&[attacker, kind, target, 1, 2, 3, 4])
}
pub(crate) fn damage(target: u16) -> Vec<u8> {
    let mut b = target.to_le_bytes().to_vec();
    b.extend(5i32.to_le_bytes());
    b.extend([0; 20]);
    b
}
/// A 224 start header: invoker, recipient, instance, ESA, V14, class, skill, V04, body.
fn effect_start(target: u16, instance: u16, esa: u16, value: f32) -> Vec<u8> {
    let mut b = words(&[OWN, target, instance, esa]);
    b.extend([0; 6]);
    b.extend(value.to_bits().to_le_bytes());
    b
}
pub(crate) fn walk(id: u16, to: (u16, u16)) -> Vec<u8> {
    let mut b = words(&[id, 100, 200, to.0, to.1]);
    b.push(1);
    b
}
fn stop(id: u16, at: (u16, u16)) -> Vec<u8> {
    let mut b = words(&[id, at.0, at.1]);
    b.push(1);
    b
}
fn slot(uid: u64, slot: u8) -> Vec<u8> {
    let mut b = uid.to_le_bytes().to_vec();
    b.push(slot);
    b
}

/// The StartSet a client replays before EnterWorld: a worn gun in slot 1, three ammo in
/// slot 20, an empty learned list and an explicitly empty own effect list. S233 comes last,
/// so a session's startup cutoff follows every other frame.
pub(crate) fn startup() -> Vec<(u16, Vec<u8>)> {
    vec![
        (679, profile()),
        (230, clock()),
        (
            231,
            list(&[
                (2, GUN, GUN_UID, 1),
                (1, AMMO, AMMO_UID, 3),
                (2, COAT, COAT_UID, 1),
            ]),
        ),
        (271, learned(&[])),
        (9, player(OWN, &[], Some(GUN), 0)),
        (7, monster()),
        (233, stats(ZONE, &[(1, GUN_UID), (20, AMMO_UID)])),
    ]
}

/// `startup` with a drill worn in slot 7 as well, for a native dig.
pub(crate) fn drill_startup() -> Vec<(u16, Vec<u8>)> {
    let mut frames = startup();
    for (opcode, payload) in &mut frames {
        match opcode {
            231 => {
                *payload = list(&[
                    (2, GUN, GUN_UID, 1),
                    (1, AMMO, AMMO_UID, 3),
                    (2, COAT, COAT_UID, 1),
                    (2, DRILL, DRILL_UID, 300),
                ])
            }
            233 => *payload = stats(ZONE, &[(1, GUN_UID), (7, DRILL_UID), (20, AMMO_UID)]),
            _ => {}
        }
    }
    frames
}

struct Run {
    d: Decoder,
    mark: u64,
    monster: Option<e::EntityKey>,
}
impl Run {
    fn new(definitions: bool) -> Self {
        let mut d = Decoder::new();
        if definitions {
            d.set_definitions(Some(self::definitions()));
        }
        d.use_native_walk_policy();
        Self {
            d,
            mark: 0,
            monster: None,
        }
    }
    fn started(packets: Vec<(u16, Vec<u8>)>) -> Self {
        let mut run = Self::new(true);
        for (opcode, payload) in packets {
            run.apply(opcode, payload);
        }
        run
    }
    /// EnterWorld, its one key request, and the answering 512.
    fn constructed(mut self) -> Self {
        self.d.construct_gun();
        assert_eq!(self.d.take_key_requests(), 1);
        self.apply(512, KEY.to_le_bytes().to_vec());
        self
    }
    fn armed() -> Self {
        let run = Self::started(startup()).constructed();
        assert_eq!(run.missing(), None);
        run
    }
    /// Armed, through state 1 and a full reload, then the tick that sets Ready.
    fn reloaded(packets: Vec<(u16, Vec<u8>)>) -> Self {
        let mut run = Self::started(packets).constructed();
        run.d.gun_tick(0.0);
        run.d.gun_tick(RELOAD_MS);
        run.d.gun_tick(1.0);
        assert!(run.ready());
        run
    }
    fn apply(&mut self, opcode: u16, payload: Vec<u8>) -> Vec<Fact> {
        self.bundle(opcode, 0, payload)
    }
    fn bundle(&mut self, opcode: u16, aux: u16, payload: Vec<u8>) -> Vec<Fact> {
        self.mark += 1;
        let facts = self
            .d
            .apply(&Packet {
                opcode,
                aux,
                mark: self.mark,
                received: Instant::now(),
                payload: Zeroizing::new(payload),
            })
            .unwrap();
        for fact in &facts {
            if let Fact::Entity(entity) = fact {
                self.monster = Some(entity.key);
            }
        }
        facts
    }
    fn missing(&self) -> Option<M> {
        self.d.gun_prerequisite()
    }
    fn key(&self) -> e::EntityKey {
        self.monster.unwrap()
    }
    fn ready(&self) -> bool {
        self.d.gun_ready(self.key())
    }
    fn waitable(&self) -> bool {
        self.d.gun_waitable(self.key())
    }
    /// Sends the shot as the session does: 477 takes the next mark.
    fn shoot(&mut self) -> Vec<u8> {
        let shot = self.d.gun_shot(self.key()).unwrap();
        self.mark += 1;
        self.d
            .expect_action(e::Action::BasicAttack(self.key()), self.mark, true)
            .unwrap();
        shot.0.to_vec()
    }
}
fn with(
    packets: Vec<(u16, Vec<u8>)>,
    opcode: u16,
    payload: Option<Vec<u8>>,
) -> Vec<(u16, Vec<u8>)> {
    let mut out: Vec<_> = packets
        .into_iter()
        .filter(|(op, _)| *op != opcode)
        .collect();
    if let Some(payload) = payload {
        out.push((opcode, payload));
    }
    out
}

#[test]
fn construction_requests_a_key_and_only_a_later_512_arms() {
    let mut run = Run::started(startup());
    // The listener exists only from EnterWorld; an earlier 512 is not replayed.
    run.apply(512, KEY.to_le_bytes().to_vec());
    assert_eq!(run.missing(), Some(M::GunKey));
    assert!(!run.d.basic_attack_supported());
    assert_eq!(run.d.take_key_requests(), 0);
    run.d.construct_gun();
    assert_eq!(run.d.take_key_requests(), 1);
    assert!(run.d.gun_awaits_key());
    assert_eq!(run.d.readiness.missing_gun, Some(M::GunKey));
    run.apply(512, KEY.to_le_bytes().to_vec());
    assert!(!run.d.gun_awaits_key());
    assert_eq!(run.missing(), None);
    assert_eq!(run.d.readiness.missing_gun, None);
    assert!(run.d.basic_attack_supported());
    run.d.construct_gun();
    assert_eq!(run.d.take_key_requests(), 0, "construction happens once");
}

#[test]
fn each_unknown_prerequisite_is_named_and_keeps_basic_attack_unsupported() {
    let base = startup;
    let cases: Vec<(&str, Run, M)> = vec![
        (
            "no stats",
            Run::started(with(base(), 233, None)).constructed_without_key(),
            M::OwnBaseline,
        ),
        (
            "no definitions",
            {
                let mut run = Run::new(false);
                for (opcode, payload) in base() {
                    run.apply(opcode, payload);
                }
                run.d.construct_gun();
                run
            },
            M::Definitions,
        ),
        (
            "slot references before the list",
            {
                let packets = base();
                let list = packets.iter().find(|(op, _)| *op == 231).unwrap().1.clone();
                // `with` moves the list after S233.
                Run::started(with(packets, 231, Some(list)))
            },
            M::Equipment,
        ),
        (
            "a gun outside slot 1",
            Run::started(with(
                base(),
                233,
                Some(stats(ZONE, &[(4, GUN_UID), (1, COAT_UID), (20, AMMO_UID)])),
            ))
            .constructed(),
            M::GunProfile,
        ),
        (
            "no ammo",
            Run::started(with(base(), 233, Some(stats(ZONE, &[(1, GUN_UID)])))).constructed(),
            M::GunAmmo,
        ),
        (
            "no learned list",
            Run::started(with(base(), 271, None)).constructed(),
            M::GunSkills,
        ),
        (
            "no own appearance",
            Run::started(with(base(), 9, None)).constructed(),
            M::GunEffectState,
        ),
        (
            "a hidden zone",
            Run::started(with(
                base(),
                233,
                Some(stats(TOWN, &[(1, GUN_UID), (20, AMMO_UID)])),
            ))
            .constructed(),
            M::GunZone,
        ),
        (
            "equipment changed before EnterWorld",
            {
                let mut packets = base();
                packets.push((82, slot(COAT_UID, 2)));
                Run::started(packets).constructed()
            },
            M::Equipment,
        ),
    ];
    for (name, run, missing) in cases {
        assert_eq!(run.missing(), Some(missing), "{name}");
        assert!(!run.d.basic_attack_supported(), "{name}");
    }
    // Without a 512 the key stays unknown.
    let mut run = Run::started(startup());
    run.d.construct_gun();
    assert_eq!(run.missing(), Some(M::GunKey));
    // Camp disarms until camp exit re-arms.
    let mut run = Run::armed();
    run.apply(377, vec![]);
    assert_eq!(run.missing(), Some(M::GunCamp));
    run.apply(380, vec![]);
    assert_eq!(run.missing(), None);
    // A new FastLoad on the own avatar makes LoadSpeed unknown.
    run.apply(224, effect_start(OWN, 5, crate::gun::FAST_LOAD, 50.0));
    assert_eq!(run.missing(), Some(M::GunEffectState));
    assert!(!run.d.basic_attack_supported());
    // An input that turns unknown after Ready blocks the shot before the next tick. A
    // foreign FastLoad leaves the controller Ready but its LoadSpeed unproven.
    let mut run = Run::reloaded(startup());
    run.apply(8, player(OTHER, &[(6, 20.0)], None, 0));
    assert_eq!(run.missing(), Some(M::GunEffectState));
    assert!(!run.ready());
    assert!(run.d.gun_shot(run.key()).is_err());
}

#[test]
fn without_a_gun_the_melee_rules_apply() {
    let run = Run::started(with(
        startup(),
        233,
        Some(stats(ZONE, &[(2, COAT_UID), (20, AMMO_UID)])),
    ))
    .constructed_without_key();
    assert_eq!(run.d.gun_worn(), Some(false));
    assert!(run.d.basic_attack_supported());
}

impl Run {
    fn constructed_without_key(mut self) -> Self {
        self.d.construct_gun();
        assert_eq!(self.d.take_key_requests(), 0);
        self
    }
}

#[test]
fn a_ticket_waits_for_ready_and_shoots_the_actual_inputs_once() {
    let mut run = Run::armed();
    assert!(run.waitable() && !run.ready());
    assert!(run.d.gun_shot(run.key()).is_err(), "never early");
    run.d.gun_tick(0.0);
    run.d.gun_tick(RELOAD_MS - 1.0);
    assert!(!run.ready());
    run.d.gun_tick(1.0);
    assert!(!run.ready(), "state 3 sets Ready on its own tick");
    run.d.gun_tick(1.0);
    assert!(run.ready());
    let shot = run.shoot();
    let crc = crate::actions::gun_crc(PROFILE.speed, PROFILE.range, PROFILE.start, KEY, false);
    assert_eq!(
        shot,
        crate::actions::gun_shot(MONSTER, crc, 1, 1).0.to_vec()
    );
    assert!(
        !run.waitable(),
        "the shot waits for own firing or a release"
    );
    assert!(run.d.gun_shot(run.key()).is_err());
    run.d.gun_tick(5000.0);
    assert!(!run.waitable(), "no local timeout or resend");
}

#[test]
fn own_firing_reloads_projects_ammo_and_attributes_one_outcome() {
    let mut run = Run::reloaded(startup());
    let key = run.key();
    run.shoot();
    // Another player's gun bundle neither fires this gun nor attributes.
    run.bundle(31, 6, strike(OTHER, 2, MONSTER));
    run.bundle(42, 6, damage(MONSTER));
    run.bundle(1951, 6, vec![]);
    assert!(run.d.take_correlated().is_empty());
    assert!(run.d.action_pending() && !run.waitable());
    run.bundle(31, 7, strike(OWN, 2, MONSTER));
    run.bundle(42, 7, damage(MONSTER));
    run.bundle(1951, 7, vec![]);
    let correlated = run.d.take_correlated();
    assert!(
        matches!(correlated.as_slice(), [c] if matches!(c.facts.as_slice(), [Fact::Outcome(e::Outcome::OwnHit(k))] if *k == key))
    );
    assert!(
        !run.d.action_pending(),
        "the outcome completes a shot without 32"
    );
    assert!(
        run.waitable() && !run.ready(),
        "own firing restarts the reload"
    );
    // Three loaded, one projected as consumed. The server's absolute count replaces the
    // projection instead of being decremented again.
    let mut w = Writer::new();
    w.u8(1);
    row(&mut w, 1, AMMO, AMMO_UID, 2, false);
    run.apply(561, w.0.to_vec());
    run.apply(31, strike(OWN, 2, MONSTER));
    assert_eq!(run.missing(), None);
    run.apply(31, strike(OWN, 2, MONSTER));
    assert_eq!(
        run.missing(),
        Some(M::GunAmmo),
        "2 - 1 projected, then exhausted"
    );
    assert!(!run.waitable());
}

#[test]
fn own_damage_restarts_reload_unless_learned() {
    let mut run = Run::reloaded(startup());
    run.apply(42, damage(OWN));
    assert!(run.waitable() && !run.ready());
    run.d.gun_tick(RELOAD_MS);
    run.d.gun_tick(1.0);
    assert!(run.ready());
    run.apply(42, damage(MONSTER));
    assert!(run.ready(), "damage to another actor is not own damage");
    run.apply(923, words(&[OWN, 0]));
    assert!(!run.ready());
    let mut run = Run::reloaded(with(startup(), 271, Some(learned(&[HIT_SKILL]))));
    run.apply(42, damage(OWN));
    run.apply(668, words(&[OWN, 0]));
    run.d.gun_tick(600.0);
    assert!(run.ready(), "3011 keeps Load and Power through own damage");
}

#[test]
fn walking_holds_the_reload_unless_learned() {
    let mut run = Run::armed();
    run.d.gun_tick(0.0);
    run.apply(24, walk(OWN, (110, 210)));
    run.d.gun_tick(RELOAD_MS * 3.0);
    run.d.gun_tick(1.0);
    assert!(!run.ready());
    run.apply(25, stop(OWN, (110, 210)));
    run.d.gun_tick(RELOAD_MS);
    run.d.gun_tick(1.0);
    assert!(run.ready());
    // An own move request counts as walking before its echo.
    let mut run = Run::armed();
    run.d.gun_tick(0.0);
    run.d.walking();
    run.d.gun_tick(RELOAD_MS * 3.0);
    assert!(!run.ready());
    let mut run = Run::started(with(startup(), 271, Some(learned(&[MOVE_SKILL])))).constructed();
    run.d.gun_tick(0.0);
    run.apply(24, walk(OWN, (110, 210)));
    run.d.gun_tick(RELOAD_MS);
    run.d.gun_tick(1.0);
    assert!(run.ready(), "3010 loads while moving");
}

#[test]
fn learned_skills_come_from_startset_and_a_new_learn_is_unknown() {
    // LearnedAtLogin (f24376) reads StartSet 271 only; the last one wins.
    let mut run = Run::started(with(startup(), 271, Some(learned(&[MOVE_SKILL]))));
    run.apply(271, learned(&[]));
    let mut run = run.constructed();
    run.d.gun_tick(0.0);
    run.apply(24, walk(OWN, (110, 210)));
    run.d.gun_tick(RELOAD_MS * 3.0);
    run.d.gun_tick(1.0);
    assert!(!run.ready(), "the later StartSet list cleared 3010");
    // A 271 after our construction may still be the client's last StartSet 271, so a
    // changed flag becomes unknown; an unchanged list keeps the flags known.
    let mut run = Run::armed();
    run.apply(271, learned(&[]));
    assert_eq!(run.missing(), None);
    run.apply(271, learned(&[MOVE_SKILL, HIT_SKILL]));
    assert_eq!(run.missing(), Some(M::GunSkills));
    assert!(!run.d.basic_attack_supported());
    let mut run = Run::reloaded(with(startup(), 271, Some(learned(&[HIT_SKILL]))));
    run.apply(271, learned(&[]));
    assert!(!run.waitable(), "the client may have dropped 3011");
    // OnLearned (f24407) may newly set a flag through an unmodelled level lookup.
    let mut run = Run::armed();
    run.apply(523, words(&[MOVE_SKILL]));
    assert_eq!(run.missing(), Some(M::GunSkills));
    assert!(!run.d.basic_attack_supported());
    // A flag that is already set stays known.
    let mut run = Run::started(with(startup(), 271, Some(learned(&[MOVE_SKILL])))).constructed();
    run.apply(523, words(&[MOVE_SKILL]));
    assert_eq!(run.missing(), None);
}

#[test]
fn load_speed_comes_from_a_complete_own_appearance_and_rewears_the_gun() {
    let mut run = Run::armed();
    run.apply(224, effect_start(OWN, 5, crate::gun::FAST_LOAD, 50.0));
    assert_eq!(run.missing(), Some(M::GunEffectState));
    // MyAppear wears the gun again: Ready waits for a fresh key.
    run.apply(9, player(OWN, &[(5, 50.0)], Some(GUN), 0));
    assert_eq!(run.missing(), Some(M::GunKey));
    assert_eq!(run.d.take_key_requests(), 1);
    run.apply(512, KEY.to_le_bytes().to_vec());
    assert_eq!(run.missing(), None);
    // FastLoad value 50 gives LoadSpeed 1.5, so 40 ms reload.
    run.d.gun_tick(0.0);
    run.d.gun_tick(RELOAD_MS * 2.0 / 3.0);
    run.d.gun_tick(1.0);
    assert!(run.ready());
    // Attributes bit 1 skips the re-wear.
    run.apply(9, player(OWN, &[(5, 50.0)], Some(GUN), 1));
    assert_eq!(run.d.take_key_requests(), 0);
    assert!(run.ready());
    // Ending the tracked instance, or a foreign FastLoad, leaves speed unknown.
    run.apply(280, words(&[OWN, 5]));
    assert_eq!(run.missing(), Some(M::GunEffectState));
    let mut run = Run::armed();
    run.apply(8, player(OTHER, &[(6, 20.0)], None, 0));
    run.apply(9, player(OWN, &[], Some(GUN), 1));
    assert_eq!(run.missing(), Some(M::GunEffectState));
}

#[test]
fn releases_end_the_wait_without_counting_as_an_outcome() {
    for release in [32u16, 310, 525] {
        let mut run = Run::reloaded(startup());
        run.shoot();
        let payload = if release == 32 { words(&[OWN]) } else { vec![] };
        run.apply(release, payload);
        assert!(run.d.take_correlated().is_empty(), "{release}");
        assert!(run.d.action_pending(), "{release}");
        assert!(
            !run.ready(),
            "{release}: Ready is refreshed by the next tick"
        );
        run.d.gun_tick(1.0);
        assert!(run.ready(), "{release}");
    }
    // Another actor's 32 does not release.
    let mut run = Run::reloaded(startup());
    run.shoot();
    run.apply(32, words(&[OTHER]));
    run.d.gun_tick(1.0);
    assert!(!run.waitable());
}

#[test]
fn equipment_callbacks_rewear_and_special_slots_load_and_unload() {
    let mut run = Run::armed();
    // 82 wears its item even when the UID is unchanged.
    run.apply(82, slot(GUN_UID, 1));
    assert_eq!(run.missing(), Some(M::GunKey));
    assert_eq!(run.d.take_key_requests(), 1);
    run.apply(512, KEY.to_le_bytes().to_vec());
    assert_eq!(run.missing(), None);
    // 521 unloads Type-29 ammo; 518 loads it again.
    run.apply(521, AMMO_UID.to_le_bytes().to_vec());
    assert_eq!(run.missing(), Some(M::GunAmmo));
    run.apply(518, AMMO_UID.to_le_bytes().to_vec());
    assert_eq!(run.missing(), None);
    // Short and unresolved bodies change nothing.
    run.apply(521, vec![1, 2]);
    run.apply(521, 0x999u64.to_le_bytes().to_vec());
    assert_eq!(run.missing(), None);
    // 85 unwears the named gun; without a gun the melee rules apply.
    run.apply(85, slot(GUN_UID, 1));
    assert_eq!(run.d.gun_worn(), Some(false));
    assert!(run.d.basic_attack_supported());
    // StartSet 518/521 replay in arrival order at construction.
    let mut packets = with(startup(), 233, Some(stats(ZONE, &[(1, GUN_UID)])));
    packets.push((518, AMMO_UID.to_le_bytes().to_vec()));
    packets.push((521, AMMO_UID.to_le_bytes().to_vec()));
    assert_eq!(
        Run::started(packets).constructed().missing(),
        Some(M::GunAmmo)
    );
    let mut packets = with(startup(), 233, Some(stats(ZONE, &[(1, GUN_UID)])));
    packets.push((521, AMMO_UID.to_le_bytes().to_vec()));
    packets.push((518, AMMO_UID.to_le_bytes().to_vec()));
    assert_eq!(Run::started(packets).constructed().missing(), None);
}

#[test]
fn camp_and_dig_hooks_follow_the_client() {
    let mut run = Run::reloaded(startup());
    // A kind-4 410 marks camp without disarming; leaving camp keeps the armed state.
    run.apply(410, vec![4]);
    assert_eq!(run.missing(), Some(M::GunCamp));
    run.apply(382, vec![]);
    assert_eq!(run.missing(), None);
    assert!(run.ready());
    // Any other 410 enters camp and disarms the shown gun.
    run.apply(410, vec![1]);
    assert_eq!(run.missing(), Some(M::GunCamp));
    run.apply(380, vec![]);
    assert_eq!(run.missing(), None);
    assert!(!run.ready(), "camp exit re-arms from state 1");
    // A dig end restarts the reload from state 1.
    let mut run = Run::reloaded(startup());
    run.d.drilling(true);
    run.d.drilling(false);
    run.d.gun_tick(1.0);
    assert!(run.waitable() && !run.ready());
}

#[test]
fn a_fainted_character_never_shoots() {
    let mut run = Run::reloaded(startup());
    run.apply(43, words(&[OWN]));
    assert!(!run.waitable() && !run.ready());
    assert!(run.d.gun_shot(run.key()).is_err());
}

#[test]
fn only_a_complete_own_swing_fires_the_gun() {
    let mut run = Run::reloaded(startup());
    run.shoot();
    run.mark += 1;
    let truncated = run.d.apply(&Packet {
        opcode: 31,
        aux: 0,
        mark: run.mark,
        received: Instant::now(),
        payload: Zeroizing::new(words(&[OWN, 2])),
    });
    assert!(truncated.is_err(), "ParseSwing throws on a truncated swing");
    assert!(!run.waitable(), "a rejected swing never fires the gun");
    run.apply(31, strike(OWN, 2, MONSTER));
    assert!(run.waitable(), "own firing starts the reload");
}

#[test]
fn an_own_effect_start_never_releases_a_shot_in_flight() {
    let mut run = Run::reloaded(startup());
    run.shoot();
    run.apply(224, effect_start(OWN, 9, crate::gun::FAST_LOAD + 1, 0.0));
    assert!(!run.waitable(), "Hit comes only from Damaged");
    // While reloading or Ready, the early reset only delays Ready.
    let mut run = Run::reloaded(startup());
    run.apply(224, effect_start(OWN, 9, crate::gun::FAST_LOAD + 1, 0.0));
    assert!(run.waitable() && !run.ready());
}
