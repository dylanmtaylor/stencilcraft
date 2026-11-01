//! Clip flattened geometry to a sheet rectangle.
//!
//! Tiling hands each sheet only the part of a shape that lies on it, so a cut path
//! never runs off the edge of the material. Cutting follows contours, not filled
//! regions: geometry that leaves the sheet stops at the edge and reappears on the
//! neighbouring sheet, which shares an overlap strip. No synthetic cut is added
//! along the sheet edge, because that would show up as a seam in the assembled work.
//! Segments clip with Liang–Barsky.

use stencilcraft_geom::{BezPath, DEFAULT_FLATTEN_STEPS, Point, Rect, path_to_polylines};

/// Two vertices closer than this are the same point (millimetres).
const MERGE: f64 = 1e-9;

/// Clip `path` to `rect`. Each contour becomes the runs of itself that lie inside
/// the rectangle; a contour that leaves the sheet comes back as open runs ending on
/// the edge. Returns an empty path when nothing of the input lies inside.
pub fn clip_path_to_rect(path: &BezPath, rect: Rect) -> BezPath {
    let mut out = BezPath::new();
    for line in path_to_polylines(path, DEFAULT_FLATTEN_STEPS) {
        if line.points.len() < 2 {
            continue;
        }
        // Walk a closed contour cyclically by repeating its first point, so a run
        // crossing the contour's start vertex is clipped like any other.
        let mut points = line.points.clone();
        if line.closed {
            let first = points[0];
            let last = points[points.len() - 1];
            if !same_point(first, last) {
                points.push(first);
            }
        }
        for run in clip_open_polyline(&points, rect) {
            let Some((first, rest)) = run.split_first() else {
                continue;
            };
            out.move_to(*first);
            for p in rest {
                out.line_to(*p);
            }
        }
    }
    out
}

fn same_point(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() <= MERGE && (a.y - b.y).abs() <= MERGE
}

/// Clip one segment to the rectangle, returning the visible piece when there is
/// one (Liang–Barsky).
fn clip_segment(a: Point, b: Point, rect: Rect) -> Option<(Point, Point)> {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    // Edge tests as (direction, distance): p·t ≤ q stays inside.
    let edges = [
        (-dx, a.x - rect.x0),
        (dx, rect.x1 - a.x),
        (-dy, a.y - rect.y0),
        (dy, rect.y1 - a.y),
    ];
    let mut t0 = 0.0_f64;
    let mut t1 = 1.0_f64;
    for (denominator, distance) in edges {
        if denominator == 0.0 {
            if distance < 0.0 {
                return None;
            }
            continue;
        }
        let t = distance / denominator;
        if denominator < 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return None;
        }
    }
    let point_at = |t: f64| Point::new(a.x + t * dx, a.y + t * dy);
    Some((point_at(t0), point_at(t1)))
}

/// Clip an open polyline into the runs that lie inside the rectangle.
fn clip_open_polyline(points: &[Point], rect: Rect) -> Vec<Vec<Point>> {
    let mut runs: Vec<Vec<Point>> = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    for pair in points.windows(2) {
        match clip_segment(pair[0], pair[1], rect) {
            Some((a, b)) => {
                match current.last() {
                    Some(last) if !same_point(*last, a) => {
                        let finished = std::mem::take(&mut current);
                        runs.push(finished);
                        current.push(a);
                    }
                    Some(_) => {}
                    None => current.push(a),
                }
                current.push(b);
            }
            None => {
                let finished = std::mem::take(&mut current);
                if finished.len() >= 2 {
                    runs.push(finished);
                }
            }
        }
    }
    if current.len() >= 2 {
        runs.push(current);
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_geom::bounds_of_points;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect::new(x0, y0, x1, y1)
    }

    fn square(x0: f64, y0: f64, size: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(x0, y0));
        p.line_to(Point::new(x0 + size, y0));
        p.line_to(Point::new(x0 + size, y0 + size));
        p.line_to(Point::new(x0, y0 + size));
        p.close_path();
        p
    }

    #[test]
    fn shape_inside_the_sheet_is_untouched() {
        let clipped = clip_path_to_rect(&square(10.0, 10.0, 20.0), rect(0.0, 0.0, 100.0, 100.0));
        let polys = path_to_polylines(&clipped, 4);
        assert_eq!(polys.len(), 1);
        let b = bounds_of_points(&polys[0].points).unwrap();
        assert!(
            (b.x0 - 10.0).abs() < 1e-9 && (b.x1 - 30.0).abs() < 1e-9,
            "{b:?}"
        );
    }

    #[test]
    fn closed_shape_is_cut_back_to_the_sheet_edge() {
        // 30 of the square's 40 mm sit past the sheet's right edge: only the part
        // inside the sheet is cut, and it ends where the material ends.
        let clipped = clip_path_to_rect(&square(90.0, 10.0, 40.0), rect(0.0, 0.0, 100.0, 100.0));
        let polys = path_to_polylines(&clipped, 4);
        assert!(!polys.is_empty(), "the on-sheet part survives");
        let mut acc: Option<Rect> = None;
        for run in &polys {
            assert!(!run.closed, "an edge-crossing cut is not closed: {run:?}");
            let b = bounds_of_points(&run.points).unwrap();
            assert!(
                b.x1 <= 100.0 + 1e-9 && b.x0 >= -1e-9,
                "off the sheet: {b:?}"
            );
            acc = Some(match acc {
                Some(a) => a.union(b),
                None => b,
            });
        }
        let all = acc.expect("runs were found");
        assert!(
            (all.x0 - 90.0).abs() < 1e-9
                && (all.x1 - 100.0).abs() < 1e-9
                && (all.y0 - 10.0).abs() < 1e-9
                && (all.y1 - 50.0).abs() < 1e-9,
            "on-sheet extent: {all:?}"
        );
    }

    #[test]
    fn shape_outside_the_sheet_disappears() {
        let clipped = clip_path_to_rect(&square(200.0, 200.0, 10.0), rect(0.0, 0.0, 100.0, 100.0));
        assert!(clipped.elements().is_empty());
    }

    #[test]
    fn open_line_keeps_only_the_part_on_the_sheet() {
        let mut line = BezPath::new();
        line.move_to(Point::new(-50.0, 20.0));
        line.line_to(Point::new(150.0, 20.0));
        let clipped = clip_path_to_rect(&line, rect(0.0, 0.0, 100.0, 100.0));
        let polys = path_to_polylines(&clipped, 4);
        assert_eq!(polys.len(), 1);
        assert!(!polys[0].closed);
        let b = bounds_of_points(&polys[0].points).unwrap();
        assert!(
            (b.x0 - 0.0).abs() < 1e-9 && (b.x1 - 100.0).abs() < 1e-9,
            "{b:?}"
        );
    }

    #[test]
    fn open_line_leaving_and_returning_makes_two_runs() {
        let mut path = BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(100.0, 100.0));
        path.line_to(Point::new(10.0, 20.0));
        // The sheet only covers the lower-left corner, so the line enters, leaves and
        // re-enters.
        let clipped = clip_path_to_rect(&path, rect(0.0, 0.0, 40.0, 40.0));
        let polys = path_to_polylines(&clipped, 4);
        assert_eq!(polys.len(), 2, "runs: {polys:?}");
        for run in &polys {
            let b = bounds_of_points(&run.points).unwrap();
            assert!(b.x0 >= -1e-9 && b.x1 <= 40.0 + 1e-9, "{b:?}");
        }
    }
}
