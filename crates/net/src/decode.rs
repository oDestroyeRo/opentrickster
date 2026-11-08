use crate::{NetError, Result, codec::Packet, wire::Reader};
use opentrickster_engine::{self as e, Fact};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ACTORS: usize = 4096;
const MAX_ITEMS: usize = 8192;

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
}
struct ItemBody {
    kind: u8,
    id: u32,
    uid: u64,
    count: u16,
    state: [u8; 4],
}

#[derive(Clone, Copy, Default)]
pub struct Readiness {
    pub own_baseline_seen: bool,
    pub inventory_list_seen: bool,
    pub equipment_references_resolved: bool,
}
/// No complete-inventory flag exists. These fields report only observed boundaries.
pub(crate) struct Decoder {
    definitions: Option<std::sync::Arc<crate::definitions::GameDefinitions>>,
    mode: Option<u8>,
    native_walk: bool,
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
    species: BTreeMap<u16, e::SpeciesKey>,
    maps: BTreeMap<u16, e::MapKey>,
    drops: BTreeMap<u16, e::DropKey>,
    slots: [u64; 23],
    missing_slots: BTreeSet<u64>,
    missing_slot_refs: [u64; 23],
    next: u64,
    pub(crate) readiness: Readiness,
}
impl Decoder {
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::with_epoch(0)
    }
    pub(crate) fn with_epoch(epoch: u64) -> Self {
        Self {
            definitions: None,
            mode: None,
            native_walk: false,
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
            species: BTreeMap::new(),
            maps: BTreeMap::new(),
            drops: BTreeMap::new(),
            slots: [0; 23],
            missing_slots: BTreeSet::new(),
            missing_slot_refs: [0; 23],
            next: epoch.saturating_mul(1 << 32),
            readiness: Readiness::default(),
        }
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
        let from = self.point.ok_or(NetError::Unsupported)?;
        let mode = self.mode.ok_or(NetError::Unsupported)?;
        crate::actions::movement(from, to, mode)
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
        let b = &p.payload;
        let mut r = Reader::new(b);
        let mut out = Vec::new();
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
                self.zone = Some(zone);
                let map = self.map_key(zone)?;
                self.map = Some(map);
                r.take(10)?;
                let galder = r.u32()? as u64;
                let hp = r.u32()?;
                let mp = r.u32()?;
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
                out.push(if p.opcode == 36 {
                    Fact::Hp(gauge(value))
                } else {
                    Fact::Mp(gauge(value))
                });
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
                    out.push(Fact::Hp(gauge(0)));
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
                r.u16()?;
                r.take(24)?;
                if r.remaining() >= 2 {
                    r.u16()?;
                }
                r.end()?;
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
                    self.change(op, item, &mut out)?;
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
                self.own = Some(r.u16()?);
                let point = e::Point {
                    x: r.u16()? as i32,
                    y: r.u16()? as i32,
                };
                r.end()?;
                self.point = Some(point);
                if let Some(map) = self.map {
                    out.push(Fact::Position { map, point });
                }
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
        if matches!(
            p.opcode,
            233 | 82 | 85 | 561 | 160 | 770 | 262 | 246 | 260 | 843 | 68
        ) {
            self.readiness.equipment_references_resolved =
                self.stats_seen && self.missing_slot_refs.iter().all(|uid| *uid == 0);
            out.push(self.drill_fact());
        }
        self.readiness.own_baseline_seen = self.baseline();
        Ok(out)
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
            quantity: item.count as u32,
        })));
        self.items.insert(
            item.uid,
            Item {
                key,
                count: item.count,
                state: item.state,
                id: item.id,
            },
        );
        Ok(())
    }
    fn change(&mut self, op: u8, item: ItemBody, out: &mut Vec<Fact>) -> Result<()> {
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
        Ok(())
    }
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
    if options && kind == 2 {
        let count = r.u8()? as usize;
        r.take(count * 12)?;
    }
    Ok(ItemBody {
        kind,
        id,
        uid,
        count,
        state,
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
        3 => target = Some(r.u16()?),
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
                _ => return Err(NetError::Unsupported),
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
    let mask = r.u8()?;
    if mask & 128 != 0 {
        return Err(NetError::Unsupported);
    }
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
        let m = r.u32()?;
        if m & !bits.iter().fold(0, |a, b| a | b) != 0 {
            return Err(NetError::Unsupported);
        }
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
    fn malformed_effect_and_player_branches() {
        assert!(effects(&mut Reader::new(&[1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0])).is_err());
        let mut player_bytes = vec![0; 43];
        assert!(player(&mut Reader::new(&player_bytes)).is_ok());
        player_bytes[5] = 128;
        assert!(player(&mut Reader::new(&player_bytes)).is_err());
        assert!(item_body(&mut Reader::new(&[]), 9, false).is_err());
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
