use crate::{InputFences, InputKey, NetError, NormalInput, Result};
use opentrickster_engine::{Action, ActionTicket, At, SessionKey};
use serde::Deserialize;
use serde_json::Value;
use std::time::{Duration, Instant};

const MAX_AGE: Duration = Duration::from_millis(250);
const MAX_ACTORS: usize = 513;

pub(crate) fn install_script() -> &'static str {
    include_str!("sampler.js")
}

// These types stay inside Net. They never refresh authoritative engine facts.
pub(crate) struct Snapshot {
    pub(crate) captured: Instant,
    pub(crate) host_epoch: u64,
    pub(crate) layout_epoch: u64,
    pub(crate) raw_ingress: u64,
    pub(crate) revision: u64,
    pub(crate) zone: u16,
    pub(crate) geometry: Geometry,
    pub(crate) pointer: Pointer,
    pub(crate) input: InputState,
    pub(crate) motion: Motion,
    pub(crate) combat: Combat,
    pub(crate) dig: DigState,
    pub(crate) binding: Binding,
    actors: Vec<Actor>,
    portals: Vec<[f64; 4]>,
}

pub(crate) struct Candidate {
    pub(crate) steps: Vec<NormalInput>,
    pub(crate) requires_resample: bool,
}

#[derive(Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Geometry {
    pub(crate) rect: [f64; 4],
    pub(crate) backing: [f64; 2],
    pub(crate) host: [f64; 4],
    pub(crate) camera: [f64; 2],
    pub(crate) zoom: f64,
    #[serde(rename = "uiScale")]
    pub(crate) ui_scale: f64,
    #[serde(rename = "pxPerDp")]
    pub(crate) px_per_dp: f64,
    pub(crate) viewport: [f64; 2],
    pub(crate) world: [f64; 2],
    pub(crate) dpr: f64,
    pub(crate) map: [f64; 2],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Pointer {
    #[serde(deserialize_with = "present")]
    pub(crate) world: Option<[f64; 2]>,
    #[serde(deserialize_with = "present")]
    pub(crate) picked: Option<u16>,
    pub(crate) ground_hover: bool,
    pub(crate) over_ui: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct InputState {
    pub(crate) left: bool,
    pub(crate) right: bool,
    pub(crate) dig_held: bool,
    pub(crate) run_held: bool,
    pub(crate) modifiers: [bool; 4],
    pub(crate) mode: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Motion {
    pub(crate) own: u16,
    pub(crate) fsm: u8,
    pub(crate) path: u16,
    pub(crate) route: u16,
    pub(crate) steering: bool,
    pub(crate) screen_held: bool,
    pub(crate) target_pending: bool,
    pub(crate) held_stop: bool,
    pub(crate) send_armed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Combat {
    #[serde(deserialize_with = "present")]
    pub(crate) target: Option<u16>,
    pub(crate) own_attack: bool,
    pub(crate) waiting: bool,
    pub(crate) fainted: bool,
    pub(crate) gun_worn: bool,
    pub(crate) gun_armed: bool,
    pub(crate) gun_ready: bool,
    pub(crate) gun_state: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DigState {
    pub(crate) fsm: u8,
    pub(crate) flags: u32,
    pub(crate) stage: u8,
    pub(crate) held: bool,
    pub(crate) in_state: bool,
    pub(crate) gauge: f64,
    #[serde(deserialize_with = "present")]
    pub(crate) area: Option<i32>,
    pub(crate) assist: u8,
    pub(crate) auto_mode: bool,
    pub(crate) auto_flag: bool,
    pub(crate) automatic: bool,
    pub(crate) crazy: bool,
    pub(crate) manual: bool,
    pub(crate) automatic_pet: bool,
    pub(crate) map_allowed: bool,
    #[serde(deserialize_with = "present")]
    pub(crate) tool_usable: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    dig: String,
    run: String,
}

impl Binding {
    pub(crate) fn dig(&self) -> InputKey {
        // parse_snapshot validates the physical code before constructing Snapshot.
        InputKey::Character(self.dig.as_bytes()[3] as char)
    }
    pub(crate) fn run(&self) -> InputKey {
        InputKey::Character(self.run.as_bytes()[3] as char)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Actor {
    raw: u16,
    #[serde(rename = "type")]
    kind: u8,
    x: f64,
    y: f64,
    hit: [f64; 4],
    hidden: bool,
    fading: bool,
}

impl Actor {
    fn contains(&self, p: [f64; 2]) -> bool {
        !self.hidden
            && self.hit[2] > 0.0
            && self.hit[3] > 0.0
            && p[0] >= self.x + self.hit[0]
            && p[0] < self.x + self.hit[0] + self.hit[2]
            && p[1] >= self.y + self.hit[1]
            && p[1] < self.y + self.hit[1] + self.hit[3]
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    version: u8,
    status: String,
    revision: u64,
    zone: u16,
    geometry: Geometry,
    pointer: Pointer,
    input: InputState,
    motion: Motion,
    combat: Combat,
    dig: DigState,
    binding: Binding,
    actors: Vec<Actor>,
    portals: Vec<[f64; 4]>,
}

fn hex(value: &Value) -> Result<u64> {
    let s = value.as_str().ok_or(NetError::Malformed)?;
    if s.is_empty()
        || s.len() > 16
        || s.starts_with('0')
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(NetError::Malformed);
    }
    u64::from_str_radix(s, 16)
        .ok()
        .filter(|n| *n > 0)
        .ok_or(NetError::Malformed)
}

fn bounded(value: f64, min: f64, max: f64) -> bool {
    value.is_finite() && value >= min && value <= max
}

fn physical_code(code: &str) -> bool {
    code.len() == 4 && code.starts_with("Key") && code.as_bytes()[3].is_ascii_uppercase()
}

fn present<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

fn bounded_shape(v: &Value, depth: u8, budget: &mut usize) -> bool {
    if depth > 8 || *budget == 0 {
        return false;
    }
    *budget -= 1;
    match v {
        Value::String(s) => s.len() <= 64,
        Value::Array(a) => {
            a.len() <= MAX_ACTORS && a.iter().all(|v| bounded_shape(v, depth + 1, budget))
        }
        Value::Object(o) => {
            o.len() <= 32
                && o.iter()
                    .all(|(k, v)| k.len() <= 32 && bounded_shape(v, depth + 1, budget))
        }
        _ => true,
    }
}

/// `captured` is the original Wry callback Instant. Subtract page age exactly once.
pub(crate) fn parse_snapshot(value: &Value, captured: Instant) -> Result<Snapshot> {
    if !bounded_shape(value, 0, &mut 16384) {
        return Err(NetError::Capacity);
    }
    if value["kind"] != "sample" {
        return Err(NetError::Malformed);
    }
    let host_epoch = hex(&value["epoch"])?;
    let layout_epoch = hex(&value["layout"])?;
    hex(&value["request"])?;
    let raw_ingress = value["rawIngress"]
        .as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or(NetError::Malformed)?;
    let age = value["ageMs"]
        .as_u64()
        .filter(|n| *n <= 250)
        .ok_or(NetError::Timeout)?;
    let captured = captured
        .checked_sub(Duration::from_millis(age))
        .ok_or(NetError::Malformed)?;
    let v = &value["snapshot"];
    if v["status"] == "unavailable" {
        if v.as_object().is_none_or(|o| o.len() != 3)
            || v["version"] != 1
            || !matches!(
                v["reason"].as_str(),
                Some(
                    "build"
                        | "cold"
                        | "memory"
                        | "capacity"
                        | "changing"
                        | "shape"
                        | "type"
                        | "generic"
                        | "binding"
                        | "input"
                        | "mode"
                        | "scene"
                        | "layout"
                        | "routing"
                )
            )
        {
            return Err(NetError::Malformed);
        }
        return Err(if v["reason"] == "build" {
            NetError::BuildChanged
        } else {
            NetError::Unsupported
        });
    }
    if v["actors"].as_array().is_none_or(|a| a.len() > MAX_ACTORS)
        || v["portals"].as_array().is_none_or(|a| a.len() > 256)
        || v["binding"]["dig"]
            .as_str()
            .is_none_or(|s| !physical_code(s))
        || v["binding"]["run"]
            .as_str()
            .is_none_or(|s| !physical_code(s))
    {
        return Err(NetError::Malformed);
    }
    let p: Payload = serde_json::from_value(v.clone()).map_err(|_| NetError::Malformed)?;
    if p.version != 1
        || p.status != "ready"
        || p.revision == 0
        || p.revision > 9_007_199_254_740_991
        || p.zone == 0
        || p.motion.own == 0
        || p.motion.fsm > 64
        || p.dig.fsm != p.motion.fsm
        || p.dig.stage > 7
        || p.dig.assist > 4
        || p.dig.flags > 255
        || p.motion.path > 1024
        || p.motion.route > 1024
        || p.combat.gun_state > 5
        || !matches!(p.input.mode, 1 | 2)
        || !bounded(p.dig.gauge, 0.0, 200.0)
        || p.dig.area.is_some_and(|a| !(0..=200).contains(&a))
        || p.binding.dig == p.binding.run
        || p.combat.target == Some(0)
        || p.pointer.picked == Some(0)
    {
        return Err(NetError::Malformed);
    }
    let g = &p.geometry;
    if !g
        .rect
        .iter()
        .all(|n| bounded(*n, -1_000_000.0, 1_000_000.0))
        || g.rect[2] <= 0.0
        || g.rect[3] <= 0.0
        || !g
            .backing
            .iter()
            .all(|n| bounded(*n, 1.0, 16384.0) && n.fract() == 0.0)
        || !g
            .camera
            .iter()
            .all(|n| bounded(*n, -1_000_000.0, 1_000_000.0) && n.fract() == 0.0)
        || !g.host.iter().all(|n| bounded(*n, 0.0, 16384.0))
        || g.host[2] <= 0.0
        || g.host[3] <= 0.0
        || g.host[0] + g.host[2] > g.backing[0]
        || g.host[1] + g.host[3] > g.backing[1]
        || !bounded(g.zoom, 0.01, 100.0)
        || !bounded(g.ui_scale, 0.01, 100.0)
        || !bounded(g.px_per_dp, 0.01, 100.0)
        || !bounded(g.dpr, 0.1, 16.0)
        || !g
            .viewport
            .iter()
            .chain(g.world.iter())
            .all(|n| bounded(*n, 1.0, 16384.0) && n.fract() == 0.0)
        || !g
            .map
            .iter()
            .all(|n| bounded(*n, 1.0, 65535.0) && n.fract() == 0.0)
        || p.pointer
            .world
            .is_some_and(|a| !a.iter().all(|n| bounded(*n, -1_000_000.0, 1_000_000.0)))
    {
        return Err(NetError::Malformed);
    }
    let mut identities = std::collections::BTreeSet::new();
    for a in &p.actors {
        if a.raw == 0
            || a.kind > 64
            || !identities.insert(a.raw)
            || ![a.x, a.y, a.hit[0], a.hit[1]]
                .iter()
                .all(|n| bounded(*n, -65535.0, 65535.0))
            || !a.hit[2..].iter().all(|n| bounded(*n, 0.0, 65535.0))
        {
            return Err(NetError::Malformed);
        }
    }
    if !identities.contains(&p.motion.own)
        || p.pointer.picked.is_some_and(|id| !identities.contains(&id))
        || p.portals
            .iter()
            .any(|r| !r.iter().all(|n| bounded(*n, 0.0, 65535.0)) || r[0] >= r[2] || r[1] >= r[3])
    {
        return Err(NetError::Malformed);
    }
    if let Some(id) = p.pointer.picked {
        let point = p.pointer.world.ok_or(NetError::Malformed)?;
        let hits: Vec<_> = p.actors.iter().filter(|a| a.contains(point)).collect();
        if hits.len() != 1 || hits[0].raw != id || id == p.motion.own {
            return Err(NetError::Malformed);
        }
    }
    Ok(Snapshot {
        captured,
        host_epoch,
        layout_epoch,
        raw_ingress,
        revision: p.revision,
        zone: p.zone,
        geometry: p.geometry,
        pointer: p.pointer,
        input: p.input,
        motion: p.motion,
        combat: p.combat,
        dig: p.dig,
        binding: p.binding,
        actors: p.actors,
        portals: p.portals,
    })
}

fn fresh(s: &Snapshot) -> bool {
    Instant::now()
        .checked_duration_since(s.captured)
        .is_some_and(|age| age <= MAX_AGE)
}

fn motion_idle(s: &Snapshot) -> bool {
    s.motion.path == 0
        && s.motion.route == 0
        && !s.motion.steering
        && !s.motion.screen_held
        && !s.motion.target_pending
        && !s.motion.held_stop
        && s.motion.send_armed
}

fn combat_idle(s: &Snapshot) -> bool {
    s.combat.target.is_none()
        && !s.combat.own_attack
        && !s.combat.waiting
        && !s.combat.fainted
        && s.combat.gun_state != 5
}

fn manual(s: &Snapshot) -> bool {
    s.dig.manual
        && s.dig.map_allowed
        && !s.dig.automatic_pet
        && s.dig.assist == 0
        && !s.dig.auto_mode
        && !s.dig.auto_flag
        && !s.dig.automatic
        && !s.dig.crazy
}

pub(crate) fn idle(s: &Snapshot) -> bool {
    fresh(s)
        && matches!(s.motion.fsm, 0 | 1)
        && motion_idle(s)
        && combat_idle(s)
        && !s.input.left
        && !s.input.right
        && !s.input.dig_held
        && !s.input.run_held
        && !s.input.modifiers.iter().any(|m| *m)
        && !s.dig.in_state
        && s.dig.fsm != 5
        && s.dig.flags & 0x0c == 0
        && !s.dig.held
        && s.dig.assist == 0
        && !s.dig.auto_mode
        && !s.dig.auto_flag
        && !s.dig.automatic
        && !s.dig.crazy
}

fn project(s: &Snapshot, p: [f64; 2]) -> Result<[f64; 2]> {
    let g = &s.geometry;
    if !p
        .iter()
        .zip(g.map)
        .all(|(n, limit)| bounded(*n, 0.0, limit) && *n < limit)
    {
        return Err(NetError::Refused);
    }
    let local = [(p[0] - g.camera[0]) * g.zoom, (p[1] - g.camera[1]) * g.zoom];
    if local[0] < 1.0
        || local[1] < 1.0
        || local[0] >= g.host[2] - 1.0
        || local[1] >= g.host[3] - 1.0
    {
        return Err(NetError::Refused);
    }
    Ok([
        g.rect[0] + (g.host[0] + local[0]) * g.rect[2] / g.backing[0],
        g.rect[1] + (g.backing[1] - g.host[1] - g.host[3] + local[1]) * g.rect[3] / g.backing[1],
    ])
}

fn world_press(s: &Snapshot, p: [f64; 2], target: Option<u16>) -> Result<Candidate> {
    let [x, y] = project(s, p)?;
    let at_point = s
        .pointer
        .world
        .is_some_and(|q| (p[0] - q[0]).abs() <= 0.25 && (p[1] - q[1]).abs() <= 0.25);
    if !at_point || s.pointer.over_ui || s.pointer.picked != target || s.pointer.ground_hover {
        return Ok(Candidate {
            steps: vec![NormalInput::PointerMove { x, y }],
            requires_resample: true,
        });
    }
    Ok(Candidate {
        steps: vec![NormalInput::LeftDown],
        requires_resample: false,
    })
}

/// The caller owns session/action identity, step issuance, receipt gating and replay prevention.
pub(crate) fn plan(
    s: &Snapshot,
    now: At,
    fences: InputFences,
    _session: SessionKey,
    ticket: &ActionTicket,
    target_raw: Option<u16>,
) -> Result<Candidate> {
    if !fresh(s) || now >= fences.not_after {
        return Err(NetError::Timeout);
    }
    if s.host_epoch != fences.host_epoch || s.layout_epoch != fences.layout_epoch {
        return Err(NetError::StaleConnection);
    }
    if s.input.modifiers.iter().any(|v| *v)
        || s.input.left
        || s.input.right
        || s.input.dig_held
        || s.input.run_held
    {
        return Err(NetError::Busy);
    }
    match ticket.action {
        Action::MoveTo(p) => {
            if !idle(s) {
                return Err(NetError::Busy);
            }
            let p = [f64::from(p.x), f64::from(p.y)];
            ground_clear(s, p)?;
            world_press(s, p, None)
        }
        Action::BasicAttack(_) => {
            if !idle(s) {
                return Err(NetError::Busy);
            }
            if s.combat.gun_worn
                && !(s.combat.gun_armed
                    && s.combat.gun_ready
                    && matches!(s.combat.gun_state, 3 | 4))
            {
                return Err(NetError::Refused);
            }
            let id = target_raw
                .filter(|id| *id != s.motion.own)
                .ok_or(NetError::Refused)?;
            let a = s
                .actors
                .iter()
                .find(|a| a.raw == id)
                .ok_or(NetError::Refused)?;
            if a.kind != 3 || a.hidden || a.fading || a.hit[2] < 4.0 || a.hit[3] < 4.0 {
                return Err(NetError::Refused);
            }
            let p = [
                a.x + a.hit[0] + a.hit[2] / 2.0,
                a.y + a.hit[1] + a.hit[3] / 2.0,
            ];
            if s.actors
                .iter()
                .any(|other| other.raw != id && other.contains(p))
            {
                return Err(NetError::Refused);
            }
            world_press(s, p, Some(id))
        }
        Action::DigAt(p) => {
            if !manual(s)
                || s.dig.tool_usable != Some(true)
                || !motion_idle(s)
                || !combat_idle(s)
                || !matches!(s.dig.fsm, 0 | 1)
                || s.dig.in_state
                || s.dig.held
                || s.dig.flags & 0x08 != 0
            {
                return Err(NetError::Refused);
            }
            let p = [f64::from(p.x), f64::from(p.y)];
            ground_clear(s, p)?;
            project(s, p)?;
            if s.dig.flags & 4 == 0 {
                if !idle(s) {
                    return Err(NetError::Busy);
                }
                return Ok(Candidate {
                    steps: vec![NormalInput::KeyDown(s.binding.dig())],
                    requires_resample: false,
                });
            }
            world_press(s, p, None)
        }
        Action::Pickup(_) | Action::HpPotion(_) => Err(NetError::Unsupported),
    }
}

fn ground_clear(s: &Snapshot, p: [f64; 2]) -> Result<()> {
    if s.actors.iter().any(|a| a.contains(p))
        || s.portals
            .iter()
            .any(|r| p[0] >= r[0] && p[0] < r[2] && p[1] >= r[1] && p[1] < r[3])
    {
        return Err(NetError::Refused);
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DigPhase {
    Charging,
    Tracking,
}

/// The controller latches Charging until stage 5, acknowledges release, then uses Tracking.
/// A decision never acknowledges delivery. Compare a new capture with the dispatch Instant.
pub(crate) fn dig_button(s: &Snapshot, phase: DigPhase) -> Result<Option<bool>> {
    if !fresh(s)
        || !manual(s)
        || s.dig.tool_usable != Some(true)
        || s.input.modifiers.iter().any(|m| *m)
        || s.dig.fsm != 5
        || !s.dig.in_state
        || s.motion.path != 0
        || s.motion.route != 0
        || s.motion.steering
        || s.motion.target_pending
        || s.motion.held_stop
    {
        return Err(NetError::Refused);
    }
    let area = s.dig.area.ok_or(NetError::Unsupported)?;
    match s.dig.stage {
        1 => Ok(None),
        7 => Ok(Some(false)),
        2..=4 if phase == DigPhase::Charging => Ok(Some(true)),
        5 if phase == DigPhase::Charging => Ok(Some(false)),
        2..=6 if phase == DigPhase::Tracking => {
            let gauge = ((s.dig.gauge as f32 * 100.0_f32) / 200.0_f32) as f64;
            let marker = (((200 - area) as f64 * 100.0) / 200.0) as f32;
            Ok(Some(gauge <= f64::from(marker) + 10.0))
        }
        _ => Err(NetError::WrongPhase),
    }
}

/// Use only on a new post-dispatch capture, under unchanged geometry and physical fences.
pub(crate) fn press_sampled(s: &Snapshot, step: &NormalInput) -> Result<bool> {
    if !fresh(s) {
        return Err(NetError::Timeout);
    }
    Ok(match *step {
        NormalInput::PointerMove { .. } => return Err(NetError::Unsupported),
        NormalInput::LeftDown if s.dig.fsm == 5 => s.dig.held,
        NormalInput::LeftDown => {
            s.input.left
                && (s.motion.screen_held
                    || s.motion.path > 0
                    || s.motion.target_pending
                    || s.combat.target.is_some()
                    || s.combat.own_attack
                    || s.combat.gun_state == 5)
        }
        NormalInput::LeftUp => !s.input.left && !s.motion.screen_held && !s.dig.held,
        NormalInput::RightDown => s.input.right,
        NormalInput::RightUp => !s.input.right,
        NormalInput::KeyDown(k) if k == s.binding.dig() => s.input.dig_held,
        NormalInput::KeyUp(k) if k == s.binding.dig() => !s.input.dig_held,
        NormalInput::KeyDown(k) if k == s.binding.run() => s.input.run_held,
        NormalInput::KeyUp(k) if k == s.binding.run() => !s.input.run_held,
        _ => return Err(NetError::Unsupported),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentrickster_engine as e;
    use serde_json::json;
    use std::{
        num::{NonZeroU16, NonZeroU32},
        process::Command,
        sync::OnceLock,
    };

    fn value() -> Value {
        static SAMPLE: OnceLock<Value> = OnceLock::new();
        SAMPLE
            .get_or_init(|| {
                let out = Command::new("node")
                    .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sampler.cjs"))
                    .arg("--snapshot")
                    .output()
                    .expect("Node is required for invented heap tests");
                assert!(out.status.success(), "invented heap sampler failed");
                serde_json::from_slice(&out.stdout).expect("private synthetic snapshot")
            })
            .clone()
    }

    fn snapshot() -> Snapshot {
        parse_snapshot(&value(), Instant::now())
            .unwrap_or_else(|_| panic!("positive invented heap snapshot"))
    }

    fn fences() -> InputFences {
        InputFences {
            host_epoch: 7,
            layout_epoch: 11,
            not_after: At(1000),
        }
    }

    // Mint local capabilities through the real engine instead of forging private key fields.
    fn ticket(action: Action) -> (SessionKey, ActionTicket) {
        let mut engine = e::Engine::new(e::Mode::Assisted);
        let effects = engine.handle(e::Input::User {
            at: At(0),
            request: e::RequestKey(1),
            command: e::Command::Open,
        });
        let session = effects
            .io
            .iter()
            .find_map(|e| {
                if let e::Effect::Open { session, .. } = e {
                    Some(*session)
                } else {
                    None
                }
            })
            .expect("engine session");
        let label = || e::DisplayText::checked("Synthetic".into()).unwrap();
        let center = e::Point { x: 500, y: 300 };
        let setup = e::Setup::checked(e::SetupDraft {
            activity: e::Activity::Combat {
                targets: e::Targets::Only(vec![e::SpeciesKey(1)]),
            },
            boundary: e::Boundary::checked(e::MapKey(1), center, NonZeroU16::new(100).unwrap())
                .unwrap(),
            hp_at: e::Percent::checked(30).unwrap(),
            weight_limit: e::Percent::checked(90).unwrap(),
            goal: None,
        })
        .unwrap_or_else(|_| panic!("fixture setup"));
        engine.handle(e::Input::User {
            at: At(1),
            request: e::RequestKey(2),
            command: e::Command::Configure(setup),
        });
        let gauge = e::Gauge {
            current: 100,
            maximum: NonZeroU32::new(100),
        };
        let effects = engine.handle(e::Input::Observed {
            at: At(2),
            observation: e::Observation {
                session,
                event: e::Event::Baseline {
                    support: e::Support {
                        build: label(),
                        move_to: true,
                        basic_attack: true,
                        dig_cycle: true,
                        pickup: true,
                        hp_potion: false,
                        quiesce: true,
                    },
                    revision: e::EvidenceKey(1),
                    resumed: false,
                    facts: vec![
                        e::Fact::Position {
                            map: e::MapKey(1),
                            point: center,
                        },
                        e::Fact::Hp(gauge.clone()),
                        e::Fact::Weight(e::Gauge {
                            current: 10,
                            maximum: NonZeroU32::new(100),
                        }),
                        e::Fact::HpPotions(5),
                        e::Fact::Inventory(e::Inventory::Reset(vec![])),
                        e::Fact::Drill(e::Drill::Equipped {
                            life: 100,
                            depth: 10,
                        }),
                        e::Fact::InventorySettled(e::IngressMark(0)),
                        e::Fact::Entity(e::Entity {
                            key: e::EntityKey(1),
                            species: e::SpeciesKey(1),
                            label: label(),
                            position: center,
                            alive: true,
                            hp: Some(gauge),
                            engaged_by_other: Some(false),
                        }),
                    ],
                },
            },
        });
        if let Some(key) = effects.io.iter().find_map(|effect| match effect {
            e::Effect::Session {
                op: e::SessionOp::Quiesce(k),
                ..
            } => Some(*k),
            _ => None,
        }) {
            engine.handle(e::Input::Observed {
                at: At(3),
                observation: e::Observation {
                    session,
                    event: e::Event::Quiescent(key),
                },
            });
        }
        let effects = engine.handle(e::Input::User {
            at: At(4),
            request: e::RequestKey(3),
            command: e::Command::Start,
        });
        let mut ticket = effects
            .io
            .into_iter()
            .find_map(|effect| match effect {
                e::Effect::Session {
                    op: e::SessionOp::Act(t),
                    ..
                } => Some(t),
                _ => None,
            })
            .expect("engine action");
        ticket.action = action;
        ticket.not_after = At(1000);
        (session, ticket)
    }

    fn candidate(s: &Snapshot, action: Action, raw: Option<u16>) -> Result<Candidate> {
        let (session, t) = ticket(action);
        let c = plan(s, At(10), fences(), session, &t, raw)?;
        assert!(c.steps.len() <= 1);
        if c.requires_resample {
            assert!(
                c.steps
                    .iter()
                    .all(|v| matches!(v, NormalInput::PointerMove { .. }))
            );
        }
        Ok(c)
    }

    #[test]
    fn strict_private_parser_preserves_capture_and_rejects_missing_state() {
        let mut v = value();
        v["ageMs"] = json!(20);
        let now = Instant::now();
        let s = parse_snapshot(&v, now).unwrap_or_else(|_| panic!("sample"));
        assert_eq!(s.captured, now - Duration::from_millis(20));
        assert_eq!(
            (
                s.host_epoch,
                s.layout_epoch,
                s.raw_ingress,
                s.revision,
                s.zone
            ),
            (7, 11, 19, 1, 17)
        );
        for path in [
            ["pointer", "picked"],
            ["pointer", "world"],
            ["combat", "target"],
            ["dig", "area"],
            ["dig", "toolUsable"],
        ] {
            let mut v = value();
            v["snapshot"][path[0]]
                .as_object_mut()
                .unwrap()
                .remove(path[1]);
            assert!(
                parse_snapshot(&v, Instant::now()).is_err(),
                "nullable field must be present"
            );
        }
        for (field, bad) in [
            ("mode", json!(0)),
            ("left", json!(null)),
            ("right", json!("false")),
        ] {
            let mut v = value();
            v["snapshot"]["input"][field] = bad;
            assert!(parse_snapshot(&v, Instant::now()).is_err());
        }
        let mut v = value();
        v["snapshot"]["geometry"]["camera"] = json!([1]);
        assert!(parse_snapshot(&v, Instant::now()).is_err());
        let mut v = value();
        v["snapshot"]["geometry"]["zoom"] = json!(0);
        assert!(parse_snapshot(&v, Instant::now()).is_err());
        let mut v = value();
        v["snapshot"]["binding"]["dig"] = json!("Key1");
        assert!(parse_snapshot(&v, Instant::now()).is_err());
        let mut v = value();
        v["snapshot"]["dig"]["gauge"] = json!(-1);
        assert!(parse_snapshot(&v, Instant::now()).is_err());
        let mut v = value();
        v["snapshot"]["surprise"] = json!(false);
        assert!(parse_snapshot(&v, Instant::now()).is_err());
        let mut v = value();
        v["snapshot"] = json!({"version":1,"status":"unavailable","reason":"cold"});
        assert!(matches!(
            parse_snapshot(&v, Instant::now()),
            Err(NetError::Unsupported)
        ));
        v["snapshot"]["reason"] = json!("anything else");
        assert!(matches!(
            parse_snapshot(&v, Instant::now()),
            Err(NetError::Malformed)
        ));
        let mut v = value();
        v["ageMs"] = json!(251);
        assert!(matches!(
            parse_snapshot(&v, Instant::now()),
            Err(NetError::Timeout)
        ));
        let mut v = value();
        v["snapshot"]["actors"] = json!(vec![v["snapshot"]["actors"][0].clone(); 514]);
        assert!(parse_snapshot(&v, Instant::now()).is_err());
    }

    #[test]
    fn changed_geometry_waits_for_a_new_source_ui_probe_before_pressing() {
        for (flag, resample) in [("--stale-zoom", true), ("--fresh-zoom", false)] {
            let output = Command::new("node")
                .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sampler.cjs"))
                .arg(flag)
                .output()
                .expect("invented geometry trace");
            assert!(output.status.success());
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            let snapshot = parse_snapshot(&value, Instant::now()).unwrap();
            let plan = candidate(&snapshot, Action::BasicAttack(e::EntityKey(1)), Some(73))
                .expect("hover or verified press");
            assert_eq!(plan.requires_resample, resample);
            if resample {
                assert!(
                    matches!(plan.steps.as_slice(), [NormalInput::PointerMove { x, y }]
                    if (*x - 211.5).abs() < 1e-8 && (*y - 216.45).abs() < 1e-8)
                );
            } else {
                assert_eq!(plan.steps, vec![NormalInput::LeftDown]);
            }
        }
    }

    #[test]
    fn source_projection_requires_new_hover_and_exact_current_pick() {
        let action = Action::BasicAttack(e::EntityKey(1));
        let mut s = snapshot();
        s.pointer.world = None;
        s.pointer.picked = None;
        let c = candidate(&s, action, Some(73)).unwrap_or_else(|_| panic!("hover"));
        assert!(c.requires_resample);
        assert_eq!(
            c.steps,
            vec![NormalInput::PointerMove {
                x: 144.0,
                y: 155.25
            }]
        );
        s.pointer.world = Some([500.0, 300.0]);
        s.pointer.picked = Some(73);
        let c = candidate(&s, action, Some(73)).unwrap_or_else(|_| panic!("current pick"));
        assert!(!c.requires_resample);
        assert_eq!(c.steps, vec![NormalInput::LeftDown]);
        s.pointer.picked = None;
        assert!(
            candidate(&s, action, Some(73))
                .unwrap_or_else(|_| panic!("resample"))
                .requires_resample
        );
        s.actors[1].fading = true;
        assert!(candidate(&s, action, Some(73)).is_err());
        s.actors[1].fading = false;
        s.actors[0].x = 500.0;
        s.actors[0].y = 300.0;
        assert!(candidate(&s, action, Some(73)).is_err());
        assert!(candidate(&s, action, Some(41)).is_err());
        let mut s = snapshot();
        s.pointer.world = Some([600.0, 400.0]);
        s.pointer.picked = None;
        assert_eq!(
            candidate(&s, Action::MoveTo(e::Point { x: 600, y: 400 }), None)
                .unwrap_or_else(|_| panic!("move"))
                .steps,
            vec![NormalInput::LeftDown]
        );
        s.portals.push([590.0, 390.0, 610.0, 410.0]);
        assert!(candidate(&s, Action::MoveTo(e::Point { x: 600, y: 400 }), None).is_err());
        let mut s = snapshot();
        s.combat.gun_worn = true;
        s.combat.gun_armed = true;
        s.combat.gun_ready = true;
        s.combat.gun_state = 2;
        assert!(candidate(&s, action, Some(73)).is_err());
        s.combat.gun_state = 3;
        assert_eq!(
            candidate(&s, action, Some(73))
                .unwrap_or_else(|_| panic!("ready gun"))
                .steps,
            vec![NormalInput::LeftDown]
        );
    }

    #[test]
    fn manual_dig_arming_hold_acknowledgement_and_cleanup_are_distinct() {
        let action = Action::DigAt(e::Point { x: 600, y: 400 });
        let mut s = snapshot();
        assert!(candidate(&s, Action::DigAt(e::Point { x: -1, y: 400 }), None).is_err());
        assert_eq!(
            candidate(&s, action, None)
                .unwrap_or_else(|_| panic!("arm"))
                .steps,
            vec![NormalInput::KeyDown(InputKey::Character('F'))]
        );
        s.dig.flags = 4;
        s.pointer.world = Some([600.0, 400.0]);
        s.pointer.picked = None;
        assert_eq!(
            candidate(&s, action, None)
                .unwrap_or_else(|_| panic!("targeting"))
                .steps,
            vec![NormalInput::LeftDown]
        );
        s.dig.manual = false;
        assert!(candidate(&s, action, None).is_err());
        s.dig.manual = true;
        s.dig.fsm = 5;
        s.motion.fsm = 5;
        s.dig.in_state = true;
        s.dig.area = Some(70);
        for stage in [2, 3, 4] {
            s.dig.stage = stage;
            assert_eq!(dig_button(&s, DigPhase::Charging).unwrap(), Some(true));
        }
        s.dig.stage = 5;
        assert_eq!(dig_button(&s, DigPhase::Charging).unwrap(), Some(false));
        s.dig.gauge = 150.0;
        assert_eq!(dig_button(&s, DigPhase::Tracking).unwrap(), Some(true));
        s.dig.gauge = 150.01;
        assert_eq!(dig_button(&s, DigPhase::Tracking).unwrap(), Some(false));
        assert!(!press_sampled(&s, &NormalInput::LeftDown).unwrap());
        s.dig.held = true;
        assert!(press_sampled(&s, &NormalInput::LeftDown).unwrap());
        assert!(!press_sampled(&s, &NormalInput::LeftUp).unwrap());
        s.dig.held = false;
        assert!(press_sampled(&s, &NormalInput::LeftUp).unwrap());
        s.dig.stage = 7;
        assert_eq!(dig_button(&s, DigPhase::Tracking).unwrap(), Some(false));
        assert!(!idle(&s));
        s.dig.fsm = 0;
        s.motion.fsm = 0;
        s.dig.in_state = false;
        assert!(dig_button(&s, DigPhase::Tracking).is_err());
        assert!(!idle(&s), "EndLocal can re-arm targeting");
        s.dig.flags = 0;
        assert!(idle(&s), "stage 7 can remain after real FSM exit");
        s.input.right = true;
        assert!(press_sampled(&s, &NormalInput::RightDown).unwrap());
        assert!(!idle(&s));
        s.input.right = false;
        assert!(press_sampled(&s, &NormalInput::RightUp).unwrap());
        assert!(idle(&s));
    }

    #[test]
    fn queues_pending_shots_physical_fences_and_expiry_refuse_idle_or_press() {
        let mut s = snapshot();
        assert!(idle(&s));
        s.motion.target_pending = true;
        assert!(!idle(&s));
        s.motion.target_pending = false;
        s.motion.held_stop = true;
        assert!(!idle(&s));
        s.motion.held_stop = false;
        s.motion.path = 1;
        assert!(!idle(&s));
        s.motion.path = 0;
        s.combat.waiting = true;
        assert!(!idle(&s));
        s.combat.waiting = false;
        s.combat.gun_state = 5;
        assert!(!idle(&s));
        s.combat.gun_state = 2;
        assert!(idle(&s));
        s.dig.assist = 2;
        assert!(!idle(&s));
        s.dig.assist = 0;
        s.dig.held = true;
        assert!(!idle(&s));
        s.dig.held = false;
        s.input.modifiers[0] = true;
        assert!(candidate(&s, Action::BasicAttack(e::EntityKey(1)), Some(73)).is_err());
        s.input.modifiers[0] = false;
        let (session, t) = ticket(Action::BasicAttack(e::EntityKey(1)));
        let mut f = fences();
        f.layout_epoch += 1;
        assert!(matches!(
            plan(&s, At(10), f, session, &t, Some(73)),
            Err(NetError::StaleConnection)
        ));
        assert!(matches!(
            plan(&s, At(1000), fences(), session, &t, Some(73)),
            Err(NetError::Timeout)
        ));
        s.captured = Instant::now() - Duration::from_secs(1);
        assert!(!idle(&s));
        assert!(matches!(
            plan(&s, At(10), fences(), session, &t, Some(73)),
            Err(NetError::Timeout)
        ));
    }
}
