//! Raster images for StencilCraft: bounded decoding of untrusted PNG, JPEG, BMP, GIF,
//! TIFF and WebP files, placing them in a document, tracing them into cut paths
//! (`trace`) and removing a flat background. (Rendering a document to PNG/JPEG is an
//! export, so it lives in `stencilcraft-import`, keeping the engine format-free.)
//!
//! Every decoder runs under byte, side, pixel and allocation caps, so a hostile file is
//! an error, never a panic or an out-of-memory abort (`AGENTS.md` › Never crash).

mod trace;

use std::io::Cursor;

use base64::Engine as _;
use image::{ImageReader, Limits, RgbaImage};
use stencilcraft_doc::RasterImage;

pub use trace::{ColourRange, Contour, TraceMode, TraceOptions, TraceOutput, contour_path, trace};

/// Largest encoded file accepted.
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
/// Largest side, in pixels.
pub const MAX_SIDE: u32 = 10_000;
/// Largest image, in pixels.
pub const MAX_PIXELS: u64 = 16_000_000;
const MAX_ALLOC: u64 = 256 * 1024 * 1024;

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
    if bytes.len() > MAX_FILE_BYTES {
        return Err(bad("the file exceeds the 64 MiB input limit"));
    }
    if is_heif(bytes) {
        return Err(bad(
            "HEIC/HEIF images are not supported yet; convert the file to PNG or JPEG",
        ));
    }
    let reader = || -> Result<ImageReader<Cursor<&[u8]>>, RasterError> {
        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| bad(format!("could not identify the format: {e}")))?;
        if reader.format().is_none() {
            return Err(bad(
                "unrecognised format (PNG, JPEG, BMP, GIF, TIFF and WebP are supported)",
            ));
        }
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_SIDE);
        limits.max_image_height = Some(MAX_SIDE);
        limits.max_alloc = Some(MAX_ALLOC);
        reader.limits(limits);
        Ok(reader)
    };
    // Read the header first so the pixel cap holds before any pixel is allocated.
    let (width, height) = reader()?
        .into_dimensions()
        .map_err(|e| bad(format!("could not read the image header: {e}")))?;
    check_size(width, height)?;
    let decoded = reader()?
        .decode()
        .map_err(|e| bad(format!("could not decode the image: {e}")))?;
    check_size(decoded.width(), decoded.height())?;
    Ok(decoded.to_rgba8())
}

fn check_size(width: u32, height: u32) -> Result<(), RasterError> {
    if width == 0 || height == 0 {
        return Err(bad("the image is empty"));
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(bad(format!(
            "{width}×{height} exceeds the 16-million-pixel limit"
        )));
    }
    Ok(())
}

/// ISO BMFF `ftyp` box naming a HEIF brand.
fn is_heif(bytes: &[u8]) -> bool {
    bytes.get(4..8) == Some(b"ftyp")
        && matches!(
            bytes.get(8..12),
            Some(b"heic" | b"heix" | b"hevc" | b"mif1" | b"msf1" | b"avif")
        )
}

/// Encode pixels as base64 PNG, the document's image storage.
pub fn encode_png_base64(pixels: &RgbaImage) -> Result<String, RasterError> {
    let mut out = Cursor::new(Vec::new());
    pixels
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| bad(format!("could not encode PNG: {e}")))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(out.into_inner()))
}

/// Decode a placed image's pixels (document data is untrusted too).
pub fn image_pixels(image: &RasterImage) -> Result<RgbaImage, RasterError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image.png_base64.as_bytes())
        .map_err(|e| bad(format!("image {}: bad pixel data: {e}", image.id)))?;
    let pixels = decode(&bytes)?;
    if pixels.dimensions() != (image.width_px, image.height_px) {
        return Err(bad(format!(
            "image {}: pixel data does not match its {}×{} size",
            image.id, image.width_px, image.height_px
        )));
    }
    Ok(pixels)
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
    use image::Rgba;

    pub(crate) fn encode(image: &RgbaImage, format: image::ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        match format {
            image::ImageFormat::Jpeg | image::ImageFormat::Bmp => {
                image::DynamicImage::ImageRgba8(image.clone())
                    .to_rgb8()
                    .write_to(&mut out, format)
                    .unwrap();
            }
            _ => image.write_to(&mut out, format).unwrap(),
        }
        out.into_inner()
    }

    #[test]
    fn decodes_every_supported_format() {
        let image = RgbaImage::from_fn(8, 6, |x, _| {
            if x < 4 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        for format in [
            image::ImageFormat::Png,
            image::ImageFormat::Jpeg,
            image::ImageFormat::Bmp,
            image::ImageFormat::Gif,
            image::ImageFormat::Tiff,
            image::ImageFormat::WebP,
        ] {
            let decoded =
                decode(&encode(&image, format)).unwrap_or_else(|e| panic!("{format:?}: {e}"));
            assert_eq!(decoded.dimensions(), (8, 6), "{format:?}");
        }
    }

    #[test]
    fn rejects_junk_heif_and_oversized_headers() {
        assert!(decode(b"not an image").is_err());
        assert!(decode(b"").is_err());
        let heic = b"\0\0\0\x18ftypheic\0\0\0\0mif1heic";
        assert!(decode(heic).unwrap_err().to_string().contains("HEIC"));
        // A BMP header claiming 60000×60000 must fail before allocating pixels.
        let mut bmp = encode(&RgbaImage::new(1, 1), image::ImageFormat::Bmp);
        bmp[18..22].copy_from_slice(&60_000_i32.to_le_bytes());
        bmp[22..26].copy_from_slice(&60_000_i32.to_le_bytes());
        assert!(decode(&bmp).is_err());
    }

    #[test]
    fn place_sizes_by_dpi_and_round_trips_pixels() {
        let pixels = RgbaImage::from_pixel(96, 48, Rgba([10, 20, 30, 255]));
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
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([250, 252, 255, 255])
            }
        });
        let out = remove_background(&pixels, 5.0).unwrap();
        assert_eq!(out.get_pixel(0, 0)[3], 0);
        assert_eq!(out.get_pixel(4, 4)[3], 255, "enclosed white stays");
        assert_eq!(out.get_pixel(2, 4)[3], 255, "the ring stays");
        assert!(remove_background(&pixels, 101.0).is_err());
    }
}
