//! Cut production for StencilCraft: the data a cutter's Make / Send step needs.
//!
//! - [`machines`]: mat sizes, roll limits and Print Then Cut limits per machine.
//! - [`layout_mats`]: pieces grouped onto mats by material, colour and operation (Attach groups
//!   kept rigid), project copies, column layout or nesting with rotation steps, manual mats,
//!   mirror per mat, mat-free feed lengths.
//! - [`build_job`]: per-mat cut operations with tool holders, passes, cut order (inside first,
//!   by colour, by layer, shortest travel), line-segment overcut, test cut, weed box and lines,
//!   origin and feed direction. The machine layer turns these into device commands.
//! - [`print_sheets`]: Print Then Cut pages with registration marks and bleed, exported with
//!   [`sheet_to_png`] (or as PDF by `stencilcraft-pdf`).
//!
//! Everything here is a pure function of the document and its options; bad input is a
//! [`LayoutError`], never a panic. Coordinates on a mat are millimetres with the origin at the
//! top-left of the cut area and y growing in the feed direction (down the mat).

mod job;
pub mod machines;
mod mats;
mod print;

pub use job::{
    AfterJob, CutBy, CutJob, CutOp, CutOrder, Feed, GroupSetting, JobOptions, MatJob, Origin,
    Registration, StippleOrder, TestCut, Weed, build_job,
};
pub use machines::{MACHINES, MachineProfile, MarkStyle, MatSize, Roll, machine};
pub use mats::{MatKey, MatLayout, MatOptions, MatPlan, PageOptions, Placement, layout_mats};
pub use print::{
    PrintItem, PrintOptions, PrintSheet, art_area, print_sheets, registration_marks, sheet_to_png,
    sheets_to_pdf,
};

use kurbo::{BezPath, PathEl, Point};

#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, LayoutError>;

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(LayoutError::Invalid(message.into()))
}

/// Checks `value` is finite and within `lo..=hi`, naming the knob otherwise.
fn check_range(value: f64, lo: f64, hi: f64, what: &str) -> Result<f64> {
    if value.is_finite() && (lo..=hi).contains(&value) {
        Ok(value)
    } else {
        invalid(format!("{what} must be between {lo} and {hi}, got {value}"))
    }
}

/// The subpaths of `path`, each with its index (the contour numbering Contour uses).
pub fn split_contours(path: &BezPath) -> Vec<(usize, BezPath)> {
    let mut out: Vec<(usize, BezPath)> = Vec::new();
    for el in path.elements() {
        if matches!(el, PathEl::MoveTo(_)) || out.is_empty() {
            let index = out.len();
            out.push((index, BezPath::new()));
        }
        if let Some((_, current)) = out.last_mut() {
            current.push(*el);
        }
    }
    out.retain(|(_, p)| p.elements().len() > 1);
    out
}

fn is_closed(path: &BezPath) -> bool {
    path.elements()
        .last()
        .is_some_and(|el| matches!(el, PathEl::ClosePath))
}

fn polygon(path: &BezPath) -> Vec<Point> {
    stencilcraft_geom::path_to_polylines(path, 16)
        .into_iter()
        .flat_map(|pl| pl.points)
        .collect()
}

fn point_in_polygon(p: Point, poly: &[Point]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (Some(a), Some(b)) = (poly.get(i), poly.get((i + n - 1) % n)) else {
            continue;
        };
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// How many other closed contours enclose each contour (open contours are 0 and enclose
/// nothing). A contour inside another is a counter, cut before its outline.
pub fn nesting_depths(contours: &[BezPath]) -> Vec<usize> {
    let polys: Vec<(bool, Vec<Point>, kurbo::Rect)> = contours
        .iter()
        .map(|c| {
            let poly = polygon(c);
            let bounds = stencilcraft_geom::bounds_of_points(&poly).unwrap_or_default();
            (is_closed(c), poly, bounds)
        })
        .collect();
    polys
        .iter()
        .enumerate()
        .map(|(i, (_, poly, bounds))| {
            let Some(probe) = poly.first() else {
                return 0;
            };
            polys
                .iter()
                .enumerate()
                .filter(|(j, (closed, other, ob))| {
                    *j != i
                        && *closed
                        && ob.x0 <= bounds.x0
                        && ob.y0 <= bounds.y0
                        && ob.x1 >= bounds.x1
                        && ob.y1 >= bounds.y1
                        && ob.area() > bounds.area()
                        && point_in_polygon(*probe, other)
                })
                .count()
        })
        .collect()
}

/// Splits a filled path into its separate pieces: each outer contour with the holes directly
/// inside it (Slice gives one shape per piece).
pub fn split_regions(path: &BezPath) -> Vec<BezPath> {
    let contours: Vec<BezPath> = split_contours(path).into_iter().map(|(_, c)| c).collect();
    let depths = nesting_depths(&contours);
    let mut regions: Vec<(usize, BezPath)> = Vec::new();
    for (i, d) in depths.iter().enumerate() {
        if d % 2 == 0
            && let Some(c) = contours.get(i)
        {
            regions.push((i, c.clone()));
        }
    }
    for (i, d) in depths.iter().enumerate() {
        if d % 2 == 0 {
            continue;
        }
        let Some(hole) = contours.get(i) else {
            continue;
        };
        let probe = polygon(hole).first().copied().unwrap_or_default();
        // The innermost enclosing outer: the one at depth d - 1 that contains the hole.
        if let Some((_, outer)) = regions.iter_mut().find(|(o, _)| {
            depths.get(*o) == Some(&(d - 1))
                && contours
                    .get(*o)
                    .is_some_and(|c| point_in_polygon(probe, &polygon(c)))
        }) {
            outer.extend(hole.iter());
        }
    }
    regions.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn rect(x: f64, y: f64, w: f64, h: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to((x, y));
        p.line_to((x + w, y));
        p.line_to((x + w, y + h));
        p.line_to((x, y + h));
        p.close_path();
        p
    }

    #[test]
    fn depths_and_regions() {
        let mut p = rect(0.0, 0.0, 10.0, 10.0);
        p.extend(rect(2.0, 2.0, 2.0, 2.0).iter());
        p.extend(rect(20.0, 0.0, 5.0, 5.0).iter());
        let contours: Vec<BezPath> = split_contours(&p).into_iter().map(|(_, c)| c).collect();
        assert_eq!(nesting_depths(&contours), vec![0, 1, 0]);
        let regions = split_regions(&p);
        assert_eq!(regions.len(), 2);
        assert_eq!(split_contours(&regions[0]).len(), 2);
    }
}
