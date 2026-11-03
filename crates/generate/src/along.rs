//! Placing things along a path: "object on path" copy transforms and leather punch points.

use kurbo::{Affine, BezPath, Point};
use serde::{Deserialize, Serialize};

use crate::{GenError, MAX_ITEMS, MAX_MM, Result, Walker, check_positive, check_range, flatten};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AlongParams {
    /// Rotate each copy to the path's tangent.
    pub perpendicular: bool,
    /// Rotation added to every copy, degrees.
    pub start_angle_deg: f64,
    /// Extra rotation added per copy, degrees.
    pub increment_angle_deg: f64,
    /// Where the first copy sits, as a distance along the path (mm).
    pub start_position_mm: f64,
    /// How much of the path to use from the start position; 0 = the rest of it.
    pub section_length_mm: f64,
    /// Number of copies (1–10000), spread evenly over the section; ignored when `step_mm` is set.
    pub count: u32,
    /// Distance between copies along the path, instead of `count`.
    pub step_mm: Option<f64>,
}

impl Default for AlongParams {
    fn default() -> Self {
        Self {
            perpendicular: true,
            start_angle_deg: 0.0,
            increment_angle_deg: 0.0,
            start_position_mm: 0.0,
            section_length_mm: 0.0,
            count: 50,
            step_mm: None,
        }
    }
}

/// The transforms that carry an object centred at `centre` to each place along the first
/// contour of `path`.
pub fn object_on_path(path: &BezPath, centre: Point, p: &AlongParams) -> Result<Vec<Affine>> {
    let start_angle = check_range(p.start_angle_deg, -36_000.0, 36_000.0, "start_angle_deg")?;
    let inc = check_range(
        p.increment_angle_deg,
        -36_000.0,
        36_000.0,
        "increment_angle_deg",
    )?;
    let start = check_range(p.start_position_mm, 0.0, MAX_MM, "start_position_mm")?;
    let section = check_range(p.section_length_mm, 0.0, MAX_MM, "section_length_mm")?;
    let contour = flatten(path)?
        .into_iter()
        .next()
        .ok_or_else(|| GenError("the path has no contour".into()))?;
    let walk = Walker::new(&contour.points, contour.closed);
    let len = walk.length();
    if len <= 0.0 {
        return Err(GenError("the path has no length".into()));
    }
    let whole = section == 0.0;
    let section = if whole { len } else { section.min(len) };
    // A closed path used whole wraps around: the last copy must not land on the first.
    let wraps = contour.closed && whole;
    let (n, step) = match p.step_mm {
        Some(step) => {
            let step = check_positive(step, MAX_MM, "step_mm")?;
            let n = if wraps {
                (section / step).floor().max(1.0)
            } else {
                (section / step).floor() + 1.0
            };
            (n, step)
        }
        None => {
            if !(1..=MAX_ITEMS as u32).contains(&p.count) {
                return Err(GenError(format!("count must be between 1 and {MAX_ITEMS}")));
            }
            let n = p.count as f64;
            let gaps = if wraps { n } else { (n - 1.0).max(1.0) };
            (n, section / gaps)
        }
    };
    if n > MAX_ITEMS as f64 {
        return Err(GenError(format!(
            "more than {MAX_ITEMS} copies; raise step_mm"
        )));
    }
    let mut out = Vec::with_capacity(n as usize);
    for k in 0..n as usize {
        let mut s = start + k as f64 * step;
        if contour.closed {
            s = s.rem_euclid(len);
        }
        let Some((pt, tangent)) = walk.at(s) else {
            continue;
        };
        let mut angle = (start_angle + inc * k as f64).to_radians();
        if p.perpendicular {
            angle += tangent.atan2();
        }
        out.push(
            Affine::translate(pt.to_vec2())
                * Affine::rotate(angle)
                * Affine::translate(-centre.to_vec2()),
        );
    }
    Ok(out)
}

/// Leather punch sizes (hole diameters, mm).
pub const PUNCH_SIZES_MM: [f64; 3] = [1.0, 3.0, 5.0];

/// Evenly spaced punch points along every contour of `path`, `spacing_mm` apart (a closed
/// contour's spacing stretches slightly so the points close up evenly).
pub fn punch_points(path: &BezPath, spacing_mm: f64) -> Result<Vec<Point>> {
    let spacing = check_positive(spacing_mm, MAX_MM, "spacing_mm")?;
    let mut out = Vec::new();
    for c in flatten(path)? {
        let walk = Walker::new(&c.points, c.closed);
        let len = walk.length();
        let n = if c.closed {
            (len / spacing).round().max(1.0)
        } else {
            (len / spacing).floor() + 1.0
        };
        if out.len() as f64 + n > MAX_ITEMS as f64 {
            return Err(GenError(format!("more than {MAX_ITEMS} punch points")));
        }
        let step = if c.closed { len / n } else { spacing };
        for k in 0..n as usize {
            if let Some((p, _)) = walk.at(k as f64 * step) {
                out.push(p);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    #[test]
    fn copies_spread_round_a_circle_facing_outward() {
        let c = circle(0.0, 0.0, 10.0);
        let p = AlongParams {
            count: 8,
            ..AlongParams::default()
        };
        let t = object_on_path(&c, Point::new(5.0, 5.0), &p).unwrap();
        assert_eq!(t.len(), 8);
        let pts: Vec<Point> = t.iter().map(|a| *a * Point::new(5.0, 5.0)).collect();
        for w in pts.windows(2) {
            let d = w[0].distance(w[1]);
            assert!(
                (d - 2.0 * 10.0 * (std::f64::consts::PI / 8.0).sin()).abs() < 0.02,
                "{d}"
            );
        }
        // A point to the right of the centre ends up along the tangent.
        let tip = t[0] * Point::new(6.0, 5.0);
        let tangent = (tip - pts[0]).normalize();
        assert!(tangent.dot(pts[0].to_vec2().normalize()).abs() < 0.05);
    }

    #[test]
    fn step_section_and_rotation_increments() {
        let mut line = BezPath::new();
        line.move_to((0.0, 0.0));
        line.line_to((100.0, 0.0));
        let p = AlongParams {
            step_mm: Some(10.0),
            start_position_mm: 20.0,
            section_length_mm: 30.0,
            perpendicular: false,
            increment_angle_deg: 90.0,
            ..AlongParams::default()
        };
        let t = object_on_path(&line, Point::ZERO, &p).unwrap();
        assert_eq!(t.len(), 4);
        let o: Vec<Point> = t.iter().map(|a| *a * Point::ZERO).collect();
        assert!((o[0].x - 20.0).abs() < 1e-9 && (o[3].x - 50.0).abs() < 1e-9);
        let r = t[1] * Point::new(1.0, 0.0) - o[1];
        assert!((r.y - 1.0).abs() < 1e-9);
        let bad = AlongParams {
            count: 0,
            ..AlongParams::default()
        };
        assert!(object_on_path(&line, Point::ZERO, &bad).is_err());
    }

    #[test]
    fn punch_points_space_evenly() {
        let sq = rect(0.0, 0.0, 35.0, 35.0);
        let pts = punch_points(&sq, 3.5).unwrap();
        assert_eq!(pts.len(), 40);
        for i in 0..pts.len() {
            let d = pts[i].distance(pts[(i + 1) % pts.len()]);
            assert!((d - 3.5).abs() < 1e-6, "{d}");
        }
        assert!(punch_points(&sq, 0.0).is_err());
        assert!(punch_points(&sq, 1e-6).is_err());
    }
}
