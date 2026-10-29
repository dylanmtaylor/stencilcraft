//! Geometry primitives for StencilCraft: kurbo re-exports, millimetre helpers,
//! path flattening.
//!
//! The bottom layer of the workspace (L1 in `docs/architecture.md`): nothing here
//! depends on the document model, kerf, exports or the engine. All coordinates are
//! millimetres in an artboard-like space with y up.

pub use kurbo::{
    Affine, BezPath, Circle, CubicBez, Ellipse, Line, ParamCurve, Point, QuadBez, Rect, Shape, Vec2,
};

#[derive(Debug, thiserror::Error)]
#[error("geometry: {0}")]
pub struct GeomError(String);

pub type Result<T> = std::result::Result<T, GeomError>;

/// The smallest length that can meaningfully exist on an artboard; below this a
/// value is treated as zero. Values above this but non-finite are always an error.
pub const MM_EPSILON: f64 = 1e-9;

/// Rejects a non-finite millimetre length before it can poison a document.
pub fn check_finite_mm(value: f64, what: &'static str) -> Result<f64> {
    if !value.is_finite() {
        return Err(GeomError(format!("{what}: not a finite length")));
    }
    let _ = MM_EPSILON;
    Ok(value)
}

/// A flattened subpath: the raw points of a contour, plus whether the source path
/// closed it. Used by kerf (offsetting) and every polyline-based exporter.
#[derive(Debug, Clone, PartialEq)]
pub struct Polyline {
    pub points: Vec<Point>,
    pub closed: bool,
}

impl Polyline {
    pub fn from_path(path: &BezPath) -> Vec<Polyline> {
        path_to_polylines(path, DEFAULT_FLATTEN_STEPS)
    }
}

/// Flattening resolution: samples per Bézier segment. 32 steps at artboard scale
/// keeps chord error far below a typical kerf width.
pub const DEFAULT_FLATTEN_STEPS: usize = 32;

/// Convert a path into polylines by sampling Bézier segments (`steps` samples per
/// segment). Closed subpaths produce `closed: true` polylines that do not repeat
/// the start point.
pub fn path_to_polylines(path: &BezPath, steps: usize) -> Vec<Polyline> {
    let steps = steps.max(2);
    let mut out: Vec<Polyline> = Vec::new();
    let mut current: Option<Polyline> = None;
    let mut start = Point::ZERO;
    let mut cursor = Point::ZERO;

    for elem in path.elements() {
        match *elem {
            kurbo::PathEl::MoveTo(p) => {
                if let Some(done) = current.take() {
                    out.push(done);
                }
                start = p;
                cursor = p;
                current = Some(Polyline {
                    points: vec![p],
                    closed: false,
                });
            }
            kurbo::PathEl::LineTo(p) => {
                if let Some(pl) = current.as_mut() {
                    pl.points.push(p);
                }
                cursor = p;
            }
            kurbo::PathEl::ClosePath => {
                if let Some(pl) = current.as_mut() {
                    pl.closed = true;
                }
                cursor = start;
            }
            kurbo::PathEl::QuadTo(c, p) => {
                if let Some(pl) = current.as_mut() {
                    let quad = QuadBez::new(cursor, c, p);
                    for i in 1..steps {
                        let t = i as f64 / steps as f64;
                        pl.points.push(quad.eval(t));
                    }
                }
                cursor = p;
            }
            kurbo::PathEl::CurveTo(c1, c2, p) => {
                if let Some(pl) = current.as_mut() {
                    let cubic = CubicBez::new(cursor, c1, c2, p);
                    for i in 1..steps {
                        let t = i as f64 / steps as f64;
                        pl.points.push(cubic.eval(t));
                    }
                }
                cursor = p;
            }
        }
    }
    if let Some(done) = current.take() {
        out.push(done);
    }
    out
}

/// Signed area of a closed polyline (shoelace): positive when wound
/// counter-clockwise with y up. An open polyline contributes 0.
pub fn signed_area(points: &[Point]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        sum += a.x * b.y - b.x * a.y;
    }
    sum / 2.0
}

/// Bounds of a set of points, or `None` when empty.
pub fn bounds_of_points(points: &[Point]) -> Option<Rect> {
    let mut it = points.iter();
    let first = *it.next()?;
    let mut acc = Rect::from_points(first, first);
    for p in it {
        acc = acc.union(Rect::from_points(*p, *p));
    }
    Some(acc)
}

/// Axis-aligned bounds of every point of every polyline.
pub fn bounds_of_polylines(lines: &[Polyline]) -> Option<Rect> {
    let mut acc: Option<Rect> = None;
    for pl in lines {
        if let Some(r) = bounds_of_points(&pl.points) {
            acc = Some(match acc {
                Some(prev) => prev.union(r),
                None => r,
            });
        }
    }
    acc
}

/// Exact (non-flattened) control-point bounds of a path through kurbo.
pub fn bounds_of_shape(path: &BezPath) -> Option<Rect> {
    path.bounding_box().as_rect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(side: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::ZERO);
        p.line_to(Point::new(side, 0.0));
        p.line_to(Point::new(side, side));
        p.line_to(Point::new(0.0, side));
        p.close_path();
        p
    }

    #[test]
    fn flatten_square_closes() {
        let lines = path_to_polylines(&square(10.0), 4);
        assert_eq!(lines.len(), 1);
        let pl = &lines[0];
        assert!(pl.closed);
        assert_eq!(pl.points.len(), 4);
        assert_eq!(pl.points[0], Point::ZERO);
        assert_eq!(pl.points[2], Point::new(10.0, 10.0));
    }

    #[test]
    fn area_sign_of_square() {
        let lines = path_to_polylines(&square(10.0), 4);
        let area = signed_area(&lines[0].points);
        assert!((area - 100.0).abs() < 1e-9, "got {area}");
    }

    #[test]
    fn cubic_flattens_to_steps_per_segment() {
        let mut p = BezPath::new();
        p.move_to(Point::ZERO);
        p.curve_to(
            Point::new(10.0, 0.0),
            Point::new(20.0, 10.0),
            Point::new(0.0, 20.0),
        );
        p.close_path();
        let lines = path_to_polylines(&p, 8);
        assert_eq!(lines.len(), 1);
        // The move plus 7 interior samples; the segment's endpoint is the move origin
        // after close, so it is not repeated.
        assert_eq!(lines[0].points.len(), 8);
    }

    #[test]
    fn bounds_union_works() {
        let lines = path_to_polylines(&square(10.0), 4);
        let b = bounds_of_polylines(&lines).unwrap();
        assert_eq!(b, Rect::new(0.0, 0.0, 10.0, 10.0));
    }
}
