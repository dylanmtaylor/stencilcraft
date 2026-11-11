//! PDF export: one page the size of the board, every visible shape as a vector stroke
//! (cut red, engrave blue, 0.25 mm), for printing at 100 % or handing to a cutter's
//! software, and Print Then Cut sheets as filled pages ([`export_print_sheets`]). Written through craft-libs' shared PDF writer (`craft_pdf::write`, over
//! Krilla). Placed raster images are not written yet.

use craft_pdf::write::backend::SerializeSettings;
use craft_pdf::write::backend::color::rgb;
use craft_pdf::write::backend::geom::{PathBuilder, Transform};
use craft_pdf::write::backend::page::PageSettings;
use craft_pdf::write::backend::paint::{Fill, LineCap, LineJoin, Stroke};
use craft_pdf::write::{Metadata, WriteError, Writer, guard};
use stencilcraft_doc::{CutStyle, Document};
use stencilcraft_layout::PrintSheet;

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("pdf: {0}")]
    Model(String),
    #[error("pdf: {0}")]
    Write(#[from] WriteError),
}

const PT_PER_MM: f64 = 72.0 / 25.4;

pub fn export(doc: &Document) -> Result<Vec<u8>, PdfError> {
    doc.validate().map_err(|e| PdfError::Model(e.to_string()))?;
    let page = PageSettings::from_wh(
        (doc.board.width_mm * PT_PER_MM) as f32,
        (doc.board.height_mm * PT_PER_MM) as f32,
    )
    .ok_or_else(|| PdfError::Model("the board has no printable size".to_string()))?;
    guard(|| {
        let metadata = Metadata {
            creator: "StencilCraft".to_string(),
            producer: "StencilCraft".to_string(),
            ..Metadata::default()
        };
        let mut writer = Writer::new(settings(), metadata);
        let mut page = writer.document_mut().start_page_with(page);
        let mut surface = page.surface();
        // Millimetres, y down, like the document.
        let scale = PT_PER_MM as f32;
        surface.push_transform(&Transform::from_row(scale, 0.0, 0.0, scale, 0.0, 0.0));
        surface.set_fill(None);
        for layer in doc.layers.iter().filter(|layer| layer.visible) {
            for shape in &layer.shapes {
                let colour = match shape.style {
                    CutStyle::Cut => rgb::Color::new(192, 57, 43),
                    CutStyle::Engrave => rgb::Color::new(45, 108, 223),
                };
                // A path with nothing to draw (a lone move) is skipped.
                let Some(path) = outline(&shape.path, shape.id)? else {
                    continue;
                };
                surface.set_stroke(Some(Stroke {
                    paint: colour.into(),
                    width: 0.25,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Stroke::default()
                }));
                surface.draw_path(&path);
            }
        }
        surface.pop();
        surface.finish();
        page.finish();
        Ok(writer.finish()?)
    })
}

/// A document path as a PDF path; `None` when it draws nothing (a lone move).
fn outline(
    source: &kurbo::BezPath,
    shape_id: u64,
) -> Result<Option<craft_pdf::write::backend::geom::Path>, PdfError> {
    let mut path = PathBuilder::new();
    let mut cursor = kurbo::Point::ZERO;
    for element in source.elements() {
        let points: &[kurbo::Point] = match element {
            kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => std::slice::from_ref(p),
            kurbo::PathEl::QuadTo(a, b) => &[*a, *b],
            kurbo::PathEl::CurveTo(a, b, c) => &[*a, *b, *c],
            kurbo::PathEl::ClosePath => &[],
        };
        if points.iter().any(|p| !(p.x.is_finite() && p.y.is_finite())) {
            return Err(PdfError::Model(format!(
                "shape {shape_id}: a point is not finite"
            )));
        }
        let mut cubic = |a: kurbo::Point, b: kurbo::Point, c: kurbo::Point| {
            path.cubic_to(
                a.x as f32, a.y as f32, b.x as f32, b.y as f32, c.x as f32, c.y as f32,
            );
        };
        match *element {
            kurbo::PathEl::MoveTo(p) => {
                path.move_to(p.x as f32, p.y as f32);
                cursor = p;
            }
            kurbo::PathEl::LineTo(p) => {
                path.line_to(p.x as f32, p.y as f32);
                cursor = p;
            }
            kurbo::PathEl::QuadTo(a, b) => {
                let raised = kurbo::QuadBez::new(cursor, a, b).raise();
                cubic(raised.p1, raised.p2, b);
                cursor = b;
            }
            kurbo::PathEl::CurveTo(a, b, c) => {
                cubic(a, b, c);
                cursor = c;
            }
            kurbo::PathEl::ClosePath => path.close(),
        }
    }
    Ok(path.finish())
}

/// Print Then Cut sheets as a multi-page PDF, one page per sheet, at actual size (print
/// at 100%, no fit-to-page): bleed bands under the artwork, then the registration marks.
pub fn export_print_sheets(sheets: &[PrintSheet]) -> Result<Vec<u8>, PdfError> {
    guard(|| {
        let metadata = Metadata {
            creator: "StencilCraft".to_string(),
            producer: "StencilCraft".to_string(),
            ..Metadata::default()
        };
        let mut writer = Writer::new(settings(), metadata);
        for sheet in sheets {
            let page = PageSettings::from_wh(
                (sheet.width_mm * PT_PER_MM) as f32,
                (sheet.height_mm * PT_PER_MM) as f32,
            )
            .ok_or_else(|| PdfError::Model("a print sheet has no printable size".to_string()))?;
            let mut page = writer.document_mut().start_page_with(page);
            let mut surface = page.surface();
            // Millimetres, y down, like the sheet.
            let scale = PT_PER_MM as f32;
            surface.push_transform(&Transform::from_row(scale, 0.0, 0.0, scale, 0.0, 0.0));
            surface.set_stroke(None);
            let mut fill = |source: &kurbo::BezPath, color: &str, id: u64| {
                // Sheet colours are `#rrggbb`; anything else prints black.
                let c = craft_svg_paths::color::parse_color(color).unwrap_or_default();
                if let Some(path) = outline(source, id)? {
                    surface.set_fill(Some(Fill {
                        paint: rgb::Color::new(c.r, c.g, c.b).into(),
                        ..Fill::default()
                    }));
                    surface.draw_path(&path);
                }
                Ok::<(), PdfError>(())
            };
            for item in sheet
                .items
                .iter()
                .filter(|i| i.bleed)
                .chain(sheet.items.iter().filter(|i| !i.bleed))
            {
                fill(&item.path, &item.color, item.shape_id)?;
            }
            for m in &sheet.marks {
                let mark = kurbo::Shape::to_path(
                    &kurbo::Rect::new(m[0], m[1], m[0] + m[2], m[1] + m[3]),
                    0.0,
                );
                fill(&mark, "#000000", 0)?;
            }
            surface.pop();
            surface.finish();
            page.finish();
        }
        Ok(writer.finish()?)
    })
}

/// Plain device RGB, uncompressed so the sheet stays readable in a text editor.
fn settings() -> SerializeSettings {
    SerializeSettings {
        compress_content_streams: false,
        ..SerializeSettings::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_page_that_a_pdf_reader_opens() {
        let mut doc = Document::default();
        let mut path = kurbo::BezPath::new();
        path.move_to((10.0, 10.0));
        path.line_to((60.0, 10.0));
        path.quad_to((60.0, 60.0), (10.0, 60.0));
        path.close_path();
        doc.push_shape(0, path, CutStyle::Cut).unwrap();
        let pdf = export(&doc).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        // An independent reader (VectorCraft's, over hayro) finds the path.
        let read = vectorcraft_pdf::import(&pdf).unwrap();
        assert!(!read.layers.is_empty());
        let mut bad = doc.clone();
        bad.board.width_mm = f64::NAN;
        assert!(export(&bad).is_err());
    }

    #[test]
    fn print_sheets_become_one_filled_page_each() {
        let mut doc = Document::default();
        doc.push_shape(
            0,
            kurbo::Shape::to_path(&kurbo::Rect::new(0.0, 0.0, 40.0, 30.0), 0.0),
            CutStyle::Cut,
        )
        .unwrap();
        let production = &mut doc.layers[0].shapes[0].production;
        production.operation = Some(stencilcraft_doc::Operation::PrintThenCut);
        production.fill_color = Some("#ff8800".to_string());
        let sheets =
            stencilcraft_layout::print_sheets(&doc, &stencilcraft_layout::PrintOptions::default())
                .unwrap();
        let pdf = export_print_sheets(&[sheets[0].clone(), sheets[0].clone()]).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let text = String::from_utf8_lossy(&pdf);
        // US Letter at actual size, twice, in the artwork's colour.
        assert_eq!(text.matches("/MediaBox[0 0 612 792]").count(), 2, "{text}");
        assert!(text.contains("1 0.53333336 0 rg"), "{text}");
        let read = vectorcraft_pdf::import(&pdf).unwrap();
        assert!(!read.layers.is_empty());
        let mut bad = sheets[0].clone();
        bad.width_mm = f64::NAN;
        assert!(export_print_sheets(&[bad]).is_err());
    }
}
