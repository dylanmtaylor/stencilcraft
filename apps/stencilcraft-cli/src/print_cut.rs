//! Printable SVG sheet for a manual print-and-cut alignment workflow.

use stencilcraft_doc::Document;

/// Build a physical-size SVG page with three high-contrast registration marks
/// and the original artwork in a separate nested SVG viewport. This is a print
/// template, not a machine scan/registration command stream.
pub fn export_sheet(doc: &Document) -> Result<String, String> {
    stencilcraft_svg::export_print_and_cut(doc).map_err(|error| error.to_string())
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
