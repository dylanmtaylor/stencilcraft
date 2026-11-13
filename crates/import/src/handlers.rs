//! Stencil document adapters for the shared format registry.

use stencilcraft_doc::{CutStyle, Document};

use crate::{ImportError, bad};

/// Shared capability records used by file dialogs and callers querying format support.
pub use craft_file_import::{
    CAPABILITIES as FORMAT_CAPABILITIES, Capability as FormatCapabilities,
};

/// Serialize a Stencil document through the shared writer registry.
///
/// The adapter owns the product-specific document-to-scene mapping; format
/// selection and codec behavior are delegated to craft-file-import.
pub fn write_document(doc: &Document, extension: &str, dpi: f64) -> Result<Vec<u8>, ImportError> {
    let name = format!("output.{extension}");
    let extension = extension.to_ascii_lowercase();
    let writer = craft_file_import::write::writer_format_for_extension(&extension)
        .ok_or_else(|| bad(&name, "unsupported output extension"))?;
    match writer {
        craft_file_import::write::WriteFormat::Png
        | craft_file_import::write::WriteFormat::Jpeg => {
            let pixels = crate::render(doc, dpi).map_err(|e| bad(&name, e))?;
            craft_file_import::write::write_raster(
                &extension,
                pixels.width(),
                pixels.height(),
                pixels.as_raw(),
                90,
            )
            .map_err(|e| bad(&name, e))
        }
        _ => {
            doc.validate().map_err(|e| bad(&name, e))?;
            let scene = output_scene(doc, writer).map_err(|e| bad(&name, e))?;
            craft_file_import::write::write_vector(&extension, &scene).map_err(|e| bad(&name, e))
        }
    }
}

fn output_scene(
    doc: &Document,
    format: craft_file_import::write::WriteFormat,
) -> Result<craft_file_import::write::OutputScene, ImportError> {
    let board = (doc.board.width_mm, doc.board.height_mm);
    if !(board.0.is_finite() && board.1.is_finite() && board.0 > 0.0 && board.1 > 0.0) {
        return Err(bad(
            "export",
            "board dimensions must be finite and positive",
        ));
    }
    let svg_bounds = doc
        .layers
        .iter()
        .filter(|layer| !layer.template)
        .filter_map(stencilcraft_doc::Layer::bounds)
        .fold(
            kurbo::Rect::new(0.0, 0.0, board.0, board.1),
            |bounds, layer| bounds.union(layer),
        );
    let svg = matches!(
        format,
        craft_file_import::write::WriteFormat::Svg | craft_file_import::write::WriteFormat::Svgz
    );
    let (width_mm, height_mm, view_box) = if svg {
        (
            svg_bounds.width(),
            svg_bounds.height(),
            [
                svg_bounds.x0,
                svg_bounds.y0,
                svg_bounds.width(),
                svg_bounds.height(),
            ],
        )
    } else {
        (board.0, board.1, [0.0, 0.0, board.0, board.1])
    };
    let mut layers = Vec::new();
    let mut mapped = vec![None; doc.layers.len()];
    for (source_index, layer) in doc
        .layers
        .iter()
        .enumerate()
        .filter(|(_, layer)| !layer.template)
    {
        let style = layer.kind;
        mapped[source_index] = Some(layers.len());
        layers.push(craft_file_import::write::OutputLayer {
            name: layer.name.clone(),
            dxf_name: layer_name(&layer.name),
            visible: layer.visible,
            dxf_color_index: layer_color(style) as u8,
            class: Some(
                match style {
                    CutStyle::Cut => "stencilcraft-cut",
                    CutStyle::Engrave => "stencilcraft-engrave",
                }
                .into(),
            ),
            data_kind: Some(
                match style {
                    CutStyle::Cut => "cut",
                    CutStyle::Engrave => "engrave",
                }
                .into(),
            ),
        });
    }
    let mut paths = Vec::new();
    for (layer_index, layer) in doc
        .layers
        .iter()
        .enumerate()
        .filter(|(_, layer)| !layer.template)
    {
        let Some(output_layer) = mapped.get(layer_index).copied().flatten() else {
            continue;
        };
        for shape in layer.shapes.iter().filter(|shape| !shape.hidden) {
            let style = shape.style;
            let color_rgb = match style {
                CutStyle::Cut => [192, 57, 43],
                CutStyle::Engrave => [45, 108, 223],
            };
            let pdf = format == craft_file_import::write::WriteFormat::Pdf;
            let dxf = format == craft_file_import::write::WriteFormat::Dxf;
            paths.push(craft_file_import::write::OutputPath {
                path: if pdf {
                    shape.path.clone()
                } else {
                    shape.cut_path().into_owned()
                },
                layer: output_layer,
                color_rgb,
                stroke_width_mm: if pdf {
                    0.25
                } else if style == CutStyle::Cut {
                    0.3
                } else {
                    0.2
                },
                class: Some(
                    match style {
                        CutStyle::Cut => "stencilcraft-cut",
                        CutStyle::Engrave => "stencilcraft-engrave",
                    }
                    .into(),
                ),
                data_kind: Some(
                    match style {
                        CutStyle::Cut => "cut",
                        CutStyle::Engrave => "engrave",
                    }
                    .into(),
                ),
                close_open_contours: dxf
                    && matches!((style, layer.kind), (CutStyle::Cut, _) | (_, CutStyle::Cut)),
            });
        }
    }
    Ok(craft_file_import::write::OutputScene {
        width_mm,
        height_mm,
        view_box,
        layers,
        paths,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_registry_writes_and_reads() {
        assert!(FORMAT_CAPABILITIES.iter().any(|capability| {
            capability.format == crate::Format::Pdf
                && capability
                    .detected_formats
                    .contains(&craft_file_import::DetectedFormat::Pdf)
        }));
        let mut doc = Document::default();
        let mut path = kurbo::BezPath::new();
        path.move_to((1.0, 1.0));
        path.line_to((9.0, 1.0));
        path.line_to((9.0, 9.0));
        path.close_path();
        doc.push_shape(0, path, stencilcraft_doc::CutStyle::Cut)
            .unwrap();
        for ext in ["svg", "dxf", "pdf", "png", "jpg"] {
            let bytes = write_document(&doc, ext, 72.0).unwrap();
            let back =
                crate::import_bytes(&bytes, &format!("x.{ext}"), &Default::default()).unwrap();
            if ["svg", "dxf", "pdf"].contains(&ext) {
                assert_eq!(back.shape_count(), 1, "{ext}");
            } else {
                assert_eq!(back.images.len(), 1, "{ext}");
            }
        }
        assert!(craft_file_import::write::writer_format_for_extension("eps").is_none());
    }
}
