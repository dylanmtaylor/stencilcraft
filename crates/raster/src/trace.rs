//! Raster → closed cut contours. Our own pipeline, in pixel space:
//!
//! 1. foreground mask: luminance (alpha composited on white) after an optional high-pass
//!    (edge) and low-pass (blur) filter, under `threshold`; or, in colour-range mode,
//!    opaque pixels whose hue falls in a range;
//! 2. despeckle: foreground islands and background pinholes smaller than `despeckle`
//!    pixels are dropped; outer-edge mode also fills every hole;
//! 3. boundary following along pixel edges with the foreground on the right, turning
//!    right first at diagonal touches (foreground is 4-connected, the same rule the
//!    despeckle labelling uses), so every contour closes;
//! 4. collinear and Douglas–Peucker simplification at `smoothing` pixels;
//! 5. [`contour_path`] keeps vertices that turn at least `corner_threshold` degrees as
//!    corners and rounds the rest with quadratic curves through edge midpoints.
//!
//! Orientation: with y down (document coordinates), outer contours have positive
//! shoelace area and holes negative.

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use stencilcraft_geom::{BezPath, Point};

use crate::{RasterError, bad};

/// Most boundary edges one trace may follow; more is an error asking for despeckle/blur.
const MAX_EDGES: usize = 8_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TraceMode {
    /// Every boundary: outlines and the holes inside them.
    #[default]
    AllEdges,
    /// Outlines only; holes are filled.
    OuterEdge,
    /// Every boundary, and the traced pixels are removed from the image.
    Detach,
}

/// Select foreground by colour instead of brightness.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColourRange {
    /// Hue range in degrees (0–360); `hue_min > hue_max` wraps through red.
    pub hue_min: f64,
    pub hue_max: f64,
    /// Minimum HSV saturation, percent (0–100), so greys are not matched by hue noise.
    #[serde(default)]
    pub min_saturation: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TraceOptions {
    /// Percent of white (0–100): darker pixels are foreground.
    pub threshold: f64,
    /// Islands and pinholes below this area (pixels) are removed.
    pub despeckle: u32,
    /// Low-pass (blur) radius in pixels, 0–50.
    pub low_pass: f64,
    /// High-pass (edge) radius in pixels, 0–50; 0 is off.
    pub high_pass: f64,
    /// Colour-range mode; brightness threshold and filters are ignored when set.
    pub colour_range: Option<ColourRange>,
    /// Simplification tolerance in pixels, 0–50 (0 keeps the pixel staircase).
    pub smoothing: f64,
    /// Vertices turning at least this many degrees (0–180) stay sharp corners.
    pub corner_threshold: f64,
    pub mode: TraceMode,
}

impl Default for TraceOptions {
    fn default() -> Self {
        Self {
            threshold: 50.0,
            despeckle: 4,
            low_pass: 0.0,
            high_pass: 0.0,
            colour_range: None,
            smoothing: 1.0,
            corner_threshold: 60.0,
            mode: TraceMode::AllEdges,
        }
    }
}

impl TraceOptions {
    pub fn validate(&self) -> Result<(), RasterError> {
        let ranges = [
            (self.threshold, 0.0, 100.0, "threshold"),
            (self.low_pass, 0.0, 50.0, "low_pass"),
            (self.high_pass, 0.0, 50.0, "high_pass"),
            (self.smoothing, 0.0, 50.0, "smoothing"),
            (self.corner_threshold, 0.0, 180.0, "corner_threshold"),
        ];
        let colour = self.colour_range.map_or([None; 3], |r| {
            [
                Some((r.hue_min, 0.0, 360.0, "hue_min")),
                Some((r.hue_max, 0.0, 360.0, "hue_max")),
                Some((r.min_saturation, 0.0, 100.0, "min_saturation")),
            ]
        });
        for (value, lo, hi, name) in ranges.into_iter().chain(colour.into_iter().flatten()) {
            if !(value.is_finite() && (lo..=hi).contains(&value)) {
                return Err(bad(format!("trace: {name} {value} is outside {lo}–{hi}")));
            }
        }
        Ok(())
    }
}

/// One closed contour in pixel coordinates (start point not repeated).
#[derive(Debug, Clone, PartialEq)]
pub struct Contour {
    pub points: Vec<Point>,
    pub hole: bool,
}

#[derive(Debug, Clone)]
pub struct TraceOutput {
    pub contours: Vec<Contour>,
    /// Detach mode: the image with the traced pixels made transparent.
    pub detached: Option<RgbaImage>,
}

pub fn trace(pixels: &RgbaImage, options: &TraceOptions) -> Result<TraceOutput, RasterError> {
    options.validate()?;
    let (w, h) = (pixels.width() as usize, pixels.height() as usize);
    if w == 0 || h == 0 {
        return Ok(TraceOutput {
            contours: Vec::new(),
            detached: None,
        });
    }
    let mut mask = foreground(pixels, options);
    despeckle(
        &mut mask,
        w,
        h,
        options.despeckle as usize,
        options.mode == TraceMode::OuterEdge,
    );
    let detached = (options.mode == TraceMode::Detach).then(|| {
        let mut out = pixels.clone();
        for (pixel, &fg) in out.pixels_mut().zip(&mask) {
            if fg {
                pixel[3] = 0;
            }
        }
        out
    });
    let contours = follow_boundaries(&mask, w, h)?
        .into_iter()
        .filter_map(|points| {
            let points = simplify(points, options.smoothing);
            (points.len() >= 3).then(|| Contour {
                hole: stencilcraft_geom::signed_area(&points) < 0.0,
                points,
            })
        })
        .collect();
    Ok(TraceOutput { contours, detached })
}

fn foreground(pixels: &RgbaImage, options: &TraceOptions) -> Vec<bool> {
    if let Some(range) = options.colour_range {
        return pixels
            .pixels()
            .map(|p| {
                let (hue, saturation) = hue_saturation(p[0], p[1], p[2]);
                let in_hue = if range.hue_min <= range.hue_max {
                    (range.hue_min..=range.hue_max).contains(&hue)
                } else {
                    hue >= range.hue_min || hue <= range.hue_max
                };
                p[3] >= 128
                    && saturation > 0.0
                    && saturation * 100.0 >= range.min_saturation
                    && in_hue
            })
            .collect();
    }
    let (w, h) = (pixels.width() as usize, pixels.height() as usize);
    let mut lum: Vec<f32> = pixels
        .pixels()
        .map(|p| {
            let y = (0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2]))
                / 255.0;
            let a = f32::from(p[3]) / 255.0;
            y * a + (1.0 - a)
        })
        .collect();
    let radius = |r: f64| r.round() as usize;
    if radius(options.high_pass) > 0 {
        let low = blur(&lum, w, h, radius(options.high_pass));
        for (value, low) in lum.iter_mut().zip(low) {
            *value = (0.5 + *value - low).clamp(0.0, 1.0);
        }
    }
    if radius(options.low_pass) > 0 {
        lum = blur(&lum, w, h, radius(options.low_pass));
    }
    let threshold = (options.threshold / 100.0) as f32;
    lum.into_iter().map(|value| value < threshold).collect()
}

fn hue_saturation(r: u8, g: u8, b: u8) -> (f64, f64) {
    let (r, g, b) = (f64::from(r), f64::from(g), f64::from(b));
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    if delta <= 0.0 {
        return (0.0, 0.0);
    }
    let hue = if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    (hue, delta / max)
}

/// Two separable box-blur passes (close to a Gaussian), edges clamped.
fn blur(values: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    let mut out = values.to_vec();
    for _ in 0..2 {
        out = box_pass(&out, w, h, radius, 1, w);
        out = box_pass(&out, h, w, radius, w, 1);
    }
    out
}

/// Box filter along lines of `len` samples spaced `step`, lines `line_step` apart.
fn box_pass(
    values: &[f32],
    len: usize,
    lines: usize,
    radius: usize,
    step: usize,
    line_step: usize,
) -> Vec<f32> {
    let mut out = vec![0.0; values.len()];
    let at = |line: usize, i: isize| -> f32 {
        let i = i.clamp(0, len as isize - 1) as usize;
        values
            .get(line * line_step + i * step)
            .copied()
            .unwrap_or(1.0)
    };
    let r = radius as isize;
    let width = (2 * r + 1) as f32;
    for line in 0..lines {
        let mut sum: f32 = (-r..=r).map(|i| at(line, i)).sum();
        for i in 0..len as isize {
            if let Some(slot) = out.get_mut(line * line_step + i as usize * step) {
                *slot = sum / width;
            }
            sum += at(line, i + r + 1) - at(line, i - r);
        }
    }
    out
}

/// Remove 4-connected regions below `min_area`: foreground islands become background
/// and enclosed background pinholes become foreground. `fill_holes` fills every
/// enclosed background region.
fn despeckle(mask: &mut [bool], w: usize, h: usize, min_area: usize, fill_holes: bool) {
    let mut seen = vec![false; mask.len()];
    let mut region = Vec::new();
    let mut stack = Vec::new();
    for start in 0..mask.len() {
        if seen[start] {
            continue;
        }
        let value = mask[start];
        let mut touches_border = false;
        region.clear();
        seen[start] = true;
        stack.push(start);
        while let Some(i) = stack.pop() {
            region.push(i);
            let (x, y) = (i % w, i / w);
            touches_border |= x == 0 || y == 0 || x + 1 == w || y + 1 == h;
            let neighbours = [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then(|| i + 1),
                (y > 0).then(|| i - w),
                (y + 1 < h).then(|| i + w),
            ];
            for n in neighbours.into_iter().flatten() {
                if !seen[n] && mask[n] == value {
                    seen[n] = true;
                    stack.push(n);
                }
            }
        }
        let small = region.len() < min_area;
        let flip = if value {
            small
        } else {
            !touches_border && (small || fill_holes)
        };
        if flip {
            for &i in &region {
                mask[i] = !value;
            }
        }
    }
}

/// Pixel-edge boundaries of the mask as closed vertex loops (start not repeated).
fn follow_boundaries(mask: &[bool], w: usize, h: usize) -> Result<Vec<Vec<Point>>, RasterError> {
    let fg = |x: isize, y: isize| -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < w
            && (y as usize) < h
            && mask[y as usize * w + x as usize]
    };
    let gw = w + 1;
    // Outgoing edge directions per lattice vertex: bit 0 east, 1 south, 2 west, 3 north.
    let mut edges = vec![0_u8; gw * (h + 1)];
    let mut total = 0_usize;
    for y in 0..h as isize {
        for x in 0..w as isize {
            if !fg(x, y) {
                continue;
            }
            let (ux, uy) = (x as usize, y as usize);
            for (open, vertex, dir) in [
                (!fg(x, y - 1), uy * gw + ux, 0),
                (!fg(x + 1, y), uy * gw + ux + 1, 1),
                (!fg(x, y + 1), (uy + 1) * gw + ux + 1, 2),
                (!fg(x - 1, y), (uy + 1) * gw + ux, 3),
            ] {
                if open {
                    edges[vertex] |= 1 << dir;
                    total += 1;
                }
            }
            if total > MAX_EDGES {
                return Err(bad(
                    "trace: the image is too detailed (over 8 million boundary edges); raise despeckle or blur",
                ));
            }
        }
    }
    let step = |vertex: usize, dir: u8| -> usize {
        match dir {
            0 => vertex + 1,
            1 => vertex + gw,
            2 => vertex - 1,
            _ => vertex - gw,
        }
    };
    // Each edge has exactly one successor (turn right first, then straight, then left;
    // at a diagonal touch the two arrivals turn to different exits), so walking from
    // any edge returns to it. `remaining` marks edges not yet on a contour.
    let mut remaining = edges.clone();
    let mut contours = Vec::new();
    for start in 0..edges.len() {
        while remaining[start] != 0 {
            let start_dir = remaining[start].trailing_zeros() as u8;
            let (mut vertex, mut dir) = (start, start_dir);
            let mut points = Vec::new();
            loop {
                points.push(Point::new((vertex % gw) as f64, (vertex / gw) as f64));
                remaining[vertex] &= !(1 << dir);
                vertex = step(vertex, dir);
                let Some(next) = [1_u8, 0, 3]
                    .iter()
                    .map(|t| (dir + t) % 4)
                    .find(|d| edges[vertex] & (1 << d) != 0)
                else {
                    return Err(bad("trace: an open boundary (internal error)"));
                };
                if vertex == start && next == start_dir {
                    break;
                }
                dir = next;
                if points.len() > total {
                    return Err(bad("trace: a boundary did not close (internal error)"));
                }
            }
            contours.push(points);
        }
    }
    Ok(contours)
}

/// Drop collinear lattice vertices, then Douglas–Peucker on the closed loop.
fn simplify(points: Vec<Point>, tolerance: f64) -> Vec<Point> {
    let n = points.len();
    let mut corners: Vec<Point> = (0..n)
        .filter_map(|i| {
            let prev = points[(i + n - 1) % n];
            let cur = points[i];
            let next = points[(i + 1) % n];
            let cross = (cur - prev).cross(next - cur);
            (cross != 0.0).then_some(cur)
        })
        .collect();
    if tolerance <= 0.0 || corners.len() <= 4 {
        return corners;
    }
    // Split the loop at its first point and the point farthest from it.
    let far = (0..corners.len())
        .max_by(|&a, &b| {
            let da = corners[a].distance_squared(corners[0]);
            let db = corners[b].distance_squared(corners[0]);
            da.total_cmp(&db)
        })
        .unwrap_or(0);
    corners.push(corners[0]);
    let mut keep = vec![false; corners.len()];
    keep[0] = true;
    keep[far] = true;
    let last = corners.len() - 1;
    keep[last] = true;
    let mut stack = vec![(0, far), (far, last)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (corners[a], corners[b]);
        let (mut worst, mut worst_d) = (a, 0.0);
        for (i, p) in corners.iter().enumerate().take(b).skip(a + 1) {
            let d = segment_distance(*p, pa, pb);
            if d > worst_d {
                worst = i;
                worst_d = d;
            }
        }
        if worst_d > tolerance {
            keep[worst] = true;
            stack.push((a, worst));
            stack.push((worst, b));
        }
    }
    corners.pop();
    corners
        .into_iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(p))
        .collect()
}

fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let len2 = ab.hypot2();
    if len2 <= 0.0 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// A contour as a closed path through `map` (pixel → document), with corners kept
/// sharp and gentler vertices rounded by quadratic curves through edge midpoints.
pub fn contour_path(
    contour: &Contour,
    corner_threshold: f64,
    map: impl Fn(Point) -> Point,
) -> BezPath {
    let p = &contour.points;
    let n = p.len();
    let mut path = BezPath::new();
    if n < 3 {
        return path;
    }
    let corner = |i: usize| -> bool {
        let a = p[i] - p[(i + n - 1) % n];
        let b = p[(i + 1) % n] - p[i];
        let turn = a.cross(b).atan2(a.dot(b)).abs().to_degrees();
        turn >= corner_threshold
    };
    let corners: Vec<bool> = (0..n).map(corner).collect();
    if corners.iter().all(|c| *c) {
        path.move_to(map(p[0]));
        for q in &p[1..] {
            path.line_to(map(*q));
        }
        path.close_path();
        return path;
    }
    let mid = |i: usize| map(p[i].midpoint(p[(i + 1) % n]));
    path.move_to(mid(n - 1));
    for (i, &sharp) in corners.iter().enumerate() {
        if sharp {
            path.line_to(map(p[i]));
            path.line_to(mid(i));
        } else {
            path.quad_to(map(p[i]), mid(i));
        }
    }
    path.close_path();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    const BLACK: Rgba<u8> = Rgba([0, 0, 0, 255]);
    const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);

    fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> bool) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| if f(x, y) { BLACK } else { WHITE })
    }

    fn near(p: Point, x: f64, y: f64) -> bool {
        (p.x - x).abs() <= 1.0 && (p.y - y).abs() <= 1.0
    }

    #[test]
    fn filled_square_is_one_contour_with_four_corners() {
        let img = image(40, 40, |x, y| {
            (10..30).contains(&x) && (10..30).contains(&y)
        });
        let out = trace(&img, &TraceOptions::default()).unwrap();
        assert_eq!(out.contours.len(), 1);
        let c = &out.contours[0];
        assert!(!c.hole);
        assert_eq!(c.points.len(), 4, "{:?}", c.points);
        for (x, y) in [(10.0, 10.0), (30.0, 10.0), (30.0, 30.0), (10.0, 30.0)] {
            assert!(
                c.points.iter().any(|p| near(*p, x, y)),
                "corner {x},{y}: {:?}",
                c.points
            );
        }
        assert!(
            stencilcraft_geom::signed_area(&c.points) > 0.0,
            "outer is positive"
        );
        let path = contour_path(c, 60.0, |p| p);
        assert_eq!(
            stencilcraft_geom::path_to_polylines(&path, 4)[0]
                .points
                .len(),
            4
        );
    }

    #[test]
    fn ring_gives_outer_and_hole_and_outer_mode_drops_the_hole() {
        let ring = |x: u32, y: u32| {
            let d = (x as f64 - 20.0).hypot(y as f64 - 20.0);
            (8.0..16.0).contains(&d)
        };
        let img = image(40, 40, ring);
        let out = trace(&img, &TraceOptions::default()).unwrap();
        assert_eq!(out.contours.len(), 2);
        assert_eq!(out.contours.iter().filter(|c| c.hole).count(), 1);
        let outer = trace(
            &img,
            &TraceOptions {
                mode: TraceMode::OuterEdge,
                ..TraceOptions::default()
            },
        )
        .unwrap();
        assert_eq!(outer.contours.len(), 1);
        assert!(!outer.contours[0].hole);
        // A smooth ring is rounded, not left as a staircase.
        let path = contour_path(&out.contours[0], 60.0, |p| p);
        assert!(
            path.elements()
                .iter()
                .any(|e| matches!(e, kurbo::PathEl::QuadTo(..)))
        );
    }

    #[test]
    fn despeckle_removes_noise_and_pinholes() {
        let img = image(40, 40, |x, y| {
            let square = (10..30).contains(&x) && (10..30).contains(&y);
            let pinhole = x == 20 && y == 20;
            let speck = (x == 2 && y == 2) || (x == 35 && (35..37).contains(&y));
            (square && !pinhole) || speck
        });
        let noisy = trace(
            &img,
            &TraceOptions {
                despeckle: 0,
                ..TraceOptions::default()
            },
        )
        .unwrap();
        assert_eq!(noisy.contours.len(), 4);
        let clean = trace(
            &img,
            &TraceOptions {
                despeckle: 3,
                ..TraceOptions::default()
            },
        )
        .unwrap();
        assert_eq!(clean.contours.len(), 1);
    }

    #[test]
    fn diagonal_touching_pixels_close_separately() {
        let img = image(4, 4, |x, y| (x, y) == (1, 1) || (x, y) == (2, 2));
        let out = trace(
            &img,
            &TraceOptions {
                despeckle: 0,
                ..TraceOptions::default()
            },
        )
        .unwrap();
        assert_eq!(out.contours.len(), 2);
        assert!(out.contours.iter().all(|c| c.points.len() == 4 && !c.hole));
        // And the inverse: a background saddle inside foreground.
        let img = image(4, 4, |x, y| !((x, y) == (1, 1) || (x, y) == (2, 2)));
        let out = trace(
            &img,
            &TraceOptions {
                despeckle: 0,
                ..TraceOptions::default()
            },
        )
        .unwrap();
        assert_eq!(out.contours.iter().filter(|c| c.hole).count(), 1);
    }

    #[test]
    fn threshold_colour_range_filters_and_detach() {
        let grey = RgbaImage::from_pixel(5, 5, Rgba([100, 100, 100, 255]));
        let at = |t: f64| {
            trace(
                &grey,
                &TraceOptions {
                    threshold: t,
                    ..TraceOptions::default()
                },
            )
            .unwrap()
            .contours
            .len()
        };
        assert_eq!(at(30.0), 0);
        assert_eq!(at(50.0), 1);
        // Red square on blue: only the red is in the 340–20 hue range.
        let img = RgbaImage::from_fn(20, 20, |x, y| {
            if (5..15).contains(&x) && (5..15).contains(&y) {
                Rgba([220, 10, 10, 255])
            } else {
                Rgba([10, 10, 220, 255])
            }
        });
        let range = ColourRange {
            hue_min: 340.0,
            hue_max: 20.0,
            min_saturation: 50.0,
        };
        let out = trace(
            &img,
            &TraceOptions {
                colour_range: Some(range),
                mode: TraceMode::Detach,
                ..TraceOptions::default()
            },
        )
        .unwrap();
        assert_eq!(out.contours.len(), 1);
        let detached = out.detached.unwrap();
        assert_eq!(detached.get_pixel(10, 10)[3], 0);
        assert_eq!(detached.get_pixel(0, 0)[3], 255);
        // High-pass keeps edges of a large dark area, so the result is a band: outer + hole.
        let big = image(60, 60, |x, y| {
            (10..50).contains(&x) && (10..50).contains(&y)
        });
        let edges = trace(
            &big,
            &TraceOptions {
                high_pass: 3.0,
                threshold: 45.0,
                ..TraceOptions::default()
            },
        )
        .unwrap();
        assert!(
            edges.contours.iter().any(|c| c.hole),
            "{}",
            edges.contours.len()
        );
        let blurred = trace(
            &big,
            &TraceOptions {
                low_pass: 4.0,
                ..TraceOptions::default()
            },
        )
        .unwrap();
        assert_eq!(blurred.contours.len(), 1);
        assert!(
            TraceOptions {
                threshold: 101.0,
                ..TraceOptions::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            TraceOptions {
                smoothing: f64::NAN,
                ..TraceOptions::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn full_and_empty_images_are_fine() {
        assert!(
            trace(&image(3, 3, |_, _| false), &TraceOptions::default())
                .unwrap()
                .contours
                .is_empty()
        );
        let all = trace(&image(3, 3, |_, _| true), &TraceOptions::default()).unwrap();
        assert_eq!(all.contours.len(), 1);
    }
}
