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
    // `$INSUNITS` is defined beginning with R2000; use that format so readers
    // land the drawing in millimetres as promised by the app export.
    drawing.format = craft_dxf::model::write::Format::R2000;
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

    fn parsed_entities(dxf: &str) -> Vec<craft_dxf::model::Entity> {
        let tags = craft_dxf::parse_ascii(dxf).unwrap();
        let drawing = craft_dxf::model::Document::read(&tags);
        drawing
            .entities
            .iter()
            .map(craft_dxf::model::Entity::read)
            .collect()
    }

    fn parsed_layers(dxf: &str) -> Vec<craft_dxf::model::Layer> {
        let tags = craft_dxf::parse_ascii(dxf).unwrap();
        craft_dxf::model::Document::read(&tags).layers()
    }

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
        let tags = craft_dxf::parse_ascii(&dxf).unwrap();
        let drawing = craft_dxf::model::Document::read(&tags);
        assert!(drawing.sections > 0);
        assert_eq!(drawing.header.insunits(), 4);
        assert_eq!(tags.last().and_then(craft_dxf::Tag::str_ref), Some("EOF"));
    }

    #[test]
    fn closed_shape_is_closed_polyline() {
        let mut doc = Document::default();
        doc.push_shape(0, tri(), CutStyle::Cut).unwrap();
        let dxf = export(&doc).unwrap();
        let entity = parsed_entities(&dxf).remove(0);
        assert_eq!(entity.common.layer, "STENCILCRAFT-CUT");
        let craft_dxf::model::EntityKind::LwPolyline(polyline) = entity.kind else {
            panic!("expected LWPOLYLINE")
        };
        assert!(polyline.closed);
        assert_eq!(polyline.vertices.len(), 3);
    }

    #[test]
    fn layers_carry_operation_colours() {
        let mut doc = Document::default();
        doc.layers
            .push(stencilcraft_doc::Layer::new("score", CutStyle::Engrave));
        doc.layers[1].visible = false;
        let dxf = export(&doc).unwrap();
        let layers = parsed_layers(&dxf);
        let cut = layers
            .iter()
            .find(|layer| layer.name == "STENCILCRAFT-CUT")
            .unwrap();
        let score = layers
            .iter()
            .find(|layer| layer.name == "STENCILCRAFT-SCORE")
            .unwrap();
        assert_eq!(cut.color, craft_dxf::model::Color::Index(1));
        assert!(cut.on);
        assert_eq!(score.color, craft_dxf::model::Color::Index(5));
        assert!(!score.on);
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
        let entity = parsed_entities(&dxf).remove(0);
        assert_eq!(entity.common.layer, "STENCILCRAFT-SCORING-PASS");
        let craft_dxf::model::EntityKind::LwPolyline(polyline) = entity.kind else {
            panic!("expected LWPOLYLINE")
        };
        assert!(!polyline.closed);
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
        let entity = parsed_entities(&dxf).remove(0);
        let craft_dxf::model::EntityKind::LwPolyline(polyline) = entity.kind else {
            panic!("expected LWPOLYLINE")
        };
        let coords: Vec<_> = polyline
            .vertices
            .iter()
            .map(|vertex| (vertex.x, vertex.y))
            .collect();
        assert_eq!(coords, [(10.0, 80.0), (40.0, 80.0), (10.0, 30.0)]);
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
