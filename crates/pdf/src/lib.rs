//! PDF export: one page the size of the board, every visible shape as a vector stroke
//! (cut red, engrave blue, 0.25 mm), for printing at 100 % or handing to a cutter's
//! software. A hand-written PDF 1.4 subset (ISO 32000-1 §7: header, four objects, a
//! cross-reference table and trailer). Placed raster images are not written yet.

use std::fmt::Write as _;

use stencilcraft_doc::{CutStyle, Document};

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("pdf: {0}")]
    Model(String),
}

const PT_PER_MM: f64 = 72.0 / 25.4;

pub fn export(doc: &Document) -> Result<Vec<u8>, PdfError> {
    doc.validate().map_err(|e| PdfError::Model(e.to_string()))?;
    let (w, h) = (
        doc.board.width_mm * PT_PER_MM,
        doc.board.height_mm * PT_PER_MM,
    );
    // Millimetres, y down, like the document.
    let mut content = format!(
        "q {PT_PER_MM:.6} 0 0 {:.6} 0 {h:.4} cm 1 J 1 j 0.25 w\n",
        -PT_PER_MM
    );
    for layer in doc.layers.iter().filter(|layer| layer.visible) {
        for shape in &layer.shapes {
            let colour = match shape.style {
                CutStyle::Cut => "0.753 0.224 0.169",
                CutStyle::Engrave => "0.176 0.424 0.875",
            };
            let _ = writeln!(content, "{colour} RG");
            let mut cursor = kurbo::Point::ZERO;
            for element in shape.path.elements() {
                let points = match *element {
                    kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => vec![p],
                    kurbo::PathEl::QuadTo(a, b) => vec![a, b],
                    kurbo::PathEl::CurveTo(a, b, c) => vec![a, b, c],
                    kurbo::PathEl::ClosePath => Vec::new(),
                };
                if points.iter().any(|p| !(p.x.is_finite() && p.y.is_finite())) {
                    return Err(PdfError::Model(format!(
                        "shape {}: a point is not finite",
                        shape.id
                    )));
                }
                match *element {
                    kurbo::PathEl::MoveTo(p) => {
                        let _ = writeln!(content, "{:.4} {:.4} m", p.x, p.y);
                        cursor = p;
                    }
                    kurbo::PathEl::LineTo(p) => {
                        let _ = writeln!(content, "{:.4} {:.4} l", p.x, p.y);
                        cursor = p;
                    }
                    kurbo::PathEl::QuadTo(a, b) => {
                        let cubic = kurbo::QuadBez::new(cursor, a, b).raise();
                        let (c1, c2) = (cubic.p1, cubic.p2);
                        let _ = writeln!(
                            content,
                            "{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} c",
                            c1.x, c1.y, c2.x, c2.y, b.x, b.y
                        );
                        cursor = b;
                    }
                    kurbo::PathEl::CurveTo(a, b, c) => {
                        let _ = writeln!(
                            content,
                            "{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} c",
                            a.x, a.y, b.x, b.y, c.x, c.y
                        );
                        cursor = c;
                    }
                    kurbo::PathEl::ClosePath => content.push_str("h\n"),
                }
            }
            content.push_str("S\n");
        }
    }
    content.push_str("Q\n");

    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.4} {h:.4}] /Contents 4 0 R /Resources << >> >>"
        ),
        format!(
            "<< /Length {} >>\nstream\n{content}endstream",
            content.len()
        ),
    ];
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        let _ = write!(out, "{} 0 obj\n{object}\nendobj\n", i + 1);
    }
    let xref = out.len();
    let _ = write!(out, "xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for offset in offsets {
        let _ = writeln!(out, "{offset:010} 00000 n ");
    }
    let _ = write!(
        out,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    );
    Ok(out.into_bytes())
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
