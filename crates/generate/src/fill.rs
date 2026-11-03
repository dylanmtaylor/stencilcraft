//! Sketch and stipple fills: pen/engrave paths that fill a closed shape with hatch lines,
//! cross-hatch, a grid, concentric rings, a spiral, or dots. The same patterns serve the
//! emboss, deboss and foil tools (they only differ in the tool that follows the path).

use std::f64::consts::TAU;

use kurbo::{BezPath, Point, Shape};
use serde::{Deserialize, Serialize};

use crate::clip::{bounds, clip_polyline, hatch};
use crate::{
    GenError, MAX_ITEMS, MAX_MM, MAX_POINTS, Result, Rng, check_positive, check_range, inside,
    offset_polygons, polygons, polylines_path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FillPattern {
    /// Parallel lines at `angle_deg`.
    #[default]
    Lines,
    /// Two diagonal families at `angle_deg ± 45°`.
    CrossHatch,
    /// Two families at `angle_deg` and `angle_deg + 90°`.
    Grid,
    /// Inset copies of the outline, `spacing_mm` apart.
    Concentric,
    /// An Archimedean spiral from the shape's centre, `spacing_mm` between turns.
    Spiral,
    /// Dots on a square grid rotated by `angle_deg`.
    StippleGrid,
    /// Dots jittered at random (seeded) within each grid cell.
    StippleRandom,
}

/// Fill parameters, millimetres and degrees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FillParams {
    pub pattern: FillPattern,
    /// Distance between lines, rings, turns or dot centres (0.05–1000).
    pub spacing_mm: f64,
    pub angle_deg: f64,
    /// Keep the fill this far inside the outline (0–1000).
    pub inset_mm: f64,
    /// Stipple dot diameter (0.01–100).
    pub dot_mm: f64,
    /// Seed for `stipple-random`.
    pub seed: u64,
}

impl Default for FillParams {
    fn default() -> Self {
        Self {
            pattern: FillPattern::Lines,
            spacing_mm: 1.0,
            angle_deg: 45.0,
            inset_mm: 0.0,
            dot_mm: 0.5,
            seed: 1,
        }
    }
}

/// Fills the closed contours of `shape` (even-odd: holes stay empty). Line patterns return
/// open paths; concentric rings and stipple dots are closed.
pub fn fill(shape: &BezPath, params: &FillParams) -> Result<BezPath> {
    let spacing = check_positive(params.spacing_mm, MAX_MM, "spacing_mm")?;
    if spacing < 0.05 {
        return Err(GenError("spacing_mm must be at least 0.05".into()));
    }
    let angle = check_range(params.angle_deg, -3600.0, 3600.0, "angle_deg")?;
    let inset = check_range(params.inset_mm, 0.0, MAX_MM, "inset_mm")?;
    let polys = polygons(shape)?;
    if let Some(b) = bounds(&polys)
        && (b.width() / spacing) * (b.height() / spacing) > 4.0 * MAX_ITEMS as f64 * 50.0
    {
        return Err(GenError(format!(
            "spacing {spacing} mm is too fine for a shape this size"
        )));
    }
    let dot = match params.pattern {
        FillPattern::StippleGrid | FillPattern::StippleRandom => {
            check_positive(params.dot_mm, 100.0, "dot_mm")?
        }
        _ => 0.0,
    };
    let region = offset_polygons(&polys, -(inset + dot / 2.0))?;
    if region.is_empty() {
        return Ok(BezPath::new());
    }
    let out = match params.pattern {
        FillPattern::Lines => polylines_path(&hatch(&region, spacing, angle)?, false),
        FillPattern::CrossHatch | FillPattern::Grid => {
            let base = if params.pattern == FillPattern::Grid {
                angle
            } else {
                angle + 45.0
            };
            let mut lines = hatch(&region, spacing, base)?;
            lines.extend(hatch(&region, spacing, base + 90.0)?);
            if lines.len() > MAX_ITEMS {
                return Err(GenError(format!("more than {MAX_ITEMS} hatch lines")));
            }
            polylines_path(&lines, false)
        }
        FillPattern::Concentric => concentric(&region, spacing)?,
        FillPattern::Spiral => spiral(&region, spacing, angle)?,
        FillPattern::StippleGrid | FillPattern::StippleRandom => stipple(
            &region,
            spacing,
            angle,
            dot,
            (params.pattern == FillPattern::StippleRandom).then_some(params.seed),
        )?,
    };
    Ok(out)
}

fn concentric(region: &[Vec<Point>], spacing: f64) -> Result<BezPath> {
    let mut rings: Vec<Vec<Point>> = Vec::new();
    let mut points = 0usize;
    let mut current = offset_polygons(region, -spacing / 2.0)?;
    while !current.is_empty() {
        points += current.iter().map(Vec::len).sum::<usize>();
        if rings.len() + current.len() > MAX_ITEMS || points > MAX_POINTS {
            return Err(GenError(
                "too many concentric rings; raise spacing_mm".into(),
            ));
        }
        let next = offset_polygons(&current, -spacing)?;
        rings.extend(current);
        current = next;
    }
    Ok(polylines_path(&rings, true))
}

fn spiral(region: &[Vec<Point>], spacing: f64, angle: f64) -> Result<BezPath> {
    let Some(b) = bounds(region) else {
        return Ok(BezPath::new());
    };
    let c = b.center();
    let reach = (b.width().hypot(b.height())) / 2.0 + spacing;
    let turns = reach / spacing;
    // About one point per half-millimetre of the outer turn, at least 32 per turn.
    let per_turn = (TAU * reach / 0.5).clamp(32.0, 720.0);
    let n = (turns * per_turn).ceil();
    if !n.is_finite() || n > MAX_POINTS as f64 {
        return Err(GenError("spiral too dense; raise spacing_mm".into()));
    }
    let phase = angle.to_radians();
    let pts: Vec<Point> = (0..=n as usize)
        .map(|i| {
            let t = i as f64 / per_turn * TAU;
            let r = spacing * t / TAU;
            Point::new(c.x + r * (t + phase).cos(), c.y + r * (t + phase).sin())
        })
        .collect();
    Ok(polylines_path(&clip_polyline(&pts, region, true), false))
}

fn stipple(
    region: &[Vec<Point>],
    spacing: f64,
    angle: f64,
    dot: f64,
    seed: Option<u64>,
) -> Result<BezPath> {
    let rot = kurbo::Affine::rotate(angle.to_radians());
    let inv = rot.inverse();
    let local: Vec<Vec<Point>> = region
        .iter()
        .map(|p| p.iter().map(|q| inv * *q).collect())
        .collect();
    let Some(b) = bounds(&local) else {
        return Ok(BezPath::new());
    };
    let (cols, rows) = ((b.width() / spacing).ceil(), (b.height() / spacing).ceil());
    let edges: usize = local.iter().map(Vec::len).sum();
    if cols * rows > 50.0 * MAX_ITEMS as f64 || cols * rows * edges as f64 > 2e8 {
        return Err(GenError(
            "stipple too dense for this shape; raise spacing_mm".into(),
        ));
    }
    let mut rng = Rng::new(seed.unwrap_or(0));
    let mut out = BezPath::new();
    let mut count = 0usize;
    for j in 0..rows as usize {
        for i in 0..cols as usize {
            let (jx, jy) = match seed {
                Some(_) => (rng.next_f64(), rng.next_f64()),
                None => (0.5, 0.5),
            };
            let p = Point::new(
                b.x0 + (i as f64 + jx) * spacing,
                b.y0 + (j as f64 + jy) * spacing,
            );
            if inside(&local, p) {
                count += 1;
                if count > MAX_ITEMS {
                    return Err(GenError(format!(
                        "more than {MAX_ITEMS} dots; raise spacing_mm"
                    )));
                }
                out.extend(kurbo::Circle::new(rot * p, dot / 2.0).path_elements(dot * 1e-3));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flatten;
    use crate::test_util::*;

    fn params(pattern: FillPattern) -> FillParams {
        FillParams {
            pattern,
            spacing_mm: 1.0,
            angle_deg: 0.0,
            ..FillParams::default()
        }
    }

    fn inside_shape(path: &BezPath, shape: &BezPath) {
        let polys = polygons(shape).unwrap();
        for c in flatten(path).unwrap() {
            for w in c.points.windows(2) {
                let mid = w[0].midpoint(w[1]);
                assert!(inside(&polys, mid), "{mid:?} outside");
            }
        }
    }

    #[test]
    fn lines_fill_square_with_spacing() {
        let sq = rect(0.0, 0.0, 10.0, 10.0);
        let out = fill(&sq, &params(FillPattern::Lines)).unwrap();
        let c = flatten(&out).unwrap();
        assert_eq!(c.len(), 10);
        assert!(c.iter().all(|c| !c.closed));
        inside_shape(&out, &sq);
    }

    #[test]
    fn grid_and_cross_hatch_have_two_families() {
        let sq = rect(0.0, 0.0, 10.0, 10.0);
        let grid = flatten(&fill(&sq, &params(FillPattern::Grid)).unwrap()).unwrap();
        assert_eq!(grid.len(), 20);
        let cross = fill(&sq, &params(FillPattern::CrossHatch)).unwrap();
        inside_shape(&cross, &sq);
        assert!(flatten(&cross).unwrap().len() > 20);
    }

    #[test]
    fn concentric_rings_step_inward() {
        let c = circle(0.0, 0.0, 10.0);
        let out = flatten(&fill(&c, &params(FillPattern::Concentric)).unwrap()).unwrap();
        assert_eq!(out.len(), 10);
        let radii: Vec<f64> = out.iter().map(|r| r.points[0].to_vec2().hypot()).collect();
        for w in radii.windows(2) {
            assert!((w[0] - w[1] - 1.0).abs() < 0.05, "{radii:?}");
        }
        assert!((radii[0] - 9.5).abs() < 0.05);
    }

    #[test]
    fn spiral_stays_inside_and_ring_hole_stays_empty() {
        let mut ring = circle(0.0, 0.0, 10.0);
        ring.extend(circle(0.0, 0.0, 4.0).reverse_subpaths());
        let out = fill(&ring, &params(FillPattern::Spiral)).unwrap();
        inside_shape(&out, &ring);
        let lines = fill(&ring, &params(FillPattern::Lines)).unwrap();
        inside_shape(&lines, &ring);
    }

    #[test]
    fn stipple_grid_counts_and_random_is_seeded() {
        let sq = rect(0.0, 0.0, 10.0, 10.0);
        let mut p = params(FillPattern::StippleGrid);
        p.dot_mm = 0.2;
        let dots = flatten(&fill(&sq, &p).unwrap()).unwrap();
        assert_eq!(dots.len(), 100);
        assert!(dots.iter().all(|d| d.closed));
        p.pattern = FillPattern::StippleRandom;
        let a = fill(&sq, &p).unwrap();
        assert_eq!(a, fill(&sq, &p).unwrap());
        p.seed = 7;
        assert_ne!(a, fill(&sq, &p).unwrap());
    }

    #[test]
    fn inset_and_bad_input() {
        let sq = rect(0.0, 0.0, 10.0, 10.0);
        let mut p = params(FillPattern::Lines);
        p.inset_mm = 2.0;
        let b = fill(&sq, &p).unwrap().bounding_box();
        assert!(b.x0 >= 1.99 && b.x1 <= 8.01);
        p.inset_mm = 20.0;
        assert!(fill(&sq, &p).unwrap().elements().is_empty());
        p.spacing_mm = 0.0;
        assert!(fill(&sq, &p).is_err());
        p.spacing_mm = f64::INFINITY;
        assert!(fill(&sq, &p).is_err());
        let mut open = BezPath::new();
        open.move_to((0.0, 0.0));
        open.line_to((1.0, 1.0));
        assert!(fill(&open, &params(FillPattern::Lines)).is_err());
    }
}
