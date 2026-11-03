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
}

impl WarpStyle {
    pub const ALL: [WarpStyle; 16] = [
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
}

impl Default for WarpParams {
    fn default() -> Self {
        Self {
            style: WarpStyle::Arc,
            bend: 0.5,
            horizontal: 0.0,
            vertical: 0.0,
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
    }
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
        for style in WarpStyle::ALL {
            let p = WarpParams {
                style,
                bend: 0.0,
                horizontal: 0.0,
                vertical: 0.0,
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
}
