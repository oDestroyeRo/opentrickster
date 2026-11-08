use crate::{
    NetError, Result,
    codec::{MAX_BUFFER, Packet},
};
use std::collections::BTreeMap;

/// Physical raw/Aux minima authorize release; a latest seen mark never does.
#[derive(Default)]
pub(crate) struct OrderedIngress {
    pending: BTreeMap<u64, Packet>,
    bytes: usize,
    frontier: u64,
}
impl OrderedIngress {
    pub(crate) fn push(&mut self, packet: Packet) -> Result<()> {
        if packet.mark <= self.frontier || self.pending.contains_key(&packet.mark) {
            return Err(NetError::Uncertain);
        }
        if self.pending.len() >= 256 || self.bytes + packet.payload.len() + 6 > MAX_BUFFER {
            return Err(NetError::Capacity);
        }
        self.bytes += packet.payload.len() + 6;
        self.pending.insert(packet.mark, packet);
        Ok(())
    }
    pub(crate) fn release(
        &mut self,
        complete_through: u64,
        held: Option<u64>,
    ) -> Result<(Vec<Packet>, Option<u64>)> {
        let through = held.map_or(complete_through, |old| {
            complete_through.min(old.saturating_sub(1))
        });
        if through < self.frontier {
            return Err(NetError::Uncertain);
        }
        let mut packets = Vec::new();
        while self
            .pending
            .first_key_value()
            .is_some_and(|(&mark, _)| mark <= through)
        {
            let (_, packet) = self.pending.pop_first().ok_or(NetError::Malformed)?;
            self.bytes -= packet.payload.len() + 6;
            packets.push(packet);
        }
        let changed = (through > self.frontier).then_some(through);
        self.frontier = through;
        Ok((packets, changed))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn packet(mark: u64) -> Packet {
        Packet {
            mark,
            opcode: 36,
            aux: 0,
            payload: zeroize::Zeroizing::new(vec![0; 4]),
            received: std::time::Instant::now(),
        }
    }
    #[test]
    fn no_frontier_passes_older_retained_groups_or_partial_packets() {
        let mut queue = OrderedIngress::default();
        queue.push(packet(20)).unwrap();
        let (packets, frontier) = queue.release(20, Some(5)).unwrap();
        assert!(packets.is_empty());
        assert_eq!(frontier, Some(4));
        queue.push(packet(5)).unwrap();
        let (packets, frontier) = queue.release(20, None).unwrap();
        assert_eq!(packets.iter().map(|p| p.mark).collect::<Vec<_>>(), [5, 20]);
        assert_eq!(frontier, Some(20));
        assert_eq!(queue.push(packet(5)).err(), Some(NetError::Uncertain));
    }
}
