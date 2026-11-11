//! DXF export (R14 ASCII subset, `LWPOLYLINE` entities).
//!
//! Coordinates are model-space millimetres; `$INSUNITS` = 4 (millimetres) so
//! readers that respect AutoCAD units land at true size either way. Curves are
//! flattened — the DXF subset here has no arc/spline entities (`docs/gaps.md` 5).
//! Each layer carries a colour index so CAM tools that key operations off layer
//! colour (cut vs score) read the file the way it was meant: cut red, engrave blue.
//! The canvas has a downward-growing Y axis; DXF model space uses an upward-growing
//! Y axis. At export, the board top maps to its height and its bottom maps to zero.
//! Autodesk describes LWPOLYLINE vertex coordinates and its default extrusion at
//! <https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-748FC305-F3F2-4F74-825A-61F04D757A50.htm>.
//! A hidden layer is written with a negative colour index, the DXF way of saying
//! "layer off".
//! The tags are written by `craft-dxf` in the DXF reference's ASCII layout: group
//! codes right-aligned to three columns, one value per line, CRLF line ends.

use craft_dxf::Tag;
use stencilcraft_doc::{CutStyle, Document};
use stencilcraft_geom::path_to_polylines;

#[derive(Debug, thiserror::Error)]
pub enum DxfError {
    #[error("dxf: {0}")]
    Write(String),
    #[error("dxf: {0}")]
    Model(String),
}

/// Serialise a document as DXF. Every document layer becomes one DXF layer named
/// `stencilcraft-<layer name>`; cut shapes are closed polylines, engrave shapes
/// open ones on their own layer.
pub fn export(doc: &Document) -> Result<String, DxfError> {
    if !doc.board.width_mm.is_finite() || !doc.board.height_mm.is_finite() {
        return Err(DxfError::Model(
            "dxf: board dimensions must be finite".to_string(),
        ));
    }
    // Header: units in millimetres, angle direction.
    let mut tags = vec![
        Tag::s(0, "SECTION"),
        Tag::s(2, "HEADER"),
        Tag::s(9, "$INSUNITS"),
        Tag::i(70, 4),
        Tag::s(9, "$LUNITS"),
        Tag::i(70, 2),
        Tag::s(0, "ENDSEC"),
        // Tables: an (auto-named) LAYER table entry per document layer.
        Tag::s(0, "SECTION"),
        Tag::s(2, "TABLES"),
        Tag::s(0, "TABLE"),
        Tag::s(2, "LAYER"),
    ];
    // Template (guide) layers are canvas-only: never exported.
    for layer in doc.layers.iter().filter(|layer| !layer.template) {
        // Colour index: 1 red for cut, 5 blue for engrave. Negative means the layer
        // is off, which is how a hidden document layer should arrive.
        let aci = i64::from(layer_color(layer.kind));
        tags.extend([
            Tag::s(0, "LAYER"),
            Tag::s(2, layer_name(&layer.name)),
            Tag::i(70, 0),
            Tag::i(62, if layer.visible { aci } else { -aci }),
            Tag::s(6, "CONTINUOUS"),
        ]);
    }
    tags.extend([Tag::s(0, "ENDTAB"), Tag::s(0, "ENDSEC")]);

    // Entities.
    tags.extend([Tag::s(0, "SECTION"), Tag::s(2, "ENTITIES")]);
    for layer in doc.layers.iter().filter(|layer| !layer.template) {
        let dxf_layer = layer_name(&layer.name);
        for shape in layer.shapes.iter().filter(|shape| !shape.hidden) {
            let closed = match (shape.style, layer.kind) {
                (CutStyle::Cut, _) | (_, CutStyle::Cut) => true,
                (CutStyle::Engrave, CutStyle::Engrave) => false,
            };
            push_polyline(
                &mut tags,
                &shape.cut_path(),
                &dxf_layer,
                closed,
                doc.board.height_mm,
            )?;
        }
    }
    tags.extend([Tag::s(0, "ENDSEC"), Tag::s(0, "EOF")]);
    Ok(craft_dxf::write_ascii(&tags))
}

/// AutoCAD colour index for an operation: red cuts, blue scores. Distinct colours
/// let a CAM tool sort operations without a convention document.
fn layer_color(kind: CutStyle) -> i32 {
    match kind {
        CutStyle::Cut => 1,
        CutStyle::Engrave => 5,
    }
}

/// DXF layer names: uppercase, ASCII, no spaces per common practice, prefix-stamped
/// so a reader always shows ours distinctly.
fn layer_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("STENCILCRAFT-{}", cleaned.to_uppercase())
}

fn push_polyline(
    tags: &mut Vec<Tag>,
    path: &kurbo::BezPath,
    layer: &str,
    closed: bool,
    board_height_mm: f64,
) -> Result<(), DxfError> {
    // Coordinates are written to a micrometre, as before.
    let rounded = |mm: f64| (mm * 1e6).round() / 1e6;
    let lines = path_to_polylines(path, stencilcraft_geom::DEFAULT_FLATTEN_STEPS);
    for pl in &lines {
        if pl.points.len() < 2 {
            continue;
        }
        tags.extend([
            Tag::s(0, "LWPOLYLINE"),
            Tag::s(8, layer),
            Tag::s(100, "AcDbEntity"),
            Tag::s(100, "AcDbPolyline"),
            Tag::i(90, i64::try_from(pl.points.len()).unwrap_or(i64::MAX)),
            Tag::i(70, i64::from(pl.closed && closed)),
        ]);
        // LWPOLYLINE vertices: one X group, then one Y group.
        for p in &pl.points {
            if !p.x.is_finite() || !p.y.is_finite() {
                return Err(DxfError::Write("dxf: a vertex is not finite".to_string()));
            }
            let y = stencilcraft_geom::artboard_y_to_cartesian(p.y, board_height_mm)
                .map_err(|error| DxfError::Write(error.to_string()))?;
            tags.extend([Tag::f(10, rounded(p.x)), Tag::f(20, rounded(y))]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_geom::Point;

    fn tri() -> kurbo::BezPath {
        let mut p = kurbo::BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(40.0, 0.0));
        p.line_to(Point::new(20.0, 30.0));
        p.close_path();
        p
    }

    /// The export as one tag per two lines, without the writer's padding and CRLF.
    fn plain(doc: &Document) -> String {
        export(doc)
            .unwrap()
            .lines()
            .map(|line| format!("{}\n", line.trim_start()))
            .collect()
    }

    #[test]
    fn has_header_entities_and_eof() {
        let doc = Document::default();
        let dxf = plain(&doc);
        assert!(dxf.contains("SECTION"));
        assert!(dxf.contains("$INSUNITS"));
        assert!(dxf.contains("ENDSEC"));
        assert!(dxf.starts_with("0\nSECTION"));
        assert!(dxf.ends_with("0\nEOF\n"));
        // On disk: codes right-aligned to three columns, CRLF line ends.
        let raw = export(&doc).unwrap();
        assert!(
            raw.starts_with("  0\r\nSECTION\r\n  2\r\nHEADER\r\n"),
            "{raw}"
        );
        assert!(raw.ends_with("  0\r\nEOF\r\n"), "{raw}");
    }

    #[test]
    fn closed_shape_is_closed_polyline() {
        let mut doc = Document::default();
        doc.push_shape(0, tri(), CutStyle::Cut).unwrap();
        let dxf = plain(&doc);
        assert!(dxf.contains("LWPOLYLINE"));
        assert!(dxf.contains("\n70\n1\n"), "closed flag: {dxf}");
        assert!(dxf.contains("STENCILCRAFT-CUT"));
    }

    #[test]
    fn layers_carry_operation_colours() {
        let mut doc = Document::default();
        doc.layers
            .push(stencilcraft_doc::Layer::new("score", CutStyle::Engrave));
        doc.layers[1].visible = false;
        let dxf = plain(&doc);
        // Layer table: red cut layer, then the hidden engrave layer written negative.
        assert!(dxf.contains("STENCILCRAFT-CUT\n70\n0\n62\n1\n"), "{dxf}");
        assert!(dxf.contains("STENCILCRAFT-SCORE\n70\n0\n62\n-5\n"), "{dxf}");
        assert_eq!(layer_color(CutStyle::Cut), 1);
        assert_eq!(layer_color(CutStyle::Engrave), 5);
    }

    #[test]
    fn engrave_layer_names_survive_sanitising() {
        let mut doc = Document::default();
        doc.layers.push(stencilcraft_doc::Layer::new(
            "scoring pass",
            CutStyle::Engrave,
        ));
        doc.push_shape(1, tri(), CutStyle::Engrave).unwrap();
        let dxf = plain(&doc);
        assert!(dxf.contains("STENCILCRAFT-SCORING-PASS"), "{dxf}");
        assert!(dxf.contains("\n70\n0\n"), "open flag for engrave");
    }

    #[test]
    fn export_flips_y_about_the_board_bottom_for_asymmetric_art() {
        let mut doc = Document::default();
        doc.board.height_mm = 100.0;
        let mut path = kurbo::BezPath::new();
        path.move_to((10.0, 20.0));
        path.line_to((40.0, 20.0));
        path.line_to((10.0, 70.0));
        path.close_path();
        doc.push_shape(0, path, CutStyle::Cut).unwrap();
        let dxf = plain(&doc);
        assert!(
            dxf.contains("10\n10.0\n20\n80.0\n10\n40.0\n20\n80.0\n10\n10.0\n20\n30.0\n"),
            "{dxf}"
        );
    }

    #[test]
    fn rejects_coordinate_overflow_when_reflecting_y() {
        let mut doc = Document::default();
        doc.board.height_mm = f64::MAX;
        let mut path = kurbo::BezPath::new();
        path.move_to((0.0, -f64::MAX));
        path.line_to((1.0, 0.0));
        doc.push_shape(0, path, CutStyle::Cut).unwrap();
        assert!(matches!(export(&doc), Err(DxfError::Write(_))));
    }

    #[test]
    fn inf_board_is_rejected() {
        let mut doc = Document::default();
        doc.board.width_mm = f64::INFINITY;
        assert!(matches!(export(&doc), Err(DxfError::Model(_))));
    }
    #[test]
    fn coordinates_are_written_to_a_micrometre() {
        let mut doc = Document::default();
        let mut p = kurbo::BezPath::new();
        p.move_to(Point::new(1.0 / 3.0, 0.0));
        p.line_to(Point::new(2.000_000_4, 0.0));
        doc.push_shape(0, p, CutStyle::Cut).unwrap();
        let dxf = plain(&doc);
        assert!(dxf.contains("10\n0.333333\n"), "{dxf}");
        assert!(dxf.contains("10\n2.0\n"), "{dxf}");
    }
}
