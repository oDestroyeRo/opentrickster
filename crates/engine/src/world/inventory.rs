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
    generations: BTreeMap<StackKey, u64>,
    unresolved: BTreeSet<StackKey>,
    observed: bool,
    complete: bool,
}

impl State {
    fn known(&self) -> bool {
        self.observed && self.unresolved.is_empty()
    }

    fn insert(&mut self, stack: &Stack) {
        if self
            .stacks
            .get(&stack.key)
            .is_none_or(|old| old.kind != stack.kind)
        {
            let generation = self.generations.entry(stack.key).or_default();
            *generation = generation.saturating_add(1);
        }
        self.stacks.insert(stack.key, stack.clone());
        self.unresolved.remove(&stack.key);
    }

    fn apply(&mut self, update: &Update) -> [Option<(ItemKind, i64)>; 2] {
        let known = self.known();
        let mut delta = [None, None];
        match update {
            Update::Snapshot(stacks) => {
                for stack in stacks {
                    self.insert(stack);
                }
                self.observed = true;
            }
            Update::Delta(Inventory::Reset(stacks)) => {
                self.stacks.clear();
                for stack in stacks {
                    self.insert(stack);
                }
                self.unresolved.clear();
                self.observed = true;
                self.complete = true;
            }
            Update::Delta(Inventory::Put(stack)) => {
                let previous = self.stacks.get(&stack.key);
                if known && (self.complete || previous.is_some()) {
                    delta[0] = previous.map(|old| (old.kind, -i64::from(old.quantity)));
                    delta[1] = Some((stack.kind, i64::from(stack.quantity)));
                }
                self.insert(stack);
                self.observed = true;
            }
            Update::Delta(Inventory::Remove(key)) => {
                if let Some(old) = self.stacks.remove(key)
                    && known
                {
                    delta[0] = Some((old.kind, -i64::from(old.quantity)));
                }
                self.unresolved.remove(key);
            }
            Update::Delta(Inventory::Quantity(key, count)) => {
                if let Some(stack) = self.stacks.get_mut(key) {
                    if known {
                        delta[0] =
                            Some((stack.kind, i64::from(*count) - i64::from(stack.quantity)));
                    }
                    stack.quantity = *count;
                } else {
                    self.unresolved.insert(*key);
                }
            }
        }
        delta
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
            let mut changes = BTreeMap::<ItemKind, i128>::new();
            for update in updates {
                for (kind, delta) in self.committed.apply(&update).into_iter().flatten() {
                    *changes.entry(kind).or_default() += i128::from(delta);
                }
            }
            for (kind, change) in changes {
                if change > 0 {
                    gains.push(Gain {
                        ingress,
                        kind,
                        quantity: u64::try_from(change).unwrap_or(u64::MAX),
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

    pub fn identity(&self, key: StackKey) -> Option<(ItemKind, u64)> {
        self.view
            .stacks
            .get(&key)
            .zip(self.view.generations.get(&key))
            .map(|(stack, generation)| (stack.kind, *generation))
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
