use crate::{
    NetError, Result,
    codec::Packet,
    definitions::GameDefinitions,
    parts::{self, Cached},
    resources,
    wire::Reader,
};
use opentrickster_engine::{self as e, Fact};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU32,
};

const MAX_ACTORS: usize = 4096;
const MAX_ITEMS: usize = 8192;
/// Bound on 518/521 held for the gun constructor's replay; more makes loaded ammo unknown.
const MAX_START_SPECIALS: usize = 64;
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
/// The page bridge forwards each of them, header-only except 151, whose UID `GameParts` reads.
const UNDECODED_INVENTORY: &[u16] = &[586, 590, 1152, 587, 599, 601, 261, 86, 35, 151];
/// `MoveSpeeds` defaults (f26839) in px/s. A zone record can replace them without a frame.
const WALK_PX_S: f64 = 115.0;
const RUN_PX_S: f64 = 184.0;

/// The own walk as the client runs it. After an echo the client walks to the server's
/// destination locally and sends nothing when its path empties.
#[derive(Clone, Copy)]
enum Walk {
    /// An own move request that no own move frame has answered. The gun counts it as walking.
    Requested,
    /// An own 24/26, and the MoveTo target this echo accepted.
    Echoed {
        to: e::Point,
        target: Option<e::Point>,
        mark: u64,
        /// When the client would end the walk. This is an estimate, never proof.
        due: std::time::Instant,
    },
}

/// The client's walk time on a straight path (`FollowServerMove` f38516, `Update` f26885),
/// plus an unproven allowance for its grid path: 9/8 of the length, 32 px and two frames.
fn walk_time(
    actor: Option<e::Point>,
    from: e::Point,
    to: e::Point,
    mode: u8,
    speed: Option<u16>,
) -> std::time::Duration {
    let distance = |a: e::Point, b: e::Point| f64::from(a.x - b.x).hypot(f64::from(a.y - b.y));
    // Beyond 320 px the client first places the actor on the echo's start.
    let actor = actor
        .filter(|a| distance(*a, from) <= 320.0)
        .unwrap_or(from);
    let length = distance(actor, from) + distance(from, to);
    let speed = speed.map_or(if mode == 2 { RUN_PX_S } else { WALK_PX_S }, f64::from);
    let ms = ((length * 9.0 / 8.0 + 32.0) * 1000.0 / speed).ceil() as u64;
    std::time::Duration::from_millis(ms + 32)
}

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
/// `Combat.U16At` (f10802) at offset 0: a byte past the end reads as zero, so it never fails.
fn padded_u16(b: &[u8]) -> u16 {
    let byte = |i: usize| b.get(i).copied().map_or(0, u16::from);
    byte(0) | byte(1) << 8
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
    /// An own move frame after the request already answered this MoveTo.
    moved: bool,
}
pub(crate) struct DeferredFacts {
    pub(crate) mark: u64,
    pub(crate) received: std::time::Instant,
    pub(crate) facts: Vec<Fact>,
}
/// Carried-inventory proof for one game session under the startup cutoff H.
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq))]
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
    /// The zone is unknown or its Type hides the gun (`MayShow`).
    GunZone,
    /// Camp entry disarmed the gun until camp exit.
    GunCamp,
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
    parts: parts::Parts,
    current_hp: Option<(u32, std::time::Instant)>,
    current_mp: Option<(u32, std::time::Instant)>,
    gun: crate::gun::Gun,
    load_speed: crate::gun::LoadSpeed,
    /// The client's `GunFlowing` exists from EnterWorld; inputs before that are replayed once.
    gun_live: bool,
    /// `(LoadsWhileMoving, KeepsLoadOnHit)`: the 271 that construction reads, then live 523.
    gun_skills: Option<(bool, bool)>,
    /// The flags of the latest 271; `LearnedAtLogin` (f24376) reads StartSet 271 only, so
    /// construction copies them once and a later list changes nothing.
    start_skills: Option<(bool, bool)>,
    /// 518 (true) and 521 (false) UIDs before construction, in arrival order.
    start_special: Vec<(bool, u64)>,
    /// A special-slot input named ammo that this decoder could not resolve.
    ammo_unknown: bool,
    /// The controller's own slot UIDs: zero at construction, then only live 82/85.
    gun_slots: [u64; 23],
    /// 82/85 arrived before construction; the client's replayed equipment omits them.
    gun_diverged: bool,
    walk: Option<Walk>,
    /// Own `Speed` from 472 in px/s. None uses the mode's default.
    speed: Option<u16>,
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
            parts: Default::default(),
            current_hp: None,
            current_mp: None,
            gun: Default::default(),
            load_speed: Default::default(),
            gun_live: false,
            gun_skills: None,
            start_skills: None,
            start_special: Vec::new(),
            ammo_unknown: false,
            gun_slots: [0; 23],
            gun_diverged: false,
            walk: None,
            speed: None,
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
        let data = self.definitions.as_deref();
        let game_parts = data.and_then(|data| {
            let row = |uid| {
                self.items.get(&uid).map(|item: &Item| parts::Row {
                    id: item.id,
                    wire_kind: item.wire_kind,
                    options: item.options.as_deref(),
                })
            };
            self.parts.value(stat, row, data)
        });
        let player = data.map(|d| d.player_levels.as_slice());
        self.status.maximum(stat, player, game_parts)
    }
    /// A page bound after this decoder's startup began: StartSet frames may have reached
    /// neither this decoder nor the page.
    pub(crate) fn lose_game_parts(&mut self) {
        self.parts.lose();
        self.note_maxima();
    }
    #[cfg(test)]
    pub(crate) fn maxima(&self) -> [Option<u32>; 3] {
        [9, 3, 6].map(|stat| self.maximum(stat).map(NonZeroU32::get))
    }
    /// The `CharacterStatus` and `GameParts` inputs, and the cache that `Parts` reads.
    #[cfg(test)]
    pub(crate) fn derived_state(&self) -> (resources::Status, parts::Parts, BTreeMap<u64, Cached>) {
        let data = self.definitions.as_deref();
        let cache = self
            .items
            .keys()
            .map(|uid| (*uid, cached(&self.items, data, *uid)));
        (self.status.clone(), self.parts.clone(), cache.collect())
    }
    /// Every input of derived maxima and carried weight.
    #[cfg(test)]
    pub(crate) fn derived_inputs(&self) -> impl PartialEq + std::fmt::Debug + use<> {
        let items = self.items.iter().map(|(uid, item)| {
            let options = item.options.clone();
            (
                *uid,
                item.id,
                item.wire_kind,
                item.count,
                item.state,
                options,
            )
        });
        let (status, parts, _) = self.derived_state();
        let carried = self.carried;
        (status, parts, items.collect::<Vec<_>>(), carried, self.own)
    }
    fn note_maxima(&mut self) {
        self.readiness.hp_maximum_known = self.maximum(9).is_some();
        self.readiness.mp_maximum_known = self.maximum(3).is_some();
        self.readiness.weight_maximum_known = self.maximum(6).is_some();
    }
    /// Whether a row would replace Net's cached one with the same ID, wire kind and options.
    fn same_row(&self, item: &ItemBody) -> bool {
        self.items.get(&item.uid).is_some_and(|old| {
            old.id == item.id
                && old.wire_kind == item.kind
                && old.options.as_deref().unwrap_or(&[]) == item.options.as_deref().unwrap_or(&[])
        })
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
    /// `gun` marks a 477 shot. It has no attack-end packet, so its correlated outcome alone
    /// completes it.
    pub(crate) fn expect_action(&mut self, action: e::Action, mark: u64, gun: bool) -> Result<()> {
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
        // An echo answers only the request it followed. The assisted path arms no walk.
        if let Some(Walk::Echoed { target, .. }) = &mut self.walk {
            *target = None;
        }
        self.pending = Some(PendingAction {
            action,
            mark,
            ended: gun,
            outcome: false,
            moved: false,
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
        // A request no frame answered starts no walk. A later echo is the server's own walk.
        if matches!(self.walk, Some(Walk::Requested)) {
            self.walk = None;
        }
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
        let live = !self.status.in_start_set(p.mark);
        self.status.observe(p)?;
        let b = &p.payload;
        let mut r = Reader::new(b);
        let mut out = Vec::new();
        let mut stray = false;
        let data = self.definitions.clone();
        let lookup = |items: &BTreeMap<u64, Item>, uid| cached(items, data.as_deref(), uid);
        if live {
            self.parts.finish_start(|uid| lookup(&self.items, uid));
        }
        self.parts.frame(p.opcode, b, live, data.as_deref(), |uid| {
            lookup(&self.items, uid)
        });
        match p.opcode {
            230 => {
                r.u32()?;
                let own = r.u16()?;
                let point = e::Point {
                    x: r.u16()? as i32,
                    y: r.u16()? as i32,
                };
                r.end()?;
                self.own = Some(own);
                self.parts.clock(own, live);
                self.point = Some(point);
                self.walk = None;
                self.speed = None;
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
                if !live {
                    self.parts
                        .start_slots(&self.slots, |uid| lookup(&self.items, uid));
                    // At H the StartSet closes here, so the resources below carry its maxima.
                    if self.status.closes_start_set(p.mark) {
                        self.parts.finish_start(|uid| lookup(&self.items, uid));
                    }
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
                    Fact::Hp(resources::gauge(hp, self.maximum(9))),
                    Fact::Mp(resources::gauge(mp, self.maximum(3))),
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
                self.gun_appearance(p.opcode, &parsed);
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
                // S15 also ends the effect whose key equals this prefix.
                if p.opcode == 15 {
                    self.load_speed.ended(id);
                }
                if Some(id) == self.own {
                    self.load_speed.own_lost();
                }
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
                // ParseMove (f11867) reads the server's start for 24 and 26 only.
                let from = if matches!(p.opcode, 24 | 26) {
                    Some(e::Point {
                        x: r.u16()? as i32,
                        y: r.u16()? as i32,
                    })
                } else {
                    None
                };
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
                    let actor = self.point.replace(point);
                    if let Some(map) = self.map {
                        out.push(Fact::Position { map, point });
                    }
                    // The first own move frame after a MoveTo request answers it.
                    let answered = self
                        .pending
                        .as_mut()
                        .and_then(|pending| match pending.action {
                            e::Action::MoveTo(target)
                                if p.mark > pending.mark && !pending.moved =>
                            {
                                pending.moved = true;
                                Some(target)
                            }
                            _ => None,
                        });
                    if let Some(from) = from {
                        // The destination is not an arrival: the client walks there locally.
                        self.walk = Some(Walk::Echoed {
                            to: point,
                            target: answered,
                            mark: p.mark,
                            due: p.received + walk_time(actor, from, point, mode, self.speed),
                        });
                    } else {
                        // The client applies an own stop at once (ApplyHeldStop f20917). Only a
                        // stop on the accepted destination, or on the target it answers, arrives.
                        let accepted = match self.walk.take() {
                            Some(Walk::Echoed { to, target, .. }) if to == point => target,
                            _ => None,
                        };
                        if let Some(target) = accepted.or(answered.filter(|t| *t == point))
                            && self.arrive(target)
                        {
                            out.push(Fact::Outcome(e::Outcome::Arrived { target, at: point }));
                        }
                    }
                }
                if let Some(a) = self.actors.get_mut(&id) {
                    a.point = point;
                }
                if let Some(f) = self.entity(id)? {
                    out.push(f);
                }
            }
            472 if self.native_walk => {
                // ParseMoveSpeed (f11868). Its handler (f38368) sets Speed to U8 × 4 px/s, or
                // clears it for zero. Only the native walk estimate reads it; the page does not
                // forward 472, because the assisted path ends a walk from the sample.
                let id = r.u16()?;
                let speed = r.u8()?;
                if Some(id) == self.own {
                    self.speed = (speed > 0).then(|| u16::from(speed) * 4);
                }
            }
            36 | 37 => {
                // Combat.I32At (f10803) needs four bytes and never checks the end.
                let value = (r.u32()? as i32).max(0) as u32;
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
            31 | 485 | 1438 => {
                // Combat.ParseSwing (f10796) reads the attacker and kind, then one position for
                // kind 3, or a target and two positions. Its later reads check what remains, and
                // it never checks the end. The correlator reads grouped strikes.
                r.u16()?;
                let kind = r.u16()?;
                r.take(if kind == 3 { 4 } else { 10 })?;
            }
            32 => {
                // Combat.ParseMessage (f10804) reads the actor with the padded U16At.
                let actor = padded_u16(b);
                if Some(actor) == self.own
                    && let Some(pending) = &mut self.pending
                    && p.mark > pending.mark
                    && matches!(pending.action, e::Action::BasicAttack(_))
                {
                    pending.ended = true;
                }
                if Some(actor) == self.own && self.gun_live && self.gun_worn() == Some(true) {
                    self.gun.shot_answered();
                }
            }
            512 => {
                let key = r.u16()?;
                r.end()?;
                // The listener exists only from construction; an earlier 512 is not replayed.
                if self.gun_live {
                    self.gun.on_key(key, self.gun_profile());
                }
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
                let skills = gun_skills(&learned);
                // The client's StartSet can end after our construction; its last 271 wins.
                if self.gun_live && self.gun_skills != Some(skills) {
                    self.gun_skills = None;
                }
                self.start_skills = Some(skills);
                self.learned = Some(learned);
            }
            523 => {
                let skill = r.u16()?;
                if let Some(learned) = &mut self.learned {
                    learned.insert(skill);
                }
                // OnLearned (f24407) also consults a skill-level lookup that is not modelled, so
                // a flag it could newly set becomes unknown.
                if self.gun_live
                    && let Some((moving, hit)) = self.gun_skills
                    && ((skill == crate::gun::MOVE_SKILL && !moving)
                        || (skill == crate::gun::HIT_SKILL && !hit))
                {
                    self.gun_skills = None;
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
                // Combat.ParseMessage (f10804) reads the target with the padded U16At and byte 2
                // only when present, so every length parses.
                let id = padded_u16(b);
                if Some(id) == self.own {
                    self.current_hp = Some((0, p.received));
                    out.push(Fact::Hp(resources::gauge(0, self.maximum(9))));
                    self.load_speed.own_lost();
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
                    if matches!(item.kind, 1 | 2) {
                        let same = self.same_row(&item);
                        self.parts
                            .list_row(item.uid, same, live, |uid| lookup(&self.items, uid));
                    }
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
                    let same = self.same_row(&item);
                    self.parts
                        .delta(p.opcode, op, item.kind, item.uid, same, live, |uid| {
                            lookup(&self.items, uid)
                        });
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
                if live {
                    self.parts
                        .equip(82, uid, slot, |uid| lookup(&self.items, uid));
                }
                if self.items.contains_key(&uid) {
                    self.slots[slot] = uid;
                    self.missing_slot_refs[slot] = 0;
                }
            }
            85 => {
                let uid = r.u64()?;
                let slot = r.u8()? as usize;
                r.end()?;
                if slot > 22 {
                    return Err(NetError::Malformed);
                }
                if live {
                    self.parts
                        .equip(85, uid, slot, |uid| lookup(&self.items, uid));
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
                let own = r.u16()?;
                let point = e::Point {
                    x: r.u16()? as i32,
                    y: r.u16()? as i32,
                };
                r.end()?;
                self.own = Some(own);
                self.parts.enter_zone(own, live);
                self.point = Some(point);
                self.walk = None;
                self.speed = None;
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
        self.gun_observe(p);
        if self.status.closes_start_set(p.mark) {
            self.parts.finish_start(|uid| lookup(&self.items, uid));
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
        self.note_maxima();
        self.readiness.missing_gun = self.gun_prerequisite();
        self.readiness.missing_hp_potion = self.potion_prerequisite();
        self.track_carried(p, stray, old_weight_maximum, &mut out)?;
        Ok(out)
    }
    /// The first unproven input of a native gun shot, or None when the controller is armed
    /// with every input known.
    pub(crate) fn gun_prerequisite(&self) -> Option<MissingPrerequisite> {
        use MissingPrerequisite as M;
        if !self.stats_seen || self.own.is_none() {
            return Some(M::OwnBaseline);
        }
        if self.definitions.is_none() {
            return Some(M::Definitions);
        }
        if !self.readiness.equipment_references_resolved
            || self.gun_worn().is_none()
            || self.gun_diverged
        {
            return Some(M::Equipment);
        }
        if self.gun_profile().is_none() {
            return Some(M::GunProfile);
        }
        if self.gun.key().is_none() {
            return Some(M::GunKey);
        }
        let ammo = self
            .items
            .get(&self.gun.loaded())
            .filter(|item| self.gun.effective(item.count) > 0)
            .and_then(|item| self.info(item.id))
            .is_some_and(|info| info.kind == 29);
        if self.ammo_unknown || !ammo {
            return Some(M::GunAmmo);
        }
        if self.gun_skills.is_none() {
            return Some(M::GunSkills);
        }
        if self.load_speed.value().is_none() {
            return Some(M::GunEffectState);
        }
        if self.zone_allows() != Some(true) {
            return Some(M::GunZone);
        }
        if self.gun.in_camp() {
            return Some(M::GunCamp);
        }
        // Wearing disarms until a fresh 512 arrives.
        if !self.gun.armed() {
            return Some(M::GunKey);
        }
        None
    }
    fn info(&self, id: u32) -> Option<&crate::definitions::ItemInfo> {
        self.definitions.as_ref()?.items.get(&id)
    }
    /// `Wear`'s test (f24375): ItemInfo Type 2 and SubType 54. None when unknown.
    fn is_gun_id(&self, id: u32) -> Option<bool> {
        let info = self.info(id)?;
        Some(info.kind == 2 && info.subtype == 54)
    }
    /// `GunWorn`/`HoldsGun` (f24382/f24383): SubType 54 in a configured gun slot.
    pub(crate) fn gun_worn(&self) -> Option<bool> {
        let data = self.definitions.as_ref()?;
        if !self.readiness.equipment_references_resolved {
            return None;
        }
        for &slot in &data.gun_slots {
            let uid = *self.slots.get(slot)?;
            if uid != 0 && self.info(self.items.get(&uid)?.id)?.subtype == 54 {
                return Some(true);
            }
        }
        Some(false)
    }
    /// The slot-1 item's `Item_GunParam2` row (`GunOn`, f24387), when usable.
    fn gun_profile(&self) -> Option<crate::definitions::GunParams> {
        let item = self.items.get(&self.slots[1])?;
        let param = *self.definitions.as_ref()?.gun_params.get(&item.id)?;
        (param.kind >= 0
            && param.speed > 0
            && param.range > 0
            && param.start >= 0
            && param.min_range >= 0
            && param.max_range >= param.min_range)
            .then_some(param)
    }
    /// `MayShow` (f24395): a known zone whose Type is not 0, 1 or 5.
    fn zone_allows(&self) -> Option<bool> {
        let (kind, _) = self.definitions.as_ref()?.zones.get(&self.zone?)?;
        Some(!matches!(kind, 0 | 1 | 5))
    }
    /// The `GunFlowing` constructor at EnterWorld (f24374): slot 20 runs the `SpecialOn`
    /// branch, other equipped items go through `Wear`, then every earlier 518/521 replays in
    /// arrival order.
    pub(crate) fn construct_gun(&mut self) {
        if std::mem::replace(&mut self.gun_live, true) {
            return;
        }
        self.gun_skills = self.start_skills;
        for slot in 0..self.slots.len() {
            let uid = self.slots[slot];
            if uid == 0 {
                continue;
            }
            let Some(id) = self.items.get(&uid).map(|item| item.id) else {
                continue;
            };
            if slot == 20 {
                self.special(true, uid);
            } else {
                self.wear(id, true);
            }
        }
        for (on, uid) in std::mem::take(&mut self.start_special) {
            self.special(on, uid);
        }
        self.readiness.missing_gun = self.gun_prerequisite();
    }
    /// `Wear(itemId, on)` (f24375). An unknown item type disarms without requesting a key.
    fn wear(&mut self, id: u32, on: bool) {
        match self.is_gun_id(id) {
            Some(true) => self.gun.wear(on),
            Some(false) => {}
            None => self.gun.wear(false),
        }
    }
    /// `Special(bytes, on)` (f24377): 518 runs `SpecialOn` (f24379), 521 unloads Type-29 ammo.
    /// A UID missing from the inventory changes nothing.
    fn special(&mut self, on: bool, uid: u64) {
        let Some(item) = self.items.get(&uid) else {
            return;
        };
        let Some(info) = self.info(item.id) else {
            self.ammo_unknown = true;
            return;
        };
        let kind = (info.kind, info.subtype);
        if on {
            self.gun.special_on(uid, kind);
        } else if kind.0 == 29 {
            self.gun.special_off();
        }
    }
    /// Empty 511 requests the controller made since the last call.
    pub(crate) fn take_key_requests(&mut self) -> u32 {
        self.gun.take_requests()
    }
    pub(crate) fn gun_awaits_key(&self) -> bool {
        self.gun_live && self.gun.awaiting_key()
    }
    fn gun_frame(&self) -> crate::gun::Frame {
        let (keeps_moving, keeps_on_hit) = self.gun_skills.unwrap_or((false, false));
        crate::gun::Frame {
            worn: self.gun_worn() == Some(true),
            moving: self.walk.is_some(),
            zone_allows: self.zone_allows() == Some(true),
            speed: self.load_speed.value(),
            keeps_moving,
            keeps_on_hit,
        }
    }
    /// One UI frame of `elapsed_ms` real milliseconds.
    pub(crate) fn gun_tick(&mut self, elapsed_ms: f64) {
        if self.gun_live {
            let frame = self.gun_frame();
            self.gun.advance(elapsed_ms, frame);
        }
    }
    /// Whether a gun ticket for `key` can wait for Ready: a valid target and an armed,
    /// fully known controller that reaches Ready without another received input.
    pub(crate) fn gun_waitable(&self, key: e::EntityKey) -> bool {
        // WorldClick rejects the own fainted state before its gun branch, so no shot can
        // leave while own HP is unknown or zero.
        self.current_hp.is_some_and(|(hp, _)| hp > 0)
            && self.raw_target(key).is_ok()
            && self.gun_prerequisite().is_none()
            && self.gun.waitable()
    }
    pub(crate) fn gun_ready(&self, key: e::EntityKey) -> bool {
        self.gun_waitable(key) && self.gun.ready()
    }
    /// The 477 payload of `Shoot`; the controller then waits for own firing or a release.
    pub(crate) fn gun_shot(&mut self, key: e::EntityKey) -> Result<crate::wire::Writer> {
        if !self.gun_ready(key) {
            return Err(NetError::Unsupported);
        }
        let target = self.raw_target(key)?;
        let mode = self.mode.ok_or(NetError::Unsupported)?;
        self.gun.shoot(target, mode).ok_or(NetError::Unsupported)
    }
    /// An own move request starts a walk before its echo arrives.
    pub(crate) fn walking(&mut self) {
        self.walk = Some(Walk::Requested);
    }
    /// The echoed own walk: its destination, the echo's mark and the estimated native end.
    pub(crate) fn walk_end(&self) -> Option<(e::Point, u64, std::time::Instant)> {
        match self.walk {
            Some(Walk::Echoed { to, mark, due, .. }) => Some((to, mark, due)),
            _ => None,
        }
    }
    /// The client ended the echoed walk, on its destination when `arrived`. Only an echo that
    /// accepted the pending MoveTo then arrives, reported under the echo's mark.
    pub(crate) fn end_walk(&mut self, arrived: bool, received: std::time::Instant) {
        let Some(Walk::Echoed {
            to, target, mark, ..
        }) = self.walk
        else {
            return;
        };
        self.walk = None;
        if let Some(target) = target
            && arrived
            && self.arrive(target)
        {
            self.correlated.push(DeferredFacts {
                mark,
                received,
                facts: vec![Fact::Outcome(e::Outcome::Arrived { target, at: to })],
            });
        }
    }
    /// No own walk waits for its echo or is still running.
    pub(crate) fn walk_settled(&self) -> bool {
        self.walk.is_none()
    }
    fn arrive(&mut self, target: e::Point) -> bool {
        let pending = self
            .pending
            .as_ref()
            .is_some_and(|p| p.action == e::Action::MoveTo(target));
        if pending {
            self.pending = None;
        }
        pending
    }
    /// A native dig begins (`DrillBegin`, f24412) or ends (`DrillEnd`, f24413).
    pub(crate) fn drilling(&mut self, active: bool) {
        if !self.gun_live {
            return;
        }
        if active {
            self.gun.drill_begin();
        } else {
            let worn = self.gun_worn() == Some(true);
            self.gun.drill_end(worn);
        }
    }
    /// BasicAttack is supported with a worn gun only when the controller is armed and every
    /// input is known; without one, the ordinary melee rules apply.
    pub(crate) fn basic_attack_supported(&self) -> bool {
        match self.gun_worn() {
            Some(true) => self.gun_prerequisite().is_none(),
            Some(false) => self.normal_weapon_mode().is_ok(),
            None => false,
        }
    }
    /// Own damage through the UI `Damaged` callback (f46664), which calls `Hit` (f24406).
    /// `ApplyMessage` (f17568) applies DamageEvent 42 and CounterDamage 923 at once and
    /// GuardDamage 668 500 ms later; each names the damaged actor in its first U16.
    fn gun_damage(&mut self, guard: bool) {
        let keeps = self.gun_skills.is_some_and(|(_, hit)| hit);
        if guard {
            self.gun.guard_hit(keeps);
        } else {
            self.gun.hit(keeps);
        }
    }
    /// An appearance's effect list and, for the own player after construction, `MyAppear`.
    fn gun_appearance(&mut self, opcode: u16, parsed: &Appearance) {
        if self.own.is_none() || Some(parsed.id) != self.own {
            self.load_speed.foreign_snapshot(parsed.fast.len());
            return;
        }
        // Own 7 is not a proven effect baseline; it can only make LoadSpeed unknown.
        if opcode == 7 {
            self.load_speed.own_lost();
            if !parsed.fast.is_empty() {
                self.load_speed.unreadable();
            }
            return;
        }
        let Some(player) = &parsed.player else {
            if !parsed.fast.is_empty() {
                self.load_speed.unreadable();
            }
            return;
        };
        self.load_speed.own_snapshot(&parsed.fast);
        // MyAppear (f24389) wears every nonzero Worn ID unless Attributes bit 1 is set.
        if self.gun_live && player.attributes & 1 == 0 {
            for &id in player.worn.iter().filter(|id| **id != 0) {
                self.wear(id, true);
            }
        }
    }
    /// Inputs of the client's `GunFlowing` and of its `LoadSpeed` outside the main match.
    fn gun_observe(&mut self, p: &Packet) {
        let b = &p.payload;
        let word = |at: usize| b.get(at..at + 2).map(|v| u16::from_le_bytes([v[0], v[1]]));
        let own = self.own;
        // A damage recipient that cannot be read may be the own avatar.
        let to_own = word(0).is_none_or(|id| own.is_some() && Some(id) == own);
        match p.opcode {
            224 | 292 => match (word(2), word(6)) {
                (Some(target), Some(esa)) if b.len() >= 14 => {
                    let mine = own.is_some() && Some(target) == own;
                    self.load_speed.skill_start(esa, mine, b[9] as i8);
                    // ESA damage to the own avatar reaches Hit through effect timers that
                    // are not modelled. The early reset applies only where it delays Ready.
                    if mine && self.gun_live {
                        let keeps = self.gun_skills.is_some_and(|(_, hit)| hit);
                        self.gun.effect_hit(keeps);
                    }
                }
                _ => self.load_speed.unreadable(),
            },
            307 => match word(2) {
                Some(esa) => self.load_speed.detached_start(esa),
                None => self.load_speed.unreadable(),
            },
            280 => match word(2) {
                Some(instance) if b.len() == 4 => self.load_speed.ended(instance),
                _ => self.load_speed.own_lost(),
            },
            // `Special` ignores a body shorter than eight bytes.
            518 | 521 => {
                let uid = b
                    .get(..8)
                    .map(|v| u64::from_le_bytes(v.try_into().unwrap_or([0; 8])));
                match uid {
                    None => {}
                    Some(uid) if !self.gun_live => {
                        if self.start_special.len() >= MAX_START_SPECIALS {
                            self.ammo_unknown = true;
                        } else {
                            self.start_special.push((p.opcode == 518, uid));
                        }
                    }
                    Some(uid) => self.special(p.opcode == 518, uid),
                }
            }
            82 | 85 if !self.gun_live => self.gun_diverged = true,
            _ => {}
        }
        if !self.gun_live {
            return;
        }
        let uid = b
            .get(..8)
            .map(|v| u64::from_le_bytes(v.try_into().unwrap_or([0; 8])));
        let slot = b.get(8).map(|s| usize::from(*s)).filter(|s| *s < 23);
        let id = |uid: u64| self.items.get(&uid).map(|item| item.id);
        match (p.opcode, uid, slot) {
            // 82 (f24424): a resolved item replaces a different resolved one, then is worn,
            // even when it is the same UID.
            (82, Some(uid), Some(slot)) => {
                if let Some(new) = id(uid) {
                    let old = self.gun_slots[slot];
                    let old = (old != 0 && old != uid).then(|| id(old)).flatten();
                    self.gun_slots[slot] = uid;
                    if let Some(old) = old {
                        self.wear(old, false);
                    }
                    self.wear(new, true);
                }
            }
            // 85 (f24425): clear the slot, then unwear the named item if it is resolved.
            (85, Some(uid), Some(slot)) => {
                self.gun_slots[slot] = 0;
                if let Some(old) = id(uid) {
                    self.wear(old, false);
                }
            }
            _ => {}
        }
        match p.opcode {
            310 | 525 if self.gun.armed() => self.gun.shot_answered(),
            410 if b.first() == Some(&4) => self.gun.camp_mark(),
            410 | 377 => self.gun.camp_enter(),
            380 | 382 => {
                let worn = self.gun_worn() == Some(true);
                let profile = self.gun_profile();
                self.gun.camp_leave(worn, profile);
            }
            // CombatModel.Begin (f17574) calls GunFired for an own Kind-2 Swing.
            // ParseSwing (f13206) needs all seven U16 fields before Begin can run.
            31 | 485 | 1438
                if b.len() >= 14 && own.is_some() && word(0) == own && word(2) == Some(2) =>
            {
                let ammo = self.items.get(&self.gun.loaded()).map(|item| {
                    let info = self.info(item.id);
                    crate::gun::Ammo {
                        count: item.count,
                        keeps_count: info.is_some_and(|info| info.flags_ex & 64 != 0),
                    }
                });
                self.gun.fired(ammo);
            }
            42 | 923 if to_own => self.gun_damage(false),
            668 if to_own => self.gun_damage(true),
            _ => {}
        }
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
        // A received count replaces the gun's local consumption projection.
        self.gun.authoritative(item.uid);
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
                self.gun.authoritative(item.uid);
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
                        self.gun.authoritative(item.uid);
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
/// `(LoadsWhileMoving, KeepsLoadOnHit)` from a 271 list.
fn gun_skills(learned: &BTreeSet<u16>) -> (bool, bool) {
    (
        learned.contains(&crate::gun::MOVE_SKILL),
        learned.contains(&crate::gun::HIT_SKILL),
    )
}
/// What `Parts` reads from the cache: whether a row is held, and whether it is ammunition
/// `GunFlowing` consumes locally (catalog Type 29 without `FlagsEx` bit 64).
fn cached(items: &BTreeMap<u64, Item>, data: Option<&GameDefinitions>, uid: u64) -> Cached {
    let Some(item) = items.get(&uid) else {
        return Cached::Absent;
    };
    match data.and_then(|data| data.items.get(&item.id)) {
        Some(info) if info.kind != 29 || info.flags_ex & 64 != 0 => Cached::Held,
        _ => Cached::Ammo,
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
    /// FastLoad `(instance, value)` entries of the EsBlock.
    fast: Vec<(u16, f32)>,
    player: Option<Player>,
}
/// The parts of a type-1 PlayerBody that the gun controller reads.
struct Player {
    attributes: u8,
    /// `Worn` item IDs from the A mask, in table order.
    worn: Vec<u32>,
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
    let (effects, fast) = effects(&mut r)?;
    r.u32()?;
    let mut target = None;
    let mut body = None;
    match kind {
        1 => body = Some(player(&mut r)?),
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
        fast,
        player: body,
    })
}
/// Whether the EsBlock lists any effect, and its FastLoad `(instance, value)` entries.
fn effects(r: &mut Reader<'_>) -> Result<(bool, Vec<(u16, f32)>)> {
    let mut fast = Vec::new();
    if r.u8()? == 0 {
        return Ok((false, fast));
    }
    let mut seen = false;
    for _ in 0..8 {
        let n = r.u8()?;
        for _ in 0..n {
            seen = true;
            let key = r.u16()?;
            // U16 instance, U8 V14, U16 skill, U16 V04.
            let instance = r.u16()?;
            r.take(5)?;
            if key == crate::gun::FAST_LOAD {
                fast.push((instance, f32::from_bits(r.u32()?)));
                continue;
            }
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
                1118 | 1487 | 1524 | 1525 | 1526 | 1527 | 1529 | 1531 | 1534 | 1537 | 1540
                | 1541 | 1542 | 1543 | 1545 | 1547 => &[2],
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
    Ok((seen, fast))
}
fn player(r: &mut Reader<'_>) -> Result<Player> {
    let attributes = r.u8()?;
    r.u32()?;
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
    let mut worn = Vec::new();
    for (index, bits) in [
        &[
            1u32, 2, 4, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536, 262144,
            524288,
        ][..],
        &[1u32, 2, 4, 4096, 8192, 524288][..],
    ]
    .into_iter()
    .enumerate()
    {
        // The client reads one value per table bit that is set; other bits carry no value.
        let m = r.u32()?;
        for bit in bits {
            if m & bit != 0 {
                let id = r.u32()?;
                if index == 0 {
                    worn.push(id);
                }
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
    Ok(Player { attributes, worn })
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
    fn derived_maxima_add_game_parts_to_the_packet_fed_core() {
        let mut definitions =
            crate::test_support::definitions(&[(7, crate::test_support::Def::Plain { weight: 1 })]);
        let data = std::sync::Arc::get_mut(&mut definitions).unwrap();
        data.player_levels = vec![1000.0; crate::resources::PLAYER_LEVELS];
        let bonuses = data.items.get_mut(&7).unwrap().stat_plus.as_mut().unwrap();
        for (stat, value) in [(9, 50.0), (3, 20.0), (6, 7.0)] {
            bonuses[stat] = value;
        }
        let mut d = Decoder::new();
        d.set_definitions(Some(definitions));
        let at = |opcode, mark, b| {
            let mut p = packet(opcode, b);
            p.mark = mark;
            p
        };
        let maxima = |d: &Decoder| [9, 3, 6].map(|stat| d.maximum(stat).map(|m| m.get()));
        let mut list = vec![1, 0];
        list.extend(item(1, 42, 1));
        d.apply(&at(231, 1, list)).unwrap();
        assert_eq!(maxima(&d), [None; 3]);
        // The 233 at H wears row 42 in slot 0; its points select level-table row 0.
        let mut stats = vec![0; 251];
        stats[..8].copy_from_slice(&42u64.to_le_bytes());
        d.startup_cutoff(2);
        d.apply(&at(233, 2, stats)).unwrap();
        assert_eq!(maxima(&d), [Some(1050), Some(1020), Some(1007)]);
        let r = &d.readiness;
        assert!(r.hp_maximum_known && r.mp_maximum_known && r.weight_maximum_known);
        // A live unequip whose leading UID is held rebuilds the snapshot.
        let mut unequip = 42u64.to_le_bytes().to_vec();
        unequip.push(0);
        d.apply(&at(85, 3, unequip)).unwrap();
        assert_eq!(maxima(&d), [Some(1000); 3]);
        // A received S282 total still wins.
        d.apply(&at(282, 4, vec![1, 9, 100, 0, 0, 0])).unwrap();
        d.lose_game_parts();
        assert_eq!(maxima(&d), [Some(100), None, None]);
    }
    #[test]
    fn login_soul_frames_after_h_keep_derived_maxima() {
        let mut definitions =
            crate::test_support::definitions(&[(7, crate::test_support::Def::Plain { weight: 1 })]);
        let data = std::sync::Arc::get_mut(&mut definitions).unwrap();
        data.player_levels = vec![1000.0; crate::resources::PLAYER_LEVELS];
        data.items.get_mut(&7).unwrap().stat_plus.as_mut().unwrap()[9] = 50.0;
        let mut d = Decoder::new();
        d.set_definitions(Some(definitions));
        let at = |opcode, mark, b| {
            let mut p = packet(opcode, b);
            p.mark = mark;
            p
        };
        let mut list = vec![1, 0];
        list.extend(item(1, 42, 1));
        d.apply(&at(231, 1, list)).unwrap();
        let mut stats = vec![0; 251];
        stats[..8].copy_from_slice(&42u64.to_le_bytes());
        d.startup_cutoff(2);
        d.apply(&at(233, 2, stats)).unwrap();
        assert_eq!(d.maximum(9).map(|m| m.get()), Some(1050));
        // The `SoulLink` and `GuardianState` frames that follow H change no row.
        d.apply(&at(1489, 3, vec![0; 6])).unwrap();
        d.apply(&at(1487, 4, vec![0; 38])).unwrap();
        assert_eq!(d.maximum(9).map(|m| m.get()), Some(1050));
        let r = &d.readiness;
        assert!(r.hp_maximum_known && r.mp_maximum_known && r.weight_maximum_known);
        // A 1489 that carries an item change may reach the client inventory at any time.
        // I32 soul, U16 count 1, then op 3 (remove) of row 42.
        let mut sync = vec![0, 0, 0, 0, 1, 0, 3];
        sync.extend(item(1, 42, 0));
        d.apply(&at(1489, 5, sync)).unwrap();
        assert_eq!(d.maximum(9), None);
        assert!(!d.readiness.hp_maximum_known);
    }
    #[test]
    fn a_worn_kind_2_row_stored_without_options_hides_derived_maxima() {
        let mut definitions =
            crate::test_support::definitions(&[(7, crate::test_support::Def::Plain { weight: 1 })]);
        let data = std::sync::Arc::get_mut(&mut definitions).unwrap();
        data.player_levels = vec![1000.0; crate::resources::PLAYER_LEVELS];
        data.items.get_mut(&7).unwrap().stat_plus.as_mut().unwrap()[9] = 50.0;
        let mut d = Decoder::new();
        d.set_definitions(Some(definitions));
        let at = |opcode, mark, b| {
            let mut p = packet(opcode, b);
            p.mark = mark;
            p
        };
        // A Kind 2 list row ends with its option count.
        let mut list = vec![1, 0];
        list.extend(item(2, 42, 1));
        list.push(0);
        d.apply(&at(231, 1, list)).unwrap();
        let mut stats = vec![0; 251];
        stats[..8].copy_from_slice(&42u64.to_le_bytes());
        d.startup_cutoff(2);
        d.apply(&at(233, 2, stats)).unwrap();
        assert_eq!(d.maximum(9).map(|m| m.get()), Some(1050));
        // Op 6 replaces the worn row and carries no options.
        let mut change = vec![6];
        change.extend(item(2, 42, 1));
        d.apply(&at(561, 3, change)).unwrap();
        assert_eq!(d.maximum(9), None);
        assert!(!d.readiness.hp_maximum_known);
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
    /// Own 321 placed at (100, 200) under the native walk policy.
    fn walker() -> Decoder {
        let mut d = Decoder::new();
        d.use_native_walk_policy();
        let mut clock = vec![0; 4];
        for n in [321u16, 100, 200] {
            clock.extend_from_slice(&n.to_le_bytes());
        }
        d.apply(&packet(230, clock)).unwrap();
        d.apply(&packet(233, vec![0; 251])).unwrap();
        d
    }
    /// An own 24 from `from` to `to`, or an own 25 at `to` without a start.
    fn own_move(mark: u64, from: Option<(u16, u16)>, to: (u16, u16)) -> Packet {
        let mut words = vec![321u16];
        words.extend(from.map(|(x, y)| [x, y]).into_iter().flatten());
        words.extend([to.0, to.1]);
        let mut payload: Vec<u8> = words.into_iter().flat_map(u16::to_le_bytes).collect();
        payload.push(1);
        let mut p = packet(if from.is_some() { 24 } else { 25 }, payload);
        p.mark = mark;
        p
    }
    /// Arrivals in `facts` and in the deferred facts after ending any echoed walk on its
    /// destination when its estimate is due.
    fn arrivals(d: &mut Decoder, facts: Vec<Fact>) -> Vec<e::Outcome> {
        if let Some((_, _, due)) = d.walk_end() {
            d.end_walk(true, due);
        }
        facts
            .into_iter()
            .chain(d.take_correlated().into_iter().flat_map(|c| c.facts))
            .filter_map(|f| match f {
                Fact::Outcome(o @ e::Outcome::Arrived { .. }) => Some(o),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn an_echo_accepts_a_move_that_arrives_when_the_client_would_stop() {
        let mut d = walker();
        let target = e::Point { x: 140, y: 200 };
        d.expect_action(e::Action::MoveTo(target), 10, false)
            .unwrap();
        d.walking();
        // The server ends the walk 4 px short and sends no stop.
        let echo = own_move(11, Some((100, 200)), (136, 200));
        let facts = d.apply(&echo).unwrap();
        assert!(!facts.iter().any(|f| matches!(f, Fact::Outcome(_))));
        assert!(d.action_pending() && !d.walk_settled());
        let end = e::Point { x: 136, y: 200 };
        let (to, mark, due) = d.walk_end().unwrap();
        assert_eq!((to, mark), (end, 11));
        // ceil((36 * 9/8 + 32) * 1000 / 115) + 32 ms at the default walk speed.
        assert_eq!(due - echo.received, std::time::Duration::from_millis(663));
        d.end_walk(true, due);
        let deferred = d.take_correlated();
        assert_eq!(deferred.len(), 1);
        assert_eq!(deferred[0].mark, 11);
        assert!(matches!(
            deferred[0].facts.as_slice(),
            [Fact::Outcome(o)] if *o == e::Outcome::Arrived { target, at: end }
        ));
        assert!(!d.action_pending() && d.walk_settled());
        // Own 472 sets Speed to 50 * 4 px/s.
        d.apply(&packet(472, vec![0x41, 0x01, 50])).unwrap();
        let echo = own_move(12, Some((136, 200)), (172, 200));
        d.apply(&echo).unwrap();
        let due = d.walk_end().unwrap().2;
        assert_eq!(
            due - echo.received,
            std::time::Duration::from_millis(363 + 32)
        );
        d.end_walk(true, due);
        // More than 320 px from the client's actor, the walk starts at the echo's start.
        let echo = own_move(13, Some((600, 200)), (636, 200));
        d.apply(&echo).unwrap();
        let due = d.walk_end().unwrap().2;
        assert_eq!(
            due - echo.received,
            std::time::Duration::from_millis(363 + 32)
        );
        // A zero speed clears the override.
        d.apply(&packet(472, vec![0x41, 0x01, 0])).unwrap();
        let echo = own_move(14, Some((636, 200)), (672, 200));
        d.apply(&echo).unwrap();
        let due = d.walk_end().unwrap().2;
        assert_eq!(due - echo.received, std::time::Duration::from_millis(663));
    }

    #[test]
    fn a_refused_or_diverted_move_never_arrives() {
        let target = e::Point { x: 140, y: 200 };
        let start = |d: &mut Decoder| {
            d.expect_action(e::Action::MoveTo(target), 10, false)
                .unwrap();
            d.walking();
        };
        // Refused by a stop where the character stands. A later echo is the server's own walk.
        let mut d = walker();
        start(&mut d);
        let mut facts = d.apply(&own_move(11, None, (100, 200))).unwrap();
        facts.extend(
            d.apply(&own_move(12, Some((100, 200)), (140, 200)))
                .unwrap(),
        );
        assert!(arrivals(&mut d, facts).is_empty());
        assert!(d.action_pending());
        // Diverted by a second echo.
        let mut d = walker();
        start(&mut d);
        let mut facts = d
            .apply(&own_move(11, Some((100, 200)), (136, 200)))
            .unwrap();
        facts.extend(
            d.apply(&own_move(12, Some((136, 200)), (150, 220)))
                .unwrap(),
        );
        assert!(arrivals(&mut d, facts).is_empty());
        assert!(d.action_pending());
        // Diverted by a stop short of the accepted destination.
        let mut d = walker();
        start(&mut d);
        let mut facts = d
            .apply(&own_move(11, Some((100, 200)), (136, 200)))
            .unwrap();
        facts.extend(d.apply(&own_move(12, None, (120, 200))).unwrap());
        assert!(d.walk_settled());
        assert!(arrivals(&mut d, facts).is_empty());
        assert!(d.action_pending());
        // Unanswered until the window closed. A late echo is the server's own walk.
        let mut d = walker();
        start(&mut d);
        d.expire_action();
        assert!(d.walk_settled());
        let facts = d
            .apply(&own_move(12, Some((100, 200)), (136, 200)))
            .unwrap();
        assert!(!d.walk_settled());
        assert!(arrivals(&mut d, facts).is_empty());
        // An expired move's echo never answers the next request to the same target, whether
        // its walk ends or a stop lands on its destination.
        for stop in [false, true] {
            let mut d = walker();
            d.expect_action(e::Action::MoveTo(target), 10, false)
                .unwrap();
            let mut facts = d
                .apply(&own_move(11, Some((100, 200)), (136, 200)))
                .unwrap();
            d.expire_action();
            d.expect_action(e::Action::MoveTo(target), 20, false)
                .unwrap();
            if stop {
                facts.extend(d.apply(&own_move(21, None, (136, 200))).unwrap());
            }
            assert!(arrivals(&mut d, facts).is_empty());
            assert!(d.action_pending());
        }
        // A stop on the accepted destination ends the walk there at once.
        let mut d = walker();
        start(&mut d);
        let mut facts = d
            .apply(&own_move(11, Some((100, 200)), (136, 200)))
            .unwrap();
        facts.extend(d.apply(&own_move(12, None, (136, 200))).unwrap());
        let end = e::Point { x: 136, y: 200 };
        assert_eq!(
            arrivals(&mut d, facts),
            [e::Outcome::Arrived { target, at: end }]
        );
        assert!(!d.action_pending() && d.walk_settled());
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
        d.expect_action(e::Action::MoveTo(to), 10, false).unwrap();
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
        assert!(d.apply(&stop).unwrap().iter().any(
            |f| matches!(f,Fact::Outcome(e::Outcome::Arrived{target,at}) if *target==to && *at==to)
        ));
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
        d.expect_action(e::Action::BasicAttack(key), 20, false)
            .unwrap();
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
        assert!(effects(&mut r).unwrap().0);
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
    #[test]
    fn hp_and_mp_need_four_bytes_and_accept_any_tail() {
        let mut d = Decoder::new();
        for aux in [0, 1] {
            for opcode in [36, 37] {
                let value = |len| Packet {
                    aux,
                    ..packet(opcode, vec![0; len])
                };
                for len in 0..4 {
                    assert!(
                        d.apply(&value(len)).is_err(),
                        "{opcode}: {len} bytes, aux {aux}"
                    );
                }
                for len in [4, 5, 6, 7, 8, crate::codec::MAX_PAYLOAD] {
                    assert!(
                        d.apply(&value(len)).is_ok(),
                        "{opcode}: {len} bytes, aux {aux}"
                    );
                }
            }
        }
    }
    #[test]
    fn attack_end_and_faint_accept_every_length() {
        let mut d = Decoder::new();
        for aux in [0, 1] {
            for opcode in [32, 43] {
                // The correlator still reads a grouped faint's target as a full U16.
                let shortest = if opcode == 43 && aux != 0 { 2 } else { 0 };
                for len in (shortest..=4).chain([crate::codec::MAX_PAYLOAD]) {
                    let frame = Packet {
                        aux,
                        ..packet(opcode, vec![0; len])
                    };
                    assert!(d.apply(&frame).is_ok(), "{opcode}: {len} bytes, aux {aux}");
                }
            }
        }
        let mut clock = vec![0; 10];
        clock[4..6].copy_from_slice(&7u16.to_le_bytes());
        d.apply(&packet(230, clock)).unwrap();
        d.apply(&packet(36, 50u32.to_le_bytes().to_vec())).unwrap();
        // The missing high byte reads as zero, so this one-byte faint names the own actor 7.
        let facts = d.apply(&packet(43, vec![7])).unwrap();
        assert!(
            facts
                .iter()
                .any(|f| matches!(f, Fact::Hp(g) if g.current == 0))
        );
        assert!(matches!(d.current_hp, Some((0, _))));
    }
    #[test]
    fn strikes_need_the_client_minimum_and_accept_any_tail() {
        let mut d = Decoder::new();
        for aux in [0, 1] {
            for opcode in [31, 485, 1438] {
                for (kind, minimum) in [(3u16, 8), (2, 14)] {
                    let strike = |len| {
                        let mut b = vec![0; len];
                        if let Some(field) = b.get_mut(2..4) {
                            field.copy_from_slice(&kind.to_le_bytes());
                        }
                        Packet {
                            aux,
                            ..packet(opcode, b)
                        }
                    };
                    let label =
                        |len: usize| format!("{opcode} kind {kind}: {len} bytes, aux {aux}");
                    for len in 0..minimum {
                        assert!(d.apply(&strike(len)).is_err(), "{}", label(len));
                    }
                    for len in (minimum..minimum + 6).chain([crate::codec::MAX_PAYLOAD]) {
                        assert!(d.apply(&strike(len)).is_ok(), "{}", label(len));
                    }
                }
            }
        }
    }
}
