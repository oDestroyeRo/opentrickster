/// CSS coordinates in the embedded document, never native screen coordinates.
#[derive(Clone, Copy, PartialEq)]
pub struct CssPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy)]
pub struct Rectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy)]
pub struct CanvasGeometry {
    pub css: Rectangle,
    pub backing_width: f64,
    pub backing_height: f64,
    pub viewport_width: f64,
    pub viewport_height: f64,
    pub host: Rectangle,
    pub world_zoom: f64,
    pub camera_x: f64,
    pub camera_y: f64,
    pub pan_pixels: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    InvalidGeometry,
    KeyboardPan,
    OutsideGameArea,
}

/// The current-build ScreenToMap inverse. The caller must separately validate
/// GameplayScreen identity, page/session/layout revisions, and input readiness.
pub struct Projection(CanvasGeometry);
impl Projection {
    pub fn checked(g: CanvasGeometry) -> Result<Self, ProjectionError> {
        let values = [
            g.css.x,
            g.css.y,
            g.css.width,
            g.css.height,
            g.backing_width,
            g.backing_height,
            g.viewport_width,
            g.viewport_height,
            g.host.x,
            g.host.y,
            g.host.width,
            g.host.height,
            g.world_zoom,
            g.camera_x,
            g.camera_y,
            g.pan_pixels,
        ];
        if !values.into_iter().all(f64::is_finite)
            || [
                g.css.width,
                g.css.height,
                g.backing_width,
                g.backing_height,
                g.viewport_width,
                g.viewport_height,
                g.host.width,
                g.host.height,
                g.world_zoom,
            ]
            .into_iter()
            .any(|v| v <= 0.)
            || g.css.x < 0.
            || g.css.y < 0.
            || g.host.x < 0.
            || g.host.y < 0.
            || g.css.x + g.css.width > g.viewport_width
            || g.css.y + g.css.height > g.viewport_height
            || g.host.x + g.host.width > g.backing_width
            || g.host.y + g.host.height > g.backing_height
        {
            return Err(ProjectionError::InvalidGeometry);
        }
        if g.pan_pixels != 0. {
            return Err(ProjectionError::KeyboardPan);
        }
        Ok(Self(g))
    }

    pub fn map_to_css(&self, world_x: f64, world_y: f64) -> Result<CssPoint, ProjectionError> {
        let g = self.0;
        let sx = g.host.x + (world_x - g.camera_x) * g.world_zoom;
        let sy = g.host.y + g.host.height - (world_y - g.camera_y) * g.world_zoom;
        self.inside_host(sx, sy)?;
        Ok(CssPoint {
            x: g.css.x + sx / g.backing_width * g.css.width,
            y: g.css.y + (g.backing_height - sy) / g.backing_height * g.css.height,
        })
    }

    pub fn css_to_map(&self, point: CssPoint) -> Result<(f64, f64), ProjectionError> {
        let g = self.0;
        let sx = (point.x - g.css.x) * g.backing_width / g.css.width;
        let sy = g.backing_height - (point.y - g.css.y) * g.backing_height / g.css.height;
        self.inside_host(sx, sy)?;
        Ok((
            (sx - g.host.x) / g.world_zoom + g.camera_x,
            (g.host.y + g.host.height - sy) / g.world_zoom + g.camera_y,
        ))
    }

    fn inside_host(&self, x: f64, y: f64) -> Result<(), ProjectionError> {
        let host = self.0.host;
        if !x.is_finite()
            || !y.is_finite()
            || x < host.x
            || x >= host.x + host.width
            || y <= host.y
            || y > host.y + host.height
        {
            Err(ProjectionError::OutsideGameArea)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn geometry() -> CanvasGeometry {
        CanvasGeometry {
            css: Rectangle {
                x: 30.,
                y: 50.,
                width: 640.,
                height: 360.,
            },
            backing_width: 1280.,
            backing_height: 720.,
            viewport_width: 800.,
            viewport_height: 500.,
            host: Rectangle {
                x: 100.,
                y: 20.,
                width: 900.,
                height: 500.,
            },
            world_zoom: 2.,
            camera_x: 100.,
            camera_y: 200.,
            pan_pixels: 0.,
        }
    }
    #[test]
    fn source_transform_uses_actual_camera_host_rectangle_and_backing_size() {
        let projection = Projection::checked(geometry()).unwrap();
        let point = projection.map_to_css(400., 300.).unwrap();
        assert_eq!((point.x, point.y), (380., 250.));
        assert_eq!(projection.css_to_map(point).unwrap(), (400., 300.));
        let horizontal = projection.map_to_css(420., 300.).unwrap();
        assert!(horizontal.x > point.x && horizontal.y == point.y);
        let vertical = projection.map_to_css(400., 320.).unwrap();
        assert!(vertical.x == point.x && vertical.y > point.y);
    }
    #[test]
    fn backing_ratio_changes_do_not_assume_device_pixel_ratio() {
        let mut g = geometry();
        g.backing_width *= 2.;
        g.backing_height *= 2.;
        g.host.x *= 2.;
        g.host.y *= 2.;
        g.host.width *= 2.;
        g.host.height *= 2.;
        g.world_zoom *= 2.;
        let point = Projection::checked(g)
            .unwrap()
            .map_to_css(400., 300.)
            .unwrap();
        assert_eq!((point.x, point.y), (380., 250.));
    }
    #[test]
    fn unknown_geometry_pan_and_offscreen_clicks_are_refused() {
        let mut g = geometry();
        g.world_zoom = 0.;
        assert!(Projection::checked(g).is_err());
        g = geometry();
        g.camera_x = f64::NAN;
        assert!(Projection::checked(g).is_err());
        g = geometry();
        g.pan_pixels = 1.;
        assert!(matches!(
            Projection::checked(g),
            Err(ProjectionError::KeyboardPan)
        ));
        let projection = Projection::checked(geometry()).unwrap();
        assert!(matches!(
            projection.map_to_css(-1000., 300.),
            Err(ProjectionError::OutsideGameArea)
        ));
    }
}
