//! Line treatments: perforated (dashed) cut lines and wavy lines, applied to any path.

use std::f64::consts::TAU;

use kurbo::{BezPath, Point, Vec2};

use crate::{
    GenError, MAX_ITEMS, MAX_MM, MAX_POINTS, Result, Walker, check_positive, check_range, flatten,
    polyline_path,
};

/// Splits every contour of `path` into dashes `cut_mm` long with `gap_mm` between them.
/// On a closed contour the pitch is stretched slightly so a whole number of dashes fits
/// and the pattern meets itself at the start.
pub fn perforate(path: &BezPath, cut_mm: f64, gap_mm: f64) -> Result<BezPath> {
    let cut = check_positive(cut_mm, MAX_MM, "cut_mm")?;
    let gap = check_positive(gap_mm, MAX_MM, "gap_mm")?;
    let mut out = BezPath::new();
    let mut dashes = 0usize;
    for c in flatten(path)? {
        let walk = Walker::new(&c.points, c.closed);
        let len = walk.length();
        let pitch = cut + gap;
        let n = if c.closed {
            (len / pitch).round().max(1.0)
        } else {
            ((len + gap) / pitch).floor().max(1.0)
        };
        if n > MAX_ITEMS as f64 || dashes + n as usize > MAX_ITEMS {
            return Err(GenError(format!(
                "more than {MAX_ITEMS} dashes; lengthen cut_mm or gap_mm"
            )));
        }
        let scale = if c.closed { len / (n * pitch) } else { 1.0 };
        for k in 0..n as usize {
            let s0 = k as f64 * pitch * scale;
            let s1 = (s0 + cut * scale).min(len);
            dashes += 1;
            polyline_path(&section(&walk, &c.points, s0, s1), false, &mut out);
        }
    }
    Ok(out)
}

/// The polyline between arc lengths `s0` and `s1`, keeping the corners in between.
fn section(walk: &Walker, points: &[Point], s0: f64, s1: f64) -> Vec<Point> {
    let mut pts = Vec::new();
    if let Some((p, _)) = walk.at(s0) {
        pts.push(p);
    }
    let n = points.len();
    for (i, c) in walk.cumulative.iter().enumerate().skip(1) {
        if *c > s0
            && *c < s1
            && let Some(b) = points.get(i % n)
        {
            pts.push(*b);
        }
    }
    if let Some((p, _)) = walk.at(s1) {
        pts.push(p);
    }
    pts
}

/// Replaces every contour with a sine wave `amplitude_mm` either side of it, one period per
/// `wavelength_mm` (adjusted so a closed contour holds a whole number of periods).
pub fn wavy(path: &BezPath, amplitude_mm: f64, wavelength_mm: f64) -> Result<BezPath> {
    let amp = check_range(amplitude_mm, 0.0, MAX_MM, "amplitude_mm")?;
    let wave = check_positive(wavelength_mm, MAX_MM, "wavelength_mm")?;
    let mut out = BezPath::new();
    let mut total = 0usize;
    for c in flatten(path)? {
        let walk = Walker::new(&c.points, c.closed);
        let len = walk.length();
        let periods = if c.closed {
            (len / wave).round().max(1.0)
        } else {
            len / wave
        };
        let period = len / periods;
        let steps = (len / (period / 16.0)).ceil().max(2.0);
        total += steps as usize;
        if !steps.is_finite() || total > MAX_POINTS {
            return Err(GenError(format!(
                "wave too fine (over {MAX_POINTS} points); lengthen wavelength_mm"
            )));
        }
        let count = if c.closed {
            steps as usize
        } else {
            steps as usize + 1
        };
        let mut pts = Vec::with_capacity(count);
        let h = len / steps / 2.0;
        for i in 0..count {
            let s = i as f64 * len / steps;
            let Some((p, _)) = walk.at(s) else { continue };
            // Tangent from neighbouring samples smooths the normal over corners.
            let (Some((a, _)), Some((b, _))) = (
                walk.at(wrap(s - h, len, c.closed)),
                walk.at(wrap(s + h, len, c.closed)),
            ) else {
                continue;
            };
            let t = b - a;
            let t = if t.hypot() > 1e-12 {
                t / t.hypot()
            } else {
                Vec2::new(1.0, 0.0)
            };
            let normal = Vec2::new(-t.y, t.x);
            pts.push(p + normal * (amp * (TAU * s / period).sin()));
        }
        polyline_path(&pts, c.closed, &mut out);
    }
    Ok(out)
}

fn wrap(s: f64, len: f64, closed: bool) -> f64 {
    if closed {
        s.rem_euclid(len)
    } else {
        s.clamp(0.0, len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    #[test]
    fn perforated_square_has_even_dashes() {
        let sq = rect(0.0, 0.0, 10.0, 10.0);
        let out = perforate(&sq, 1.5, 0.5).unwrap();
        let c = flatten(&out).unwrap();
        assert_eq!(c.len(), 20);
        for d in &c {
            assert!(!d.closed);
            let l = crate::polyline_length(&d.points, false);
            assert!((l - 1.5).abs() < 1e-6, "{l}");
        }
        // The dash across a corner keeps the corner point.
        let open = {
            let mut p = BezPath::new();
            p.move_to((0.0, 0.0));
            p.line_to((10.0, 0.0));
            p
        };
        // 0-2, 3-5, 6-8: only whole dashes on an open line.
        assert_eq!(
            flatten(&perforate(&open, 2.0, 1.0).unwrap()).unwrap().len(),
            3
        );
        assert!(perforate(&sq, 0.0, 1.0).is_err());
        assert!(perforate(&sq, 1e-6, 1e-6).is_err());
    }

    #[test]
    fn wavy_circle_stays_closed_within_amplitude() {
        let c = circle(0.0, 0.0, 10.0);
        let out = flatten(&wavy(&c, 0.5, 3.0).unwrap()).unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].closed);
        let r: Vec<f64> = out[0].points.iter().map(|p| p.to_vec2().hypot()).collect();
        let (lo, hi) = r
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
        assert!(
            lo > 9.45 && lo < 9.6 && hi > 10.4 && hi < 10.55,
            "{lo} {hi}"
        );
        assert!(wavy(&c, 0.5, 0.0).is_err());
        assert!(wavy(&c, 0.5, 1e-7).is_err());
    }
}
