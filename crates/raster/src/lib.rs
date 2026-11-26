//! Raster images for StencilCraft: bounded decoding of untrusted PNG, JPEG, BMP, GIF,
//! TIFF and WebP files, placing them in a document, tracing them into cut paths
//! (`trace`) and removing a flat background. (Rendering a document to PNG/JPEG is an
//! export, so it lives in `stencilcraft-import`, keeping the engine format-free.)
//!
//! Every decoder runs under byte, side, pixel and allocation caps, so a hostile file is
//! an error, never a panic or an out-of-memory abort (`AGENTS.md` › Never crash).

mod palette;
mod trace;

/// The RGBA pixel buffer the raster functions take and return.
pub use craft_raster_codecs::common::Pixels as RgbaImage;
use craft_raster_codecs::common::{self, DecodeLimits};
use stencilcraft_doc::RasterImage;

pub use palette::{MAX_COLOURS, colour_masks, hex, palette, reduce_colours};

mod corners;
pub use corners::detect_page_corners;
pub use trace::{
    ColourRange, Contour, TraceMode, TraceOptions, TraceOutput, contour_path, has_transparency,
    trace, trace_mask,
};

/// Largest encoded file accepted.
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
/// Largest side, in pixels.
pub const MAX_SIDE: u32 = 10_000;
/// Largest image, in pixels.
pub const MAX_PIXELS: u64 = 16_000_000;
const MAX_ALLOC: u64 = 256 * 1024 * 1024;

fn decode_limits() -> DecodeLimits {
    DecodeLimits {
        max_file_bytes: MAX_FILE_BYTES,
        max_width: MAX_SIDE,
        max_height: MAX_SIDE,
        max_pixels: MAX_PIXELS,
        max_alloc_bytes: MAX_ALLOC,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RasterError {
    #[error("image: {0}")]
    Bad(String),
}

fn bad(message: impl Into<String>) -> RasterError {
    RasterError::Bad(message.into())
}

/// Decode an image file under the caps. Animated GIFs give their first frame.
pub fn decode(bytes: &[u8]) -> Result<RgbaImage, RasterError> {
    common::decode_rgba(bytes, decode_limits()).map_err(|e| bad(e.to_string()))
}

/// Encode pixels as base64 PNG, the document's image storage.
pub fn encode_png_base64(pixels: &RgbaImage) -> Result<String, RasterError> {
    common::encode_png_base64(pixels).map_err(|e| bad(e.to_string()))
}

/// Decode a placed image's pixels (document data is untrusted too).
pub fn image_pixels(image: &RasterImage) -> Result<RgbaImage, RasterError> {
    let pixels = common::decode_png_base64(&image.png_base64, decode_limits())
        .map_err(|e| bad(format!("image {}: bad pixel data: {e}", image.id)))?;
    if pixels.dimensions() != (image.width_px, image.height_px) {
        return Err(bad(format!(
            "image {}: pixel data does not match its {}×{} size",
            image.id, image.width_px, image.height_px
        )));
    }
    Ok(pixels)
}

/// The pixels of `image` inside the document rectangle `area` (millimetres: x, y, width,
/// height), with the pixel offset of the crop's top-left. `None` when the area misses the
/// image or is not finite.
pub fn crop_to_area(
    image: &RasterImage,
    pixels: &RgbaImage,
    area: [f64; 4],
) -> Option<(RgbaImage, u32, u32)> {
    let [x, y, w, h] = area;
    if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return None;
    }
    if !(image.width_mm > 0.0 && image.height_mm > 0.0) {
        return None;
    }
    let (sx, sy) = (
        f64::from(image.width_px) / image.width_mm,
        f64::from(image.height_px) / image.height_mm,
    );
    let to_px = |v: f64, scale: f64, max: u32| ((v * scale).floor()).clamp(0.0, f64::from(max));
    let to_px_up = |v: f64, scale: f64, max: u32| ((v * scale).ceil()).clamp(0.0, f64::from(max));
    let x0 = to_px(x - image.x_mm, sx, image.width_px);
    let y0 = to_px(y - image.y_mm, sy, image.height_px);
    let x1 = to_px_up(x + w - image.x_mm, sx, image.width_px);
    let y1 = to_px_up(y + h - image.y_mm, sy, image.height_px);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    // Clamped into 0..=width_px/height_px above, so the casts are exact.
    let (x0, y0, x1, y1) = (x0 as u32, y0 as u32, x1 as u32, y1 as u32);
    let crop = RgbaImage::from_fn(x1 - x0, y1 - y0, |cx, cy| {
        pixels
            .get_pixel_checked(x0.saturating_add(cx), y0.saturating_add(cy))
            .copied()
            .unwrap_or(common::Pixel([255, 255, 255, 0]))
    });
    Some((crop, x0, y0))
}

/// A placed image from pixels, at the origin, sized by `dpi` (pixels per inch).
pub fn place(pixels: &RgbaImage, name: &str, dpi: f64) -> Result<RasterImage, RasterError> {
    if !(dpi.is_finite() && (1.0..=10_000.0).contains(&dpi)) {
        return Err(bad(format!("dpi {dpi} is outside 1–10000")));
    }
    let mm_per_px = 25.4 / dpi;
    Ok(RasterImage {
        id: 0,
        name: name.to_string(),
        x_mm: 0.0,
        y_mm: 0.0,
        width_mm: f64::from(pixels.width()) * mm_per_px,
        height_mm: f64::from(pixels.height()) * mm_per_px,
        width_px: pixels.width(),
        height_px: pixels.height(),
        png_base64: encode_png_base64(pixels)?.into(),
    })
}

/// Same placement, same id, new pixels (detach, background removal).
pub fn with_pixels(image: &RasterImage, pixels: &RgbaImage) -> Result<RasterImage, RasterError> {
    Ok(RasterImage {
        png_base64: encode_png_base64(pixels)?.into(),
        width_px: pixels.width(),
        height_px: pixels.height(),
        ..image.clone()
    })
}

/// Most pixels a straightened image may have.
const MAX_RECTIFY_PIXELS: u64 = 16_000_000;

/// Solve the 8×8 system for the homography taking the unit square's corners (0,0), (1,0),
/// (1,1), (0,1) to `quad` (clockwise from the top left). `None` when the quad is degenerate.
fn unit_square_homography(quad: [[f64; 2]; 4]) -> Option<[f64; 8]> {
    let unit = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut m = [[0.0f64; 9]; 8];
    for (k, (u, q)) in unit.iter().zip(quad).enumerate() {
        let (x, y) = (u[0], u[1]);
        let (sx, sy) = (q[0], q[1]);
        m[2 * k] = [x, y, 1.0, 0.0, 0.0, 0.0, -x * sx, -y * sx, sx];
        m[2 * k + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -x * sy, -y * sy, sy];
    }
    // Gaussian elimination with partial pivoting.
    for col in 0..8 {
        let pivot = (col..8).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[pivot][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, pivot);
        for row in 0..8 {
            if row != col {
                let f = m[row][col] / m[col][col];
                for c in col..9 {
                    m[row][c] -= f * m[col][c];
                }
            }
        }
    }
    let mut h = [0.0; 8];
    for (i, slot) in h.iter_mut().enumerate() {
        *slot = m[i][8] / m[i][i];
    }
    h.iter().all(|v| v.is_finite()).then_some(h)
}

/// PixScan, by hand: the part of `pixels` inside `corners` (pixel coordinates of the page's
/// top-left, top-right, bottom-right and bottom-left corners in a photo) straightened into a
/// `width` × `height` pixel image by a perspective (homography) warp, bilinear sampled.
pub fn rectify(
    pixels: &RgbaImage,
    corners: [[f64; 2]; 4],
    width: u32,
    height: u32,
) -> Result<RgbaImage, RasterError> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_RECTIFY_PIXELS {
        return Err(bad(format!(
            "rectify: {width} × {height} pixels is empty or too large"
        )));
    }
    if !corners.iter().flatten().all(|v| v.is_finite()) {
        return Err(bad("rectify: the corners are not finite".to_string()));
    }
    let h = unit_square_homography(corners)
        .ok_or_else(|| bad("rectify: the four corners do not make a quadrilateral".to_string()))?;
    let (sw, sh) = (pixels.width(), pixels.height());
    let sample = |x: f64, y: f64| -> [f64; 4] {
        if !(x.is_finite() && y.is_finite()) || x < 0.0 || y < 0.0 {
            return [255.0, 255.0, 255.0, 0.0];
        }
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        // Non-negative and finite here, so the casts saturate harmlessly.
        let (ix, iy) = (x0 as u32, y0 as u32);
        let px = |cx: u32, cy: u32| -> [f64; 4] {
            pixels
                .get_pixel_checked(cx.min(sw.saturating_sub(1)), cy.min(sh.saturating_sub(1)))
                .map_or([255.0, 255.0, 255.0, 0.0], |p| p.0.map(f64::from))
        };
        if ix >= sw || iy >= sh {
            return [255.0, 255.0, 255.0, 0.0];
        }
        let (a, b, c, d) = (
            px(ix, iy),
            px(ix.saturating_add(1), iy),
            px(ix, iy.saturating_add(1)),
            px(ix.saturating_add(1), iy.saturating_add(1)),
        );
        let mut out = [0.0; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * fx;
            let bottom = c[k] + (d[k] - c[k]) * fx;
            out[k] = top + (bottom - top) * fy;
        }
        out
    };
    Ok(RgbaImage::from_fn(width, height, |ox, oy| {
        let u = (f64::from(ox) + 0.5) / f64::from(width);
        let v = (f64::from(oy) + 0.5) / f64::from(height);
        let w = h[6] * u + h[7] * v + 1.0;
        let (x, y) = if w.abs() > 1e-12 {
            (
                (h[0] * u + h[1] * v + h[2]) / w,
                (h[3] * u + h[4] * v + h[5]) / w,
            )
        } else {
            (f64::NAN, f64::NAN)
        };
        // Each channel is within 0..=255 (a blend of pixel values), so the casts are exact.
        common::Pixel(sample(x, y).map(|c| c.round().clamp(0.0, 255.0) as u8))
    }))
}

/// Image Effects: brightness, contrast and saturation (each −100…100, 0 leaves it), then
/// greyscale and invert. Alpha is kept.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Adjust {
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub grayscale: bool,
    pub invert: bool,
}

pub fn adjust(pixels: &RgbaImage, a: &Adjust) -> Result<RgbaImage, RasterError> {
    for (value, name) in [
        (a.brightness, "brightness"),
        (a.contrast, "contrast"),
        (a.saturation, "saturation"),
    ] {
        if !(value.is_finite() && (-100.0..=100.0).contains(&value)) {
            return Err(bad(format!("{name} {value} is outside -100–100")));
        }
    }
    let brightness = (a.brightness / 100.0) as f32;
    // Contrast scales about mid-grey: −100 flattens, +100 doubles the spread.
    let contrast = (1.0 + a.contrast / 100.0) as f32;
    let saturation = (1.0 + a.saturation / 100.0) as f32;
    let mut out = pixels.clone();
    for pixel in out.pixels_mut() {
        let [r, g, b, alpha] = pixel.0;
        let mut c = [r, g, b].map(|v| f32::from(v) / 255.0);
        for v in &mut c {
            *v = ((*v - 0.5) * contrast + 0.5 + brightness).clamp(0.0, 1.0);
        }
        let grey = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
        let factor = if a.grayscale { 0.0 } else { saturation };
        for v in &mut c {
            *v = (grey + (*v - grey) * factor).clamp(0.0, 1.0);
            if a.invert {
                *v = 1.0 - *v;
            }
        }
        // In 0..=1 after the clamps, so the casts are exact enough and cannot overflow.
        let [r, g, b] = c.map(|v| (v * 255.0).round() as u8);
        pixel.0 = [r, g, b, alpha];
    }
    Ok(out)
}

/// Largest median radius, pixels.
pub const MAX_SMOOTH_RADIUS: u32 = 10;

/// Upload cleanup's Despeckle: opaque islands (four-way connected) of fewer than `area` pixels
/// made transparent: the specks a background removal leaves.
pub fn despeckle(pixels: &RgbaImage, area: u32) -> Result<RgbaImage, RasterError> {
    if area == 0 || area > 1_000_000 {
        return Err(bad(format!("despeckle area {area} is outside 1–1000000")));
    }
    let (w, h) = (pixels.width(), pixels.height());
    let opaque = |x: u32, y: u32| {
        pixels
            .get_pixel_checked(x, y)
            .is_some_and(|p| p.0[3] >= 128)
    };
    let index = |x: u32, y: u32| (y as usize) * (w as usize) + x as usize;
    let mut seen = vec![false; (w as usize) * (h as usize)];
    let mut out = pixels.clone();
    let clear = common::Pixel([0, 0, 0, 0]);
    for sy in 0..h {
        for sx in 0..w {
            if seen.get(index(sx, sy)).copied().unwrap_or(true) || !opaque(sx, sy) {
                continue;
            }
            // Gather the island.
            let mut island = Vec::new();
            let mut stack = vec![(sx, sy)];
            while let Some((x, y)) = stack.pop() {
                let Some(slot) = seen.get_mut(index(x, y)) else {
                    continue;
                };
                if *slot || !opaque(x, y) {
                    continue;
                }
                *slot = true;
                island.push((x, y));
                if x > 0 {
                    stack.push((x - 1, y));
                }
                if y > 0 {
                    stack.push((x, y - 1));
                }
                if x + 1 < w {
                    stack.push((x + 1, y));
                }
                if y + 1 < h {
                    stack.push((x, y + 1));
                }
            }
            if island.len() < area as usize {
                for (x, y) in island {
                    if let Some(p) = out.get_pixel_mut_checked(x, y) {
                        *p = clear;
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Upload cleanup's Smooth: each channel the median of the `radius` square around it (1–10
/// pixels), which removes noise and keeps edges.
pub fn smooth(pixels: &RgbaImage, radius: u32) -> Result<RgbaImage, RasterError> {
    if radius == 0 || radius > MAX_SMOOTH_RADIUS {
        return Err(bad(format!(
            "smooth radius {radius} is outside 1–{MAX_SMOOTH_RADIUS}"
        )));
    }
    let (w, h) = (pixels.width(), pixels.height());
    let r = i64::from(radius);
    let mut window: Vec<u8> = Vec::with_capacity(((2 * r + 1) * (2 * r + 1)) as usize);
    let mut out = pixels.clone();
    for y in 0..h {
        for x in 0..w {
            let mut median = [0u8; 4];
            for (c, slot) in median.iter_mut().enumerate() {
                window.clear();
                for dy in -r..=r {
                    for dx in -r..=r {
                        let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
                        if let (Ok(nx), Ok(ny)) = (u32::try_from(nx), u32::try_from(ny))
                            && let Some(p) = pixels.get_pixel_checked(nx, ny)
                            && let Some(v) = p.0.get(c)
                        {
                            window.push(*v);
                        }
                    }
                }
                window.sort_unstable();
                *slot = window.get(window.len() / 2).copied().unwrap_or(0);
            }
            if let Some(p) = out.get_pixel_mut_checked(x, y) {
                p.0 = median;
            }
        }
    }
    Ok(out)
}

/// Most stroke points an erase brush stroke takes.
pub const MAX_BRUSH_POINTS: usize = 10_000;

/// Upload cleanup's Erase brush: every pixel whose centre is within `radius` pixels of the
/// stroke through `points` (pixel coordinates; one point is a dab) made transparent.
pub fn erase_brush(
    pixels: &RgbaImage,
    points: &[[f64; 2]],
    radius: f64,
) -> Result<RgbaImage, RasterError> {
    if !(radius.is_finite() && radius > 0.0 && radius <= 10_000.0) {
        return Err(bad(format!("brush radius {radius} is outside (0, 10000]")));
    }
    if points.is_empty() || points.len() > MAX_BRUSH_POINTS {
        return Err(bad(format!("a stroke has 1 to {MAX_BRUSH_POINTS} points")));
    }
    if !points.iter().flatten().all(|v| v.is_finite()) {
        return Err(bad("stroke points must be finite".to_string()));
    }
    let mut out = pixels.clone();
    let (w, h) = (f64::from(pixels.width()), f64::from(pixels.height()));
    let segments: Vec<([f64; 2], [f64; 2])> = match points {
        [only] => vec![(*only, *only)],
        _ => points
            .windows(2)
            .filter_map(|s| Some((*s.first()?, *s.get(1)?)))
            .collect(),
    };
    let r2 = radius * radius;
    let clear = common::Pixel([0, 0, 0, 0]);
    for (a, b) in segments {
        // The segment's box grown by the radius, clamped to the image.
        let x0 = (a[0].min(b[0]) - radius).floor().max(0.0);
        let x1 = (a[0].max(b[0]) + radius).ceil().min(w);
        let y0 = (a[1].min(b[1]) - radius).floor().max(0.0);
        let y1 = (a[1].max(b[1]) + radius).ceil().min(h);
        if !(x0 < x1 && y0 < y1) {
            continue;
        }
        let (d0, d1) = (b[0] - a[0], b[1] - a[1]);
        let len2 = d0 * d0 + d1 * d1;
        // Inside the image's pixel range after the clamps, so the casts are exact.
        for y in (y0 as u32)..(y1 as u32) {
            for x in (x0 as u32)..(x1 as u32) {
                let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                let t = if len2 > 0.0 {
                    (((cx - a[0]) * d0 + (cy - a[1]) * d1) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (px, py) = (a[0] + t * d0 - cx, a[1] + t * d1 - cy);
                if px * px + py * py <= r2
                    && let Some(p) = out.get_pixel_mut_checked(x, y)
                {
                    *p = clear;
                }
            }
        }
    }
    Ok(out)
}

/// Upload cleanup's Select: the pixels like the one at (`x`, `y`) made transparent. Like means
/// every channel within `tolerance` percent (0–100) of the picked colour; `contiguous` takes
/// only the region connected to the pick (four-way), else every such pixel in the image. An
/// already transparent pick, or one outside the image, changes nothing.
pub fn erase_colour(
    pixels: &RgbaImage,
    x: u32,
    y: u32,
    tolerance: f64,
    contiguous: bool,
) -> Result<RgbaImage, RasterError> {
    if !(tolerance.is_finite() && (0.0..=100.0).contains(&tolerance)) {
        return Err(bad(format!("tolerance {tolerance} is outside 0–100")));
    }
    let mut out = pixels.clone();
    let Some(pick) = pixels.get_pixel_checked(x, y).map(|p| p.0) else {
        return Ok(out);
    };
    if pick[3] < 128 {
        return Ok(out);
    }
    // Within 0..=255 after the clamp, so the cast is exact.
    let limit = (tolerance / 100.0 * 255.0).round().clamp(0.0, 255.0) as i32;
    let like = |p: [u8; 4]| {
        p[3] >= 128 && (0..3).all(|c| (i32::from(p[c]) - i32::from(pick[c])).abs() <= limit)
    };
    let clear = common::Pixel([0, 0, 0, 0]);
    let (w, h) = (pixels.width(), pixels.height());
    if !contiguous {
        for p in out.pixels_mut() {
            if like(p.0) {
                *p = clear;
            }
        }
        return Ok(out);
    }
    let index = |x: u32, y: u32| (y as usize) * (w as usize) + x as usize;
    let mut seen = vec![false; (w as usize) * (h as usize)];
    let mut stack = vec![(x, y)];
    while let Some((cx, cy)) = stack.pop() {
        let Some(slot) = seen.get_mut(index(cx, cy)) else {
            continue;
        };
        if *slot {
            continue;
        }
        *slot = true;
        let Some(p) = pixels.get_pixel_checked(cx, cy).map(|p| p.0) else {
            continue;
        };
        if !like(p) {
            continue;
        }
        if let Some(px) = out.get_pixel_mut_checked(cx, cy) {
            *px = clear;
        }
        if cx > 0 {
            stack.push((cx - 1, cy));
        }
        if cy > 0 {
            stack.push((cx, cy - 1));
        }
        if cx + 1 < w {
            stack.push((cx + 1, cy));
        }
        if cy + 1 < h {
            stack.push((cx, cy + 1));
        }
    }
    Ok(out)
}

/// Make the background transparent: flood-fill (4-connected) from every border pixel
/// whose colour is within `tolerance` percent (0–100, per channel) of the border's most
/// common colour. Already transparent pixels count as background.
pub fn remove_background(pixels: &RgbaImage, tolerance: f64) -> Result<RgbaImage, RasterError> {
    if !(tolerance.is_finite() && (0.0..=100.0).contains(&tolerance)) {
        return Err(bad(format!("tolerance {tolerance} is outside 0–100")));
    }
    let (w, h) = (pixels.width() as usize, pixels.height() as usize);
    if w == 0 || h == 0 {
        return Ok(pixels.clone());
    }
    let raw = pixels.as_raw();
    let at = |i: usize| -> [u8; 4] {
        let p = raw.get(i * 4..i * 4 + 4).unwrap_or(&[0; 4]);
        [p[0], p[1], p[2], p[3]]
    };
    let border: Vec<usize> = (0..w)
        .flat_map(|x| [x, (h - 1) * w + x])
        .chain((0..h).flat_map(|y| [y * w, y * w + w - 1]))
        .collect();
    // Most common opaque border colour, quantised so JPEG noise still agrees.
    let mut counts = std::collections::HashMap::<[u8; 3], usize>::new();
    for &i in &border {
        let p = at(i);
        if p[3] >= 128 {
            *counts.entry([p[0] >> 4, p[1] >> 4, p[2] >> 4]).or_default() += 1;
        }
    }
    let reference = counts
        .into_iter()
        .max_by_key(|(key, n)| (*n, *key))
        .map(|(key, _)| key);
    let limit = (tolerance / 100.0 * 255.0).round() as i32;
    let reference_colour = border
        .iter()
        .map(|&i| at(i))
        .find(|p| p[3] >= 128 && reference == Some([p[0] >> 4, p[1] >> 4, p[2] >> 4]));
    let is_background = |p: [u8; 4]| {
        p[3] < 128
            || reference_colour
                .is_some_and(|r| (0..3).all(|c| (i32::from(p[c]) - i32::from(r[c])).abs() <= limit))
    };
    let mut out = pixels.clone();
    let mut seen = vec![false; w * h];
    let mut stack: Vec<usize> = Vec::new();
    for &i in &border {
        if !seen[i] && is_background(at(i)) {
            seen[i] = true;
            stack.push(i);
        }
        while let Some(i) = stack.pop() {
            if let Some(alpha) = out.get_mut(i * 4 + 3..i * 4 + 4) {
                alpha[0] = 0;
            }
            let (x, y) = (i % w, i / w);
            let neighbours = [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then(|| i + 1),
                (y > 0).then(|| i - w),
                (y + 1 < h).then(|| i + w),
            ];
            for n in neighbours.into_iter().flatten() {
                if !seen[n] && is_background(at(n)) {
                    seen[n] = true;
                    stack.push(n);
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_raster_codecs::common::{Pixel, encode_for_path};

    fn encode(image: &RgbaImage, extension: &str) -> Vec<u8> {
        encode_for_path(format!("fixture.{extension}"), image).unwrap()
    }

    #[test]
    fn decodes_every_supported_format() {
        let image = RgbaImage::from_fn(8, 6, |x, _| {
            if x < 4 {
                Pixel([0, 0, 0, 255])
            } else {
                Pixel([255, 255, 255, 255])
            }
        });
        for extension in ["png", "jpg", "bmp", "gif", "tiff", "webp"] {
            let decoded =
                decode(&encode(&image, extension)).unwrap_or_else(|e| panic!("{extension}: {e}"));
            assert_eq!(decoded.dimensions(), (8, 6), "{extension}");
        }
    }

    #[test]
    fn despeckle_removes_small_islands_and_keeps_large_ones() {
        let image = RgbaImage::from_fn(30, 20, |x, y| {
            let big = (2..12).contains(&x) && (2..12).contains(&y);
            let speck = (20..22).contains(&x) && (5..7).contains(&y);
            if big || speck {
                Pixel([0, 0, 0, 255])
            } else {
                Pixel([0, 0, 0, 0])
            }
        });
        let out = despeckle(&image, 10).unwrap();
        assert_eq!(out.get_pixel(5, 5).0[3], 255, "the 100 px island stays");
        assert_eq!(out.get_pixel(20, 5).0[3], 0, "the 4 px speck goes");
        assert!(despeckle(&image, 0).is_err());
    }

    #[test]
    fn smooth_removes_lone_noise_and_keeps_edges() {
        let image = RgbaImage::from_fn(20, 20, |x, y| {
            if (x, y) == (5, 5) {
                Pixel([255, 255, 255, 255])
            } else if x < 10 {
                Pixel([0, 0, 0, 255])
            } else {
                Pixel([200, 200, 200, 255])
            }
        });
        let out = smooth(&image, 1).unwrap();
        assert_eq!(out.get_pixel(5, 5).0[0], 0, "the lone white pixel is gone");
        assert_eq!(out.get_pixel(8, 10).0[0], 0, "left of the edge stays dark");
        assert_eq!(out.get_pixel(11, 10).0[0], 200, "right of it stays light");
        assert!(smooth(&image, 0).is_err() && smooth(&image, 99).is_err());
    }

    #[test]
    fn erase_brush_clears_along_the_stroke_only() {
        let image = RgbaImage::from_pixel(40, 20, Pixel([10, 10, 10, 255]));
        let out = erase_brush(&image, &[[5.0, 10.0], [35.0, 10.0]], 3.0).unwrap();
        assert_eq!(out.get_pixel(20, 10).0[3], 0, "on the stroke");
        assert_eq!(out.get_pixel(20, 2).0[3], 255, "away from it");
        let dab = erase_brush(&image, &[[10.0, 10.0]], 2.0).unwrap();
        assert_eq!(dab.get_pixel(10, 10).0[3], 0, "a single point is a dab");
        assert_eq!(dab.get_pixel(20, 10).0[3], 255);
        // Off the image: nothing, and no panic.
        assert_eq!(
            erase_brush(&image, &[[-50.0, -50.0], [-40.0, -40.0]], 3.0).unwrap(),
            image
        );
        assert!(erase_brush(&image, &[], 3.0).is_err());
        assert!(erase_brush(&image, &[[f64::NAN, 1.0]], 3.0).is_err());
        assert!(erase_brush(&image, &[[1.0, 1.0]], 0.0).is_err());
    }

    #[test]
    fn erase_colour_clears_the_picked_region_or_every_match() {
        // Two red squares apart on white: the left one at x 0..4, the right at 8..12.
        let image = RgbaImage::from_fn(12, 4, |x, _| {
            if !(4..8).contains(&x) {
                Pixel([200, 10, 10, 255])
            } else {
                Pixel([255, 255, 255, 255])
            }
        });
        let region = erase_colour(&image, 1, 1, 10.0, true).unwrap();
        assert_eq!(
            region.get_pixel(2, 2).0[3],
            0,
            "the picked square is cleared"
        );
        assert_eq!(region.get_pixel(10, 2).0[3], 255, "the other square stays");
        assert_eq!(region.get_pixel(5, 2).0[3], 255, "white stays");
        let every = erase_colour(&image, 1, 1, 10.0, false).unwrap();
        assert_eq!(every.get_pixel(10, 2).0[3], 0, "every match is cleared");
        assert_eq!(every.get_pixel(5, 2).0[3], 255);
        assert_eq!(
            erase_colour(&image, 99, 1, 10.0, true).unwrap(),
            image,
            "outside: no change"
        );
        assert!(erase_colour(&image, 1, 1, f64::NAN, true).is_err());
    }

    #[test]
    fn rectify_straightens_a_skewed_page() {
        // A 100×100 photo: a white page with a black square marking its top-left quarter,
        // photographed as the axis-aligned rect (10,10)-(90,90).
        let photo = RgbaImage::from_fn(100, 100, |x, y| {
            if (10..50).contains(&x) && (10..50).contains(&y) {
                Pixel([0, 0, 0, 255])
            } else {
                Pixel([255, 255, 255, 255])
            }
        });
        let out = rectify(
            &photo,
            [[10.0, 10.0], [90.0, 10.0], [90.0, 90.0], [10.0, 90.0]],
            40,
            40,
        )
        .unwrap();
        assert_eq!(out.get_pixel(5, 5).0[0], 0, "top-left quarter is black");
        assert_eq!(out.get_pixel(35, 35).0[0], 255, "bottom-right is white");
        assert!(
            rectify(&photo, [[0.0, 0.0]; 4], 10, 10).is_err(),
            "degenerate"
        );
        assert!(
            rectify(
                &photo,
                [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                0,
                10
            )
            .is_err()
        );
        assert!(
            rectify(
                &photo,
                [[f64::NAN, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                5,
                5
            )
            .is_err()
        );
    }

    #[test]
    fn adjust_brightness_greyscale_invert() {
        let pixels = RgbaImage::from_pixel(2, 2, Pixel([200, 50, 50, 128]));
        let same = adjust(&pixels, &Adjust::default()).unwrap();
        assert_eq!(same, pixels);
        let grey = adjust(
            &pixels,
            &Adjust {
                grayscale: true,
                ..Adjust::default()
            },
        )
        .unwrap();
        let p = grey.get_pixel(0, 0).0;
        assert!(p[0] == p[1] && p[1] == p[2] && p[3] == 128, "{p:?}");
        let inverted = adjust(
            &pixels,
            &Adjust {
                invert: true,
                ..Adjust::default()
            },
        )
        .unwrap();
        assert_eq!(inverted.get_pixel(0, 0).0, [55, 205, 205, 128]);
        let bright = adjust(
            &pixels,
            &Adjust {
                brightness: 100.0,
                ..Adjust::default()
            },
        )
        .unwrap();
        assert_eq!(bright.get_pixel(0, 0).0, [255, 255, 255, 128]);
        assert!(
            adjust(
                &pixels,
                &Adjust {
                    contrast: f64::NAN,
                    ..Adjust::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_junk_heif_and_oversized_headers() {
        assert!(decode(b"not an image").is_err());
        assert!(decode(b"").is_err());
        let heic = b"\0\0\0\x18ftypheic\0\0\0\0mif1heic";
        assert!(decode(heic).unwrap_err().to_string().contains("HEIC"));
        // A BMP header claiming 60000×60000 must fail before allocating pixels.
        let mut bmp = encode(&RgbaImage::new(1, 1), "bmp");
        bmp[18..22].copy_from_slice(&60_000_i32.to_le_bytes());
        bmp[22..26].copy_from_slice(&60_000_i32.to_le_bytes());
        assert!(decode(&bmp).is_err());
    }

    #[test]
    fn place_sizes_by_dpi_and_round_trips_pixels() {
        let pixels = RgbaImage::from_pixel(96, 48, Pixel([10, 20, 30, 255]));
        let mut placed = place(&pixels, "p", 96.0).unwrap();
        assert!((placed.width_mm - 25.4).abs() < 1e-9);
        assert!((placed.height_mm - 12.7).abs() < 1e-9);
        assert_eq!(image_pixels(&placed).unwrap(), pixels);
        assert!(place(&pixels, "p", f64::NAN).is_err());
        placed.width_px = 3;
        assert!(image_pixels(&placed).is_err());
    }

    #[test]
    fn remove_background_clears_only_the_border_connected_colour() {
        // White frame, black ring, white centre: the centre is not connected to the border.
        let pixels = RgbaImage::from_fn(9, 9, |x, y| {
            let d = (x as i32 - 4).abs().max((y as i32 - 4).abs());
            if d == 2 || d == 3 {
                Pixel([0, 0, 0, 255])
            } else {
                Pixel([250, 252, 255, 255])
            }
        });
        let out = remove_background(&pixels, 5.0).unwrap();
        assert_eq!(out.get_pixel(0, 0)[3], 0);
        assert_eq!(out.get_pixel(4, 4)[3], 255, "enclosed white stays");
        assert_eq!(out.get_pixel(2, 4)[3], 255, "the ring stays");
        assert!(remove_background(&pixels, 101.0).is_err());
    }
}
