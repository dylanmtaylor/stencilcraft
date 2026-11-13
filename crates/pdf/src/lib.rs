//! PDF export: one page the size of the board, every visible shape as a vector stroke
//! (cut red, engrave blue, 0.25 mm), for printing at 100 % or handing to a cutter's
//! software. PDF serialization and drawing are delegated to the shared craft-pdf writer.

use craft_pdf::write::{Metadata, Writer, backend};
use stencilcraft_doc::{CutStyle, Document};

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("pdf: {0}")]
    Model(String),
}

impl From<craft_pdf::write::WriteError> for PdfError {
    fn from(error: craft_pdf::write::WriteError) -> Self {
        Self::Model(error.to_string())
    }
}

const PT_PER_MM: f64 = 72.0 / 25.4;

/// One path to draw on a [`PdfPage`]. RGB values and width are model units, not PDF syntax.
#[derive(Debug, Clone)]
pub struct PdfPath {
    pub path: kurbo::BezPath,
    pub color: [u8; 3],
    pub stroke_width_mm: Option<f64>,
}

/// A physical page and its vector paths, used by print layout and document export adapters.
#[derive(Debug, Clone)]
pub struct PdfPage {
    pub width_mm: f64,
    pub height_mm: f64,
    pub paths: Vec<PdfPath>,
}

pub fn export(doc: &Document) -> Result<Vec<u8>, PdfError> {
    doc.validate().map_err(|e| PdfError::Model(e.to_string()))?;
    let mut page = PdfPage {
        width_mm: doc.board.width_mm,
        height_mm: doc.board.height_mm,
        paths: Vec::new(),
    };
    for layer in doc.layers.iter().filter(|layer| layer.visible) {
        for shape in &layer.shapes {
            let color = match shape.style {
                CutStyle::Cut => [192, 57, 43],
                CutStyle::Engrave => [45, 108, 223],
            };
            page.paths.push(PdfPath {
                path: shape.path.clone(),
                color,
                stroke_width_mm: Some(0.25),
            });
        }
    }
    export_pages(&[page])
}

/// Serialize one or more application-adapted vector pages through the shared PDF writer.
pub fn export_pages(pages: &[PdfPage]) -> Result<Vec<u8>, PdfError> {
    craft_pdf::write::guard(|| {
        let mut settings = backend::SerializeSettings::default();
        settings.configuration =
            craft_pdf::write::configuration(backend::configure::PdfVersion::Pdf14, None)?;
        let mut writer = Writer::new(settings, Metadata::default());
        for (page_index, source) in pages.iter().enumerate() {
            let width = source.width_mm * PT_PER_MM;
            let height = source.height_mm * PT_PER_MM;
            let page_settings = backend::page::PageSettings::from_wh(width as f32, height as f32)
                .filter(|_| width.is_finite() && height.is_finite())
                .ok_or_else(|| {
                    PdfError::Model(format!(
                        "page {page_index}: dimensions are not finite or positive"
                    ))
                })?;
            let mut page = writer.document_mut().start_page_with(page_settings);
            let mut surface = page.surface();
            for source_path in &source.paths {
                let mut path = backend::geom::PathBuilder::new();
                for element in source_path.path.elements() {
                    let scale = |p: kurbo::Point| -> Result<(f32, f32), PdfError> {
                        if !(p.x.is_finite() && p.y.is_finite()) {
                            return Err(PdfError::Model(format!(
                                "page {page_index}: a path point is not finite"
                            )));
                        }
                        let x = p.x * PT_PER_MM;
                        let y = p.y * PT_PER_MM;
                        if !(x.is_finite()
                            && y.is_finite()
                            && x.abs() <= f64::from(f32::MAX)
                            && y.abs() <= f64::from(f32::MAX))
                        {
                            return Err(PdfError::Model(format!(
                                "page {page_index}: a path point is outside the PDF coordinate range"
                            )));
                        }
                        Ok((x as f32, y as f32))
                    };
                    match *element {
                        kurbo::PathEl::MoveTo(p) => {
                            let (x, y) = scale(p)?;
                            path.move_to(x, y);
                        }
                        kurbo::PathEl::LineTo(p) => {
                            let (x, y) = scale(p)?;
                            path.line_to(x, y);
                        }
                        kurbo::PathEl::QuadTo(a, b) => {
                            let (x1, y1) = scale(a)?;
                            let (x, y) = scale(b)?;
                            path.quad_to(x1, y1, x, y);
                        }
                        kurbo::PathEl::CurveTo(a, b, c) => {
                            let (x1, y1) = scale(a)?;
                            let (x2, y2) = scale(b)?;
                            let (x, y) = scale(c)?;
                            path.cubic_to(x1, y1, x2, y2, x, y);
                        }
                        kurbo::PathEl::ClosePath => path.close(),
                    }
                }
                if let Some(path) = path.finish() {
                    let color = backend::color::rgb::Color::new(
                        source_path.color[0],
                        source_path.color[1],
                        source_path.color[2],
                    );
                    if let Some(width_mm) = source_path.stroke_width_mm {
                        if !(width_mm.is_finite() && width_mm > 0.0) {
                            return Err(PdfError::Model(format!(
                                "page {page_index}: stroke width must be finite and positive"
                            )));
                        }
                        surface.set_fill(None);
                        surface.set_stroke(Some(backend::paint::Stroke {
                            paint: color.into(),
                            width: (width_mm * PT_PER_MM) as f32,
                            ..Default::default()
                        }));
                    } else {
                        surface.set_stroke(None);
                        surface.set_fill(Some(backend::paint::Fill {
                            paint: color.into(),
                            ..Default::default()
                        }));
                    }
                    surface.draw_path(&path);
                }
            }
            surface.finish();
            page.finish();
        }
        writer.finish().map_err(PdfError::from)
    })
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
        let text = String::from_utf8(pdf.clone()).unwrap();
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.ends_with("%%EOF\n"));
        assert!(text.contains("/MediaBox [0 0 283.4646 425.1969]"), "{text}");
        // An independent reader (VectorCraft's, over hayro) finds the path.
        let read = vectorcraft_pdf::import(&pdf).unwrap();
        assert!(!read.layers.is_empty());
        let mut bad = doc.clone();
        bad.board.width_mm = f64::NAN;
        assert!(export(&bad).is_err());
    }
}
