//! Adapter from the shared Silhouette Studio reader to the StencilCraft document model.

use kurbo::{Affine, BezPath};
use stencilcraft_doc::{CutStyle, Document, Layer};

pub const MAX_FILE_BYTES: usize = craft_silhouette_studio::MAX_FILE_BYTES;

pub fn is_studio_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(craft_file_formats::from_extension)
        == Some(craft_file_formats::Format::Studio)
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V2,
    V3,
}

#[derive(Debug, Clone)]
pub struct Imported {
    pub doc: Document,
    pub version: Version,
    pub warnings: Vec<String>,
}

pub fn import(bytes: &[u8], name: &str) -> Result<Imported, StudioError> {
    let parsed = craft_silhouette_studio::import(bytes).map_err(|error| {
        use craft_silhouette_studio::Error as E;
        match error {
            E::NotStudio => StudioError::NotStudio(name.to_owned()),
            E::Unsupported { version } => StudioError::Unsupported {
                name: name.to_owned(),
                version,
            },
            E::Protected => StudioError::Protected(name.to_owned()),
            E::TooLarge => StudioError::TooLarge(name.to_owned()),
            E::Malformed(what) => StudioError::Malformed {
                name: name.to_owned(),
                what,
            },
        }
    })?;
    let height = parsed.page_mm.map_or(210.0, |(_, h)| h);
    let mut doc = Document {
        layers: Vec::new(),
        ..Document::default()
    };
    if let Some((w, h)) = parsed.page_mm {
        doc.board.width_mm = w;
        doc.board.height_mm = h;
    }
    let to_doc = Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, height]);
    for shape in parsed.shapes {
        let layer = layer_for(&mut doc, shape.line_color);
        let path: BezPath = to_doc * shape.path;
        doc.push_shape(layer, path, CutStyle::Cut)
            .map_err(|e| StudioError::Malformed {
                name: name.to_owned(),
                what: e.to_string(),
            })?;
    }
    if doc.layers.is_empty() {
        doc.layers.push(Layer::new("cut", CutStyle::Cut));
    }
    doc.validate().map_err(|e| StudioError::Malformed {
        name: name.to_owned(),
        what: e.to_string(),
    })?;
    Ok(Imported {
        doc,
        version: match parsed.version {
            craft_silhouette_studio::Version::V2 => Version::V2,
            craft_silhouette_studio::Version::V3 => Version::V3,
        },
        warnings: parsed.warnings,
    })
}

/// Detect and import a Silhouette document from disk. Format recognition stays in the shared
/// file-format registry; this crate only adapts the recognized Studio document into Stencil data.
pub fn import_path(path: &str) -> Result<Option<Imported>, String> {
    if std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(craft_file_formats::from_extension)
        == Some(craft_file_formats::Format::StencilCraft)
    {
        return Ok(None);
    }
    let size = std::fs::metadata(path)
        .map_err(|error| format!("{path}: {error}"))?
        .len();
    if size > MAX_FILE_BYTES as u64 {
        let is_studio_extension = std::path::Path::new(path)
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .and_then(craft_file_formats::from_extension)
            == Some(craft_file_formats::Format::Studio);
        if is_studio_extension {
            return Err(StudioError::TooLarge(path.to_owned()).to_string());
        }
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
    if craft_file_formats::detect(path, &bytes) != Some(craft_file_formats::Format::Studio) {
        return Ok(None);
    }
    import(&bytes, path)
        .map(Some)
        .map_err(|error| error.to_string())
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
