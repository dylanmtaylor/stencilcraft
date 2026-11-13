//! One self-contained handler per file format: detect, read, write, capabilities.
//!
//! shortcut: this is a local trait shaped for the `FormatHandler` registry being defined
//! on `feat/silhouette-formats` (docs/formats/README.md there); reconcile the two when
//! both land, keeping these handlers as the registrations.

use stencilcraft_doc::{CutStyle, Document};

use crate::{Format, ImportError, ImportOptions, bad};

/// What a handler can do, and what the user should be told it loses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub read: bool,
    pub write: bool,
    /// Reading then writing keeps everything StencilCraft models.
    pub lossless: bool,
    /// User-facing notes about what does not survive.
    pub warnings: &'static [&'static str],
}

/// Export settings a writer may use.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportOptions {
    /// Raster writers: pixels per inch.
    pub dpi: f64,
    /// The target file's extension, lower case (`png`, `jpg` …).
    pub extension: String,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            dpi: 300.0,
            extension: String::new(),
        }
    }
}

pub trait FormatHandler {
    fn name(&self) -> &'static str;
    /// Shared format identifiers accepted by this app handler.
    fn formats(&self) -> &'static [craft_file_import::DetectedFormat];
    fn detect(&self, bytes: &[u8], extension: &str) -> bool;
    fn read(
        &self,
        bytes: &[u8],
        name: &str,
        options: &ImportOptions,
    ) -> Result<Document, ImportError>;
    fn write(&self, doc: &Document, options: &ExportOptions) -> Result<Vec<u8>, ImportError>;
    fn capabilities(&self) -> Capabilities;
}

/// The handlers this crate provides, in detection order.
pub fn handlers() -> Vec<Box<dyn FormatHandler>> {
    craft_file_import::CAPABILITIES
        .iter()
        .map(|capability| Box::new(Handler(capability.format)) as Box<dyn FormatHandler>)
        .collect()
}

/// The handler for a file to write, by extension.
pub fn writer_for(extension: &str) -> Option<Box<dyn FormatHandler>> {
    let format = craft_file_import::write::writer_family_for_extension(extension)?;
    Some(Box::new(Handler(format)))
}

/// Serialize a Stencil document through the shared writer registry.
///
/// The adapter owns the product-specific document-to-scene mapping; format
/// selection and codec behavior are delegated to craft-file-import.
pub fn write_document(doc: &Document, extension: &str, dpi: f64) -> Result<Vec<u8>, ImportError> {
    writer_for(extension)
        .ok_or_else(|| {
            bad(
                &format!("output.{extension}"),
                "unsupported output extension",
            )
        })?
        .write(
            doc,
            &ExportOptions {
                dpi,
                extension: extension.to_ascii_lowercase(),
            },
        )
}

struct Handler(Format);

const PATHS_ONLY: &str = "only path geometry is read: fills, stroke styles, text, images and symbol instances are dropped";
const VECTOR_READ: &[&str] = &[
    PATHS_ONLY,
    "every path comes in as a cut shape on one layer",
];

impl FormatHandler for Handler {
    fn name(&self) -> &'static str {
        match self.0 {
            Format::Svg => "SVG",
            Format::Svgz => "SVGZ",
            Format::Dxf => "DXF",
            Format::Pdf => "PDF",
            Format::Eps => "EPS / PostScript / Illustrator",
            Format::Raster => "Raster image",
        }
    }

    fn formats(&self) -> &'static [craft_file_import::DetectedFormat] {
        craft_file_import::supported_formats(self.0)
    }

    fn detect(&self, bytes: &[u8], extension: &str) -> bool {
        craft_file_import::format_for(&format!("file.{extension}"), bytes) == Some(self.0)
    }

    fn read(
        &self,
        bytes: &[u8],
        name: &str,
        options: &ImportOptions,
    ) -> Result<Document, ImportError> {
        crate::read_format(self.0, bytes, name, options)
    }

    fn write(&self, doc: &Document, options: &ExportOptions) -> Result<Vec<u8>, ImportError> {
        let name = format!("output.{}", options.extension);
        let writer = craft_file_import::write::writer_format_for_extension(&options.extension)
            .ok_or_else(|| bad(&name, "no shared writer supports this extension"))?;
        match writer {
            craft_file_import::write::WriteFormat::Png
            | craft_file_import::write::WriteFormat::Jpeg => {
                let pixels = crate::render(doc, options.dpi).map_err(|e| bad(&name, e))?;
                craft_file_import::write::write_raster(
                    &options.extension,
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
                craft_file_import::write::write_vector(&options.extension, &scene)
                    .map_err(|e| bad(&name, e))
            }
        }
    }

    fn capabilities(&self) -> Capabilities {
        let write = craft_file_import::supports_writer(self.0);
        match self.0 {
            Format::Svg => Capabilities {
                read: true,
                write,
                lossless: false,
                warnings: &[
                    "placed images are not written",
                    "unsupported SVG features fail import explicitly",
                ],
            },
            Format::Svgz => Capabilities {
                read: true,
                write,
                lossless: false,
                warnings: VECTOR_READ,
            },
            Format::Dxf => Capabilities {
                read: true,
                write,
                lossless: false,
                warnings: &[
                    "export flattens curves into polylines and drops placed images",
                    "import reads path geometry only; text, hatches and fills are dropped",
                ],
            },
            Format::Pdf => Capabilities {
                read: true,
                write,
                lossless: false,
                warnings: &[
                    "export draws strokes only and drops placed images",
                    PATHS_ONLY,
                ],
            },
            Format::Eps => Capabilities {
                read: true,
                write,
                lossless: false,
                warnings: VECTOR_READ,
            },
            Format::Raster => Capabilities {
                read: true,
                write,
                lossless: false,
                warnings: &[
                    "import places the image to trace; it is not cut until traced",
                    "export (PNG/JPEG only) is a picture at the chosen dpi, not editable geometry",
                    "HEIC/HEIF and animated frames past the first are not read",
                ],
            },
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
    fn registry_detects_reads_and_writes() {
        let handlers = handlers();
        assert!(
            handlers
                .iter()
                .any(|h| h.detect(b"%PDF-1.4", "bin") && h.name() == "PDF")
        );
        assert!(
            handlers
                .iter()
                .all(|h| !h.capabilities().warnings.is_empty())
        );
        let mut doc = Document::default();
        let mut path = kurbo::BezPath::new();
        path.move_to((1.0, 1.0));
        path.line_to((9.0, 1.0));
        path.line_to((9.0, 9.0));
        path.close_path();
        doc.push_shape(0, path, stencilcraft_doc::CutStyle::Cut)
            .unwrap();
        for ext in ["svg", "dxf", "pdf", "png", "jpg"] {
            let writer = writer_for(ext).unwrap();
            let bytes = writer
                .write(
                    &doc,
                    &ExportOptions {
                        dpi: 72.0,
                        extension: ext.to_string(),
                    },
                )
                .unwrap();
            let reader = handlers.iter().find(|h| h.detect(&bytes, ext)).unwrap();
            let back = reader
                .read(&bytes, &format!("x.{ext}"), &ImportOptions::default())
                .unwrap();
            if ["svg", "dxf", "pdf"].contains(&ext) {
                assert_eq!(back.shape_count(), 1, "{ext}");
            } else {
                assert_eq!(back.images.len(), 1, "{ext}");
            }
        }
        assert!(writer_for("eps").is_none());
    }
}
