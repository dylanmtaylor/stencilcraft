//! The Knife and Eraser tools, after Silhouette Studio (manual SS-UM-151 ch. 04, observed in
//! 5.0.556): both act on selected shapes and have two modes.
//!
//! - **Solid** treats a closed shape as an area: the cutter (the knife's strip, the eraser's
//!   brush stroke) is subtracted and every separate piece that is left becomes its own shape.
//! - **Outline** treats every shape as lines: the parts of its contours inside the cutter are
//!   removed and the rest become open paths. Curves stay curves: only the segments the cutter
//!   touches are split, at crossings found by bisection on the segment's own parameter.
//!
//! Everything here is pure geometry with bounded work; the engine validates the inputs.

use kurbo::{
    BezPath, Line, ParamCurve, ParamCurveArclen, PathEl, PathSeg, Point, Rect, Shape, Vec2,
};
use vectorcraft_geom::{FillRule, PathData};

/// The thinnest knife strip, so a zero-gap Solid cut still separates the pieces.
pub const MIN_KNIFE_STRIP_MM: f64 = 0.01;
/// More contours than this are not split into pieces (the result stays one compound shape).
const MAX_SPLIT_CONTOURS: usize = 2000;
/// Samples per segment when looking for where it enters or leaves the cutter.
const SAMPLES_PER_SEGMENT: usize = 64;
/// Bisection steps for a crossing: 2^-30 of the segment's parameter.
const BISECT_STEPS: usize = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CutMode {
    Solid,
    Outline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BrushShape {
    Round,
    Square,
}

/// The knife's cut as an area: a strip `width` wide centred on `from`→`to`.
pub fn knife_strip(from: Point, to: Point, width: f64) -> BezPath {
    let width = width.max(MIN_KNIFE_STRIP_MM);
    let run = to - from;
    let len = run.hypot();
    let normal = if len > 0.0 {
        Vec2::new(-run.y, run.x) / len * (width / 2.0)
    } else {
        Vec2::new(0.0, width / 2.0)
    };
    let mut strip = BezPath::new();
    strip.move_to(from + normal);
    strip.line_to(to + normal);
    strip.line_to(to - normal);
    strip.line_to(from - normal);
    strip.close_path();
    strip
}

/// The eraser's stroke as an area: `points` swept by a brush `size` across.
pub fn brush_region(points: &[Point], size: f64, brush: BrushShape) -> BezPath {
    let half = size / 2.0;
    let dab = |p: Point| match brush {
        BrushShape::Round => kurbo::Circle::new(p, half).to_path(0.01),
        BrushShape::Square => Rect::from_center_size(p, (size, size)).to_path(0.01),
    };
    match points {
        [] => BezPath::new(),
        [only] => dab(*only),
        [first, rest @ ..] => {
            let mut line = BezPath::new();
            line.move_to(*first);
            for p in rest {
                line.line_to(*p);
            }
            let (cap, join) = match brush {
                BrushShape::Round => (
                    vectorcraft_pathops::Cap::Round,
                    vectorcraft_pathops::Join::Round,
                ),
                BrushShape::Square => (
                    vectorcraft_pathops::Cap::Square,
                    vectorcraft_pathops::Join::Miter,
                ),
            };
            vectorcraft_pathops::outline_stroke(
                &PathData::from_bezpath(&line),
                size,
                cap,
                join,
                4.0,
            )
            .to_bezpath()
        }
    }
}

/// Whether `path` is made only of closed contours (an area Solid mode can cut).
pub fn is_closed_area(path: &BezPath) -> bool {
    let mut open = false;
    let mut any = false;
    for el in path.elements() {
        match el {
            PathEl::MoveTo(_) => {
                if open {
                    return false;
                }
                open = true;
                any = true;
            }
            PathEl::ClosePath => open = false,
            _ => {}
        }
    }
    any && !open
}

/// Solid mode: `area` minus `cutter`, split into its separate pieces. An empty result means the
/// cutter covered the whole shape.
pub fn solid_cut(area: &BezPath, cutter: &BezPath) -> Result<Vec<BezPath>, String> {
    let result = vectorcraft_pathops::try_boolean(
        &PathData::from_bezpath(area),
        FillRule::EvenOdd,
        &PathData::from_bezpath(cutter),
        FillRule::NonZero,
        vectorcraft_pathops::BoolOp::Difference,
        0.001,
    )
    .map_err(|e| e.to_string())?
    .to_bezpath();
    Ok(split_components(&result))
}

/// The separate pieces of an area: each outer contour with the holes directly inside it.
pub fn split_components(path: &BezPath) -> Vec<BezPath> {
    let contours = contours(path);
    if contours.len() <= 1 || contours.len() > MAX_SPLIT_CONTOURS {
        return if path.elements().is_empty() {
            Vec::new()
        } else {
            vec![path.clone()]
        };
    }
    // A point on each contour: contours of a boolean result never cross, so it is strictly
    // inside or outside every other contour.
    let probe: Vec<Option<Point>> = contours
        .iter()
        .map(|c| c.segments().next().map(|s| s.eval(0.5)))
        .collect();
    let contains = |outer: usize, inner: usize| -> bool {
        match (contours.get(outer), probe.get(inner).copied().flatten()) {
            (Some(c), Some(p)) => outer != inner && c.winding(p) != 0,
            _ => false,
        }
    };
    let n = contours.len();
    let depth: Vec<usize> = (0..n)
        .map(|i| (0..n).filter(|&j| contains(j, i)).count())
        .collect();
    let mut pieces: Vec<(usize, BezPath)> = contours
        .iter()
        .zip(&depth)
        .enumerate()
        .filter(|(_, (_, d))| *d % 2 == 0)
        .map(|(i, (c, _))| (i, c.clone()))
        .collect();
    for (hole, hole_depth) in depth.iter().enumerate().filter(|(_, d)| *d % 2 == 1) {
        // Its piece is the containing outer contour one level up.
        let parent = depth
            .iter()
            .enumerate()
            .find(|&(j, d)| d + 1 == *hole_depth && contains(j, hole))
            .map(|(j, _)| j);
        if let (Some(parent), Some(h)) = (parent, contours.get(hole))
            && let Some((_, piece)) = pieces.iter_mut().find(|(i, _)| *i == parent)
        {
            piece.extend(h.elements().iter().copied());
        }
    }
    pieces.into_iter().map(|(_, p)| p).collect()
}

/// Outline mode: the parts of `path`'s contours outside `cutter`, as paths. Contours the cutter
/// misses are kept as they are (closed ones stay closed); cut ones become open pieces.
pub fn outline_cut(path: &BezPath, cutter: &BezPath) -> Vec<BezPath> {
    let inside = |p: Point| cutter.winding(p) != 0;
    let bounds = cutter.bounding_box();
    let mut pieces = Vec::new();
    for contour in contours(path) {
        let closed = contour.elements().last() == Some(&PathEl::ClosePath);
        let segments: Vec<PathSeg> = contour.segments().collect();
        // Runs of the contour outside the cutter, as lists of (sub)segments.
        let mut runs: Vec<Vec<PathSeg>> = Vec::new();
        let mut current: Vec<PathSeg> = Vec::new();
        let mut touched = false;
        for seg in &segments {
            if !seg.bounding_box().overlaps(bounds) && !seg.bounding_box().is_zero_area() {
                current.push(*seg);
                continue;
            }
            for (t0, t1, outside) in intervals(seg, &inside) {
                if outside {
                    current.push(seg.subsegment(t0..t1));
                } else {
                    touched = true;
                    if !current.is_empty() {
                        runs.push(std::mem::take(&mut current));
                    }
                }
            }
        }
        if !touched {
            pieces.push(contour);
            continue;
        }
        if !current.is_empty() {
            if closed && let Some(first) = runs.first_mut() {
                // The run through the closing point continues into the first one.
                current.append(first);
                *first = current;
            } else {
                runs.push(current);
            }
        }
        pieces.extend(
            runs.iter()
                .filter(|r| length(r) > 1e-6)
                .map(|r| open_path(r)),
        );
    }
    pieces
}

/// Outline mode for the knife without a gap: split the contours exactly where `from`→`to`
/// crosses them.
pub fn knife_split(path: &BezPath, from: Point, to: Point) -> Vec<BezPath> {
    let line = Line::new(from, to);
    let mut pieces = Vec::new();
    for contour in contours(path) {
        let closed = contour.elements().last() == Some(&PathEl::ClosePath);
        let mut runs: Vec<Vec<PathSeg>> = Vec::new();
        let mut current: Vec<PathSeg> = Vec::new();
        let mut cut = false;
        for seg in contour.segments() {
            let mut ts: Vec<f64> = seg
                .intersect_line(line)
                .iter()
                .filter(|hit| (0.0..=1.0).contains(&hit.line_t))
                .map(|hit| hit.segment_t)
                // A crossing at a segment's start is the previous segment's end: count it there.
                .filter(|t| t.is_finite() && *t > 1e-9 && *t <= 1.0)
                .map(|t| if t > 1.0 - 1e-9 { 1.0 } else { t })
                .collect();
            ts.sort_by(f64::total_cmp);
            ts.dedup();
            let mut start = 0.0;
            for t in ts {
                if t > start {
                    current.push(seg.subsegment(start..t));
                }
                runs.push(std::mem::take(&mut current));
                start = t;
                cut = true;
            }
            if start < 1.0 {
                current.push(seg.subsegment(start..1.0));
            }
        }
        if !cut {
            pieces.push(contour);
            continue;
        }
        if closed && let Some(first) = runs.first_mut() {
            current.append(first);
            *first = current;
        } else if !current.is_empty() {
            runs.push(current);
        }
        pieces.extend(
            runs.iter()
                .filter(|r| length(r) > 1e-6)
                .map(|r| open_path(r)),
        );
    }
    pieces
}

/// Each contour of `path` as its own path.
fn contours(path: &BezPath) -> Vec<BezPath> {
    let mut out: Vec<BezPath> = Vec::new();
    for el in path.elements() {
        if matches!(el, PathEl::MoveTo(_)) || out.is_empty() {
            out.push(BezPath::new());
        }
        if let Some(last) = out.last_mut() {
            last.push(*el);
        }
    }
    out.retain(|c| c.segments().next().is_some());
    out
}

/// `seg` split into intervals of its parameter that are wholly inside or outside the cutter.
fn intervals(seg: &PathSeg, inside: &impl Fn(Point) -> bool) -> Vec<(f64, f64, bool)> {
    let at = |t: f64| inside(seg.eval(t));
    let mut out = Vec::new();
    let mut start = 0.0;
    let mut state = at(0.0);
    for i in 1..=SAMPLES_PER_SEGMENT {
        let t = i as f64 / SAMPLES_PER_SEGMENT as f64;
        let here = at(t);
        if here != state {
            // Bisect the crossing between the previous sample and this one.
            let (mut lo, mut hi) = ((i - 1) as f64 / SAMPLES_PER_SEGMENT as f64, t);
            for _ in 0..BISECT_STEPS {
                let mid = (lo + hi) / 2.0;
                if at(mid) == state {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let cross = (lo + hi) / 2.0;
            if cross > start {
                out.push((start, cross, !state));
            }
            start = cross;
            state = here;
        }
    }
    if start < 1.0 {
        out.push((start, 1.0, !state));
    }
    out
}

fn length(run: &[PathSeg]) -> f64 {
    run.iter().map(|s| s.arclen(1e-6)).sum()
}

fn open_path(run: &[PathSeg]) -> BezPath {
    let mut path = BezPath::new();
    if let Some(first) = run.first() {
        path.move_to(first.start());
    }
    for seg in run {
        match seg {
            PathSeg::Line(l) => path.line_to(l.p1),
            PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
            PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
        Rect::new(x0, y0, x1, y1).to_path(0.01)
    }

    #[test]
    fn a_solid_knife_cut_across_a_square_leaves_two_closed_pieces() {
        let square = rect(0.0, 0.0, 10.0, 10.0);
        let strip = knife_strip(Point::new(-1.0, -1.0), Point::new(11.0, 11.0), 0.0);
        let pieces = solid_cut(&square, &strip).expect("cut");
        assert_eq!(pieces.len(), 2, "{pieces:?}");
        for piece in &pieces {
            assert!(is_closed_area(piece));
            assert!((piece.area().abs() - 50.0).abs() < 0.2, "{}", piece.area());
        }
    }

    #[test]
    fn a_solid_cut_keeps_holes_with_their_piece() {
        // A frame (square with a square hole) cut down the middle on the left side only.
        let mut frame = rect(0.0, 0.0, 30.0, 30.0);
        frame.extend(rect(10.0, 10.0, 20.0, 20.0).elements().iter().copied());
        let pieces = split_components(&frame);
        assert_eq!(pieces.len(), 1, "a hole is not a piece of its own");
        let strip = knife_strip(Point::new(5.0, -1.0), Point::new(5.0, 31.0), 1.0);
        let pieces = solid_cut(&frame, &strip).expect("cut");
        assert_eq!(pieces.len(), 2);
        let total: f64 = pieces.iter().map(|p| p.area().abs()).sum();
        assert!((total - (900.0 - 100.0 - 30.0)).abs() < 0.5, "{total}");
    }

    #[test]
    fn the_outline_knife_splits_curves_exactly_where_the_line_crosses() {
        let circle = kurbo::Circle::new((0.0, 0.0), 10.0).to_path(0.01);
        let pieces = knife_split(&circle, Point::new(-20.0, 0.0), Point::new(20.0, 0.0));
        assert_eq!(pieces.len(), 2);
        for piece in &pieces {
            assert!(!is_closed_area(piece), "outline pieces are open");
            let ends: Vec<Point> = [piece.segments().next(), piece.segments().last()]
                .into_iter()
                .flatten()
                .map(|s| s.eval(0.0))
                .collect();
            assert!(
                ends.iter()
                    .all(|p| (p.to_vec2().hypot() - 10.0).abs() < 1e-6)
            );
            assert!(
                piece.segments().all(|s| matches!(s, PathSeg::Cubic(_))),
                "still curves"
            );
        }
        // A line that misses keeps the shape whole and closed.
        let kept = knife_split(&circle, Point::new(-20.0, 30.0), Point::new(20.0, 30.0));
        assert_eq!(kept.len(), 1);
        assert!(is_closed_area(&kept[0]));
        // The cut is a segment, not an infinite line: stopping short cuts nothing.
        let short = knife_split(&circle, Point::new(-5.0, 0.0), Point::new(5.0, 0.0));
        assert_eq!(short.len(), 1);
    }

    #[test]
    fn the_outline_eraser_removes_only_what_the_brush_covers() {
        let square = rect(0.0, 0.0, 10.0, 10.0);
        // A round brush 2 mm across, dragged over the top edge's middle.
        let region = brush_region(
            &[Point::new(5.0, -3.0), Point::new(5.0, 3.0)],
            2.0,
            BrushShape::Round,
        );
        let pieces = outline_cut(&square, &region);
        assert_eq!(
            pieces.len(),
            1,
            "one gap opens the square into one open line"
        );
        let piece = &pieces[0];
        assert!(!is_closed_area(piece));
        let total: f64 = piece.segments().map(|s| s.arclen(1e-6)).sum();
        assert!((total - 38.0).abs() < 0.01, "{total}");
    }

    #[test]
    fn a_solid_eraser_stroke_across_a_rectangle_leaves_two_rectangles() {
        let r = rect(0.0, 0.0, 20.0, 10.0);
        let region = brush_region(
            &[Point::new(10.0, -5.0), Point::new(10.0, 15.0)],
            2.0,
            BrushShape::Square,
        );
        let pieces = solid_cut(&r, &region).expect("erase");
        assert_eq!(pieces.len(), 2);
        let total: f64 = pieces.iter().map(|p| p.area().abs()).sum();
        assert!((total - 180.0).abs() < 0.1, "{total}");
        // A dab that covers everything leaves nothing.
        let big = brush_region(&[Point::new(10.0, 5.0)], 100.0, BrushShape::Square);
        assert!(solid_cut(&r, &big).expect("erase").is_empty());
    }

    #[test]
    fn degenerate_input_is_harmless() {
        assert!(
            brush_region(&[], 2.0, BrushShape::Round)
                .elements()
                .is_empty()
        );
        // A knife from a point to itself (the engine refuses it) is a harmless empty strip.
        let strip = knife_strip(Point::new(1.0, 1.0), Point::new(1.0, 1.0), 0.0);
        assert!(strip.area().abs() < 1e-9);
        assert!(split_components(&BezPath::new()).is_empty());
        assert!(outline_cut(&BezPath::new(), &strip).is_empty());
        assert!(!is_closed_area(&BezPath::new()));
    }
}
