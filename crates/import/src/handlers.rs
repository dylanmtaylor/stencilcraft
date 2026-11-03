//! One self-contained handler per file format: detect, read, write, capabilities.
//!
//! shortcut: this is a local trait shaped for the `FormatHandler` registry being defined
//! on `feat/silhouette-formats` (docs/formats/README.md there); reconcile the two when
//! both land, keeping these handlers as the registrations.

use stencilcraft_doc::Document;

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
    fn extensions(&self) -> &'static [&'static str];
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
    vec![
        Box::new(Handler(Format::Pdf)),
        Box::new(Handler(Format::Eps)),
        Box::new(Handler(Format::Dxf)),
        Box::new(Handler(Format::Svg)),
        Box::new(Handler(Format::Raster)),
    ]
}

/// The handler for a file to write, by extension.
pub fn writer_for(extension: &str) -> Option<Box<dyn FormatHandler>> {
    let extension = extension.to_ascii_lowercase();
    handlers()
        .into_iter()
        .find(|h| h.capabilities().write && h.extensions().contains(&extension.as_str()))
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
            Format::Dxf => "DXF",
            Format::Pdf => "PDF",
            Format::Eps => "EPS / PostScript / Illustrator",
            Format::Raster => "Raster image",
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::Svg => &["svg"],
            Format::Dxf => &["dxf"],
            Format::Pdf => &["pdf"],
            Format::Eps => &["eps", "ps", "ai"],
            Format::Raster => &["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "webp"],
        }
    }

    fn detect(&self, bytes: &[u8], extension: &str) -> bool {
        Format::detect(&format!("file.{extension}"), bytes) == Some(self.0)
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
        match self.0 {
            Format::Svg => stencilcraft_svg::export(doc)
                .map(String::into_bytes)
                .map_err(|e| bad(&name, e)),
            Format::Dxf => stencilcraft_dxf::export(doc)
                .map(String::into_bytes)
                .map_err(|e| bad(&name, e)),
            Format::Pdf => stencilcraft_pdf::export(doc).map_err(|e| bad(&name, e)),
            Format::Raster => {
                let format = crate::RenderFormat::from_extension(&options.extension)
                    .ok_or_else(|| bad(&name, "raster export writes .png or .jpg"))?;
                crate::render_document(doc, options.dpi, format).map_err(|e| bad(&name, e))
            }
            Format::Eps => Err(bad(&name, "EPS export is not supported")),
        }
    }

    fn capabilities(&self) -> Capabilities {
        match self.0 {
            Format::Svg => Capabilities {
                read: true,
                write: true,
                lossless: false,
                warnings: &[
                    "placed images are not written",
                    "unsupported SVG features fail import explicitly",
                ],
            },
            Format::Dxf => Capabilities {
                read: true,
                write: true,
                lossless: false,
                warnings: &[
                    "export flattens curves into polylines and drops placed images",
                    "import reads path geometry only; text, hatches and fills are dropped",
                ],
            },
            Format::Pdf => Capabilities {
                read: true,
                write: true,
                lossless: false,
                warnings: &[
                    "export draws strokes only and drops placed images",
                    PATHS_ONLY,
                ],
            },
            Format::Eps => Capabilities {
                read: true,
                write: false,
                lossless: false,
                warnings: VECTOR_READ,
            },
            Format::Raster => Capabilities {
                read: true,
                write: true,
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
