//! DXF export (R14 ASCII subset, `LWPOLYLINE` entities).
//!
//! Coordinates are model-space millimetres; `$INSUNITS` = 4 (millimetres) so
//! readers that respect AutoCAD units land at true size either way. Curves are
//! flattened — the DXF subset here has no arc/spline entities (`docs/gaps.md` 5).
//! Each layer carries a colour index so CAM tools that key operations off layer
//! colour (cut vs score) read the file the way it was meant: cut red, engrave blue.
//! A hidden layer is written with a negative colour index, the DXF way of saying
//! "layer off".
//! Group codes one per line, values one per line, exactly as the DXF reference's
//! ASCII layout.

use std::fmt::Write as _;

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
    let mut out = String::with_capacity(4096);
    let pair = |out: &mut String, code: i32, value: &str| -> Result<(), DxfError> {
        // The DXF ASCII grammar is one group code, then one value.
        let _ = writeln!(out, "{code}");
        let _ = writeln!(out, "{value}");
        Ok(())
    };

    // Header: units in millimetres, angle direction.
    pair(&mut out, 0, "SECTION")?;
    pair(&mut out, 2, "HEADER")?;
    pair(&mut out, 9, "$INSUNITS")?;
    pair(&mut out, 70, "4")?;
    pair(&mut out, 9, "$LUNITS")?;
    pair(&mut out, 70, "2")?;
    pair(&mut out, 0, "ENDSEC")?;

    // Tables: an (auto-named) LAYER table entry per document layer.
    pair(&mut out, 0, "SECTION")?;
    pair(&mut out, 2, "TABLES")?;
    pair(&mut out, 0, "TABLE")?;
    pair(&mut out, 2, "LAYER")?;
    for layer in &doc.layers {
        pair(&mut out, 0, "LAYER")?;
        pair(&mut out, 2, &layer_name(&layer.name))?;
        pair(&mut out, 70, "0")?;
        // Colour index: 1 red for cut, 5 blue for engrave. Negative means the layer
        // is off, which is how a hidden document layer should arrive.
        let aci = layer_color(layer.kind);
        let value = if layer.visible {
            aci.to_string()
        } else {
            format!("-{aci}")
        };
        pair(&mut out, 62, &value)?;
        pair(&mut out, 6, "CONTINUOUS")?;
    }
    pair(&mut out, 0, "ENDTAB")?;
    pair(&mut out, 0, "ENDSEC")?;

    // Entities.
    pair(&mut out, 0, "SECTION")?;
    pair(&mut out, 2, "ENTITIES")?;
    for layer in &doc.layers {
        let dxf_layer = layer_name(&layer.name);
        for shape in &layer.shapes {
            let closed = match (shape.style, layer.kind) {
                (CutStyle::Cut, _) | (_, CutStyle::Cut) => true,
                (CutStyle::Engrave, CutStyle::Engrave) => false,
            };
            push_polyline(&mut out, &shape.path, &dxf_layer, closed)?;
        }
    }
    pair(&mut out, 0, "ENDSEC")?;
    pair(&mut out, 0, "EOF")?;
    Ok(out)
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
    out: &mut String,
    path: &kurbo::BezPath,
    layer: &str,
    closed: bool,
) -> Result<(), DxfError> {
    let lines = path_to_polylines(path, stencilcraft_geom::DEFAULT_FLATTEN_STEPS);
    for pl in &lines {
        if pl.points.len() < 2 {
            continue;
        }
        let w = |out: &mut String, code: i32, value: String| -> Result<(), DxfError> {
            let _ = writeln!(out, "{code}");
            let _ = writeln!(out, "{value}");
            Ok(())
        };
        w(out, 0, "LWPOLYLINE".to_string())?;
        w(out, 8, layer.to_string())?;
        w(out, 100, "AcDbEntity".to_string())?;
        w(out, 100, "AcDbPolyline".to_string())?;
        w(out, 90, pl.points.len().to_string())?;
        w(
            out,
            70,
            if pl.closed && closed { "1" } else { "0" }.to_string(),
        )?;
        // LWPOLYLINE vertices: one X group, then one Y group.
        for p in &pl.points {
            if !p.x.is_finite() || !p.y.is_finite() {
                return Err(DxfError::Write("dxf: a vertex is not finite".to_string()));
            }
            w(out, 10, format!("{:.6}", p.x))?;
            w(out, 20, format!("{:.6}", p.y))?;
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

    #[test]
    fn has_header_entities_and_eof() {
        let doc = Document::default();
        let dxf = export(&doc).unwrap();
        assert!(dxf.contains("SECTION"));
        assert!(dxf.contains("$INSUNITS"));
        assert!(dxf.contains("ENDSEC"));
        assert!(dxf.starts_with("0\nSECTION"));
        assert!(dxf.ends_with("0\nEOF\n"));
    }

    #[test]
    fn closed_shape_is_closed_polyline() {
        let mut doc = Document::default();
        doc.push_shape(0, tri(), CutStyle::Cut).unwrap();
        let dxf = export(&doc).unwrap();
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
        let dxf = export(&doc).unwrap();
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
        let dxf = export(&doc).unwrap();
        assert!(dxf.contains("STENCILCRAFT-SCORING-PASS"), "{dxf}");
        assert!(dxf.contains("\n70\n0\n"), "open flag for engrave");
    }

    #[test]
    fn inf_board_is_rejected() {
        let mut doc = Document::default();
        doc.board.width_mm = f64::INFINITY;
        assert!(matches!(export(&doc), Err(DxfError::Model(_))));
    }
}
