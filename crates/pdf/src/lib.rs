//! PDF export: one page the size of the board, every visible shape as a vector stroke
//! (cut red, engrave blue, 0.25 mm), for printing at 100 % or handing to a cutter's
//! software. PDF serialization and drawing are delegated to the shared craft-pdf writer.

use stencilcraft_doc::{Document, Operation};

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
        let color = match layer.operation {
            Some(Operation::Engrave(_)) => [45, 108, 223],
            _ => [192, 57, 43],
        };
        for id in &layer.members {
            let Some(shape) = doc.get_shape(*id) else {
                continue;
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
        doc.push_shape(0, path).unwrap();
        let pdf = export(&doc).unwrap();
        use kurbo::Shape as _;
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.trim_ascii_end().ends_with(b"%%EOF"));
        let read = craft_pdf::paths::import(&pdf, Default::default()).unwrap();
        assert_eq!(read.pages.len(), 1);
        let page = &read.pages[0];
        let pt_per_mm = 72.0 / 25.4;
        assert!((page.crop_box.width() - doc.board.width_mm * pt_per_mm).abs() < 0.02);
        assert!((page.crop_box.height() - doc.board.height_mm * pt_per_mm).abs() < 0.02);
        let expected = (kurbo::Affine::scale(pt_per_mm)
            * doc
                .get_shape(doc.layers[0].members[0])
                .unwrap()
                .path
                .clone())
        .bounding_box();
        assert!(
            page.paths.iter().any(|path| {
                let actual = path.bounding_box();
                [
                    actual.x0 - expected.x0,
                    actual.y0 - expected.y0,
                    actual.x1 - expected.x1,
                    actual.y1 - expected.y1,
                ]
                .iter()
                .all(|d| d.abs() < 0.02)
            }),
            "exported path geometry must survive PDF import"
        );
        let mut bad = doc.clone();
        bad.board.width_mm = f64::NAN;
        assert!(export(&bad).is_err());
    }
}
