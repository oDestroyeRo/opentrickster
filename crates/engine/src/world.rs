mod inventory;

use crate::*;
use inventory::{Gain, InventoryBook, Update};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Field {
    Label,
    Profile,
    MapLabel(MapKey),
    Position,
    Hp,
    Mp,
    Weight,
    Potions,
    PotionStack,
    Drill,
    Entity(EntityKey),
    Drop(DropKey),
    Inventory,
}

pub(crate) struct Evidence {
    pub ingress: IngressMark,
    pub at: At,
    pub outcomes: Vec<Outcome>,
    pub gains: Vec<Gain>,
}

pub(crate) struct Book {
    pub own: OwnView,
    pub entities: BTreeMap<EntityKey, Entity>,
    pub drops: BTreeMap<DropKey, Drop>,
    pub inventory: InventoryBook,
    hp_potion: Option<(StackKey, ItemKind, u64)>,
    stamps: BTreeMap<Field, (IngressMark, At)>,
    incarnations: BTreeMap<EntityKey, u64>,
    map_labels: BTreeMap<MapKey, DisplayText>,
    map_floor: IngressMark,
}

impl Book {
    pub fn new() -> Self {
        Self {
            own: OwnView {
                label: None,
                profile: None,
                map_label: None,
                position: None,
                vitals: None,
                drill: Drill::Unknown,
                last_state_at: None,
                inventory: InventoryKnowledge::Unknown,
            },
            entities: BTreeMap::new(),
            drops: BTreeMap::new(),
            inventory: InventoryBook::new(),
            hp_potion: None,
            stamps: BTreeMap::new(),
            incarnations: BTreeMap::new(),
            map_labels: BTreeMap::new(),
            map_floor: IngressMark(0),
        }
    }

    pub fn incarnation(&self, key: EntityKey) -> u64 {
        self.incarnations.get(&key).copied().unwrap_or(0)
    }

    pub fn hp_potion(&self) -> Option<StackKey> {
        if self.inventory.knowledge() != InventoryKnowledge::Observed {
            return None;
        }
        self.hp_potion
            .filter(|(key, kind, generation)| {
                self.inventory.identity(*key) == Some((*kind, *generation))
            })
            .map(|(key, _, _)| key)
    }

    fn accept(&mut self, field: Field, ingress: IngressMark, at: At) -> bool {
        if matches!(field, Field::Entity(_) | Field::Drop(_)) && ingress < self.map_floor {
            return false;
        }
        if self
            .stamps
            .get(&field)
            .is_some_and(|(old, _)| *old > ingress)
        {
            return false;
        }
        self.stamps.insert(field, (ingress, at));
        true
    }

    pub fn missing_or_stale(&self, at: At, digging: bool) -> Vec<Blocker> {
        let mut fields = vec![
            (Field::Position, StateKind::Position),
            (Field::Hp, StateKind::Hp),
            (Field::Weight, StateKind::Weight),
            (Field::Inventory, StateKind::Inventory),
        ];
        if digging {
            fields.push((Field::Drill, StateKind::Drill));
        }
        let mut blockers: Vec<_> = fields
            .into_iter()
            .filter_map(|(field, kind)| {
                (!self.stamps.contains_key(&field)).then_some(Blocker::NeedsObservation(kind))
            })
            .collect();
        if self
            .own
            .last_state_at
            .is_some_and(|seen| at.0.saturating_sub(seen.0) >= 15_000)
        {
            blockers.push(Blocker::NeedsFreshObservation(StateKind::WorldState));
        }
        if self
            .inventory
            .deadline()
            .is_some_and(|deadline| at >= deadline)
        {
            blockers.push(Blocker::NeedsFreshObservation(StateKind::Inventory));
        }
        blockers
    }

    pub fn invalidate(&mut self) {
        self.inventory = InventoryBook::new();
        self.hp_potion = None;
        self.own.inventory = InventoryKnowledge::Unknown;
        self.own.last_state_at = None;
        self.stamps.clear();
        self.entities.clear();
        self.drops.clear();
    }

    pub fn fresh_until(&self) -> Option<At> {
        let state = self
            .own
            .last_state_at
            .map(|seen| At(seen.0.saturating_add(15_000)));
        state.into_iter().chain(self.inventory.deadline()).min()
    }

    pub fn entity_fresh(&self, key: EntityKey, at: At) -> bool {
        self.stamps
            .get(&Field::Entity(key))
            .is_some_and(|(_, seen)| at.0.saturating_sub(seen.0) < 15_000)
    }

    pub fn drop_fresh(&self, key: DropKey, at: At) -> bool {
        self.stamps
            .get(&Field::Drop(key))
            .is_some_and(|(_, seen)| at.0.saturating_sub(seen.0) < 15_000)
    }

    pub fn mark_killed(&mut self, key: EntityKey, ingress: IngressMark, at: At) {
        if self.accept(Field::Entity(key), ingress, at)
            && let Some(entity) = self.entities.get_mut(&key)
        {
            entity.alive = false;
        }
    }

    pub fn mark_picked(&mut self, key: DropKey, ingress: IngressMark, at: At) {
        if self.accept(Field::Drop(key), ingress, at) {
            self.drops.remove(&key);
        }
    }

    pub fn arrived(&mut self, point: Point, ingress: IngressMark, at: At) {
        if self.accept(Field::Position, ingress, at)
            && let Some((map, _)) = self.own.position
        {
            self.own.position = Some((map, point));
            self.mark_state_seen(at);
        }
    }

    fn vitals(&mut self) -> &mut Vitals {
        self.own.vitals.get_or_insert(Vitals {
            hp: None,
            mp: None,
            weight: None,
            hp_potions: None,
        })
    }

    pub fn apply(&mut self, facts: Vec<Fact>, ingress: IngressMark, now: At) -> Evidence {
        let mut evidence = Evidence {
            ingress,
            at: now,
            outcomes: Vec::new(),
            gains: Vec::new(),
        };
        let mut inventory = Vec::new();
        let mut settled: Option<IngressMark> = None;
        let mut potion = None;
        let mut state = false;
        for fact in facts {
            match fact {
                Fact::OwnLabel(label) if self.accept(Field::Label, ingress, now) => {
                    self.own.label = Some(label);
                }
                Fact::Profile(profile) if self.accept(Field::Profile, ingress, now) => {
                    self.own.profile = Some(profile);
                }
                Fact::MapLabel { map, label }
                    if self.accept(Field::MapLabel(map), ingress, now) =>
                {
                    self.map_labels.insert(map, label);
                    self.own.map_label = self
                        .own
                        .position
                        .and_then(|(map, _)| self.map_labels.get(&map).cloned());
                }
                Fact::Position { map, point }
                    if wire_point(point) && self.accept(Field::Position, ingress, now) =>
                {
                    if self.own.position.is_some_and(|(old, _)| old != map) {
                        self.entities.clear();
                        self.drops.clear();
                        self.map_floor = ingress;
                    }
                    self.own.position = Some((map, point));
                    self.own.map_label = self.map_labels.get(&map).cloned();
                    state = true;
                }
                Fact::Hp(gauge) if valid_gauge(&gauge) && self.accept(Field::Hp, ingress, now) => {
                    self.vitals().hp = Some(gauge);
                    state = true;
                }
                Fact::Mp(gauge) if valid_gauge(&gauge) && self.accept(Field::Mp, ingress, now) => {
                    self.vitals().mp = Some(gauge);
                    state = true;
                }
                Fact::Weight(gauge)
                    if valid_gauge(&gauge) && self.accept(Field::Weight, ingress, now) =>
                {
                    self.vitals().weight = Some(gauge);
                    state = true;
                }
                Fact::HpPotions(count) if self.accept(Field::Potions, ingress, now) => {
                    self.vitals().hp_potions = Some(count);
                    state = true;
                }
                Fact::HpPotionStack(stack) if self.accept(Field::PotionStack, ingress, now) => {
                    potion = Some(stack);
                }
                Fact::Drill(drill) if self.accept(Field::Drill, ingress, now) => {
                    self.own.drill = drill;
                    state = true;
                }
                Fact::Entity(entity)
                    if wire_point(entity.position)
                        && entity.hp.as_ref().is_none_or(valid_gauge)
                        && self.accept(Field::Entity(entity.key), ingress, now) =>
                {
                    if entity.alive && self.entities.get(&entity.key).is_none_or(|old| !old.alive) {
                        let generation = self.incarnations.entry(entity.key).or_insert(0);
                        *generation = generation.saturating_add(1);
                    }
                    self.entities.insert(entity.key, entity);
                    state = true;
                }
                Fact::EntityGone(key) if self.accept(Field::Entity(key), ingress, now) => {
                    self.entities.remove(&key);
                    state = true;
                }
                Fact::Drop(drop)
                    if wire_point(drop.position)
                        && self.accept(Field::Drop(drop.key), ingress, now) =>
                {
                    self.drops.insert(drop.key, drop);
                    state = true;
                }
                Fact::DropGone(key) if self.accept(Field::Drop(key), ingress, now) => {
                    self.drops.remove(&key);
                    state = true;
                }
                Fact::StateCurrent => state = true,
                Fact::Inventory(update) => inventory.push(Update::Delta(update)),
                Fact::InventorySnapshot(stacks) => inventory.push(Update::Snapshot(stacks)),
                Fact::InventorySettled(frontier) => {
                    settled = Some(settled.map_or(frontier, |old| old.max(frontier)));
                }
                Fact::Outcome(outcome) => evidence.outcomes.push(outcome),
                _ => {}
            }
        }
        state |= !inventory.is_empty();
        self.inventory.observe(inventory, ingress, now);
        if let Some(frontier) = settled {
            evidence.gains = self.inventory.settle(frontier);
        }
        if let Some(proof) = potion {
            self.hp_potion = proof.and_then(|key| {
                self.inventory
                    .identity(key)
                    .map(|(kind, generation)| (key, kind, generation))
            });
        }
        self.own.inventory = self.inventory.knowledge();
        if let Some(stamp) = self.inventory.stamp() {
            self.stamps.insert(Field::Inventory, stamp);
        } else {
            self.stamps.remove(&Field::Inventory);
        }
        if state {
            self.mark_state_seen(now);
        }
        evidence
    }

    fn mark_state_seen(&mut self, at: At) {
        self.own.last_state_at = Some(At(self
            .own
            .last_state_at
            .map_or(at.0, |old| old.0.max(at.0))));
    }
}

pub(crate) fn valid_gauge(gauge: &Gauge) -> bool {
    gauge.maximum.is_none_or(|max| gauge.current <= max.get())
}
