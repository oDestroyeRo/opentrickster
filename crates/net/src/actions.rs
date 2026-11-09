use crate::{NetError, Result, wire::Writer};
use opentrickster_engine::Point;

pub(crate) fn coordinate(point: Point) -> Result<(u16, u16)> {
    Ok((
        u16::try_from(point.x).map_err(|_| NetError::Malformed)?,
        u16::try_from(point.y).map_err(|_| NetError::Malformed)?,
    ))
}
pub(crate) fn movement(from: Point, to: Point, mode: u8) -> Result<Writer> {
    let (fx, fy) = coordinate(from)?;
    let (tx, ty) = coordinate(to)?;
    let mut w = Writer::new();
    for n in [fx, fy, tx, ty] {
        w.u16(n);
    }
    w.u8(mode);
    Ok(w)
}
pub(crate) fn normal_attack(target: u16, mode: u8) -> (Writer, Writer) {
    let mut selected = Writer::new();
    selected.u16(target);
    let mut attack = Writer::new();
    attack.u16(target);
    attack.u8(mode);
    (selected, attack)
}
pub(crate) fn gun_shot(target: u16, crc: u16, bunch: u16, mode: u8) -> Writer {
    let mut w = Writer::new();
    for n in [target, crc, bunch] {
        w.u16(n);
    }
    w.u8(mode);
    w
}
/// `Shoot`'s 16-bit check word (f24401): wrapping I32 shifts of the gun profile, the received
/// 512 key and the strict critical bit.
pub(crate) fn gun_crc(speed: i32, range: i32, start: i32, key: u16, critical: bool) -> u16 {
    let base = (range.wrapping_shl(3) ^ speed.wrapping_shl(7) ^ start).wrapping_shl(5);
    (base ^ i32::from(key) | i32::from(critical)) as u16
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_backed_ordinary_and_gun_serializers() {
        assert_eq!(
            &*movement(Point { x: 10, y: 20 }, Point { x: 30, y: 40 }, 2)
                .unwrap()
                .0,
            &[10, 0, 20, 0, 30, 0, 40, 0, 2]
        );
        assert!(movement(Point { x: -1, y: 0 }, Point { x: 0, y: 0 }, 0).is_err());
        let (selected, attack) = normal_attack(0x1234, 2);
        assert_eq!(&*selected.0, &[0x34, 0x12]);
        assert_eq!(&*attack.0, &[0x34, 0x12, 2]);
        let crc = gun_crc(7, 11, 3, 17, true);
        let shot = gun_shot(0x1234, crc, 2, 1);
        assert_eq!(shot.0.len(), 7);
        assert_eq!(&shot.0[2..4], &crc.to_le_bytes());
    }
}
