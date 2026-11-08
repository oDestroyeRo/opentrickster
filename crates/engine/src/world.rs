use crate::*;
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
    Stack(StackKey),
}

pub(crate) struct Evidence {
    pub ingress: IngressMark,
    pub at: At,
    pub outcomes: Vec<Outcome>,
    pub gains: BTreeMap<ItemKind, u64>,
}

pub(crate) struct Book {
    pub own: OwnView,
    pub entities: BTreeMap<EntityKey, Entity>,
    pub drops: BTreeMap<DropKey, Drop>,
    pub bag: BTreeMap<StackKey, Stack>,
    complete_inventory_observed: bool,
    pub hp_potion: Option<StackKey>,
    stamps: BTreeMap<Field, (IngressMark, At)>,
    incarnations: BTreeMap<EntityKey, u64>,
    map_labels: BTreeMap<MapKey, DisplayText>,
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
            bag: BTreeMap::new(),
            complete_inventory_observed: false,
            hp_potion: None,
            stamps: BTreeMap::new(),
            incarnations: BTreeMap::new(),
            map_labels: BTreeMap::new(),
        }
    }

    pub fn incarnation(&self, key: EntityKey) -> u64 {
        self.incarnations.get(&key).copied().unwrap_or(0)
    }

    fn accept(&mut self, field: Field, ingress: IngressMark, at: At) -> bool {
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
        blockers
    }

    pub fn invalidate(&mut self) {
        self.complete_inventory_observed = false;
        self.hp_potion = None;
        self.own.inventory = InventoryKnowledge::Unknown;
        self.own.last_state_at = None;
        self.stamps.clear();
        self.bag.clear();
        self.entities.clear();
        self.drops.clear();
    }

    pub fn fresh_until(&self) -> Option<At> {
        self.own
            .last_state_at
            .map(|seen| At(seen.0.saturating_add(15_000)))
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

    fn quantities(&self) -> BTreeMap<ItemKind, u64> {
        let mut quantities = BTreeMap::new();
        for stack in self.bag.values() {
            let entry = quantities.entry(stack.kind).or_insert(0u64);
            *entry = entry.saturating_add(u64::from(stack.quantity));
        }
        quantities
    }

    pub fn apply(&mut self, facts: Vec<Fact>, ingress: IngressMark, now: At) -> Evidence {
        let mut evidence = Evidence {
            ingress,
            at: now,
            outcomes: Vec::new(),
            gains: BTreeMap::new(),
        };
        let before = self.quantities();
        let known_before = self.own.inventory == InventoryKnowledge::Observed;
        let complete_before = self.complete_inventory_observed;
        let known_stacks: Vec<_> = self.bag.keys().copied().collect();
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
                    self.hp_potion = stack;
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
                Fact::Inventory(inventory) => {
                    state |= self.inventory(inventory, ingress, now);
                }
                Fact::Outcome(outcome) => evidence.outcomes.push(outcome),
                _ => {}
            }
        }
        if known_before {
            let after = if complete_before {
                self.quantities()
            } else {
                let mut quantities = BTreeMap::new();
                for key in known_stacks {
                    if let Some(stack) = self.bag.get(&key) {
                        let entry = quantities.entry(stack.kind).or_insert(0u64);
                        *entry = entry.saturating_add(u64::from(stack.quantity));
                    }
                }
                quantities
            };
            for (kind, count) in after {
                let gain = count.saturating_sub(before.get(&kind).copied().unwrap_or(0));
                if gain > 0 {
                    evidence.gains.insert(kind, gain);
                }
            }
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

    fn inventory(&mut self, inventory: Inventory, ingress: IngressMark, now: At) -> bool {
        if self
            .stamps
            .get(&Field::Inventory)
            .is_some_and(|(floor, _)| *floor > ingress)
        {
            return false;
        }
        match inventory {
            Inventory::Reset(stacks) => {
                // A reset cannot replace mutations already observed at a later ingress.
                if self
                    .stamps
                    .iter()
                    .any(|(field, (mark, _))| matches!(field, Field::Stack(_)) && *mark > ingress)
                {
                    return false;
                }
                self.bag = stacks.into_iter().map(|stack| (stack.key, stack)).collect();
                self.own.inventory = InventoryKnowledge::Observed;
                self.complete_inventory_observed = true;
                self.stamps.insert(Field::Inventory, (ingress, now));
            }
            Inventory::Put(stack) if self.accept(Field::Stack(stack.key), ingress, now) => {
                self.bag.insert(stack.key, stack);
                self.own.inventory = InventoryKnowledge::Observed;
            }
            Inventory::Remove(key) if self.accept(Field::Stack(key), ingress, now) => {
                self.bag.remove(&key);
            }
            Inventory::Quantity(key, count) if self.accept(Field::Stack(key), ingress, now) => {
                let Some(stack) = self.bag.get_mut(&key) else {
                    return false;
                };
                stack.quantity = count;
            }
            _ => return false,
        }
        if self.own.inventory == InventoryKnowledge::Observed {
            self.stamps
                .entry(Field::Inventory)
                .and_modify(|(_, at)| *at = now)
                .or_insert((IngressMark(0), now));
        }
        true
    }
}

pub(crate) fn valid_gauge(gauge: &Gauge) -> bool {
    gauge.maximum.is_none_or(|max| gauge.current <= max.get())
}
