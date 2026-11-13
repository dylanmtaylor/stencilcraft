//! Print Then Cut / Print & Cut sheets: the page a home printer prints, with registration marks
//! and bleed, as PDF or PNG.

use kurbo::{BezPath, PathEl, Rect};
use serde::{Deserialize, Serialize};
use stencilcraft_doc::{Document, GroupKind, Shape};
use stencilcraft_kerf::{JoinStyle, OffsetOptions};

use crate::machines::{MachineProfile, MarkStyle, machine};
use crate::mats::{MatOptions, PageOptions, layout_mats};
use crate::{Result, check_range, invalid};

/// Clearance between the mark lines and the artwork.
const ART_GAP_MM: f64 = 2.0;
/// Largest PNG this exporter makes.
const MAX_PIXELS: u64 = 60_000_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrintOptions {
    pub mats: MatOptions,
    /// Print past the cut line so a slightly-off cut shows no white edge (on by default).
    pub bleed: bool,
    /// Bleed radius in mm (0–10; default 0.05 in).
    pub bleed_mm: f64,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            mats: MatOptions::default(),
            bleed: true,
            bleed_mm: 0.05 * 25.4,
        }
    }
}

/// One filled area of print colour, in page millimetres (y down).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrintItem {
    pub shape_id: u64,
    /// `#rrggbb`.
    pub color: String,
    /// A bleed band (drawn under the artwork).
    pub bleed: bool,
    pub path: BezPath,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrintSheet {
    /// The mat (in the layout) this page prints.
    pub mat: usize,
    pub width_mm: f64,
    pub height_mm: f64,
    pub style: MarkStyle,
    /// Filled mark rectangles `[x, y, width, height]`.
    pub marks: Vec<[f64; 4]>,
    /// The area artwork may use `[x0, y0, x1, y1]`.
    pub art_area: [f64; 4],
    pub items: Vec<PrintItem>,
}

fn marks_box(page: &PageOptions) -> Rect {
    let i = page.mark_inset_mm;
    Rect::new(i, i, page.width_mm - i, page.height_mm - i)
}

/// The printable artwork area: inside the marks, clamped to the machine's Print Then Cut limit.
pub fn art_area(page: &PageOptions, profile: &MachineProfile) -> Rect {
    let g = page.mark_thickness_mm + ART_GAP_MM;
    let m = marks_box(page);
    let b = Rect::new(m.x0 + g, m.y0 + g, m.x1 - g, m.y1 - g);
    match profile.print_max_mm {
        Some((w, h)) => Rect::new(b.x0, b.y0, b.x1.min(b.x0 + w), b.y1.min(b.y0 + h)),
        None => b,
    }
}

/// Mark rectangles `[x, y, width, height]` for a page: a solid square outside the top-left
/// corner of the marks box (Silhouette and Cricut styles) and L brackets on its corners.
pub fn registration_marks(page: &PageOptions, style: MarkStyle) -> Vec<[f64; 4]> {
    let b = marks_box(page);
    let (len, t) = (page.mark_length_mm, page.mark_thickness_mm);
    let q = len / 2.0;
    let mut out = Vec::new();
    let bracket = |out: &mut Vec<[f64; 4]>, cx: f64, cy: f64, dx: f64, dy: f64| {
        let hx = if dx > 0.0 { cx } else { cx - len };
        let hy = if dy > 0.0 { cy } else { cy - t };
        out.push([hx, hy, len, t]);
        let vx = if dx > 0.0 { cx } else { cx - t };
        let vy = if dy > 0.0 { cy } else { cy - len };
        out.push([vx, vy, t, len]);
    };
    if style != MarkStyle::FourCorner {
        out.push([b.x0 - q, b.y0 - q, q, q]);
    } else {
        bracket(&mut out, b.x0, b.y0, 1.0, 1.0);
    }
    bracket(&mut out, b.x1, b.y0, -1.0, 1.0);
    bracket(&mut out, b.x0, b.y1, 1.0, -1.0);
    if style != MarkStyle::Silhouette {
        bracket(&mut out, b.x1, b.y1, -1.0, -1.0);
    }
    out
}

fn print_color(shape: &Shape) -> String {
    shape
        .production
        .fill_color
        .clone()
        .or_else(|| shape.production.line_color.clone())
        .unwrap_or_else(|| "#000000".to_string())
}

/// Artwork of one shape: a flattened shape prints its members, anything else prints itself.
fn artwork(shape: &Shape) -> Vec<Shape> {
    match &shape.production.group {
        Some(g) if g.kind == GroupKind::Flatten => g.members_now(&shape.path),
        _ => vec![shape.clone()],
    }
}

/// One sheet per Print Then Cut mat of the layout.
pub fn print_sheets(doc: &Document, options: &PrintOptions) -> Result<Vec<PrintSheet>> {
    check_range(options.bleed_mm, 0.0, 10.0, "bleed_mm")?;
    let layout = layout_mats(doc, &options.mats)?;
    let Some(profile) = machine(&layout.machine) else {
        return invalid(format!("unknown machine {}", layout.machine));
    };
    let page = &options.mats.page;
    let style = page.marks.unwrap_or(profile.marks);
    let area = art_area(page, profile);
    let offset = OffsetOptions {
        join: JoinStyle::Round,
        ..OffsetOptions::default()
    };
    let mut sheets = Vec::new();
    for mat in layout.mats.iter().filter(|m| m.key.print_then_cut) {
        let mut items = Vec::new();
        for placement in &mat.placements {
            let map = placement.affine();
            for id in &placement.shape_ids {
                let Some(shape) = doc.get_shape(*id) else {
                    continue;
                };
                for art in artwork(shape) {
                    let path = map * art.cut_path().into_owned();
                    let color = print_color(&art);
                    if options.bleed && options.bleed_mm > 0.0 {
                        let grown =
                            stencilcraft_kerf::offset_path(&path, options.bleed_mm, &offset)
                                .map_err(|e| crate::LayoutError::Invalid(format!("bleed: {e}")))?;
                        items.push(PrintItem {
                            shape_id: shape.id,
                            color: color.clone(),
                            bleed: true,
                            path: grown,
                        });
                    }
                    items.push(PrintItem {
                        shape_id: shape.id,
                        color,
                        bleed: false,
                        path,
                    });
                }
            }
        }
        sheets.push(PrintSheet {
            mat: mat.index,
            width_mm: page.width_mm,
            height_mm: page.height_mm,
            style,
            marks: registration_marks(page, style),
            art_area: [area.x0, area.y0, area.x1, area.y1],
            items,
        });
    }
    if sheets.is_empty() {
        return invalid("nothing to print: no shape is Print Then Cut or flattened");
    }
    Ok(sheets)
}

fn rgb(color: &str) -> (u8, u8, u8) {
    let hex = color.trim_start_matches('#');
    let channel = |i: usize| {
        hex.get(i..i + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
            .unwrap_or(0)
    };
    if hex.len() == 6 {
        (channel(0), channel(2), channel(4))
    } else {
        (0, 0, 0)
    }
}

/// A multi-page PDF, one page per sheet, at actual size (print at 100%, no fit-to-page).
pub fn sheets_to_pdf(sheets: &[PrintSheet]) -> Result<Vec<u8>> {
    let pages = sheets
        .iter()
        .map(|sheet| {
            let mut paths = sheet
                .items
                .iter()
                .filter(|item| item.bleed)
                .chain(sheet.items.iter().filter(|item| !item.bleed))
                .map(|item| {
                    let (r, g, b) = rgb(&item.color);
                    stencilcraft_pdf::PdfPath {
                        path: item.path.clone(),
                        color: [r, g, b],
                        stroke_width_mm: None,
                    }
                })
                .collect::<Vec<_>>();
            for mark in &sheet.marks {
                let mut path = BezPath::new();
                path.move_to((mark[0], mark[1]));
                path.line_to((mark[0] + mark[2], mark[1]));
                path.line_to((mark[0] + mark[2], mark[1] + mark[3]));
                path.line_to((mark[0], mark[1] + mark[3]));
                path.close_path();
                paths.push(stencilcraft_pdf::PdfPath {
                    path,
                    color: [0, 0, 0],
                    stroke_width_mm: None,
                });
            }
            stencilcraft_pdf::PdfPage {
                width_mm: sheet.width_mm,
                height_mm: sheet.height_mm,
                paths,
            }
        })
        .collect::<Vec<_>>();
    stencilcraft_pdf::export_pages(&pages)
        .map_err(|error| crate::LayoutError::Invalid(error.to_string()))
}

/// A PNG of one sheet at `dpi` (50–1200) on a white page.
pub fn sheet_to_png(sheet: &PrintSheet, dpi: f64) -> Result<Vec<u8>> {
    check_range(dpi, 50.0, 1200.0, "dpi")?;
    let scale = dpi / 25.4;
    let (w, h) = (
        (sheet.width_mm * scale).ceil(),
        (sheet.height_mm * scale).ceil(),
    );
    if w * h > MAX_PIXELS as f64 {
        return invalid(format!("a {w}×{h} pixel PNG is too large; lower the dpi"));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (pw, ph) = (w as u32, h as u32);
    let Some(mut pixmap) = tiny_skia::Pixmap::new(pw, ph) else {
        return invalid("could not allocate the PNG");
    };
    pixmap.fill(tiny_skia::Color::WHITE);
    let transform = tiny_skia::Transform::from_scale(scale as f32, scale as f32);
    let mut paint = tiny_skia::Paint {
        anti_alias: true,
        ..tiny_skia::Paint::default()
    };
    let draw = |pixmap: &mut tiny_skia::Pixmap,
                paint: &mut tiny_skia::Paint,
                path: &BezPath,
                color: &str| {
        let (r, g, b) = rgb(color);
        paint.set_color_rgba8(r, g, b, 255);
        let mut pb = tiny_skia::PathBuilder::new();
        for el in path.elements() {
            match *el {
                PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
                PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
                PathEl::QuadTo(q, p) => pb.quad_to(q.x as f32, q.y as f32, p.x as f32, p.y as f32),
                PathEl::CurveTo(a, b, p) => pb.cubic_to(
                    a.x as f32, a.y as f32, b.x as f32, b.y as f32, p.x as f32, p.y as f32,
                ),
                PathEl::ClosePath => pb.close(),
            }
        }
        if let Some(path) = pb.finish() {
            pixmap.fill_path(&path, paint, tiny_skia::FillRule::Winding, transform, None);
        }
    };
    for item in sheet
        .items
        .iter()
        .filter(|i| i.bleed)
        .chain(sheet.items.iter().filter(|i| !i.bleed))
    {
        draw(&mut pixmap, &mut paint, &item.path, &item.color);
    }
    for m in &sheet.marks {
        let r = Rect::new(m[0], m[1], m[0] + m[2], m[1] + m[3]);
        let mut p = BezPath::new();
        p.move_to((r.x0, r.y0));
        p.line_to((r.x1, r.y0));
        p.line_to((r.x1, r.y1));
        p.line_to((r.x0, r.y1));
        p.close_path();
        draw(&mut pixmap, &mut paint, &p, "#000000");
    }
    let pixels = craft_raster_codecs::common::Pixels::from_raw(pw, ph, pixmap.data().to_vec())
        .ok_or_else(|| {
            crate::LayoutError::Invalid("could not build the PNG pixel buffer".into())
        })?;
    craft_raster_codecs::common::encode_png(&pixels)
        .map_err(|e| crate::LayoutError::Invalid(format!("png: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rect;
    use stencilcraft_doc::{CutStyle, Operation};

    fn ptc_doc() -> Document {
        let mut doc = Document::default();
        doc.push_shape(0, rect(0.0, 0.0, 40.0, 30.0), CutStyle::Cut)
            .unwrap();
        let s = &mut doc.layers[0].shapes[0].production;
        s.operation = Some(Operation::PrintThenCut);
        s.fill_color = Some("#ff8800".to_string());
        doc
    }

    #[test]
    fn bleed_expands_bounds_by_its_radius() {
        let sheets = print_sheets(&ptc_doc(), &PrintOptions::default()).unwrap();
        let sheet = &sheets[0];
        let art = sheet.items.iter().find(|i| !i.bleed).unwrap();
        let bleed = sheet.items.iter().find(|i| i.bleed).unwrap();
        let a = stencilcraft_geom::bounds_of_shape(&art.path).unwrap();
        let b = stencilcraft_geom::bounds_of_shape(&bleed.path).unwrap();
        let r = PrintOptions::default().bleed_mm;
        for (outer, inner) in [
            (a.x0 - b.x0, r),
            (b.x1 - a.x1, r),
            (a.y0 - b.y0, r),
            (b.y1 - a.y1, r),
        ] {
            assert!((outer - inner).abs() < 0.02, "{outer} vs {inner}");
        }
        // Artwork sits inside the area the marks leave free.
        let area = sheet.art_area;
        assert!(
            a.x0 >= area[0] - 1e-9 && a.y0 >= area[1] - 1e-9,
            "{a:?} {area:?}"
        );
        let none = PrintOptions {
            bleed: false,
            ..PrintOptions::default()
        };
        assert!(
            print_sheets(&ptc_doc(), &none).unwrap()[0]
                .items
                .iter()
                .all(|i| !i.bleed)
        );
    }

    #[test]
    fn marks_styles_and_limits() {
        let page = PageOptions::default();
        assert_eq!(registration_marks(&page, MarkStyle::Silhouette).len(), 5);
        assert_eq!(registration_marks(&page, MarkStyle::Cricut).len(), 7);
        assert_eq!(registration_marks(&page, MarkStyle::FourCorner).len(), 8);
        for m in registration_marks(&page, MarkStyle::Cricut) {
            assert!(
                m[0] >= 0.0
                    && m[1] >= 0.0
                    && m[0] + m[2] <= page.width_mm
                    && m[1] + m[3] <= page.height_mm
            );
        }
        // The Joy 2 prints at most 10.62 × 3.07 in; a taller image is refused.
        let mut doc = ptc_doc();
        doc.layers[0].shapes[0].path = rect(0.0, 0.0, 40.0, 200.0);
        let joy = PrintOptions {
            mats: MatOptions {
                machine: "cricut-joy".to_string(),
                ..MatOptions::default()
            },
            ..PrintOptions::default()
        };
        assert!(print_sheets(&doc, &joy).is_err());
        assert!(print_sheets(&ptc_doc(), &joy).is_ok());
        assert!(print_sheets(&Document::default(), &PrintOptions::default()).is_err());
    }

    #[test]
    fn pdf_and_png_export() {
        let sheets = print_sheets(&ptc_doc(), &PrintOptions::default()).unwrap();
        let pdf = sheets_to_pdf(&sheets).unwrap();
        let text = String::from_utf8(pdf).unwrap();
        assert!(text.starts_with("%PDF-1.4") && text.ends_with("%%EOF\n"));
        assert!(text.contains("/MediaBox [0 0 612.000 792.000]"));
        let png = sheet_to_png(&sheets[0], 72.0).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert!(sheet_to_png(&sheets[0], 5000.0).is_err());
    }
}
