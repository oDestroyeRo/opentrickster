pub const TOOLBAR: f32 = 52.;
pub const PULSE: f32 = 3.;
pub const PANEL: f32 = 344.;
pub const TICKER: f32 = 34.;
pub const STRIP_MIN: f32 = 150.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Fits the child in the laid-out stage. The three-pixel margin contains the focus ring.
pub fn game_rect(x: f32, y: f32, width: f32, height: f32) -> Option<GameRect> {
    if ![x, y, width, height].into_iter().all(f32::is_finite) {
        return None;
    }
    let available_width = (width - 6.).max(0.);
    let available_height = (height - 6.).max(0.);
    let width = available_width.min(available_height * 16. / 9.);
    let height = width * 9. / 16.;
    (width >= 1. && height >= 1.).then_some(GameRect {
        x: x + 3. + (available_width - width) / 2.,
        y: y + 3. + (available_height - height) / 2.,
        width,
        height,
    })
}

pub fn stage_height(window_width: f32, window_height: f32, panel: bool) -> f32 {
    let body = (window_height - TOOLBAR - PULSE).max(0.);
    if panel {
        let width = (window_width - PANEL - 6.).max(0.);
        (width * 9. / 16. + 6.).min((body - STRIP_MIN).max(0.))
    } else {
        (body - TICKER).max(0.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_stays_in_reserved_stage_at_all_supported_sizes() {
        for (width, height) in [(972., 641.), (1252., 772.), (1316., 757.), (1560., 960.)] {
            for panel in [true, false] {
                let stage = stage_height(width, height, panel);
                let pane = width - if panel { PANEL } else { 0. };
                let r = game_rect(0., TOOLBAR + PULSE, pane, stage).unwrap();
                assert!((r.width / r.height - 16. / 9.).abs() < 0.00001);
                assert!(r.x >= 3. && r.x + r.width <= pane - 2.99);
                assert!(r.y >= TOOLBAR + PULSE + 3.);
                assert!(r.y + r.height <= TOOLBAR + PULSE + stage - 2.99);
                if panel {
                    assert!(height - TOOLBAR - PULSE - stage >= STRIP_MIN);
                }
            }
        }
    }

    #[test]
    fn zero_sized_or_nonfinite_stage_does_not_produce_native_bounds() {
        assert!(game_rect(0., 0., 0., 0.).is_none());
        assert!(game_rect(0., 0., f32::NAN, 500.).is_none());
    }
}
