//! Trace by Colour: the image's main colours (histogram and k-means) and, for each, a mask of the
//! pixels nearest it, so each colour traces into its own layer and every opaque pixel
//! belongs to exactly one colour.

use crate::{RasterError, RgbaImage, bad};
use craft_raster_codecs::common::Pixel;

/// Most colours a palette has.
pub const MAX_COLOURS: usize = 16;
/// Pixels sampled for the palette (larger images are strided).
const MAX_SAMPLES: usize = 250_000;
/// Pixels at least this transparent are background and belong to no colour.
const OPAQUE: u8 = 128;

/// Two palette colours closer than this (squared RGB distance) are one colour: anti-aliased
/// edges and noise do not become colours of their own.
const MIN_SEPARATION: u32 = 48 * 48 * 3;
/// A colour covers at least this share of the pixels (percent): the thin spread of
/// anti-aliased shades along edges is not a colour.
const MIN_SHARE_PCT: u64 = 1;
/// Refinement passes after picking the colours.
const REFINE_PASSES: usize = 4;

/// Up to `count` (1–16) main colours of the opaque pixels, darkest first: the most common
/// distinct colours (a 4-bit-per-channel histogram), refined by a few k-means passes so each
/// is the mean of the pixels nearest it. Deterministic; fewer when the image has fewer
/// distinct colours.
pub fn palette(pixels: &RgbaImage, count: usize) -> Result<Vec<[u8; 3]>, RasterError> {
    if !(1..=MAX_COLOURS).contains(&count) {
        return Err(bad(format!("palette: 1 to {MAX_COLOURS} colours")));
    }
    let total = pixels.pixels().len();
    let stride = total.div_ceil(MAX_SAMPLES).max(1);
    let samples: Vec<[u8; 3]> = pixels
        .pixels()
        .step_by(stride)
        .filter(|p| p.0[3] >= OPAQUE)
        .map(|p| [p.0[0], p.0[1], p.0[2]])
        .collect();
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    // Histogram: 16 levels per channel, each bucket keeping its count and colour sum.
    let mut buckets = vec![(0u64, [0u64; 3]); 4096];
    for p in &samples {
        let index =
            (usize::from(p[0] >> 4) << 8) | (usize::from(p[1] >> 4) << 4) | usize::from(p[2] >> 4);
        if let Some((n, sum)) = buckets.get_mut(index) {
            *n += 1;
            for (s, v) in sum.iter_mut().zip(p) {
                *s += u64::from(*v);
            }
        }
    }
    let mut ranked: Vec<(u64, [u8; 3])> = buckets
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, sum)| (*n, sum.map(|s| u8::try_from(s / n).unwrap_or(u8::MAX))))
        .collect();
    // Most common first; ties by colour, so the order never depends on the sort.
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let sampled = u64::try_from(samples.len()).unwrap_or(u64::MAX);
    let mut colours: Vec<[u8; 3]> = Vec::with_capacity(count);
    for (n, colour) in ranked {
        if colours.len() == count {
            break;
        }
        // Colours are ranked by count, so once one is too rare the rest are too (but keep at
        // least one colour).
        if !colours.is_empty() && n.saturating_mul(100) < sampled.saturating_mul(MIN_SHARE_PCT) {
            break;
        }
        if colours
            .iter()
            .all(|c| distance(*c, colour) >= MIN_SEPARATION)
        {
            colours.push(colour);
        }
    }
    // k-means: each colour becomes the mean of the samples nearest it.
    for _ in 0..REFINE_PASSES {
        let mut sums = vec![(0u64, [0u64; 3]); colours.len()];
        for p in &samples {
            if let Some((i, _)) = colours
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| distance(*p, **c))
                && let Some((n, sum)) = sums.get_mut(i)
            {
                *n += 1;
                for (s, v) in sum.iter_mut().zip(p) {
                    *s += u64::from(*v);
                }
            }
        }
        for (colour, (n, sum)) in colours.iter_mut().zip(&sums) {
            if *n > 0 {
                *colour = sum.map(|s| u8::try_from(s / n).unwrap_or(u8::MAX));
            }
        }
    }
    colours.sort_by_key(|c| u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2]));
    colours.dedup();
    Ok(colours)
}

/// Squared distance between two colours.
fn distance(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| {
            let d = i32::from(*x) - i32::from(y);
            d.unsigned_abs().pow(2)
        })
        .sum()
}

/// One mask per colour of `colours`: black where the opaque pixel is nearest that colour,
/// white elsewhere, ready for the brightness trace.
pub fn colour_masks(
    pixels: &RgbaImage,
    colours: &[[u8; 3]],
) -> Result<Vec<RgbaImage>, RasterError> {
    if colours.is_empty() || colours.len() > MAX_COLOURS {
        return Err(bad(format!("colour masks: 1 to {MAX_COLOURS} colours")));
    }
    let white = Pixel([255, 255, 255, 255]);
    let black = Pixel([0, 0, 0, 255]);
    let mut masks =
        vec![RgbaImage::from_pixel(pixels.width(), pixels.height(), white); colours.len()];
    for (x, y, p) in pixels.enumerate_pixels() {
        if p.0[3] < OPAQUE {
            continue;
        }
        let rgb = [p.0[0], p.0[1], p.0[2]];
        let nearest = colours
            .iter()
            .enumerate()
            .min_by_key(|(_, c)| distance(rgb, **c))
            .map(|(i, _)| i);
        if let Some(slot) = nearest
            .and_then(|i| masks.get_mut(i))
            .and_then(|mask| mask.get_pixel_mut_checked(x, y))
        {
            *slot = black;
        }
    }
    Ok(masks)
}

/// `#rrggbb` for a colour.
pub fn hex(colour: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", colour[0], colour[1], colour[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_colours() -> RgbaImage {
        RgbaImage::from_fn(30, 10, |x, _| match x / 10 {
            0 => Pixel([200, 20, 20, 255]),
            1 => Pixel([20, 200, 20, 255]),
            _ => Pixel([20, 20, 200, 255]),
        })
    }

    #[test]
    fn palette_finds_the_colours_and_masks_partition_the_pixels() {
        let image = three_colours();
        let colours = palette(&image, 3).unwrap();
        assert_eq!(colours.len(), 3, "{colours:?}");
        assert!(colours.contains(&[200, 20, 20]));
        let masks = colour_masks(&image, &colours).unwrap();
        for (x, y, _) in image.enumerate_pixels() {
            let owners = masks.iter().filter(|m| m.get_pixel(x, y).0[0] == 0).count();
            assert_eq!(owners, 1, "each pixel in one mask");
        }
        // Anti-aliased edges between flat colours do not become colours.
        let soft = RgbaImage::from_fn(400, 10, |x, _| match x {
            0..=198 => Pixel([0, 0, 0, 255]),
            199 => Pixel([128, 128, 128, 255]),
            _ => Pixel([255, 255, 255, 255]),
        });
        let two = palette(&soft, 4).unwrap();
        assert_eq!(two.len(), 2, "{two:?}");
        // Asking for more colours than there are gives what there is.
        assert_eq!(palette(&image, 8).unwrap().len(), 3);
        assert!(palette(&image, 0).is_err());
        assert!(colour_masks(&image, &[]).is_err());
        assert_eq!(hex([255, 0, 16]), "#ff0010");
        // Transparent pixels are no colour.
        let clear = RgbaImage::from_pixel(4, 4, Pixel([0, 0, 0, 0]));
        assert!(palette(&clear, 2).unwrap().is_empty());
    }
}
