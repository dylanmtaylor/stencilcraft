//! Bounded raster-to-vector threshold silhouette tracing for the CLI.

use std::collections::BTreeSet;
use std::io::Cursor;

use image::{ImageReader, Limits};
use stencilcraft_doc::{CutStyle, Document, Layer};
use stencilcraft_geom::BezPath;
use stencilcraft_geom::Point;

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_IMAGE_SIDE: u32 = 10_000;
const MAX_PIXELS: usize = 16_000_000;
const MAX_BOUNDARY_EDGES: usize = 1_000_000;
const MAX_CONTOURS: usize = 100_000;
const PIXELS_PER_INCH: f64 = 96.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceOptions {
    /// Pixels with white-composited luminance <= threshold are foreground.
    pub threshold: u8,
}

impl Default for TraceOptions {
    fn default() -> Self {
        Self { threshold: 128 }
    }
}

/// Read PNG/JPEG bytes under strict limits, then make one closed cut path per
/// connected boundary contour. The SVG/CSS UI isn't involved; imports remain
/// deterministic and work headlessly.
pub fn trace_file(path: &str, options: TraceOptions) -> Result<Document, String> {
    let metadata = std::fs::metadata(path).map_err(|error| format!("{path}: {error}"))?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!("{path}: image exceeds the 64 MiB input limit"));
    }
    let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
    trace_bytes(&bytes, options).map_err(|error| format!("{path}: {error}"))
}

fn trace_bytes(bytes: &[u8], options: TraceOptions) -> Result<Document, String> {
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("image exceeds the 64 MiB input limit".to_string());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("could not identify image format: {error}"))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|error| format!("could not decode PNG/JPEG image: {error}"))?;
    let (width, height) = (decoded.width(), decoded.height());
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| "image dimensions overflow".to_string())?;
    if width == 0 || height == 0 || pixel_count > MAX_PIXELS {
        return Err(format!(
            "image dimensions {width}×{height} exceed the 16-million-pixel trace limit"
        ));
    }

    let rgba = decoded.to_rgba8();
    let foreground: Vec<bool> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| {
            let alpha = u16::from(pixel[3]);
            let luminance = (299_u32 * u32::from(pixel[0])
                + 587_u32 * u32::from(pixel[1])
                + 114_u32 * u32::from(pixel[2])
                + 500)
                / 1000;
            let composite = (luminance * u32::from(alpha)
                + 255 * u32::from(255_u8.saturating_sub(pixel[3]))
                + 127)
                / 255;
            composite <= u32::from(options.threshold)
        })
        .collect();

    let mut edges = BTreeSet::<u64>::new();
    let mut edge_count = 0_usize;
    let grid_width = u64::from(width) + 1;
    let mut add_edge = |x: u32, y: u32, direction: u64| -> Result<(), String> {
        edge_count = edge_count
            .checked_add(1)
            .ok_or_else(|| "trace boundary edge count overflow".to_string())?;
        if edge_count > MAX_BOUNDARY_EDGES {
            return Err("trace exceeds the 1,000,000-boundary-edge limit".to_string());
        }
        let vertex = u64::from(y)
            .checked_mul(grid_width)
            .and_then(|row| row.checked_add(u64::from(x)))
            .ok_or_else(|| "image grid index overflow".to_string())?;
        edges.insert(vertex * 4 + direction);
        Ok(())
    };

    for y in 0..height {
        for x in 0..width {
            let index = (y as usize) * width as usize + x as usize;
            if !foreground[index] {
                continue;
            }
            if y == 0 || !foreground[((y - 1) as usize) * width as usize + x as usize] {
                add_edge(x, y, 0)?; // top, east
            }
            if x + 1 == width || !foreground[y as usize * width as usize + x as usize + 1] {
                add_edge(x + 1, y, 1)?; // right, south
            }
            if y + 1 == height || !foreground[((y + 1) as usize) * width as usize + x as usize] {
                add_edge(x + 1, y + 1, 2)?; // bottom, west
            }
            if x == 0 || !foreground[y as usize * width as usize + x as usize - 1] {
                add_edge(x, y + 1, 3)?; // left, north
            }
        }
    }

    let pixels_to_mm = 25.4 / PIXELS_PER_INCH;
    let mut doc = Document {
        board: stencilcraft_doc::Board {
            width_mm: f64::from(width) * pixels_to_mm,
            height_mm: f64::from(height) * pixels_to_mm,
        },
        layers: vec![Layer::new("trace", CutStyle::Cut)],
        ..Document::default()
    };
    let mut contour_count = 0_usize;
    while let Some(&first_edge) = edges.iter().next() {
        contour_count += 1;
        if contour_count > MAX_CONTOURS {
            return Err("trace exceeds the 100,000-contour limit".to_string());
        }
        let mut edge = first_edge;
        let start_vertex = edge / 4;
        let mut points = Vec::<Point>::new();
        let mut steps = 0_usize;
        loop {
            let vertex = edge / 4;
            let direction = edge % 4;
            let x = vertex % grid_width;
            let y = vertex / grid_width;
            points.push(Point::new(x as f64 * pixels_to_mm, y as f64 * pixels_to_mm));
            edges.remove(&edge);
            steps += 1;
            if steps > MAX_BOUNDARY_EDGES {
                return Err("trace contour exceeded the edge limit".to_string());
            }
            let next_vertex = match direction {
                0 => vertex + 1,
                1 => vertex + grid_width,
                2 => vertex - 1,
                _ => vertex - grid_width,
            };
            if next_vertex == start_vertex {
                break;
            }
            let mut next = None;
            // Keep foreground on the right; at diagonal-touch vertices, a
            // rightmost turn separates touching pixel components cleanly.
            for turn in [1_u64, 0, 3, 2] {
                let candidate = next_vertex * 4 + (direction + turn) % 4;
                if edges.contains(&candidate) {
                    next = Some(candidate);
                    break;
                }
            }
            edge = next.ok_or_else(|| "trace generated an open boundary".to_string())?;
        }
        edge_count = edge_count.saturating_sub(steps);
        simplify_collinear(&mut points);
        if points.len() >= 3 {
            let mut path = BezPath::new();
            let first = points[0];
            path.move_to(first);
            for point in points.iter().skip(1) {
                path.line_to(*point);
            }
            path.close_path();
            doc.push_shape(0, path, CutStyle::Cut)
                .map_err(|error| error.to_string())?;
        }
    }
    doc.validate().map_err(|error| error.to_string())?;
    Ok(doc)
}

/// Remove intermediate boundary vertices that lie on the same grid line.
fn simplify_collinear(points: &mut Vec<Point>) {
    if points.len() < 3 {
        return;
    }
    let mut simplified = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let previous = points[(index + points.len() - 1) % points.len()];
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        let incoming = current - previous;
        let outgoing = next - current;
        if incoming.x * outgoing.y - incoming.y * outgoing.x != 0.0 {
            simplified.push(current);
        }
    }
    *points = simplified;
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn png(width: u32, height: u32, pixels: impl Fn(u32, u32) -> Rgba<u8>) -> Vec<u8> {
        let image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(width, height, pixels);
        let mut output = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut output, image::ImageFormat::Png)
            .expect("encode png");
        output.into_inner()
    }

    #[test]
    fn traces_a_closed_dark_silhouette_and_preserves_scale() {
        let bytes = png(4, 3, |x, y| {
            if x == 1 || x == 2 && y == 1 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let doc = trace_bytes(&bytes, TraceOptions::default()).expect("traces PNG");
        assert_eq!(doc.shape_count(), 1);
        assert!((doc.board.width_mm - 4.0 * 25.4 / 96.0).abs() < 1e-9);
        let polylines = stencilcraft_geom::path_to_polylines(
            &doc.layers[0].shapes[0].path,
            stencilcraft_geom::DEFAULT_FLATTEN_STEPS,
        );
        assert_eq!(polylines.len(), 1);
        assert!(polylines[0].closed);
        assert_eq!(polylines[0].points.len(), 8);
    }

    #[test]
    fn traces_disjoint_contours_and_white_holes() {
        let bytes = png(7, 5, |x, y| {
            if x < 5 && y < 5 && (x == 0 || x == 4 || y == 0 || y == 4) || x == 6 && y == 2 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let doc = trace_bytes(&bytes, TraceOptions::default()).expect("traces contours");
        assert_eq!(
            doc.shape_count(),
            3,
            "outer boundary, enclosed hole, and separate island"
        );
        let paths = doc.layers[0]
            .shapes
            .iter()
            .map(|shape| stencilcraft_geom::path_to_polylines(&shape.path, 4))
            .collect::<Vec<_>>();
        assert!(
            paths
                .iter()
                .all(|contours| contours.len() == 1 && contours[0].closed)
        );
    }

    #[test]
    fn transparent_black_is_not_traced_as_foreground() {
        let bytes = png(2, 2, |_, _| Rgba([0, 0, 0, 0]));
        let doc = trace_bytes(&bytes, TraceOptions::default()).expect("traces PNG");
        assert_eq!(doc.shape_count(), 0);
    }

    #[test]
    fn rejects_malformed_and_oversized_inputs() {
        assert!(trace_bytes(b"not an image", TraceOptions::default()).is_err());
        assert!(
            trace_bytes(
                &vec![0; MAX_FILE_BYTES as usize + 1],
                TraceOptions::default()
            )
            .unwrap_err()
            .contains("64 MiB")
        );
    }

    #[test]
    fn decodes_jpeg_raster_inputs() {
        let image =
            image::DynamicImage::ImageRgb8(ImageBuffer::from_pixel(2, 2, image::Rgb([0_u8, 0, 0])));
        let mut output = Cursor::new(Vec::new());
        image
            .write_to(&mut output, image::ImageFormat::Jpeg)
            .expect("encode jpeg");
        let doc = trace_bytes(output.get_ref(), TraceOptions::default()).expect("traces JPEG");
        assert_eq!(doc.shape_count(), 1);
    }

    #[test]
    fn threshold_is_configurable() {
        let bytes = png(1, 1, |_, _| Rgba([160, 160, 160, 255]));
        assert_eq!(
            trace_bytes(&bytes, TraceOptions { threshold: 128 })
                .unwrap()
                .shape_count(),
            0
        );
        assert_eq!(
            trace_bytes(&bytes, TraceOptions { threshold: 160 })
                .unwrap()
                .shape_count(),
            1
        );
    }
}
