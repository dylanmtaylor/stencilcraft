//! Generators and effects for StencilCraft, in pure geometry (no document, no UI):
//! parametric shapes, sketch/stipple fills, rhinestone templates, pop-up card struts,
//! jigsaw puzzles, weeding boxes, registration marks, warps, perforated and wavy lines.
//!
//! Every function takes millimetre geometry (y up) and returns `Result`: parameters come
//! from untrusted messages, so each is checked finite and in range, and output sizes are
//! capped ([`MAX_ITEMS`], [`MAX_POINTS`]). Behaviour follows the public user documentation
//! of consumer cutting software (`docs/reference-apps.md`); the algorithms are our own.

#![forbid(unsafe_code)]

pub mod along;
pub mod clip;
pub mod fill;
pub mod lines;
pub mod popup;
pub mod puzzle;
pub mod rhinestone;
pub mod shapes;
pub mod warp;
pub mod weed;

use kurbo::{BezPath, PathEl, Point, Rect, Shape};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct GenError(pub String);

pub type Result<T> = std::result::Result<T, GenError>;

/// Most discrete items (stones, dots, dashes, lines) one generator may emit.
pub const MAX_ITEMS: usize = 20_000;
/// Most polyline points one generator may emit or work on.
pub const MAX_POINTS: usize = 500_000;

pub(crate) fn bad<T>(message: impl Into<String>) -> Result<T> {
    Err(GenError(message.into()))
}

/// `value` must be finite and within `[min, max]`.
pub(crate) fn check_range(value: f64, min: f64, max: f64, what: &str) -> Result<f64> {
    if !value.is_finite() || value < min || value > max {
        return bad(format!(
            "{what} must be between {min} and {max} (got {value})"
        ));
    }
    Ok(value)
}

/// `value` must be finite and strictly positive, at most `max`.
pub(crate) fn check_positive(value: f64, max: f64, what: &str) -> Result<f64> {
    if !value.is_finite() || value <= 0.0 || value > max {
        return bad(format!(
            "{what} must be above 0 and at most {max} (got {value})"
        ));
    }
    Ok(value)
}

/// Largest length (mm) any generator accepts: 100 m.
pub(crate) const MAX_MM: f64 = 100_000.0;

/// A flattened contour.
#[derive(Debug, Clone, PartialEq)]
pub struct Contour {
    pub points: Vec<Point>,
    pub closed: bool,
}

/// Flattens `path` into contours with a tolerance relative to its size (closed contours do
/// not repeat their first point). Errors on non-finite or oversized geometry.
pub fn flatten(path: &BezPath) -> Result<Vec<Contour>> {
    let bounds = path_bounds(path)?;
    let size = bounds.width().max(bounds.height());
    let tolerance = (size * 2e-4).max(1e-3);
    let mut out: Vec<Contour> = Vec::new();
    let mut total = 0usize;
    let mut overflow = false;
    vectorcraft_geom::flatten(path.iter(), tolerance, |el| {
        if overflow {
            return;
        }
        match el {
            PathEl::MoveTo(p) => out.push(Contour {
                points: vec![p],
                closed: false,
            }),
            PathEl::LineTo(p) => {
                if let Some(c) = out.last_mut() {
                    c.points.push(p);
                }
            }
            PathEl::ClosePath => {
                if let Some(c) = out.last_mut() {
                    c.closed = true;
                }
            }
            _ => {}
        }
        total += 1;
        overflow = total > MAX_POINTS;
    });
    if overflow {
        return bad(format!("path is too detailed (over {MAX_POINTS} points)"));
    }
    for c in &mut out {
        c.points.dedup_by(|a, b| a.distance(*b) < 1e-9);
        if c.closed
            && c.points.len() > 1
            && c.points
                .first()
                .zip(c.points.last())
                .is_some_and(|(a, b)| a.distance(*b) < 1e-9)
        {
            c.points.pop();
        }
    }
    out.retain(|c| c.points.len() > 1);
    Ok(out)
}

/// The closed contours of `path` as polygons (for fills and clipping).
pub fn polygons(path: &BezPath) -> Result<Vec<Vec<Point>>> {
    let polys: Vec<Vec<Point>> = flatten(path)?
        .into_iter()
        .filter(|c| c.closed && c.points.len() >= 3)
        .map(|c| c.points)
        .collect();
    if polys.is_empty() {
        return bad("the shape has no closed contour to work inside");
    }
    Ok(polys)
}

/// Bounds of a path, refusing empty or non-finite geometry and absurd sizes.
pub fn path_bounds(path: &BezPath) -> Result<Rect> {
    let finite = path.elements().iter().all(|el| match *el {
        PathEl::MoveTo(p) | PathEl::LineTo(p) => p.is_finite(),
        PathEl::QuadTo(a, b) => a.is_finite() && b.is_finite(),
        PathEl::CurveTo(a, b, c) => a.is_finite() && b.is_finite() && c.is_finite(),
        PathEl::ClosePath => true,
    });
    if path.elements().is_empty() || !finite {
        return bad("path has no finite geometry");
    }
    let r = path.bounding_box();
    if r.width() > MAX_MM || r.height() > MAX_MM || r.x0.abs() > MAX_MM || r.y0.abs() > MAX_MM {
        return bad(format!("path is larger than {MAX_MM} mm"));
    }
    Ok(r)
}

/// A polyline as a path.
pub fn polyline_path(points: &[Point], closed: bool, out: &mut BezPath) {
    let mut it = points.iter();
    let Some(first) = it.next() else { return };
    out.move_to(*first);
    for p in it {
        out.line_to(*p);
    }
    if closed {
        out.close_path();
    }
}

/// Many polylines as one compound path.
pub fn polylines_path(lines: &[Vec<Point>], closed: bool) -> BezPath {
    let mut out = BezPath::new();
    for l in lines {
        polyline_path(l, closed, &mut out);
    }
    out
}

/// Even-odd point-in-polygon over a set of rings.
pub fn inside(polys: &[Vec<Point>], p: Point) -> bool {
    let mut odd = false;
    for poly in polys {
        let n = poly.len();
        for i in 0..n {
            let (Some(a), Some(b)) = (poly.get(i), poly.get((i + 1) % n)) else {
                continue;
            };
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                odd = !odd;
            }
        }
    }
    odd
}

/// Offsets closed polygons by `delta` (negative insets) through VectorCraft's robust
/// offsetter; returns the resulting rings (empty when an inset swallows the shape).
pub fn offset_polygons(polys: &[Vec<Point>], delta: f64) -> Result<Vec<Vec<Point>>> {
    if delta.abs() < 1e-9 {
        return Ok(polys.to_vec());
    }
    let path = vectorcraft_geom::PathData::from_bezpath(&polylines_path(polys, true));
    let offset =
        vectorcraft_pathops::offset_path(&path, delta, vectorcraft_pathops::Join::Round, 4.0);
    let bez = offset.to_bezpath();
    if bez.elements().is_empty() {
        return Ok(Vec::new());
    }
    Ok(flatten(&bez)?
        .into_iter()
        .filter(|c| c.closed && c.points.len() >= 3)
        .map(|c| c.points)
        .collect())
}

/// Length of a polyline (closing segment included when `closed`).
pub fn polyline_length(points: &[Point], closed: bool) -> f64 {
    let mut len: f64 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
    if closed && let (Some(a), Some(b)) = (points.first(), points.last()) {
        len += a.distance(*b);
    }
    len
}

/// Walks a polyline by arc length: `at(s)` gives the point and unit tangent at distance `s`.
pub(crate) struct Walker<'a> {
    points: &'a [Point],
    closed: bool,
    pub cumulative: Vec<f64>,
}

impl<'a> Walker<'a> {
    pub fn new(points: &'a [Point], closed: bool) -> Self {
        let mut cumulative = Vec::with_capacity(points.len() + 1);
        let mut acc = 0.0;
        cumulative.push(0.0);
        let n = points.len();
        let segs = if closed { n } else { n.saturating_sub(1) };
        for i in 0..segs {
            if let (Some(a), Some(b)) = (points.get(i), points.get((i + 1) % n)) {
                acc += a.distance(*b);
            }
            cumulative.push(acc);
        }
        Self {
            points,
            closed,
            cumulative,
        }
    }

    pub fn length(&self) -> f64 {
        self.cumulative.last().copied().unwrap_or(0.0)
    }

    pub fn at(&self, s: f64) -> Option<(Point, kurbo::Vec2)> {
        let n = self.points.len();
        let s = s.clamp(0.0, self.length());
        // First segment whose end reaches `s`.
        let seg = self
            .cumulative
            .partition_point(|c| *c < s)
            .saturating_sub(1)
            .min(self.cumulative.len().saturating_sub(2));
        let a = *self.points.get(seg)?;
        let b = *self
            .points
            .get(if self.closed { (seg + 1) % n } else { seg + 1 })?;
        let start = *self.cumulative.get(seg)?;
        let len = a.distance(b);
        let t = if len > 0.0 { (s - start) / len } else { 0.0 };
        let dir = if len > 0.0 {
            (b - a) / len
        } else {
            kurbo::Vec2::new(1.0, 0.0)
        };
        Some((a.lerp(b, t.clamp(0.0, 1.0)), dir))
    }
}

/// A small deterministic PRNG (xorshift64*) so seeded generators repeat exactly.
pub(crate) struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self((seed ^ 0x9E37_79B9_7F4A_7C15) | 1)
    }
    /// Uniform in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
pub(crate) mod test_util {
    use super::*;

    pub fn rect(x: f64, y: f64, w: f64, h: f64) -> BezPath {
        kurbo::Rect::new(x, y, x + w, y + h).to_path(0.1)
    }

    pub fn circle(cx: f64, cy: f64, r: f64) -> BezPath {
        kurbo::Circle::new((cx, cy), r).to_path(0.01)
    }

    /// Whether two segments properly cross (touching at endpoints does not count).
    pub fn segments_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
        let o = |p: Point, q: Point, r: Point| (q - p).cross(r - p);
        let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
        d1 * d2 < -1e-12 && d3 * d4 < -1e-12
    }

    /// Whether a closed polyline crosses itself (non-adjacent edges).
    pub fn self_intersects(points: &[Point]) -> bool {
        let n = points.len();
        for i in 0..n {
            for j in i + 2..n {
                if i == 0 && j == n - 1 {
                    continue;
                }
                if segments_cross(
                    points[i],
                    points[(i + 1) % n],
                    points[j],
                    points[(j + 1) % n],
                ) {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::*;
    use super::*;

    #[test]
    fn flatten_closes_and_bounds_hold() {
        let c = flatten(&circle(0.0, 0.0, 10.0)).unwrap();
        assert_eq!(c.len(), 1);
        assert!(c[0].closed);
        for p in &c[0].points {
            assert!((p.to_vec2().hypot() - 10.0).abs() < 0.01);
        }
    }

    #[test]
    fn inside_even_odd_ring() {
        let mut ring = polygons(&circle(0.0, 0.0, 10.0)).unwrap();
        ring.extend(polygons(&circle(0.0, 0.0, 5.0)).unwrap());
        assert!(!inside(&ring, Point::ZERO));
        assert!(inside(&ring, Point::new(7.0, 0.0)));
        assert!(!inside(&ring, Point::new(11.0, 0.0)));
    }

    #[test]
    fn non_finite_and_huge_paths_are_refused() {
        let mut p = BezPath::new();
        p.move_to((f64::NAN, 0.0));
        p.line_to((1.0, 1.0));
        assert!(flatten(&p).is_err());
        assert!(flatten(&rect(0.0, 0.0, 1e9, 1.0)).is_err());
        assert!(polygons(&BezPath::new()).is_err());
    }

    #[test]
    fn walker_measures_square() {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        let w = Walker::new(&pts, true);
        assert!((w.length() - 40.0).abs() < 1e-9);
        let (p, _) = w.at(15.0).unwrap();
        assert!(p.distance(Point::new(10.0, 5.0)) < 1e-9);
        let (p, _) = w.at(35.0).unwrap();
        assert!(p.distance(Point::new(0.0, 5.0)) < 1e-9);
    }

    #[test]
    fn offset_insets_square() {
        let sq = polygons(&rect(0.0, 0.0, 10.0, 10.0)).unwrap();
        let inset = offset_polygons(&sq, -2.0).unwrap();
        let b = crate::clip::bounds(&inset).unwrap();
        assert!(
            (b.x0 - 2.0).abs() < 0.05 && (b.x1 - 8.0).abs() < 0.05,
            "{b:?}"
        );
        assert!(offset_polygons(&sq, -6.0).unwrap().is_empty());
    }
}
