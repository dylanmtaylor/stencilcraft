//! Envelope warps: parametric distortions of a path inside its own bounds, with a bend
//! amount and horizontal/vertical perspective distortion. The result is a densely sampled
//! polyline, so curves bend smoothly; `path.simplify` can refit it afterwards.
//!
//! [`warp`] is pure, so a frontend can call it every frame to preview before applying.

use std::f64::consts::{FRAC_PI_2, PI};

use kurbo::{BezPath, Point};
use serde::{Deserialize, Serialize};

use crate::{GenError, MAX_POINTS, Result, check_range, flatten, path_bounds, polyline_path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WarpStyle {
    /// Only the horizontal/vertical distortion.
    None,
    /// Bend the whole shape along a circular arc.
    #[default]
    Arc,
    /// Bottom edge bows; top stays straight.
    ArcLower,
    /// Top edge bows; bottom stays straight.
    ArcUpper,
    /// Both edges bow the same way.
    Arch,
    /// Top bows up, bottom bows down.
    Bulge,
    /// Bottom corners lift (or drop).
    ShellLower,
    /// Top corners drop (or lift).
    ShellUpper,
    /// One full sine period across the width.
    Flag,
    /// Two sine periods across the width.
    Wave,
    /// Fat middle, pinched ends.
    Fish,
    /// Rises along an S-curve from left to right.
    Rise,
    /// Magnifies the centre radially.
    Fisheye,
    /// Pushes every edge outward.
    Inflate,
    /// Pinches horizontally and stretches vertically.
    Squeeze,
    /// Rotates the centre more than the edges.
    Twist,
    /// Wraps the design onto the flattened side of a cone (a tapered cup or tumbler): the top
    /// edge keeps its length on the cone's top circle, lower rows shrink towards the bottom.
    /// Uses the `cone_*` sizes, not `bend`.
    Conical,
}

impl WarpStyle {
    pub const ALL: [WarpStyle; 17] = [
        WarpStyle::None,
        WarpStyle::Arc,
        WarpStyle::ArcLower,
        WarpStyle::ArcUpper,
        WarpStyle::Arch,
        WarpStyle::Bulge,
        WarpStyle::ShellLower,
        WarpStyle::ShellUpper,
        WarpStyle::Flag,
        WarpStyle::Wave,
        WarpStyle::Fish,
        WarpStyle::Rise,
        WarpStyle::Fisheye,
        WarpStyle::Inflate,
        WarpStyle::Squeeze,
        WarpStyle::Twist,
        WarpStyle::Conical,
    ];
}

/// Warp parameters; each amount is −1…1 (0 leaves the shape alone).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WarpParams {
    pub style: WarpStyle,
    pub bend: f64,
    pub horizontal: f64,
    pub vertical: f64,
    /// Conical: the cone's top and bottom diameters and its slant height (the side's length
    /// from rim to base), in millimetres.
    pub cone_top_mm: f64,
    pub cone_bottom_mm: f64,
    pub cone_slant_mm: f64,
}

impl Default for WarpParams {
    fn default() -> Self {
        Self {
            style: WarpStyle::Arc,
            bend: 0.5,
            horizontal: 0.0,
            vertical: 0.0,
            cone_top_mm: 90.0,
            cone_bottom_mm: 65.0,
            cone_slant_mm: 150.0,
        }
    }
}

/// Maps a point in normalised box coordinates (`u`, `v` in −1…1, y up).
fn map(style: WarpStyle, b: f64, u: f64, v: f64) -> (f64, f64) {
    let up = (1.0 + v) / 2.0;
    let low = (1.0 - v) / 2.0;
    let edge = 1.0 - u * u;
    match style {
        WarpStyle::None => (u, v),
        WarpStyle::Arc => {
            let phi = b * FRAC_PI_2;
            if phi.abs() < 1e-9 {
                return (u, v);
            }
            // Radius at the inner edge stays 1/φ > 0, so the shape never folds over.
            let r = 1.0 / phi + phi.signum();
            ((r + v) * (u * phi).sin(), (r + v) * (u * phi).cos() - r)
        }
        WarpStyle::ArcLower => (u, v - b * low * edge),
        WarpStyle::ArcUpper => (u, v + b * up * edge),
        WarpStyle::Arch => (u, v + b * edge),
        WarpStyle::Bulge => (u, v + b * v * edge),
        WarpStyle::ShellLower => (u, v + b * low * u * u),
        WarpStyle::ShellUpper => (u, v - b * up * u * u),
        WarpStyle::Flag => (u, v + b * 0.5 * (PI * u).sin()),
        WarpStyle::Wave => (u, v + b * 0.3 * (2.0 * PI * u).sin()),
        WarpStyle::Fish => (u, v + b * 0.5 * v * (PI * u).cos()),
        WarpStyle::Rise => (u, v + b * 0.5 * (FRAC_PI_2 * u).sin()),
        WarpStyle::Fisheye => {
            let r2 = (u * u + v * v).min(1.0);
            let k = 1.0 + b * 0.5 * (1.0 - r2);
            (u * k, v * k)
        }
        WarpStyle::Inflate => (u + b * 0.3 * u * (1.0 - v * v), v + b * 0.3 * v * edge),
        WarpStyle::Squeeze => (u - b * 0.3 * u * (1.0 - v * v), v + b * 0.3 * v * edge),
        WarpStyle::Twist => {
            let a = b * FRAC_PI_2 * (1.0 - (u * u + v * v).sqrt() / 2f64.sqrt()).max(0.0);
            let (s, c) = a.sin_cos();
            (u * c - v * s, u * s + v * c)
        }
        // Mapped in page millimetres by `cone_map`, not here.
        WarpStyle::Conical => (u, v),
    }
}

/// The radius of the cone's top circle once its side is laid flat, signed: positive when the
/// top is the wider end (the flat pattern's centre lies below the design), negative when the
/// bottom is. `None` for a cylinder (equal diameters), which needs no warp.
fn cone_radius(params: &WarpParams) -> Result<Option<f64>> {
    let top = check_range(params.cone_top_mm, 1.0, 10_000.0, "cone top diameter")?;
    let bottom = check_range(params.cone_bottom_mm, 1.0, 10_000.0, "cone bottom diameter")?;
    let slant = check_range(params.cone_slant_mm, 1.0, 10_000.0, "cone slant height")?;
    let taper = top - bottom;
    if taper.abs() < 1e-6 {
        return Ok(None);
    }
    if slant <= (taper / 2.0).abs() {
        return Err(GenError(
            "the slant height must be longer than the radius change".into(),
        ));
    }
    Ok(Some(slant * top / taper))
}

/// A document point (y up) on the flattened cone whose top circle has radius `r`, with the
/// design's top edge at `top` and its middle at `cx`: the top edge becomes an arc of radius `r`
/// round a centre `r` below it, lower rows arcs of smaller radius.
fn cone_map(r: f64, cx: f64, top: f64, p: Point) -> Point {
    let rho = r - (top - p.y);
    let angle = (p.x - cx) / r;
    let (s, c) = angle.sin_cos();
    Point::new(cx + rho * s, top - r + rho * c)
}

/// Warps `path` inside its bounds. Closed contours stay closed; the output is a polyline
/// sampled at most 1/200 of the bounds' diagonal apart.
pub fn warp(path: &BezPath, params: &WarpParams) -> Result<BezPath> {
    let bend = check_range(params.bend, -1.0, 1.0, "bend")?;
    let h = check_range(params.horizontal, -1.0, 1.0, "horizontal")?;
    let vd = check_range(params.vertical, -1.0, 1.0, "vertical")?;
    let bounds = path_bounds(path)?;
    let (hw, hh) = (bounds.width() / 2.0, bounds.height() / 2.0);
    if hw <= 1e-9 || hh <= 1e-9 {
        return Err(GenError("warp needs a shape with width and height".into()));
    }
    let c = bounds.center();
    let cone = if params.style == WarpStyle::Conical {
        match cone_radius(params)? {
            Some(r) if bounds.height() >= r.abs() * 0.999 && r > 0.0 => {
                return Err(GenError(
                    "the design is taller than the flattened cone can hold".into(),
                ));
            }
            Some(r) => Some(r),
            None => return Ok(path.clone()),
        }
    } else {
        None
    };
    let step = (hw.hypot(hh) * 2.0) / 200.0;
    let mut out = BezPath::new();
    let mut total = 0usize;
    for contour in flatten(path)? {
        let n = contour.points.len();
        let segs = if contour.closed { n } else { n - 1 };
        let mut pts = Vec::new();
        for i in 0..segs {
            let (Some(a), Some(b)) = (contour.points.get(i), contour.points.get((i + 1) % n))
            else {
                continue;
            };
            let k = (a.distance(*b) / step).ceil().max(1.0) as usize;
            total += k;
            if total > MAX_POINTS {
                return Err(GenError(format!("warp result over {MAX_POINTS} points")));
            }
            for j in 0..k {
                pts.push(a.lerp(*b, j as f64 / k as f64));
            }
        }
        if !contour.closed
            && let Some(last) = contour.points.last()
        {
            pts.push(*last);
        }
        let warped: Vec<Point> = pts
            .iter()
            .map(|p| {
                if let Some(r) = cone {
                    return cone_map(r, c.x, bounds.y1, *p);
                }
                let (u, v) = ((p.x - c.x) / hw, (p.y - c.y) / hh);
                let (u, v) = map(params.style, bend, u, v);
                // Perspective: height grows to the right (h > 0), width grows upward (v > 0).
                let (u, v) = (u * (1.0 + vd * 0.5 * v), v * (1.0 + h * 0.5 * u));
                Point::new(c.x + u * hw, c.y + v * hh)
            })
            .collect();
        polyline_path(&warped, contour.closed, &mut out);
    }
    path_bounds(&out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;
    use kurbo::Shape;

    #[test]
    fn zero_amounts_keep_the_shape() {
        let sq = rect(0.0, 0.0, 20.0, 10.0);
        for style in WarpStyle::ALL
            .into_iter()
            .filter(|s| *s != WarpStyle::Conical)
        {
            let p = WarpParams {
                style,
                bend: 0.0,
                horizontal: 0.0,
                vertical: 0.0,
                ..WarpParams::default()
            };
            let out = warp(&sq, &p).unwrap();
            let b = out.bounding_box();
            assert!(
                (b.x0).abs() < 1e-6 && (b.x1 - 20.0).abs() < 1e-6,
                "{style:?}"
            );
            assert!(
                (b.y0).abs() < 1e-6 && (b.y1 - 10.0).abs() < 1e-6,
                "{style:?}"
            );
        }
    }

    #[test]
    fn every_style_keeps_closed_contours_closed_and_simple() {
        let sq = rect(0.0, 0.0, 20.0, 10.0);
        for style in WarpStyle::ALL {
            for bend in [-0.8, 0.8] {
                let p = WarpParams {
                    style,
                    bend,
                    horizontal: 0.3,
                    vertical: -0.2,
                    ..WarpParams::default()
                };
                let c = flatten(&warp(&sq, &p).unwrap()).unwrap();
                assert_eq!(c.len(), 1);
                assert!(c[0].closed);
                assert!(!self_intersects(&c[0].points), "{style:?} {bend}");
            }
        }
    }

    #[test]
    fn arc_bends_and_bulge_grows() {
        let sq = rect(0.0, 0.0, 20.0, 10.0);
        let arc = warp(&sq, &WarpParams::default()).unwrap();
        // Ends of the bottom edge drop below the original bottom.
        assert!(arc.bounding_box().y0 < -1.0);
        let bulge = WarpParams {
            style: WarpStyle::Bulge,
            bend: 0.5,
            ..WarpParams::default()
        };
        let b = warp(&sq, &bulge).unwrap().bounding_box();
        assert!(
            (b.y1 - 12.5).abs() < 0.05 && (b.y0 + 2.5).abs() < 0.05,
            "{b:?}"
        );
    }

    #[test]
    fn bad_amounts_and_flat_shapes_are_refused() {
        let sq = rect(0.0, 0.0, 20.0, 10.0);
        let p = WarpParams {
            bend: 2.0,
            ..WarpParams::default()
        };
        assert!(warp(&sq, &p).is_err());
        let p = WarpParams {
            horizontal: f64::NAN,
            ..WarpParams::default()
        };
        assert!(warp(&sq, &p).is_err());
        let mut line = BezPath::new();
        line.move_to((0.0, 0.0));
        line.line_to((10.0, 0.0));
        assert!(warp(&line, &WarpParams::default()).is_err());
    }

    #[test]
    fn conical_wraps_onto_a_flattened_cone() {
        // A 100 × 20 mm band for a cup 90 mm across the top, 60 at the bottom, 150 mm slant.
        let band = rect(0.0, 0.0, 100.0, 20.0);
        let params = WarpParams {
            style: WarpStyle::Conical,
            cone_top_mm: 90.0,
            cone_bottom_mm: 60.0,
            cone_slant_mm: 150.0,
            ..WarpParams::default()
        };
        let out = warp(&band, &params).unwrap();
        let b = out.bounding_box();
        // The top edge keeps its arc length, so the chord is a little shorter than 100.
        assert!(b.width() < 100.0 && b.width() > 97.0, "{}", b.width());
        // The ends drop as the band curves round the centre below it; the top stays put.
        assert!(b.height() > 20.5, "{}", b.height());
        assert!(
            (b.y1 - 20.0).abs() < 1e-3,
            "the middle of the top edge stays: {}",
            b.y1
        );
        assert!(b.y0 < 0.0, "the ends hang below the band: {}", b.y0);
        let r = cone_radius(&params).unwrap().unwrap();
        assert!((r - 450.0).abs() < 1e-9, "R = L·D/(D−d) = 150·90/30");
        let middle_top = cone_map(r, 50.0, 20.0, Point::new(50.0, 20.0));
        assert!(middle_top.distance(Point::new(50.0, 20.0)) < 1e-9);
        // A cylinder needs no warp; nonsense sizes and a too-tall design are errors.
        let cylinder = WarpParams {
            cone_bottom_mm: 90.0,
            ..params
        };
        assert_eq!(warp(&band, &cylinder).unwrap(), band);
        for bad in [f64::NAN, 0.0, -5.0] {
            assert!(
                warp(
                    &band,
                    &WarpParams {
                        cone_slant_mm: bad,
                        ..params
                    }
                )
                .is_err()
            );
        }
        let short = WarpParams {
            cone_slant_mm: 10.0,
            ..params
        };
        assert!(
            warp(&band, &short).is_err(),
            "slant shorter than the radius change"
        );
        let tall = rect(0.0, 0.0, 10.0, 500.0);
        assert!(warp(&tall, &params).is_err());
    }
}
