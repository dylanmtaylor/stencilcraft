//! File-menu actions beyond open/save: import of the other vector formats (DXF, PDF, AI/EPS),
//! export to PDF/PNG/JPEG, Save Selection As and Close Tab.

use crate::StencilApp;

/// A vector format File › Import reads (through `stencilcraft-import`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportKind {
    Dxf,
    Pdf,
    /// The first page of a PDF placed as a picture (Silhouette's Import PDF as Image).
    PdfImage,
    AiEps,
    /// Machine-embroidery stitch files, each thread colour a layer of stitch runs.
    Embroidery,
}

impl ImportKind {
    fn filter(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Dxf => ("DXF", &["dxf"]),
            Self::Pdf | Self::PdfImage => ("PDF", &["pdf"]),
            Self::Embroidery => ("Embroidery", &["pes", "dst", "exp", "jef"]),
            Self::AiEps => ("Illustrator or EPS", &["ai", "eps", "ps"]),
        }
    }
}

/// A format File › Export writes with `stencilcraft_import::write_document`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    Pdf,
    Png,
    Jpeg,
    /// A Silhouette Studio document (`.studio3`) Silhouette opens.
    Studio3,
    /// A legacy Silhouette Studio V2 document (`.studio`).
    Studio,
}

impl ExportKind {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Studio3 => "studio3",
            Self::Studio => "studio",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Studio3 => "Silhouette Studio",
            Self::Studio => "Silhouette Studio V2",
        }
    }
}

/// Raster exports render at this resolution.
pub const EXPORT_DPI: f64 = 300.0;

impl StencilApp {
    pub fn import_vector_dialog(&mut self, kind: ImportKind) {
        let (name, extensions) = kind.filter();
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(name, extensions)
            .pick_file()
        {
            if kind == ImportKind::PdfImage {
                self.import_pdf_as_image(&path);
            } else {
                self.import_svg(path);
            }
        }
    }

    /// Place the first page of the PDF at `path` as a picture, at 300 dpi.
    pub fn import_pdf_as_image(&mut self, path: &std::path::Path) {
        let name = path.to_string_lossy();
        match std::fs::read(path)
            .map_err(|e| format!("{name}: {e}"))
            .and_then(|bytes| {
                stencilcraft_import::import_pdf_as_image(&bytes, &name, 300.0)
                    .map_err(|e| e.to_string())
            }) {
            Ok(image) => self.place_image(image),
            Err(error) => self.status = error,
        }
    }

    /// The document as `kind`'s bytes.
    pub fn export_bytes(&self, kind: ExportKind) -> Result<Vec<u8>, String> {
        match kind {
            ExportKind::Studio3 => {
                return stencilcraft_studio::export(&self.engine.doc).map_err(|e| e.to_string());
            }
            ExportKind::Studio => {
                return stencilcraft_studio::export_v2(&self.engine.doc).map_err(|e| e.to_string());
            }
            _ => {}
        }
        stencilcraft_import::write_document(&self.engine.doc, kind.extension(), EXPORT_DPI)
            .map_err(|e| e.to_string())
    }

    pub fn export_dialog(&mut self, kind: ExportKind) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(kind.label(), &[kind.extension()])
            .set_file_name(format!("export.{}", kind.extension()))
            .save_file()
        else {
            return;
        };
        self.status = match self
            .export_bytes(kind)
            .and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()))
        {
            Ok(()) => format!("exported {} to {}", kind.label(), path.display()),
            Err(e) => format!("{} export failed: {e}", kind.label()),
        };
    }

    /// A copy of the document holding only the selected shapes (groups and pages kept).
    pub fn selection_document(&self) -> Option<stencilcraft_doc::Document> {
        if self.selected_shapes.is_empty() {
            return None;
        }
        let mut doc = self.engine.doc.clone();
        for layer in &mut doc.layers {
            layer
                .shapes
                .retain(|shape| self.selected_shapes.contains(&shape.id));
        }
        doc.images.clear();
        Some(doc)
    }

    /// File › Save Selection As: save just the selection as its own document; the open
    /// document and its file are unchanged.
    pub fn save_selection_dialog(&mut self) {
        let Some(doc) = self.selection_document() else {
            self.status = "select shapes to save".to_string();
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("StencilCraft", &[stencilcraft_doc::FORMAT_EXTENSION])
            .set_file_name(format!("selection.{}", stencilcraft_doc::FORMAT_EXTENSION))
            .save_file()
        else {
            return;
        };
        let path = path.to_string_lossy().into_owned();
        self.status = match stencilcraft_doc::save_to(&doc, &path) {
            Ok(()) => format!("saved the selection to {path}"),
            Err(e) => e.to_string(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_engine::Command;

    #[test]
    fn exports_and_selection_copy() {
        let mut app = StencilApp::new(None);
        for x in [10.0, 40.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 10.0,
                width: 20.0,
                height: 20.0,
            });
        }
        for kind in [ExportKind::Pdf, ExportKind::Png, ExportKind::Jpeg] {
            let bytes = app.export_bytes(kind).expect("exports");
            assert!(bytes.len() > 16, "{kind:?}");
        }
        assert!(
            app.export_bytes(ExportKind::Pdf)
                .expect("pdf")
                .starts_with(b"%PDF")
        );
        assert!(app.selection_document().is_none());
        let first = app
            .engine
            .doc
            .shapes()
            .map(|(_, s)| s.id)
            .next()
            .expect("shape");
        app.set_selection([first]);
        let doc = app.selection_document().expect("selection");
        assert_eq!(doc.shapes().count(), 1);
        assert_eq!(
            app.engine.doc.shapes().count(),
            2,
            "the document is unchanged"
        );
    }
}
