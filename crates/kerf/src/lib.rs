//! Kerf compensation: offset every closed cut contour so the cut part lands at its
//! drawn size. A laser or blade removes a strip `kerf` wide; the centreline that
//! would sit on the drawn outline must therefore move outward by `kerf / 2` on
//! outer contours and inward by `kerf / 2` on counters.
//!
//! Robustness limits (see `docs/gaps.md` 2): offsetting works on simple (non-self-
//! intersecting) contours and is exact for convex ones; a join that would need a
//! miter longer than the limit is bevelled instead of spiked. Counters are
//! detected by point containment against the shape's other contours, not full
//! winding algebra yet.

use kurbo::Point;
use stencilcraft_doc::{CutStyle, Document};
use stencilcraft_geom::{Polyline, signed_area};

#[derive(Debug, thiserror::Error)]
pub enum KerfError {
    #[error("kerf: {0}")]
    Invalid(String),
    #[error("kerf: {0}")]
    Model(String),
}

/// Offset one closed polygon by `delta` (positive = outward, no matter how the
/// contour is wound). Returns the offset polygon with one vertex per input vertex.
/// Corners are miter-joined, clamped to `miter_limit` × `|delta|` and bevelled
/// beyond that. Degenerate edges (zero length) use either neighbour's normal.
pub fn offset_closed_polygon(points: &[Point], delta: f64, miter_limit: f64) -> Vec<Point> {
    let n = points.len();
    if n < 3 || delta.abs() < 1e-12 {
        return points.to_vec();
    }
    let limit = miter_limit.abs().max(1.0) * delta.abs();

    // Outward unit normal per edge. For CCW winding (positive signed area, y up)
    // the outward normal of edge e is (e.y, −e.x); CW winding flips it.
    let face = if signed_area(points) > 0.0 { 1.0 } else { -1.0 };
    let normals: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let a = points[i];
            let b = points[(i + 1) % n];
            let ex = b.x - a.x;
            let ey = b.y - a.y;
            let len = (ex * ex + ey * ey).sqrt();
            if len < 1e-12 {
                (0.0, 0.0)
            } else {
                (face * ey / len, -face * ex / len)
            }
        })
        .collect();

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        // The join of edges (i−1 → i) and (i → i+1) sits at vertex i; offset along
        // the miter direction between the two adjacent edge normals.
        let n_prev = normals[(i + n - 1) % n];
        let n_cur = normals[i];
        if n_cur.0 == 0.0 && n_cur.1 == 0.0 {
            out.push(points[i]);
            continue;
        }
        let sum = (n_prev.0 + n_cur.0, n_prev.1 + n_cur.1);
        let sum_len2 = sum.0 * sum.0 + sum.1 * sum.1;
        if sum_len2 < 1e-18 {
            // The adjacent normals cancel: no clean miter exists, so push straight
            // out along the current edge's normal.
            out.push(Point::new(
                points[i].x + delta * n_cur.0,
                points[i].y + delta * n_cur.1,
            ));
            continue;
        }
        // cos(θ/2) of the corner: the dot of the miter direction with either normal.
        let sum_len = sum_len2.sqrt();
        let cos_half = (sum.0 * n_cur.0 + sum.1 * n_cur.1) / sum_len;
        let miter_dir = (
            (sum.0 / sum_len) * delta / cos_half.max(1e-9),
            (sum.1 / sum_len) * delta / cos_half.max(1e-9),
        );
        if delta.abs() / cos_half.max(1e-9) > limit {
            // Beyond the miter limit: bevel, pushing halfway between the two
            // adjacent edge offsets.
            let dx = (n_prev.0 + n_cur.0) * 0.5 * delta;
            let dy = (n_prev.1 + n_cur.1) * 0.5 * delta;
            out.push(Point::new(points[i].x + dx, points[i].y + dy));
        } else {
            out.push(Point::new(
                points[i].x + miter_dir.0,
                points[i].y + miter_dir.1,
            ));
        }
    }
    out
}

/// Apply kerf compensation to a document: every closed cut contour moves by
/// `kerf / 2` — outward on outer contours, inward when all of a contour's vertices
/// lie inside another contour of the same shape (a counter, e.g. material that
/// must fall out). Engrave shapes and open contours are left unchanged: tool
/// compensation for them is a machine-side setting. Returns a new document; the
/// input document is never mutated.
pub fn apply(doc: &Document, miter_limit: f64) -> Result<Document, KerfError> {
    if !doc.kerf.width_mm.is_finite() || doc.kerf.width_mm < 0.0 {
        return Err(KerfError::Invalid(format!(
            "kerf width {} mm is not a valid non-negative width",
            doc.kerf.width_mm
        )));
    }
    let delta = doc.kerf.width_mm / 2.0;
    let mut out = doc.clone();
    for layer in &mut out.layers {
        for shape in &mut layer.shapes {
            if shape.style != CutStyle::Cut {
                continue;
            }
            shape.path = offset_shape_path(&shape.path, delta, miter_limit)?;
        }
    }
    Ok(out)
}

/// Offsets every closed subpath of one path by `delta`, deciding the side via
/// containment against the shape's other subpaths.
fn offset_shape_path(
    path: &kurbo::BezPath,
    delta: f64,
    miter_limit: f64,
) -> Result<kurbo::BezPath, KerfError> {
    let lines = Polyline::from_path(path);
    let mut moved: Vec<Polyline> = Vec::with_capacity(lines.len());
    for (i, pl) in lines.iter().enumerate() {
        if !pl.closed || pl.points.len() < 3 {
            moved.push(pl.clone());
            continue;
        }
        let inside = lines.iter().enumerate().any(|(j, other)| {
            i != j && other.closed && other.points.len() >= 3 && is_inside(pl, other)
        });
        let signed = if inside { -delta } else { delta };
        moved.push(Polyline {
            points: offset_closed_polygon(&pl.points, signed, miter_limit),
            closed: true,
        });
    }
    // Rebuild as a polyline path: cut machines follow straight steps anyway, so
    // the flattened outline is the honest representation post-compensation.
    let mut out = kurbo::BezPath::new();
    for pl in &moved {
        let Some(first) = pl.points.first() else {
            continue;
        };
        out.move_to(*first);
        for p in pl.points.iter().skip(1) {
            out.line_to(*p);
        }
        if pl.closed {
            out.close_path();
        }
    }
    if out.elements().is_empty() && !path.elements().is_empty() {
        return Err(KerfError::Model(
            "kerf: path had no plottable contours".to_string(),
        ));
    }
    Ok(out)
}

/// Are all the vertices of `pl` inside `ring` (a closed contour)?
fn is_inside(pl: &Polyline, ring: &Polyline) -> bool {
    ring.closed && pl.points.iter().all(|p| in_ring(p, &ring.points))
}

/// Point-in-polygon (even-odd ray cast). Vertices and boundary points count as
/// inside; rings with fewer than 3 points report outside.
fn in_ring(p: &Point, ring: &[Point]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        if (a.y > p.y) != (b.y > p.y) {
            let x_at = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if p.x < x_at {
                inside = !inside;
            }
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_doc::{Board, KerfSettings, Layer};
    use stencilcraft_geom::bounds_of_points;

    fn square(size: f64) -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(size, 0.0),
            Point::new(size, size),
            Point::new(0.0, size),
        ]
    }

    fn ccw_path(size: f64) -> kurbo::BezPath {
        let mut p = kurbo::BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(size, 0.0));
        p.line_to(Point::new(size, size));
        p.line_to(Point::new(0.0, size));
        p.close_path();
        p
    }

    #[test]
    fn expand_square_keeps_count_and_grows() {
        let before = square(10.0);
        let after = offset_closed_polygon(&before, 0.5, 4.0);
        assert_eq!(after.len(), 4);
        let b0 = bounds_of_points(&before).unwrap();
        let b1 = bounds_of_points(&after).unwrap();
        assert!(
            (b1.width() - (b0.width() + 1.0)).abs() < 1e-9,
            "{b0:?} → {b1:?}"
        );
        assert!((b1.height() - (b0.height() + 1.0)).abs() < 1e-9);
    }

    #[test]
    fn shrink_square_shrinks() {
        let after = offset_closed_polygon(&square(10.0), -0.5, 4.0);
        let b1 = bounds_of_points(&after).unwrap();
        assert!((b1.width() - 9.0).abs() < 1e-9, "{b1:?}");
    }

    #[test]
    fn grows_regardless_of_winding() {
        let mut winding_flipped = square(10.0);
        winding_flipped.reverse();
        let after = offset_closed_polygon(&winding_flipped, 0.5, 4.0);
        let b1 = bounds_of_points(&after).unwrap();
        assert!((b1.width() - 11.0).abs() < 1e-9, "{b1:?}");
    }

    #[test]
    fn miter_spike_is_bevelled() {
        // A needle-thin wedge would need an enormous miter; the clamp must hold.
        let wedge = vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 0.001),
        ];
        let after = offset_closed_polygon(&wedge, 1.0, 4.0);
        let b = bounds_of_points(&after).unwrap();
        assert!(b.width() < 16.0, "miter blew up: {b:?}");
    }

    #[test]
    fn near_degenerate_edge_tolerated() {
        let mut pts = square(10.0);
        pts.insert(1, Point::new(3.0, 0.0));
        pts.insert(2, Point::new(3.0 + 1e-10, 0.0));
        let after = offset_closed_polygon(&pts, 0.25, 4.0);
        assert_eq!(after.len(), pts.len());
    }

    #[test]
    fn document_apply_grows_every_shape() {
        let mut doc = Document {
            board: Board::default(),
            kerf: KerfSettings {
                width_mm: 0.2,
                apply_to_exports: false,
            },
            layers: vec![Layer::new("cut", CutStyle::Cut)],
            next_shape_id: 1,
        };
        doc.push_shape(0, ccw_path(50.0), CutStyle::Cut).unwrap();
        let out = apply(&doc, 4.0).unwrap();
        let before = doc.layers[0].shapes[0].bounds().unwrap();
        let after = out.layers[0].shapes[0].bounds().unwrap();
        assert!((after.width() - before.width() - 0.2).abs() < 1e-6);
        assert!((after.height() - before.height() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn engrave_shapes_untouched() {
        let mut doc = Document::default();
        doc.push_shape(0, ccw_path(50.0), CutStyle::Engrave)
            .unwrap();
        let original = doc.layers[0].shapes[0].path.clone();
        let out = apply(&doc, 4.0).unwrap();
        assert_eq!(out.layers[0].shapes[0].path, original);
    }

    #[test]
    fn nested_counter_moves_inward_while_outer_contour_expands() {
        let mut doc = Document {
            board: Board {
                width_mm: 120.0,
                height_mm: 120.0,
            },
            kerf: KerfSettings {
                width_mm: 0.4,
                apply_to_exports: true,
            },
            layers: vec![Layer::new("cut", CutStyle::Cut)],
            next_shape_id: 1,
        };
        doc.push_shape(0, ccw_path(100.0), CutStyle::Cut).unwrap();
        let mut counter = kurbo::BezPath::new();
        counter.move_to(Point::new(30.0, 30.0));
        counter.line_to(Point::new(70.0, 30.0));
        counter.line_to(Point::new(70.0, 70.0));
        counter.line_to(Point::new(30.0, 70.0));
        counter.close_path();
        // Both contours are deliberately one shape; they are subpaths, and the
        // inner one must move into the scrap rather than enlarge the hole.
        let mut compound = ccw_path(100.0);
        compound.extend(counter.elements().iter().copied());
        doc.layers[0].shapes[0].path = compound;
        let compensated = apply(&doc, 4.0).unwrap();
        let polylines = Polyline::from_path(&compensated.layers[0].shapes[0].path);
        assert_eq!(polylines.len(), 2);
        let outer = bounds_of_points(&polylines[0].points).unwrap();
        let inner = bounds_of_points(&polylines[1].points).unwrap();
        assert!((outer.width() - 100.4).abs() < 1e-6, "outer: {outer:?}");
        assert!((inner.width() - 39.6).abs() < 1e-6, "counter: {inner:?}");
    }

    #[test]
    fn negative_kerf_is_rejected() {
        let mut doc = Document::default();
        doc.kerf.width_mm = -1.0;
        assert!(matches!(apply(&doc, 4.0), Err(KerfError::Invalid(_))));
    }
}
