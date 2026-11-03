//! Clipping polylines against even-odd polygon sets, and scanline hatching.

use kurbo::{Affine, Point, Rect};

use crate::{GenError, MAX_ITEMS, MAX_POINTS, Result, inside};

/// Bounds of every ring.
pub fn bounds(polys: &[Vec<Point>]) -> Option<Rect> {
    polys.iter().flatten().fold(None, |acc: Option<Rect>, p| {
        Some(acc.map_or(Rect::from_points(*p, *p), |r| r.union_pt(*p)))
    })
}

/// Parameters `t` in (0, 1) where segment `a`–`b` crosses any polygon edge.
fn crossings(polys: &[Vec<Point>], a: Point, b: Point, out: &mut Vec<f64>) {
    let d = b - a;
    for poly in polys {
        let n = poly.len();
        for i in 0..n {
            let (Some(p), Some(q)) = (poly.get(i), poly.get((i + 1) % n)) else {
                continue;
            };
            let e = *q - *p;
            let den = d.cross(e);
            if den.abs() < 1e-15 {
                continue;
            }
            let w = *p - a;
            let t = w.cross(e) / den;
            let u = w.cross(d) / den;
            if t > 0.0 && t < 1.0 && (0.0..1.0).contains(&u) {
                out.push(t);
            }
        }
    }
}

/// The parts of a polyline inside (`keep_inside`) or outside the polygon set.
pub fn clip_polyline(points: &[Point], polys: &[Vec<Point>], keep_inside: bool) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    let mut ts = Vec::new();
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        ts.clear();
        ts.push(0.0);
        crossings(polys, a, b, &mut ts);
        ts.push(1.0);
        ts.sort_by(f64::total_cmp);
        for pair in ts.windows(2) {
            let (t0, t1) = (pair[0], pair[1]);
            if t1 - t0 < 1e-12 {
                continue;
            }
            let mid = a.lerp(b, (t0 + t1) / 2.0);
            if inside(polys, mid) == keep_inside {
                let (p0, p1) = (a.lerp(b, t0), a.lerp(b, t1));
                if current.last().is_none_or(|l| l.distance(p0) > 1e-9) {
                    if current.len() > 1 {
                        out.push(std::mem::take(&mut current));
                    }
                    current.clear();
                    current.push(p0);
                }
                current.push(p1);
            } else if current.len() > 1 {
                out.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if current.len() > 1 {
        out.push(current);
    }
    out
}

/// Parallel hatch lines at `angle_deg` (0 = horizontal), `spacing` apart, clipped to the
/// polygon set. The first line sits half a spacing inside the bounds. Lines alternate
/// direction so a pen travels little between them.
pub fn hatch(polys: &[Vec<Point>], spacing: f64, angle_deg: f64) -> Result<Vec<Vec<Point>>> {
    let rot = Affine::rotate(angle_deg.to_radians());
    let inv = rot.inverse();
    let local: Vec<Vec<Point>> = polys
        .iter()
        .map(|p| p.iter().map(|q| inv * *q).collect())
        .collect();
    let Some(b) = bounds(&local) else {
        return Ok(Vec::new());
    };
    let rows = (b.height() / spacing).floor().max(1.0);
    if !rows.is_finite() || rows > MAX_ITEMS as f64 {
        return Err(GenError(format!(
            "spacing {spacing} mm would need more than {MAX_ITEMS} lines"
        )));
    }
    let edges: usize = polys.iter().map(Vec::len).sum();
    if rows * edges as f64 > 50.0 * MAX_POINTS as f64 {
        return Err(GenError(
            "the shape is too detailed for this spacing".into(),
        ));
    }
    let start = b.y0 + (b.height() - (rows - 1.0) * spacing) / 2.0;
    let mut out = Vec::new();
    for k in 0..rows as usize {
        let y = start + k as f64 * spacing;
        let line = [Point::new(b.x0 - 1.0, y), Point::new(b.x1 + 1.0, y)];
        let mut pieces = clip_polyline(&line, &local, true);
        if k % 2 == 1 {
            pieces.reverse();
            for p in &mut pieces {
                p.reverse();
            }
        }
        for p in pieces {
            out.push(p.into_iter().map(|q| rot * q).collect());
        }
        if out.len() > MAX_ITEMS {
            return Err(GenError(format!("more than {MAX_ITEMS} hatch lines")));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polygons;
    use crate::test_util::*;

    #[test]
    fn clip_line_through_ring_gives_two_pieces() {
        let mut ring = polygons(&circle(0.0, 0.0, 10.0)).unwrap();
        ring.extend(polygons(&circle(0.0, 0.0, 5.0)).unwrap());
        let pieces = clip_polyline(
            &[Point::new(-20.0, 0.0), Point::new(20.0, 0.0)],
            &ring,
            true,
        );
        assert_eq!(pieces.len(), 2);
        let outside = clip_polyline(
            &[Point::new(-20.0, 0.0), Point::new(20.0, 0.0)],
            &ring,
            false,
        );
        assert_eq!(outside.len(), 3);
    }

    #[test]
    fn hatch_square_counts_and_spacing() {
        let sq = polygons(&rect(0.0, 0.0, 10.0, 10.0)).unwrap();
        let lines = hatch(&sq, 1.0, 0.0).unwrap();
        assert_eq!(lines.len(), 10);
        assert!((lines[0][0].y - 0.5).abs() < 1e-9);
        let ys: Vec<f64> = lines.iter().map(|l| l[0].y).collect();
        for w in ys.windows(2) {
            assert!((w[1] - w[0] - 1.0).abs() < 1e-9);
        }
        // Alternating direction.
        assert!(lines[0][0].x < lines[0][1].x && lines[1][0].x > lines[1][1].x);
        assert!(hatch(&sq, 1e-6, 0.0).is_err());
    }
}
