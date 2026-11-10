use crate::{NetError, Result, codec::Packet, wire::Reader};
use opentrickster_engine as e;
use std::{collections::BTreeMap, time::Instant};

pub(crate) struct Correlated {
    pub(crate) mark: u64,
    pub(crate) received: Instant,
    pub(crate) outcome: e::Outcome,
}
struct Strike {
    attacker: u16,
    target: u16,
    mark: u64,
    received: Instant,
}
#[derive(Default)]
struct Bundle {
    strike: Option<Strike>,
    ambiguous: bool,
    hits: Vec<(u16, u64, Instant, bool)>,
}
pub(crate) struct Pending {
    pub(crate) target: u16,
    pub(crate) key: e::EntityKey,
    pub(crate) floor: u64,
}
#[derive(Default)]
pub(crate) struct Correlator {
    groups: BTreeMap<u16, Bundle>,
    pending: Option<Pending>,
}
impl Correlator {
    pub(crate) fn expect(&mut self, pending: Pending) {
        self.pending = Some(pending);
    }
    pub(crate) fn clear(&mut self) {
        self.pending = None;
        self.groups.clear();
    }
    /// An own strike at the pending target, after its floor, is the server resolving the
    /// attack, with or without damage (docs/research/gun-combat-control.md).
    fn answers(&self, strike: &Strike, own: Option<u16>) -> Option<&Pending> {
        self.pending.as_ref().filter(|pending| {
            Some(strike.attacker) == own
                && strike.target == pending.target
                && strike.mark > pending.floor
        })
    }
    pub(crate) fn observe(&mut self, p: &Packet, own: Option<u16>) -> Result<Vec<Correlated>> {
        if matches!(p.opcode, 31 | 485 | 1438) {
            let strike = swing(p)?;
            if p.aux == 0 {
                // CombatLink.Receive emits an ungrouped strike at once, without hits.
                let answered = strike
                    .as_ref()
                    .and_then(|s| Some((s, self.answers(s, own)?)));
                return Ok(answered
                    .map(|(s, pending)| resolved(s, pending))
                    .into_iter()
                    .collect());
            }
            if !self.groups.contains_key(&p.aux) && self.groups.len() >= 64 {
                return Err(NetError::Capacity);
            }
            let group = self.groups.entry(p.aux).or_default();
            match strike {
                Some(strike) if group.strike.is_none() => group.strike = Some(strike),
                _ => group.ambiguous = true,
            }
            return Ok(Vec::new());
        }
        if p.aux == 0 {
            return Ok(Vec::new());
        }
        if p.opcode == 1952 {
            self.groups.remove(&p.aux);
            return Ok(Vec::new());
        }
        if p.opcode == 1951 {
            let Some(group) = self.groups.remove(&p.aux) else {
                return Ok(Vec::new());
            };
            let Some(strike) = group.strike.filter(|_| !group.ambiguous) else {
                return Ok(Vec::new());
            };
            let Some(pending) = self.answers(&strike, own) else {
                return Ok(Vec::new());
            };
            let mut out = Vec::new();
            let mut hit = false;
            let mut kill = false;
            for (target, mark, received, faint) in group.hits {
                if target != pending.target || mark <= pending.floor {
                    continue;
                }
                if faint && !kill {
                    kill = true;
                    out.push(Correlated {
                        mark,
                        received,
                        outcome: e::Outcome::OwnKill(pending.key),
                    });
                } else if !faint && !hit {
                    hit = true;
                    out.push(Correlated {
                        mark,
                        received,
                        outcome: e::Outcome::OwnHit(pending.key),
                    });
                }
            }
            // A miss deals no damage, but its strike still resolves the attack.
            if out.is_empty() {
                out.push(resolved(&strike, pending));
            }
            return Ok(out);
        }
        if !matches!(p.opcode, 42 | 43) {
            return Ok(Vec::new());
        }
        if !self.groups.contains_key(&p.aux) && self.groups.len() >= 64 {
            return Err(NetError::Capacity);
        }
        let group = self.groups.entry(p.aux).or_default();
        let mut r = Reader::new(&p.payload);
        let target = r.u16()?;
        let relevant = if p.opcode == 42 {
            (r.u32()? as i32) > 0
        } else {
            true
        };
        if relevant {
            if group.hits.len() >= 256 {
                return Err(NetError::Capacity);
            }
            group
                .hits
                .push((target, p.mark, p.received, p.opcode == 43));
        }
        Ok(Vec::new())
    }
}

/// Reads a strike as `Combat.ParseSwing` does. Kind 3 is the multitarget grammar, never a
/// normal attack, so it yields no strike.
fn swing(p: &Packet) -> Result<Option<Strike>> {
    let mut r = Reader::new(&p.payload);
    let attacker = r.u16()?;
    if r.u16()? == 3 {
        return Ok(None);
    }
    let target = r.u16()?;
    r.take(8)?;
    if r.remaining() >= 2 {
        r.u16()?;
    }
    if r.remaining() > 0 {
        r.u8()?;
    }
    Ok(Some(Strike {
        attacker,
        target,
        mark: p.mark,
        received: p.received,
    }))
}

/// `OwnHit` confirms the attack; only a same-bundle faint counts as a kill.
fn resolved(strike: &Strike, pending: &Pending) -> Correlated {
    Correlated {
        mark: strike.mark,
        received: strike.received,
        outcome: e::Outcome::OwnHit(pending.key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(opcode: u16, aux: u16, mark: u64, payload: Vec<u8>) -> Packet {
        Packet {
            opcode,
            aux,
            mark,
            payload: zeroize::Zeroizing::new(payload),
            received: Instant::now(),
        }
    }
    fn swing(actor: u16, target: u16) -> Vec<u8> {
        [actor, 2, target, 1, 2, 3, 4]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect()
    }
    #[test]
    fn only_complete_unambiguous_own_bundles_attribute_hits_and_faints() {
        let mut c = Correlator::default();
        c.expect(Pending {
            target: 99,
            key: e::EntityKey(500),
            floor: 10,
        });
        for (aux, attacker, mark) in [(1, 7, 11), (2, 8, 15), (3, 7, 9)] {
            c.observe(&packet(31, aux, mark, swing(attacker, 99)), Some(7))
                .unwrap();
            let mut damage = 99u16.to_le_bytes().to_vec();
            damage.extend_from_slice(&3u32.to_le_bytes());
            damage.extend_from_slice(&[0; 20]);
            assert!(
                c.observe(&packet(42, aux, 20, damage), Some(7))
                    .unwrap()
                    .is_empty()
            );
            c.observe(&packet(43, aux, 21, 99u16.to_le_bytes().to_vec()), Some(7))
                .unwrap();
            let outcomes = c.observe(&packet(1951, aux, 22, vec![]), Some(7)).unwrap();
            if aux == 1 {
                assert_eq!(outcomes.len(), 2);
                assert_eq!(outcomes[0].mark, 20);
                assert!(matches!(
                    outcomes[1].outcome,
                    e::Outcome::OwnKill(e::EntityKey(500))
                ));
            } else {
                assert!(outcomes.is_empty());
            }
        }
        c.observe(&packet(31, 4, 30, swing(7, 99)), Some(7))
            .unwrap();
        c.observe(&packet(31, 4, 31, swing(8, 99)), Some(7))
            .unwrap();
        c.observe(&packet(43, 4, 32, 99u16.to_le_bytes().to_vec()), Some(7))
            .unwrap();
        assert!(
            c.observe(&packet(1951, 4, 33, vec![]), Some(7))
                .unwrap()
                .is_empty()
        );
        assert!(
            c.observe(&packet(43, 0, 34, 99u16.to_le_bytes().to_vec()), Some(7))
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn an_own_strike_resolves_the_attack_without_damage() {
        let expect = |c: &mut Correlator| {
            c.expect(Pending {
                target: 99,
                key: e::EntityKey(500),
                floor: 10,
            })
        };
        let hit = |outcomes: &[Correlated], mark| matches!(outcomes, [Correlated { mark: m, outcome: e::Outcome::OwnHit(e::EntityKey(500)), .. }] if *m == mark);
        let mut miss = 99u16.to_le_bytes().to_vec();
        miss.extend_from_slice(&0u32.to_le_bytes());
        miss.extend_from_slice(&[0; 20]);
        // A grouped miss: zero damage, no faint.
        let mut c = Correlator::default();
        expect(&mut c);
        c.observe(&packet(31, 1, 11, swing(7, 99)), Some(7))
            .unwrap();
        c.observe(&packet(42, 1, 12, miss.clone()), Some(7))
            .unwrap();
        assert!(hit(
            &c.observe(&packet(1951, 1, 13, vec![]), Some(7)).unwrap(),
            11
        ));
        // A grouped strike with no hits at all.
        c.observe(&packet(485, 2, 14, swing(7, 99)), Some(7))
            .unwrap();
        assert!(hit(
            &c.observe(&packet(1951, 2, 15, vec![]), Some(7)).unwrap(),
            14
        ));
        // An ungrouped strike resolves at once.
        assert!(hit(
            &c.observe(&packet(1438, 0, 16, swing(7, 99)), Some(7))
                .unwrap(),
            16
        ));
        // Another attacker, another target, an old strike, the multitarget kind, a bundle
        // with two strikes, and a cancelled bundle stay unattributed.
        let mut multi = swing(7, 99);
        multi[2] = 3;
        for (aux, strikes) in [
            (0, vec![(17, swing(8, 99))]),
            (0, vec![(18, swing(7, 98))]),
            (0, vec![(9, swing(7, 99))]),
            (0, vec![(19, multi.clone())]),
            (3, vec![(20, multi)]),
            (4, vec![(21, swing(7, 99)), (22, swing(8, 99))]),
        ] {
            let mut out = Vec::new();
            for (mark, strike) in strikes {
                out.extend(c.observe(&packet(31, aux, mark, strike), Some(7)).unwrap());
            }
            out.extend(c.observe(&packet(1951, aux, 23, vec![]), Some(7)).unwrap());
            assert!(out.is_empty(), "aux {aux}");
        }
        c.observe(&packet(31, 5, 24, swing(7, 99)), Some(7))
            .unwrap();
        c.observe(&packet(1952, 5, 25, vec![]), Some(7)).unwrap();
        assert!(
            c.observe(&packet(1951, 5, 26, vec![]), Some(7))
                .unwrap()
                .is_empty()
        );
        // Without a pending attack nothing resolves.
        c.clear();
        assert!(
            c.observe(&packet(31, 0, 27, swing(7, 99)), Some(7))
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn a_grouped_strike_needs_14_bytes_and_attributes_with_any_tail() {
        for len in 0..14 {
            let mut strike = swing(7, 99);
            strike.truncate(len);
            let result = Correlator::default().observe(&packet(31, 1, 11, strike), Some(7));
            assert!(result.is_err(), "{len} bytes");
        }
        for len in [14, 15, 16, 17, 18, 19, crate::codec::MAX_PAYLOAD] {
            let mut c = Correlator::default();
            c.expect(Pending {
                target: 99,
                key: e::EntityKey(500),
                floor: 10,
            });
            let mut strike = swing(7, 99);
            strike.resize(len, 0);
            c.observe(&packet(31, 1, 11, strike), Some(7)).unwrap();
            let mut damage = 99u16.to_le_bytes().to_vec();
            damage.extend_from_slice(&3u32.to_le_bytes());
            damage.extend_from_slice(&[0; 20]);
            c.observe(&packet(42, 1, 12, damage), Some(7)).unwrap();
            let outcomes = c.observe(&packet(1951, 1, 13, vec![]), Some(7)).unwrap();
            assert!(
                matches!(
                    outcomes.as_slice(),
                    [Correlated {
                        mark: 12,
                        outcome: e::Outcome::OwnHit(e::EntityKey(500)),
                        ..
                    }]
                ),
                "{len} bytes"
            );
        }
    }
}
