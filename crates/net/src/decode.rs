use crate::{
    NetError, Result, codec::Packet, definitions::GameDefinitions, resources, wire::Reader,
};
use opentrickster_engine::{self as e, Fact};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU32,
};

const MAX_ACTORS: usize = 4096;
const MAX_ITEMS: usize = 8192;
/// Build whose startup cutoff H is accepted as the complete carried inventory.
///
/// H is the last raw game-link receipt among accepted S5, S679, S230 and S233, with
/// delivery settled through it. It is a candidate from
/// `docs/research/native-inventory-baseline.md` ("Startup waits and the real replay
/// window", "Carried weight and invalidation"). Set this to the exact verified build
/// only after Trial A and Trial B there pass for that build. Until then, and after any
/// pin migration to another build, carried weight and HP potion counts stay unknown.
// Accepted from Trials A and B (docs/research/native-inventory-trials.md).
const INVENTORY_CUTOFF_BUILD: Option<&str> = Some("ea1a68bd89958fae94179f1e0835ffb5");
/// Logical opcodes whose `ReadItemChange` rows this decoder applies to the cache.
const INVENTORY_DELTAS: [u16; 8] = [561, 160, 770, 262, 246, 260, 843, 68];
/// Inventory subscribes to these after replay, but this decoder has no grammar for them.
/// 518/521 (slot 20), 82/85 (slots), 83 (refusal) and 41 (Galder) leave carried weight alone.
const UNDECODED_INVENTORY: &[u16] = crate::page::HEADER_ONLY;

pub(crate) fn text(s: String) -> Result<e::DisplayText> {
    if s.len() > 256 || s.chars().any(char::is_control) {
        return Err(NetError::Malformed);
    }
    e::DisplayText::checked(s).map_err(|_| NetError::Malformed)
}
fn gauge(current: u32) -> e::Gauge {
    e::Gauge {
        current,
        maximum: None,
    }
}
struct Actor {
    key: e::EntityKey,
    kind: u16,
    species: e::SpeciesKey,
    label: String,
    point: e::Point,
    alive: bool,
    hp: Option<u32>,
    target: Option<u16>,
    effects: bool,
}
struct Item {
    key: e::StackKey,
    count: u16,
    state: [u8; 4],
    id: u32,
    wire_kind: u8,
    options: Option<Vec<[u32; 3]>>,
}
struct ItemBody {
    kind: u8,
    id: u32,
    uid: u64,
    count: u16,
    state: [u8; 4],
    options: Option<Vec<[u32; 3]>>,
}
struct PendingAction {
    action: e::Action,
    mark: u64,
    ended: bool,
    outcome: bool,
}
pub(crate) struct DeferredFacts {
    pub(crate) mark: u64,
    pub(crate) received: std::time::Instant,
    pub(crate) facts: Vec<Fact>,
}
/// Carried-inventory proof for one game session under the startup cutoff H.
#[derive(Clone, Copy)]
enum Carried {
    /// The cutoff is unaccepted, or an input made weight and potions unknown for this session.
    Unknown,
    /// Accepted cutoff; waits for H and for delivery settled through it.
    Startup { cutoff: Option<u64> },
    /// The cache was complete at H, and every later inventory input was applied in order.
    Complete { cutoff: u64 },
}
struct Carry {
    weight: u32,
    maximum: Option<NonZeroU32>,
    potions: u32,
}
impl Carry {
    fn facts(self) -> [Fact; 2] {
        [
            Fact::Weight(e::Gauge {
                current: self.weight,
                maximum: self.maximum,
            }),
            Fact::HpPotions(self.potions),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingPrerequisite {
    OwnBaseline,
    Definitions,
    Equipment,
    GunProfile,
    GunKey,
    GunAmmo,
    GunSkills,
    GunEffectState,
    PotionInventory,
    PotionInstanceOverrides,
    PotionInstanceOptions,
    PotionExpiry,
    PotionNormalForm,
    PotionUseState,
    InventoryCompleteness,
}
#[derive(Clone, Copy)]
pub struct Readiness {
    pub own_baseline_seen: bool,
    pub inventory_list_seen: bool,
    pub equipment_references_resolved: bool,
    pub hp_maximum_known: bool,
    pub mp_maximum_known: bool,
    pub weight_maximum_known: bool,
    pub missing_gun: Option<MissingPrerequisite>,
    pub missing_hp_potion: Option<MissingPrerequisite>,
}
impl Default for Readiness {
    fn default() -> Self {
        Self {
            own_baseline_seen: false,
            inventory_list_seen: false,
            equipment_references_resolved: false,
            hp_maximum_known: false,
            mp_maximum_known: false,
            weight_maximum_known: false,
            missing_gun: Some(MissingPrerequisite::OwnBaseline),
            missing_hp_potion: Some(MissingPrerequisite::OwnBaseline),
        }
    }
}
/// No complete-inventory flag exists. These fields report only observed boundaries.
pub(crate) struct Decoder {
    definitions: Option<std::sync::Arc<crate::definitions::GameDefinitions>>,
    mode: Option<u8>,
    native_walk: bool,
    physical_uncertain: bool,
    pending: Option<PendingAction>,
    combat: crate::combat::Correlator,
    correlated: Vec<DeferredFacts>,
    status: resources::Status,
    current_hp: Option<(u32, std::time::Instant)>,
    current_mp: Option<(u32, std::time::Instant)>,
    gun_key: Option<u16>,
    learned: Option<BTreeSet<u16>>,
    zone: Option<u16>,
    dig_active: bool,
    own: Option<u16>,
    map: Option<e::MapKey>,
    point: Option<e::Point>,
    clock_seen: bool,
    profile_seen: bool,
    stats_seen: bool,
    profile: Option<e::Profile>,
    actors: BTreeMap<u16, Actor>,
    items: BTreeMap<u64, Item>,
    kinds: BTreeMap<(u8, u32), e::ItemKind>,
    item_descriptors: Vec<(e::ItemKind, crate::ItemDescriptor)>,
    species: BTreeMap<u16, e::SpeciesKey>,
    maps: BTreeMap<u16, e::MapKey>,
    drops: BTreeMap<u16, e::DropKey>,
    slots: [u64; 23],
    missing_slots: BTreeSet<u64>,
    missing_slot_refs: [u64; 23],
    next: u64,
    carried: Carried,
    pub(crate) readiness: Readiness,
}
impl Decoder {
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::with_epoch(0)
    }
    /// Tests exercise both cutoff states without changing `INVENTORY_CUTOFF_BUILD`.
    #[cfg(test)]
    pub(crate) fn set_inventory_cutoff(&mut self, accepted: bool) {
        self.carried = if accepted {
            Carried::Startup { cutoff: None }
        } else {
            Carried::Unknown
        };
    }
    pub(crate) fn with_epoch(epoch: u64) -> Self {
        Self {
            definitions: None,
            mode: None,
            native_walk: false,
            physical_uncertain: false,
            pending: None,
            combat: Default::default(),
            correlated: Vec::new(),
            status: Default::default(),
            current_hp: None,
            current_mp: None,
            gun_key: None,
            learned: None,
            zone: None,
            dig_active: false,
            own: None,
            map: None,
            point: None,
            clock_seen: false,
            profile_seen: false,
            stats_seen: false,
            profile: None,
            actors: BTreeMap::new(),
            items: BTreeMap::new(),
            kinds: BTreeMap::new(),
            item_descriptors: Vec::new(),
            species: BTreeMap::new(),
            maps: BTreeMap::new(),
            drops: BTreeMap::new(),
            slots: [0; 23],
            missing_slots: BTreeSet::new(),
            missing_slot_refs: [0; 23],
            next: epoch.saturating_mul(1 << 32),
            carried: if INVENTORY_CUTOFF_BUILD == Some(crate::BUILD) {
                Carried::Startup { cutoff: None }
            } else {
                Carried::Unknown
            },
            readiness: Readiness::default(),
        }
    }
    /// Records H once the four startup receipts are known. Packets are numbered in raw
    /// order, so every packet decoded before this call precedes H.
    pub(crate) fn startup_cutoff(&mut self, cutoff: u64) {
        self.status.startup(cutoff);
        if let Carried::Startup { cutoff: None } = self.carried {
            self.carried = Carried::Startup {
                cutoff: Some(cutoff),
            };
        }
    }
    /// Reports carried weight and HP potions once delivery has settled through H.
    /// A missing list, definition or baseline leaves both unknown for this session.
    pub(crate) fn settle_carried(&mut self, frontier: u64) -> Vec<Fact> {
        let Carried::Startup {
            cutoff: Some(cutoff),
        } = self.carried
        else {
            return Vec::new();
        };
        if frontier < cutoff {
            return Vec::new();
        }
        // Only a received list at or before H distinguishes an empty bag from a missing one.
        match self
            .readiness
            .inventory_list_seen
            .then(|| self.carry())
            .flatten()
        {
            Some(carry) => {
                self.carried = Carried::Complete { cutoff };
                carry.facts().into()
            }
            None => {
                self.carried = Carried::Unknown;
                Vec::new()
            }
        }
    }
    /// The engine cannot forget a reported fact, so losing a reported value fails the session.
    fn lose_carried(&mut self) -> Result<()> {
        let reported = matches!(self.carried, Carried::Complete { .. });
        self.carried = Carried::Unknown;
        if reported {
            Err(NetError::Uncertain)
        } else {
            Ok(())
        }
    }
    fn track_carried(
        &mut self,
        p: &Packet,
        stray: bool,
        old_maximum: Option<NonZeroU32>,
        out: &mut Vec<Fact>,
    ) -> Result<()> {
        let cutoff = match self.carried {
            Carried::Unknown => return Ok(()),
            Carried::Startup { cutoff } => cutoff,
            Carried::Complete { cutoff } => Some(cutoff),
        };
        let lost = if cutoff.is_some_and(|cutoff| p.mark > cutoff) {
            // A late list, a row naming an uncached carried UID, or an inventory input
            // without a decoder grammar means the cache no longer mirrors the client's.
            p.opcode == 231 || stray || UNDECODED_INVENTORY.contains(&p.opcode)
        } else {
            // Replay through H applies only 231/233 (and 518/521 slot 20); Inventory
            // subscribes to deltas after replay, so an earlier delta is outside the cutoff.
            INVENTORY_DELTAS.contains(&p.opcode)
        };
        if lost {
            return self.lose_carried();
        }
        if matches!(self.carried, Carried::Complete { .. })
            && (INVENTORY_DELTAS.contains(&p.opcode)
                || p.opcode == 233
                || old_maximum != self.maximum(6))
        {
            let Some(carry) = self.carry() else {
                return self.lose_carried();
            };
            out.extend(carry.facts());
        }
        Ok(())
    }
    /// Weight and HP potions of the whole cache, or None when any input is unknown.
    fn carry(&self) -> Option<Carry> {
        let data = self.definitions.as_ref()?;
        let profile = self.profile.as_ref()?;
        let mut weight = 0i64;
        let mut potions = 0u32;
        // Equipped and expiring items stay in the cache; neither leaves carried weight.
        for item in self.items.values() {
            let info = data.items.get(&item.id)?;
            // ItemWeight.Of: ItemInfo.Weight x pieces, where pieces is Count only for wire Kind 1.
            let pieces = if item.wire_kind == 1 {
                i64::from(item.count)
            } else {
                1
            };
            weight = weight.checked_add(info.weight.checked_mul(pieces)?)?;
            // Reuses potion_prerequisite's criteria: a stack counts only when nothing but
            // the per-use state remains unproven. Instance options, overrides, expiry and
            // normal-form restrictions exclude it.
            if potion_reason(data, profile, item) == Some(MissingPrerequisite::PotionUseState) {
                potions = potions.checked_add(u32::from(item.count))?;
            }
        }
        let weight = u32::try_from(weight).ok()?;
        Some(Carry {
            weight,
            // As for resource composites, an exceeded maximum is reported as unknown.
            maximum: self.maximum(6).filter(|max| weight <= max.get()),
            potions,
        })
    }
    /// Maximum HP (9), MP (3) or weight (6): `CharacterStatus.Total`, or unknown.
    fn maximum(&self, stat: usize) -> Option<NonZeroU32> {
        let player = self
            .definitions
            .as_deref()
            .map(|d| d.player_levels.as_slice());
        self.status.maximum(stat, player)
    }
    pub(crate) fn set_definitions(
        &mut self,
        data: Option<std::sync::Arc<crate::definitions::GameDefinitions>>,
    ) {
        self.definitions = data;
    }
    pub(crate) fn use_native_walk_policy(&mut self) {
        self.native_walk = true;
        self.mode = Some(1);
    }
    pub(crate) fn move_payload(&self, to: e::Point) -> Result<crate::wire::Writer> {
        self.map.ok_or(NetError::Unsupported)?;
        let from = self.point.ok_or(NetError::Unsupported)?;
        let mode = self.mode.ok_or(NetError::Unsupported)?;
        crate::actions::movement(from, to, mode)
    }
    pub(crate) fn expect_action(&mut self, action: e::Action, mark: u64) -> Result<()> {
        if self.pending.is_some() {
            return Err(NetError::Busy);
        }
        if let e::Action::BasicAttack(key) = action {
            self.combat.expect(crate::combat::Pending {
                target: self.raw_target(key)?,
                key,
                floor: mark,
            });
        }
        self.pending = Some(PendingAction {
            action,
            mark,
            ended: false,
            outcome: false,
        });
        Ok(())
    }
    pub(crate) fn action_pending(&self) -> bool {
        self.pending.is_some()
    }
    /// The action's window closed without a correlated outcome; later evidence is not attributed.
    pub(crate) fn expire_action(&mut self) {
        self.pending = None;
        self.combat.clear();
    }
    pub(crate) fn physical_uncertain(&self) -> bool {
        self.physical_uncertain
    }
    pub(crate) fn sampling_identity(&self) -> Option<(u16, u16)> {
        Some((self.own?, self.zone?))
    }
    pub(crate) fn take_correlated(&mut self) -> Vec<DeferredFacts> {
        std::mem::take(&mut self.correlated)
    }
    pub(crate) fn normal_weapon_mode(&self) -> Result<u8> {
        let data = self.definitions.as_ref().ok_or(NetError::Unsupported)?;
        if !self.readiness.equipment_references_resolved {
            return Err(NetError::Unsupported);
        }
        for &slot in &data.gun_slots {
            let uid = *self.slots.get(slot).ok_or(NetError::Malformed)?;
            if uid == 0 {
                continue;
            }
            let item = self.items.get(&uid).ok_or(NetError::Unsupported)?;
            let (_, subtype) = data.item_types.get(&item.id).ok_or(NetError::Unsupported)?;
            if *subtype == 54 {
                return Err(NetError::Unsupported);
            }
        }
        self.mode.ok_or(NetError::Unsupported)
    }
    pub(crate) fn attack_payloads(
        &self,
        key: e::EntityKey,
    ) -> Result<(crate::wire::Writer, crate::wire::Writer)> {
        Ok(crate::actions::normal_attack(
            self.raw_target(key)?,
            self.normal_weapon_mode()?,
        ))
    }
    pub(crate) fn dig_inputs(&self) -> Result<crate::dig::Inputs> {
        let data = self.definitions.as_ref().ok_or(NetError::Unsupported)?;
        let zone = self.zone.ok_or(NetError::Unsupported)?;
        let (kind, subtype) = data.zones.get(&zone).ok_or(NetError::Unsupported)?;
        if matches!(*kind, 1 | 5 | 6)
            || *subtype == 14
            || !self.readiness.equipment_references_resolved
        {
            return Err(NetError::Unsupported);
        }
        let item = self
            .items
            .get(&self.slots[7])
            .ok_or(NetError::Unsupported)?;
        let (kind, _) = data.item_types.get(&item.id).ok_or(NetError::Unsupported)?;
        if *kind != 6 || item.count == 0 {
            return Err(NetError::Unsupported);
        }
        let params = *data.drills.get(&item.id).ok_or(NetError::Unsupported)?;
        let metadata = data.drill_meta.get(&item.id).ok_or(NetError::Unsupported)?;
        if metadata.grade <= 0 || metadata.maximum_life <= 0 {
            return Err(NetError::Unsupported);
        }
        Ok(crate::dig::Inputs {
            params,
            life: item.count,
            actor: self.own.ok_or(NetError::Unsupported)?,
            mode: self.mode.ok_or(NetError::Unsupported)?,
        })
    }
    fn next(&mut self) -> Result<u64> {
        self.next = self.next.checked_add(1).ok_or(NetError::Capacity)?;
        Ok(self.next)
    }
    fn item_kind(&mut self, kind: u8, id: u32) -> Result<e::ItemKind> {
        if let Some(k) = self.kinds.get(&(kind, id)) {
            return Ok(*k);
        }
        if self.kinds.len() >= MAX_ITEMS {
            return Err(NetError::Capacity);
        }
        let k = e::ItemKind(self.next()?);
        self.kinds.insert((kind, id), k);
        if let Some(descriptor) = self.definitions.as_ref().and_then(|d| d.descriptor(id)) {
            self.item_descriptors.push((k, descriptor));
        }
        Ok(k)
    }
    fn species(&mut self, id: u16) -> Result<e::SpeciesKey> {
        if let Some(k) = self.species.get(&id) {
            return Ok(*k);
        }
        if self.species.len() >= MAX_ACTORS {
            return Err(NetError::Capacity);
        }
        let k = e::SpeciesKey(self.next()?);
        self.species.insert(id, k);
        Ok(k)
    }
    fn map_key(&mut self, id: u16) -> Result<e::MapKey> {
        if let Some(k) = self.maps.get(&id) {
            return Ok(*k);
        }
        if self.maps.len() >= MAX_ACTORS {
            return Err(NetError::Capacity);
        }
        let k = e::MapKey(self.next()?);
        self.maps.insert(id, k);
        Ok(k)
    }
    pub(crate) fn baseline(&self) -> bool {
        self.clock_seen && self.profile_seen && self.stats_seen
    }
    pub(crate) fn raw_target(&self, key: e::EntityKey) -> Result<u16> {
        self.actors
            .iter()
            .find_map(|(&id, a)| {
                (a.key == key
                    && a.kind == 3
                    && a.alive
                    && !a.effects
                    && (a.target == Some(0)
                        || a.target.is_some_and(|target| Some(target) == self.own)))
                .then_some(id)
            })
            .ok_or(NetError::Unsupported)
    }
    fn entity(&self, id: u16) -> Result<Option<Fact>> {
        let Some(a) = self.actors.get(&id) else {
            return Ok(None);
        };
        if a.kind != 3 {
            return Ok(None);
        }
        Ok(Some(Fact::Entity(e::Entity {
            key: a.key,
            species: a.species,
            label: text(a.label.clone())?,
            position: a.point,
            alive: a.alive,
            hp: a.hp.map(gauge),
            engaged_by_other: match a.target {
                Some(0) => Some(false),
                Some(t) if Some(t) == self.own => Some(false),
                Some(t) => self
                    .actors
                    .get(&t)
                    .and_then(|target| matches!(target.kind, 1 | 3).then_some(true)),
                None => None,
            },
        })))
    }
    pub(crate) fn apply(&mut self, p: &Packet) -> Result<Vec<Fact>> {
        let old_maxima = [self.maximum(9), self.maximum(3)];
        let old_weight_maximum = self.maximum(6);
        self.status.observe(p)?;
        let b = &p.payload;
        let mut r = Reader::new(b);
        let mut out = Vec::new();
        let mut stray = false;
        match p.opcode {
            230 => {
                r.u32()?;
                self.own = Some(r.u16()?);
                let point = e::Point {
                    x: r.u16()? as i32,
                    y: r.u16()? as i32,
                };
                r.end()?;
                self.point = Some(point);
                self.clock_seen = true;
                if let Some(map) = self.map {
                    out.push(Fact::Position { map, point });
                }
            }
            679 => {
                if b.len() < 69 {
                    return Err(NetError::Truncated);
                }
                r.take(17)?;
                let label = r.fixed(20)?;
                r.u16()?;
                out.push(Fact::OwnLabel(text(label)?));
                self.profile_seen = true;
            }
            233 => {
                if b.len() < 251 {
                    return Err(NetError::Truncated);
                }
                for slot in &mut self.slots {
                    let uid = r.u64()?;
                    *slot = if uid >= 2 { uid } else { 0 };
                }
                self.missing_slots.clear();
                self.missing_slot_refs = [0; 23];
                for (slot, uid) in self.slots.iter_mut().enumerate() {
                    if *uid != 0 && !self.items.contains_key(uid) {
                        self.missing_slots.insert(*uid);
                        self.missing_slot_refs[slot] = *uid;
                        *uid = 0;
                    }
                }
                r.take(28)?;
                let tm_level = r.u16()?;
                let level = r.u16()?;
                r.take(3)?;
                let zone = r.u16()?;
                if self.zone.is_some_and(|old| old != zone) {
                    self.physical_uncertain = true;
                    self.actors.clear();
                    self.drops.clear();
                    self.combat.clear();
                    self.pending = None;
                }
                self.zone = Some(zone);
                let map = self.map_key(zone)?;
                self.map = Some(map);
                if let Some(label) = self
                    .definitions
                    .as_ref()
                    .and_then(|d| d.zone_labels.get(&zone))
                {
                    out.push(Fact::MapLabel {
                        map,
                        label: text(label.clone())?,
                    });
                }
                r.take(10)?;
                let galder = r.u32()? as u64;
                let hp = r.u32()?;
                let mp = r.u32()?;
                self.current_hp = Some((hp, p.received));
                self.current_mp = Some((mp, p.received));
                let tm_experience = r.u32()?;
                let experience = r.u32()?;
                let profile = e::Profile {
                    level,
                    tm_level,
                    experience,
                    tm_experience,
                    galder,
                };
                self.profile = Some(e::Profile {
                    level,
                    tm_level,
                    experience,
                    tm_experience,
                    galder,
                });
                out.extend([
                    Fact::Profile(profile),
                    Fact::Hp(gauge(hp)),
                    Fact::Mp(gauge(mp)),
                ]);
                if let Some(point) = self.point {
                    out.push(Fact::Position { map, point });
                }
                self.stats_seen = true;
                self.readiness.equipment_references_resolved = self.missing_slots.is_empty();
            }
            7..=9 => {
                let parsed = appearance(b)?;
                if self.actors.len() >= MAX_ACTORS && !self.actors.contains_key(&parsed.id) {
                    return Err(NetError::Capacity);
                }
                // Every appearance begins a fresh incarnation, fencing delayed action evidence.
                let key = e::EntityKey(self.next()?);
                let species = self.species(parsed.species)?;
                if let Some(old) = self.actors.remove(&parsed.id)
                    && old.kind == 3
                {
                    out.push(Fact::EntityGone(old.key));
                }
                self.actors.insert(
                    parsed.id,
                    Actor {
                        key,
                        kind: parsed.kind,
                        species,
                        label: parsed.label,
                        point: parsed.point,
                        alive: true,
                        hp: None,
                        target: parsed.target,
                        effects: parsed.effects,
                    },
                );
                if let Some(f) = self.entity(parsed.id)? {
                    out.push(f);
                }
            }
            6 | 15 | 16 | 21 => {
                let id = r.u16()?;
                // Live ea1a68bd removals on 15 carry one trailing byte; the client reads the ID.
                if p.opcode == 15 && r.remaining() == 1 {
                    r.u8()?;
                }
                r.end()?;
                if let Some(old) = self.actors.remove(&id)
                    && old.kind == 3
                {
                    out.push(Fact::EntityGone(old.key));
                }
                for actor in self.actors.values_mut() {
                    if actor.target == Some(id) {
                        actor.target = None;
                    }
                }
            }
            24..=27 | 729 => {
                let id = r.u16()?;
                if matches!(p.opcode, 24 | 26) {
                    r.take(4)?;
                }
                let point = e::Point {
                    x: r.u16()? as i32,
                    y: r.u16()? as i32,
                };
                let mode = r.u8()?;
                r.end()?;
                if Some(id) == self.own && p.opcode != 729 {
                    if !self.native_walk {
                        self.mode = Some(mode);
                    }
                    self.point = Some(point);
                    if let Some(map) = self.map {
                        out.push(Fact::Position { map, point });
                    }
                    if matches!(p.opcode, 25 | 27) && self.pending.as_ref().is_some_and(|pending| {
                        p.mark > pending.mark
                            && matches!(pending.action,e::Action::MoveTo(target) if target==point)
                    }) {
                        self.pending = None;
                        out.push(Fact::Outcome(e::Outcome::Arrived(point)));
                    }
                    // A destination in a movement packet is not proof of arrival or idle.
                }
                if let Some(a) = self.actors.get_mut(&id) {
                    a.point = point;
                }
                if let Some(f) = self.entity(id)? {
                    out.push(f);
                }
            }
            36 | 37 => {
                let value = (r.u32()? as i32).max(0) as u32;
                r.end()?;
                if p.opcode == 36 {
                    self.current_hp = Some((value, p.received));
                } else {
                    self.current_mp = Some((value, p.received));
                }
                out.push(if p.opcode == 36 {
                    Fact::Hp(resources::gauge(value, self.maximum(9)))
                } else {
                    Fact::Mp(resources::gauge(value, self.maximum(3)))
                });
            }
            32 => {
                let actor = r.u16()?;
                r.end()?;
                if Some(actor) == self.own
                    && let Some(pending) = &mut self.pending
                    && p.mark > pending.mark
                    && matches!(pending.action, e::Action::BasicAttack(_))
                {
                    pending.ended = true;
                }
            }
            512 => {
                let key = r.u16()?;
                r.end()?;
                self.gun_key = Some(key);
            }
            271 => {
                let count = r.u16()? as usize;
                let mut learned = BTreeSet::new();
                for _ in 0..count {
                    learned.insert(r.u16()?);
                    r.u8()?;
                    r.u16()?;
                }
                r.end()?;
                self.learned = Some(learned);
            }
            523 => {
                let skill = r.u16()?;
                if let Some(learned) = &mut self.learned {
                    learned.insert(skill);
                }
            }
            286 | 289 => {
                let id = r.u16()?;
                let hp = (r.u32()? as i32).max(0) as u32;
                if p.opcode == 289 {
                    r.u32()?;
                }
                if let Some(a) = self.actors.get_mut(&id) {
                    a.hp = Some(hp);
                }
                if let Some(f) = self.entity(id)? {
                    out.push(f);
                }
            }
            43 => {
                let id = r.u16()?;
                if r.remaining() > 0 {
                    r.u8()?;
                }
                r.end()?;
                if Some(id) == self.own {
                    self.current_hp = Some((0, p.received));
                    out.push(Fact::Hp(resources::gauge(0, self.maximum(9))));
                }
                if let Some(a) = self.actors.get_mut(&id) {
                    a.alive = false;
                    a.hp = Some(0);
                }
                if let Some(f) = self.entity(id)? {
                    out.push(f);
                }
                // Faint never awards a kill. Its packet contains no attacker attribution.
            }
            42 => {
                // Combat.ParseDamage reads a U16 Target and 24 damage bytes, then a U16 Guard
                // only if two bytes remain. It never checks the end and ignores any later bytes.
                r.take(26)?;
            }
            231 => {
                let count = r.u16()? as usize;
                if count > MAX_ITEMS {
                    return Err(NetError::Capacity);
                }
                let mut list = Vec::new();
                for _ in 0..count {
                    let kind = r.u8()?;
                    list.push(item_body(&mut r, kind, true)?);
                }
                r.end()?;
                let mut observed = Vec::new();
                for item in list {
                    let mut facts = Vec::new();
                    self.put(item, &mut facts)?;
                    for fact in facts {
                        if let Fact::Inventory(e::Inventory::Put(stack)) = fact {
                            observed.push(stack);
                        }
                    }
                }
                out.push(Fact::InventorySnapshot(observed));
                self.readiness.inventory_list_seen = true;
            }
            561 | 160 | 770 | 262 | 246 | 260 | 843 | 68 => {
                if p.opcode == 770 {
                    r.take(20)?;
                }
                let count = match p.opcode {
                    262 | 246 | 260 => (r.u16()? as usize).min(3),
                    843 => r.u32()? as usize,
                    _ => 1,
                };
                if count > MAX_ITEMS {
                    return Err(NetError::Capacity);
                }
                let mut changes = Vec::new();
                for _ in 0..count {
                    let op = r.u8()?;
                    let kind = r.u8()?;
                    changes.push((op, item_body(&mut r, kind, matches!(op, 2 | 4))?));
                }
                // The target's three-entry readers intentionally leave the remainder unread.
                if !matches!(p.opcode, 262 | 246 | 260) {
                    r.end()?;
                }
                for (op, item) in changes {
                    // 68 updates an existing row; naming an uncached UID diverges from the client.
                    stray |= p.opcode == 68
                        && matches!(item.kind, 1 | 2)
                        && !self.items.contains_key(&item.uid);
                    stray |= self.change(op, item, &mut out)?;
                }
            }
            82 => {
                let uid = r.u64()?;
                let slot = r.u8()? as usize;
                r.end()?;
                if slot > 22 {
                    return Err(NetError::Malformed);
                }
                if self.items.contains_key(&uid) {
                    self.slots[slot] = uid;
                    self.missing_slot_refs[slot] = 0;
                }
            }
            85 => {
                r.u64()?;
                let slot = r.u8()? as usize;
                r.end()?;
                if slot > 22 {
                    return Err(NetError::Malformed);
                }
                self.slots[slot] = 0;
                self.missing_slot_refs[slot] = 0;
            }
            41 => {
                let amount = r.u32()? as u64;
                r.end()?;
                if let Some(p) = &mut self.profile {
                    p.galder = amount;
                    out.push(Fact::Profile(e::Profile {
                        level: p.level,
                        tm_level: p.tm_level,
                        experience: p.experience,
                        tm_experience: p.tm_experience,
                        galder: p.galder,
                    }));
                }
            }
            10..=14 | 617 => {
                if matches!(p.opcode, 12 | 14 | 617) {
                    r.u16()?;
                }
                let id = r.u16()?;
                let point = e::Point {
                    x: r.u16()? as i32,
                    y: r.u16()? as i32,
                };
                if matches!(p.opcode, 13 | 14) {
                    r.take(4)?;
                }
                r.u16()?;
                r.take(3)?;
                let kind = r.u8()?;
                let item_id = r.u32()?;
                r.u64()?;
                let count = r.u16()?;
                // ParseGroundItem stops at Count. Live ea1a68bd ground items carry four more bytes
                // (ItemBody's state, which kinds 2 and 3 have); the client never reads them.
                if matches!(kind, 2 | 3) && r.remaining() == 4 {
                    r.take(4)?;
                }
                r.end()?;
                if self.drops.len() >= MAX_ITEMS && !self.drops.contains_key(&id) {
                    return Err(NetError::Capacity);
                }
                let key = if let Some(key) = self.drops.get(&id) {
                    *key
                } else {
                    e::DropKey(self.next()?)
                };
                self.drops.insert(id, key);
                let kind = self.item_kind(kind, item_id)?;
                out.push(Fact::Drop(e::Drop {
                    key,
                    kind,
                    position: point,
                    quantity: count as u32,
                }));
            }
            34 => {
                r.u16()?;
                let ground = r.u16()?;
                let kind = r.u8()?;
                item_body(&mut r, kind, false)?;
                r.end()?;
                if let Some(key) = self.drops.remove(&ground) {
                    out.push(Fact::DropGone(key));
                }
                // The caption path does not insert inventory or prove a pickup action.
            }
            437 => {
                self.physical_uncertain = true;
                self.own = Some(r.u16()?);
                let point = e::Point {
                    x: r.u16()? as i32,
                    y: r.u16()? as i32,
                };
                r.end()?;
                self.point = Some(point);
                self.map = None;
                self.zone = None;
                self.stats_seen = false;
                self.actors.clear();
                self.drops.clear();
                self.pending = None;
                self.combat.clear();
            }
            45 => {
                self.dig_active = true;
                out.push(Fact::DigAccepted);
            }
            371 => {
                let id = r.u16()?;
                let point = e::Point {
                    x: r.u16()? as i32,
                    y: r.u16()? as i32,
                };
                let branch = r.u8()?;
                if branch != 0 {
                    r.u16()?;
                } else {
                    r.take(3)?;
                }
                r.u16()?;
                r.u8()?;
                r.end()?;
                if Some(id) == self.own && self.dig_active {
                    out.push(Fact::DigSpot(point));
                }
            }
            50 => {
                let kind = r.u8()?;
                r.u16()?;
                let item = r.u32()?;
                r.end()?;
                let applies = self.dig_active || (11..=19).contains(&kind);
                if !applies {
                    return Ok(out);
                }
                self.dig_active = false;
                match kind {
                    2 | 9 => out.push(Fact::Outcome(e::Outcome::DigEnded(e::DigEnd::Empty))),
                    3 => {
                        let item = self.item_kind(1, item)?;
                        out.push(Fact::Outcome(e::Outcome::DigEnded(
                            e::DigEnd::ItemReported(item),
                        )));
                    }
                    4 | 6 => out.push(Fact::Outcome(e::Outcome::Rejected {
                        subject: e::Subject::Dig,
                        reason: e::Rejection::Resources,
                    })),
                    7 => out.push(Fact::Outcome(e::Outcome::Rejected {
                        subject: e::Subject::Dig,
                        reason: e::Rejection::Forbidden,
                    })),
                    _ => {}
                }
            }
            _ => {}
        }
        let correlated = self.combat.observe(p, self.own)?;
        let completed = !correlated.is_empty();
        for result in correlated {
            let key = match result.outcome {
                e::Outcome::OwnHit(key) | e::Outcome::OwnKill(key) => key,
                _ => return Err(NetError::Malformed),
            };
            if self.actors.values().any(|a| a.key == key) {
                if let Some(pending) = &mut self.pending
                    && matches!(pending.action,e::Action::BasicAttack(target) if target==key)
                {
                    pending.outcome = true;
                }
                if self.correlated.len() >= 256 {
                    return Err(NetError::Capacity);
                }
                self.correlated.push(DeferredFacts {
                    mark: result.mark,
                    received: result.received,
                    facts: vec![Fact::Outcome(result.outcome)],
                });
            }
        }
        if completed {
            self.combat.clear();
        }
        if self.pending.as_ref().is_some_and(|p| p.ended && p.outcome) {
            self.pending = None;
            self.combat.clear();
        }
        if p.opcode != 233 && old_maxima != [self.maximum(9), self.maximum(3)] {
            let mut facts = Vec::new();
            let mut original = None;
            for (value, stat, hp) in [(self.current_hp, 9, true), (self.current_mp, 3, false)] {
                if let Some((current, received)) = value {
                    original = Some(
                        original.map_or(received, |old: std::time::Instant| old.min(received)),
                    );
                    let gauge = resources::composite(current, self.maximum(stat));
                    facts.push(if hp { Fact::Hp(gauge) } else { Fact::Mp(gauge) });
                }
            }
            if let Some(received) = original {
                self.correlated.push(DeferredFacts {
                    mark: p.mark,
                    received,
                    facts,
                });
            }
        }
        if matches!(
            p.opcode,
            233 | 82 | 85 | 561 | 160 | 770 | 262 | 246 | 260 | 843 | 68
        ) {
            self.readiness.equipment_references_resolved =
                self.stats_seen && self.missing_slot_refs.iter().all(|uid| *uid == 0);
            out.push(self.drill_fact());
        }
        self.readiness.own_baseline_seen = self.baseline();
        self.readiness.hp_maximum_known = self.maximum(9).is_some();
        self.readiness.mp_maximum_known = self.maximum(3).is_some();
        self.readiness.weight_maximum_known = self.maximum(6).is_some();
        self.readiness.missing_gun = self.gun_prerequisite();
        self.readiness.missing_hp_potion = self.potion_prerequisite();
        self.track_carried(p, stray, old_weight_maximum, &mut out)?;
        Ok(out)
    }
    fn gun_prerequisite(&self) -> Option<MissingPrerequisite> {
        use MissingPrerequisite as M;
        if !self.stats_seen {
            return Some(M::OwnBaseline);
        }
        let Some(data) = &self.definitions else {
            return Some(M::Definitions);
        };
        if !self.readiness.equipment_references_resolved {
            return Some(M::Equipment);
        }
        let Some(gun) = self.items.get(&self.slots[1]) else {
            return Some(M::GunProfile);
        };
        let Some(param) = data.gun_params.get(&gun.id) else {
            return Some(M::GunProfile);
        };
        if param.kind < 0
            || param.speed <= 0
            || param.range <= 0
            || param.start < 0
            || param.min_range < 0
            || param.max_range < param.min_range
        {
            return Some(M::GunProfile);
        }
        if self.gun_key.is_none() {
            return Some(M::GunKey);
        }
        let Some(ammo) = self.items.get(&self.slots[20]).filter(|i| i.count > 0) else {
            return Some(M::GunAmmo);
        };
        let Some(info) = data.items.get(&ammo.id).filter(|i| i.kind == 29) else {
            return Some(M::GunAmmo);
        };
        let _infinite_local_ammo = info.flags_ex & 64 != 0;
        if self.learned.is_none() {
            return Some(M::GunSkills);
        }
        Some(M::GunEffectState)
    }
    fn potion_prerequisite(&self) -> Option<MissingPrerequisite> {
        use MissingPrerequisite as M;
        let Some(data) = &self.definitions else {
            return Some(M::Definitions);
        };
        let Some(profile) = &self.profile else {
            return Some(M::OwnBaseline);
        };
        Some(
            self.items
                .values()
                .rev()
                .find_map(|item| potion_reason(data, profile, item))
                .unwrap_or(M::PotionInventory),
        )
    }
    pub(crate) fn item_label(&self, kind: e::ItemKind) -> Option<e::DisplayText> {
        Some(self.item_descriptor(kind)?.label)
    }
    pub(crate) fn item_descriptor(&self, kind: e::ItemKind) -> Option<crate::ItemDescriptor> {
        let id = self
            .kinds
            .iter()
            .find_map(|((_, id), key)| (*key == kind).then_some(id))?;
        self.definitions.as_ref()?.descriptor(*id)
    }
    pub(crate) fn take_item_descriptors(&mut self) -> Vec<(e::ItemKind, crate::ItemDescriptor)> {
        std::mem::take(&mut self.item_descriptors)
    }
    fn drill_fact(&self) -> Fact {
        let drill = if !self.stats_seen || !self.readiness.equipment_references_resolved {
            e::Drill::Unknown
        } else if self.slots[7] == 0 {
            e::Drill::Absent
        } else if let (Some(data), Some(item)) = (&self.definitions, self.items.get(&self.slots[7]))
        {
            match data.item_types.get(&item.id) {
                Some((6, _)) => match data.drills.get(&item.id) {
                    Some(p) if p.limit > 0 => e::Drill::Equipped {
                        life: item.count as u32,
                        depth: p.limit as u32,
                    },
                    _ => e::Drill::Unknown,
                },
                Some(_) => e::Drill::Absent,
                None => e::Drill::Unknown,
            }
        } else {
            e::Drill::Unknown
        };
        Fact::Drill(drill)
    }
    fn put(&mut self, item: ItemBody, out: &mut Vec<Fact>) -> Result<()> {
        if !matches!(item.kind, 1 | 2) {
            return Ok(());
        }
        if self.items.len() >= MAX_ITEMS && !self.items.contains_key(&item.uid) {
            return Err(NetError::Capacity);
        }
        let key = if let Some(old) = self.items.get(&item.uid) {
            old.key
        } else {
            e::StackKey(self.next()?)
        };
        let kind = self.item_kind(item.kind, item.id)?;
        out.push(Fact::Inventory(e::Inventory::Put(e::Stack {
            key,
            kind,
            quantity: if item.kind == 1 { item.count as u32 } else { 1 },
        })));
        self.items.insert(
            item.uid,
            Item {
                key,
                count: item.count,
                state: item.state,
                id: item.id,
                wire_kind: item.kind,
                options: item.options,
            },
        );
        Ok(())
    }
    /// Returns whether a carried row named a UID absent from the cache.
    fn change(&mut self, op: u8, item: ItemBody, out: &mut Vec<Fact>) -> Result<bool> {
        let stray = matches!(op, 1 | 3 | 5 | 7)
            && matches!(item.kind, 1 | 2)
            && !self.items.contains_key(&item.uid);
        match op {
            2 | 4 | 6 | 8 => self.put(item, out)?,
            3 | 5 | 7 => {
                if let Some(old) = self.items.remove(&item.uid) {
                    out.push(Fact::Inventory(e::Inventory::Remove(old.key)));
                }
                for slot in &mut self.slots {
                    if *slot == item.uid {
                        *slot = 0;
                    }
                }
            }
            1 => {
                if let Some(old) = self.items.get_mut(&item.uid) {
                    if item.kind == 2 {
                        old.state = item.state;
                    } else {
                        old.count = item.count;
                        out.push(Fact::Inventory(e::Inventory::Quantity(
                            old.key,
                            item.count as u32,
                        )));
                    }
                }
            }
            _ => return Err(NetError::Unsupported),
        }
        Ok(stray)
    }
}
/// Why a cached stack is not yet a proven usable HP potion, or None when it is no candidate.
fn potion_reason(
    data: &GameDefinitions,
    profile: &e::Profile,
    item: &Item,
) -> Option<MissingPrerequisite> {
    use MissingPrerequisite as M;
    if item.count == 0 || item.wire_kind != 1 {
        return None;
    }
    let info = data.items.get(&item.id)?;
    if info.flags & 2 == 0
        || (info.hp <= 0 && info.hp_rate <= 0.0)
        || i32::from(profile.level) < i32::from(info.min_level as i16)
    {
        return None;
    }
    Some(if info.class == 2 && item.options.is_none() {
        M::PotionInstanceOptions
    } else if info.class == 2
        && item
            .options
            .as_ref()
            .is_some_and(|rows| rows.iter().any(|r| r[0] == 13))
    {
        M::PotionInstanceOverrides
    } else if info.exist_type != 0 || info.life != 0 || info.ftype != 0 {
        M::PotionExpiry
    } else if data.normal_form_effects.contains(&info.effect) {
        M::PotionNormalForm
    } else {
        M::PotionUseState
    })
}
fn item_body(r: &mut Reader<'_>, kind: u8, options: bool) -> Result<ItemBody> {
    if !matches!(kind, 1..=3) {
        return Err(NetError::Unsupported);
    }
    let id = r.u32()?;
    let uid = r.u64()?;
    let count = r.u16()?;
    let mut state = [0; 4];
    if matches!(kind, 2 | 3) {
        state.copy_from_slice(r.take(4)?);
    }
    let mut parsed_options = None;
    if options && kind == 2 {
        let count = r.u8()? as usize;
        let mut rows = Vec::with_capacity(count);
        for _ in 0..count {
            rows.push([r.u32()?, r.u32()?, r.u32()?]);
        }
        parsed_options = Some(rows);
    }
    Ok(ItemBody {
        kind,
        id,
        uid,
        count,
        state,
        options: parsed_options,
    })
}
struct Appearance {
    id: u16,
    kind: u16,
    species: u16,
    label: String,
    point: e::Point,
    target: Option<u16>,
    effects: bool,
}
fn appearance(b: &[u8]) -> Result<Appearance> {
    let mut r = Reader::new(b);
    let id = r.u16()?;
    let kind = r.u16()?;
    let species = r.u16()?;
    r.take(8)?;
    let point = e::Point {
        x: r.u16()? as i32,
        y: r.u16()? as i32,
    };
    r.take(4)?;
    let label = r.cstr()?;
    r.take(7)?;
    let effects = effects(&mut r)?;
    r.u32()?;
    let mut target = None;
    match kind {
        1 => player(&mut r)?,
        2 => {
            r.take(6)?;
        }
        3 => {
            target = Some(r.u16()?);
            // Live ea1a68bd monsters carry one trailing byte; the client stops after Target.
            if r.remaining() == 1 {
                r.u8()?;
            }
        }
        4 | 16 => {
            r.u16()?;
            if kind == 16 && r.remaining() > 0 {
                r.take(14)?;
            }
        }
        _ => {}
    }
    r.end()?;
    Ok(Appearance {
        id,
        kind,
        species,
        label: if label.is_empty() {
            "Unnamed actor".into()
        } else {
            label
        },
        point,
        target,
        effects,
    })
}
fn effects(r: &mut Reader<'_>) -> Result<bool> {
    if r.u8()? == 0 {
        return Ok(false);
    }
    let mut seen = false;
    for _ in 0..8 {
        let n = r.u8()?;
        for _ in 0..n {
            seen = true;
            let key = r.u16()?;
            r.take(7)?;
            if key == 1425 {
                let n = r.u16()? as usize;
                r.take(n * 4)?;
                let m = r.u32()? as usize;
                let bytes = m.checked_mul(2).ok_or(NetError::Capacity)?;
                r.take(bytes)?;
                if n != 0 {
                    r.u16()?;
                }
                continue;
            }
            let descriptors: &[u8] = match key {
                1118 | 1476 | 1487 | 1524 | 1525 | 1526 | 1527 | 1529 | 1531 | 1534 | 1537
                | 1540 | 1541 | 1542 | 1543 | 1545 | 1547 => &[2],
                1153 => &[0, 3],
                1186 => &[4, 2],
                1192 | 1267 | 1369 | 1371 | 1373 | 1375 | 1377 => &[1, 4],
                1208 | 1248 | 1390 | 1494 => &[0],
                1295 | 1380 => &[4, 1],
                1309 => &[1, 1],
                1316 | 1353 | 1358 => &[1, 4, 2],
                1333 | 1361 | 1386 | 1394 | 1533 => &[2, 2],
                1357 => &[4, 1, 2],
                1388 => &[5, 5],
                1423 | 1483 => &[2, 2, 2],
                1479 | 1492 => &[1, 2, 2],
                // EsBlock.Read gives a type outside the compiled table an empty body. The entry
                // still marks the actor as affected, so it is never classified or targeted.
                _ => &[],
            };
            for &d in descriptors {
                match d {
                    5 => {
                        r.cstr()?;
                    }
                    0 => {
                        r.take(1)?;
                    }
                    1 => {
                        r.take(2)?;
                    }
                    _ => {
                        r.take(4)?;
                    }
                }
            }
        }
    }
    Ok(seen)
}
fn player(r: &mut Reader<'_>) -> Result<()> {
    r.take(5)?;
    // ReadPlayerBody has no branch for mask bit 128.
    let mask = r.u8()?;
    for bit in [1, 2, 4, 8, 16] {
        if mask & bit != 0 {
            if bit == 8 {
                r.u8()?;
            }
            r.cstr()?;
        }
    }
    for bits in [
        &[
            1u32, 2, 4, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536, 262144,
            524288,
        ][..],
        &[1u32, 2, 4, 4096, 8192, 524288][..],
    ] {
        // The client reads one value per table bit that is set; other bits carry no value.
        let m = r.u32()?;
        for bit in bits {
            if m & bit != 0 {
                r.u32()?;
            }
        }
    }
    r.u8()?;
    r.cstr()?;
    r.cstr()?;
    r.take(8)?;
    if mask & 32 != 0 {
        let n = r.u32()? as usize;
        if n >= 0x20000000 {
            return Err(NetError::Capacity);
        }
        r.take(n * 4)?;
    }
    r.take(4)?;
    if mask & 64 != 0 {
        r.cstr()?;
    }
    let title = r.u16()?;
    r.take(11)?;
    if matches!(title, 4 | 6) {
        r.cstr()?;
    }
    r.u8()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroizing;
    fn packet(opcode: u16, b: Vec<u8>) -> Packet {
        Packet {
            opcode,
            aux: 0,
            payload: Zeroizing::new(b),
            mark: 1,
            received: std::time::Instant::now(),
        }
    }
    fn item(kind: u8, uid: u64, count: u16) -> Vec<u8> {
        let mut b = vec![kind];
        b.extend(7u32.to_le_bytes());
        b.extend(uid.to_le_bytes());
        b.extend(count.to_le_bytes());
        if kind != 1 {
            b.extend([0; 4]);
        }
        b
    }
    #[test]
    fn inventory_absent_empty_merge_and_absolute_delta() {
        let mut d = Decoder::new();
        assert!(!d.readiness.inventory_list_seen);
        assert!(
            matches!(d.apply(&packet(231,vec![0,0])).unwrap().as_slice(),[Fact::InventorySnapshot(items)] if items.is_empty())
        );
        assert!(d.readiness.inventory_list_seen);
        for uid in [42u64, 43] {
            let mut b = vec![1, 0];
            b.extend(item(1, uid, 5));
            d.apply(&packet(231, b)).unwrap();
        }
        assert_eq!(d.items.len(), 2);
        let mut b = vec![1];
        b.extend(item(1, 42, 2));
        d.apply(&packet(561, b)).unwrap();
        assert_eq!(d.items[&42].count, 2);
        let mut b = vec![1];
        b.extend(item(1, 999, 8));
        d.apply(&packet(561, b)).unwrap();
        assert!(!d.items.contains_key(&999));
        let mut b = vec![3];
        b.extend(item(1, 42, 0));
        d.apply(&packet(561, b)).unwrap();
        assert!(!d.items.contains_key(&42));
    }
    #[test]
    fn wire_kind_options_and_drill_life_are_preserved_separately_from_pieces() {
        let mut d = Decoder::new();
        let mut bytes = vec![1, 0];
        bytes.extend(item(2, 42, 300));
        bytes.push(2);
        let rows = [[13u32, 0x1234, 0x80000000], [90, 1, 2]];
        for row in rows {
            for value in row {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let facts = d.apply(&packet(231, bytes)).unwrap();
        assert!(
            matches!(facts.as_slice(),[Fact::InventorySnapshot(stacks)] if stacks.len()==1&&stacks[0].quantity==1)
        );
        assert_eq!(d.items[&42].wire_kind, 2);
        assert_eq!(d.items[&42].count, 300);
        assert_eq!(d.items[&42].options.as_deref(), Some(rows.as_slice()));
        let mut change = vec![1];
        change.extend(item(2, 42, 999));
        let len = change.len();
        change[len - 4..].copy_from_slice(&[1, 2, 3, 4]);
        d.apply(&packet(561, change)).unwrap();
        assert_eq!(d.items[&42].count, 300);
        assert_eq!(d.items[&42].state, [1, 2, 3, 4]);
        assert_eq!(d.items[&42].options.as_deref(), Some(rows.as_slice()));
    }
    #[test]
    fn complete_status_inputs_without_game_parts_leave_maxima_unknown() {
        let mut definitions = crate::test_support::definitions(&[]);
        std::sync::Arc::get_mut(&mut definitions)
            .unwrap()
            .player_levels = vec![1000.0; crate::resources::PLAYER_LEVELS];
        let mut d = Decoder::new();
        d.set_definitions(Some(definitions));
        d.startup_cutoff(1);
        d.apply(&packet(233, vec![0; 251])).unwrap();
        let known = |d: &Decoder| {
            let r = &d.readiness;
            [
                r.hp_maximum_known,
                r.mp_maximum_known,
                r.weight_maximum_known,
            ]
        };
        assert_eq!(known(&d), [false; 3]);
        let mut total = packet(282, vec![1, 9, 100, 0, 0, 0]);
        total.mark = 2;
        d.apply(&total).unwrap();
        assert_eq!(known(&d), [true, false, false]);
    }
    #[test]
    fn resource_composites_keep_old_current_time_and_never_guess_a_reduction() {
        let mut d = Decoder::new();
        let original = std::time::Instant::now() - std::time::Duration::from_secs(2);
        let mut hp = packet(36, 77u32.to_le_bytes().to_vec());
        hp.received = original;
        hp.mark = 10;
        d.apply(&hp).unwrap();
        for (mark, max) in [(11, 100i32), (12, 60i32)] {
            let mut bytes = vec![1, 9];
            bytes.extend_from_slice(&max.to_le_bytes());
            let mut total = packet(282, bytes);
            total.mark = mark;
            assert!(d.apply(&total).unwrap().is_empty());
            let delayed = d.take_correlated();
            assert_eq!(delayed.len(), 1);
            assert_eq!(delayed[0].received, original);
            assert_eq!(delayed[0].mark, mark);
            assert!(
                matches!(&delayed[0].facts[0],Fact::Hp(gauge) if gauge.current==77&&gauge.maximum.map(|v|v.get())==if max==100 {Some(100)}else{None})
            );
        }
    }
    #[test]
    fn native_walk_and_pending_actions_require_own_source_evidence() {
        let mut d = Decoder::new();
        d.use_native_walk_policy();
        let mut clock = vec![0; 10];
        clock[4..6].copy_from_slice(&321u16.to_le_bytes());
        d.apply(&packet(230, clock)).unwrap();
        d.apply(&packet(233, vec![0; 251])).unwrap();
        let to = e::Point { x: 100, y: 200 };
        d.expect_action(e::Action::MoveTo(to), 10).unwrap();
        let mut payload = Vec::new();
        for n in [321u16, 0, 0, 100, 200] {
            payload.extend_from_slice(&n.to_le_bytes());
        }
        payload.push(2);
        let mut movement = packet(24, payload);
        movement.mark = 11;
        let facts = d.apply(&movement).unwrap();
        assert!(d.action_pending());
        assert!(!facts.iter().any(|f| matches!(f, Fact::Outcome(_))));
        let mut payload = Vec::new();
        for n in [321u16, 100, 200] {
            payload.extend_from_slice(&n.to_le_bytes());
        }
        payload.push(2);
        let mut stop = packet(25, payload);
        stop.mark = 12;
        assert!(
            d.apply(&stop)
                .unwrap()
                .iter()
                .any(|f| matches!(f,Fact::Outcome(e::Outcome::Arrived(p)) if *p==to))
        );
        assert!(!d.action_pending());
        assert_eq!(
            *d.move_payload(e::Point { x: 200, y: 300 })
                .unwrap()
                .0
                .last()
                .unwrap(),
            1
        );
        let mut monster = vec![0; 38];
        monster[..2].copy_from_slice(&9u16.to_le_bytes());
        monster[2..4].copy_from_slice(&3u16.to_le_bytes());
        monster[22] = b'M';
        d.apply(&packet(7, monster)).unwrap();
        let key = d.actors[&9].key;
        d.expect_action(e::Action::BasicAttack(key), 20).unwrap();
        let payload = [321u16, 0, 9, 1, 2, 3, 4]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect();
        let mut strike = packet(31, payload);
        strike.aux = 7;
        strike.mark = 21;
        d.apply(&strike).unwrap();
        let mut payload = 9u16.to_le_bytes().to_vec();
        payload.extend_from_slice(&3u32.to_le_bytes());
        payload.extend_from_slice(&[0; 20]);
        let mut hit = packet(42, payload);
        hit.aux = 7;
        hit.mark = 22;
        d.apply(&hit).unwrap();
        let mut end = packet(32, 321u16.to_le_bytes().to_vec());
        end.mark = 23;
        d.apply(&end).unwrap();
        assert!(d.action_pending());
        assert!(d.take_correlated().is_empty());
        let mut boundary = packet(1951, vec![]);
        boundary.aux = 7;
        boundary.mark = 24;
        d.apply(&boundary).unwrap();
        let correlated = d.take_correlated();
        assert_eq!(correlated.len(), 1);
        assert_eq!(correlated[0].mark, 22);
        assert!(
            matches!(correlated[0].facts.as_slice(),[Fact::Outcome(e::Outcome::OwnHit(actual))] if *actual==key)
        );
        assert!(!d.action_pending());
    }
    #[test]
    fn malformed_effect_and_player_branches() {
        // One group-0 entry of a type outside the compiled table: nine-byte prefix, empty body.
        let unknown = [1, 1, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut r = Reader::new(&unknown);
        assert!(effects(&mut r).unwrap());
        assert!(r.end().is_ok());
        let mut extra = unknown.to_vec();
        extra.push(0);
        let mut r = Reader::new(&extra);
        effects(&mut r).unwrap();
        assert!(r.end().is_err());
        assert!(effects(&mut Reader::new(&unknown[..10])).is_err());
        let mut player_bytes = vec![0; 43];
        assert!(player(&mut Reader::new(&player_bytes)).is_ok());
        player_bytes[5] = 128;
        let mut r = Reader::new(&player_bytes);
        player(&mut r).unwrap();
        assert!(r.end().is_ok());
        // A_mask bit 8 is outside the slot table, so it adds no value.
        player_bytes[6..10].copy_from_slice(&8u32.to_le_bytes());
        let mut r = Reader::new(&player_bytes);
        player(&mut r).unwrap();
        assert!(r.end().is_ok());
        assert!(item_body(&mut Reader::new(&[]), 9, false).is_err());
    }
    #[test]
    fn an_affected_monster_with_an_unclassified_effect_is_tracked_but_never_targeted() {
        let mut monster = vec![0; 38];
        monster[22] = b'S';
        monster[0..2].copy_from_slice(&321u16.to_le_bytes());
        monster[2..4].copy_from_slice(&3u16.to_le_bytes());
        monster[36..38].copy_from_slice(&0u16.to_le_bytes());
        // Replace the empty effect block (offset 31) with one unclassified entry.
        let mut block = vec![1, 1, 7, 0];
        block.extend([0; 7]);
        block.extend([0; 7]);
        monster.splice(31..32, block);
        let mut d = Decoder::new();
        let facts = d.apply(&packet(8, monster)).unwrap();
        assert!(matches!(facts.as_slice(), [Fact::Entity(_)]));
        let key = d.actors[&321].key;
        assert!(d.raw_target(key).is_err());
    }
    #[test]
    fn all_startup_waits_and_unknown_inventory() {
        let mut d = Decoder::new();
        let mut b = vec![0; 251];
        b[214..216].copy_from_slice(&42u16.to_le_bytes());
        // A reference missing at its original replay point remains explicitly unresolved.
        b[..8].copy_from_slice(&42u64.to_le_bytes());
        d.apply(&packet(233, b)).unwrap();
        assert!(!d.baseline());
        assert!(!d.readiness.equipment_references_resolved);
        assert!(!d.readiness.inventory_list_seen);
        let mut profile = vec![0; 69];
        profile[17] = b'S';
        d.apply(&packet(679, profile)).unwrap();
        assert!(!d.baseline());
        d.apply(&packet(230, vec![0; 10])).unwrap();
        assert!(d.baseline());
    }
    #[test]
    fn live_monster_and_removal_tails_allow_exactly_one_trailing_byte() {
        let mut monster = vec![0; 38];
        monster[22] = b'S';
        monster[0..2].copy_from_slice(&321u16.to_le_bytes());
        monster[2..4].copy_from_slice(&3u16.to_le_bytes());
        monster[36..38].copy_from_slice(&77u16.to_le_bytes());
        let mut d = Decoder::new();
        let mut tail = monster.clone();
        tail.push(0);
        d.apply(&packet(8, tail.clone())).unwrap();
        assert_eq!(d.actors[&321].target, Some(77));
        tail.push(0);
        assert!(d.apply(&packet(8, tail)).is_err());
        let mut owned = monster;
        owned[2..4].copy_from_slice(&4u16.to_le_bytes());
        owned.push(0);
        assert!(d.apply(&packet(8, owned)).is_err());
        let mut removal = 321u16.to_le_bytes().to_vec();
        removal.push(1);
        assert!(d.apply(&packet(6, removal.clone())).is_err());
        let facts = d.apply(&packet(15, removal.clone())).unwrap();
        assert!(matches!(facts.as_slice(), [Fact::EntityGone(_)]));
        assert!(!d.actors.contains_key(&321));
        removal.push(1);
        assert!(d.apply(&packet(15, removal)).is_err());
    }
    #[test]
    fn ground_items_allow_item_state_only_for_kinds_two_and_three() {
        let ground = |kind: u8, extra: usize| {
            let mut b = vec![0; 26];
            b[0..2].copy_from_slice(&5u16.to_le_bytes());
            b[11] = kind;
            b[12..16].copy_from_slice(&7u32.to_le_bytes());
            b[24..26].copy_from_slice(&1u16.to_le_bytes());
            b.extend(vec![0; extra]);
            packet(10, b)
        };
        let mut d = Decoder::new();
        for kind in [1u8, 2, 3] {
            assert!(d.apply(&ground(kind, 0)).is_ok(), "kind {kind}");
        }
        assert!(d.apply(&ground(2, 4)).is_ok());
        assert!(d.apply(&ground(3, 4)).is_ok());
        assert!(d.apply(&ground(1, 4)).is_err());
        assert!(d.apply(&ground(2, 3)).is_err());
        assert!(d.apply(&ground(2, 5)).is_err());
    }
    #[test]
    fn damage_accepts_every_length_the_client_parses() {
        let mut d = Decoder::new();
        for aux in [0, 1] {
            let damage = |len| Packet {
                aux,
                ..packet(42, vec![0; len])
            };
            for len in 0..26 {
                assert!(d.apply(&damage(len)).is_err(), "{len} bytes, aux {aux}");
            }
            for len in [26, 27, 28, 29, 30, 31, crate::codec::MAX_PAYLOAD] {
                assert!(d.apply(&damage(len)).is_ok(), "{len} bytes, aux {aux}");
            }
        }
    }
    #[test]
    fn raw_incarnations_are_not_reused_and_faint_is_not_kill() {
        let mut b = vec![0; 38];
        b[22] = b'S';
        b[0..2].copy_from_slice(&321u16.to_le_bytes());
        b[2..4].copy_from_slice(&3u16.to_le_bytes());
        let mut d = Decoder::new();
        d.apply(&packet(7, b.clone())).unwrap();
        let old = d.actors[&321].key;
        d.apply(&packet(7, b)).unwrap();
        assert!(d.actors[&321].key != old);
        let facts = d.apply(&packet(43, 321u16.to_le_bytes().to_vec())).unwrap();
        assert!(
            facts
                .iter()
                .all(|f| !matches!(f, Fact::Outcome(e::Outcome::OwnKill(_))))
        );
    }
}
