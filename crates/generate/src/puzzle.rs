//! Jigsaw puzzle generator: interior cut lines that divide a shape into rows × columns of
//! interlocking pieces. Each cell edge carries a round knob on a short neck, pointing into
//! one neighbour or the other (seeded). With `tab_size` at most 0.35 of the smaller cell
//! side, knobs never touch each other or another edge, so the pieces are simple.

use std::f64::consts::PI;

use kurbo::{Arc, BezPath, Point, Vec2};
use serde::{Deserialize, Serialize};

use crate::clip::{bounds, clip_polyline};
use crate::{GenError, MAX_ITEMS, Result, Rng, check_range, flatten, polygons, polylines_path};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PuzzleParams {
    pub rows: u32,
    pub columns: u32,
    /// Knob height as a fraction of the smaller cell side (0–0.35; 0 = straight cuts).
    pub tab_size: f64,
    pub seed: u64,
}

impl Default for PuzzleParams {
    fn default() -> Self {
        Self {
            rows: 4,
            columns: 5,
            tab_size: 0.25,
            seed: 1,
        }
    }
}

/// One cell edge from `a` to `b`, knob of height `h` on the left (`side` = 1) or right.
fn edge(a: Point, b: Point, h: f64, side: f64) -> Vec<Point> {
    let len = a.distance(b);
    if h <= 0.0 || len <= 0.0 {
        return vec![a, b];
    }
    let t = (b - a) / len;
    let n = Vec2::new(-t.y, t.x) * side;
    let r = h / 2.0;
    let neck = 0.6 * r;
    let centre_off = h - r;
    let rise = centre_off - (r * r - neck * neck).sqrt();
    let mid = a.lerp(b, 0.5);
    let at = |along: f64, up: f64| mid + t * along + n * up;
    let mut pts = vec![a, at(-neck, 0.0), at(-neck, rise)];
    // Round the knob the long way: from the left neck through the top to the right neck.
    let c = at(0.0, centre_off);
    let start = (at(-neck, rise) - c).atan2();
    let end = (at(neck, rise) - c).atan2();
    let mut sweep = end - start;
    // Pass through the far side (along +n), whichever winding that needs.
    let far = n.atan2();
    let through = |s: f64| {
        let d = (far - start).rem_euclid(2.0 * PI);
        if s > 0.0 { d < s } else { d - 2.0 * PI > s }
    };
    if !through(sweep) {
        sweep = if sweep > 0.0 {
            sweep - 2.0 * PI
        } else {
            sweep + 2.0 * PI
        };
    }
    let mut arc = BezPath::new();
    arc.move_to(c + Vec2::from_angle(start) * r);
    Arc::new(c, Vec2::new(r, r), start, sweep, 0.0)
        .to_cubic_beziers(1e-4, |p1, p2, p3| arc.curve_to(p1, p2, p3));
    if let Ok(flat) = flatten(&arc)
        && let Some(c) = flat.first()
    {
        pts.extend(c.points.iter().skip(1));
    }
    pts.extend([at(neck, rise), at(neck, 0.0), b]);
    pts
}

/// The cell edges, one polyline each, before clipping (used by tests to check overlaps).
pub fn cell_edges(rect: kurbo::Rect, p: &PuzzleParams) -> Result<Vec<Vec<Point>>> {
    if !(1..=100).contains(&p.rows) || !(1..=100).contains(&p.columns) {
        return Err(GenError(
            "rows and columns must be between 1 and 100".into(),
        ));
    }
    let tab = check_range(p.tab_size, 0.0, 0.35, "tab_size")?;
    let (rows, cols) = (p.rows as usize, p.columns as usize);
    if (rows * cols) > MAX_ITEMS {
        return Err(GenError(format!("more than {MAX_ITEMS} pieces")));
    }
    let (cw, ch) = (rect.width() / cols as f64, rect.height() / rows as f64);
    let h = tab * cw.min(ch);
    let mut rng = Rng::new(p.seed);
    let mut flip = || if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
    let at = |i: usize, j: usize| Point::new(rect.x0 + i as f64 * cw, rect.y0 + j as f64 * ch);
    let mut out = Vec::new();
    for i in 1..cols {
        for j in 0..rows {
            out.push(edge(at(i, j), at(i, j + 1), h, flip()));
        }
    }
    for j in 1..rows {
        for i in 0..cols {
            out.push(edge(at(i, j), at(i + 1, j), h, flip()));
        }
    }
    Ok(out)
}

/// Interior cut lines over `shape`'s bounds, clipped to its inside. The shape's own outline
/// stays the puzzle's border.
pub fn puzzle(shape: &BezPath, p: &PuzzleParams) -> Result<BezPath> {
    let polys = polygons(shape)?;
    let rect = bounds(&polys).ok_or_else(|| GenError("shape has no area".into()))?;
    let edges = cell_edges(rect, p)?;
    let mut lines = Vec::new();
    for e in &edges {
        lines.extend(clip_polyline(e, &polys, true));
    }
    Ok(polylines_path(&lines, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    #[test]
    fn edges_count_and_never_cross() {
        let r = kurbo::Rect::new(0.0, 0.0, 50.0, 40.0);
        let p = PuzzleParams {
            tab_size: 0.35,
            ..PuzzleParams::default()
        };
        let edges = cell_edges(r, &p).unwrap();
        assert_eq!(edges.len(), 4 * 4 + 3 * 5);
        for (i, a) in edges.iter().enumerate() {
            for b in &edges[i + 1..] {
                for s in a.windows(2) {
                    for t in b.windows(2) {
                        assert!(!segments_cross(s[0], s[1], t[0], t[1]), "edges cross");
                    }
                }
            }
            // Knobs stay inside the neighbouring cells.
            for q in a {
                assert!(q.x >= -1e-9 && q.x <= 50.0 + 1e-9 && q.y >= -1e-9 && q.y <= 40.0 + 1e-9);
            }
        }
    }

    #[test]
    fn knob_reaches_its_height() {
        let e = edge(Point::new(0.0, 0.0), Point::new(10.0, 0.0), 3.0, 1.0);
        let top = e.iter().map(|p| p.y).fold(f64::MIN, f64::max);
        assert!((top - 3.0).abs() < 0.01, "{top}");
        let e = edge(Point::new(0.0, 0.0), Point::new(10.0, 0.0), 3.0, -1.0);
        let bottom = e.iter().map(|p| p.y).fold(f64::MAX, f64::min);
        assert!((bottom + 3.0).abs() < 0.01, "{bottom}");
    }

    #[test]
    fn puzzle_clips_to_a_circle() {
        let c = circle(0.0, 0.0, 30.0);
        let out = puzzle(&c, &PuzzleParams::default()).unwrap();
        let polys = polygons(&c).unwrap();
        for l in flatten(&out).unwrap() {
            for w in l.points.windows(2) {
                assert!(crate::inside(&polys, w[0].midpoint(w[1])));
            }
        }
        let rect_out = puzzle(&rect(0.0, 0.0, 50.0, 40.0), &PuzzleParams::default()).unwrap();
        assert_eq!(flatten(&rect_out).unwrap().len(), 31);
        let bad = PuzzleParams {
            tab_size: 0.5,
            ..PuzzleParams::default()
        };
        assert!(puzzle(&c, &bad).is_err());
        let bad = PuzzleParams {
            rows: 0,
            ..PuzzleParams::default()
        };
        assert!(puzzle(&c, &bad).is_err());
    }
}
