//! Native normal-gun controller.
//!
//! A transcription of the client's `GunFlowing` for build `ea1a68bd`, driven only by received
//! state, public definitions and elapsed session time. States keep the source's numeric
//! branches; their descriptions are not recovered identifiers. Contracts and function indices
//! are in `docs/research/gun-combat-control.md` and `crates/net/README.md`.

use crate::{definitions::GunParams, wire::Writer};

/// Learned skill that keeps loading while moving (`GunFlowing.LoadsWhileMoving` +80).
pub(crate) const MOVE_SKILL: u16 = 3010;
/// Learned skill that keeps Load and Power on own damage (`GunFlowing.KeepsLoadOnHit` +81).
pub(crate) const HIT_SKILL: u16 = 3011;
/// ESA type of FastLoad, the only effect that writes `GunFlowing.LoadSpeed`.
pub(crate) const FAST_LOAD: u16 = 1476;
/// Promoted binary32 one sixth in `Tick` state 2 (f24393).
const SIXTH: f64 = f64::from_bits(0x3fc5_5555_6000_0000);
/// State 2 enters 3 when the reload candidate reaches this (f24393, `f64.ge`).
const LOADED: f64 = f64::from_bits(0x4068_ffff_f000_0000);
/// State 3 stores 200 when the power candidate exceeds this (f24393, `f64.gt`).
const POWER_CAP: f64 = f64::from_bits(0x4069_0000_1000_0000);
/// `GuardDamage` reaches a player's `ApplyDamage` through `After(500)` (ApplyMessage f17568).
const GUARD_DELAY_MS: f64 = 500.0;
/// Bound on delayed guard hits; a full list keeps the latest deadlines.
const MAX_GUARD_HITS: usize = 32;

/// `GunFlowing.State` (+48).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// 0: no automatic action; Ready is false.
    S0,
    /// 1: ammo is loaded; the next tick starts reloading when the zone allows it.
    S1,
    /// 2: reloading; Ready is false.
    S2,
    /// 3: Ready; power rises.
    S3,
    /// 4: Ready; power is full.
    S4,
    /// 5: a shot was sent; waits for own firing or a release. There is no local timeout.
    S5,
}

/// The loaded ammo stack as the client's inventory sees it at own firing.
#[derive(Clone, Copy)]
pub(crate) struct Ammo {
    /// Authoritative cached count of the loaded UID.
    pub(crate) count: u16,
    /// `ItemInfo.FlagsEx & 64` skips the local `Inventory.Consume`.
    pub(crate) keeps_count: bool,
}

pub(crate) struct Gun {
    armed: bool,
    ready: bool,
    state: State,
    load: f32,
    power: f32,
    param: Option<GunParams>,
    /// Loaded ammo UID; zero means none.
    loaded: u64,
    bunch: i32,
    key: Option<u16>,
    in_camp: bool,
    shown: bool,
    /// Fractional milliseconds kept by the UI tick callback (f46669).
    residue: f64,
    /// Local `Inventory.Consume` since the last authoritative count of `loaded`.
    consumed: i64,
    /// Empty logical 511 key requests that `Wear` sent and the session has not.
    requests: u32,
    /// A worn gun requested a key and no 512 has answered yet.
    awaiting: bool,
    /// Milliseconds until each delayed own guard damage reaches `Hit`.
    guard: Vec<f64>,
}

impl Default for Gun {
    fn default() -> Self {
        Self {
            armed: false,
            ready: false,
            state: State::S0,
            load: 0.0,
            power: 0.0,
            param: None,
            loaded: 0,
            bunch: 1,
            key: None,
            in_camp: false,
            shown: false,
            residue: 0.0,
            consumed: 0,
            requests: 0,
            awaiting: false,
            guard: Vec::new(),
        }
    }
}

impl Gun {
    pub(crate) fn armed(&self) -> bool {
        self.armed
    }
    pub(crate) fn key(&self) -> Option<u16> {
        self.key
    }
    pub(crate) fn in_camp(&self) -> bool {
        self.in_camp
    }
    pub(crate) fn loaded(&self) -> u64 {
        self.loaded
    }
    #[cfg(test)]
    pub(crate) fn state(&self) -> State {
        self.state
    }
    /// The loaded stack's count after local consumption, as `TakeBunch` reads it.
    pub(crate) fn effective(&self, count: u16) -> i64 {
        i64::from(count) - self.consumed
    }
    /// Ready (+45) in a Ready branch. A reset that has not been ticked yet does not count.
    pub(crate) fn ready(&self) -> bool {
        self.armed && self.ready && matches!(self.state, State::S3 | State::S4)
    }
    /// Armed and loading toward Ready, as each shot leaves it, with a gauge that `frame`
    /// lets rise. A gauge that cannot rise is not a reload anyone can wait for.
    pub(crate) fn reloading(&self, frame: Frame) -> bool {
        frame.worn
            && self.armed
            && match self.state {
                State::S1 => frame.zone_allows,
                State::S2 => frame.speed.is_some() && self.param.is_some(),
                _ => false,
            }
    }
    /// Armed in a state that reaches Ready without another received input.
    pub(crate) fn waitable(&self) -> bool {
        self.armed && matches!(self.state, State::S1 | State::S2 | State::S3 | State::S4)
    }
    pub(crate) fn awaiting_key(&self) -> bool {
        self.awaiting
    }
    pub(crate) fn take_requests(&mut self) -> u32 {
        std::mem::take(&mut self.requests)
    }
    fn reset(&mut self) {
        self.state = State::S0;
        self.load = 0.0;
        self.power = 0.0;
        self.armed = false;
        self.ready = false;
    }
    fn unload(&mut self) {
        self.loaded = 0;
        self.bunch = 1;
        self.state = State::S0;
        self.consumed = 0;
    }
    /// `Wear(itemId, on)` (f24375) for an item whose info is Type 2 and SubType 54.
    /// Wearing requests a fresh key; both directions reset and disarm.
    pub(crate) fn wear(&mut self, on: bool) {
        self.reset();
        self.awaiting = on;
        if on {
            self.requests = self.requests.saturating_add(1);
        } else {
            self.shown = false;
        }
    }
    /// 512 through `OnKey` (f24386): store the key and arm when not armed.
    pub(crate) fn on_key(&mut self, key: u16, profile: Option<GunParams>) {
        self.key = Some(key);
        self.awaiting = false;
        if !self.armed {
            self.gun_on(profile);
        }
    }
    /// `GunOn` (f24387). `profile` is the slot-1 item's `Item_GunParam2` row.
    pub(crate) fn gun_on(&mut self, profile: Option<GunParams>) {
        let Some(profile) = profile else {
            return;
        };
        if self.in_camp {
            return;
        }
        self.param = Some(profile);
        self.armed = true;
        self.state = if self.loaded != 0 {
            State::S1
        } else {
            State::S0
        };
    }
    /// Slot 20 at construction and live 518 through `SpecialOn` (f24379, ctor f24374).
    /// `kind` is the item's `(Type, SubType)`.
    pub(crate) fn special_on(&mut self, uid: u64, kind: (i32, i32)) {
        match kind {
            (29, _) => {
                self.bunch = 1;
                self.loaded = uid;
                self.state = State::S1;
                self.consumed = 0;
            }
            (16, _) | (11, 60) => self.unload(),
            _ => {}
        }
    }
    /// Live 521 through `Special(bytes, false)` (f24377) for an item of Type 29.
    pub(crate) fn special_off(&mut self) {
        self.unload();
        self.shown = false;
    }
    /// An authoritative count or removal replaced the client's projected count.
    pub(crate) fn authoritative(&mut self, uid: u64) {
        if uid == self.loaded {
            self.consumed = 0;
        }
    }
    /// The own avatar's per-frame `Update(dt, moving)` (f24392) with the UI residue (f46669).
    /// `speed` is unknown while active effects leave `LoadSpeed` unproven; reloading then
    /// makes no progress, so readiness can only come later than the client's.
    pub(crate) fn advance(&mut self, elapsed_ms: f64, frame: Frame) {
        let total = elapsed_ms + self.residue;
        // i32.trunc_sat_f64_s: Rust's saturating cast truncates the same way.
        let dt = total as i32;
        self.residue = total - f64::from(dt);
        if frame.worn && self.armed {
            self.tick(dt, frame);
        }
        // A delayed guard hit lands after this frame's progress, which it then discards.
        for remaining in &mut self.guard {
            *remaining -= f64::from(dt);
        }
        let due = self.guard.len();
        self.guard.retain(|remaining| *remaining > 0.0);
        if self.guard.len() != due {
            self.hit(frame.keeps_on_hit);
        }
    }
    /// One `Tick` branch (f24393). Several states never run in one call.
    fn tick(&mut self, dt: i32, frame: Frame) {
        match self.state {
            State::S0 | State::S5 => self.ready = false,
            State::S1 => {
                if frame.zone_allows {
                    self.shown = true;
                    self.load = 0.0;
                    self.power = 0.0;
                    self.state = State::S2;
                } else {
                    self.state = State::S0;
                    self.shown = false;
                }
            }
            State::S2 => {
                self.ready = false;
                let (Some(speed), Some(param)) = (frame.speed, self.param) else {
                    self.load = 0.0;
                    return;
                };
                let candidate = f64::from(speed) * SIXTH * f64::from(dt) * f64::from(param.speed)
                    / 100.0
                    + f64::from(self.load);
                self.load = candidate as f32;
                if frame.moving && !frame.keeps_moving {
                    self.load = 0.0;
                    return;
                }
                if candidate >= LOADED {
                    self.power = 0.0;
                    self.state = State::S3;
                }
            }
            State::S3 => {
                self.ready = true;
                let Some(param) = self.param else {
                    return;
                };
                let candidate = f64::from(param.range) / 1200.0 * f64::from(dt) * 100.0 / 100.0
                    + f64::from(self.power);
                self.power = if candidate > POWER_CAP {
                    200.0
                } else {
                    candidate as f32
                };
                if self.power >= 200.0 {
                    self.state = State::S4;
                }
            }
            State::S4 => self.ready = true,
        }
    }
    /// `Shoot(target, moveMode)` (f24401): only when Ready, with the actual profile, key,
    /// Bunch and move mode. Critical is computed, never forced.
    pub(crate) fn shoot(&mut self, target: u16, mode: u8) -> Option<Writer> {
        if !self.ready() {
            return None;
        }
        let (param, key) = (self.param?, self.key?);
        let critical = self.power > param.start as f32 && self.power < param.range as f32;
        self.state = State::S5;
        self.ready = false;
        let crc = crate::actions::gun_crc(param.speed, param.range, param.start, key, critical);
        Some(crate::actions::gun_shot(
            target,
            crc,
            self.bunch as u16,
            mode,
        ))
    }
    /// Own Kind-2 firing through `Fired`/`TakeBunch` (f24402/f24403). `ammo` is None when
    /// the loaded UID is no longer in the inventory.
    pub(crate) fn fired(&mut self, ammo: Option<Ammo>) {
        if self.loaded == 0 {
            return;
        }
        let Some(ammo) = ammo else {
            self.unload();
            self.shown = false;
            return;
        };
        let remaining = self.effective(ammo.count) - i64::from(self.bunch);
        if !ammo.keeps_count {
            self.consumed = self.consumed.saturating_add(i64::from(self.bunch));
        }
        if remaining <= 0 {
            self.unload();
            self.shown = false;
            return;
        }
        self.load = 0.0;
        self.power = 0.0;
        self.state = State::S2;
    }
    /// Own 32 (with the gun worn), or 310/525 while armed, through `ShotAnswered` (f24405).
    pub(crate) fn shot_answered(&mut self) {
        if self.state == State::S5 {
            self.state = State::S4;
        }
    }
    /// Own damage through `Hit` (f24406), unconditional on the current state.
    pub(crate) fn hit(&mut self, keeps_on_hit: bool) {
        if !keeps_on_hit {
            self.power = 0.0;
            self.state = State::S2;
            self.load = 0.0;
        }
    }
    /// An own ESA start whose damage reaches `Hit` later through unmodelled effect timers.
    /// Resetting early only while reloading or Ready can only delay Ready. A shot in flight
    /// (state 5) keeps waiting for its own firing or a release, as in the client.
    pub(crate) fn effect_hit(&mut self, keeps_on_hit: bool) {
        if matches!(self.state, State::S2 | State::S3 | State::S4) {
            self.hit(keeps_on_hit);
        }
    }
    /// Own `GuardDamage`: the client applies it 500 ms later. Resetting now as well can only
    /// delay readiness.
    pub(crate) fn guard_hit(&mut self, keeps_on_hit: bool) {
        self.hit(keeps_on_hit);
        if self.guard.len() >= MAX_GUARD_HITS {
            let earliest = self
                .guard
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.total_cmp(b.1))
                .map(|(at, _)| at);
            if let Some(at) = earliest {
                self.guard.swap_remove(at);
            }
        }
        self.guard.push(GUARD_DELAY_MS);
    }
    /// 377 or a 410 other than kind 4 through `CampEnter` (f24417). Only a shown gun resets.
    pub(crate) fn camp_enter(&mut self) {
        self.in_camp = true;
        if self.shown {
            self.shown = false;
            self.reset();
        }
    }
    /// A native dig start, as the drill hook's `DrillBegin` (f24412) hides the gauge.
    pub(crate) fn drill_begin(&mut self) {
        self.shown = false;
    }
    /// A native dig end, as `DrillEnd` (f24413) re-shows the gauge and restarts state 1.
    pub(crate) fn drill_end(&mut self, worn: bool) {
        if worn && !self.shown {
            self.shown = true;
            if self.param.is_some() {
                self.state = if self.loaded != 0 {
                    State::S1
                } else {
                    State::S0
                };
            }
        }
    }
    /// 410 with first byte 4 marks camp without resetting (b__8 f24431).
    pub(crate) fn camp_mark(&mut self) {
        self.in_camp = true;
    }
    /// 380/382 through `CampLeave` (f24418).
    pub(crate) fn camp_leave(&mut self, worn: bool, profile: Option<GunParams>) {
        self.in_camp = false;
        if worn && !self.shown {
            self.gun_on(profile);
            self.shown = true;
        }
    }
}

/// Per-frame inputs that `Update`/`Tick` read from outside `GunFlowing`.
#[derive(Clone, Copy)]
pub(crate) struct Frame {
    /// `GunWorn`: a SubType-54 item in a configured gun slot.
    pub(crate) worn: bool,
    /// The own avatar is walking.
    pub(crate) moving: bool,
    /// `MayShow` (f24395): a known zone Type other than 0, 1 and 5.
    pub(crate) zone_allows: bool,
    /// Known `LoadSpeed`.
    pub(crate) speed: Option<f32>,
    pub(crate) keeps_moving: bool,
    pub(crate) keeps_on_hit: bool,
}

/// `LoadSpeed` from FastLoad `f32(1 + f64(f32(100 - v)) / 100)` (`ESAFastLoad.Speed` f16108,
/// stored by `SetLoadSpeed` f24409); unusable values stay unknown instead of becoming 1.
pub(crate) fn load_speed(v: f32) -> Option<f32> {
    let speed = (1.0 + f64::from(100.0f32 - v) / 100.0) as f32;
    (v.is_finite() && speed.is_finite() && speed > 0.0).then_some(speed)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Attached {
    None,
    One { instance: u16, speed: f32 },
    Unusable,
}

/// What the client's `GunFlowing.LoadSpeed` is, from received effect state
/// (`docs/research/native-own-effect-lifecycle.md`). The constructor stores 1 and only a
/// FastLoad writes it later (`docs/research/native-gun-login-baseline.md`). Any FastLoad
/// event a complete own appearance after construction does not account for makes it unknown;
/// detached and foreign FastLoads keep it unknown for the rest of the session because the
/// client's setter has no owner check.
#[derive(Default)]
pub(crate) struct LoadSpeed {
    own: Option<Attached>,
    stale: bool,
    tainted: bool,
    built: bool,
}

impl LoadSpeed {
    pub(crate) fn value(&self) -> Option<f32> {
        if self.stale || self.tainted {
            return None;
        }
        match self.own? {
            Attached::None => Some(1.0),
            Attached::One { speed, .. } => Some(speed),
            Attached::Unusable => None,
        }
    }
    /// A complete own type-1 appearance on 8 or 9 replaces the own attached list. Before
    /// construction the host may drop a FastLoad's write, so such a snapshot settles nothing,
    /// and one that lists a FastLoad waits for a snapshot after construction.
    pub(crate) fn own_snapshot(&mut self, fast: &[(u16, f32)]) {
        let own = match fast {
            [] => Attached::None,
            [(instance, v)] => load_speed(*v).map_or(Attached::Unusable, |speed| Attached::One {
                instance: *instance,
                speed,
            }),
            _ => Attached::Unusable,
        };
        if self.built {
            self.stale = false;
        } else if own != Attached::None {
            self.stale = true;
        }
        self.own = Some(own);
    }
    /// Another actor's appearance with FastLoad writes the client's scalar too.
    pub(crate) fn foreign_snapshot(&mut self, fast: usize) {
        if fast > 0 {
            self.tainted = true;
        }
    }
    /// A 224/292 start. An own-recipient FastLoad with EsClass other than -1 or 6 is
    /// attached, so the next own snapshot lists it. Any other one is never attached, and an
    /// own snapshot cannot end it, so the speed stays unknown for the session.
    pub(crate) fn skill_start(&mut self, esa: u16, own: bool, class: i8) {
        if esa == FAST_LOAD {
            if own && !matches!(class, -1 | 6) {
                self.stale = true;
            } else {
                self.tainted = true;
            }
        }
    }
    /// A 224/292 too short to name its ESA type could be a FastLoad.
    pub(crate) fn unreadable(&mut self) {
        self.tainted = true;
    }
    /// A 307 start is detached from every actor.
    pub(crate) fn detached_start(&mut self, esa: u16) {
        if esa == FAST_LOAD {
            self.tainted = true;
        }
    }
    /// 280 or 15 may end the tracked own FastLoad, or another effect with the same key.
    pub(crate) fn ended(&mut self, instance: u16) {
        if matches!(self.own, Some(Attached::One { instance: i, .. }) if i == instance) {
            self.stale = true;
        }
    }
    /// Own faint or removal ends attached effects.
    pub(crate) fn own_lost(&mut self) {
        if matches!(self.own, Some(Attached::One { .. })) {
            self.stale = true;
        }
    }
    /// Whether a complete own snapshot can still make the speed known: no FastLoad outside
    /// the own attached list has written the client's scalar.
    pub(crate) fn knowable(&self) -> bool {
        !self.tainted
    }
    /// The constructor (f24374) stores 1, so without an earlier own list no FastLoad is
    /// attached.
    pub(crate) fn constructed(&mut self) {
        self.built = true;
        self.own.get_or_insert(Attached::None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_proves_one_unless_a_fast_load_came_first() {
        let mut s = LoadSpeed::default();
        assert_eq!(s.value(), None);
        s.constructed();
        assert_eq!(s.value(), Some(1.0), "the constructor stores 1");
        s.constructed();
        assert_eq!(s.value(), Some(1.0));
        // The host drops a write before the controller is provided, so no snapshot before
        // construction settles a FastLoad.
        let mut s = LoadSpeed::default();
        s.own_snapshot(&[(4, 50.0)]);
        s.constructed();
        assert_eq!(s.value(), None);
        s.own_snapshot(&[(4, 50.0)]);
        assert_eq!(s.value(), Some(1.5));
        let mut s = LoadSpeed::default();
        s.own_snapshot(&[(4, 50.0)]);
        s.own_snapshot(&[]);
        s.constructed();
        assert_eq!(s.value(), None);
        let mut s = LoadSpeed::default();
        s.skill_start(FAST_LOAD, true, 0);
        s.own_snapshot(&[]);
        s.constructed();
        assert_eq!(s.value(), None);
        s.own_snapshot(&[]);
        assert_eq!(s.value(), Some(1.0));
        // An empty own list before construction agrees with the constructor.
        let mut s = LoadSpeed::default();
        s.own_snapshot(&[]);
        s.constructed();
        assert_eq!(s.value(), Some(1.0));
        let mut s = LoadSpeed::default();
        s.foreign_snapshot(1);
        s.constructed();
        s.own_snapshot(&[]);
        assert_eq!(s.value(), None);
    }

    #[test]
    fn unattached_own_fast_load_is_never_repaired_by_an_own_snapshot() {
        for class in [-1, 6] {
            let mut s = LoadSpeed::default();
            s.constructed();
            s.own_snapshot(&[]);
            assert_eq!(s.value(), Some(1.0));
            s.skill_start(FAST_LOAD, true, class);
            s.own_snapshot(&[]);
            assert_eq!(s.value(), None, "EsClass {class} never attaches");
        }
        let mut s = LoadSpeed::default();
        s.constructed();
        s.own_snapshot(&[]);
        s.skill_start(FAST_LOAD, true, 1);
        assert_eq!(s.value(), None);
        s.own_snapshot(&[]);
        assert_eq!(
            s.value(),
            Some(1.0),
            "an attached start is listed by the snapshot"
        );
    }

    #[test]
    fn an_effect_reset_never_releases_a_shot_in_flight() {
        let mut gun = Gun {
            state: State::S5,
            ..Gun::default()
        };
        gun.effect_hit(false);
        assert_eq!(gun.state, State::S5);
        for state in [State::S2, State::S3, State::S4] {
            gun.state = state;
            gun.load = 1.0;
            gun.effect_hit(false);
            assert_eq!((gun.state, gun.load), (State::S2, 0.0));
        }
        gun.state = State::S4;
        gun.effect_hit(true);
        assert_eq!(gun.state, State::S4, "skill 3011 keeps the reload");
    }

    fn profile(speed: i32, range: i32, start: i32) -> GunParams {
        GunParams {
            kind: 0,
            speed,
            range,
            start,
            min_range: 0,
            max_range: 10,
        }
    }
    fn frame() -> Frame {
        Frame {
            worn: true,
            moving: false,
            zone_allows: true,
            speed: Some(1.0),
            keeps_moving: false,
            keeps_on_hit: false,
        }
    }
    /// Loaded, keyed and armed at state 1.
    fn armed(param: GunParams) -> Gun {
        let mut g = Gun::default();
        g.special_on(9, (29, 0));
        g.wear(true);
        assert_eq!(g.take_requests(), 1);
        g.on_key(0x55aa, Some(param));
        assert!(g.armed() && g.state() == State::S1);
        g
    }
    /// Milliseconds of state 2 at speed 1 for GaugeSpeed 100: 200 * 600 / 100.
    const RELOAD_MS: f64 = 1200.0;

    #[test]
    fn arming_requires_wear_key_profile_and_camp_exit() {
        let mut g = Gun::default();
        g.on_key(1, None);
        assert!(!g.armed());
        g.camp_enter();
        g.on_key(2, Some(profile(100, 300, 100)));
        assert!(!g.armed() && g.in_camp());
        g.camp_leave(true, Some(profile(100, 300, 100)));
        assert!(g.armed() && g.state() == State::S0 && g.key() == Some(2));
        // Entering camp disarms only a shown gun; a 410 kind-4 mark never resets.
        g.camp_mark();
        assert!(g.armed() && g.in_camp());
        g.camp_enter();
        assert!(!g.armed());
        g.camp_leave(true, Some(profile(100, 300, 100)));
        assert!(g.armed());
        g.special_on(7, (29, 0));
        assert_eq!((g.loaded(), g.state()), (7, State::S1));
        g.special_on(8, (11, 60));
        assert_eq!((g.loaded(), g.state()), (0, State::S0));
        g.special_on(8, (11, 59));
        g.special_on(9, (5, 0));
        assert_eq!(g.loaded(), 0);
        g.wear(false);
        assert!(!g.armed() && g.take_requests() == 0);
    }

    #[test]
    fn state_one_waits_for_an_allowed_zone_and_ignores_its_dt() {
        let mut g = armed(profile(100, 300, 100));
        g.advance(
            5000.0,
            Frame {
                zone_allows: false,
                ..frame()
            },
        );
        assert_eq!(g.state(), State::S0);
        let mut g = armed(profile(100, 300, 100));
        g.advance(5000.0, frame());
        assert_eq!((g.state(), g.load), (State::S2, 0.0));
        assert!(!g.ready());
    }

    #[test]
    fn reload_threshold_is_exact_binary64_and_one_branch_per_tick() {
        let mut g = armed(profile(100, 300, 100));
        g.advance(1.0, frame());
        // One oversized tick crosses the threshold but does not also charge power.
        g.advance(RELOAD_MS * 10.0, frame());
        assert_eq!((g.state(), g.power), (State::S3, 0.0));
        assert!(!g.ready(), "state 3 sets Ready on its own tick");
        g.advance(1.0, frame());
        assert!(g.ready());

        let mut g = armed(profile(100, 300, 100));
        g.advance(0.0, frame());
        g.advance(RELOAD_MS - 1.0, frame());
        assert_eq!(g.state(), State::S2);
        g.advance(1.0, frame());
        assert_eq!(g.state(), State::S3);

        // With dt 0 the candidate is the stored binary32 Load: the largest value below 200
        // stays in state 2 and 200 itself enters state 3.
        let mut g = armed(profile(100, 300, 100));
        g.advance(0.0, frame());
        g.load = f32::from_bits(200f32.to_bits() - 1);
        g.advance(0.0, frame());
        assert_eq!(g.state(), State::S2);
        g.load = 200.0;
        g.advance(0.0, frame());
        assert_eq!(g.state(), State::S3);
        // The threshold is the binary64 midpoint below 200.
        assert_eq!(LOADED, 200.0 - 2f64.powi(-17));
        assert_eq!(POWER_CAP, 200.0 + 2f64.powi(-17));
        // A candidate exactly on the midpoint enters state 3: the comparison is inclusive.
        // 512 ms adds exactly 11184811 * 2^-17, and this Load is exact in binary32.
        let mut g = armed(profile(100, 300, 100));
        g.advance(0.0, frame());
        g.load = 3_757_397.0 / 32_768.0;
        assert_eq!(SIXTH * 512.0 * 100.0 / 100.0 + f64::from(g.load), LOADED);
        g.advance(512.0, frame());
        assert_eq!((g.state(), g.load), (State::S3, 200.0));
    }

    #[test]
    fn critical_window_is_strict_on_both_bounds() {
        // Key 0 and a CRC base with bit 0 clear make the critical bit observable.
        for (power, critical) in [(100.0, false), (100.5, true), (149.5, true), (150.0, false)] {
            let mut g = armed(profile(100, 150, 100));
            g.state = State::S3;
            g.ready = true;
            g.key = Some(0);
            g.power = power;
            let shot = g.shoot(1, 1).unwrap();
            let crc = crate::actions::gun_crc(100, 150, 100, 0, critical);
            assert_eq!(&shot.0[2..4], &crc.to_le_bytes(), "power {power}");
            assert_eq!(crc & 1, critical as u16);
        }
    }

    #[test]
    fn fractional_milliseconds_carry_in_the_residue() {
        let mut g = armed(profile(100, 300, 100));
        g.advance(0.0, frame());
        for _ in 0..4 {
            g.advance(0.25, frame());
        }
        // Four quarter milliseconds make one whole millisecond of loading.
        let one = (f64::from(1.0f32) * SIXTH * 1.0 * 100.0 / 100.0) as f32;
        assert_eq!(g.load, one);
        assert_eq!(g.residue, 0.0);
    }

    #[test]
    fn power_saturates_strictly_above_the_cap_and_enters_four_at_200() {
        let mut g = armed(profile(100, 1200, 100));
        g.advance(0.0, frame());
        g.advance(RELOAD_MS, frame());
        assert_eq!(g.state(), State::S3);
        // PowRange 1200: one millisecond adds exactly 1.
        g.advance(199.0, frame());
        assert_eq!((g.state(), g.power), (State::S3, 199.0));
        g.advance(1.0, frame());
        assert_eq!((g.state(), g.power), (State::S4, 200.0));
        let mut g = armed(profile(100, 1200, 100));
        g.state = State::S3;
        g.power = 199.5;
        g.advance(1.0, frame());
        assert_eq!(g.power, 200.0, "candidate 200.5 exceeds the cap");
    }

    #[test]
    fn movement_resets_load_unless_learned() {
        let mut g = armed(profile(100, 300, 100));
        g.advance(0.0, frame());
        g.advance(
            600.0,
            Frame {
                moving: true,
                ..frame()
            },
        );
        assert_eq!((g.state(), g.load), (State::S2, 0.0));
        g.advance(
            600.0,
            Frame {
                moving: true,
                keeps_moving: true,
                ..frame()
            },
        );
        assert!(g.load > 0.0);
        g.advance(
            600.0,
            Frame {
                speed: None,
                ..frame()
            },
        );
        assert_eq!(
            (g.state(), g.load),
            (State::S2, 0.0),
            "unknown speed never loads"
        );
    }

    #[test]
    fn critical_uses_strict_bounds_and_the_shot_waits_for_firing() {
        let mut g = armed(profile(100, 1200, 100));
        g.advance(0.0, frame());
        g.advance(RELOAD_MS, frame());
        g.advance(100.0, frame());
        assert_eq!(g.power, 100.0);
        let shot = g.shoot(0x1234, 1).unwrap();
        let crc = crate::actions::gun_crc(100, 1200, 100, 0x55aa, false);
        assert_eq!(
            &*shot.0,
            &[0x34, 0x12, crc as u8, (crc >> 8) as u8, 1, 0, 1]
        );
        assert_eq!(g.state(), State::S5);
        assert!(g.shoot(0x1234, 1).is_none(), "no second shot while waiting");
        g.advance(5000.0, frame());
        assert_eq!(g.state(), State::S5, "waiting has no local timeout");
        g.shot_answered();
        assert_eq!(g.state(), State::S4);
        assert!(!g.ready(), "Ready is refreshed by the next tick");
        g.advance(1.0, frame());
        // A release keeps the old power; the move mode is the caller's.
        let shot = g.shoot(0x1234, 2).unwrap();
        let crc = crate::actions::gun_crc(100, 1200, 100, 0x55aa, false);
        assert_eq!(&shot.0[2..4], &crc.to_le_bytes());
        assert_eq!(shot.0[6], 2);
    }

    #[test]
    fn own_firing_consumes_projects_and_exhausts() {
        let mut g = armed(profile(100, 300, 100));
        g.state = State::S5;
        g.fired(Some(Ammo {
            count: 3,
            keeps_count: false,
        }));
        assert_eq!((g.state(), g.effective(3)), (State::S2, 2));
        g.state = State::S5;
        g.fired(Some(Ammo {
            count: 3,
            keeps_count: false,
        }));
        assert_eq!((g.state(), g.effective(3)), (State::S2, 1));
        // An authoritative count replaces the projection instead of being decremented again.
        g.authoritative(9);
        assert_eq!(g.effective(1), 1);
        g.state = State::S5;
        g.fired(Some(Ammo {
            count: 1,
            keeps_count: false,
        }));
        assert_eq!((g.state(), g.loaded()), (State::S0, 0));
        assert!(!g.ready());
        // FlagsEx 64 skips consumption but still tests the old count.
        let mut g = armed(profile(100, 300, 100));
        g.fired(Some(Ammo {
            count: 2,
            keeps_count: true,
        }));
        assert_eq!((g.state(), g.effective(2)), (State::S2, 2));
        g.fired(Some(Ammo {
            count: 1,
            keeps_count: true,
        }));
        assert_eq!(g.loaded(), 0);
        let mut g = armed(profile(100, 300, 100));
        g.fired(None);
        assert_eq!((g.state(), g.loaded()), (State::S0, 0));
        g.fired(Some(Ammo {
            count: 5,
            keeps_count: false,
        }));
        assert_eq!(g.state(), State::S0, "nothing loaded: no firing");
    }

    #[test]
    fn own_damage_restarts_reload_unless_learned() {
        let mut g = armed(profile(100, 300, 100));
        g.advance(0.0, frame());
        g.advance(RELOAD_MS, frame());
        g.advance(1.0, frame());
        assert!(g.ready());
        g.hit(true);
        assert!(g.ready());
        g.hit(false);
        assert!(!g.ready() && g.state() == State::S2 && g.load == 0.0);
        // A hit while waiting abandons the wait without a release.
        g.state = State::S5;
        g.hit(false);
        assert_eq!(g.state(), State::S2);
        g.shot_answered();
        assert_eq!(g.state(), State::S2);
    }

    #[test]
    fn guard_damage_resets_now_and_again_after_500ms() {
        let mut g = armed(profile(100, 300, 100));
        g.advance(0.0, frame());
        g.guard_hit(false);
        g.advance(499.0, frame());
        let loaded = g.load;
        assert!(loaded > 0.0);
        g.advance(1.0, frame());
        assert_eq!(g.load, 0.0, "the delayed hit resets the 500 ms of progress");
        assert!(g.guard.is_empty());
    }

    #[test]
    fn a_dig_end_restarts_reload_from_state_one() {
        let mut g = armed(profile(100, 300, 100));
        g.advance(0.0, frame());
        g.advance(RELOAD_MS, frame());
        assert_eq!(g.state(), State::S3);
        g.drill_begin();
        g.drill_end(true);
        assert_eq!(g.state(), State::S1);
        g.advance(1.0, frame());
        assert_eq!((g.state(), g.load), (State::S2, 0.0));
    }

    #[test]
    fn unworn_or_unarmed_guns_do_not_tick() {
        let mut g = armed(profile(100, 300, 100));
        g.advance(
            1.0,
            Frame {
                worn: false,
                ..frame()
            },
        );
        assert_eq!(g.state(), State::S1);
        g.wear(true);
        g.advance(1.0, frame());
        assert_eq!(g.state(), State::S0);
        assert_eq!(g.take_requests(), 1);
    }

    #[test]
    fn load_speed_needs_a_complete_own_snapshot_and_stays_conservative() {
        let mut s = LoadSpeed::default();
        assert_eq!(s.value(), None);
        s.constructed();
        s.own_snapshot(&[]);
        assert_eq!(s.value(), Some(1.0));
        s.own_snapshot(&[(4, 50.0)]);
        assert_eq!(s.value(), Some(1.5));
        assert_eq!(
            load_speed(100.25),
            Some((1.0 + f64::from(100.0f32 - 100.25) / 100.0) as f32)
        );
        assert_eq!(load_speed(f32::NAN), None);
        assert_eq!(load_speed(300.0), None);
        s.ended(5);
        assert_eq!(s.value(), Some(1.5));
        s.ended(4);
        assert_eq!(s.value(), None);
        s.own_snapshot(&[]);
        assert_eq!(s.value(), Some(1.0));
        s.skill_start(FAST_LOAD, true, 0);
        assert_eq!(s.value(), None);
        s.own_snapshot(&[(1, 0.0), (2, 0.0)]);
        assert_eq!(
            s.value(),
            None,
            "overlapping FastLoads have no recomputation rule"
        );
        s.own_snapshot(&[]);
        s.skill_start(FAST_LOAD + 1, false, 0);
        s.detached_start(FAST_LOAD + 1);
        s.foreign_snapshot(0);
        assert_eq!(s.value(), Some(1.0));
        s.foreign_snapshot(1);
        s.own_snapshot(&[]);
        assert_eq!(
            s.value(),
            None,
            "a foreign FastLoad is not repaired by own appearance"
        );
        let mut s = LoadSpeed::default();
        s.constructed();
        s.own_snapshot(&[]);
        s.detached_start(FAST_LOAD);
        s.own_snapshot(&[]);
        assert_eq!(s.value(), None);
        let mut s = LoadSpeed::default();
        s.constructed();
        s.own_snapshot(&[(3, 20.0)]);
        s.own_lost();
        assert_eq!(s.value(), None);
    }
}
