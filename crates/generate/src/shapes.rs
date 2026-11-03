//! Parametric basic shapes ("Flexishapes"): each is built in its own unit space and then
//! fitted to the requested box. Rectangle, rounded rectangle, ellipse, regular polygon and
//! star already exist as live VectorCraft primitives in the engine; these are the rest.

use std::f64::consts::{PI, TAU};

use kurbo::{Affine, Arc, BezPath, Point, Shape, Vec2};
use serde::{Deserialize, Serialize};

use crate::{MAX_MM, Result, bad, check_range, path_bounds};

/// Which triangle a [`ShapeKind::Triangle`] draws in its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TriangleKind {
    /// Apex at the top centre of the box.
    #[default]
    Isosceles,
    /// Right angle at the bottom-left corner.
    Right,
    /// Equal sides: the largest one that fits the box, centred.
    Equilateral,
}

/// A parametric shape and its parameters. Ratios are fractions of the box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ShapeKind {
    Heart,
    Arrow {
        /// Head length as a fraction of the width (0.05–0.95, default 0.4).
        #[serde(default = "d_0_4")]
        head_length: f64,
        /// Shaft thickness as a fraction of the height (0.05–1, default 0.4).
        #[serde(default = "d_0_4")]
        shaft_width: f64,
    },
    SpeechBubble {
        /// Tail height as a fraction of the box height (0.05–0.6, default 0.25).
        #[serde(default = "d_0_25")]
        tail_height: f64,
        /// Tail base width as a fraction of the width (0.05–0.5, default 0.2).
        #[serde(default = "d_0_2")]
        tail_width: f64,
        /// Body corner radius as a fraction of the smaller body side (0–0.5, default 0.15).
        #[serde(default = "d_0_15")]
        corner_radius: f64,
    },
    /// A donut: an ellipse with a concentric hole.
    Ring {
        /// Hole size as a fraction of the outer size (0.05–0.95, default 0.6).
        #[serde(default = "d_0_6")]
        inner_ratio: f64,
    },
    Crescent {
        /// Thickness at the middle as a fraction of the width (0.05–0.95, default 0.35).
        #[serde(default = "d_0_35")]
        thickness: f64,
    },
    Gear {
        /// Number of teeth (3–200, default 12).
        #[serde(default = "d_teeth")]
        teeth: u32,
        /// Tooth depth as a fraction of the radius (0.02–0.5, default 0.15).
        #[serde(default = "d_0_15")]
        tooth_depth: f64,
        /// Centre hole as a fraction of the radius (0 = none, up to 0.8).
        #[serde(default)]
        hole_ratio: f64,
    },
    /// An open Archimedean spiral from the centre outward.
    Spiral {
        /// Turns (0.5–100, default 3).
        #[serde(default = "d_turns")]
        turns: f64,
    },
    /// A ribbon banner with V notches at both ends.
    Banner {
        /// Notch depth as a fraction of the width (0–0.45, default 0.12).
        #[serde(default = "d_0_12")]
        notch: f64,
    },
    ScallopCircle {
        /// Number of scallops (3–360, default 12).
        #[serde(default = "d_scallops")]
        scallops: u32,
    },
    ScallopSquare {
        /// Scallops per side (1–100, default 4).
        #[serde(default = "d_scallops_side")]
        scallops: u32,
    },
    Triangle {
        #[serde(default)]
        variant: TriangleKind,
    },
}

fn d_0_4() -> f64 {
    0.4
}
fn d_0_25() -> f64 {
    0.25
}
fn d_0_2() -> f64 {
    0.2
}
fn d_0_15() -> f64 {
    0.15
}
fn d_0_6() -> f64 {
    0.6
}
fn d_0_35() -> f64 {
    0.35
}
fn d_0_12() -> f64 {
    0.12
}
fn d_teeth() -> u32 {
    12
}
fn d_turns() -> f64 {
    3.0
}
fn d_scallops() -> u32 {
    12
}
fn d_scallops_side() -> u32 {
    4
}

/// Builds `kind` fitted to the box `(x, y, width, height)` (millimetres, y up).
pub fn shape(kind: &ShapeKind, x: f64, y: f64, width: f64, height: f64) -> Result<BezPath> {
    for (v, what) in [(x, "x"), (y, "y")] {
        check_range(v, -MAX_MM, MAX_MM, what)?;
    }
    for (v, what) in [(width, "width"), (height, "height")] {
        crate::check_positive(v, MAX_MM, what)?;
    }
    let mut keep_aspect = false;
    let unit = match *kind {
        ShapeKind::Heart => heart(),
        ShapeKind::Arrow {
            head_length,
            shaft_width,
        } => {
            let h = check_range(head_length, 0.05, 0.95, "head_length")?;
            let s = check_range(shaft_width, 0.05, 1.0, "shaft_width")? / 2.0;
            polygon(&[
                (0.0, 0.5 - s),
                (1.0 - h, 0.5 - s),
                (1.0 - h, 0.0),
                (1.0, 0.5),
                (1.0 - h, 1.0),
                (1.0 - h, 0.5 + s),
                (0.0, 0.5 + s),
            ])
        }
        ShapeKind::SpeechBubble {
            tail_height,
            tail_width,
            corner_radius,
        } => speech_bubble(
            check_range(tail_height, 0.05, 0.6, "tail_height")?,
            check_range(tail_width, 0.05, 0.5, "tail_width")?,
            check_range(corner_radius, 0.0, 0.5, "corner_radius")?,
            width,
            height,
        ),
        ShapeKind::Ring { inner_ratio } => {
            let r = check_range(inner_ratio, 0.05, 0.95, "inner_ratio")?;
            let mut p = kurbo::Circle::new((0.0, 0.0), 1.0).to_path(1e-4);
            // The hole runs the other way so non-zero and even-odd agree.
            let hole = kurbo::Circle::new((0.0, 0.0), r)
                .to_path(1e-4)
                .reverse_subpaths();
            p.extend(hole);
            p
        }
        ShapeKind::Crescent { thickness } => {
            crescent(check_range(thickness, 0.05, 0.95, "thickness")? * 2.0)
        }
        ShapeKind::Gear {
            teeth,
            tooth_depth,
            hole_ratio,
        } => {
            if !(3..=200).contains(&teeth) {
                return bad(format!("teeth must be between 3 and 200 (got {teeth})"));
            }
            let depth = check_range(tooth_depth, 0.02, 0.5, "tooth_depth")?;
            let hole = check_range(hole_ratio, 0.0, 0.8, "hole_ratio")?;
            if hole >= 1.0 - depth - 0.05 {
                return bad("hole_ratio must leave material inside the tooth roots");
            }
            gear(teeth, depth, hole)
        }
        ShapeKind::Spiral { turns } => spiral(check_range(turns, 0.5, 100.0, "turns")?),
        ShapeKind::Banner { notch } => {
            let n = check_range(notch, 0.0, 0.45, "notch")?;
            polygon(&[
                (0.0, 0.0),
                (1.0, 0.0),
                (1.0 - n, 0.5),
                (1.0, 1.0),
                (0.0, 1.0),
                (n, 0.5),
            ])
        }
        ShapeKind::ScallopCircle { scallops } => {
            if !(3..=360).contains(&scallops) {
                return bad(format!(
                    "scallops must be between 3 and 360 (got {scallops})"
                ));
            }
            scallop_circle(scallops)
        }
        ShapeKind::ScallopSquare { scallops } => {
            if !(1..=100).contains(&scallops) {
                return bad(format!(
                    "scallops must be between 1 and 100 (got {scallops})"
                ));
            }
            scallop_square(scallops)
        }
        ShapeKind::Triangle { variant } => match variant {
            TriangleKind::Isosceles => polygon(&[(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)]),
            TriangleKind::Right => polygon(&[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]),
            TriangleKind::Equilateral => {
                keep_aspect = true;
                polygon(&[(0.0, 0.0), (1.0, 0.0), (0.5, 3f64.sqrt() / 2.0)])
            }
        },
    };
    fit(&unit, x, y, width, height, keep_aspect)
}

/// Scales `path`'s bounds onto the box (keeping aspect: as wide as fits, centred).
fn fit(path: &BezPath, x: f64, y: f64, w: f64, h: f64, keep_aspect: bool) -> Result<BezPath> {
    let b = path_bounds(path)?;
    if b.width() <= 0.0 || b.height() <= 0.0 {
        return bad("shape collapsed to nothing");
    }
    let (mut sx, mut sy) = (w / b.width(), h / b.height());
    let (mut ox, mut oy) = (x, y);
    if keep_aspect {
        let s = sx.min(sy);
        ox += (w - b.width() * s) / 2.0;
        oy += (h - b.height() * s) / 2.0;
        (sx, sy) = (s, s);
    }
    Ok(Affine::translate((ox, oy))
        * Affine::scale_non_uniform(sx, sy)
        * Affine::translate((-b.x0, -b.y0))
        * path.clone())
}

fn polygon(points: &[(f64, f64)]) -> BezPath {
    let pts: Vec<Point> = points.iter().map(|&(x, y)| Point::new(x, y)).collect();
    let mut p = BezPath::new();
    crate::polyline_path(&pts, true, &mut p);
    p
}

fn heart() -> BezPath {
    let mut p = BezPath::new();
    p.move_to((0.0, -1.0));
    p.curve_to((-0.2, -0.6), (-1.0, -0.3), (-1.0, 0.35));
    p.curve_to((-1.0, 0.9), (-0.35, 1.15), (0.0, 0.6));
    p.curve_to((0.35, 1.15), (1.0, 0.9), (1.0, 0.35));
    p.curve_to((1.0, -0.3), (0.2, -0.6), (0.0, -1.0));
    p.close_path();
    p
}

/// A rounded body over a tail at the lower left, built in millimetres so the corner
/// radius stays circular.
fn speech_bubble(tail_h: f64, tail_w: f64, radius: f64, w: f64, h: f64) -> BezPath {
    let body_y = h * tail_h;
    let body_h = h - body_y;
    let r = radius * w.min(body_h);
    let k = 0.552_284_75 * r;
    let (tx0, tx1) = (w * 0.15, w * (0.15 + tail_w));
    let mut p = BezPath::new();
    p.move_to((r, body_y));
    p.line_to((tx0.max(r), body_y));
    p.line_to((tx0 * 0.6, 0.0));
    p.line_to((tx1.min(w - r).max(tx0.max(r)), body_y));
    p.line_to((w - r, body_y));
    p.curve_to((w - r + k, body_y), (w, body_y + r - k), (w, body_y + r));
    p.line_to((w, h - r));
    p.curve_to((w, h - r + k), (w - r + k, h), (w - r, h));
    p.line_to((r, h));
    p.curve_to((r - k, h), (0.0, h - r + k), (0.0, h - r));
    p.line_to((0.0, body_y + r));
    p.curve_to((0.0, body_y + r - k), (r - k, body_y), (r, body_y));
    p.close_path();
    p
}

/// Unit circle minus a unit circle shifted right by `d` (0 < d < 2).
fn crescent(d: f64) -> BezPath {
    let half = d / 2.0;
    let h = (1.0 - half * half).sqrt();
    let alpha = h.atan2(half);
    let gamma = h.atan2(half);
    let mut p = BezPath::new();
    p.move_to((half, h));
    let outer = Arc::new(
        (0.0, 0.0),
        Vec2::new(1.0, 1.0),
        alpha,
        TAU - 2.0 * alpha,
        0.0,
    );
    outer.to_cubic_beziers(1e-5, |a, b, c| p.curve_to(a, b, c));
    let inner = Arc::new((d, 0.0), Vec2::new(1.0, 1.0), PI + gamma, -2.0 * gamma, 0.0);
    inner.to_cubic_beziers(1e-5, |a, b, c| p.curve_to(a, b, c));
    p.close_path();
    p
}

fn gear(teeth: u32, depth: f64, hole: f64) -> BezPath {
    let n = teeth as usize;
    let step = TAU / n as f64;
    let (outer, root) = (1.0, 1.0 - depth);
    let mut pts = Vec::with_capacity(n * 4);
    for i in 0..n {
        let a = i as f64 * step;
        // root 0.25, flank 0.15, tip 0.35, flank 0.25 of each pitch.
        for (frac, r) in [(0.0, root), (0.25, root), (0.4, outer), (0.75, outer)] {
            let t = a + frac * step;
            pts.push(Point::new(r * t.cos(), r * t.sin()));
        }
    }
    let mut p = BezPath::new();
    crate::polyline_path(&pts, true, &mut p);
    if hole > 0.0 {
        p.extend(
            kurbo::Circle::new((0.0, 0.0), hole)
                .to_path(1e-4)
                .reverse_subpaths(),
        );
    }
    p
}

fn spiral(turns: f64) -> BezPath {
    let steps = (turns * 64.0).ceil() as usize;
    let mut p = BezPath::new();
    p.move_to((0.0, 0.0));
    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        let a = t * turns * TAU;
        p.line_to((t * a.cos(), t * a.sin()));
    }
    p
}

fn scallop_circle(n: u32) -> BezPath {
    let half = PI / n as f64;
    let (chord_mid, r) = (half.cos(), half.sin());
    let mut p = BezPath::new();
    for i in 0..n {
        let theta = i as f64 * 2.0 * half;
        let c = Point::new(chord_mid * theta.cos(), chord_mid * theta.sin());
        let arc = Arc::new(c, Vec2::new(r, r), theta - PI / 2.0, PI, 0.0);
        if i == 0 {
            p.move_to(arc.center + Vec2::from_angle(arc.start_angle) * r);
        }
        arc.to_cubic_beziers(1e-5, |a, b, e| p.curve_to(a, b, e));
    }
    p.close_path();
    p
}

fn scallop_square(n: u32) -> BezPath {
    let r = 1.0 / n as f64;
    let mut p = BezPath::new();
    p.move_to((0.0, 0.0));
    // Sides counter-clockwise: bottom, right, top, left; each bump bulges outward.
    let sides = [
        (Point::new(0.0, 0.0), Vec2::new(2.0, 0.0)),
        (Point::new(2.0, 0.0), Vec2::new(0.0, 2.0)),
        (Point::new(2.0, 2.0), Vec2::new(-2.0, 0.0)),
        (Point::new(0.0, 2.0), Vec2::new(0.0, -2.0)),
    ];
    for (start, dir) in sides {
        let unit = dir / 2.0;
        let start_angle = unit.atan2() + PI;
        for i in 0..n {
            let c = start + unit * (r * (2 * i + 1) as f64);
            // Sweeping counter-clockwise from behind puts the bump on the outer (right) side.
            Arc::new(c, Vec2::new(r, r), start_angle, PI, 0.0)
                .to_cubic_beziers(1e-5, |a, b, e| p.curve_to(a, b, e));
        }
    }
    p.close_path();
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::self_intersects;
    use crate::{flatten, polygons};

    fn all_kinds() -> Vec<ShapeKind> {
        let j = |s: &str| serde_json::from_str::<ShapeKind>(s).unwrap();
        [
            r#"{"kind":"heart"}"#,
            r#"{"kind":"arrow"}"#,
            r#"{"kind":"speech-bubble"}"#,
            r#"{"kind":"ring"}"#,
            r#"{"kind":"crescent"}"#,
            r#"{"kind":"gear","teeth":20,"hole_ratio":0.3}"#,
            r#"{"kind":"spiral","turns":4}"#,
            r#"{"kind":"banner"}"#,
            r#"{"kind":"scallop-circle","scallops":9}"#,
            r#"{"kind":"scallop-square","scallops":3}"#,
            r#"{"kind":"triangle","variant":"right"}"#,
            r#"{"kind":"triangle","variant":"equilateral"}"#,
            r#"{"kind":"triangle"}"#,
        ]
        .iter()
        .map(|s| j(s))
        .collect()
    }

    #[test]
    fn every_shape_fits_its_box_closes_and_does_not_self_intersect() {
        for kind in all_kinds() {
            let p = shape(&kind, 10.0, 20.0, 40.0, 30.0).unwrap();
            let b = p.bounding_box();
            let eq = matches!(
                kind,
                ShapeKind::Triangle {
                    variant: TriangleKind::Equilateral
                }
            );
            if eq {
                assert!(b.x0 >= 10.0 && b.x1 <= 50.0 && (b.y1 - b.y0 - 30.0).abs() < 1e-6);
            } else {
                assert!(
                    (b.x0 - 10.0).abs() < 1e-6 && (b.x1 - 50.0).abs() < 1e-6,
                    "{kind:?} {b:?}"
                );
                assert!(
                    (b.y0 - 20.0).abs() < 1e-6 && (b.y1 - 50.0).abs() < 1e-6,
                    "{kind:?} {b:?}"
                );
            }
            let contours = flatten(&p).unwrap();
            let open = matches!(kind, ShapeKind::Spiral { .. });
            for c in &contours {
                assert_eq!(c.closed, !open, "{kind:?}");
                if !open {
                    assert!(!self_intersects(&c.points), "{kind:?} self-intersects");
                }
            }
        }
    }

    #[test]
    fn ring_and_gear_hole_counts() {
        let ring = shape(&ShapeKind::Ring { inner_ratio: 0.5 }, 0.0, 0.0, 10.0, 10.0).unwrap();
        let polys = polygons(&ring).unwrap();
        assert_eq!(polys.len(), 2);
        assert!(!crate::inside(&polys, Point::new(5.0, 5.0)));
        assert!(crate::inside(&polys, Point::new(5.0, 9.0)));
        let gear = shape(
            &ShapeKind::Gear {
                teeth: 10,
                tooth_depth: 0.2,
                hole_ratio: 0.0,
            },
            0.0,
            0.0,
            10.0,
            10.0,
        )
        .unwrap();
        assert_eq!(polygons(&gear).unwrap()[0].len(), 40);
    }

    #[test]
    fn equilateral_has_equal_sides() {
        let p = shape(
            &ShapeKind::Triangle {
                variant: TriangleKind::Equilateral,
            },
            0.0,
            0.0,
            30.0,
            100.0,
        )
        .unwrap();
        let pts = &polygons(&p).unwrap()[0];
        let s: Vec<f64> = (0..3).map(|i| pts[i].distance(pts[(i + 1) % 3])).collect();
        assert!((s[0] - s[1]).abs() < 1e-9 && (s[1] - s[2]).abs() < 1e-9);
    }

    #[test]
    fn bad_parameters_are_refused() {
        let gear = ShapeKind::Gear {
            teeth: 2,
            tooth_depth: 0.1,
            hole_ratio: 0.0,
        };
        assert!(shape(&gear, 0.0, 0.0, 10.0, 10.0).is_err());
        assert!(shape(&ShapeKind::Heart, 0.0, 0.0, 0.0, 10.0).is_err());
        assert!(shape(&ShapeKind::Heart, f64::NAN, 0.0, 5.0, 10.0).is_err());
        assert!(shape(&ShapeKind::Ring { inner_ratio: 1.5 }, 0.0, 0.0, 5.0, 5.0).is_err());
        assert!(shape(&ShapeKind::Spiral { turns: 1e9 }, 0.0, 0.0, 5.0, 5.0).is_err());
    }
}
