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
        let Some(source) = premultiplied(&pixels) else {
            continue;
        };
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
            fill_shape(&mut pixmap, shape, &path, scale);
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    let mut out = RgbaImage::new(pixmap.width(), pixmap.height());
    for (dst, src) in out
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(pixmap.data().as_chunks::<4>().0)
    {
        // The sheet is opaque white, so premultiplied equals straight colour.
        dst.copy_from_slice(src);
    }
    Ok(out)
}

/// Pixels as the premultiplied RGBA tiny-skia draws from.
fn premultiplied(pixels: &RgbaImage) -> Option<Pixmap> {
    let mut source = Pixmap::new(pixels.width(), pixels.height())?;
    for (dst, src) in source
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(pixels.as_raw().as_chunks::<4>().0)
    {
        let a = u16::from(src[3]);
        for c in 0..3 {
            dst[c] = ((u16::from(src[c]) * a + 127) / 255) as u8;
        }
        dst[3] = src[3];
    }
    Some(source)
}

/// A shape's pattern fill, else its fill colour, inside `path` (page pixels, even-odd).
fn fill_shape(
    pixmap: &mut Pixmap,
    shape: &stencilcraft_doc::Shape,
    path: &tiny_skia::Path,
    scale: f64,
) {
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    let tile = shape
        .production
        .fill_pattern
        .as_deref()
        .and_then(|pattern| {
            Some((
                pattern,
                premultiplied(&stencilcraft_raster::pattern_pixels(pattern).ok()?)?,
            ))
        });
    if let Some((pattern, tile)) = &tile {
        let per_px = (pattern.tile_mm * scale / f64::from(tile.width().max(1))) as f32;
        let place = Transform::from_rotate(pattern.angle_deg as f32).pre_scale(per_px, per_px);
        paint.shader = tiny_skia::Pattern::new(
            tile.as_ref(),
            tiny_skia::SpreadMode::Repeat,
            tiny_skia::FilterQuality::Bilinear,
            1.0,
            place,
        );
    } else if let Some((g, [r0, g0, b0], [r1, g1, b1])) = shape
        .production
        .fill_gradient
        .as_deref()
        .and_then(|g| Some((g, hex_rgb(&g.from)?, hex_rgb(&g.to)?)))
    {
        // The gradient's ends across the shape's bounds, in page pixels.
        let bounds = kurbo::Shape::bounding_box(&shape.path);
        let (sin, cos) = g.angle_deg.to_radians().sin_cos();
        let half = (bounds.width() * cos.abs() + bounds.height() * sin.abs()) / 2.0;
        let c = bounds.center();
        let end = |s: f64| {
            tiny_skia::Point::from_xy(
                ((c.x + s * half * cos) * scale) as f32,
                ((c.y - s * half * sin) * scale) as f32,
            )
        };
        let Some(shader) = tiny_skia::LinearGradient::new(
            end(-1.0),
            end(1.0),
            vec![
                tiny_skia::GradientStop::new(0.0, Color::from_rgba8(r0, g0, b0, 0xff)),
                tiny_skia::GradientStop::new(1.0, Color::from_rgba8(r1, g1, b1, 0xff)),
            ],
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        ) else {
            return;
        };
        paint.shader = shader;
    } else if let Some([r, g, b]) = shape.production.fill_color.as_deref().and_then(hex_rgb) {
        paint.set_color_rgba8(r, g, b, 0xff);
    } else {
        return;
    }
    pixmap.fill_path(
        path,
        &paint,
        tiny_skia::FillRule::EvenOdd,
        Transform::identity(),
        None,
    );
}

/// `#rrggbb` as R, G, B.
fn hex_rgb(hex: &str) -> Option<[u8; 3]> {
    let digits = hex.trim().trim_start_matches('#');
    if digits.len() != 6 || !digits.is_ascii() {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
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

    #[test]
    fn fills_and_pattern_fills_render() {
        let mut doc = Document::default();
        doc.board.width_mm = 50.0;
        doc.board.height_mm = 20.0;
        let square = |x: f64| {
            let mut p = stencilcraft_geom::BezPath::new();
            p.move_to((x, 2.0));
            p.line_to((x + 16.0, 2.0));
            p.line_to((x + 16.0, 18.0));
            p.line_to((x, 18.0));
            p.close_path();
            p
        };
        let solid = doc.push_shape(0, square(2.0), CutStyle::Cut).unwrap().id;
        let tiled = doc.push_shape(0, square(30.0), CutStyle::Cut).unwrap().id;
        doc.get_shape_mut(solid).unwrap().production.fill_color = Some("#00ff00".into());
        let tile =
            RgbaImage::from_pixel(2, 2, craft_raster_codecs::common::Pixel([0, 0, 255, 255]));
        doc.get_shape_mut(tiled).unwrap().production.fill_pattern =
            Some(Box::new(stencilcraft_doc::FillPattern {
                png_base64: stencilcraft_raster::encode_png_base64(&tile)
                    .unwrap()
                    .into(),
                tile_mm: 5.0,
                angle_deg: 30.0,
            }));
        let out = render(&doc, 72.0).unwrap();
        let at = |x_mm: f64, y_mm: f64| {
            let px = out.get_pixel((x_mm / 25.4 * 72.0) as u32, (y_mm / 25.4 * 72.0) as u32);
            [px.0[0], px.0[1], px.0[2]]
        };
        assert_eq!(at(10.0, 10.0), [0, 255, 0], "solid fill");
        assert_eq!(at(38.0, 10.0), [0, 0, 255], "pattern fill");
        assert_eq!(at(25.0, 10.0), [255, 255, 255], "between them");
        doc.get_shape_mut(solid).unwrap().production.fill_gradient =
            Some(Box::new(stencilcraft_doc::FillGradient {
                from: "#ff0000".into(),
                to: "#0000ff".into(),
                angle_deg: 0.0,
            }));
        let out = render(&doc, 72.0).unwrap();
        let px = |x_mm: f64| {
            out.get_pixel((x_mm / 25.4 * 72.0) as u32, (10.0 / 25.4 * 72.0) as u32)
                .0
        };
        let (left, right) = (px(3.0), px(17.0));
        assert!(left[0] > 200 && left[2] < 60, "red end {left:?}");
        assert!(right[2] > 200 && right[0] < 60, "blue end {right:?}");
        // At 90° the gradient starts at the bottom of the page-down shape and ends at its top.
        if let Some(g) = doc
            .get_shape_mut(solid)
            .unwrap()
            .production
            .fill_gradient
            .as_mut()
        {
            g.angle_deg = 90.0;
        }
        let out = render(&doc, 72.0).unwrap();
        let py = |y_mm: f64| {
            out.get_pixel((10.0 / 25.4 * 72.0) as u32, (y_mm / 25.4 * 72.0) as u32)
                .0
        };
        let (top, bottom) = (py(3.0), py(17.0));
        assert!(top[2] > 200 && top[0] < 60, "blue at the top {top:?}");
        assert!(
            bottom[0] > 200 && bottom[2] < 60,
            "red at the bottom {bottom:?}"
        );
    }

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
