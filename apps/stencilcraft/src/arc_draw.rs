//! The Arc tool, following Silhouette Studio: drag from the centre out to the arc's start point
//! (that sets the radius), then move the pointer to sweep the arc at that radius and click to
//! place it. The sweep follows the pointer continuously, so it can pass 180° and wrap up to a
//! full turn in either direction.

use stencilcraft_geom::{BezPath, Point, Vec2};

use std::f64::consts::{FRAC_PI_2, TAU};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcDraft {
    pub centre: Point,
    pub radius: f64,
    /// Angle of the start point from the centre (radians, y down as on the canvas).
    pub start_angle: f64,
    /// Signed sweep so far (radians), kept within ±2π.
    pub sweep: f64,
    /// The pointer angle seen last, to unwrap the sweep across ±π.
    last_pointer_angle: f64,
}

impl ArcDraft {
    /// From the first drag; `None` when the radius is too small or not finite.
    pub fn new(centre: Point, start: Point) -> Option<Self> {
        let run = start - centre;
        let radius = run.hypot();
        if !(radius.is_finite() && radius >= 0.5 && centre.x.is_finite() && centre.y.is_finite()) {
            return None;
        }
        let start_angle = run.y.atan2(run.x);
        Some(Self {
            centre,
            radius,
            start_angle,
            sweep: 0.0,
            last_pointer_angle: start_angle,
        })
    }

    /// The pointer moved to `pointer`: extend the sweep by the angle it turned through.
    /// `snap` (Shift) rounds the sweep to 15° steps.
    pub fn track(&mut self, pointer: Point, snap: bool) {
        let run = pointer - self.centre;
        if run.hypot() < 1e-9 || !run.x.is_finite() || !run.y.is_finite() {
            return;
        }
        let angle = run.y.atan2(run.x);
        let mut turn = angle - self.last_pointer_angle;
        if turn > std::f64::consts::PI {
            turn -= TAU;
        } else if turn < -std::f64::consts::PI {
            turn += TAU;
        }
        self.last_pointer_angle = angle;
        self.sweep = (self.sweep + turn).clamp(-TAU, TAU);
        if snap {
            let step = 15f64.to_radians();
            self.sweep = (self.sweep / step).round() * step;
        }
    }

    pub fn start(&self) -> Point {
        self.centre + Vec2::from_angle(self.start_angle) * self.radius
    }

    /// Where the arc ends after the current sweep.
    pub fn end(&self) -> Point {
        self.centre + Vec2::from_angle(self.start_angle + self.sweep) * self.radius
    }

    /// The sweep in degrees as shown while drawing: its size, unsigned, as Silhouette shows it.
    pub fn sweep_degrees(&self) -> f64 {
        self.sweep.to_degrees().abs()
    }

    /// The arc so far, or `None` while it has no length.
    pub fn path(&self) -> Option<BezPath> {
        (self.sweep.abs() > 1e-6)
            .then(|| arc_path(self.centre, self.radius, self.start_angle, self.sweep))
    }
}

/// A circular arc as cubic Béziers, one per quarter turn or less.
pub fn arc_path(centre: Point, radius: f64, start_angle: f64, sweep: f64) -> BezPath {
    let sweep = sweep.clamp(-TAU, TAU);
    let pieces = (sweep.abs() / FRAC_PI_2).ceil().max(1.0) as usize;
    let step = sweep / pieces as f64;
    // Control handle length for a circular arc of `step` radians.
    let k = 4.0 / 3.0 * (step / 4.0).tan() * radius;
    let at = |angle: f64| centre + Vec2::from_angle(angle) * radius;
    let tangent = |angle: f64| Vec2::from_angle(angle + FRAC_PI_2);
    let mut path = BezPath::new();
    path.move_to(at(start_angle));
    for i in 0..pieces {
        let a0 = start_angle + step * i as f64;
        let a1 = a0 + step;
        path.curve_to(at(a0) + tangent(a0) * k, at(a1) - tangent(a1) * k, at(a1));
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_geom::{ParamCurve, Shape};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn the_first_drag_sets_centre_radius_and_start() {
        let draft = ArcDraft::new(p(10.0, 10.0), p(20.0, 10.0)).expect("radius 10");
        assert!((draft.radius - 10.0).abs() < 1e-12);
        assert_eq!(draft.start(), p(20.0, 10.0));
        assert!(
            ArcDraft::new(p(0.0, 0.0), p(0.1, 0.0)).is_none(),
            "too small"
        );
        assert!(ArcDraft::new(p(f64::NAN, 0.0), p(5.0, 0.0)).is_none());
    }

    #[test]
    fn the_sweep_follows_the_pointer_past_half_a_turn() {
        let mut draft = ArcDraft::new(p(0.0, 0.0), p(10.0, 0.0)).expect("arc");
        // Pointer goes round through +y (screen down), then past the opposite side.
        for degrees in [30.0_f64, 90.0, 150.0, 179.0, -170.0, -120.0] {
            draft.track(
                Point::new(0.0, 0.0) + Vec2::from_angle(degrees.to_radians()) * 7.0,
                false,
            );
        }
        assert!(
            (draft.sweep.to_degrees() - 240.0).abs() < 1e-6,
            "{}",
            draft.sweep.to_degrees()
        );
        let path = draft.path().expect("arc path");
        // Every point of the arc lies on the circle.
        for seg in path.segments() {
            for i in 0..=8 {
                let q = seg.eval(f64::from(i) / 8.0);
                assert!((q.to_vec2().hypot() - 10.0).abs() < 0.01, "{q:?}");
            }
        }
        assert_eq!(path.segments().count(), 3, "240° is three pieces");
        let bounds = path.bounding_box();
        assert!(bounds.width() > 19.0, "{bounds:?}");
    }

    #[test]
    fn shift_snaps_to_15_degree_steps_and_sweep_is_bounded() {
        let mut draft = ArcDraft::new(p(0.0, 0.0), p(10.0, 0.0)).expect("arc");
        draft.track(
            Point::new(0.0, 0.0) + Vec2::from_angle(37f64.to_radians()) * 3.0,
            true,
        );
        assert!((draft.sweep.to_degrees() - 30.0).abs() < 1e-9);
        for _ in 0..20 {
            for degrees in [90.0_f64, 180.0, -90.0, 0.0] {
                draft.track(Vec2::from_angle(degrees.to_radians()).to_point(), false);
            }
        }
        assert!(draft.sweep.abs() <= TAU + 1e-12);
        draft.track(p(f64::NAN, 1.0), false);
        draft.track(p(0.0, 0.0), false);
        assert!(draft.sweep.is_finite());
    }
}
