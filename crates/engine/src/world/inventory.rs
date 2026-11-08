use crate::*;
use std::collections::{BTreeMap, BTreeSet};

const PENDING_LIMIT: usize = 256;

pub(super) enum Update {
    Delta(Inventory),
    Snapshot(Vec<Stack>),
}

pub(crate) struct Gain {
    pub ingress: IngressMark,
    pub kind: ItemKind,
    pub quantity: u64,
}

#[derive(Clone, Default)]
struct State {
    stacks: BTreeMap<StackKey, Stack>,
    unresolved: BTreeSet<StackKey>,
    observed: bool,
    complete: bool,
}

impl State {
    fn known(&self) -> bool {
        self.observed && self.unresolved.is_empty()
    }

    fn apply(&mut self, update: &Update) -> Option<(ItemKind, u64)> {
        let known = self.known();
        let mut gain = None;
        match update {
            Update::Snapshot(stacks) => {
                for stack in stacks {
                    self.stacks.insert(stack.key, stack.clone());
                    self.unresolved.remove(&stack.key);
                }
                self.observed = true;
            }
            Update::Delta(Inventory::Reset(stacks)) => {
                self.stacks = stacks
                    .iter()
                    .map(|stack| (stack.key, stack.clone()))
                    .collect();
                self.unresolved.clear();
                self.observed = true;
                self.complete = true;
            }
            Update::Delta(Inventory::Put(stack)) => {
                let previous = self.stacks.get(&stack.key);
                if known && (self.complete || previous.is_some()) {
                    let before = previous
                        .filter(|old| old.kind == stack.kind)
                        .map_or(0, |old| old.quantity);
                    gain = Some((stack.kind, u64::from(stack.quantity.saturating_sub(before))));
                }
                self.stacks.insert(stack.key, stack.clone());
                self.unresolved.remove(&stack.key);
                self.observed = true;
            }
            Update::Delta(Inventory::Remove(key)) => {
                self.stacks.remove(key);
                self.unresolved.remove(key);
            }
            Update::Delta(Inventory::Quantity(key, count)) => {
                if let Some(stack) = self.stacks.get_mut(key) {
                    if known {
                        gain = Some((stack.kind, u64::from(count.saturating_sub(stack.quantity))));
                    }
                    stack.quantity = *count;
                } else {
                    self.unresolved.insert(*key);
                }
            }
        }
        gain.filter(|(_, quantity)| *quantity > 0)
    }
}

pub(crate) struct InventoryBook {
    pub stacks: BTreeMap<StackKey, Stack>,
    committed: State,
    view: State,
    pending: BTreeMap<IngressMark, (At, Vec<Update>)>,
    settled: Option<IngressMark>,
    latest: Option<(IngressMark, At)>,
    poisoned: bool,
}

impl InventoryBook {
    pub fn new() -> Self {
        Self {
            stacks: BTreeMap::new(),
            committed: State::default(),
            view: State::default(),
            pending: BTreeMap::new(),
            settled: None,
            latest: None,
            poisoned: false,
        }
    }

    pub(super) fn observe(&mut self, updates: Vec<Update>, ingress: IngressMark, at: At) {
        if updates.is_empty() || self.poisoned {
            return;
        }
        if self.settled.is_some_and(|floor| ingress <= floor)
            || (!self.pending.contains_key(&ingress) && self.pending.len() >= PENDING_LIMIT)
        {
            self.poisoned = true;
            self.pending.clear();
            return;
        }
        if self.latest.is_none_or(|(mark, _)| ingress >= mark) {
            self.latest = Some((ingress, at));
        }
        self.pending
            .entry(ingress)
            .or_insert_with(|| (at, Vec::new()))
            .1
            .extend(updates);
        self.rebuild();
    }

    pub fn settle(&mut self, frontier: IngressMark) -> Vec<Gain> {
        if self.poisoned || self.settled.is_some_and(|old| frontier <= old) {
            return Vec::new();
        }
        let mut gains = Vec::new();
        while self
            .pending
            .first_key_value()
            .is_some_and(|(mark, _)| *mark <= frontier)
        {
            let (ingress, (_, updates)) = self.pending.pop_first().expect("pending entry checked");
            for update in updates {
                if let Some((kind, quantity)) = self.committed.apply(&update) {
                    gains.push(Gain {
                        ingress,
                        kind,
                        quantity,
                    });
                }
            }
        }
        self.settled = Some(frontier);
        self.rebuild();
        gains
    }

    fn rebuild(&mut self) {
        self.view = self.committed.clone();
        for (_, updates) in self.pending.values() {
            for update in updates {
                self.view.apply(update);
            }
        }
        self.stacks.clone_from(&self.view.stacks);
    }

    pub fn knowledge(&self) -> InventoryKnowledge {
        if !self.poisoned && self.view.known() {
            InventoryKnowledge::Observed
        } else {
            InventoryKnowledge::Unknown
        }
    }

    pub fn stamp(&self) -> Option<(IngressMark, At)> {
        (self.knowledge() == InventoryKnowledge::Observed)
            .then_some(self.latest)
            .flatten()
    }

    pub fn pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn deadline(&self) -> Option<At> {
        self.pending
            .values()
            .map(|(at, _)| At(at.0.saturating_add(15_000)))
            .min()
    }
}
