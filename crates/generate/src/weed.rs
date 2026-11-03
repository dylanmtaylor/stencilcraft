//! Weeding aids and registration: a weeding box around a design with internal weed lines
//! that split the waste into strips (stopping short of the design), and alignment marks.

use kurbo::{BezPath, Point, Rect, Shape};
use serde::{Deserialize, Serialize};

use crate::clip::clip_polyline;
use crate::{GenError, MAX_ITEMS, MAX_MM, Result, check_positive, check_range, polygons};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WeedDirection {
    #[default]
    Vertical,
    Horizontal,
    Both,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WeedParams {
    /// Box distance from the design's bounds (0–1000 mm).
    pub margin_mm: f64,
    /// Distance between internal weed lines; 0 = box only.
    pub spacing_mm: f64,
    pub direction: WeedDirection,
    /// Weed lines stop this far from the design (0–100 mm).
    pub clearance_mm: f64,
}

impl Default for WeedParams {
    fn default() -> Self {
        Self {
            margin_mm: 3.0,
            spacing_mm: 0.0,
            direction: WeedDirection::Vertical,
            clearance_mm: 1.0,
        }
    }
}

/// The weeding box and the internal weed lines for the designs in `shapes`.
pub fn weed(shapes: &[BezPath], p: &WeedParams) -> Result<(BezPath, BezPath)> {
    let margin = check_range(p.margin_mm, 0.0, 1000.0, "margin_mm")?;
    let spacing = check_range(p.spacing_mm, 0.0, MAX_MM, "spacing_mm")?;
    let clearance = check_range(p.clearance_mm, 0.0, 100.0, "clearance_mm")?;
    let mut polys = Vec::new();
    let mut extent: Option<Rect> = None;
    for s in shapes {
        let b = crate::path_bounds(s)?;
        extent = Some(extent.map_or(b, |e| e.union(b)));
        // Open paths have no inside; they only grow the box.
        if let Ok(p) = polygons(s) {
            polys.extend(p);
        }
    }
    let area = extent
        .ok_or_else(|| GenError("weed needs at least one shape".into()))?
        .inflate(margin, margin);
    let boxed = area.to_path(0.1);
    let mut lines = BezPath::new();
    if spacing > 0.0 {
        let mut add = |vertical: bool| -> Result<()> {
            let (lo, hi) = if vertical {
                (area.x0, area.x1)
            } else {
                (area.y0, area.y1)
            };
            let n = ((hi - lo) / spacing).floor();
            if n > MAX_ITEMS as f64 {
                return Err(GenError(format!("more than {MAX_ITEMS} weed lines")));
            }
            for k in 1..=n as usize {
                let c = lo + k as f64 * spacing;
                if c >= hi - 1e-9 {
                    break;
                }
                let line = if vertical {
                    [Point::new(c, area.y0), Point::new(c, area.y1)]
                } else {
                    [Point::new(area.x0, c), Point::new(area.x1, c)]
                };
                for piece in clip_polyline(&line, &polys, false) {
                    let (Some(a), Some(b)) = (piece.first(), piece.last()) else {
                        continue;
                    };
                    // Pull back from the design, never from the box edge.
                    let len = a.distance(*b);
                    let on_box = |q: &Point| {
                        (q.x - area.x0).abs() < 1e-6
                            || (q.x - area.x1).abs() < 1e-6
                            || (q.y - area.y0).abs() < 1e-6
                            || (q.y - area.y1).abs() < 1e-6
                    };
                    let s0 = if on_box(a) { 0.0 } else { clearance };
                    let s1 = if on_box(b) { 0.0 } else { clearance };
                    if len - s0 - s1 <= 1e-6 {
                        continue;
                    }
                    lines.move_to(a.lerp(*b, s0 / len));
                    lines.line_to(a.lerp(*b, 1.0 - s1 / len));
                }
            }
            Ok(())
        };
        match p.direction {
            WeedDirection::Vertical => add(true)?,
            WeedDirection::Horizontal => add(false)?,
            WeedDirection::Both => {
                add(true)?;
                add(false)?;
            }
        }
    }
    Ok((boxed, lines))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarkStyle {
    /// L-shaped brackets at the four corners.
    #[default]
    Corners,
    /// A cross with a circle at each corner.
    Crosshair,
}

/// Registration marks around `bounds` grown by `margin_mm`, arms `arm_mm` long. Cutting the
/// same marks on every layer of a layered design lines the layers up.
pub fn registration(
    bounds: Rect,
    margin_mm: f64,
    arm_mm: f64,
    style: MarkStyle,
) -> Result<BezPath> {
    let margin = check_range(margin_mm, 0.0, 1000.0, "margin_mm")?;
    let arm = check_positive(arm_mm, 1000.0, "arm_mm")?;
    let r = bounds.inflate(margin, margin);
    let mut out = BezPath::new();
    for (corner, sx, sy) in [
        (Point::new(r.x0, r.y0), 1.0, 1.0),
        (Point::new(r.x1, r.y0), -1.0, 1.0),
        (Point::new(r.x1, r.y1), -1.0, -1.0),
        (Point::new(r.x0, r.y1), 1.0, -1.0),
    ] {
        match style {
            MarkStyle::Corners => {
                out.move_to(corner + (arm * sx, 0.0));
                out.line_to(corner);
                out.line_to(corner + (0.0, arm * sy));
            }
            MarkStyle::Crosshair => {
                let c = corner + (arm / 2.0 * sx, arm / 2.0 * sy);
                out.move_to(c - (arm / 2.0, 0.0));
                out.line_to(c + (arm / 2.0, 0.0));
                out.move_to(c - (0.0, arm / 2.0));
                out.line_to(c + (0.0, arm / 2.0));
                out.extend(kurbo::Circle::new(c, arm / 4.0).path_elements(1e-3));
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

    #[test]
    fn box_and_lines_avoid_the_design() {
        let design = [circle(10.0, 10.0, 5.0), rect(30.0, 0.0, 10.0, 20.0)];
        let p = WeedParams {
            margin_mm: 2.0,
            spacing_mm: 4.0,
            clearance_mm: 1.0,
            direction: WeedDirection::Both,
        };
        let (b, lines) = weed(&design, &p).unwrap();
        let bb = b.bounding_box();
        assert_eq!(bb, Rect::new(3.0, -2.0, 42.0, 22.0));
        let polys: Vec<_> = design.iter().flat_map(|d| polygons(d).unwrap()).collect();
        let lines = flatten(&lines).unwrap();
        assert!(!lines.is_empty());
        for l in &lines {
            for q in &l.points {
                assert!(!crate::inside(&polys, *q), "{q:?} on the design");
            }
        }
        // Vertical lines over a bar stop exactly the clearance above and below it.
        let bar = [rect(0.0, 10.0, 20.0, 5.0)];
        let p = WeedParams {
            margin_mm: 4.0,
            spacing_mm: 5.0,
            clearance_mm: 1.0,
            direction: WeedDirection::Vertical,
        };
        let (_, lines) = weed(&bar, &p).unwrap();
        let lines = flatten(&lines).unwrap();
        // x = 1, 6, 11, 16 split around the bar; x = 21 runs past it.
        assert_eq!(lines.len(), 9);
        for l in &lines {
            for q in &l.points {
                assert!(
                    [6.0, 9.0, 16.0, 19.0]
                        .iter()
                        .any(|y| (q.y - y).abs() < 1e-9),
                    "{q:?}"
                );
            }
        }
        let (_, none) = weed(&design, &WeedParams::default()).unwrap();
        assert!(none.elements().is_empty());
        assert!(weed(&[], &WeedParams::default()).is_err());
    }

    #[test]
    fn marks_count() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let c = registration(r, 5.0, 8.0, MarkStyle::Corners).unwrap();
        assert_eq!(flatten(&c).unwrap().len(), 4);
        assert_eq!(c.bounding_box(), r.inflate(5.0, 5.0));
        let x = registration(r, 5.0, 8.0, MarkStyle::Crosshair).unwrap();
        assert_eq!(flatten(&x).unwrap().len(), 12);
        assert_eq!(x.bounding_box(), r.inflate(5.0, 5.0));
        assert!(registration(r, 5.0, 0.0, MarkStyle::Corners).is_err());
    }
}
