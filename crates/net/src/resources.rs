use crate::{Result, codec::Packet, wire::Reader};
use opentrickster_engine::Gauge;
use std::num::NonZeroU32;

#[derive(Default)]
pub(crate) struct Totals([Option<i32>; 12]);
impl Totals {
    pub(crate) fn observe(&mut self, p: &Packet) -> Result<()> {
        match p.opcode {
            282 | 283 => {
                let mut r = Reader::new(&p.payload);
                let count = r.u8()?;
                let mut values = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    values.push((r.u8()?, r.u32()? as i32));
                }
                r.end()?;
                if p.opcode == 283 {
                    self.0 = [None; 12];
                } else {
                    for (stat, value) in values {
                        if let Some(total) = self.0.get_mut(stat as usize) {
                            *total = Some(value);
                        }
                    }
                }
            }
            233 | 39 | 82 | 85 | 109 | 110 | 437 | 518 | 521 | 561 | 160 | 770 | 262 | 246
            | 260 | 843 | 68 => self.0 = [None; 12],
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn maximum(&self, stat: usize) -> Option<NonZeroU32> {
        self.0
            .get(stat)
            .copied()
            .flatten()
            .and_then(|v| u32::try_from(v).ok())
            .and_then(NonZeroU32::new)
    }
    pub(crate) fn gauge(&self, current: u32, stat: usize) -> Gauge {
        let maximum = self.maximum(stat);
        Gauge {
            current: maximum.map_or(current, |max| current.min(max.get())),
            maximum,
        }
    }
    pub(crate) fn composite(&self, current: u32, stat: usize) -> Gauge {
        Gauge {
            current,
            maximum: self
                .maximum(stat)
                .filter(|maximum| current <= maximum.get()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(opcode: u16, payload: Vec<u8>) -> Packet {
        Packet {
            opcode,
            aux: 0,
            mark: 1,
            received: std::time::Instant::now(),
            payload: zeroize::Zeroizing::new(payload),
        }
    }
    #[test]
    fn totals_are_nullable_partial_and_invalidated_by_recompute() {
        let mut t = Totals::default();
        assert!(t.gauge(80, 9).maximum.is_none());
        let mut bytes = vec![2, 9];
        bytes.extend_from_slice(&100i32.to_le_bytes());
        bytes.push(3);
        bytes.extend_from_slice(&50i32.to_le_bytes());
        t.observe(&p(282, bytes.clone())).unwrap();
        assert_eq!(t.gauge(120, 9).current, 100);
        assert_eq!(t.maximum(3).unwrap().get(), 50);
        assert!(t.maximum(6).is_none());
        bytes.pop();
        assert!(t.observe(&p(282, bytes)).is_err());
        assert_eq!(t.maximum(9).unwrap().get(), 100);
        t.observe(&p(283, vec![0])).unwrap();
        assert!(t.maximum(9).is_none());
        let mut bytes = vec![1, 9];
        bytes.extend_from_slice(&(-1i32).to_le_bytes());
        t.observe(&p(282, bytes)).unwrap();
        assert!(t.maximum(9).is_none());
    }
}
