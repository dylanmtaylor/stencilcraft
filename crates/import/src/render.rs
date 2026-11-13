//! Document → PNG/JPEG at a chosen resolution: white sheet, placed images, then cut
//! lines red and engrave lines blue (the SVG exporter's colours), hairlines 0.25 mm.

use craft_raster_codecs::common::Pixels as RgbaImage;
use stencilcraft_doc::{CutStyle, Document};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform};

use stencilcraft_raster::{RasterError, image_pixels};

fn bad(message: impl Into<String>) -> RasterError {
    RasterError::Bad(message.into())
}

/// Most pixels one render may allocate (a 4 bytes per pixel buffer of 256 MiB).
pub const MAX_RENDER_PIXELS: u64 = 64_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderFormat {
    Png,
    Jpeg,
}

impl RenderFormat {
    /// From a file extension (`png`, `jpg`, `jpeg`, any case).
    pub fn from_extension(ext: &str) -> Option<Self> {
        match craft_file_import::write::writer_format_for_extension(ext)? {
            craft_file_import::write::WriteFormat::Png => Some(Self::Png),
            craft_file_import::write::WriteFormat::Jpeg => Some(Self::Jpeg),
            _ => None,
        }
    }
}

/// Render the board at `dpi` (pixels per inch, e.g. 300, 600, 1200).
pub fn render(doc: &Document, dpi: f64) -> Result<RgbaImage, RasterError> {
    if !(dpi.is_finite() && (1.0..=4800.0).contains(&dpi)) {
        return Err(bad(format!("render: dpi {dpi} is outside 1–4800")));
    }
    doc.validate().map_err(|e| bad(e.to_string()))?;
    let scale = dpi / 25.4;
    let (w, h) = (
        (doc.board.width_mm * scale).ceil(),
        (doc.board.height_mm * scale).ceil(),
    );
    if w * h > MAX_RENDER_PIXELS as f64 {
        return Err(bad(format!(
            "render: {w}×{h} pixels exceeds the 64-million-pixel limit; lower the dpi"
        )));
    }
    let mut pixmap = Pixmap::new(w.max(1.0) as u32, h.max(1.0) as u32)
        .ok_or_else(|| bad("render: could not allocate the page"))?;
    pixmap.fill(Color::WHITE);
    let page = Transform::from_scale(scale as f32, scale as f32);

    for placed in &doc.images {
        let pixels = image_pixels(placed)?;
        let Some(mut source) = Pixmap::new(pixels.width(), pixels.height()) else {
            continue;
        };
        // tiny-skia wants premultiplied RGBA.
        for (dst, src) in source
            .data_mut()
            .chunks_exact_mut(4)
            .zip(pixels.as_raw().chunks_exact(4))
        {
            let a = u16::from(src[3]);
            for c in 0..3 {
                dst[c] = ((u16::from(src[c]) * a + 127) / 255) as u8;
            }
            dst[3] = src[3];
        }
        let placement = page
            .pre_translate(placed.x_mm as f32, placed.y_mm as f32)
            .pre_scale(
                (placed.width_mm / f64::from(placed.width_px)) as f32,
                (placed.height_mm / f64::from(placed.height_px)) as f32,
            );
        pixmap.draw_pixmap(
            0,
            0,
            source.as_ref(),
            &PixmapPaint::default(),
            placement,
            None,
        );
    }

    let stroke = Stroke {
        width: (0.25 * scale).max(1.0) as f32,
        ..Stroke::default()
    };
    for layer in doc.layers.iter().filter(|layer| layer.visible) {
        for shape in &layer.shapes {
            let mut paint = Paint {
                anti_alias: true,
                ..Paint::default()
            };
            match shape.style {
                CutStyle::Cut => paint.set_color_rgba8(0xc0, 0x39, 0x2b, 0xff),
                CutStyle::Engrave => paint.set_color_rgba8(0x2d, 0x6c, 0xdf, 0xff),
            }
            let Some(path) = skia_path(&shape.path, scale) else {
                continue;
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    let mut out = RgbaImage::new(pixmap.width(), pixmap.height());
    for (dst, src) in out.chunks_exact_mut(4).zip(pixmap.data().chunks_exact(4)) {
        // The sheet is opaque white, so premultiplied equals straight colour.
        dst.copy_from_slice(src);
    }
    Ok(out)
}

/// The path in page pixels (scaled here so stroke width is in pixels).
fn skia_path(path: &stencilcraft_geom::BezPath, scale: f64) -> Option<tiny_skia::Path> {
    let p = |point: kurbo::Point| ((point.x * scale) as f32, (point.y * scale) as f32);
    let mut builder = PathBuilder::new();
    for element in path.elements() {
        match *element {
            kurbo::PathEl::MoveTo(a) => {
                let (x, y) = p(a);
                builder.move_to(x, y);
            }
            kurbo::PathEl::LineTo(a) => {
                let (x, y) = p(a);
                builder.line_to(x, y);
            }
            kurbo::PathEl::QuadTo(a, b) => {
                let ((ax, ay), (bx, by)) = (p(a), p(b));
                builder.quad_to(ax, ay, bx, by);
            }
            kurbo::PathEl::CurveTo(a, b, c) => {
                let ((ax, ay), (bx, by), (cx, cy)) = (p(a), p(b), p(c));
                builder.cubic_to(ax, ay, bx, by, cx, cy);
            }
            kurbo::PathEl::ClosePath => builder.close(),
        }
    }
    builder.finish()
}

/// Render and encode.
pub fn render_document(
    doc: &Document,
    dpi: f64,
    format: RenderFormat,
) -> Result<Vec<u8>, RasterError> {
    let pixels = render(doc, dpi)?;
    let extension = match format {
        RenderFormat::Png => "png",
        RenderFormat::Jpeg => "jpg",
    };
    craft_file_import::write::write_raster(
        extension,
        pixels.width(),
        pixels.height(),
        pixels.as_raw(),
        90,
    )
    .map_err(|e| bad(format!("render: could not encode: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_at_dpi_with_cut_lines() {
        let mut doc = Document::default();
        doc.board.width_mm = 25.4;
        doc.board.height_mm = 25.4;
        let mut path = stencilcraft_geom::BezPath::new();
        path.move_to((5.0, 5.0));
        path.line_to((20.0, 5.0));
        path.line_to((20.0, 20.0));
        path.close_path();
        doc.push_shape(0, path, CutStyle::Cut).unwrap();
        let img = render(&doc, 300.0).unwrap();
        assert_eq!(img.dimensions(), (300, 300));
        let red = img.pixels().filter(|p| p[0] > 150 && p[1] < 100).count();
        assert!(red > 100, "{red}");
        assert!(render(&doc, 0.0).is_err());
        doc.board.width_mm = 10_000.0;
        assert!(render(&doc, 1200.0).is_err(), "pixel cap");
        doc.board.width_mm = 25.4;
        for format in [RenderFormat::Png, RenderFormat::Jpeg] {
            let bytes = render_document(&doc, 96.0, format).unwrap();
            assert_eq!(
                stencilcraft_raster::decode(&bytes).unwrap().dimensions(),
                (96, 96)
            );
        }
    }
}
