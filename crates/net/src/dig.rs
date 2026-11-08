use crate::{
    NetError, Result,
    wire::{Reader, Writer},
};

#[derive(Clone, Copy)]
pub(crate) struct Params {
    pub(crate) area: i32,
    pub(crate) speed: i32,
    pub(crate) mass: i32,
    pub(crate) limit: i32,
    pub(crate) nature: u32,
}
#[derive(Clone, Copy)]
pub(crate) struct Inputs {
    pub(crate) params: Params,
    pub(crate) life: u16,
    pub(crate) actor: u16,
    pub(crate) mode: u8,
}
#[derive(Clone, Copy)]
struct Spot {
    actor: u16,
    x: u16,
    y: u16,
    unusable: i16,
    bottom: i32,
    item: i32,
    nature: u8,
    key: u16,
}
impl Spot {
    fn parse(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let actor = r.u16()?;
        let x = r.u16()?;
        let y = r.u16()?;
        let branch = r.u8()?;
        let (unusable, bottom, item, nature) = if branch != 0 {
            (r.u16()? as i16, 0, 0, 0)
        } else {
            (0, r.u8()? as i32 * 5, r.u8()? as i32 * 5, r.u8()?)
        };
        let key = r.u16()?;
        r.u8()?;
        r.end()?;
        Ok(Self {
            actor,
            x,
            y,
            unusable,
            bottom,
            item,
            nature,
            key,
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Off,
    Waiting,
    Idle,
    Charging,
    Falling,
    Drilling,
    Coasting,
    Finished,
}
#[derive(Clone, Copy)]
enum Policy {
    Begin,
    Charge,
    Drill,
    Wait,
}
pub(crate) struct Cycle {
    inputs: Inputs,
    point: (u16, u16),
    stage: Stage,
    policy: Policy,
    accepted: bool,
    spot: Option<Spot>,
    begun: bool,
    held: bool,
    gauge: f32,
    depth4: f32,
    scoop_ms: i32,
    elapsed: i32,
    starts13: i32,
    key: u16,
    life: i32,
}
impl Cycle {
    pub(crate) fn new(inputs: Inputs, point: opentrickster_engine::Point) -> Result<Self> {
        let point = crate::actions::coordinate(point)?;
        let p = inputs.params;
        if !(1..200).contains(&p.area)
            || p.speed <= 0
            || p.mass <= 0
            || p.limit <= 0
            || inputs.life == 0
        {
            return Err(NetError::Unsupported);
        }
        Ok(Self {
            inputs,
            point,
            stage: Stage::Waiting,
            policy: Policy::Begin,
            accepted: false,
            spot: None,
            begun: false,
            held: false,
            gauge: 0.0,
            depth4: 0.0,
            scoop_ms: 0,
            elapsed: 0,
            starts13: 0,
            key: 0,
            life: inputs.life as i32,
        })
    }
    pub(crate) fn observe(&mut self, opcode: u16, payload: &[u8]) -> Result<()> {
        match opcode {
            45 => self.accepted = true,
            371 if self.accepted => {
                let spot = Spot::parse(payload)?;
                if spot.actor == self.inputs.actor && !self.begun {
                    if (spot.x, spot.y) != self.point {
                        return Err(NetError::Uncertain);
                    }
                    self.spot = Some(spot);
                }
            }
            50 => {
                let mut r = Reader::new(payload);
                let kind = r.u8()?;
                r.u16()?;
                r.u32()?;
                r.end()?;
                if self.accepted || (11..=19).contains(&kind) {
                    self.stage = Stage::Off;
                    self.held = false;
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn stopped(&self) -> bool {
        self.stage == Stage::Off
    }
    pub(crate) fn tick(&mut self, dt_ms: u64) -> Result<Option<(u16, Writer)>> {
        if !self.accepted || self.stage == Stage::Off || self.stage == Stage::Finished {
            return Ok(None);
        }
        let dt = i32::try_from(dt_ms).map_err(|_| NetError::Uncertain)?;
        let p = self.inputs.params;
        if self.stage == Stage::Waiting
            && let Some(spot) = self.spot
        {
            if spot.unusable > 0 {
                self.stage = Stage::Off;
                return Ok(None);
            }
            if spot.key & 1 != 0 && spot.item <= 0 {
                return Ok(self.cancel());
            }
            self.key = spot.key & 0xfffe;
            self.begun = true;
            self.stage = Stage::Idle;
        }
        let mut stage = self.stage;
        if stage == Stage::Idle {
            self.decay();
            if self.held {
                self.starts13 = self.starts13.wrapping_add(13);
                let wear =
                    if p.nature != 0 && self.spot.is_some_and(|s| s.nature as u32 != p.nature) {
                        2
                    } else {
                        1
                    };
                self.life = self.life.wrapping_sub(wear).max(0);
                stage = Stage::Charging;
                self.stage = stage;
            }
        }
        let marker = (200 - p.area) as f32;
        const K: f64 = f64::from_bits(0x3fc5_5555_6000_0000);
        match stage {
            Stage::Charging => {
                self.decay();
                self.gauge =
                    ((dt as f32 as f64) * K * (p.speed as f64) / 100.0 + self.gauge as f64) as f32;
                if self.gauge >= marker {
                    self.stage = Stage::Drilling;
                } else if !self.held {
                    self.stage = Stage::Falling;
                }
            }
            Stage::Falling => {
                self.decay();
                let sum = dt.wrapping_neg() as f64 * K * p.speed as f64 / 100.0 + self.gauge as f64;
                self.gauge = sum as f32;
                if sum <= f64::from_bits(0x3690_0000_0000_0000) {
                    self.gauge = 0.0;
                    self.stage = Stage::Idle;
                } else if self.held {
                    self.stage = Stage::Charging;
                }
            }
            Stage::Drilling | Stage::Coasting => {
                self.scoop_ms = self.scoop_ms.wrapping_add(dt);
                let delta = if stage == Stage::Drilling {
                    dt
                } else {
                    dt.wrapping_neg()
                };
                let sum = delta as f64 * K * p.speed as f64 / 100.0 + self.gauge as f64;
                self.gauge = sum as f32;
                if self.scoop_ms >= 120 {
                    self.scoop_ms = self.scoop_ms.wrapping_sub(120);
                    self.depth4 += (p.mass as f32) * 4.0;
                }
                if stage == Stage::Drilling {
                    if sum >= f64::from_bits(0x4068_ffff_f000_0000) {
                        self.stage = Stage::Idle;
                        self.gauge = 0.0;
                        self.depth4 = 0.0;
                    } else if !self.held {
                        self.stage = Stage::Coasting;
                    }
                } else if self.gauge < marker {
                    self.stage = Stage::Falling;
                } else if self.held {
                    self.stage = Stage::Drilling;
                }
            }
            _ => {}
        }
        if matches!(
            stage,
            Stage::Waiting | Stage::Charging | Stage::Falling | Stage::Drilling | Stage::Coasting
        ) {
            self.elapsed = self.elapsed.wrapping_add(dt);
        }
        let mut message = None;
        if matches!(
            self.stage,
            Stage::Idle | Stage::Charging | Stage::Falling | Stage::Drilling | Stage::Coasting
        ) {
            let spot = self.spot.ok_or(NetError::WrongPhase)?;
            let depth = self.depth4 * 0.25;
            let target = if spot.item <= 0 {
                spot.bottom
            } else {
                spot.item
            };
            let limit = if depth >= target as f32 {
                Some(target)
            } else if depth >= p.limit as f32 {
                Some(p.limit)
            } else {
                None
            };
            if let Some(limit) = limit {
                self.depth4 = limit as f32 * 4.0;
                self.stage = Stage::Finished;
                message = Some((
                    49,
                    end(
                        p,
                        self.key,
                        self.starts13 / 13,
                        self.elapsed,
                        (self.depth4 * 0.25) as i32,
                    ),
                ));
            }
        }
        self.policy();
        Ok(message)
    }
    fn decay(&mut self) {
        let d = self.depth4 * 0.25;
        self.depth4 = if d <= 0.35f32 {
            0.0
        } else {
            (d + (-0.35f32)) * 4.0
        };
    }
    fn policy(&mut self) {
        match self.policy {
            Policy::Begin => {
                if self.stage == Stage::Idle {
                    self.held = true;
                } else if self.stage == Stage::Charging {
                    self.policy = Policy::Charge;
                }
            }
            Policy::Charge => {
                if self.stage == Stage::Drilling {
                    self.held = false;
                    self.policy = Policy::Drill;
                } else {
                    self.held = true;
                }
            }
            Policy::Drill => {
                if self.stage == Stage::Finished {
                    self.held = false;
                    self.policy = Policy::Wait;
                } else {
                    let gauge_percent = (self.gauge * 100.0) / 200.0;
                    let marker_percent =
                        ((200 - self.inputs.params.area) as f64 * 100.0 / 200.0) as f32;
                    self.held = (gauge_percent as f64) <= marker_percent as f64 + 10.0;
                }
            }
            Policy::Wait => {}
        }
    }
    pub(crate) fn cancel(&mut self) -> Option<(u16, Writer)> {
        if self.stage == Stage::Off {
            return None;
        }
        let finished = self.stage == Stage::Finished;
        self.stage = Stage::Off;
        self.held = false;
        if finished {
            return None;
        }
        let mut w = Writer::new();
        w.u16((self.starts13 / 13) as u16);
        w.u16(self.elapsed as u16);
        Some((48, w))
    }
}
fn end(p: Params, key: u16, starts: i32, elapsed: i32, depth: i32) -> Writer {
    let crc = key as u32
        ^ p.area as u32
        ^ (p.speed as u32).wrapping_shl(1)
        ^ ((p.mass as u32 ^ (depth as u32).wrapping_shl(3) ^ (starts as u32).wrapping_shl(7))
            .wrapping_shl(5));
    let mut w = Writer::new();
    for n in [starts as u16, elapsed as u16, depth as u16, crc as u16] {
        w.u16(n);
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inputs(limit: i32) -> Inputs {
        Inputs {
            params: Params {
                area: 80,
                speed: 100,
                mass: 2,
                limit,
                nature: 0,
            },
            life: 100,
            actor: 7,
            mode: 2,
        }
    }
    fn spot(item: u8, key: u16) -> Vec<u8> {
        let mut w = Writer::new();
        for n in [7, 10, 20] {
            w.u16(n);
        }
        w.u8(0);
        w.u8(20);
        w.u8(item);
        w.u8(3);
        w.u16(key);
        w.u8(25);
        w.0.to_vec()
    }
    #[test]
    fn complete_normal_cycles_match_independent_reference() {
        for (item, limit, expected, frames) in [
            (8, 100, [1, 0, 0x40, 0x0c, 40, 0, 0xec, 0x2a], 197),
            (0, 100, [1, 0, 0x50, 0x1a, 100, 0, 0xec, 0x66], 422),
            (8, 20, [1, 0, 0x90, 0x07, 20, 0, 0xec, 0x16], 122),
        ] {
            let mut c =
                Cycle::new(inputs(limit), opentrickster_engine::Point { x: 10, y: 20 }).unwrap();
            c.observe(45, &[]).unwrap();
            c.observe(371, &spot(item, 0x1234)).unwrap();
            let mut sent = None;
            for frame in 1..=1000 {
                if let Some(message) = c.tick(16).unwrap() {
                    sent = Some((frame, message));
                    break;
                }
            }
            let (frame, (opcode, payload)) = sent.unwrap();
            assert_eq!(opcode, 49);
            assert_eq!(&*payload.0, &expected);
            assert_eq!(frame, frames);
            assert!(c.tick(16).unwrap().is_none());
            assert!(c.cancel().is_none());
        }
    }
    #[test]
    fn waiting_early_odd_key_and_active_cancel_use_real_counters() {
        let mut c = Cycle::new(inputs(100), opentrickster_engine::Point { x: 10, y: 20 }).unwrap();
        assert!(c.tick(16).unwrap().is_none());
        assert_eq!(c.elapsed, 0);
        c.observe(45, &[]).unwrap();
        c.tick(32).unwrap();
        c.observe(371, &spot(0, 0x1235)).unwrap();
        let (opcode, payload) = c.tick(16).unwrap().unwrap();
        assert_eq!(opcode, 48);
        assert_eq!(&*payload.0, &[0, 0, 32, 0]);
        assert!(c.stopped());
        assert!(c.cancel().is_none());
        let mut c = Cycle::new(inputs(100), opentrickster_engine::Point { x: 10, y: 20 }).unwrap();
        c.accepted = true;
        c.starts13 = 26;
        c.elapsed = 70000;
        assert_eq!(&*c.cancel().unwrap().1.0, &[2, 0, 0x70, 0x11]);
    }
    #[test]
    fn delayed_tick_scoops_once_and_overflow_precedes_completion() {
        let mut c = Cycle::new(inputs(100), opentrickster_engine::Point { x: 10, y: 20 }).unwrap();
        c.accepted = true;
        c.spot = Some(Spot::parse(&spot(8, 0x1234)).unwrap());
        c.begun = true;
        c.stage = Stage::Coasting;
        c.gauge = 180.0;
        c.tick(360).unwrap();
        assert_eq!(c.scoop_ms, 240);
        assert_eq!(c.depth4, 8.0);
        c.stage = Stage::Drilling;
        c.held = true;
        c.gauge = 199.0;
        c.depth4 = 100.0;
        assert!(c.tick(16).unwrap().is_none());
        assert!(c.stage == Stage::Idle);
        assert_eq!(c.depth4, 0.0);
        c.elapsed = i32::MAX;
        c.stage = Stage::Waiting;
        c.spot = None;
        c.tick(16).unwrap();
        assert_eq!(c.elapsed, i32::MIN + 15);
    }
}
