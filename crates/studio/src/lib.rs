//! Reader for Silhouette Studio documents: `.studio` (header `silhouette04;`) and
//! `.studio3` (header `silhouette05;`).
//!
//! Everything here comes from differential analysis of documents we drew and saved
//! ourselves in Silhouette Studio; see `docs/formats/silhouette-studio.md` for the
//! layout, the sample pairs behind each fact, and what is still unknown.
//!
//! The bytes are read by craft-libs' `craft-silhouette-studio` (bounded, fails closed,
//! protected designs rejected); this crate turns its shapes into a document: y-up
//! coordinates and one cut layer per line colour.

use kurbo::Affine;
use stencilcraft_doc::format::{Capabilities, Converted, Fidelity, FormatHandler};
use stencilcraft_doc::{CutStyle, Document, Layer};

pub use craft_silhouette_studio::{MAX_FILE_BYTES, Version, is_studio_path, sniff};

#[derive(Debug, thiserror::Error)]
pub enum StudioError {
    #[error("{0}: not a Silhouette Studio document")]
    NotStudio(String),
    #[error("{name}: Silhouette Studio document version {version} is not supported yet")]
    Unsupported { name: String, version: String },
    #[error(
        "{0}: this design is protected or encrypted (for example a store purchase); StencilCraft does not open protected designs"
    )]
    Protected(String),
    #[error("{0}: file exceeds the 64 MiB import limit")]
    TooLarge(String),
    #[error("{name}: damaged Silhouette Studio document: {what}")]
    Malformed { name: String, what: String },
}

/// A successful import: the document plus notes about what was skipped or assumed.
#[derive(Debug, Clone)]
pub struct Imported {
    pub doc: Document,
    pub version: Version,
    pub warnings: Vec<String>,
}

/// Reads a `.studio` / `.studio3` document into a StencilCraft document.
pub fn import(bytes: &[u8], name: &str) -> Result<Imported, StudioError> {
    use craft_silhouette_studio::Error;
    let bad = |what: &str| StudioError::Malformed {
        name: name.to_string(),
        what: what.to_string(),
    };
    let read = craft_silhouette_studio::import(bytes).map_err(|e| match e {
        Error::NotStudio => StudioError::NotStudio(name.to_string()),
        Error::Unsupported { version } => StudioError::Unsupported {
            name: name.to_string(),
            version,
        },
        Error::Protected => StudioError::Protected(name.to_string()),
        Error::TooLarge => StudioError::TooLarge(name.to_string()),
        Error::Malformed(what) => bad(&what),
    })?;
    let mut warnings = Vec::new();
    let mut doc = Document {
        layers: Vec::new(),
        ..Document::default()
    };
    match read.page_mm {
        Some((w, h)) => {
            doc.board.width_mm = w;
            doc.board.height_mm = h;
        }
        None => {
            warnings.push("page size not found; using the default sheet".to_string());
        }
    }
    warnings.extend(read.warnings);
    // Silhouette is y-down from the page's top-left; documents are y-up.
    let to_doc = Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, doc.board.height_mm]);
    for shape in read.shapes {
        let layer = layer_for(&mut doc, shape.line_color);
        doc.push_shape(layer, to_doc * shape.path, CutStyle::Cut)
            .map_err(|e| bad(&e.to_string()))?;
    }
    if doc.layers.is_empty() {
        doc.layers.push(Layer::new("cut", CutStyle::Cut));
    }

    doc.validate().map_err(|e| bad(&e.to_string()))?;
    Ok(Imported {
        doc,
        version: read.version,
        warnings,
    })
}

/// One cut layer per line colour, like Silhouette's Send by Line Color; shapes whose
/// style was not found go on a plain `cut` layer.
fn layer_for(doc: &mut Document, color: Option<[u8; 3]>) -> usize {
    let (name, color) = match color {
        Some([r, g, b]) => {
            let hex = format!("#{r:02x}{g:02x}{b:02x}");
            (format!("Line {hex}"), Some(hex))
        }
        None => ("cut".to_string(), None),
    };
    if let Some(i) = doc.layers.iter().position(|l| l.name == name) {
        return i;
    }
    let mut layer = Layer::new(name, CutStyle::Cut);
    layer.color = color;
    doc.layers.push(layer);
    doc.layers.len() - 1
}

/// The Silhouette Studio handler for the shared format seam (`.studio` and `.studio3`).
pub struct StudioHandler;

impl FormatHandler for StudioHandler {
    fn name(&self) -> &'static str {
        "Silhouette Studio (.studio, .studio3)"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["studio", "studio3"]
    }
    fn sniff(&self, bytes: &[u8]) -> bool {
        sniff(bytes).is_some()
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: Fidelity::Lossy,
            write: Fidelity::None,
            notes: &[
                "Reads every object's outline in page coordinates (open and closed paths, lines and Bézier curves, compound paths, text as outlines) and the page size.",
                "Open paths, text, colours, layers and cut settings are not read yet.",
                "Protected or encrypted store designs are rejected, never decrypted.",
            ],
        }
    }
    fn read(&self, bytes: &[u8], name: &str) -> Result<Converted<Document>, String> {
        let got = import(bytes, name).map_err(|e| e.to_string())?;
        Ok(Converted {
            value: got.doc,
            warnings: got.warnings,
        })
    }
}
