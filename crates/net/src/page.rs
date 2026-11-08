use crate::{
    NetError, Result,
    codec::{MAX_PAYLOAD, OpcodeMap},
    profile::ORIGIN,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::time::Instant;
use zeroize::Zeroizing;

pub(crate) const INCOMING: &[u16] = &[
    5, 159, 679, 230, 233, 7, 8, 9, 6, 15, 16, 21, 24, 25, 26, 27, 729, 36, 37, 38, 1030, 286, 289,
    43, 42, 231, 561, 160, 770, 262, 246, 260, 843, 68, 82, 85, 41, 10, 11, 12, 13, 14, 617, 34,
    45, 46, 371, 50, 1951, 1952, 512, 518, 521, 31, 485, 1438, 32, 310, 525, 39, 109, 110, 271,
    282, 283, 523, 437,
];
pub(crate) const OUTGOING: &[u16] = &[24, 288, 29, 477, 44, 48, 49];

/// Host-only binding. Never print this value or the script/message that carries its guard.
pub struct PageBinding {
    pub(crate) epoch: u64,
    pub(crate) guard: Zeroizing<String>,
}
impl PageBinding {
    pub fn new(host_epoch: u64, guard: String) -> Result<Self> {
        let guard = Zeroizing::new(guard);
        if host_epoch == 0 || guard.len() != 32 || !guard.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(NetError::Malformed);
        }
        Ok(Self {
            epoch: host_epoch,
            guard,
        })
    }
    pub(crate) fn epoch_text(&self) -> String {
        format!("{:x}", self.epoch)
    }
}
/// Executable script containing a private host guard. Intentionally no Debug/Serialize.
pub struct ObserverScript(Zeroizing<String>);
impl ObserverScript {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageFault {
    BuildChanged,
    Capacity,
    Malformed,
    UnsupportedBinary,
    SendUncertain,
    UnknownFlow,
    CloseFailed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageNotice {
    Closed { request: u64, sockets: u32 },
    Sample { request: u64, available: bool },
    Fault(PageFault),
}
pub(crate) struct State {
    pub(crate) binding: PageBinding,
    pub(crate) sequence: u64,
    pub(crate) raw_ingress: u64,
    pub(crate) closing: Option<u64>,
}
impl State {
    pub(crate) fn new(binding: PageBinding) -> Self {
        Self {
            binding,
            sequence: 0,
            raw_ingress: 0,
            closing: None,
        }
    }
    pub(crate) fn close_script(&mut self, request: u64) -> Result<ObserverScript> {
        if request == 0 {
            return Err(NetError::Malformed);
        }
        if self.closing.is_some_and(|old| old != request) {
            return Err(NetError::Busy);
        }
        self.closing = Some(request);
        Ok(ObserverScript(Zeroizing::new(format!(
            "globalThis.__opentricksterNetCloseV1?.(\"{}\",\"{}\",\"{request:x}\");",
            self.binding.epoch_text(),
            self.binding.guard.as_str()
        ))))
    }
    pub(crate) fn sample_script(&self, request: u64, layout: u64) -> Result<ObserverScript> {
        if request == 0 || layout == 0 {
            return Err(NetError::Malformed);
        }
        if self.closing.is_some() {
            return Err(NetError::Closed);
        }
        Ok(ObserverScript(Zeroizing::new(format!(
            "globalThis.__opentricksterNetSampleV1?.(\"{}\",\"{}\",\"{request:x}\",\"{layout:x}\");",
            self.binding.epoch_text(),
            self.binding.guard.as_str()
        ))))
    }
    pub(crate) fn parse(&mut self, origin: &str, json: &str) -> Result<serde_json::Value> {
        if origin != ORIGIN || json.len() > 65536 {
            return Err(NetError::Malformed);
        }
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|_| NetError::Malformed)?;
        if value["channel"] != "opentrickster.net.v1"
            || value["epoch"] != self.binding.epoch_text()
            || value["guard"].as_str() != Some(self.binding.guard.as_str())
        {
            return Err(NetError::StaleConnection);
        }
        let sequence = value["sequence"]
            .as_u64()
            .filter(|n| *n > self.sequence && *n <= 9_007_199_254_740_991)
            .ok_or(NetError::StaleConnection)?;
        if sequence != self.sequence + 1 && value["kind"] != "closed" {
            return Err(NetError::Uncertain);
        }
        if value["kind"] != "closed" {
            let ingress = value["rawIngress"]
                .as_u64()
                .filter(|n| *n >= self.raw_ingress && *n <= 9_007_199_254_740_991)
                .ok_or(NetError::Uncertain)?;
            if value["ingress"].as_u64().is_some_and(|mark| mark > ingress) {
                return Err(NetError::Malformed);
            }
            self.raw_ingress = ingress;
        }
        self.sequence = sequence;
        Ok(value)
    }
}
pub(crate) fn script(binding: &PageBinding) -> Result<ObserverScript> {
    let c2s = OpcodeMap::current(b"c2s");
    let s2c = OpcodeMap::current(b"s2c");
    let settings = serde_json::json!({"origin":ORIGIN,"build":crate::BUILD,"epoch":binding.epoch_text(),"guard":binding.guard.as_str(),"incoming":INCOMING.iter().map(|op|s2c.wire_opcode(*op)).collect::<Vec<_>>(),"outgoing":OUTGOING.iter().map(|op|c2s.wire_opcode(*op)).collect::<Vec<_>>(),"login":c2s.wire_opcode(4),"profile":s2c.wire_opcode(679),"accepted":s2c.wire_opcode(5),"refused":s2c.wire_opcode(159),"markers":[s2c.wire_opcode(1951),s2c.wire_opcode(1952)]});
    let settings =
        Zeroizing::new(serde_json::to_string(&settings).map_err(|_| NetError::Malformed)?);
    Ok(ObserverScript(Zeroizing::new(
        include_str!("page_bridge.js")
            .replace("__OPENTRICKSTER_SETTINGS__", &settings)
            .replace(
                "__OPENTRICKSTER_SAMPLER__",
                crate::sampler::install_script(),
            ),
    )))
}
pub(crate) fn captured(value: &serde_json::Value, at: Instant) -> Result<Instant> {
    let age = value["ageMs"]
        .as_u64()
        .filter(|n| *n <= 60_000)
        .ok_or(NetError::Malformed)?;
    at.checked_sub(std::time::Duration::from_millis(age))
        .ok_or(NetError::Malformed)
}
pub(crate) fn connection(value: &serde_json::Value) -> Result<crate::ConnectionKey> {
    Ok(crate::ConnectionKey(
        value["connection"]
            .as_u64()
            .filter(|n| (1..=64).contains(n))
            .ok_or(NetError::Malformed)?,
    ))
}
pub(crate) fn ordinal(value: &serde_json::Value) -> Result<u64> {
    value["ingress"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
        .ok_or(NetError::Malformed)
}
pub(crate) fn frame(
    value: &serde_json::Value,
    captured: Instant,
) -> Result<(crate::ObservedDirection, crate::codec::Packet)> {
    let direction = match value["direction"].as_str() {
        Some("inbound") => crate::ObservedDirection::Inbound,
        Some("outbound") => crate::ObservedDirection::Outbound,
        _ => return Err(NetError::Malformed),
    };
    let encoded = value["data"]
        .as_str()
        .filter(|s| s.len() <= 43668)
        .ok_or(NetError::Capacity)?;
    let bytes = Zeroizing::new(STANDARD.decode(encoded).map_err(|_| NetError::Malformed)?);
    if bytes.len() < 6
        || bytes.len() > MAX_PAYLOAD + 6
        || u16::from_le_bytes(bytes[..2].try_into().unwrap()) as usize != bytes.len() - 6
    {
        return Err(NetError::Malformed);
    }
    static IN: std::sync::OnceLock<OpcodeMap> = std::sync::OnceLock::new();
    static OUT: std::sync::OnceLock<OpcodeMap> = std::sync::OnceLock::new();
    let map = if direction == crate::ObservedDirection::Inbound {
        IN.get_or_init(|| OpcodeMap::current(b"s2c"))
    } else {
        OUT.get_or_init(|| OpcodeMap::current(b"c2s"))
    };
    let wire = u16::from_le_bytes(bytes[2..4].try_into().unwrap());
    let opcode = map.logical_opcode(wire);
    let allowed = if direction == crate::ObservedDirection::Inbound {
        INCOMING
    } else {
        OUTGOING
    };
    if !allowed.contains(&opcode) {
        return Err(NetError::Malformed);
    }
    if direction == crate::ObservedDirection::Inbound
        && opcode == 679
        && bytes[6..]
            .iter()
            .enumerate()
            .any(|(i, b)| *b != 0 && !((17..=38).contains(&i) || [53, 54, 56, 57].contains(&i)))
    {
        return Err(NetError::Malformed);
    }
    if direction == crate::ObservedDirection::Inbound
        && matches!(opcode, 5 | 159 | 1951 | 1952)
        && bytes.len() != 6
    {
        return Err(NetError::Malformed);
    }
    let mark = ordinal(value)?;
    Ok((
        direction,
        crate::codec::Packet {
            opcode,
            aux: u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
            payload: Zeroizing::new(bytes[6..].to_vec()),
            mark,
            received: captured,
        },
    ))
}
pub(crate) fn fault(value: &serde_json::Value) -> PageFault {
    match value["code"].as_str() {
        Some("build-changed") => PageFault::BuildChanged,
        Some("capacity") => PageFault::Capacity,
        Some("binary-type") => PageFault::UnsupportedBinary,
        Some("send-uncertain") => PageFault::SendUncertain,
        Some("unknown-flow") => PageFault::UnknownFlow,
        Some("socket-close") => PageFault::CloseFailed,
        _ => PageFault::Malformed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifier_never_forwards_auth_and_cleanup_requires_real_close_events() {
        use std::{
            io::Write,
            process::{Command, Stdio},
        };
        let binding = PageBinding::new(7, "00112233445566778899aabbccddeeff".into()).unwrap();
        let script = script(&binding).unwrap();
        let c2s = OpcodeMap::current(b"c2s");
        let s2c = OpcodeMap::current(b"s2c");
        let input = serde_json::json!({"script":script.as_str(),"auth":c2s.wire_opcode(11501),"ls":c2s.wire_opcode(2000),"ticket":s2c.wire_opcode(2014),"move":c2s.wire_opcode(24),"hp":s2c.wire_opcode(36)});
        let mut child = Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/page_bridge.cjs"
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Node is required for page bridge behavior checks");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("PASS page privacy"));
    }
}
