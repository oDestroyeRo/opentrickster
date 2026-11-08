use crate::{NetError, Result, auth};
use zeroize::Zeroizing;

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pub(crate) pos: usize,
}
impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(NetError::Capacity)?;
        let slice = self.bytes.get(self.pos..end).ok_or(NetError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }
    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub(crate) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub(crate) fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub(crate) fn fixed(&mut self, n: usize) -> Result<String> {
        let bytes = self.take(n)?;
        Ok(auth::decode(
            &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(n)],
        ))
    }
    pub(crate) fn cstr(&mut self) -> Result<String> {
        let n = self.bytes[self.pos..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(NetError::Truncated)?;
        let text = auth::decode(self.take(n)?);
        self.take(1)?;
        Ok(text)
    }
    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }
    pub(crate) fn end(&self) -> Result<()> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(NetError::Malformed)
        }
    }
}

pub(crate) struct Writer(pub(crate) Zeroizing<Vec<u8>>);
impl Writer {
    pub(crate) fn new() -> Self {
        Self(Zeroizing::new(Vec::new()))
    }
    pub(crate) fn raw(&mut self, b: &[u8]) {
        self.0.extend_from_slice(b);
    }
    pub(crate) fn u8(&mut self, n: u8) {
        self.0.push(n);
    }
    pub(crate) fn u16(&mut self, n: u16) {
        self.raw(&n.to_le_bytes());
    }
    pub(crate) fn u32(&mut self, n: u32) {
        self.raw(&n.to_le_bytes());
    }
    pub(crate) fn u64(&mut self, n: u64) {
        self.raw(&n.to_le_bytes());
    }
    pub(crate) fn fixed(&mut self, s: &str, n: usize) -> Result<()> {
        let b = Zeroizing::new(auth::encode(s));
        if b.len() >= n || b.contains(&0) {
            return Err(NetError::Malformed);
        }
        self.raw(&b);
        let total = self.0.len() + n - b.len();
        self.0.resize(total, 0);
        Ok(())
    }
    pub(crate) fn cstr(&mut self, s: &str) -> Result<()> {
        let b = Zeroizing::new(auth::encode(s));
        if b.contains(&0) {
            return Err(NetError::Malformed);
        }
        self.raw(&b);
        self.u8(0);
        Ok(())
    }
}
