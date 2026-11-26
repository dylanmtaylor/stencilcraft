//! PixScan's automatic corner finding: in a photo of a page (or mat) lying on a darker
//! background, the page's four corners, ready for [`crate::rectify`].
//!
//! The page is the largest bright region after an Otsu threshold on brightness; its corners are
//! the region's pixels furthest along the four diagonals (top left is the smallest x + y, and so
//! on), which holds for a page photographed square-on or turned well under 45°.

use crate::RgbaImage;

/// The most pixels the search visits (a 40-megapixel photo).
const MAX_PIXELS: u64 = 40_000_000;

/// Brightness 0–255 of a pixel, transparent counting as black.
fn luma(p: &[u8; 4]) -> u8 {
    let y = (299 * u32::from(p[0]) + 587 * u32::from(p[1]) + 114 * u32::from(p[2])) / 1000;
    // In 0..=255 by construction.
    (y * u32::from(p[3]) / 255) as u8
}

/// Otsu's threshold of a brightness histogram: the level that best splits it in two.
fn otsu(histogram: &[u64; 256]) -> u8 {
    let total: u64 = histogram.iter().sum();
    if total == 0 {
        return 128;
    }
    let sum: f64 = histogram
        .iter()
        .enumerate()
        .map(|(i, &n)| i as f64 * n as f64)
        .sum();
    let (mut below, mut below_sum, mut best, mut level) = (0u64, 0.0f64, -1.0f64, 128u8);
    for (i, &n) in histogram.iter().enumerate() {
        below += n;
        if below == 0 || below == total {
            continue;
        }
        below_sum += i as f64 * n as f64;
        let above = total - below;
        let (mean_below, mean_above) = (below_sum / below as f64, (sum - below_sum) / above as f64);
        let between = below as f64 * above as f64 * (mean_below - mean_above).powi(2);
        if between > best {
            best = between;
            // In 0..=255 as an index of the histogram.
            level = i as u8;
        }
    }
    level
}

/// The page's corners in pixel coordinates, top left, top right, bottom right, bottom left;
/// `None` when no bright region covers at least a tenth of the photo.
pub fn detect_page_corners(pixels: &RgbaImage) -> Option<[[f64; 2]; 4]> {
    let (w, h) = (pixels.width() as usize, pixels.height() as usize);
    let count = (w as u64).checked_mul(h as u64)?;
    if w < 4 || h < 4 || count > MAX_PIXELS {
        return None;
    }
    let lum: Vec<u8> = pixels.pixels().map(|p| luma(&p.0)).collect();
    let mut histogram = [0u64; 256];
    for &v in &lum {
        histogram[usize::from(v)] += 1;
    }
    let level = otsu(&histogram);
    let bright: Vec<bool> = lum.iter().map(|&v| v > level).collect();
    // The largest 4-connected bright region, by an explicit stack (no recursion).
    let mut seen = vec![false; bright.len()];
    let mut best: Vec<usize> = Vec::new();
    let mut stack = Vec::new();
    for start in 0..bright.len() {
        if !bright[start] || seen[start] {
            continue;
        }
        let mut region = Vec::new();
        seen[start] = true;
        stack.push(start);
        while let Some(i) = stack.pop() {
            region.push(i);
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if bright.get(j).copied().unwrap_or(false) && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
        if region.len() > best.len() {
            best = region;
        }
    }
    if (best.len() as u64) * 10 < count {
        return None;
    }
    let point = |i: usize| ((i % w) as f64, (i / w) as f64);
    let pick = |key: &dyn Fn((f64, f64)) -> f64| {
        best.iter()
            .map(|&i| point(i))
            .min_by(|a, b| key(*a).total_cmp(&key(*b)))
            .map(|(x, y)| [x + 0.5, y + 0.5])
    };
    Some([
        pick(&|(x, y)| x + y)?,
        pick(&|(x, y)| y - x)?,
        pick(&|(x, y)| -(x + y))?,
        pick(&|(x, y)| x - y)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_raster_codecs::common::Pixel;

    /// A white quadrilateral on a dark background.
    fn photo(corners: [[f64; 2]; 4]) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(240, 180, Pixel([40, 40, 45, 255]));
        let inside = |x: f64, y: f64| {
            (0..4).all(|i| {
                let [ax, ay] = corners[i];
                let [bx, by] = corners[(i + 1) % 4];
                (bx - ax) * (y - ay) - (by - ay) * (x - ax) >= 0.0
            })
        };
        for y in 0..180 {
            for x in 0..240 {
                if inside(f64::from(x) + 0.5, f64::from(y) + 0.5) {
                    img.put_pixel(x, y, Pixel([235, 232, 225, 255]));
                }
            }
        }
        img
    }

    #[test]
    fn a_tilted_page_on_a_dark_table_gives_its_four_corners() {
        let truth = [[40.0, 30.0], [200.0, 20.0], [215.0, 150.0], [30.0, 160.0]];
        let found = detect_page_corners(&photo(truth)).expect("a page");
        for (f, t) in found.iter().zip(truth) {
            assert!(
                (f[0] - t[0]).abs() < 4.0 && (f[1] - t[1]).abs() < 4.0,
                "{f:?} vs {t:?}"
            );
        }
    }

    #[test]
    fn no_page_and_tiny_photos_find_nothing() {
        let dark = RgbaImage::from_pixel(100, 100, Pixel([10, 10, 10, 255]));
        assert_eq!(detect_page_corners(&dark), None);
        assert_eq!(
            detect_page_corners(&RgbaImage::from_pixel(2, 2, Pixel([0, 0, 0, 255]))),
            None
        );
    }
}
