//! Printable SVG sheet for a manual print-and-cut alignment workflow.

use std::fmt::Write as _;

use stencilcraft_doc::Document;

const PAGE_MARGIN_MM: f64 = 12.0;
const MARK_SIZE_MM: f64 = 6.0;

/// Build a physical-size SVG page with three high-contrast registration marks
/// and the original artwork in a separate nested SVG viewport. This is a print
/// template, not a machine scan/registration command stream.
pub fn export_sheet(doc: &Document) -> Result<String, String> {
    doc.validate().map_err(|error| error.to_string())?;
    let artwork = stencilcraft_svg::export(doc).map_err(|error| error.to_string())?;
    let page_width = doc.board.width_mm + 2.0 * PAGE_MARGIN_MM;
    let page_height = doc.board.height_mm + 2.0 * PAGE_MARGIN_MM;
    if !page_width.is_finite() || !page_height.is_finite() {
        return Err("print-cut: page dimensions are not finite".to_string());
    }

    let mut svg = String::with_capacity(artwork.len() + 1024);
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{page_width:.3}mm\" height=\"{page_height:.3}mm\" viewBox=\"0 0 {page_width:.3} {page_height:.3}\">"
    );
    svg.push_str("  <title>StencilCraft print-and-cut alignment sheet</title>\n");
    let _ = writeln!(
        svg,
        "  <g id=\"registration-marks\" fill=\"#000000\" aria-label=\"Registration marks: top-left square, top-right and bottom-left corners\">"
    );
    // A square establishes the origin. Asymmetric corner marks preserve page
    // orientation and provide two additional, widely spaced alignment points.
    let _ = writeln!(
        svg,
        "    <rect x=\"0\" y=\"0\" width=\"{MARK_SIZE_MM:.3}\" height=\"{MARK_SIZE_MM:.3}\"/>"
    );
    let _ = writeln!(
        svg,
        "    <path d=\"M {:.3} 0 H {:.3} V {:.3} H {:.3} V {:.3} H {:.3} Z\"/>",
        page_width,
        page_width - MARK_SIZE_MM,
        MARK_SIZE_MM,
        page_width - 1.5,
        1.5,
        page_width - MARK_SIZE_MM,
    );
    let _ = writeln!(
        svg,
        "    <path d=\"M 0 {:.3} H {:.3} V {:.3} H {:.3} V {:.3} H 0 Z\"/>",
        page_height,
        MARK_SIZE_MM,
        page_height - MARK_SIZE_MM,
        1.5,
        page_height - 1.5,
    );
    svg.push_str("  </g>\n");
    let _ = writeln!(
        svg,
        "  <svg x=\"{PAGE_MARGIN_MM:.3}\" y=\"{PAGE_MARGIN_MM:.3}\" width=\"{:.3}mm\" height=\"{:.3}mm\" viewBox=\"0 0 {:.3} {:.3}\" overflow=\"hidden\">",
        doc.board.width_mm, doc.board.height_mm, doc.board.width_mm, doc.board.height_mm,
    );
    // `stencilcraft-svg` output is generated locally and has no user text in
    // attributes other than escaped layer names already fixed to safe values.
    if let Some(root_start) = artwork.find('>')
        && let Some(root_end) = artwork.rfind("</svg>")
        && root_start < root_end
    {
        svg.push_str(&artwork[root_start + 1..root_end]);
    } else {
        return Err("print-cut: could not embed SVG artwork".to_string());
    }
    svg.push_str("  </svg>\n</svg>\n");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_testkit::starter;

    #[test]
    fn sheet_has_physical_page_size_registration_marks_and_design_viewport() {
        let doc = starter();
        let svg = export_sheet(&doc).expect("sheet export");
        assert!(svg.contains("width=\"124.000mm\""));
        assert!(svg.contains("height=\"174.000mm\""));
        assert!(svg.contains("id=\"registration-marks\""));
        assert!(svg.matches("<rect").count() >= 1);
        assert!(svg.contains("<path"));
        assert!(svg.contains("class=\"stencilcraft-cut\""));
        assert!(svg.contains("<svg x=\"12.000\" y=\"12.000\""));
        assert!(svg.ends_with("</svg>\n</svg>\n"));
    }
}
