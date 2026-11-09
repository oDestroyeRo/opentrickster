//! `CharacterStatus.Total` for maximum HP (stat 9), MP (stat 3) and weight (stat 6) on
//! build ea1a68bd. `crate::parts` supplies the `GameParts` term.
//! docs/research/derived-resource-maxima.md cites every input.
use crate::{Result, codec::Packet, wire::Reader};
use opentrickster_engine::Gauge;
use std::num::NonZeroU32;

const STATS: usize = 12;
type Values = [i32; STATS];
type Nullable = [Option<i32>; STATS];
/// The public player level table: 999 levels x 12 stats x 3 columns.
pub(crate) const PLAYER_LEVELS: usize = 999 * STATS * 3;

/// The client's `CharacterStatus` inputs, applied in arrival order. Frames through the
/// startup cutoff H are the client's StartSet, which `CharacterStatus` reads only through
/// `ApplyStart`; later frames reach its hooks. `None` marks an input Net cannot prove.
#[cfg_attr(test, derive(Clone, Debug, PartialEq))]
pub(crate) struct Status {
    /// S282 `Totals`, cleared by the recompute events Net has always recognised.
    totals: Nullable,
    /// The startup cutoff H, once known. Until then every decoded frame precedes it.
    cutoff: Option<u64>,
    /// A StartSet 233 recomputes after `SoulSeedLink` has replayed the whole StartSet.
    start_pending: bool,
    points: Option<[u16; STATS]>,
    buffs: Values,
    overrides: Option<Nullable>,
    aura: Option<Values>,
    /// `recomputedBase` and `recomputedAura`, copied by the last recompute.
    recomputed: Option<(Nullable, Values)>,
}
impl Default for Status {
    fn default() -> Self {
        Self {
            totals: [None; STATS],
            cutoff: None,
            start_pending: false,
            points: None,
            buffs: [0; STATS],
            overrides: Some([None; STATS]),
            aura: Some([0; STATS]),
            recomputed: Some(([None; STATS], [0; STATS])),
        }
    }
}
impl Status {
    pub(crate) fn startup(&mut self, cutoff: u64) {
        self.cutoff = Some(cutoff);
    }
    /// Whether a frame belongs to the StartSet, at or before H.
    pub(crate) fn in_start_set(&self, mark: u64) -> bool {
        self.cutoff.is_none_or(|cutoff| mark <= cutoff)
    }
    /// Whether the StartSet is complete once this frame is applied.
    pub(crate) fn closes_start_set(&self, mark: u64) -> bool {
        self.cutoff.is_some_and(|cutoff| mark >= cutoff)
    }
    pub(crate) fn observe(&mut self, p: &Packet) -> Result<()> {
        let values = match p.opcode {
            282 | 283 => Some(indexed(&p.payload)?),
            _ => None,
        };
        match p.opcode {
            282 => {
                for (stat, value) in values.iter().flatten() {
                    if let Some(total) = self.totals.get_mut(*stat as usize) {
                        *total = Some(*value);
                    }
                }
            }
            283 | 233 | 39 | 82 | 85 | 109 | 110 | 437 | 518 | 521 | 561 | 160 | 770 | 262
            | 246 | 260 | 843 | 68 => self.totals = [None; STATS],
            _ => {}
        }
        if self.in_start_set(p.mark) {
            self.start(p);
        } else {
            self.live(p, values.as_deref());
        }
        if self.start_pending && self.closes_start_set(p.mark) {
            self.start_pending = false;
            self.recompute(true);
        }
        Ok(())
    }
    /// StartSet: `ApplyStart` reads only 233, and `SoulSeedLink` replays its own opcodes
    /// before `CharacterStatus` subscribes to aura events.
    fn start(&mut self, p: &Packet) {
        match p.opcode {
            233 if p.payload.len() >= 217 => {
                self.points = block(&p.payload, 184);
                self.start_pending = true;
            }
            1065 | 1070 | 1071 => self.seed(p),
            _ => {}
        }
    }
    fn live(&mut self, p: &Packet, values: Option<&[(u8, i32)]>) {
        let b = &p.payload;
        match p.opcode {
            39 | 629 => self.apply_block(b, 0),
            // The handler slices bytes 33..37 before the block, so a shorter frame throws.
            634 if b.len() < 37 => self.points = None,
            634 => self.apply_block(b, 0),
            649 => self.apply_block(b, 69),
            109 => {
                self.points = b
                    .get(2..26)
                    .map(|_| std::array::from_fn(|s| u16_at(b, 2 + 2 * s)));
                if self.points.is_some() {
                    self.recompute(true);
                }
            }
            283 => {
                for &(stat, value) in values.into_iter().flatten() {
                    if let Some(buff) = self.buffs.get_mut(stat as usize) {
                        *buff = value;
                    }
                }
                self.recompute(false);
            }
            784 => match (indexed(b), self.overrides.as_mut()) {
                (Ok(values), Some(overrides)) => {
                    for (stat, value) in values {
                        if let Some(entry) = overrides.get_mut(stat as usize) {
                            *entry = Some(value);
                        }
                    }
                }
                _ => self.overrides = None,
            },
            1065 | 1070 | 1071 => {
                self.seed(p);
                if p.opcode != 1065 && self.aura.is_some() {
                    self.recompute(false);
                }
            }
            // `AvatarStats.OnEquip` and `OnSetItem` recompute only after the UI install.
            82 | 85 | 837 => self.maybe_recompute(false),
            // `ImprintLink` raises an event, and so recomputes, only when its parser succeeds.
            1386 | 1387 | 1389 => self.maybe_recompute(true),
            _ => {}
        }
    }
    /// `SoulSeedLink.Receive`: the live aura bonus vector.
    fn seed(&mut self, p: &Packet) {
        match p.opcode {
            // `ParsePlanted` runs before the reset, so a non-zero aura may survive a throw.
            1065 if self.aura != Some([0; STATS]) => self.aura = None,
            1065 => {}
            1070 => {
                self.aura = p
                    .payload
                    .get(12..60)
                    .map(|_| std::array::from_fn(|s| i32_at(&p.payload, 12 + 4 * s)));
            }
            1071 => self.aura = Some([0; STATS]),
            _ => {}
        }
    }
    /// `ApplyBlock`: a stat block at `offset` replaces the points and recomputes.
    fn apply_block(&mut self, b: &[u8], offset: usize) {
        if let Some(points) = block(b, offset) {
            self.points = Some(points);
            self.recompute(true);
        }
    }
    /// `Recompute(clear)`: copies overrides and the live aura into the recomputed vectors.
    fn recompute(&mut self, clear: bool) {
        if clear {
            self.overrides = Some([None; STATS]);
        }
        self.recomputed = self.overrides.zip(self.aura);
    }
    /// A recompute Net cannot prove ran: whatever it would change becomes unknown.
    fn maybe_recompute(&mut self, clear: bool) {
        let overrides = if clear {
            Some([None; STATS])
        } else {
            self.overrides
        };
        if overrides != self.overrides {
            self.overrides = None;
        }
        if overrides.zip(self.aura) != self.recomputed {
            self.recomputed = None;
        }
    }
    /// `Total` without `GameParts`: `(recomputedBase ?? base) + Buffs + recomputedAura`.
    fn core(&self, stat: usize, player: &[f64]) -> Option<i32> {
        if self.start_pending {
            return None;
        }
        let (base, aura) = self.recomputed?;
        let base = match base[stat] {
            Some(base) => base,
            None => level_base(self.points?[stat], stat, player)?,
        };
        Some(base.wrapping_add(self.buffs[stat]).wrapping_add(aura[stat]))
    }
    /// A received S282 total, else `CharacterStatus.Total` with `GameParts` term `parts`
    /// when every term is proven.
    pub(crate) fn maximum(
        &self,
        stat: usize,
        player: Option<&[f64]>,
        parts: Option<i32>,
    ) -> Option<NonZeroU32> {
        let total = match self.totals[stat] {
            Some(total) => total,
            None => parts?.wrapping_add(self.core(stat, player?)?),
        };
        u32::try_from(total).ok().and_then(NonZeroU32::new)
    }
}

/// `CombatData.StatBase`: level `points/4 + 1`, column 2; levels past 999 give 0.
fn level_base(points: u16, stat: usize, player: &[f64]) -> Option<i32> {
    if player.len() != PLAYER_LEVELS {
        return None;
    }
    let level = usize::from(points / 4);
    // `as` saturates and maps NaN to 0, as `trunc_sat` after the client's NaN check does.
    Some(if level > 998 {
        0
    } else {
        player[(level * STATS + stat) * 3 + 2] as i32
    })
}
/// `Status.ReadStatBlock`: 12 U16 points from `offset`, present only when 33 bytes fit.
fn block(b: &[u8], offset: usize) -> Option<[u16; STATS]> {
    b.get(offset..offset + 33)
        .map(|_| std::array::from_fn(|s| u16_at(b, offset + 2 * s)))
}
fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn i32_at(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
/// `Status.ParseIndexedValues` with wide values: count U8, then (stat U8, value I32).
fn indexed(b: &[u8]) -> Result<Vec<(u8, i32)>> {
    let mut r = Reader::new(b);
    let count = r.u8()?;
    let values = (0..count)
        .map(|_| Ok((r.u8()?, r.u32()? as i32)))
        .collect::<Result<Vec<_>>>()?;
    r.end()?;
    Ok(values)
}

pub(crate) fn gauge(current: u32, maximum: Option<NonZeroU32>) -> Gauge {
    Gauge {
        current: maximum.map_or(current, |max| current.min(max.get())),
        maximum,
    }
}
pub(crate) fn composite(current: u32, maximum: Option<NonZeroU32>) -> Gauge {
    Gauge {
        current,
        maximum: maximum.filter(|maximum| current <= maximum.get()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const H: u64 = 10;
    fn p(opcode: u16, mark: u64, payload: Vec<u8>) -> Packet {
        Packet {
            opcode,
            aux: 0,
            mark,
            received: std::time::Instant::now(),
            payload: zeroize::Zeroizing::new(payload),
        }
    }
    /// Synthetic levels: stat `s` at level index `l` has base `1000 * l + s`.
    fn table() -> Vec<f64> {
        (0..PLAYER_LEVELS)
            .map(|i| match i % 3 {
                2 => (1000 * (i / 36) + i / 3 % STATS) as f64,
                _ => f64::NAN,
            })
            .collect()
    }
    fn indexed_payload(values: &[(u8, i32)]) -> Vec<u8> {
        let mut b = vec![values.len() as u8];
        for (stat, value) in values {
            b.push(*stat);
            b.extend_from_slice(&value.to_le_bytes());
        }
        b
    }
    /// A stat block of `len` bytes whose points sit at `offset`; stat `s` gets `4 * level[s]`.
    fn block_payload(len: usize, offset: usize, level: impl Fn(usize) -> u16) -> Vec<u8> {
        let mut b = vec![0; len];
        for s in 0..STATS {
            b[offset + 2 * s..offset + 2 * s + 2].copy_from_slice(&(4 * level(s)).to_le_bytes());
        }
        b
    }
    fn aura_payload(len: usize, bonus: impl Fn(usize) -> i32) -> Vec<u8> {
        let mut b = vec![0; 12];
        for s in 0..STATS {
            b.extend_from_slice(&bonus(s).to_le_bytes());
        }
        b.resize(len, 0);
        b
    }
    /// The StartSet through H: a 233 at level index 2 for every stat.
    fn started() -> Status {
        let mut status = Status::default();
        status.startup(H);
        status
            .observe(&p(233, H, block_payload(251, 184, |_| 2)))
            .unwrap();
        status
    }
    fn core(status: &Status) -> [Option<i32>; 3] {
        let table = table();
        [9, 3, 6].map(|stat| status.core(stat, &table))
    }
    #[test]
    fn totals_are_nullable_partial_and_invalidated_by_recompute() {
        let mut t = Status::default();
        assert!(gauge(80, t.maximum(9, None, None)).maximum.is_none());
        let mut bytes = indexed_payload(&[(9, 100), (3, 50)]);
        t.observe(&p(282, 1, bytes.clone())).unwrap();
        assert_eq!(gauge(120, t.maximum(9, None, None)).current, 100);
        assert_eq!(t.maximum(3, None, None).unwrap().get(), 50);
        assert!(t.maximum(6, None, None).is_none());
        bytes.pop();
        assert!(t.observe(&p(282, 1, bytes)).is_err());
        assert_eq!(t.maximum(9, None, None).unwrap().get(), 100);
        t.observe(&p(283, 1, vec![0])).unwrap();
        assert!(t.maximum(9, None, None).is_none());
        t.observe(&p(282, 1, indexed_payload(&[(9, -1)]))).unwrap();
        assert!(t.maximum(9, None, None).is_none());
    }
    #[test]
    fn a_received_total_wins_over_game_parts_plus_core() {
        let mut status = started();
        let table = table();
        assert_eq!(core(&status), [Some(2009), Some(2003), Some(2006)]);
        let derived = |status: &Status, parts| {
            [9, 3, 6].map(|stat| status.maximum(stat, Some(&table), parts).map(|v| v.get()))
        };
        assert_eq!(derived(&status, None), [None; 3]);
        assert_eq!(
            derived(&status, Some(-9)),
            [Some(2000), Some(1994), Some(1997)]
        );
        assert_eq!(derived(&status, Some(-2009)), [None, None, None]);
        assert!(status.maximum(9, None, Some(1)).is_none());
        let total = indexed_payload(&[(9, 500)]);
        status.observe(&p(282, H + 1, total)).unwrap();
        assert_eq!(derived(&status, None), [Some(500), None, None]);
        assert_eq!(
            derived(&status, Some(1)),
            [Some(500), Some(2004), Some(2007)]
        );
    }
    #[test]
    fn the_level_table_base_needs_points_and_the_whole_table() {
        assert_eq!(core(&Status::default()), [None; 3]);
        let status = started();
        assert!(status.core(9, &table()[1..]).is_none());
        let mut status = Status::default();
        status.startup(H);
        status
            .observe(&p(233, H, block_payload(251, 184, |s| [999, 998][s % 2])))
            .unwrap();
        assert_eq!(core(&status), [Some(998_009), Some(998_003), Some(0)]);
        let mut status = Status::default();
        status.startup(H);
        status.observe(&p(233, H, vec![0; 216])).unwrap();
        assert_eq!(core(&status), [None; 3]);
    }
    #[test]
    fn the_startset_reaches_status_only_through_233_and_recomputes_at_h() {
        let mut status = Status::default();
        status
            .observe(&p(233, 1, block_payload(251, 184, |_| 2)))
            .unwrap();
        status
            .observe(&p(283, 2, indexed_payload(&[(9, 7)])))
            .unwrap();
        status
            .observe(&p(784, 3, indexed_payload(&[(3, 5)])))
            .unwrap();
        status
            .observe(&p(39, 4, block_payload(33, 0, |_| 5)))
            .unwrap();
        status
            .observe(&p(109, 5, block_payload(26, 2, |_| 5)))
            .unwrap();
        status
            .observe(&p(1070, 6, aura_payload(60, |s| s as i32)))
            .unwrap();
        assert_eq!(core(&status), [None; 3]);
        status.startup(H);
        status.observe(&p(5, H, Vec::new())).unwrap();
        assert_eq!(core(&status), [Some(2018), Some(2006), Some(2012)]);
    }
    #[test]
    fn stat_blocks_replace_points_and_short_blocks_change_nothing() {
        for (opcode, len, offset) in [(39, 33, 0), (629, 33, 0), (634, 37, 0), (649, 102, 69)] {
            let mut status = started();
            status
                .observe(&p(opcode, H + 1, block_payload(len - 1, offset, |_| 1)))
                .unwrap();
            if opcode == 634 {
                assert_eq!(core(&status), [None; 3]);
            } else {
                assert_eq!(core(&status), [Some(2009), Some(2003), Some(2006)]);
            }
            status
                .observe(&p(opcode, H + 2, block_payload(len, offset, |_| 3)))
                .unwrap();
            assert_eq!(core(&status), [Some(3009), Some(3003), Some(3006)]);
        }
        let mut status = started();
        status
            .observe(&p(109, H + 1, block_payload(26, 2, |_| 4)))
            .unwrap();
        assert_eq!(core(&status), [Some(4009), Some(4003), Some(4006)]);
        status.observe(&p(109, H + 2, vec![0; 25])).unwrap();
        assert_eq!(core(&status), [None; 3]);
    }
    #[test]
    fn buffs_overwrite_listed_stats_and_recompute_without_clearing_overrides() {
        let mut status = started();
        status
            .observe(&p(283, H + 1, indexed_payload(&[(9, 40), (12, 1)])))
            .unwrap();
        assert_eq!(core(&status), [Some(2049), Some(2003), Some(2006)]);
        status
            .observe(&p(283, H + 2, indexed_payload(&[(3, -3)])))
            .unwrap();
        assert_eq!(core(&status), [Some(2049), Some(2000), Some(2006)]);
        assert!(status.observe(&p(283, H + 3, vec![1, 9])).is_err());
    }
    #[test]
    fn base_overrides_apply_at_the_next_recompute_and_clear_on_a_block() {
        let mut status = started();
        status
            .observe(&p(784, H + 1, indexed_payload(&[(9, 77), (12, 1)])))
            .unwrap();
        assert_eq!(core(&status)[0], Some(2009));
        status
            .observe(&p(283, H + 2, indexed_payload(&[])))
            .unwrap();
        assert_eq!(core(&status), [Some(77), Some(2003), Some(2006)]);
        status
            .observe(&p(39, H + 3, block_payload(33, 0, |_| 2)))
            .unwrap();
        assert_eq!(core(&status)[0], Some(2009));
        status.observe(&p(784, H + 4, vec![1, 9])).unwrap();
        status
            .observe(&p(283, H + 5, indexed_payload(&[])))
            .unwrap();
        assert_eq!(core(&status), [None; 3]);
        status
            .observe(&p(109, H + 6, block_payload(26, 2, |_| 2)))
            .unwrap();
        assert_eq!(core(&status), [Some(2009), Some(2003), Some(2006)]);
    }
    #[test]
    fn the_aura_bonus_follows_seed_frames_and_unparsed_frames_are_unknown() {
        let mut status = started();
        status
            .observe(&p(1070, H + 1, aura_payload(60, |s| 10 * s as i32)))
            .unwrap();
        assert_eq!(core(&status), [Some(2099), Some(2033), Some(2066)]);
        status.observe(&p(1071, H + 2, Vec::new())).unwrap();
        assert_eq!(core(&status), [Some(2009), Some(2003), Some(2006)]);
        status.observe(&p(1065, H + 3, Vec::new())).unwrap();
        status
            .observe(&p(283, H + 4, indexed_payload(&[])))
            .unwrap();
        assert_eq!(core(&status)[0], Some(2009));
        status
            .observe(&p(1070, H + 5, aura_payload(60, |_| 1)))
            .unwrap();
        status.observe(&p(1065, H + 6, Vec::new())).unwrap();
        assert_eq!(core(&status)[0], Some(2010));
        status
            .observe(&p(283, H + 7, indexed_payload(&[])))
            .unwrap();
        assert_eq!(core(&status), [None; 3]);
        status.observe(&p(1071, H + 8, Vec::new())).unwrap();
        assert_eq!(core(&status)[0], Some(2009));
        status
            .observe(&p(1070, H + 9, aura_payload(59, |_| 1)))
            .unwrap();
        status
            .observe(&p(39, H + 10, block_payload(33, 0, |_| 2)))
            .unwrap();
        assert_eq!(core(&status), [None; 3]);
    }
    #[test]
    fn unobservable_recomputes_lose_only_the_state_they_could_change() {
        for opcode in [82, 85, 837, 1386, 1387, 1389] {
            let mut status = started();
            status.observe(&p(opcode, H + 1, Vec::new())).unwrap();
            assert_eq!(core(&status)[0], Some(2009));
            status
                .observe(&p(784, H + 2, indexed_payload(&[(9, 77)])))
                .unwrap();
            status.observe(&p(opcode, H + 3, Vec::new())).unwrap();
            // An equip or set recompute would copy the override; an imprint one would
            // clear it first, leaving the copied vectors as they were either way.
            let imprint = opcode > 1000;
            assert_eq!(core(&status)[0], imprint.then_some(2009));
            status
                .observe(&p(283, H + 4, indexed_payload(&[])))
                .unwrap();
            assert_eq!(core(&status)[0], (!imprint).then_some(77));
            status
                .observe(&p(39, H + 5, block_payload(33, 0, |_| 2)))
                .unwrap();
            assert_eq!(core(&status)[0], Some(2009));
        }
    }
}
