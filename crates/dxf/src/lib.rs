//! StencilCraft layer and operation mapping to the shared typed DXF writer.

use stencilcraft_doc::{CutStyle, Document};
use stencilcraft_geom::path_to_polylines;

#[derive(Debug, thiserror::Error)]
pub enum DxfError {
    #[error("dxf: {0}")]
    Write(String),
    #[error("dxf: {0}")]
    Model(String),
}

fn layer_color(kind: CutStyle) -> i32 {
    match kind {
        CutStyle::Cut => 1,
        CutStyle::Engrave => 5,
    }
}
fn layer_name(name: &str) -> String {
    let cleaned: String = name
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

/// Map the app model to typed DXF entities and let craft-dxf write the file grammar.
pub fn export(doc: &Document) -> Result<String, DxfError> {
    if !doc.board.width_mm.is_finite() || !doc.board.height_mm.is_finite() {
        return Err(DxfError::Model(
            "dxf: board dimensions must be finite".into(),
        ));
    }
    let mut drawing = craft_dxf::model::write::Drawing::default();
    drawing.format = craft_dxf::model::write::Format::R14;
    drawing.insunits = 4;
    drawing.metric = true;
    drawing.extents = [0.0, 0.0, doc.board.width_mm, doc.board.height_mm];
    drawing.limits = drawing.extents;
    for layer in doc.layers.iter().filter(|layer| !layer.template) {
        drawing.layers.push(craft_dxf::model::Layer {
            handle: None,
            name: layer_name(&layer.name),
            color: craft_dxf::model::Color::Index(layer_color(layer.kind) as u8),
            on: layer.visible,
            frozen: false,
            vp_freeze_new: false,
            locked: false,
            plot: true,
            linetype: "Continuous".into(),
            lineweight: craft_dxf::model::Lineweight::Default,
            plot_style: None,
            transparency: craft_dxf::model::Transparency::ByLayer,
            description: None,
        });
    }
    for layer in doc.layers.iter().filter(|layer| !layer.template) {
        let dxf_layer = layer_name(&layer.name);
        for shape in layer.shapes.iter().filter(|shape| !shape.hidden) {
            let close_for_operation = matches!(
                (shape.style, layer.kind),
                (CutStyle::Cut, _) | (_, CutStyle::Cut)
            );
            for polyline in
                path_to_polylines(&shape.cut_path(), stencilcraft_geom::DEFAULT_FLATTEN_STEPS)
            {
                if polyline.points.len() < 2 {
                    continue;
                }
                let mut vertices = Vec::with_capacity(polyline.points.len());
                for point in &polyline.points {
                    if !point.x.is_finite() || !point.y.is_finite() {
                        return Err(DxfError::Write("dxf: a vertex is not finite".into()));
                    }
                    let y =
                        stencilcraft_geom::artboard_y_to_cartesian(point.y, doc.board.height_mm)
                            .map_err(|e| DxfError::Write(e.to_string()))?;
                    vertices.push(craft_dxf::model::LwVertex {
                        x: point.x,
                        y,
                        ..Default::default()
                    });
                }
                drawing.entities.push(craft_dxf::model::Entity {
                    common: craft_dxf::model::Common {
                        handle: None,
                        owner: None,
                        layer: dxf_layer.clone(),
                        linetype: None,
                        color: craft_dxf::model::Color::ByLayer,
                        lineweight: craft_dxf::model::Lineweight::ByLayer,
                        ltscale: 1.0,
                        visible: true,
                        paper_space: false,
                        transparency: craft_dxf::model::Transparency::ByLayer,
                        thickness: 0.0,
                        extrusion: [0.0, 0.0, 1.0],
                        plot_style: None,
                    },
                    kind: craft_dxf::model::EntityKind::LwPolyline(craft_dxf::model::LwPolyline {
                        vertices,
                        closed: polyline.closed && close_for_operation,
                        plinegen: false,
                        const_width: 0.0,
                        elevation: 0.0,
                    }),
                });
            }
        }
    }
    let (tags, skipped) = drawing.tags();
    if skipped > 0 {
        return Err(DxfError::Write(format!(
            "dxf: shared writer skipped {skipped} entities"
        )));
    }
    Ok(craft_dxf::write_ascii(&tags))
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
    fn export_flips_y_about_the_board_bottom_for_asymmetric_art() {
        let mut doc = Document::default();
        doc.board.height_mm = 100.0;
        let mut path = kurbo::BezPath::new();
        path.move_to((10.0, 20.0));
        path.line_to((40.0, 20.0));
        path.line_to((10.0, 70.0));
        path.close_path();
        doc.push_shape(0, path, CutStyle::Cut).unwrap();
        let dxf = export(&doc).unwrap();
        assert!(dxf.contains("10\n10.000000\n20\n80.000000\n10\n40.000000\n20\n80.000000\n10\n10.000000\n20\n30.000000\n"), "{dxf}");
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
}
