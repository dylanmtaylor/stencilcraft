//! PDF export: one page the size of the board, every visible shape as a vector stroke
//! (cut red, engrave blue, 0.25 mm), for printing at 100 % or handing to a cutter's
//! software. PDF serialization and drawing are delegated to the shared craft-pdf writer.

use stencilcraft_doc::{CutStyle, Document};

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("pdf: {0}")]
    Model(String),
}

pub fn export(doc: &Document) -> Result<Vec<u8>, PdfError> {
    doc.validate().map_err(|e| PdfError::Model(e.to_string()))?;
    let mut page = craft_file_import::write::PdfPage {
        width_mm: doc.board.width_mm,
        height_mm: doc.board.height_mm,
        view_box: [0.0, 0.0, doc.board.width_mm, doc.board.height_mm],
        paths: Vec::new(),
    };
    for layer in doc.layers.iter().filter(|layer| layer.visible) {
        for shape in &layer.shapes {
            let color = match shape.style {
                CutStyle::Cut => [192, 57, 43],
                CutStyle::Engrave => [45, 108, 223],
            };
            page.paths.push(craft_file_import::write::PdfPath {
                path: shape.path.clone(),
                color_rgb: color,
                stroke_width_mm: Some(0.25),
            });
        }
    }
    craft_file_import::write::write_pdf_pages(&[page])
        .map_err(|error| PdfError::Model(error.to_string()))
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
        let read = craft_pdf::paths::import(&pdf, Default::default()).unwrap();
        assert!(!read.pages.is_empty());
        assert!(!read.pages[0].paths.is_empty());
        let mut bad = doc.clone();
        bad.board.width_mm = f64::NAN;
        assert!(export(&bad).is_err());
    }
}
