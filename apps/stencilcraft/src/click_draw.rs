//! Click-to-place drawing for the Polyline and Curve tools, following Silhouette Studio: each
//! click drops a point, clicking the first point closes the shape, a double-click (or Enter/Esc)
//! ends it open, and Backspace takes back the last point. Polyline points are corners; the
//! Curve tool passes a smooth curve through every point (Catmull-Rom, written as cubic Béziers).

use stencilcraft_geom::{BezPath, Point, Vec2};

/// The most points one draft keeps, so a stuck pointer cannot grow it without bound.
pub const MAX_POINTS: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickKind {
    Polyline,
    Curve,
}

/// What a click did to the draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickResult {
    /// A point was added; keep drawing.
    Added,
    /// The click landed on the first point: the shape is closed and finished.
    Closed,
    /// Nothing changed (too close to the last point, or the draft is full).
    Ignored,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClickDraft {
    pub kind: ClickKind,
    pub points: Vec<Point>,
    pub closed: bool,
}

impl ClickDraft {
    pub fn new(kind: ClickKind) -> Self {
        Self {
            kind,
            points: Vec::new(),
            closed: false,
        }
    }

    /// A click at `at`. `snap` is the distance (mm) within which a click counts as "on" an
    /// existing point; `constrain` (Shift) snaps the new segment to 45° steps.
    pub fn click(&mut self, at: Point, snap: f64, constrain: bool) -> ClickResult {
        if !at.x.is_finite() || !at.y.is_finite() {
            return ClickResult::Ignored;
        }
        let snap = if snap.is_finite() { snap.max(0.0) } else { 0.0 };
        if let Some(first) = self.points.first()
            && self.points.len() >= 3
            && (at - *first).hypot() <= snap
        {
            self.closed = true;
            return ClickResult::Closed;
        }
        let at = match self.points.last() {
            Some(last) if constrain => constrain_45(*last, at),
            _ => at,
        };
        if self
            .points
            .last()
            .is_some_and(|last| (at - *last).hypot() <= snap.max(1e-9))
            || self.points.len() >= MAX_POINTS
        {
            return ClickResult::Ignored;
        }
        self.points.push(at);
        ClickResult::Added
    }

    /// Backspace: take back the last point. Returns whether anything is left.
    pub fn undo(&mut self) -> bool {
        self.points.pop();
        !self.points.is_empty()
    }

    /// The path drawn so far, plus a live segment to `cursor` while drawing.
    pub fn preview(&self, cursor: Option<Point>, constrain: bool) -> BezPath {
        let mut points = self.points.clone();
        if !self.closed
            && let (Some(cursor), Some(last)) = (cursor, self.points.last())
            && cursor.x.is_finite()
            && cursor.y.is_finite()
        {
            points.push(if constrain {
                constrain_45(*last, cursor)
            } else {
                cursor
            });
        }
        build(self.kind, &points, self.closed)
    }

    /// The finished path, or `None` when fewer than two points were placed.
    pub fn finish(&self) -> Option<BezPath> {
        (self.points.len() >= 2).then(|| build(self.kind, &self.points, self.closed))
    }
}

/// `to` moved onto the nearest 45° ray from `from`, keeping its distance.
pub fn constrain_45(from: Point, to: Point) -> Point {
    let run = to - from;
    let length = run.hypot();
    if length == 0.0 || !length.is_finite() {
        return to;
    }
    let step = std::f64::consts::FRAC_PI_4;
    let angle = (run.y.atan2(run.x) / step).round() * step;
    from + Vec2::from_angle(angle) * length
}

fn build(kind: ClickKind, points: &[Point], closed: bool) -> BezPath {
    match kind {
        ClickKind::Polyline => polyline(points, closed),
        ClickKind::Curve => curve_through(points, closed),
    }
}

pub fn polyline(points: &[Point], closed: bool) -> BezPath {
    let mut path = BezPath::new();
    let mut iter = points.iter();
    if let Some(first) = iter.next() {
        path.move_to(*first);
        for p in iter {
            path.line_to(*p);
        }
        if closed && points.len() >= 3 {
            path.close_path();
        }
    }
    path
}

/// A smooth curve through every point (uniform Catmull-Rom as cubic Béziers). Open curves
/// repeat their end points as the outer neighbours; closed curves wrap around.
pub fn curve_through(points: &[Point], closed: bool) -> BezPath {
    let n = points.len();
    if n < 3 {
        return polyline(points, false);
    }
    let closed = closed && n >= 3;
    let at = |i: isize| -> Point {
        let len = n as isize;
        let index = if closed {
            i.rem_euclid(len)
        } else {
            i.clamp(0, len - 1)
        };
        // `index` is within 0..n by construction; fall back to the first point anyway.
        usize::try_from(index)
            .ok()
            .and_then(|i| points.get(i).copied())
            .unwrap_or(Point::ORIGIN)
    };
    let mut path = BezPath::new();
    path.move_to(at(0));
    let segments = if closed { n } else { n - 1 };
    for i in 0..segments as isize {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let c1 = p1 + (p2 - p0) / 6.0;
        let c2 = p2 - (p3 - p1) / 6.0;
        path.curve_to(c1, c2, p2);
    }
    if closed {
        path.close_path();
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    use stencilcraft_geom::CubicBez;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// The last element is a close (the only element without an end point).
    fn ends_closed(path: &BezPath) -> bool {
        path.elements()
            .last()
            .is_some_and(|e| e.end_point().is_none())
    }

    #[test]
    fn clicking_the_first_point_closes_a_polyline() {
        let mut draft = ClickDraft::new(ClickKind::Polyline);
        assert_eq!(draft.click(p(0.0, 0.0), 2.0, false), ClickResult::Added);
        assert_eq!(draft.click(p(10.0, 0.0), 2.0, false), ClickResult::Added);
        // Two points are not enough to close: this lands as a new point near the start.
        assert_eq!(draft.click(p(5.0, 8.0), 2.0, false), ClickResult::Added);
        assert_eq!(draft.click(p(1.0, 1.0), 2.0, false), ClickResult::Closed);
        let path = draft.finish().expect("closed triangle");
        assert_eq!(path.elements().len(), 4, "move, 2 lines, close: {path:?}");
        assert!(ends_closed(&path));
    }

    #[test]
    fn repeated_clicks_on_the_last_point_are_ignored_so_double_click_adds_one_point() {
        let mut draft = ClickDraft::new(ClickKind::Polyline);
        draft.click(p(0.0, 0.0), 2.0, false);
        draft.click(p(10.0, 0.0), 2.0, false);
        assert_eq!(draft.click(p(10.5, 0.0), 2.0, false), ClickResult::Ignored);
        assert_eq!(draft.points.len(), 2);
        assert!(draft.finish().is_some());
    }

    #[test]
    fn shift_constrains_each_segment_to_45_degrees() {
        let mut draft = ClickDraft::new(ClickKind::Polyline);
        draft.click(p(0.0, 0.0), 1.0, false);
        draft.click(p(10.0, 1.0), 1.0, true);
        let second = draft.points[1];
        assert!(second.y.abs() < 1e-9, "{second:?}");
        draft.click(p(20.0, 9.0), 1.0, true);
        let third = draft.points[2];
        assert!(
            ((third.x - second.x) - (third.y - second.y)).abs() < 1e-9,
            "{third:?}"
        );
    }

    #[test]
    fn the_curve_passes_through_every_clicked_point() {
        let points = [p(0.0, 0.0), p(10.0, 10.0), p(20.0, 0.0), p(30.0, 10.0)];
        let path = curve_through(&points, false);
        let ends: Vec<Point> = path
            .elements()
            .iter()
            .filter_map(|e| e.end_point())
            .collect();
        assert_eq!(ends, points.to_vec());
        // Interior points are smooth: incoming and outgoing tangents are collinear.
        let cubics: Vec<CubicBez> = path.segments().map(|s| s.to_cubic()).collect();
        for pair in cubics.windows(2) {
            if let [a, b] = pair {
                let t_in = a.p3 - a.p2;
                let t_out = b.p1 - b.p0;
                assert!(t_in.cross(t_out).abs() < 1e-9, "{a:?} {b:?}");
            }
        }
        let closed = curve_through(&points, true);
        assert!(ends_closed(&closed));
        assert_eq!(closed.segments().count(), points.len());
    }

    #[test]
    fn hostile_input_never_panics() {
        let mut draft = ClickDraft::new(ClickKind::Curve);
        assert_eq!(
            draft.click(p(f64::NAN, 0.0), 1.0, true),
            ClickResult::Ignored
        );
        assert_eq!(
            draft.click(p(0.0, 0.0), f64::INFINITY, true),
            ClickResult::Added
        );
        assert!(draft.finish().is_none(), "one point is not a path");
        assert!(!draft.undo());
        assert!(!draft.undo(), "undo on an empty draft is harmless");
        assert_eq!(constrain_45(p(1.0, 1.0), p(1.0, 1.0)), p(1.0, 1.0));
        let _ = draft.preview(Some(p(f64::NAN, f64::NAN)), true);
    }
}
