//! `AvatarStats.Part`, the `GameParts` term of `CharacterStatus.Total` on build ea1a68bd:
//! the equipment snapshot, the own set bonuses and the imprint bonus. Net reports the
//! steady state with `GameParts` installed at H, and only while every input is proven.
//! docs/research/derived-resource-maxima.md cites every input.
use crate::{definitions::GameDefinitions, wire::Reader};
use std::collections::{BTreeMap, BTreeSet};

const SLOTS: usize = 23;
/// `GunFlowing.Loaded` is the Type 29 ammunition in this slot.
const AMMO_SLOT: usize = 20;
/// `ItemTip.PropertyEntries` adds an entry only while it holds at most 31.
const ENTRIES: usize = 32;
/// StartSet frames kept for the client's later passes. More make `GameParts` unknown.
const DEFERRED: usize = 1024;
/// Unproven UIDs kept at once, as many rows as the decoder accepts in one list. More make
/// `GameParts` unknown.
const UNPROVEN: usize = 8192;
/// Distinct own set IDs kept. More make the set term unknown.
const SETS: usize = 1024;

/// Opcodes whose client handlers change inventory rows or slots through paths Net does
/// not model: the `Inventory` hooks without a decoder grammar, and every other caller of
/// `Inventory.Apply` or `Consume` that a frame reaches except the soul frames 1487 and
/// 1489. Any of them, StartSet included, leaves `GameParts` unknown for the session.
/// Sorted. The `Inventory` hook for 261 has an empty body, so 261 is not listed.
pub(crate) const UNMODELED: &[u16] = &[
    71, 73, 74, 75, 77, 78, 89, 94, 95, 96, 98, 100, 101, 102, 103, 104, 105, 112, 113, 115, 116,
    117, 140, 143, 150, 168, 169, 170, 171, 174, 175, 178, 179, 190, 195, 199, 202, 219, 255, 293,
    335, 342, 343, 345, 346, 348, 349, 351, 353, 354, 356, 358, 360, 363, 367, 368, 369, 370, 449,
    473, 558, 586, 587, 590, 599, 601, 697, 700, 705, 706, 720, 735, 788, 802, 831, 853, 859, 860,
    862, 863, 865, 866, 873, 878, 879, 881, 882, 884, 885, 898, 904, 905, 907, 908, 910, 911, 913,
    914, 916, 917, 919, 920, 975, 976, 979, 980, 984, 986, 987, 1010, 1014, 1016, 1019, 1021, 1088,
    1091, 1098, 1100, 1102, 1103, 1106, 1108, 1141, 1142, 1144, 1152, 1173, 1174, 1177, 1178, 1179,
    1184, 1185, 1208, 1209, 1211, 1212, 1214, 1215, 1217, 1218, 1237, 1240, 1266, 1270, 1271, 1273,
    1284, 1287, 1288, 1290, 1291, 1293, 1294, 1312, 1314, 1365, 1366, 1367, 1368, 1369, 1391, 1429,
    1435, 1443, 1450, 1465, 1467, 1468, 1470, 1471, 1473, 1474, 1476, 1477, 1479, 1480, 1482, 1488,
    1490, 1491,
];

/// What Net's inventory cache holds for a UID.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Cached {
    Absent,
    Held,
    /// A row `GunFlowing` consumes locally once it is loaded: catalog Type 29 without
    /// `FlagsEx` bit 64, or a row whose catalog entry is missing.
    Ammo,
}

/// A cached inventory row as `AvatarStats.Equipment` reads it.
pub(crate) struct Row<'a> {
    pub(crate) id: u32,
    pub(crate) wire_kind: u8,
    /// The options on the wire, or `None` for a row that came without them. The client
    /// then holds an empty list.
    pub(crate) options: Option<&'a [[u32; 3]]>,
}

/// The client state behind `Part`. `None` marks state Net cannot prove.
#[cfg_attr(test, derive(Clone, Debug, PartialEq))]
pub(crate) struct Parts {
    /// The client `Inventory` slots. They follow `SetSlot` and ignore a live 233.
    slots: [Option<u64>; SLOTS],
    /// A worn row or slot changed after H with no rebuild since, so the `Rebuild`
    /// snapshot depends on the unobservable install time.
    stale: bool,
    /// UIDs whose client row, or whether the client holds one, Net cannot prove.
    diverged: BTreeSet<u64>,
    /// Ammunition that has been `GunFlowing.Loaded`. The client may have consumed it.
    loaded: BTreeSet<u64>,
    /// StartSet 518 (true) and 521 for `Inventory`'s second replay pass.
    specials: Vec<(bool, u64)>,
    /// StartSet 1387 item changes, which `ImprintLink` applies after that replay.
    changes: Vec<Change>,
    /// The StartSet has been closed.
    started: bool,
    /// An unmodelled inventory input arrived, or the input stream is filtered.
    lost: bool,
    clock_seen: bool,
    /// `GameSession.Me.Id`.
    me: Option<u16>,
    /// The own `ProfileTitles` set list, as a count per set ID.
    sets: Option<BTreeMap<u32, u32>>,
    /// Every `ImprintList` accumulator is provably zero.
    imprints_empty: bool,
}
impl Default for Parts {
    fn default() -> Self {
        Self {
            slots: [Some(0); SLOTS],
            stale: false,
            diverged: BTreeSet::new(),
            loaded: BTreeSet::new(),
            specials: Vec::new(),
            changes: Vec::new(),
            started: false,
            lost: false,
            clock_seen: false,
            me: None,
            sets: Some(BTreeMap::new()),
            imprints_empty: true,
        }
    }
}

/// `Inventory.Apply` of one `ReadItemChange`, as it affects rows and slots.
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq))]
enum Change {
    /// Ops 2, 4, 6 and 8 with Kind 1 or 2 replace the row.
    Set(u64),
    /// Ops 3, 5 and 7 remove the row and clear the first slot holding a non-zero UID.
    Remove(u64),
}
fn change(op: u8, kind: u8, uid: u64) -> Option<Change> {
    match op {
        2 | 4 | 6 | 8 if matches!(kind, 1 | 2) => Some(Change::Set(uid)),
        3 | 5 | 7 => Some(Change::Remove(uid)),
        _ => None,
    }
}

impl Parts {
    /// The input stream lacks frames `GameParts` depends on.
    pub(crate) fn lose(&mut self) {
        self.lost = true;
    }
    /// Whether the client inventory holds `uid`, or None when Net cannot prove it.
    fn held(&self, uid: u64, cached: &impl Fn(u64) -> Cached) -> Option<bool> {
        if self.diverged.contains(&uid) || self.loaded.contains(&uid) {
            return None;
        }
        Some(cached(uid) != Cached::Absent)
    }
    /// Ammunition left in slot 20 becomes `GunFlowing.Loaded`.
    fn note_loaded(&mut self, cached: &impl Fn(u64) -> Cached) {
        if let Some(uid) = self.slots[AMMO_SLOT]
            && uid != 0
            && cached(uid) == Cached::Ammo
        {
            self.loaded.insert(uid);
        }
    }

    /// A frame the decoder leaves to `Parts`, read as leniently as the client reads it.
    pub(crate) fn frame(
        &mut self,
        opcode: u16,
        b: &[u8],
        live: bool,
        definitions: Option<&GameDefinitions>,
        cached: impl Fn(u64) -> Cached,
    ) {
        if UNMODELED.binary_search(&opcode).is_ok() {
            self.lost = true;
            return;
        }
        match opcode {
            // `Inventory.Special`: U64 UID; before H only in the second replay pass.
            518 | 521 => {
                let Some(uid) = u64_at(b, 0) else { return };
                if live {
                    self.special(opcode == 518, uid, true, &cached);
                } else if self.specials.len() < DEFERRED {
                    self.specials.push((opcode == 518, uid));
                } else {
                    self.lost = true;
                }
            }
            // The hook raises `Used` only for a held row; `ItemsModel.OnInventory` then
            // consumes one piece of it, maybe all.
            151 if live => {
                if let Some(uid) = u64_at(b, 0)
                    && self.held(uid, &cached) != Some(false)
                {
                    self.consumed(uid, true);
                }
            }
            837 if live => self.set_item(b, definitions.and_then(|d| d.set_bonuses.as_ref())),
            1386 => self.imprint_list(b),
            1387 => self.imprint_used(b, live, &cached),
            1487 => self.soul_state(b, live),
            1489 => self.soul_sync(b, live),
            _ => {}
        }
    }

    /// A StartSet 233: `Inventory` keeps a slot UID only when the row is already held.
    pub(crate) fn start_slots(&mut self, uids: &[u64; SLOTS], cached: impl Fn(u64) -> Cached) {
        let slots = uids.map(|uid| match uid {
            0 | 1 => Some(0),
            uid => match self.held(uid, &cached) {
                Some(true) => Some(uid),
                Some(false) => Some(0),
                None => None,
            },
        });
        self.slots = slots;
    }
    /// The frame at H closes the StartSet: `Inventory`'s second pass, then the item
    /// changes that `ImprintLink` replays.
    pub(crate) fn finish_start(&mut self, cached: impl Fn(u64) -> Cached) {
        if std::mem::replace(&mut self.started, true) {
            return;
        }
        for (equip, uid) in std::mem::take(&mut self.specials) {
            self.special(equip, uid, false, &cached);
        }
        for change in std::mem::take(&mut self.changes) {
            self.client_change(change, false);
        }
        self.note_loaded(&cached);
    }

    /// 82 and 85 after H: the `Inventory` hook, then `AvatarStats.OnEquip`, which
    /// rebuilds the snapshot when the client holds the leading UID.
    pub(crate) fn equip(
        &mut self,
        opcode: u16,
        uid: u64,
        slot: usize,
        cached: impl Fn(u64) -> Cached,
    ) {
        let before = self.slots;
        let held = self.held(uid, &cached);
        if opcode == 85 {
            self.slots[slot] = Some(0);
        } else {
            match held {
                Some(true) => self.set_slot(slot, uid),
                Some(false) => {}
                // Either nothing changes or `SetSlot` stores the UID.
                None if self.slots[slot] == Some(uid) => {}
                None => {
                    self.forget(uid);
                    self.slots[slot] = None;
                }
            }
        }
        if held == Some(true) {
            self.stale = false;
        } else if self.slots != before {
            self.stale = true;
        }
        self.note_loaded(&cached);
    }
    /// `Inventory.Special`: 518 stores a held UID in slot 20; 521 empties slot 20 when
    /// it holds that UID. Neither rebuilds the snapshot.
    fn special(&mut self, equip: bool, uid: u64, live: bool, cached: &impl Fn(u64) -> Cached) {
        let before = self.slots;
        let held = self.held(uid, cached);
        match (equip, held, self.slots[AMMO_SLOT]) {
            (_, Some(false), _) => {}
            (true, Some(true), _) => self.set_slot(AMMO_SLOT, uid),
            (true, None, current) if current != Some(uid) => {
                self.forget(uid);
                self.slots[AMMO_SLOT] = None;
            }
            (false, Some(true), Some(current)) if current == uid => self.set_slot(AMMO_SLOT, 0),
            (false, None, Some(current)) if current == uid => self.slots[AMMO_SLOT] = None,
            _ => {}
        }
        if live && self.slots != before {
            self.stale = true;
        }
        self.note_loaded(cached);
    }
    /// `Inventory.SetSlot`: `Array.IndexOf` clears the first other slot holding the UID.
    fn set_slot(&mut self, slot: usize, uid: u64) {
        if self.slots[slot] == Some(uid) {
            return;
        }
        if uid != 0
            && let Some(first) = self.slots.iter().position(|s| s.is_none_or(|v| v == uid))
        {
            if self.slots[slot].is_some() && self.slots[first].is_some() {
                self.slots[first] = Some(0);
            } else {
                self.forget(uid);
            }
        }
        self.slots[slot] = Some(uid);
    }
    /// Whether a known slot holds `uid`. Zero marks an empty slot, never a worn row.
    fn worn(&self, uid: u64) -> bool {
        uid != 0 && self.slots.contains(&Some(uid))
    }
    /// Every known slot holding `uid` becomes unproven. Clearing a slot writes zero, so
    /// an empty slot stays empty.
    fn forget(&mut self, uid: u64) {
        if uid == 0 {
            return;
        }
        for slot in &mut self.slots {
            if *slot == Some(uid) {
                *slot = None;
            }
        }
    }
    /// The first slot holding `uid`, as `Array.IndexOf` finds it, becomes empty.
    fn clear_first(&mut self, uid: u64, live: bool) {
        if uid == 0 {
            return;
        }
        let Some(first) = self.slots.iter().position(|s| s.is_none_or(|v| v == uid)) else {
            return;
        };
        if self.slots[first].is_some() {
            self.slots[first] = Some(0);
            self.stale |= live;
        } else {
            self.forget(uid);
        }
    }

    /// A Kind 1 or 2 row of a 231, before Net caches it. `same` is whether its ID, wire
    /// kind and options equal Net's cached row. `Inventory` reads 231 only in its replay;
    /// after H only Net applies it.
    pub(crate) fn list_row(
        &mut self,
        uid: u64,
        same: bool,
        live: bool,
        cached: impl Fn(u64) -> Cached,
    ) {
        if !live {
            self.put(uid, same, false);
        } else if !(same && self.held(uid, &cached) == Some(true)) {
            self.diverge(uid);
        }
    }
    /// One decoded `ReadItemChange`, before Net applies it. `Inventory` subscribes to the
    /// deltas only after its replay, and its 68 hook applies a change only to a held row.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn delta(
        &mut self,
        opcode: u16,
        op: u8,
        kind: u8,
        uid: u64,
        same: bool,
        live: bool,
        cached: impl Fn(u64) -> Cached,
    ) {
        let Some(change) = change(op, kind, uid) else {
            return;
        };
        let held = self.held(uid, &cached);
        match (live, change) {
            (false, Change::Set(uid)) => self.diverge(uid),
            (false, Change::Remove(uid)) if held != Some(false) => self.diverge(uid),
            (false, Change::Remove(_)) => {}
            (true, Change::Set(uid)) if opcode == 68 && held == Some(false) => self.diverge(uid),
            (true, Change::Set(uid)) if opcode == 68 && held.is_none() => {
                if !same && self.slots.contains(&Some(uid)) {
                    self.stale = true;
                }
            }
            (true, Change::Remove(_)) if opcode == 68 && held == Some(false) => {}
            (true, Change::Remove(uid)) if opcode == 68 && held.is_none() => {
                self.consumed(uid, true)
            }
            (true, Change::Set(uid)) => self.put(uid, same, true),
            (true, Change::Remove(uid)) => self.removed(uid, true),
        }
    }
    /// A row both Net and the client set. `same` is whether its ID, wire kind and options
    /// equal Net's old row.
    fn put(&mut self, uid: u64, same: bool, live: bool) {
        let unproven = self.diverged.remove(&uid);
        if live && (unproven || !same) && self.worn(uid) {
            self.stale = true;
        }
    }
    /// A row both Net and the client remove.
    fn removed(&mut self, uid: u64, live: bool) {
        self.diverged.remove(&uid);
        self.clear_first(uid, live);
    }
    /// A row only Net changed: the client's row, or whether it holds one, is unproven.
    fn diverge(&mut self, uid: u64) {
        if self.diverged.len() < UNPROVEN || self.diverged.contains(&uid) {
            self.diverged.insert(uid);
        } else {
            self.lost = true;
        }
    }
    /// A change only the client applies.
    fn client_change(&mut self, change: Change, live: bool) {
        match change {
            Change::Set(uid) => {
                self.diverge(uid);
                if live && self.worn(uid) {
                    self.stale = true;
                }
            }
            Change::Remove(uid) => {
                self.diverge(uid);
                self.clear_first(uid, live);
            }
        }
    }
    /// A row the client may have changed or removed, in a way Net cannot tell.
    fn consumed(&mut self, uid: u64, live: bool) {
        self.diverge(uid);
        if self.worn(uid) {
            self.forget(uid);
            self.stale |= live;
        }
    }

    /// 230 sets `GameSession.Me`. Which of several differing IDs wins is unproven.
    pub(crate) fn clock(&mut self, id: u16, live: bool) {
        if !std::mem::replace(&mut self.clock_seen, true) {
            self.me = Some(id);
        } else if self.me != Some(id) {
            self.unknown_me(live);
        }
    }
    /// 437: after H, `ProfileTitles` re-keys the own list under the new ID and the
    /// session then moves `Me`. Before H, whether `Me` moves is unproven.
    pub(crate) fn enter_zone(&mut self, id: u16, live: bool) {
        if live {
            self.me = Some(id);
        } else if self.me != Some(id) {
            self.unknown_me(false);
        }
    }
    /// Before H every `ProfileTitles` list is still empty, so only later 837s need `Me`.
    /// After H the own list may now be another character's.
    fn unknown_me(&mut self, live: bool) {
        self.me = None;
        if live {
            self.sets = None;
        }
    }
    /// 837 after H: `ServerNotices.ParseSetItem` reads the character as the low 16 bits of
    /// a U32, the set as a U32 and completion as a U8; a shorter frame throws first.
    /// `ProfileTitles.OnSetItem` appends a complete set and swap-removes one entry for an
    /// incomplete one. `KnownSet` only drops IDs without a `SetItemParam` row. Such a set
    /// adds nothing in any character's list, so Net skips it whenever `bonuses` is known.
    fn set_item(&mut self, b: &[u8], bonuses: Option<&BTreeMap<i64, [i16; 12]>>) {
        let Some(frame) = b.get(..9) else { return };
        let set = u32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
        if bonuses.is_some_and(|bonuses| !bonuses.contains_key(&i64::from(set))) {
            return;
        }
        let Some(me) = self.me else {
            self.sets = None;
            return;
        };
        let Some(sets) = &mut self.sets else { return };
        if u16::from_le_bytes([frame[0], frame[1]]) != me {
            return;
        }
        if frame[8] != 0 {
            if sets.len() >= SETS && !sets.contains_key(&set) {
                self.sets = None;
                return;
            }
            let count = sets.entry(set).or_insert(0);
            *count = count.saturating_add(1);
        } else if let Some(count) = sets.get_mut(&set) {
            *count -= 1;
            if *count == 0 {
                sets.remove(&set);
            }
        }
    }
    /// 1386: `ParseList` reads a U8 count, then 24-byte entries. On success `ImprintModel`
    /// replaces its accumulators; a throw raises no event.
    fn imprint_list(&mut self, b: &[u8]) {
        match b.split_first() {
            Some((0, _)) => self.imprints_empty = true,
            Some((&count, entries)) if entries.len() >= 24 * usize::from(count) => {
                self.imprints_empty = false;
            }
            _ => {}
        }
    }
    /// 1387: `ParseUsed` reads U32, U64 and U64, then a `ReadItemChange`. Only a complete
    /// parse applies the change to `Inventory` and raises `Used`. Before H the change
    /// waits for `ImprintLink`'s replay.
    fn imprint_used(&mut self, b: &[u8], live: bool, cached: &impl Fn(u64) -> Cached) {
        let Some((op, kind, uid)) = b.get(20..).and_then(|b| item_change(&mut Reader::new(b)))
        else {
            return;
        };
        self.imprints_empty = false;
        let change = match (op, change(op, kind, uid)) {
            (1, _) => return,
            (_, Some(change)) => change,
            // A Kind 3 or empty row is not stored.
            (2 | 4 | 6 | 8, None) => return,
            _ => {
                self.lost = true;
                return;
            }
        };
        if live {
            self.client_change(change, true);
            self.note_loaded(cached);
        } else if self.changes.len() < DEFERRED {
            self.changes.push(change);
        } else {
            self.lost = true;
        }
    }
    /// 1487 reaches `SoulLink`, which reads at most an I32 and changes no row, and
    /// `GuardianState`. There `SoulManage.ParseState` reads 37 bytes, then a U8 flag and,
    /// when it is set, a `ReadItemChange` that `Receive` applies to `Inventory`.
    fn soul_state(&mut self, b: &[u8], live: bool) {
        let changes = match b.get(37) {
            Some(0) => Some(false),
            Some(_) => b
                .get(38..)
                .and_then(|b| item_change(&mut Reader::new(b)))
                .map(|_| true),
            None => None,
        };
        self.soul(changes, live);
    }
    /// 1489 reaches only `SoulLink`. `Soul.ParseSync` reads an I32, then a U16 count of
    /// `ReadItemChange`s. `Receive` consumes a held piece for each change, then applies
    /// them all to `Inventory`.
    fn soul_sync(&mut self, b: &[u8], live: bool) {
        let mut r = Reader::new(b);
        let count = r.take(4).and_then(|_| r.u16()).ok();
        let parsed = count.filter(|&n| (0..n).all(|_| item_change(&mut r).is_some()));
        self.soul(parsed.map(|n| n > 0), live);
    }
    /// `changes`: whether a 1487 or 1489 parsed with an item change, or `None` when a read
    /// threw. The client ignores trailing bytes. A throw comes before any effect, and the
    /// session catches it per handler. `SoulLink` and `GuardianState` exist only once a UI
    /// or feature creates them, and their ctors replay the StartSet with no catch. So when
    /// a change applies is unobservable, and a StartSet frame that throws fails that install.
    fn soul(&mut self, changes: Option<bool>, live: bool) {
        match changes {
            Some(false) => {}
            None if live => {}
            _ => self.lost = true,
        }
    }

    /// `AvatarStats.Part(stat)` when every term is proven.
    pub(crate) fn value<'a>(
        &self,
        stat: usize,
        row: impl Fn(u64) -> Option<Row<'a>>,
        definitions: &GameDefinitions,
    ) -> Option<i32> {
        if self.lost || !self.started || self.stale || !self.imprints_empty {
            return None;
        }
        let equipment = self.equipment(stat, row, definitions)?;
        Some(equipment.wrapping_add(self.set_items(stat, definitions)?))
    }
    /// `AvatarStats.Equipment`: the per-row `ItemStat` sum over the snapshot.
    fn equipment<'a>(
        &self,
        stat: usize,
        row: impl Fn(u64) -> Option<Row<'a>>,
        definitions: &GameDefinitions,
    ) -> Option<i32> {
        let mut total = 0i32;
        for slot in self.slots {
            let uid = slot?;
            // `Rebuild` skips a zero UID, an empty slot, before `Inventory.Get`.
            if uid == 0 {
                continue;
            }
            if self.diverged.contains(&uid) {
                return None;
            }
            // `Inventory.Get` returns null for an absent row, and the entry is skipped.
            let Some(row) = row(uid) else { continue };
            // Ops 6 and 8 store a Kind 2 row without its options. The client then reads
            // none, but the server's options, and so its maximum, are unknown.
            if row.wire_kind == 2 && row.options.is_none() {
                return None;
            }
            if row.id == 0 {
                continue;
            }
            let stat_plus = definitions.items.get(&row.id)?.stat_plus.as_deref()?;
            let value = item_stat(stat, &row, stat_plus);
            // Loaded ammunition can leave the client's slot at any shot.
            if value != 0 && self.loaded.contains(&uid) {
                return None;
            }
            total = total.wrapping_add(value);
        }
        Some(total)
    }
    /// `AvatarStats.SetItems`: the I16 sum of the own sets' bonuses. A set ID without a
    /// `SetItemParam` row adds nothing: the client skips its null bonus row.
    fn set_items(&self, stat: usize, definitions: &GameDefinitions) -> Option<i32> {
        let sets = self.sets.as_ref()?;
        if sets.is_empty() {
            return Some(0);
        }
        let bonuses = definitions.set_bonuses.as_ref()?;
        let total = sets.iter().fold(0i32, |total, (set, &count)| {
            let bonus = bonuses.get(&i64::from(*set)).map_or(0, |b| b[stat] as u16);
            total.wrapping_add(i32::from(bonus).wrapping_mul(count as i32))
        });
        Some(i32::from(total as i16))
    }
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}
/// `ReadItemChange`: U8 op, U8 kind, then `ReadItemBody`, which reads nothing for a kind
/// outside 1..=3 and otherwise U32 ID, U64 UID, U16 count and, unless Kind 1, four state
/// bytes. Ops 2 and 4 of Kind 2 add a U8 count of 12-byte options.
fn item_change(r: &mut Reader<'_>) -> Option<(u8, u8, u64)> {
    let op = r.u8().ok()?;
    let kind = r.u8().ok()?;
    if !(1..=3).contains(&kind) {
        return Some((op, kind, 0));
    }
    r.u32().ok()?;
    let uid = r.u64().ok()?;
    r.u16().ok()?;
    if kind != 1 {
        r.take(4).ok()?;
    }
    if matches!(op, 2 | 4) && kind == 2 {
        let count = usize::from(r.u8().ok()?);
        r.take(12 * count).ok()?;
    }
    Some((op, kind, uid))
}
/// `AvatarStats.ItemStat`: the row's property `10000 + stat` when non-zero, else the
/// catalog `StatPlus[stat]` truncated to I16, or 0 past the list's end.
fn item_stat(stat: usize, row: &Row<'_>, stat_plus: &[f64]) -> i32 {
    let fallback = stat_plus
        .get(stat)
        .map_or(0, |v| i32::from(*v as i32 as i16));
    // `PropertyEntries` returns null unless Kind 2, and `Properties.Sum` of null is 0.
    let property = if row.wire_kind == 2 {
        property_sum(stat, row.options.unwrap_or(&[]), stat_plus) as i32
    } else {
        0
    };
    if property != 0 { property } else { fallback }
}
/// `ItemTip.PropertyEntries` then `Properties.Sum(10000 + stat)`. Options come first:
/// kind is the option ID, property the low 16 bits of A and value its signed high half.
/// Each non-zero `StatPlus[i]` adds a kind-10 entry unless one with that property exists.
fn property_sum(stat: usize, options: &[[u32; 3]], stat_plus: &[f64]) -> f64 {
    let mut entries: Vec<(u32, u32, f64)> = Vec::with_capacity(ENTRIES);
    for &[id, a, _] in options {
        if entries.len() < ENTRIES {
            entries.push((id, a & 0xffff, f64::from((a >> 16) as u16 as i16)));
        }
    }
    for (i, &value) in stat_plus.iter().enumerate() {
        let property = 10000 + i as u32;
        if value != 0.0
            && entries.len() < ENTRIES
            && !entries.iter().any(|e| e.0 == 10 && e.1 == property)
        {
            entries.push((10, property, value));
        }
    }
    let property = 10000 + stat as u32;
    entries
        .iter()
        .filter(|e| e.1 == property)
        .fold(0.0, |sum, e| sum + e.2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Def, definitions};
    use std::sync::Arc;

    const HP: usize = 9;
    const MP: usize = 3;
    const WT: usize = 6;
    /// Synthetic catalog rows. `StatPlus` is zero unless listed.
    const PLAIN: u32 = 100; // HP 50, MP 20, WT 7
    const BARE: u32 = 101;
    const WIDE: u32 = 102; // HP 40000, past I16
    const AMMO: u32 = 103; // Type 29
    const HOT_AMMO: u32 = 104; // Type 29, HP 5
    const ENDLESS: u32 = 105; // Type 29 with FlagsEx 64, HP 3
    const NO_LIST: u32 = 106; // no StatPlus list
    const SHORT: u32 = 107; // two StatPlus entries
    const MISSING: u32 = 999; // no catalog row
    /// Synthetic `SetItemParam` rows.
    const SET: u32 = 7; // HP 100, MP 10
    const BIG_SET: u32 = 8; // HP 20000
    const NO_ROW_SET: u32 = 9;

    /// A row as the decoder caches it: catalog ID, wire kind, and the options when the wire
    /// carried them.
    type CachedRow = (u32, u8, Option<Vec<[u32; 3]>>);
    type Rows = BTreeMap<u64, CachedRow>;
    /// A 231 row, or an op 2 or 4 change, carries options for Kind 2 only.
    fn cached_row(id: u32, kind: u8, options: &[[u32; 3]], carried: bool) -> CachedRow {
        (id, kind, (kind == 2 && carried).then(|| options.to_vec()))
    }
    /// `Decoder::same_row`: options the wire did not carry compare as an empty list.
    fn same_row(old: Option<&CachedRow>, new: &CachedRow) -> bool {
        old.is_some_and(|(id, kind, options)| {
            (*id, *kind) == (new.0, new.1)
                && options.as_deref().unwrap_or(&[]) == new.2.as_deref().unwrap_or(&[])
        })
    }
    fn lookup(rows: &Rows, data: &GameDefinitions, uid: u64) -> Cached {
        match rows.get(&uid) {
            None => Cached::Absent,
            Some((id, _, _)) => match data.items.get(id) {
                Some(info) if info.kind != 29 || info.flags_ex & 64 != 0 => Cached::Held,
                _ => Cached::Ammo,
            },
        }
    }
    /// `Parts` beside a stand-in for Net's cache, which every change updates the way the
    /// decoder does.
    struct Fx {
        parts: Parts,
        rows: Rows,
        data: Arc<GameDefinitions>,
    }
    fn catalog() -> Arc<GameDefinitions> {
        let ids = [PLAIN, BARE, WIDE, AMMO, HOT_AMMO, ENDLESS, NO_LIST, SHORT];
        let mut data = definitions(&ids.map(|id| (id, Def::Plain { weight: 1 })));
        let d = Arc::get_mut(&mut data).unwrap();
        for (id, stats) in [
            (PLAIN, &[(HP, 50.0), (MP, 20.0), (WT, 7.0)][..]),
            (WIDE, &[(HP, 40000.0)]),
            (HOT_AMMO, &[(HP, 5.0)]),
            (ENDLESS, &[(HP, 3.0)]),
        ] {
            let list = d.items.get_mut(&id).unwrap().stat_plus.as_mut().unwrap();
            for &(stat, value) in stats {
                list[stat] = value;
            }
        }
        for id in [AMMO, HOT_AMMO, ENDLESS] {
            d.items.get_mut(&id).unwrap().kind = 29;
        }
        d.items.get_mut(&ENDLESS).unwrap().flags_ex = 64;
        d.items.get_mut(&NO_LIST).unwrap().stat_plus = None;
        d.items.get_mut(&SHORT).unwrap().stat_plus = Some(vec![1.0, 2.0]);
        let bonus = |values: &[(usize, i16)]| {
            let mut row = [0; 12];
            for &(stat, value) in values {
                row[stat] = value;
            }
            row
        };
        d.set_bonuses = Some(BTreeMap::from([
            (i64::from(SET), bonus(&[(HP, 100), (MP, 10)])),
            (i64::from(BIG_SET), bonus(&[(HP, 20000)])),
        ]));
        data
    }
    impl Fx {
        fn new() -> Self {
            Self {
                parts: Parts::default(),
                rows: Rows::new(),
                data: catalog(),
            }
        }
        fn row(&mut self, uid: u64, id: u32, kind: u8, options: &[[u32; 3]]) {
            self.rows.insert(uid, cached_row(id, kind, options, true));
        }
        /// A StartSet 233 with these slot UIDs.
        fn slots(&mut self, slots: &[(usize, u64)]) {
            let mut uids = [0; SLOTS];
            for &(slot, uid) in slots {
                uids[slot] = uid;
            }
            let Self { parts, rows, data } = self;
            parts.start_slots(&uids, |uid| lookup(rows, data, uid));
        }
        fn finish(&mut self) {
            let Self { parts, rows, data } = self;
            parts.finish_start(|uid| lookup(rows, data, uid));
        }
        fn start(&mut self, slots: &[(usize, u64)]) {
            self.slots(slots);
            self.finish();
        }
        fn frame(&mut self, opcode: u16, b: &[u8], live: bool) {
            let Self { parts, rows, data } = self;
            parts.frame(opcode, b, live, Some(&**data), |uid| {
                lookup(rows, data, uid)
            });
        }
        fn equip(&mut self, opcode: u16, uid: u64, slot: usize) {
            let Self { parts, rows, data } = self;
            parts.equip(opcode, uid, slot, |uid| lookup(rows, data, uid));
        }
        fn list(&mut self, uid: u64, id: u32, kind: u8, options: &[[u32; 3]], live: bool) {
            let row = cached_row(id, kind, options, true);
            let same = same_row(self.rows.get(&uid), &row);
            let Self { parts, rows, data } = self;
            parts.list_row(uid, same, live, |uid| lookup(rows, data, uid));
            rows.insert(uid, row);
        }
        #[allow(clippy::too_many_arguments)]
        fn delta(
            &mut self,
            opcode: u16,
            op: u8,
            uid: u64,
            id: u32,
            kind: u8,
            options: &[[u32; 3]],
            live: bool,
        ) {
            let row = cached_row(id, kind, options, matches!(op, 2 | 4));
            let same = same_row(self.rows.get(&uid), &row);
            let Self { parts, rows, data } = self;
            parts.delta(opcode, op, kind, uid, same, live, |uid| {
                lookup(rows, data, uid)
            });
            match op {
                2 | 4 | 6 | 8 if matches!(kind, 1 | 2) => {
                    rows.insert(uid, row);
                }
                3 | 5 | 7 => {
                    rows.remove(&uid);
                }
                _ => {}
            }
        }
        fn values(&self) -> [Option<i32>; 3] {
            [HP, MP, WT].map(|stat| {
                let row = |uid| {
                    self.rows.get(&uid).map(|(id, kind, options)| Row {
                        id: *id,
                        wire_kind: *kind,
                        options: options.as_deref(),
                    })
                };
                self.parts.value(stat, row, &self.data)
            })
        }
    }
    /// An option row: ID, then property in the low half of A and value in its high half.
    fn opt(id: u32, property: u16, value: i16) -> [u32; 3] {
        [id, u32::from(property) | (u32::from(value as u16) << 16), 0]
    }
    fn uid(uid: u64) -> [u8; 8] {
        uid.to_le_bytes()
    }
    /// One `ReadItemChange` of ID 1 and count 1, with no options.
    fn change_bytes(op: u8, kind: u8, uid: u64) -> Vec<u8> {
        let mut b = vec![op, kind];
        if (1..=3).contains(&kind) {
            b.extend(1u32.to_le_bytes());
            b.extend(uid.to_le_bytes());
            b.extend(1u16.to_le_bytes());
            if kind != 1 {
                b.extend([0; 4]);
            }
            if matches!(op, 2 | 4) && kind == 2 {
                b.push(0);
            }
        }
        b
    }
    /// A 1387 payload: U32, U64 and U64, then one `ReadItemChange`.
    fn used(op: u8, kind: u8, uid: u64) -> Vec<u8> {
        let mut b = vec![0; 20];
        b.extend(change_bytes(op, kind, uid));
        b
    }
    /// A 1489 payload: an I32, then a U16 count and that many `ReadItemChange`s.
    fn sync(changes: &[&[u8]]) -> Vec<u8> {
        let mut b = vec![0; 4];
        b.extend((changes.len() as u16).to_le_bytes());
        changes.iter().for_each(|change| b.extend(*change));
        b
    }
    /// A 1487 payload: 37 bytes, then the change flag and the bytes after it.
    fn state(flag: u8, rest: &[u8]) -> Vec<u8> {
        let mut b = vec![0; 37];
        b.push(flag);
        b.extend(rest);
        b
    }
    /// An 837 payload. Only the low 16 bits of the leading U32 name the character.
    fn set_item(id: u16, set: u32, complete: bool) -> Vec<u8> {
        let mut b = (u32::from(id) | 0x5a5a_0000).to_le_bytes().to_vec();
        b.extend(set.to_le_bytes());
        b.push(u8::from(complete));
        b
    }
    const NONE: [Option<i32>; 3] = [None; 3];
    const ZERO: [Option<i32>; 3] = [Some(0); 3];
    const PLAIN_SUM: [Option<i32>; 3] = [Some(50), Some(20), Some(7)];

    #[test]
    fn unmodelled_opcodes_are_sorted_and_disjoint_from_modelled_ones() {
        assert!(UNMODELED.windows(2).all(|w| w[0] < w[1]));
        // Modelled, or hooks that only emit events (35, 41, 83, 86) or do nothing (261).
        for opcode in [
            15, 35, 41, 68, 82, 83, 85, 86, 151, 160, 230, 231, 233, 246, 260, 261, 262, 437, 518,
            521, 561, 770, 837, 843, 999, 1386, 1387, 1389, 1487, 1489,
        ] {
            assert!(UNMODELED.binary_search(&opcode).is_err(), "{opcode}");
        }
    }
    #[test]
    fn soul_frames_without_an_item_change_keep_the_term() {
        let mut fx = Fx::new();
        fx.row(11, PLAIN, 1, &[]);
        fx.slots(&[(0, 11)]);
        // The lazy ctors replay these StartSet frames without a throw.
        fx.frame(1489, &sync(&[]), false);
        fx.frame(1487, &state(0, &[]), false);
        fx.finish();
        // The login frames after H: a 6-byte 1489 and a 38-byte 1487.
        fx.frame(1489, &sync(&[]), true);
        fx.frame(1487, &state(0, &[]), true);
        assert_eq!(fx.values(), PLAIN_SUM);
        // The client ignores trailing bytes.
        let remove = change_bytes(3, 1, 11);
        let mut b = sync(&[]);
        b.extend(&remove);
        fx.frame(1489, &b, true);
        fx.frame(1487, &state(0, &remove), true);
        assert_eq!(fx.values(), PLAIN_SUM);
    }
    #[test]
    fn a_soul_frame_with_an_item_change_loses_the_term() {
        let set = change_bytes(2, 1, 12);
        let remove = change_bytes(3, 1, 13);
        for live in [false, true] {
            for (opcode, b) in [
                (1489, sync(&[&set])),
                (1489, sync(&[&set, &remove])),
                (1487, state(1, &set)),
            ] {
                let mut fx = Fx::new();
                if live {
                    fx.start(&[]);
                }
                fx.frame(opcode, &b, live);
                fx.start(&[]);
                assert_eq!(fx.values(), NONE, "{opcode} {live}");
            }
        }
    }
    #[test]
    fn a_soul_frame_that_throws_loses_the_term_only_in_the_start_set() {
        let set = change_bytes(2, 2, 12);
        let cut = &set[..set.len() - 1];
        let mut counted = sync(&[]);
        counted[4] = 1;
        let thrown = [
            (1489, vec![0; 5]),
            // The login length cannot hold a change, whatever the count says.
            (1489, counted),
            (1489, sync(&[&set, cut])),
            // `SoulLink` returns early below 4 bytes; `GuardianState` still throws.
            (1487, vec![0; 3]),
            (1487, vec![0; 37]),
            (1487, state(1, &[])),
            (1487, state(1, cut)),
        ];
        // The session catches each throw before any effect.
        let mut fx = Fx::new();
        fx.start(&[]);
        for (opcode, b) in &thrown {
            fx.frame(*opcode, b, true);
        }
        assert_eq!(fx.values(), ZERO);
        // The StartSet replay in a later `SoulLink` or `GuardianState` ctor rethrows.
        for (opcode, b) in thrown {
            let mut fx = Fx::new();
            fx.frame(opcode, &b, false);
            fx.start(&[]);
            assert_eq!(fx.values(), NONE, "{opcode} {}", b.len());
        }
    }
    #[test]
    fn nothing_is_proven_before_the_start_set_closes() {
        let mut fx = Fx::new();
        assert_eq!(fx.values(), NONE);
        fx.slots(&[]);
        assert_eq!(fx.values(), NONE);
        fx.finish();
        assert_eq!(fx.values(), ZERO);
    }
    #[test]
    fn a_non_zero_option_property_wins_over_the_catalog_bonus() {
        let mut fx = Fx::new();
        // Wire Kind 1: no property entries, so the catalog bonus.
        fx.row(11, PLAIN, 1, &[]);
        // An option property; the zero catalog bonus adds no entry.
        fx.row(12, BARE, 2, &[opt(5, 10009, 30)]);
        // A kind-10 option suppresses the catalog entry for its property only.
        fx.row(13, PLAIN, 2, &[opt(10, 10009, -5)]);
        // Option and catalog entries cancel out, so the I16 catalog fallback applies.
        fx.row(14, PLAIN, 2, &[opt(5, 10009, -50)]);
        fx.start(&[(0, 11), (1, 12), (2, 13), (3, 14)]);
        assert_eq!(fx.values(), [Some(125), Some(60), Some(21)]);
    }
    #[test]
    fn a_worn_kind_2_row_without_wire_options_is_unknown() {
        let mut fx = Fx::new();
        fx.row(11, PLAIN, 2, &[]);
        fx.start(&[(0, 11)]);
        assert_eq!(fx.values(), PLAIN_SUM);
        // Op 6 stores the row without options: the client reads none, the server may not.
        fx.delta(561, 6, 11, PLAIN, 2, &[], true);
        assert_eq!(fx.values(), NONE);
        // A live 231 brings the server's options back into Net's cache.
        fx.list(11, PLAIN, 2, &[], true);
        assert_eq!(fx.values(), PLAIN_SUM);
        // An unworn row without options does not matter.
        fx.delta(561, 8, 12, PLAIN, 2, &[], true);
        assert_eq!(fx.values(), PLAIN_SUM);
    }
    #[test]
    fn property_entries_stop_at_32_and_skip_the_i16_truncation() {
        let mut fx = Fx::new();
        fx.row(11, WIDE, 1, &[]);
        fx.row(12, WIDE, 2, &[]);
        let mut full = vec![opt(5, 10000, 1); 32];
        full.push(opt(5, 10009, 30));
        fx.row(13, BARE, 2, &full);
        let mut room = vec![opt(5, 10000, 1); 31];
        room.push(opt(5, 10009, 30));
        fx.row(14, BARE, 2, &room);
        fx.start(&[(0, 11), (1, 12), (2, 13), (3, 14)]);
        assert_eq!(
            fx.values()[0],
            Some(i32::from(40000_u16 as i16) + 40000 + 30)
        );
    }
    #[test]
    fn zero_ids_short_lists_and_absent_rows_add_nothing() {
        let mut fx = Fx::new();
        fx.row(11, 0, 1, &[]);
        fx.row(12, SHORT, 1, &[]);
        fx.row(13, PLAIN, 1, &[]);
        fx.row(14, BARE, 1, &[]);
        // The 233 lists row 3 twice; removing it clears only the first slot.
        fx.start(&[(0, 11), (1, 12), (2, 13), (3, 13)]);
        assert_eq!(fx.values(), [Some(100), Some(40), Some(14)]);
        fx.delta(561, 3, 13, PLAIN, 1, &[], true);
        assert_eq!(fx.values(), NONE);
        // After a rebuild the remaining slot names an absent row, which `Get` skips.
        fx.equip(85, 14, 9);
        assert_eq!(fx.values(), ZERO);
    }
    #[test]
    fn a_missing_catalog_row_or_bonus_list_is_unknown() {
        for id in [MISSING, NO_LIST] {
            let mut fx = Fx::new();
            fx.row(11, id, 1, &[]);
            fx.start(&[(0, 11)]);
            assert_eq!(fx.values(), NONE, "{id}");
        }
    }
    #[test]
    fn start_set_slots_keep_only_rows_held_at_the_233() {
        let mut fx = Fx::new();
        fx.list(11, PLAIN, 1, &[], false);
        fx.slots(&[(0, 11), (1, 12), (2, 11)]);
        // A later 231 does not repair the slot of a row listed too late.
        fx.list(12, PLAIN, 1, &[], false);
        fx.finish();
        assert_eq!(fx.values(), [Some(100), Some(40), Some(14)]);
    }
    #[test]
    fn start_set_specials_run_after_the_first_pass() {
        let mut fx = Fx::new();
        fx.frame(518, &uid(11), false);
        fx.list(11, PLAIN, 1, &[], false);
        fx.start(&[]);
        assert_eq!(fx.values(), PLAIN_SUM);
        let mut fx = Fx::new();
        fx.frame(518, &uid(11), false);
        fx.frame(521, &uid(11), false);
        fx.list(11, PLAIN, 1, &[], false);
        fx.start(&[]);
        assert_eq!(fx.values(), ZERO);
    }
    #[test]
    fn start_set_deltas_diverge_until_a_231_lists_the_row() {
        let mut fx = Fx::new();
        fx.delta(561, 2, 11, PLAIN, 1, &[], false);
        fx.start(&[(0, 11)]);
        assert_eq!(fx.values(), NONE);
        let mut fx = Fx::new();
        fx.delta(561, 2, 11, PLAIN, 1, &[], false);
        fx.list(11, PLAIN, 1, &[], false);
        fx.start(&[(0, 11)]);
        assert_eq!(fx.values(), PLAIN_SUM);
        // A StartSet removal of a row neither side holds changes nothing.
        let mut fx = Fx::new();
        fx.delta(561, 3, 0, 0, 3, &[], false);
        fx.start(&[]);
        assert_eq!(fx.values(), ZERO);
    }
    #[test]
    fn start_set_imprint_changes_apply_after_the_inventory_replay() {
        let mut fx = Fx::new();
        fx.list(11, PLAIN, 1, &[], false);
        fx.frame(1387, &used(3, 1, 11), false);
        fx.frame(1386, &[0], false);
        // The 233 still sees row 1; the replayed removal then clears its slot.
        fx.start(&[(0, 11)]);
        assert_eq!(fx.values(), ZERO);
    }
    #[test]
    fn live_equip_rebuilds_only_when_the_client_holds_the_uid() {
        let mut fx = Fx::new();
        fx.row(11, PLAIN, 1, &[]);
        fx.row(12, BARE, 1, &[]);
        fx.start(&[]);
        fx.equip(82, 11, 4);
        assert_eq!(fx.values(), PLAIN_SUM);
        // `SetSlot` clears the slot the UID held before.
        fx.equip(82, 11, 6);
        assert_eq!(fx.values(), PLAIN_SUM);
        // An unheld leading UID: the slot clears but the snapshot is not rebuilt.
        fx.equip(85, 99, 6);
        assert_eq!(fx.values(), NONE);
        fx.equip(82, 99, 6);
        assert_eq!(fx.values(), NONE);
        fx.equip(85, 12, 0);
        assert_eq!(fx.values(), ZERO);
    }
    #[test]
    fn live_specials_move_slot_20_without_a_rebuild() {
        let mut fx = Fx::new();
        fx.row(11, PLAIN, 1, &[]);
        fx.row(12, BARE, 1, &[]);
        fx.start(&[]);
        fx.frame(518, &uid(11), true);
        assert_eq!(fx.values(), NONE);
        fx.equip(85, 12, 0);
        assert_eq!(fx.values(), PLAIN_SUM);
        fx.frame(521, &uid(12), true);
        fx.frame(518, &[1, 0, 0], true);
        assert_eq!(fx.values(), PLAIN_SUM);
        fx.frame(521, &uid(11), true);
        assert_eq!(fx.values(), NONE);
        fx.equip(85, 12, 0);
        assert_eq!(fx.values(), ZERO);
    }
    #[test]
    fn worn_row_changes_wait_for_a_rebuild() {
        let mut fx = Fx::new();
        // A kind-5 option adds to the catalog's kind-10 entry: 30 + 50.
        let worn = [opt(5, 10009, 30)];
        fx.row(11, PLAIN, 2, &worn);
        fx.row(12, BARE, 1, &[]);
        fx.start(&[(0, 11)]);
        assert_eq!(fx.values(), [Some(80), Some(20), Some(7)]);
        fx.delta(561, 2, 11, PLAIN, 2, &worn, true);
        fx.delta(561, 1, 11, PLAIN, 2, &[], true);
        fx.delta(561, 2, 12, BARE, 2, &[], true);
        assert_eq!(fx.values(), [Some(80), Some(20), Some(7)]);
        // The snapshot keeps the old clone of a changed worn row.
        fx.delta(561, 2, 11, PLAIN, 2, &[opt(5, 10009, 40)], true);
        assert_eq!(fx.values(), NONE);
        fx.equip(85, 12, 9);
        assert_eq!(fx.values(), [Some(90), Some(20), Some(7)]);
        fx.delta(561, 3, 11, PLAIN, 2, &[], true);
        assert_eq!(fx.values(), NONE);
        fx.equip(85, 12, 9);
        assert_eq!(fx.values(), ZERO);
        fx.delta(561, 3, 12, BARE, 2, &[], true);
        assert_eq!(fx.values(), ZERO);
    }
    #[test]
    fn a_68_changes_only_a_held_row() {
        let mut fx = Fx::new();
        fx.row(11, PLAIN, 1, &[]);
        fx.start(&[(0, 11)]);
        fx.delta(68, 3, 99, BARE, 1, &[], true);
        assert_eq!(fx.values(), PLAIN_SUM);
        // Only Net creates this row, so the client may not hold it for an 82.
        fx.delta(68, 2, 99, PLAIN, 1, &[], true);
        assert_eq!(fx.values(), PLAIN_SUM);
        fx.equip(82, 99, 1);
        assert_eq!(fx.values(), NONE);
    }
    #[test]
    fn a_live_231_row_diverges_unless_identical() {
        let mut fx = Fx::new();
        fx.row(11, PLAIN, 1, &[]);
        fx.start(&[(0, 11)]);
        fx.list(11, PLAIN, 1, &[], true);
        assert_eq!(fx.values(), PLAIN_SUM);
        fx.list(11, BARE, 1, &[], true);
        assert_eq!(fx.values(), NONE);
        // A later delta of the row makes both sides agree again, but the snapshot is old.
        fx.delta(561, 2, 11, BARE, 1, &[], true);
        assert_eq!(fx.values(), NONE);
    }
    #[test]
    fn unproven_rows_past_the_cap_lose_the_term() {
        let mut fx = Fx::new();
        for uid in 1..=UNPROVEN as u64 {
            fx.delta(561, 2, uid, PLAIN, 1, &[], false);
        }
        fx.start(&[]);
        assert_eq!(fx.values(), ZERO);
        // A UID that is already unproven takes no more room.
        fx.list(1, BARE, 1, &[], true);
        assert_eq!(fx.values(), ZERO);
        fx.list(UNPROVEN as u64 + 1, BARE, 1, &[], true);
        assert_eq!(fx.values(), NONE);
    }
    #[test]
    fn a_used_row_may_be_consumed() {
        let mut fx = Fx::new();
        fx.row(11, PLAIN, 1, &[]);
        fx.row(12, BARE, 1, &[]);
        fx.start(&[(0, 11)]);
        fx.frame(151, &uid(99), true);
        fx.frame(151, &[1; 7], true);
        fx.frame(151, &uid(12), true);
        fx.frame(151, &uid(11), false);
        assert_eq!(fx.values(), PLAIN_SUM);
        fx.frame(151, &uid(11), true);
        assert_eq!(fx.values(), NONE);
    }
    #[test]
    fn the_imprint_bonus_is_zero_only_while_the_list_is_provably_empty() {
        let mut fx = Fx::new();
        fx.start(&[]);
        assert_eq!(fx.values(), ZERO);
        // `ParseList` throws on a short list and raises no event.
        fx.frame(1386, &[1; 24], true);
        assert_eq!(fx.values(), ZERO);
        let mut one = vec![1];
        one.extend([0; 24]);
        fx.frame(1386, &one, true);
        assert_eq!(fx.values(), NONE);
        fx.frame(1389, &[], true);
        assert_eq!(fx.values(), NONE);
        fx.frame(1386, &[0], true);
        assert_eq!(fx.values(), ZERO);
        let mut truncated = used(2, 2, 5);
        truncated.truncate(truncated.len() - 2);
        fx.frame(1387, &[0; 21], true);
        fx.frame(1387, &truncated, true);
        assert_eq!(fx.values(), ZERO);
        for change in [used(1, 1, 5), used(2, 9, 0), used(2, 3, 5)] {
            fx.frame(1387, &change, true);
            assert_eq!(fx.values(), NONE);
            fx.frame(1386, &[0], true);
            assert_eq!(fx.values(), ZERO);
        }
        // `Inventory.Apply` has no case for this op.
        fx.frame(1387, &used(9, 1, 5), true);
        fx.frame(1386, &[0], true);
        assert_eq!(fx.values(), NONE);
    }
    #[test]
    fn a_live_1387_change_reaches_the_client_inventory() {
        let mut fx = Fx::new();
        fx.row(11, PLAIN, 1, &[]);
        fx.row(12, BARE, 1, &[]);
        fx.start(&[(0, 11)]);
        fx.frame(1387, &used(3, 1, 11), true);
        fx.frame(1386, &[0], true);
        assert_eq!(fx.values(), NONE);
        fx.equip(85, 12, 9);
        assert_eq!(fx.values(), ZERO);
        // A replaced worn row is unproven until a decoded change sets it again.
        fx.equip(82, 12, 0);
        fx.frame(1387, &used(2, 1, 12), true);
        fx.frame(1386, &[0], true);
        assert_eq!(fx.values(), NONE);
        fx.delta(561, 2, 12, PLAIN, 1, &[], true);
        fx.equip(85, 12, 9);
        assert_eq!(fx.values(), PLAIN_SUM);
    }
    #[test]
    fn own_set_bonuses_follow_live_837_frames() {
        let mut fx = Fx::new();
        fx.parts.clock(5, false);
        fx.frame(837, &set_item(5, SET, true), false);
        fx.start(&[]);
        assert_eq!(fx.values(), ZERO);
        fx.frame(837, &set_item(5, SET, true), true);
        assert_eq!(fx.values(), [Some(100), Some(10), Some(0)]);
        fx.frame(837, &set_item(6, SET, true), true);
        fx.frame(837, &set_item(5, NO_ROW_SET, true), true);
        fx.frame(837, &set_item(5, BIG_SET, true)[..8], true);
        assert_eq!(fx.values(), [Some(100), Some(10), Some(0)]);
        fx.frame(837, &set_item(5, SET, true), true);
        assert_eq!(fx.values(), [Some(200), Some(20), Some(0)]);
        fx.frame(837, &set_item(5, SET, false), true);
        fx.frame(837, &set_item(5, BIG_SET, false), true);
        assert_eq!(fx.values(), [Some(100), Some(10), Some(0)]);
        // The I16 sum wraps.
        fx.frame(837, &set_item(5, BIG_SET, true), true);
        fx.frame(837, &set_item(5, BIG_SET, true), true);
        assert_eq!(fx.values()[0], Some(40100_i32 as i16 as i32));
        // 437 keeps the own list under the new ID.
        fx.parts.enter_zone(9, true);
        fx.frame(837, &set_item(5, SET, false), true);
        fx.frame(837, &set_item(9, BIG_SET, false), true);
        assert_eq!(fx.values(), [Some(20100), Some(10), Some(0)]);
    }
    #[test]
    fn set_bonuses_need_me_and_the_bonus_table() {
        // Before H every list is empty, so an unknown `Me` matters only for a later 837.
        let mut fx = Fx::new();
        fx.parts.clock(5, false);
        fx.parts.clock(6, false);
        fx.start(&[]);
        assert_eq!(fx.values(), ZERO);
        // A set without a bonus row adds nothing, whichever character holds it.
        fx.frame(837, &set_item(5, NO_ROW_SET, true), true);
        assert_eq!(fx.values(), ZERO);
        fx.frame(837, &set_item(5, SET, true), true);
        assert_eq!(fx.values(), NONE);
        let mut fx = Fx::new();
        fx.parts.clock(5, false);
        fx.parts.enter_zone(6, false);
        fx.start(&[]);
        fx.parts.enter_zone(7, true);
        fx.frame(837, &set_item(7, SET, true), true);
        assert_eq!(fx.values(), [Some(100), Some(10), Some(0)]);
        // A live 230 naming another ID may move `Me` to another character's list.
        fx.parts.clock(7, true);
        assert_eq!(fx.values(), [Some(100), Some(10), Some(0)]);
        fx.parts.clock(8, true);
        assert_eq!(fx.values(), NONE);
        // JSON definitions carry no `SetItemParam` table.
        let mut fx = Fx::new();
        Arc::get_mut(&mut fx.data).unwrap().set_bonuses = None;
        fx.parts.clock(5, false);
        fx.start(&[]);
        assert_eq!(fx.values(), ZERO);
        fx.frame(837, &set_item(5, NO_ROW_SET, true), true);
        assert_eq!(fx.values(), NONE);
    }
    #[test]
    fn sets_without_a_bonus_row_take_no_room() {
        let mut fx = Fx::new();
        fx.parts.clock(5, false);
        fx.start(&[]);
        for set in 100..=100 + SETS as u32 {
            fx.frame(837, &set_item(5, set, true), true);
        }
        fx.frame(837, &set_item(5, SET, true), true);
        assert_eq!(fx.values(), [Some(100), Some(10), Some(0)]);
    }
    #[test]
    fn own_sets_past_the_cap_are_unknown() {
        /// Completes, then breaks, sets 0 to `sets - 1`.
        fn cycle(fx: &mut Fx, sets: u32) {
            for complete in [true, false] {
                for set in 0..sets {
                    fx.frame(837, &set_item(5, set, complete), true);
                }
            }
        }
        // Without a `SetItemParam` table every set is kept.
        let mut fx = Fx::new();
        Arc::get_mut(&mut fx.data).unwrap().set_bonuses = None;
        fx.parts.clock(5, false);
        fx.start(&[]);
        cycle(&mut fx, SETS as u32);
        assert_eq!(fx.values(), ZERO);
        cycle(&mut fx, SETS as u32 + 1);
        assert_eq!(fx.values(), NONE);
    }
    #[test]
    fn a_set_count_saturates() {
        let mut fx = Fx::new();
        fx.parts.clock(5, false);
        fx.start(&[]);
        let full = Some(BTreeMap::from([(SET, u32::MAX)]));
        fx.parts.sets = full.clone();
        fx.frame(837, &set_item(5, SET, true), true);
        assert_eq!(fx.parts.sets, full);
    }
    #[test]
    fn loaded_ammunition_must_add_nothing() {
        let mut fx = Fx::new();
        fx.row(11, AMMO, 1, &[]);
        fx.row(12, HOT_AMMO, 1, &[]);
        fx.row(13, ENDLESS, 1, &[]);
        fx.row(14, BARE, 1, &[]);
        fx.start(&[(20, 11)]);
        assert_eq!(fx.values(), ZERO);
        fx.frame(518, &uid(12), true);
        fx.equip(85, 14, 0);
        assert_eq!(fx.values(), [None, Some(0), Some(0)]);
        // `FlagsEx` bit 64 ammunition is never consumed locally.
        fx.frame(518, &uid(13), true);
        fx.equip(85, 14, 0);
        assert_eq!(fx.values(), [Some(3), Some(0), Some(0)]);
        // The client may already have consumed ammunition it loaded.
        fx.equip(82, 12, 5);
        assert_eq!(fx.values(), NONE);
    }
    #[test]
    fn empty_slots_never_read_a_uid_zero_row() {
        let mut fx = Fx::new();
        fx.list(0, PLAIN, 1, &[], false);
        fx.list(11, PLAIN, 1, &[], false);
        fx.start(&[(0, 11)]);
        // `Rebuild` skips a zero UID before `Inventory.Get`.
        assert_eq!(fx.values(), PLAIN_SUM);
        // So the UID-0 row is never worn, whatever happens to it.
        fx.delta(561, 2, 0, WIDE, 1, &[], true);
        fx.list(0, PLAIN, 1, &[], true);
        fx.frame(151, &uid(0), true);
        assert_eq!(fx.values(), PLAIN_SUM);
        // Clearing a slot writes zero, so an 82 that may store UID 0 in slot 0 leaves the
        // empty slots empty.
        fx.equip(82, 0, 0);
        assert_eq!(fx.values(), NONE);
        fx.equip(82, 11, 0);
        assert_eq!(fx.values(), PLAIN_SUM);
    }
    #[test]
    fn an_unmodelled_frame_loses_the_term() {
        for live in [false, true] {
            let mut fx = Fx::new();
            if live {
                fx.start(&[]);
            }
            fx.frame(UNMODELED[0], &[], live);
            fx.start(&[]);
            assert_eq!(fx.values(), NONE);
        }
        let mut fx = Fx::new();
        fx.start(&[]);
        fx.parts.lose();
        assert_eq!(fx.values(), NONE);
    }
}
