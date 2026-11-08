use crate::{NetError, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    time::Instant,
};
use zeroize::Zeroizing;

pub(crate) const MAX_PAYLOAD: usize = 32745;
pub(crate) const MAX_BUFFER: usize = 1_048_576;
pub(crate) const MAX_PACKETS: usize = 4096;
const SEED: &str = "Yi9ApMr88JAXsVztVJYSxdzPbba7CrrOlJ0GmnNHLXI=";

pub(crate) struct OpcodeMap {
    wire: Vec<u16>,
    inverse: Vec<u16>,
}
impl OpcodeMap {
    pub(crate) fn current(direction: &[u8]) -> Self {
        Self::from_seed(&STANDARD.decode(SEED).unwrap(), direction).unwrap()
    }
    fn from_seed(seed: &[u8], direction: &[u8]) -> Result<Self> {
        if seed.len() != 32 || ![b"c2s".as_slice(), b"s2c"].contains(&direction) {
            return Err(NetError::Malformed);
        }
        let mut hash = Sha256::new();
        hash.update(seed);
        hash.update(direction);
        let h = hash.finalize();
        let mut state: Vec<u32> = h[..16]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect();
        let mut wire: Vec<u16> = (0..=u16::MAX).collect();
        for i in (1..65536).rev() {
            let [a, b, c, d]: [u32; 4] = state.as_slice().try_into().unwrap();
            let r = b.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
            wire.swap(i, (r as usize) % (i + 1));
            state.copy_from_slice(&[
                a ^ b ^ d,
                b ^ a ^ c,
                c ^ a ^ (b << 9),
                (b ^ d).rotate_left(11),
            ]);
        }
        let mut inverse = vec![0; 65536];
        for (logical, &mapped) in wire.iter().enumerate() {
            inverse[mapped as usize] = logical as u16;
        }
        Ok(Self { wire, inverse })
    }
    pub(crate) fn wire_opcode(&self, opcode: u16) -> u16 {
        self.wire[opcode as usize]
    }
    pub(crate) fn logical_opcode(&self, wire: u16) -> u16 {
        self.inverse[wire as usize]
    }
    pub(crate) fn encode(
        &self,
        opcode: u16,
        aux: u16,
        payload: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>> {
        if payload.len() > MAX_PAYLOAD {
            return Err(NetError::Capacity);
        }
        let mut b = Zeroizing::new(Vec::with_capacity(payload.len() + 6));
        b.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        b.extend_from_slice(&self.wire[opcode as usize].to_le_bytes());
        b.extend_from_slice(&aux.to_le_bytes());
        b.extend_from_slice(payload);
        Ok(b)
    }
}

pub(crate) struct Packet {
    pub(crate) opcode: u16,
    pub(crate) aux: u16,
    pub(crate) payload: Zeroizing<Vec<u8>>,
    pub(crate) mark: u64,
    pub(crate) received: Instant,
}
pub(crate) struct Frames {
    map: OpcodeMap,
    pending: Zeroizing<Vec<u8>>,
    poisoned: bool,
    started: Option<(u64, Instant)>,
}
impl Frames {
    pub(crate) fn new(direction: &[u8]) -> Self {
        Self {
            map: OpcodeMap::current(direction),
            pending: Zeroizing::new(Vec::new()),
            poisoned: false,
            started: None,
        }
    }
    pub(crate) fn push(&mut self, bytes: &[u8], order: &mut u64) -> Result<Vec<Packet>> {
        self.push_at(bytes, order, Instant::now())
    }
    pub(crate) fn push_at(
        &mut self,
        bytes: &[u8],
        order: &mut u64,
        received: Instant,
    ) -> Result<Vec<Packet>> {
        if self.poisoned {
            return Err(NetError::Closed);
        }
        let result = self.parse(bytes, order, received);
        if result.is_err() {
            self.poisoned = true;
            self.pending.clear();
        }
        result
    }
    fn parse(&mut self, bytes: &[u8], order: &mut u64, now: Instant) -> Result<Vec<Packet>> {
        if bytes.len() > MAX_BUFFER || self.pending.len() + bytes.len() > MAX_BUFFER {
            return Err(NetError::Capacity);
        }
        self.pending.extend_from_slice(bytes);
        let mut pos = 0;
        let mut out = Vec::new();
        while self.pending.len() > pos {
            if self.started.is_none() {
                *order = order
                    .checked_add(1)
                    .filter(|n| *n <= u64::MAX / 4)
                    .ok_or(NetError::Capacity)?;
                self.started = Some((*order, now));
            }
            if self.pending.len() - pos < 6 {
                break;
            }
            let h = &self.pending[pos..pos + 6];
            let n = u16::from_le_bytes(h[..2].try_into().unwrap()) as usize;
            if n > MAX_PAYLOAD {
                return Err(NetError::Capacity);
            }
            if self.pending.len() - pos < 6 + n {
                break;
            }
            if out.len() >= MAX_PACKETS {
                return Err(NetError::Capacity);
            }
            let (mark, received) = self.started.take().ok_or(NetError::Malformed)?;
            out.push(Packet {
                opcode: self.map.inverse[u16::from_le_bytes(h[2..4].try_into().unwrap()) as usize],
                aux: u16::from_le_bytes(h[4..6].try_into().unwrap()),
                payload: Zeroizing::new(self.pending[pos + 6..pos + 6 + n].to_vec()),
                mark,
                received,
            });
            pos += 6 + n;
        }
        self.pending.drain(..pos);
        Ok(out)
    }
    pub(crate) fn pending_mark(&self) -> Option<u64> {
        self.started.map(|(mark, _)| mark)
    }
    pub(crate) fn finish(&self) -> Result<()> {
        if self.pending.is_empty() {
            Ok(())
        } else {
            Err(NetError::Truncated)
        }
    }
}

#[derive(Default)]
pub(crate) struct AuxGroups {
    groups: BTreeMap<u16, Vec<Packet>>,
    bytes: usize,
    packets: usize,
}
impl AuxGroups {
    pub(crate) fn oldest_mark(&self) -> Option<u64> {
        self.groups
            .values()
            .flat_map(|packets| packets.iter().map(|p| p.mark))
            .min()
    }

    pub(crate) fn push(&mut self, p: Packet) -> Result<Vec<Packet>> {
        if p.aux == 0 {
            return Ok(vec![p]);
        }
        if matches!(p.opcode, 1951 | 1952) {
            let mut ready = self.groups.remove(&p.aux).unwrap_or_default();
            self.bytes -= ready.iter().map(|p| p.payload.len() + 6).sum::<usize>();
            self.packets -= ready.len();
            if p.opcode == 1952 {
                self.groups.insert(p.aux, Vec::new());
            } else {
                ready.push(p);
            }
            if self.groups.len() > 64 {
                return Err(NetError::Capacity);
            }
            return Ok(ready);
        }
        if self.bytes + p.payload.len() + 6 > MAX_BUFFER
            || self.packets >= MAX_PACKETS
            || (!self.groups.contains_key(&p.aux) && self.groups.len() >= 64)
        {
            return Err(NetError::Capacity);
        }
        self.bytes += p.payload.len() + 6;
        self.packets += 1;
        self.groups.entry(p.aux).or_default().push(p);
        Ok(Vec::new())
    }
}

#[derive(Default)]
pub(crate) struct Retained {
    sent: u64,
    acked: u64,
    bytes: usize,
    queue: VecDeque<Zeroizing<Vec<u8>>>,
    head: usize,
}
impl Retained {
    pub(crate) fn sent(&mut self, bytes: &[u8]) -> Result<()> {
        if self.bytes + bytes.len() > MAX_BUFFER {
            return Err(NetError::Capacity);
        }
        self.sent = self
            .sent
            .checked_add(bytes.len() as u64)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or(NetError::Capacity)?;
        self.bytes += bytes.len();
        self.queue.push_back(Zeroizing::new(bytes.to_vec()));
        Ok(())
    }
    pub(crate) fn ack(&mut self, n: u64, resuming: bool) -> Result<()> {
        if n > self.sent || (resuming && n < self.acked) {
            return Err(NetError::Malformed);
        }
        if n <= self.acked {
            return Ok(());
        }
        let mut remove = (n - self.acked) as usize;
        self.bytes -= remove;
        self.acked = n;
        while remove > 0 {
            let front = self.queue.front().ok_or(NetError::Malformed)?;
            let left = front.len() - self.head;
            if remove < left {
                self.head += remove;
                break;
            }
            remove -= left;
            self.queue.pop_front();
            self.head = 0;
        }
        Ok(())
    }
    pub(crate) fn abandon(&mut self) {
        self.queue.clear();
        self.head = 0;
        self.bytes = 0;
        self.acked = self.sent;
    }
    #[cfg(test)]
    fn suffix(&self) -> Vec<u8> {
        self.queue
            .iter()
            .enumerate()
            .flat_map(|(i, b)| b[if i == 0 { self.head } else { 0 }..].iter().copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn digest(b: &[u16]) -> String {
        format!(
            "{:x}",
            Sha256::digest(b.iter().flat_map(|n| n.to_le_bytes()).collect::<Vec<_>>())
        )
    }
    #[test]
    fn current_maps_and_synthetic_seed() {
        for (dir, wire, inv) in [
            (
                b"c2s",
                "db98aa5f8e2df90f4d4baa756a9fad7849f6d307e2c155f5b6b5c9ba58c11fe1",
                "9e374006761f9bf18e3a59b6b4914c86b673f22230d9bab238e4c56120e350e4",
            ),
            (
                b"s2c",
                "e0386023dd44e6cd0e70099542b5f9943969feff662b5797cb73e5edab72f58d",
                "c5d1b355f0c5cf75af90935158df8d6b7309e860710b760fb1f04207169c6b9c",
            ),
        ] {
            let m = OpcodeMap::current(dir);
            assert_eq!(digest(&m.wire), wire);
            assert_eq!(digest(&m.inverse), inv);
            for i in 0..65536 {
                assert_eq!(m.inverse[m.wire[i] as usize] as usize, i);
            }
        }
        let m = OpcodeMap::from_seed(&(0..32).collect::<Vec<u8>>(), b"c2s").unwrap();
        assert_eq!(
            digest(&m.wire),
            "c87a5dc1ad4dc4657d8ad4ababf6bb39e83daddb12cf61c76e50e89f65e6edab"
        );
        assert_eq!(
            &*m.encode(0x1234, 0, &[0xaa, 0xbb]).unwrap(),
            &[2, 0, 0x45, 0x39, 0, 0, 0xaa, 0xbb]
        );
        assert!(OpcodeMap::from_seed(&[0; 31], b"c2s").is_err());
    }
    #[test]
    fn every_stream_split_and_per_byte() {
        let m = OpcodeMap::current(b"s2c");
        let mut b = m.encode(2014, 0, b"ticket").unwrap();
        b.extend_from_slice(&m.encode(3, 0, b"").unwrap());
        for split in 0..=b.len() {
            let mut f = Frames::new(b"s2c");
            let mut mark = 0;
            let mut out = f.push(&b[..split], &mut mark).unwrap();
            out.extend(f.push(&b[split..], &mut mark).unwrap());
            assert_eq!(out.iter().map(|p| p.opcode).collect::<Vec<_>>(), [2014, 3]);
            f.finish().unwrap();
        }
        let mut f = Frames::new(b"s2c");
        let mut mark = 0;
        let mut out = Vec::new();
        for byte in b.iter() {
            out.extend(f.push(&[*byte], &mut mark).unwrap());
        }
        assert_eq!(out.len(), 2);
    }
    #[test]
    fn limits_and_truncated_stream() {
        let m = OpcodeMap::current(b"s2c");
        let max = m.encode(1, 0, &vec![0; MAX_PAYLOAD]).unwrap();
        assert!(m.encode(1, 0, &vec![0; MAX_PAYLOAD + 1]).is_err());
        assert_eq!(Frames::new(b"s2c").push(&max, &mut 0).unwrap().len(), 1);
        let mut f = Frames::new(b"s2c");
        f.push(&max[..max.len() - 1], &mut 0).unwrap();
        assert_eq!(f.finish(), Err(NetError::Truncated));
        assert!(f.push(&[0; MAX_BUFFER], &mut 0).is_err());
        assert!(f.push(&[], &mut 0).is_err());
        let mut f = Frames::new(b"s2c");
        assert!(f.push(&[0xea, 0x7f, 0, 0, 0, 0], &mut 0).is_err());
    }
    #[test]
    fn independent_aux_and_original_marks() {
        let mut a = AuxGroups::default();
        let p = |opcode, aux, mark| Packet {
            opcode,
            aux,
            mark,
            received: Instant::now(),
            payload: Zeroizing::new(vec![]),
        };
        a.push(p(10, 1, 1)).unwrap();
        a.push(p(11, 2, 2)).unwrap();
        assert_eq!(a.push(p(12, 0, 3)).unwrap()[0].mark, 3);
        assert_eq!(a.push(p(1952, 2, 4)).unwrap()[0].mark, 2);
        a.push(p(13, 2, 5)).unwrap();
        let first = a.push(p(1951, 1, 6)).unwrap();
        assert_eq!(first.iter().map(|p| p.mark).collect::<Vec<_>>(), [1, 6]);
        assert_eq!(a.push(p(1951, 2, 7)).unwrap().len(), 2);
    }
    #[test]
    fn byte_ack_partial_stale_and_resume_boundaries() {
        let mut r = Retained::default();
        r.sent(b"abcdef").unwrap();
        r.sent(b"ghij").unwrap();
        r.ack(3, false).unwrap();
        assert_eq!(r.suffix(), b"defghij");
        r.ack(2, false).unwrap();
        assert_eq!(r.suffix(), b"defghij");
        assert_eq!(r.ack(2, true), Err(NetError::Malformed));
        assert_eq!(r.ack(11, false), Err(NetError::Malformed));
        r.ack(7, true).unwrap();
        assert_eq!(r.suffix(), b"hij");
        r.ack(10, false).unwrap();
        assert!(r.suffix().is_empty());
    }
    #[test]
    fn partial_header_keeps_original_ingress_before_a_later_send() {
        let map = OpcodeMap::current(b"s2c");
        let bytes = map.encode(36, 7, &[1, 0, 0, 0]).unwrap();
        let mut frames = Frames::new(b"s2c");
        let mut order = 0;
        let before = Instant::now();
        assert!(frames.push(&bytes[..1], &mut order).unwrap().is_empty());
        assert_eq!(order, 1);
        order += 1;
        let send_mark = order;
        let packet = frames.push(&bytes[1..], &mut order).unwrap().pop().unwrap();
        assert!(packet.mark < send_mark);
        assert!(packet.received >= before);
        assert_eq!(order, 2);
        let original = packet.received;
        let mut groups = AuxGroups::default();
        groups.push(packet).unwrap();
        let marker = Packet {
            opcode: 1951,
            aux: 7,
            payload: Zeroizing::new(vec![]),
            mark: 3,
            received: Instant::now(),
        };
        let released = groups.push(marker).unwrap();
        assert_eq!(released[0].received, original);
        assert_eq!(released[0].mark, 1);
    }
}
