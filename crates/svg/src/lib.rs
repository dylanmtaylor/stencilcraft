//! StencilCraft SVG import and export policy on shared Craft-Libs SVG parsing and writing.

use std::collections::HashMap;

use kurbo::BezPath;
use stencilcraft_doc::{CutStyle, Document, Layer};
use stencilcraft_geom::{Polyline, path_to_polylines};

use craft_svg::write::{Attribute as SvgAttribute, Element as SvgElement, Scene as SvgScene};

#[derive(Debug, thiserror::Error)]
pub enum SvgError {
    #[error("svg export: failure to build markup: {0}")]
    Write(String),
    #[error("svg import: {0}")]
    Import(String),
    #[error("svg: {0}")]
    Model(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TilePreviewOptions {
    pub columns: usize,
    pub scale: f64,
}
impl Default for TilePreviewOptions {
    fn default() -> Self {
        Self {
            columns: 3,
            scale: 0.25,
        }
    }
}
pub const TILE_PREVIEW_GAP_MM: f64 = 6.0;
pub const TILE_PREVIEW_MAX_MM: f64 = 100_000.0;

fn class_for(style: CutStyle) -> &'static str {
    match style {
        CutStyle::Cut => "stencilcraft-cut",
        CutStyle::Engrave => "stencilcraft-engrave",
    }
}
fn view_bounds(doc: &Document) -> kurbo::Rect {
    doc.layers
        .iter()
        .filter(|layer| !layer.template)
        .filter_map(Layer::bounds)
        .fold(
            kurbo::Rect::new(0.0, 0.0, doc.board.width_mm, doc.board.height_mm),
            |bounds, layer| bounds.union(layer),
        )
}
fn document_elements(
    doc: &Document,
    output_layers_only: bool,
    include_title: bool,
) -> Vec<SvgElement> {
    let mut elements = Vec::new();
    if include_title {
        elements.push(SvgElement::Title(format!(
            "stencilcraft {}: {} layers, {} shapes, kerf {:.3} mm",
            stencilcraft_doc::FORMAT_NAME,
            doc.layers.len(),
            doc.shape_count(),
            doc.kerf.width_mm
        )));
    }
    let parents = doc.group_parent_map();
    for layer in doc.layers.iter().filter(|layer| {
        if output_layers_only {
            layer.is_output()
        } else {
            !layer.template
        }
    }) {
        let mut children = Vec::new();
        for shape in layer.shapes.iter().filter(|shape| !shape.hidden) {
            let chain = stencilcraft_doc::canvas::group_chain_in(
                &parents,
                stencilcraft_doc::GroupChild::Shape(shape.id),
            );
            let mut element = SvgElement::Path {
                attributes: vec![SvgAttribute::text("class", class_for(shape.style))],
                path: shape.cut_path().into_owned(),
            };
            for id in chain.iter().rev() {
                element = SvgElement::Group {
                    attributes: vec![
                        SvgAttribute::text("class", "group"),
                        SvgAttribute::text("data-group", id.to_string()),
                    ],
                    children: vec![element],
                };
            }
            children.push(element);
        }
        elements.push(SvgElement::Group {
            attributes: vec![
                SvgAttribute::text("class", class_for(layer.kind)),
                SvgAttribute::text(
                    "data-kind",
                    match layer.kind {
                        CutStyle::Cut => "cut",
                        CutStyle::Engrave => "engrave",
                    },
                ),
                SvgAttribute::text(
                    "style",
                    if layer.visible {
                        "display:visible"
                    } else {
                        "display:hidden"
                    },
                ),
            ],
            children,
        });
    }
    elements
}

pub fn export(doc: &Document) -> Result<String, SvgError> {
    let bounds = view_bounds(doc);
    Ok(SvgScene {
        width_mm: bounds.width(),
        height_mm: bounds.height(),
        view_box: [bounds.x0, bounds.y0, bounds.width(), bounds.height()],
        elements: document_elements(doc, false, true),
    }
    .write())
}

pub fn export_tile_preview(
    pieces: &[Document],
    options: &TilePreviewOptions,
) -> Result<String, SvgError> {
    if pieces.is_empty() {
        return Err(SvgError::Model(
            "preview: the tiling produced no sheets to show".into(),
        ));
    }
    if options.columns == 0 {
        return Err(SvgError::Model(
            "preview: columns must be at least 1".into(),
        ));
    }
    if !options.scale.is_finite() || options.scale <= 0.0 {
        return Err(SvgError::Model(format!(
            "preview: scale {} is not a positive number",
            options.scale
        )));
    }
    let columns = options.columns.min(pieces.len());
    let rows = pieces.len().div_ceil(columns);
    let mut cell_w: f64 = 0.0;
    let mut cell_h: f64 = 0.0;
    for piece in pieces {
        let (w, h) = (piece.board.width_mm, piece.board.height_mm);
        if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
            return Err(SvgError::Model(format!(
                "preview: sheet {w}×{h} mm is not a usable size"
            )));
        }
        cell_w = cell_w.max(w * options.scale);
        cell_h = cell_h.max(h * options.scale);
    }
    let gap = TILE_PREVIEW_GAP_MM;
    let page_w = columns as f64 * cell_w + (columns as f64 + 1.0) * gap;
    let page_h = rows as f64 * cell_h + (rows as f64 + 1.0) * gap;
    if !(page_w.is_finite() && page_h.is_finite())
        || page_w > TILE_PREVIEW_MAX_MM
        || page_h > TILE_PREVIEW_MAX_MM
    {
        return Err(SvgError::Model(format!(
            "preview: a {page_w:.0}×{page_h:.0} mm page is too big; use a smaller scale or fewer sheets"
        )));
    }
    let mut elements = vec![
        SvgElement::Title(format!(
            "StencilCraft tiling preview: {} sheets, {} × {} cells",
            pieces.len(),
            columns,
            rows
        )),
        SvgElement::Style(
            concat!(
                ".preview-frame { fill: none; stroke: #9aa0a6; stroke-width: 0.25; }\n",
                ".stencilcraft-cut { fill: none; stroke: #c0392b; stroke-width: 0.3; }\n",
                ".stencilcraft-engrave { fill: none; stroke: #2d6cdf; stroke-width: 0.2; }\n",
                ".preview-label { fill: #5f6368; font-family: sans-serif; font-size: 3.2px; }"
            )
            .into(),
        ),
    ];
    for (index, piece) in pieces.iter().enumerate() {
        let col = index % columns;
        let row = index / columns;
        let x = gap + col as f64 * (cell_w + gap);
        let y = gap + (rows - 1 - row) as f64 * (cell_h + gap);
        let cell_height = piece.board.height_mm * options.scale;
        let children = vec![
            SvgElement::Text {
                attributes: vec![SvgAttribute::text("class", "preview-label")],
                x,
                y: y - 1.6,
                text: format!(
                    "sheet {} of {} · {:.0}×{:.0} mm",
                    index + 1,
                    pieces.len(),
                    piece.board.width_mm,
                    piece.board.height_mm
                ),
            },
            SvgElement::Rect {
                attributes: vec![SvgAttribute::text("class", "preview-frame")],
                x,
                y,
                width: piece.board.width_mm * options.scale,
                height: cell_height,
            },
            SvgElement::Group {
                attributes: vec![SvgAttribute {
                    name: "transform".into(),
                    value: craft_svg::write::Value::TranslateScale {
                        x,
                        y: y + cell_height,
                        scale_x: options.scale,
                        scale_y: -options.scale,
                    },
                }],
                children: document_elements(piece, true, false),
            },
        ];
        elements.push(SvgElement::Group {
            attributes: vec![
                SvgAttribute::text("class", "sheet"),
                SvgAttribute::number("data-sheet", index as f64),
                SvgAttribute::text(
                    "data-size",
                    format!("{:.1}x{:.1}", piece.board.width_mm, piece.board.height_mm),
                ),
            ],
            children,
        });
    }
    Ok(SvgScene {
        width_mm: page_w,
        height_mm: page_h,
        view_box: [0.0, 0.0, page_w, page_h],
        elements,
    }
    .write())
}

pub fn export_print_and_cut(doc: &Document) -> Result<String, SvgError> {
    const MARGIN: f64 = 12.0;
    const MARK: f64 = 6.0;
    doc.validate().map_err(|e| SvgError::Model(e.to_string()))?;
    let page_w = doc.board.width_mm + 2.0 * MARGIN;
    let page_h = doc.board.height_mm + 2.0 * MARGIN;
    if !page_w.is_finite() || !page_h.is_finite() {
        return Err(SvgError::Model(
            "print-cut: page dimensions are not finite".into(),
        ));
    }
    let mut marks = vec![SvgElement::Rect {
        attributes: Vec::new(),
        x: 0.0,
        y: 0.0,
        width: MARK,
        height: MARK,
    }];
    let mut top = BezPath::new();
    top.move_to((page_w, 0.0));
    top.line_to((page_w - MARK, 0.0));
    top.line_to((page_w - MARK, MARK));
    top.line_to((page_w - 1.5, MARK));
    top.line_to((page_w - 1.5, 1.5));
    top.line_to((page_w - MARK, 1.5));
    top.close_path();
    marks.push(SvgElement::Path {
        attributes: Vec::new(),
        path: top,
    });
    let mut bottom = BezPath::new();
    bottom.move_to((0.0, page_h));
    bottom.line_to((MARK, page_h));
    bottom.line_to((MARK, page_h - MARK));
    bottom.line_to((1.5, page_h - MARK));
    bottom.line_to((1.5, page_h - 1.5));
    bottom.line_to((0.0, page_h - MARK));
    bottom.close_path();
    marks.push(SvgElement::Path {
        attributes: Vec::new(),
        path: bottom,
    });
    let elements = vec![
        SvgElement::Title("StencilCraft print-and-cut alignment sheet".into()),
        SvgElement::Group {
            attributes: vec![
                SvgAttribute::text("id", "registration-marks"),
                SvgAttribute::text("fill", "#000000"),
                SvgAttribute::text(
                    "aria-label",
                    "Registration marks: top-left square, top-right and bottom-left corners",
                ),
            ],
            children: marks,
        },
        SvgElement::NestedSvg {
            attributes: vec![
                SvgAttribute::number("x", MARGIN),
                SvgAttribute::number("y", MARGIN),
                SvgAttribute::millimetres("width", doc.board.width_mm),
                SvgAttribute::millimetres("height", doc.board.height_mm),
                SvgAttribute::view_box([0.0, 0.0, doc.board.width_mm, doc.board.height_mm]),
                SvgAttribute::text("overflow", "hidden"),
            ],
            children: document_elements(doc, false, true),
        },
    ];
    Ok(SvgScene {
        width_mm: page_w,
        height_mm: page_h,
        view_box: [0.0, 0.0, page_w, page_h],
        elements,
    }
    .write())
}

#[derive(Debug)]
pub struct SvgImport {
    pub document: Document,
    pub warnings: Vec<String>,
}
pub fn import(svg: &str, name: &str) -> Result<Document, SvgError> {
    import_with_warnings(svg, name).map(|got| got.document)
}
pub fn import_with_warnings(svg: &str, name: &str) -> Result<SvgImport, SvgError> {
    let imported = craft_file_import::import(name, svg.as_bytes(), Default::default())
        .map_err(|error| SvgError::Import(error.to_string()))?;
    let craft_file_import::Imported::Vector(scene) = imported else {
        return Err(SvgError::Import("SVG reader returned raster data".into()));
    };
    let mut doc = Document {
        board: stencilcraft_doc::Board {
            width_mm: scene.width_mm,
            height_mm: scene.height_mm,
        },
        ..Document::default()
    };
    let mut layers = HashMap::<(String, bool, Option<String>), usize>::new();
    layers.insert(("cut".into(), false, None), 0);
    let mut warnings = scene.warnings;
    for shape in scene.shapes {
        if shape.path.elements().is_empty() {
            continue;
        }
        let classes = shape
            .class
            .as_deref()
            .unwrap_or("")
            .split_ascii_whitespace();
        let style = if shape.tag.as_deref() == Some("engrave")
            || classes.clone().any(|class| class == "stencilcraft-engrave")
        {
            CutStyle::Engrave
        } else {
            CutStyle::Cut
        };
        let layer_name = shape.layer.unwrap_or_else(|| match style {
            CutStyle::Cut => "cut".into(),
            CutStyle::Engrave => "engrave".into(),
        });
        let color = shape
            .stroke_rgb
            .map(|[red, green, blue]| format!("#{red:02x}{green:02x}{blue:02x}"));
        let key = (
            layer_name.clone(),
            style == CutStyle::Engrave,
            color.clone(),
        );
        let index = if let Some(index) = layers.get(&key) {
            *index
        } else {
            let mut layer = Layer::new(layer_name, style);
            layer.color = color;
            doc.layers.push(layer);
            let i = doc.layers.len() - 1;
            layers.insert(key, i);
            i
        };
        doc.push_shape(index, shape.path, style)
            .map_err(|e| SvgError::Model(e.to_string()))?;
    }
    if scene.dropped_warnings > 0 {
        warnings.push(format!(
            "{} additional SVG warnings omitted",
            scene.dropped_warnings
        ));
    }
    doc.validate().map_err(|e| SvgError::Model(e.to_string()))?;
    Ok(SvgImport {
        document: doc,
        warnings,
    })
}
pub fn polylines_of(bez: &BezPath) -> Vec<Polyline> {
    path_to_polylines(bez, stencilcraft_geom::DEFAULT_FLATTEN_STEPS)
}

#[cfg(test)]
mod tests_group_walk {
    use super::*;

    #[test]
    fn groups_are_skipped_not_rejected() {
        let doc = import(
            r#"<svg><g class="stencilcraft-cut"><path d="M 0 0 L 5 0 L 5 5 Z"/></g></svg>"#,
            "grouped.svg",
        )
        .unwrap();
        assert_eq!(doc.shape_count(), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_svg::paths::{
        MAX_CSS_BYTES, MAX_SVG_BYTES, MAX_SVG_DEPTH, MAX_SVG_NODES, MAX_SVG_WARNINGS,
    };
    use stencilcraft_geom::Point;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(x, y));
        p.line_to(Point::new(x + w, y));
        p.line_to(Point::new(x + w, y + h));
        p.line_to(Point::new(x, y + h));
        p.close_path();
        p
    }

    #[test]
    fn export_includes_board_size_in_viewbox() {
        let doc = Document::default();
        let svg = export(&doc).unwrap();
        assert!(
            svg.contains("viewBox=\"0.000 0.000 100.000 150.000\""),
            "{svg}"
        );
        assert!(svg.contains("width=\"100.000mm\""));
        assert!(svg.contains("data-kind=\"cut\""));
    }

    #[test]
    fn export_shapes_as_paths() {
        let mut doc = Document::default();
        doc.push_shape(0, rect(10.0, 10.0, 20.0, 20.0), CutStyle::Cut)
            .unwrap();
        let svg = export(&doc).unwrap();
        assert!(svg.contains("d=\"M10"), "{svg}");
        assert!(svg.contains("class=\"stencilcraft-cut\""));
    }

    #[test]
    fn hidden_shapes_are_not_exported() {
        let mut doc = Document::default();
        doc.push_shape(0, rect(10.0, 10.0, 20.0, 20.0), CutStyle::Cut)
            .unwrap();
        doc.push_shape(0, rect(50.0, 50.0, 5.0, 5.0), CutStyle::Cut)
            .unwrap();
        doc.layers[0].shapes[1].hidden = true;
        let svg = export(&doc).unwrap();
        assert!(svg.contains("d=\"M10"), "{svg}");
        assert!(!svg.contains("d=\"M50"), "{svg}");
    }

    fn preview_piece(w: f64, h: f64, style: CutStyle) -> Document {
        let mut doc = Document {
            board: stencilcraft_doc::Board {
                width_mm: w,
                height_mm: h,
            },
            ..Document::default()
        };
        doc.layers.clear();
        doc.layers.push(stencilcraft_doc::Layer::new("cut", style));
        doc.push_shape(0, rect(0.0, 0.0, w, h), style).unwrap();
        doc
    }

    #[test]
    fn tile_preview_pages_every_sheet_with_stroked_frames() {
        let pieces = vec![
            preview_piece(60.0, 80.0, CutStyle::Cut),
            preview_piece(60.0, 80.0, CutStyle::Engrave),
        ];
        let svg = export_tile_preview(
            &pieces,
            &TilePreviewOptions {
                columns: 2,
                scale: 0.5,
            },
        )
        .unwrap();
        // Side by side: 2 cells of 30 × 40 mm plus three 6 mm gaps across, two 6 mm
        // gaps down.
        assert!(svg.contains("width=\"78.000mm\""), "{svg}");
        assert!(svg.contains("height=\"52.000mm\""), "{svg}");
        assert_eq!(svg.matches("class=\"sheet\"").count(), 2, "{svg}");
        assert_eq!(svg.matches("class=\"preview-frame\"").count(), 2, "{svg}");
        // One cut sheet and one engrave sheet, each drawn with its own class.
        assert_eq!(svg.matches("class=\"stencilcraft-cut\"").count(), 1);
        assert_eq!(svg.matches("class=\"stencilcraft-engrave\"").count(), 1);
        // Outlines, not fills: the whole point of the preview is to show the detail.
        assert!(svg.contains(".preview-frame { fill: none;"), "{svg}");
        assert!(svg.contains(".stencilcraft-cut { fill: none;"), "{svg}");
        // Content is flipped into the page's y-down space and anchored to the cell's
        // bottom edge: 6 mm gap + 40 mm cell = 46 mm.
        assert!(
            svg.contains("translate(6.000 46.000) scale(0.5000 -0.5000)"),
            "{svg}"
        );
        assert!(svg.contains("data-sheet=\"1\""), "{svg}");
    }

    #[test]
    fn tile_preview_labels_each_sheet() {
        let pieces = vec![
            preview_piece(60.0, 80.0, CutStyle::Cut),
            preview_piece(60.0, 80.0, CutStyle::Cut),
        ];
        let svg = export_tile_preview(
            &pieces,
            &TilePreviewOptions {
                columns: 1,
                scale: 0.5,
            },
        )
        .unwrap();
        assert!(svg.contains(">sheet 1 of 2 · 60×80 mm</text>"), "{svg}");
        assert!(svg.contains(">sheet 2 of 2 · 60×80 mm</text>"), "{svg}");
        // Column-major tiling stacks the first sheet at the bottom of the page.
        let first = svg.find("data-sheet=\"0\"").unwrap();
        let second = svg.find("data-sheet=\"1\"").unwrap();
        let first_y = &svg[first..second];
        assert!(
            first_y.contains("y=\"52.000\""),
            "sheet 0 should sit below sheet 1: {svg}"
        );
    }

    #[test]
    fn tile_preview_refuses_impossible_pages() {
        let pieces = vec![preview_piece(60.0, 80.0, CutStyle::Cut)];
        assert!(matches!(
            export_tile_preview(&[], &TilePreviewOptions::default()),
            Err(SvgError::Model(_))
        ));
        assert!(matches!(
            export_tile_preview(
                &pieces,
                &TilePreviewOptions {
                    columns: 0,
                    scale: 1.0
                }
            ),
            Err(SvgError::Model(_))
        ));
        assert!(matches!(
            export_tile_preview(
                &pieces,
                &TilePreviewOptions {
                    columns: 1,
                    scale: 0.0
                }
            ),
            Err(SvgError::Model(_))
        ));
        let huge = vec![preview_piece(200_000.0, 200_000.0, CutStyle::Cut)];
        assert!(matches!(
            export_tile_preview(
                &huge,
                &TilePreviewOptions {
                    columns: 1,
                    scale: 1.0
                }
            ),
            Err(SvgError::Model(_))
        ));
    }

    #[test]
    fn import_roundtrips_path_data() {
        let mut doc = Document::default();
        doc.push_shape(0, rect(0.0, 0.0, 30.0, 40.0), CutStyle::Cut)
            .unwrap();
        let svg = export(&doc).unwrap();
        let back = import(&svg, "test.svg").unwrap();
        assert_eq!(back.shape_count(), 1);
        assert_eq!(
            back.layers[0].shapes[0].path.to_svg(),
            doc.layers[0].shapes[0].path.to_svg()
        );
    }

    #[test]
    fn import_gives_calm_errors() {
        assert!(matches!(
            import("<path></path>", "nod.svg"),
            Err(SvgError::Import(ref m)) if m.contains("d attribute")
        ));
        assert!(matches!(
            import("<path d=\"M 0 0 Q\"/>", "bad.svg"),
            Err(SvgError::Import(_))
        ));
        let transformed = import(
            "<g transform=\"scale(nope)\"><path d=\"M0 0 L1 0 Z\"/></g>",
            "tx.svg",
        );
        assert!(matches!(
            transformed,
            Err(SvgError::Import(ref m)) if m.contains("transform")
        ));
    }

    #[test]
    fn import_grows_board_for_outside_art() {
        let back = import("<path d=\"M 200 250 L 210 250 L 210 260 Z\"/>", "big.svg").unwrap();
        assert!(back.board.width_mm >= 210.0);
        assert!(back.board.height_mm >= 260.0);
    }

    #[test]
    fn imports_all_common_basic_shapes() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <rect x="1" y="2" width="10" height="8"/>
          <circle cx="20" cy="20" r="5"/>
          <ellipse cx="40" cy="20" rx="8" ry="4"/>
          <line x1="1" y1="30" x2="20" y2="30"/>
          <polyline points="1,40 10,45 20,40"/>
          <polygon points="30,40 40,40 35,50"/>
          <path d="M50 50 C55 45 60 55 65 50 Z"/>
        </svg>"#;
        let doc = import(svg, "shapes.svg").unwrap();
        assert_eq!(doc.shape_count(), 7);
        assert!(doc.layers[0].shapes[0].path.elements().len() >= 5);
        assert!(doc.layers[0].shapes[1].path.elements().len() >= 6);
    }

    #[test]
    fn imports_nested_group_transforms_and_inherited_production_style() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <g transform="translate(10,20)" data-kind="engrave">
            <g scale="2" transform="scale(2)">
              <rect x="1" y="2" width="3" height="4"/>
            </g>
          </g>
        </svg>"#;
        let doc = import(svg, "groups.svg").unwrap();
        assert_eq!(doc.shape_count(), 1);
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.layers[1].kind, CutStyle::Engrave);
        let b = doc.layers[1].shapes[0].bounds().unwrap();
        assert!((b.x0 - 12.0).abs() < 1e-9, "{b:?}");
        assert!((b.y0 - 24.0).abs() < 1e-9, "{b:?}");
        assert!((b.width() - 6.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn imports_local_use_with_x_y_transform_and_inherited_style() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <defs>
            <g id="shape" data-kind="engrave" transform="scale(2)">
              <rect x="1" y="2" width="3" height="4"/>
            </g>
            <symbol id="badge" viewBox="10 20 20 10" preserveAspectRatio="xMidYMid meet" data-kind="engrave">
              <rect x="10" y="20" width="20" height="10"/>
            </symbol>
          </defs>
          <use href="#shape" x="10" y="20" transform="translate(5,0)"/>
          <use xlink:href="#shape" x="30" y="5"
            xmlns:xlink="http://www.w3.org/1999/xlink"/>
          <use href="#badge" x="50" y="10" width="40" height="40"/>
        </svg>"##;
        let doc = import(svg, "use.svg").unwrap();
        assert_eq!(doc.shape_count(), 3);
        assert_eq!(doc.layers[0].shapes.len(), 0);
        assert_eq!(doc.layers[1].shapes.len(), 3);
        let first = doc.layers[1].shapes[0].bounds().unwrap();
        assert!((first.x0 - 17.0).abs() < 1e-9, "{first:?}");
        assert!((first.y0 - 24.0).abs() < 1e-9, "{first:?}");
        assert!((first.width() - 6.0).abs() < 1e-9, "{first:?}");
        let second = doc.layers[1].shapes[1].bounds().unwrap();
        assert!((second.x0 - 32.0).abs() < 1e-9, "{second:?}");
        assert!((second.y0 - 9.0).abs() < 1e-9, "{second:?}");
        let badge = doc.layers[1].shapes[2].bounds().unwrap();
        assert!((badge.x0 - 50.0).abs() < 1e-9, "{badge:?}");
        assert!((badge.y0 - 20.0).abs() < 1e-9, "{badge:?}");
        assert!((badge.width() - 40.0).abs() < 1e-9, "{badge:?}");
        assert!((badge.height() - 20.0).abs() < 1e-9, "{badge:?}");
        assert_eq!(doc.layers[1].shapes[2].style, CutStyle::Engrave);
    }

    #[test]
    fn local_use_cycles_missing_and_external_references_fail_calmly() {
        for svg in [
            r##"<svg><defs><g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g></defs><use href="#a"/></svg>"##,
            r##"<svg><use href="#missing"/></svg>"##,
            r#"<svg><use href="other.svg#shape"/></svg>"#,
            r##"<svg><defs><text id="label">no</text></defs><use href="#label"/></svg>"##,
            r##"<svg><rect id="same"/><circle id="same"/></svg>"##,
            r##"<svg><defs><symbol id="cycle"><use href="#cycle"/></symbol></defs><use href="#cycle"/></svg>"##,
            r##"<svg><defs><symbol id="bad" viewBox="0 0 0 4"><rect width="2" height="2"/></symbol></defs><use href="#bad"/></svg>"##,
            r##"<svg><defs><symbol id="zero" viewBox="0 0 4 4"><rect width="2" height="2"/></symbol></defs><use href="#zero" width="-1"/></svg>"##,
        ] {
            assert!(import(svg, "use-error.svg").is_err(), "{svg}");
        }
    }

    #[test]
    fn viewbox_maps_physical_mm_and_preserves_aspect_ratio() {
        for preserve in ["xMidYMid meet extra", "notxMidYMid"] {
            let svg = format!(
                r#"<svg width="200mm" height="100mm" viewBox="0 0 100 100" preserveAspectRatio="{preserve}"/>"#
            );
            assert!(import(&svg, "aspect.svg").is_err(), "{preserve}");
        }
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200mm" height="100mm"
          viewBox="0 0 100 100" preserveAspectRatio="none">
          <rect x="10" y="20" width="5" height="4"/>
        </svg>"#;
        let doc = import(svg, "viewbox.svg").unwrap();
        assert_eq!(doc.board.width_mm, 200.0);
        assert_eq!(doc.board.height_mm, 100.0);
        let b = doc.layers[0].shapes[0].bounds().unwrap();
        assert!((b.x0 - 20.0).abs() < 1e-9, "{b:?}");
        assert!((b.y0 - 20.0).abs() < 1e-9, "{b:?}");
        assert!((b.width() - 10.0).abs() < 1e-9, "{b:?}");
        assert!((b.height() - 4.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn imports_hidden_visibility_and_inline_styles() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <g style="display:none"><rect width="10" height="10"/></g>
          <g style="visibility:hidden" data-kind="engrave">
            <line style="visibility:visible" x1="1" y1="2" x2="3" y2="4"/>
            <line x1="5" y1="6" x2="7" y2="8"/>
          </g>
        </svg>"#;
        let doc = import(svg, "style.svg").unwrap();
        assert_eq!(doc.shape_count(), 1);
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.layers[1].shapes.len(), 1);
    }

    #[test]
    fn applies_css_selector_specificity_cascade_and_inherited_visibility() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <style>
            /* supported selector subset */
            path { data-kind: cut; }
            .mark { data-kind: engrave; visibility: hidden; }
            g#source > path.mark { visibility: visible; }
            path.mark[id="first"] { display: none; }
            path.mark[data-kind="engrave"] { stroke: red; }
            .mark { data-kind: cut !important; }
          </style>
          <g id="source">
            <path id="first" class="mark" d="M1 1L2 1"/>
            <path id="second" class="mark" style="data-kind:engrave" d="M3 3L4 3"/>
          </g>
        </svg>"##;
        let doc = import(svg, "css.svg").unwrap();
        assert_eq!(doc.shape_count(), 1);
        assert_eq!(doc.layers[0].shapes.len(), 1);
        assert_eq!(doc.layers[0].shapes[0].bounds().unwrap().x0, 3.0);
        assert_eq!(doc.layers.len(), 1);
    }

    #[test]
    fn rejects_unsupported_css_selectors_and_caps_stylesheet_size() {
        assert!(
            import(
                r##"<svg><style>path:hover { display:none; }</style><path d="M0 0L1 1"/></svg>"##,
                "pseudo.svg"
            )
            .is_err()
        );
        let large = format!(
            "<svg><style>{}</style></svg>",
            " ".repeat(MAX_CSS_BYTES + 1)
        );
        assert!(import(&large, "css-large.svg").is_err());
    }

    #[test]
    fn reports_unsupported_drawables_instead_of_dropping_them() {
        let svg =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text x="0" y="10">hello</text></svg>"#;
        assert!(matches!(
            import(svg, "text.svg"),
            Err(SvgError::Import(ref message)) if message.contains("<text> \"hello\" skipped")
        ));
        for svg in [
            r#"<svg><image href="a.png" width="1" height="1"/></svg>"#,
            r#"<svg><foreignObject/></svg>"#,
            r#"<svg><blink/></svg>"#,
        ] {
            assert!(import_with_warnings(svg, "drawable.svg").is_err(), "{svg}");
        }
    }

    #[test]
    fn rejects_nonfinite_geometry_and_malformed_number_lists() {
        assert!(import(r#"<svg viewBox="0 0 NaN 100"/>"#, "nan.svg").is_err());
        assert!(import(r#"<svg><polygon points="0,0,,1,1"/></svg>"#, "points.svg").is_err());
        assert!(
            import(
                r#"<svg><rect x="1e400" width="10" height="10"/></svg>"#,
                "huge.svg"
            )
            .is_err()
        );
        assert!(
            import(
                &format!("<svg>{}</svg>", " ".repeat(MAX_SVG_BYTES)),
                "large.svg"
            )
            .is_err()
        );
    }

    #[test]
    fn parses_transform_lists_and_keeps_physical_px_units() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="96px" height="48px">
          <g transform="translate(10 0) scale(2)"><line x1="0" y1="0" x2="4" y2="0"/></g>
        </svg>"#;
        let doc = import(svg, "px.svg").unwrap();
        assert!((doc.board.width_mm - 25.4).abs() < 1e-9);
        assert!((doc.board.height_mm - 12.7).abs() < 1e-9);
        let b = doc.layers[0].shapes[0].bounds().unwrap();
        assert!((b.x0 - 10.0 * 25.4 / 96.0).abs() < 1e-9, "{b:?}");
        assert!((b.width() - 8.0 * 25.4 / 96.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn malformed_xml_and_resource_limits_are_errors_not_panics() {
        assert!(matches!(
            import("<svg><g></svg>", "bad-xml.svg"),
            Err(SvgError::Import(ref e)) if e.contains("invalid XML")
        ));
        let deep = format!(
            "{}<path d=\"M0 0L1 0\"/>{}",
            "<g>".repeat(MAX_SVG_DEPTH + 2),
            "</g>".repeat(MAX_SVG_DEPTH + 2)
        );
        assert!(matches!(
            import(&deep, "deep.svg"),
            Err(SvgError::Import(ref e)) if e.contains("nesting exceeds")
        ));
        let nodes = format!("<svg>{}</svg>", "<g/>".repeat(MAX_SVG_NODES + 1));
        assert!(matches!(
            import(&nodes, "nodes.svg"),
            Err(SvgError::Import(ref e)) if e.contains("100,000-node")
        ));
    }

    #[test]
    fn deterministically_mutated_svg_never_panics() {
        let seeds = [
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="2" y="3" width="7" height="8"/></svg>"#,
            // Colour, CSS, clip, text, mask and filter paths in one seed.
            r##"<svg viewBox="0 0 20 20"><style>@media print{g{x:y}} g>rect:not(.a)+rect[id^="r"]{stroke:hsl(9,50%,50%)}</style><clipPath id="c" transform="scale(1)"><circle cx="5" cy="5" r="4"/></clipPath><g clip-path="url(#c)" color="red"><rect width="9" height="9" stroke="currentColor"/><rect id="r2" width="3" height="3"/><path d="M0 0L20 20"/></g><text id="t">hi</text><g mask="url(#m)" filter="url(#f)"><rect width="1" height="1" stroke="#0f08"/></g></svg>"##,
        ];
        for (seed_index, seed) in seeds.iter().enumerate() {
            let bytes = seed.as_bytes();
            let mut state = 0x51_6f_9a_u64 + seed_index as u64;
            for _ in 0..1024 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let index = (state as usize) % bytes.len();
                let mut mutated = bytes.to_vec();
                mutated[index] = (state >> 32) as u8;
                if let Ok(text) = std::str::from_utf8(&mutated)
                    && let Ok(imported) = import_with_warnings(text, "mutated.svg")
                {
                    imported.document.validate().unwrap();
                    assert!(imported.warnings.len() <= MAX_SVG_WARNINGS + 1);
                }
            }
        }
    }

    fn lenient(svg: &str) -> SvgImport {
        import_with_warnings(svg, "test.svg").unwrap()
    }

    fn layer<'a>(doc: &'a Document, name: &str) -> &'a stencilcraft_doc::Layer {
        doc.layers
            .iter()
            .find(|layer| layer.name == name)
            .unwrap_or_else(|| panic!("no layer {name}: {:?}", doc.layers))
    }

    #[test]
    fn stroke_colours_become_one_layer_each() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <style>.blue { stroke: rgb(0 0 100%); }</style>
          <rect width="1" height="1" stroke="red"/>
          <rect width="1" height="1" style="stroke:#F00"/>
          <rect width="1" height="1" stroke="hsl(0, 100%, 50%)"/>
          <rect width="1" height="1" class="blue"/>
          <g color="rgba(0, 128, 0, 0.5)"><rect width="1" height="1" stroke="currentColor"/></g>
          <rect width="1" height="1" stroke="url(#gradient) cornflowerblue"/>
          <rect width="1" height="1" stroke="url(#gradient)"/>
          <rect width="1" height="1" fill="black"/>
          <rect width="1" height="1" stroke="none" fill="black"/>
          <g data-kind="engrave"><rect width="1" height="1" stroke="#00800080"/></g>
        </svg>"##;
        let imported = lenient(svg);
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let doc = imported.document;
        assert_eq!(doc.shape_count(), 10);
        assert_eq!(doc.layers[0].name, "cut");
        assert_eq!(doc.layers[0].color, None);
        assert_eq!(doc.layers[0].shapes.len(), 3);
        let red = layer(&doc, "Line #ff0000");
        assert_eq!(red.shapes.len(), 3);
        assert_eq!(red.color.as_deref(), Some("#ff0000"));
        assert_eq!(red.kind, CutStyle::Cut);
        assert_eq!(layer(&doc, "Line #0000ff").shapes.len(), 1);
        assert_eq!(layer(&doc, "Line #008000").shapes.len(), 1);
        assert_eq!(layer(&doc, "Line #6495ed").shapes.len(), 1);
        let engrave = layer(&doc, "engrave #008000");
        assert_eq!(engrave.kind, CutStyle::Engrave);
        assert_eq!(engrave.color.as_deref(), Some("#008000"));
        assert_eq!(engrave.shapes[0].style, CutStyle::Engrave);
        // The same document comes through the strict import unchanged.
        assert_eq!(
            import(svg, "colours.svg").unwrap().layers.len(),
            doc.layers.len()
        );
    }

    #[test]
    fn unreadable_stroke_colour_warns_and_strict_import_refuses() {
        let svg = r#"<svg><rect id="odd" width="1" height="1" stroke="color(srgb 1 0 0)"/></svg>"#;
        let imported = lenient(svg);
        assert_eq!(imported.document.layers[0].shapes.len(), 1);
        assert_eq!(imported.warnings.len(), 1);
        assert!(
            imported.warnings[0].contains("<rect id=\"odd\">"),
            "{:?}",
            imported.warnings
        );
        assert!(import(svg, "odd.svg").is_err());
    }

    fn bounds_of(doc: &Document) -> Vec<kurbo::Rect> {
        doc.layers
            .iter()
            .flat_map(|layer| layer.shapes.iter())
            .map(|shape| shape.bounds().unwrap())
            .collect()
    }

    fn near(a: kurbo::Rect, b: (f64, f64, f64, f64)) -> bool {
        (a.x0 - b.0).abs() < 1e-3
            && (a.y0 - b.1).abs() < 1e-3
            && (a.x1 - b.2).abs() < 1e-3
            && (a.y1 - b.3).abs() < 1e-3
    }

    #[test]
    fn clip_paths_clip_closed_and_open_geometry() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <defs>
            <clipPath id="half"><rect x="0" y="0" width="10" height="20"/></clipPath>
            <clipPath id="moved" transform="translate(50 0)"><rect width="5" height="5"/></clipPath>
            <clipPath id="disc"><circle cx="30" cy="30" r="5"/></clipPath>
            <clipPath id="nothing"/>
          </defs>
          <g clip-path="url(#half)">
            <rect width="20" height="20"/>
            <line x1="-5" y1="5" x2="25" y2="5"/>
          </g>
          <rect x="40" y="0" width="20" height="20" clip-path="url('#moved')"/>
          <rect x="20" y="20" width="20" height="20" style="clip-path: url(#disc)"/>
          <g transform="translate(0 60)"><rect width="20" height="20" clip-path="url(#half)"/></g>
          <rect width="20" height="20" clip-path="url(#nothing)"/>
          <rect x="80" y="80" width="5" height="5" clip-path="url(#half)"/>
        </svg>"##;
        let imported = lenient(svg);
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let bounds = bounds_of(&imported.document);
        assert_eq!(bounds.len(), 5, "{bounds:?}");
        assert!(near(bounds[0], (0.0, 0.0, 10.0, 20.0)), "{bounds:?}");
        assert!(near(bounds[1], (0.0, 5.0, 10.0, 5.0)), "{bounds:?}");
        assert!(near(bounds[2], (50.0, 0.0, 55.0, 5.0)), "{bounds:?}");
        assert!(near(bounds[3], (25.0, 25.0, 35.0, 35.0)), "{bounds:?}");
        // The clip region lives in the referencing element's user space.
        assert!(near(bounds[4], (0.0, 60.0, 10.0, 80.0)), "{bounds:?}");
        // The clipped closed shape stays closed (a cut contour, not an open run).
        let first = &imported.document.layers[0].shapes[0].path;
        assert!(matches!(
            first.elements().last(),
            Some(kurbo::PathEl::ClosePath)
        ));
    }

    #[test]
    fn clip_paths_that_cannot_be_applied_warn() {
        for (svg, needle) in [
            (
                r##"<svg><clipPath id="c" clipPathUnits="objectBoundingBox"><rect width="1" height="1"/></clipPath><rect width="9" height="9" clip-path="url(#c)"/></svg>"##,
                "objectBoundingBox",
            ),
            (
                r##"<svg><rect width="9" height="9" clip-path="url(#missing)"/></svg>"##,
                "not a local <clipPath>",
            ),
            (
                r##"<svg><rect width="9" height="9" clip-path="inset(1px)"/></svg>"##,
                "not a local <clipPath>",
            ),
            (
                r##"<svg><rect id="r" width="1" height="1"/><rect width="9" height="9" clip-path="url(#r)"/></svg>"##,
                "not a <clipPath>",
            ),
            (
                r##"<svg><clipPath id="c"><text>A</text><rect width="1" height="1"/></clipPath><rect width="9" height="9" clip-path="url(#c)"/></svg>"##,
                "inside a clip path",
            ),
        ] {
            let imported = lenient(svg);
            assert!(
                imported.warnings.iter().any(|w| w.contains(needle)),
                "{svg}: {:?}",
                imported.warnings
            );
            assert!(imported.document.shape_count() >= 1, "{svg}");
            assert!(import(svg, "clip.svg").is_err(), "{svg}");
        }
    }

    #[test]
    fn advanced_css_selectors_match_per_selectors_level_3() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <style>
            rect[data-tag~="b"] { stroke: #000001; }
            rect[id^="pre"] { stroke: #000002; }
            rect[id$="post"] { stroke: #000003; }
            rect[id*="mid"] { stroke: #000004; }
            rect[lang|="en"] { stroke: #000005; }
            g > line:first-child { stroke: #000006; }
            rect:not(.keep):last-child { stroke: #000007; }
            #anchor + rect { stroke: #000008; }
            #anchor ~ circle { stroke: #000009; }
            :root > rect.top { stroke: #00000a; }
          </style>
          <rect class="top" width="1" height="1"/>
          <g>
            <line x2="1"/>
            <rect class="keep" data-tag="a b c" width="1" height="1"/>
            <rect id="prefix" width="1" height="1"/>
            <rect id="the-post" width="1" height="1"/>
            <rect id="amidb" width="1" height="1"/>
            <rect lang="en-GB" width="1" height="1"/>
            <rect id="anchor" class="keep" width="1" height="1"/>
            <rect width="1" height="1"/>
            <circle r="1"/>
            <rect class="keep" width="1" height="1"/>
            <rect width="1" height="1"/>
          </g>
        </svg>"##;
        let imported = lenient(svg);
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let names: Vec<_> = imported
            .document
            .layers
            .iter()
            .map(|layer| (layer.name.as_str(), layer.shapes.len()))
            .collect();
        for n in 1..=10 {
            let name = format!("Line #{n:06x}");
            assert!(
                names
                    .iter()
                    .any(|(layer, count)| *layer == name && *count == 1),
                "{name}: {names:?}"
            );
        }
        // Unmatched: the anchor, and the last-but-one `.keep` rect (excluded by :not).
        assert_eq!(imported.document.layers[0].shapes.len(), 2);
    }

    #[test]
    fn css_at_rules_and_unsupported_selectors_warn_instead_of_failing() {
        let svg = r##"<svg><style>
            @charset "utf-8";
            @import url(other.css);
            @font-face { font-family: X; src: url(x.woff); }
            @media (max-width: 10px) { rect { display: none; } }
            rect:hover, rect.ok { stroke: red; }
            a::before { stroke: blue; }
          </style><rect class="ok" width="1" height="1"/></svg>"##;
        let imported = lenient(svg);
        let warnings = imported.warnings.join("\n");
        assert!(warnings.contains("@import"), "{warnings}");
        assert!(warnings.contains("@media"), "{warnings}");
        assert!(!warnings.contains("font-face"), "{warnings}");
        assert!(!warnings.contains("charset"), "{warnings}");
        assert!(warnings.contains("rect:hover"), "{warnings}");
        assert!(warnings.contains("a::before"), "{warnings}");
        // The @media rule is not applied; the plain rule after it still is.
        assert_eq!(layer(&imported.document, "Line #ff0000").shapes.len(), 1);
        assert!(import(svg, "media.svg").is_err());
        for broken in [
            "<svg><style>@media screen { rect { x: y; }</style></svg>",
            "<svg><style>@import</style></svg>",
            "<svg><style>rect } {</style></svg>",
            "<svg><style>rect:not(:not(.a)) { stroke: red; }</style><rect/></svg>",
        ] {
            let result = import_with_warnings(broken, "broken.svg");
            assert!(
                result.is_err() || result.is_ok_and(|i| !i.warnings.is_empty()),
                "{broken}"
            );
        }
    }

    #[test]
    fn text_masks_and_filters_warn_by_name() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
          <text id="title" x="0" y="10" font-size="12">Hello <tspan>  wide   world</tspan></text>
          <text display="none">hidden</text>
          <g visibility="hidden"><text>also hidden</text></g>
          <rect id="masked" width="1" height="1" mask="url(#m)"/>
          <g filter="url(#shadow)"><rect width="1" height="1"/></g>
          <rect width="1" height="1" style="filter:none; mask:none"/>
        </svg>"##;
        let imported = lenient(svg);
        assert_eq!(imported.document.shape_count(), 3);
        assert_eq!(imported.warnings.len(), 3, "{:?}", imported.warnings);
        assert!(imported.warnings[0].contains(r#"<text id="title"> "Hello wide world""#));
        assert!(imported.warnings[0].contains("convert text to paths"));
        assert!(imported.warnings[1].contains(r#"<rect id="masked">: mask ignored"#));
        assert!(imported.warnings[2].contains("<g>: filter ignored"));
        // Masks and filters do not inherit, so the filtered group warns once.
        assert!(import(svg, "text.svg").is_err());
    }

    #[test]
    fn warnings_are_capped() {
        let svg = format!(
            "<svg>{}</svg>",
            (0..200)
                .map(|i| format!("<text id=\"t{i}\">x</text>"))
                .collect::<String>()
        );
        let imported = lenient(&svg);
        assert_eq!(imported.warnings.len(), MAX_SVG_WARNINGS + 1);
        assert!(imported.warnings[MAX_SVG_WARNINGS].contains("136 more"));
    }
}
