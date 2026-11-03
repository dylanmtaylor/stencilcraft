//! StencilCraft document model and the `.stencilcraft` JSON format.
//!
//! A document holds an artboard in millimetres, kerf settings and layers of shapes.
//! Every read path returns `Result` with a calm, printable error: a malformed file
//! must never crash (`AGENTS.md` › "Never crash"), because it is untrusted input.

use kurbo::BezPath;
use serde::{Deserialize, Serialize};
use stencilcraft_geom::{GeomError, bounds_of_shape, check_finite_mm};

/// Native format name, written into every saved file and checked on load.
pub const FORMAT_NAME: &str = "stencilcraft";
/// Current format version; loaders accept exactly what they can read and say so
/// otherwise.
pub const FORMAT_VERSION: u64 = 1;

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("{path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    #[error("{path}: not a stencilcraft file")]
    NotAStencilcraftFile { path: String },

    #[error("{path}: format version {found}, this build reads {expected}")]
    FormatVersion {
        path: String,
        found: u64,
        expected: u64,
    },

    #[error("{path}: {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },

    #[error("{path}: {message}")]
    Structure { path: String, message: String },

    #[error("document: {0}")]
    Model(String),

    #[error(transparent)]
    Geom(#[from] GeomError),
}

/// How a shape is physically produced on a machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CutStyle {
    /// Trace the closed outline with the cutting tool.
    Cut,
    /// Trace without cutting: scoring, engraving outlines, marking.
    Engrave,
}

/// Kerf settings held in the document; applying them is `stencilcraft-kerf`'s job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KerfSettings {
    /// Nominal kerf (material removed by the tool), in millimetres. Compensation
    /// offsets contours by `width_mm / 2`.
    pub width_mm: f64,
    /// Whether exports should carry compensated contours already.
    pub apply_to_exports: bool,
}

impl Default for KerfSettings {
    fn default() -> Self {
        Self {
            width_mm: 0.0,
            apply_to_exports: false,
        }
    }
}

/// The cutting sheet: artboard size in millimetres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Board {
    pub width_mm: f64,
    pub height_mm: f64,
}

impl Default for Board {
    fn default() -> Self {
        // A default sheet for vinyl work: 100 × 150 mm.
        Self {
            width_mm: 100.0,
            height_mm: 150.0,
        }
    }
}

/// One layer of the document: a group of shapes with a production style and a
/// visibility flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub kind: CutStyle,
    pub visible: bool,
    pub shapes: Vec<Shape>,
}

impl Layer {
    pub fn new(name: impl Into<String>, kind: CutStyle) -> Self {
        Self {
            name: name.into(),
            kind,
            visible: true,
            shapes: Vec::new(),
        }
    }

    /// Sum of per-shape bounds; `None` when the layer has no shapes or only empty
    /// paths.
    pub fn bounds(&self) -> Option<kurbo::Rect> {
        let mut acc: Option<kurbo::Rect> = None;
        for shape in &self.shapes {
            if let Some(r) = shape.bounds() {
                acc = Some(match acc {
                    Some(prev) => prev.union(r),
                    None => r,
                });
            }
        }
        acc
    }
}

/// A path plus its production style. kurbo's `BezPath` serialises with the
/// workspace's `serde` feature on — no wrapper needed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub id: u64,
    pub path: BezPath,
    pub style: CutStyle,
    /// Hidden shapes are not drawn, selected or exported.
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    /// Locked shapes cannot be selected or edited until unlocked.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl Shape {
    pub fn bounds(&self) -> Option<kurbo::Rect> {
        bounds_of_shape(&self.path)
    }
}

/// Most placed raster images one document may hold.
pub const MAX_IMAGES: usize = 256;
/// Most base64 PNG bytes one placed image may carry (about 96 MiB of PNG).
pub const MAX_IMAGE_DATA: usize = 128 * 1024 * 1024;

/// A placed raster image (a trace source, never cut). Pixels are PNG, base64-encoded so
/// the document stays JSON; `Arc` keeps undo snapshots from copying the bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterImage {
    /// Shares the shape id counter, so an id names one object in the document.
    pub id: u64,
    pub name: String,
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    pub width_px: u32,
    pub height_px: u32,
    pub png_base64: std::sync::Arc<str>,
}

impl RasterImage {
    fn validate(&self) -> Result<(), DocError> {
        for (value, what) in [
            (self.x_mm, "image x"),
            (self.y_mm, "image y"),
            (self.width_mm, "image width"),
            (self.height_mm, "image height"),
        ] {
            check_finite_mm(value, what)?;
        }
        if self.width_mm <= 0.0
            || self.height_mm <= 0.0
            || self.width_px == 0
            || self.height_px == 0
        {
            return Err(DocError::Model(format!(
                "image {}: width and height must be positive",
                self.id
            )));
        }
        if self.png_base64.len() > MAX_IMAGE_DATA {
            return Err(DocError::Model(format!(
                "image {}: pixel data exceeds the 128 MiB limit",
                self.id
            )));
        }
        Ok(())
    }
}

/// A StencilCraft document: artboard, kerf configuration, layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub board: Board,
    pub kerf: KerfSettings,
    pub layers: Vec<Layer>,
    /// Next free shape id; maintained by whoever mutates the document.
    #[serde(default)]
    pub next_shape_id: u64,
    /// Placed raster images, bottom first, drawn under the cut layers.
    #[serde(default)]
    pub images: Vec<RasterImage>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            board: Board::default(),
            kerf: KerfSettings::default(),
            layers: vec![Layer::new("cut", CutStyle::Cut)],
            next_shape_id: 1,
            images: Vec::new(),
        }
    }
}

impl Document {
    /// Shapes across all layers, in order, paired with their layer index.
    pub fn shapes(&self) -> impl Iterator<Item = (usize, &Shape)> {
        self.layers
            .iter()
            .enumerate()
            .flat_map(|(li, l)| l.shapes.iter().map(move |s| (li, s)))
    }

    pub fn shape_count(&self) -> usize {
        self.layers.iter().map(|l| l.shapes.len()).sum()
    }

    pub fn get_shape(&self, id: u64) -> Option<&Shape> {
        self.shapes().map(|(_, s)| s).find(|s| s.id == id)
    }

    pub fn get_shape_mut(&mut self, id: u64) -> Option<&mut Shape> {
        self.layers
            .iter_mut()
            .flat_map(|l| l.shapes.iter_mut())
            .find(|s| s.id == id)
    }

    /// Allocates the next shape id and records the shape in `layer_index`.
    /// Errors come out printable: messages name the layer and the fault.
    pub fn push_shape(
        &mut self,
        layer_index: usize,
        path: BezPath,
        style: CutStyle,
    ) -> Result<Shape, DocError> {
        if layer_index >= self.layers.len() {
            return Err(DocError::Model(format!(
                "layer {layer_index}: no such layer (document has {})",
                self.layers.len()
            )));
        }
        let Some(layer) = self.layers.get_mut(layer_index) else {
            return Err(DocError::Model(format!("layer {layer_index}: disappeared")));
        };
        let id = self.next_shape_id;
        self.next_shape_id = self
            .next_shape_id
            .checked_add(1)
            .ok_or_else(|| DocError::Model("shape ids exhausted".to_string()))?;
        let shape = Shape {
            id,
            path,
            style,
            hidden: false,
            locked: false,
        };
        layer.shapes.push(shape.clone());
        Ok(shape)
    }

    /// Places a raster image with the next object id and returns that id.
    pub fn push_image(&mut self, mut image: RasterImage) -> Result<u64, DocError> {
        if self.images.len() >= MAX_IMAGES {
            return Err(DocError::Model(format!(
                "a document holds at most {MAX_IMAGES} images"
            )));
        }
        image.id = self.next_shape_id;
        image.validate()?;
        self.next_shape_id = self
            .next_shape_id
            .checked_add(1)
            .ok_or_else(|| DocError::Model("shape ids exhausted".to_string()))?;
        let id = image.id;
        self.images.push(image);
        Ok(id)
    }

    pub fn get_image(&self, id: u64) -> Option<&RasterImage> {
        self.images.iter().find(|image| image.id == id)
    }

    /// Checks the board is a sane sheet: positive, finite dimensions.
    pub fn validate(&self) -> Result<(), DocError> {
        check_finite_mm(self.board.width_mm, "board width")?;
        check_finite_mm(self.board.height_mm, "board height")?;
        check_finite_mm(self.kerf.width_mm, "kerf width")?;
        if self.board.width_mm <= 0.0 || self.board.height_mm <= 0.0 {
            return Err(DocError::Model(
                "board: width and height must be positive".to_string(),
            ));
        }
        if self.kerf.width_mm < 0.0 {
            return Err(DocError::Model(
                "kerf: width cannot be negative".to_string(),
            ));
        }
        if self.images.len() > MAX_IMAGES {
            return Err(DocError::Model(format!(
                "a document holds at most {MAX_IMAGES} images"
            )));
        }
        self.images.iter().try_for_each(RasterImage::validate)
    }

    /// Serialises with the format header. The header wraps the document so a reader
    /// knows what it is before reading the rest.
    pub fn save_json(&self) -> Result<String, DocError> {
        self.validate()?;
        let wrapper = serde_json::json!({
            "stencilcraft": { "name": FORMAT_NAME, "version": FORMAT_VERSION },
            "board": self.board,
            "kerf": self.kerf,
            "layers": self.layers,
            "next-shape-id": self.next_shape_id,
            "images": self.images,
        });
        serde_json::to_string_pretty(&wrapper).map_err(|e| DocError::Structure {
            path: "output".to_string(),
            message: e.to_string(),
        })
    }

    /// Parses a document from JSON with calm, positioned error messages.
    pub fn load_json(json: &str) -> Result<Document, DocError> {
        load_json_named(json, "input")
    }
}

fn load_json_named(json: &str, path: &str) -> Result<Document, DocError> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| DocError::Parse {
        path: path.to_string(),
        source: e,
    })?;
    let some_header = value.get("stencilcraft");
    let Some(header) = some_header else {
        return Err(DocError::NotAStencilcraftFile {
            path: path.to_string(),
        });
    };
    let name = header.get("name").and_then(serde_json::Value::as_str);
    if name != Some(FORMAT_NAME) {
        return Err(DocError::NotAStencilcraftFile {
            path: path.to_string(),
        });
    }
    match header.get("version").and_then(serde_json::Value::as_u64) {
        Some(v) if v == FORMAT_VERSION => {}
        Some(found) => {
            return Err(DocError::FormatVersion {
                path: path.to_string(),
                found,
                expected: FORMAT_VERSION,
            });
        }
        None => {
            return Err(DocError::NotAStencilcraftFile {
                path: path.to_string(),
            });
        }
    }
    // Read fields individually so one mistake is reported as its own structure
    // error rather than an opaque serde error for the whole document.
    let board: Board = field(&value, "board", path)?;
    let kerf: KerfSettings = field(&value, "kerf", path)?;
    let layers: Vec<Layer> = field(&value, "layers", path)?;
    let next_shape_id: u64 = value
        .get("next-shape-id")
        .map(|v| serde_json::from_value::<u64>(v.clone()))
        .transpose()
        .map_err(|e| structure_err(path, &e))?
        .unwrap_or(1);
    let images: Vec<RasterImage> = match value.get("images") {
        None => Vec::new(),
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| structure_err(path, &e))?,
    };
    let doc = Document {
        board,
        kerf,
        layers,
        next_shape_id,
        images,
    };
    doc.validate()?;
    Ok(doc)
}

fn field<T: serde::de::DeserializeOwned>(
    value: &serde_json::Value,
    key: &str,
    path: &str,
) -> Result<T, DocError> {
    let v = value.get(key).ok_or_else(|| DocError::Structure {
        path: path.to_string(),
        message: format!("missing `{key}`"),
    })?;
    serde_json::from_value(v.clone()).map_err(|e| structure_err(path, &e))
}

fn structure_err(path: &str, e: &serde_json::Error) -> DocError {
    DocError::Structure {
        path: path.to_string(),
        message: e.to_string(),
    }
}

/// File helpers: errors keep the path so the message names the file.
pub fn save_to(doc: &Document, path: &str) -> Result<(), DocError> {
    let json = doc.save_json()?;
    craft_atomic::atomic_write(std::path::Path::new(path), json.as_bytes()).map_err(|source| {
        DocError::Io {
            path: path.to_string(),
            source,
        }
    })
}

pub fn load_from(path: &str) -> Result<Document, DocError> {
    let json = std::fs::read_to_string(path).map_err(|source| DocError::Io {
        path: path.to_string(),
        source,
    })?;
    load_json_named(&json, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_geom::Point;

    fn rect_path(x: f64, y: f64, w: f64, h: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(x, y));
        p.line_to(Point::new(x + w, y));
        p.line_to(Point::new(x + w, y + h));
        p.line_to(Point::new(x, y + h));
        p.close_path();
        p
    }

    fn sample() -> Document {
        let mut doc = Document::default();
        doc.kerf.width_mm = 0.2;
        doc.push_shape(0, rect_path(10.0, 20.0, 30.0, 40.0), CutStyle::Cut)
            .unwrap();
        doc
    }

    #[test]
    fn round_trip() {
        let doc = sample();
        let json = doc.save_json().unwrap();
        let back = Document::load_json(&json).unwrap();
        assert_eq!(doc, back);
    }

    #[test]
    fn flags_round_trip_and_default_off() {
        let mut doc = sample();
        let plain = doc.save_json().unwrap();
        assert!(!plain.contains("hidden") && !plain.contains("locked"));
        doc.layers[0].shapes[0].hidden = true;
        doc.layers[0].shapes[0].locked = true;
        let back = Document::load_json(&doc.save_json().unwrap()).unwrap();
        assert_eq!(doc, back);
    }

    #[test]
    fn header_written() {
        let json = sample().save_json().unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let h = &v["stencilcraft"];
        assert_eq!(h["name"].as_str(), Some(FORMAT_NAME));
        assert_eq!(h["version"].as_u64(), Some(FORMAT_VERSION));
    }

    #[test]
    fn junk_is_an_error_not_a_crash() {
        assert!(Document::load_json("not json at all").is_err());
        assert!(matches!(
            Document::load_json("{}"),
            Err(DocError::NotAStencilcraftFile { .. })
        ));
    }

    #[test]
    fn future_version_is_rejected_calmly() {
        let json = r#"{"stencilcraft": {"name": "stencilcraft", "version": 99}}"#;
        assert!(matches!(
            Document::load_json(json),
            Err(DocError::FormatVersion {
                found: 99,
                expected: 1,
                ..
            })
        ));
    }

    #[test]
    fn missing_field_has_structure_error() {
        let json = r#"{"stencilcraft": {"name": "stencilcraft", "version": 1},
            "board": {"width_mm": 1, "height_mm": 1}}"#;
        assert!(matches!(
            Document::load_json(json),
            Err(DocError::Structure { ref message, .. }) if message.contains("kerf")
        ));
    }

    #[test]
    fn images_round_trip_and_share_the_id_counter() {
        let mut doc = sample();
        let id = doc
            .push_image(RasterImage {
                id: 0,
                name: "photo".to_string(),
                x_mm: 0.0,
                y_mm: 0.0,
                width_mm: 10.0,
                height_mm: 5.0,
                width_px: 2,
                height_px: 1,
                png_base64: "AAAA".into(),
            })
            .unwrap();
        assert_eq!(id, 2);
        let back = Document::load_json(&doc.save_json().unwrap()).unwrap();
        assert_eq!(back, doc);
        let mut bad = doc.clone();
        bad.images[0].width_mm = -1.0;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn push_shape_assigns_ids() {
        let mut doc = Document::default();
        let s = doc
            .push_shape(0, rect_path(0.0, 0.0, 5.0, 5.0), CutStyle::Cut)
            .unwrap();
        assert_eq!(s.id, 1);
        let s2 = doc
            .push_shape(0, rect_path(5.0, 5.0, 5.0, 5.0), CutStyle::Cut)
            .unwrap();
        assert_eq!(s2.id, 2);
        assert!(matches!(
            doc.push_shape(9, rect_path(0.0, 0.0, 1.0, 1.0), CutStyle::Cut),
            Err(DocError::Model(_))
        ));
    }
}
