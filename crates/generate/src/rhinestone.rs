//! Rhinestone templates: holes (circles) the size of a stone, placed along a shape's
//! outline (one or two lines) and/or filling its inside on a grid or hexagonal pattern.

use kurbo::{BezPath, Circle, Point, Shape};
use serde::{Deserialize, Serialize};

use crate::clip::{bounds, clip_polyline};
use crate::{GenError, MAX_ITEMS, Result, Walker, check_range, flatten, offset_polygons, polygons};

/// Stone sizes ("SS", stone size) and their nominal diameters in millimetres.
pub const STONE_SIZES: [(&str, f64); 7] = [
    ("ss6", 2.0),
    ("ss8", 2.4),
    ("ss10", 2.8),
    ("ss12", 3.1),
    ("ss16", 3.9),
    ("ss20", 4.7),
    ("ss30", 6.4),
];

/// A stone size preset's diameter (`"ss10"`, case-insensitive).
pub fn stone_diameter(preset: &str) -> Result<f64> {
    STONE_SIZES
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(preset))
        .map(|(_, d)| *d)
        .ok_or_else(|| {
            GenError(format!(
                "unknown stone size {preset:?} (ss6, ss8, ss10, ss12, ss16, ss20, ss30)"
            ))
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StoneLayout {
    /// Stones centred on the outline.
    #[default]
    Outline,
    /// Stones filling the inside.
    Fill,
    /// Outline lines and a fill inside them.
    OutlineFill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StonePattern {
    /// Rows and columns.
    Grid,
    /// Alternate rows offset by half a pitch (denser).
    #[default]
    Hex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RhinestoneParams {
    /// Preset name (`ss6`…`ss30`); ignored when `stone_mm` is set.
    pub size: String,
    /// Explicit stone (hole) diameter, 0.5–20 mm.
    pub stone_mm: Option<f64>,
    /// Gap between neighbouring stones (0–50 mm).
    pub spacing_mm: f64,
    pub layout: StoneLayout,
    /// Outline lines: 1 or 2 (the second runs one pitch inside the first).
    pub lines: u32,
    pub pattern: StonePattern,
}

impl Default for RhinestoneParams {
    fn default() -> Self {
        Self {
            size: "ss10".into(),
            stone_mm: None,
            spacing_mm: 1.0,
            layout: StoneLayout::Outline,
            lines: 1,
            pattern: StonePattern::Hex,
        }
    }
}

/// Stone centres and the diameter.
pub fn stone_centres(shape: &BezPath, p: &RhinestoneParams) -> Result<(Vec<Point>, f64)> {
    let d = match p.stone_mm {
        Some(d) => check_range(d, 0.5, 20.0, "stone_mm")?,
        None => stone_diameter(&p.size)?,
    };
    let gap = check_range(p.spacing_mm, 0.0, 50.0, "spacing_mm")?;
    if !(1..=2).contains(&p.lines) {
        return Err(GenError("lines must be 1 or 2".into()));
    }
    let pitch = d + gap;
    let mut centres = Vec::new();
    let outline = matches!(p.layout, StoneLayout::Outline | StoneLayout::OutlineFill);
    if outline {
        let mut contours: Vec<(Vec<Point>, bool)> = flatten(shape)?
            .into_iter()
            .map(|c| (c.points, c.closed))
            .collect();
        for line in 0..p.lines {
            if line > 0 {
                let closed: Vec<Vec<Point>> = contours
                    .iter()
                    .filter(|c| c.1)
                    .map(|c| c.0.clone())
                    .collect();
                contours = offset_polygons(&closed, -pitch)?
                    .into_iter()
                    .map(|c| (c, true))
                    .collect();
            }
            for (pts, closed) in &contours {
                along(pts, *closed, pitch, d, &mut centres)?;
            }
        }
    }
    if matches!(p.layout, StoneLayout::Fill | StoneLayout::OutlineFill) {
        let polys = polygons(shape)?;
        // Inside the outline lines (if any) by a pitch, else keep whole stones inside.
        let inset = if outline {
            pitch * p.lines as f64
        } else {
            d / 2.0
        };
        let region = offset_polygons(&polys, -inset)?;
        fill_points(&region, pitch, p.pattern, &mut centres)?;
    }
    Ok((centres, d))
}

/// Evenly spaced stones along one contour; a closed contour's pitch stretches slightly so
/// the last stone does not crowd the first.
fn along(pts: &[Point], closed: bool, pitch: f64, d: f64, out: &mut Vec<Point>) -> Result<()> {
    let walk = Walker::new(pts, closed);
    let len = walk.length();
    let n = if closed {
        (len / pitch).floor()
    } else {
        (len / pitch).floor() + 1.0
    };
    if n < 1.0 {
        if let Some(p) = pts.first() {
            out.push(*p);
        }
        return Ok(());
    }
    if out.len() as f64 + n > MAX_ITEMS as f64 {
        return Err(GenError(format!(
            "more than {MAX_ITEMS} stones; raise spacing"
        )));
    }
    let step = if closed { len / n } else { pitch };
    let first = out.len();
    for k in 0..n as usize {
        let Some((p, _)) = walk.at(k as f64 * step) else {
            continue;
        };
        // Across a sharp corner the chord is shorter than the step: drop a stone that
        // would overlap its neighbour (or, closing the loop, the first one).
        let near = |q: Option<&Point>| q.is_some_and(|q| q.distance(p) < d - 1e-9);
        let last_of_loop = closed && k + 1 == n as usize;
        if (out.len() > first && near(out.last())) || (last_of_loop && near(out.get(first))) {
            continue;
        }
        out.push(p);
    }
    Ok(())
}

fn fill_points(
    region: &[Vec<Point>],
    pitch: f64,
    pattern: StonePattern,
    out: &mut Vec<Point>,
) -> Result<()> {
    let Some(b) = bounds(region) else {
        return Ok(());
    };
    let row = match pattern {
        StonePattern::Grid => pitch,
        StonePattern::Hex => pitch * 3f64.sqrt() / 2.0,
    };
    let rows = (b.height() / row).floor() + 1.0;
    if rows * (b.width() / pitch) > 4.0 * MAX_ITEMS as f64 {
        return Err(GenError(format!(
            "more than {MAX_ITEMS} stones; raise spacing"
        )));
    }
    let y0 = b.y0 + (b.height() - (rows - 1.0) * row) / 2.0;
    let x0 = b.x0 + b.width().rem_euclid(pitch) / 2.0;
    for j in 0..rows as usize {
        let y = y0 + j as f64 * row;
        let shift = if pattern == StonePattern::Hex && j % 2 == 1 {
            pitch / 2.0
        } else {
            0.0
        };
        let line = [Point::new(b.x0 - 1.0, y), Point::new(b.x1 + 1.0, y)];
        // A shared lattice keeps hex rows aligned across separate spans of the shape.
        for span in clip_polyline(&line, region, true) {
            let (Some(a), Some(z)) = (span.first(), span.last()) else {
                continue;
            };
            let (lo, hi) = (a.x.min(z.x), a.x.max(z.x));
            let first = ((lo - x0 - shift) / pitch).ceil();
            let mut x = x0 + shift + first * pitch;
            while x <= hi + 1e-9 {
                out.push(Point::new(x, y));
                if out.len() > MAX_ITEMS {
                    return Err(GenError(format!(
                        "more than {MAX_ITEMS} stones; raise spacing"
                    )));
                }
                x += pitch;
            }
        }
    }
    Ok(())
}

/// The template: one compound path of stone-sized circles.
pub fn rhinestones(shape: &BezPath, p: &RhinestoneParams) -> Result<BezPath> {
    let (centres, d) = stone_centres(shape, p)?;
    let mut out = BezPath::new();
    for c in centres {
        out.extend(Circle::new(c, d / 2.0).path_elements(d * 1e-3));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    fn min_gap(points: &[Point]) -> f64 {
        let mut m = f64::MAX;
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                m = m.min(points[i].distance(points[j]));
            }
        }
        m
    }

    #[test]
    fn outline_circle_is_evenly_spaced() {
        let c = circle(0.0, 0.0, 20.0);
        let p = RhinestoneParams::default();
        let (pts, d) = stone_centres(&c, &p).unwrap();
        assert_eq!(d, 2.8);
        let len = std::f64::consts::TAU * 20.0;
        assert_eq!(pts.len(), (len / 3.8).floor() as usize);
        let gaps: Vec<f64> = (0..pts.len())
            .map(|i| pts[i].distance(pts[(i + 1) % pts.len()]))
            .collect();
        let (lo, hi) = gaps
            .iter()
            .fold((f64::MAX, 0.0f64), |(a, b), g| (a.min(*g), b.max(*g)));
        assert!(hi - lo < 0.01 && lo >= 3.8 - 0.05, "{lo} {hi}");
    }

    #[test]
    fn double_line_and_fill_never_overlap() {
        let sq = rect(0.0, 0.0, 40.0, 30.0);
        for layout in [StoneLayout::Fill, StoneLayout::OutlineFill] {
            for pattern in [StonePattern::Grid, StonePattern::Hex] {
                let p = RhinestoneParams {
                    layout,
                    pattern,
                    lines: 2,
                    stone_mm: Some(3.0),
                    spacing_mm: 0.5,
                    ..RhinestoneParams::default()
                };
                let (pts, d) = stone_centres(&sq, &p).unwrap();
                assert!(pts.len() > 20);
                assert!(
                    min_gap(&pts) >= d - 1e-6,
                    "{layout:?} {pattern:?} {}",
                    min_gap(&pts)
                );
                if layout == StoneLayout::Fill {
                    let polys = polygons(&sq).unwrap();
                    assert!(pts.iter().all(|c| crate::inside(&polys, *c)));
                }
            }
        }
    }

    #[test]
    fn grid_fill_count() {
        let sq = rect(0.0, 0.0, 10.0, 10.0);
        let p = RhinestoneParams {
            layout: StoneLayout::Fill,
            pattern: StonePattern::Grid,
            stone_mm: Some(1.0),
            spacing_mm: 1.0,
            ..RhinestoneParams::default()
        };
        // Region 9 × 9 at pitch 2: 5 × 5 stones.
        assert_eq!(stone_centres(&sq, &p).unwrap().0.len(), 25);
        let path = rhinestones(&sq, &p).unwrap();
        assert_eq!(flatten(&path).unwrap().len(), 25);
    }

    #[test]
    fn bad_presets_and_caps() {
        let sq = rect(0.0, 0.0, 10.0, 10.0);
        let p = RhinestoneParams {
            size: "ss99".into(),
            ..RhinestoneParams::default()
        };
        assert!(stone_centres(&sq, &p).is_err());
        let p = RhinestoneParams {
            lines: 3,
            ..RhinestoneParams::default()
        };
        assert!(stone_centres(&sq, &p).is_err());
        let huge = rect(0.0, 0.0, 5000.0, 5000.0);
        let p = RhinestoneParams {
            layout: StoneLayout::Fill,
            stone_mm: Some(0.5),
            spacing_mm: 0.0,
            ..RhinestoneParams::default()
        };
        assert!(stone_centres(&huge, &p).is_err());
    }
}
