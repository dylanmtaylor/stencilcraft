//! StencilCraft document model and the `.stencilcraft` JSON format.
//!
//! A document holds an artboard in millimetres, kerf settings and layers of shapes.
//! Every read path returns `Result` with a calm, printable error: a malformed file
//! must never crash (`AGENTS.md` › "Never crash"), because it is untrusted input.

pub mod canvas;
pub mod library;
pub mod recovery;
pub mod templates;


pub use canvas::{
    Grid, Group, GroupChild, Guide, GuideAxis, Metadata, Orientation, Page, PagePreset,
    PageSetup, SnapSettings,
};
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
    /// A locked layer's shapes cannot be selected or edited.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    /// A template (guide) layer is shown on the canvas but never exported or cut.
    #[serde(default, skip_serializing_if = "is_false")]
    pub template: bool,
    /// Material colour label, `#rrggbb`; Color Sync merges similar ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

impl Layer {
    pub fn new(name: impl Into<String>, kind: CutStyle) -> Self {
        Self {
            name: name.into(),
            kind,
            visible: true,
            shapes: Vec::new(),
            locked: false,
            template: false,
            color: None,
        }
    }

    /// Whether exports and cuts include this layer: visible and not a template.
    pub fn is_output(&self) -> bool {
        self.visible && !self.template
    }

    /// Whether newly created shapes may land here: not locked, not a template.
    pub fn takes_new_shapes(&self) -> bool {
        !self.locked && !self.template
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

fn yes() -> bool {
    true
}

impl Shape {
    pub fn bounds(&self) -> Option<kurbo::Rect> {
        bounds_of_shape(&self.path)
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
    /// Mat, media, colour and name of the main board (page 0).
    #[serde(default)]
    pub board_setup: PageSetup,
    /// Extra pages beside the main board.
    #[serde(default)]
    pub pages: Vec<Page>,
    #[serde(default)]
    pub guides: Vec<Guide>,
    #[serde(default)]
    pub grid: Grid,
    #[serde(default)]
    pub snap: SnapSettings,
    /// Groups; members that no longer exist are ignored and dropped on save.
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub meta: Metadata,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            board: Board::default(),
            kerf: KerfSettings::default(),
            layers: vec![Layer::new("cut", CutStyle::Cut)],
            next_shape_id: 1,
            board_setup: PageSetup::default(),
            pages: Vec::new(),
            guides: Vec::new(),
            grid: Grid::default(),
            snap: SnapSettings::default(),
            groups: Vec::new(),
            meta: Metadata::default(),
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
        self.validate_canvas()
    }

    /// Serialises with the format header. The header wraps the document so a reader
    /// knows what it is before reading the rest. Canvas fields are written only when
    /// set, so a plain document saves exactly as it did before they existed.
    pub fn save_json(&self) -> Result<String, DocError> {
        self.validate()?;
        let mut wrapper = serde_json::json!({
            "stencilcraft": { "name": FORMAT_NAME, "version": FORMAT_VERSION },
            "board": self.board,
            "kerf": self.kerf,
            "layers": self.layers,
            "next-shape-id": self.next_shape_id,
        });
        let groups = self.pruned_groups();
        let optional = [
            ("board-setup", !self.board_setup.is_empty(), json(&self.board_setup)),
            ("pages", !self.pages.is_empty(), json(&self.pages)),
            ("guides", !self.guides.is_empty(), json(&self.guides)),
            ("grid", self.grid != Grid::default(), json(&self.grid)),
            ("snap", self.snap != SnapSettings::default(), json(&self.snap)),
            ("groups", !groups.is_empty(), json(&groups)),
            ("meta", !self.meta.is_empty(), json(&self.meta)),
        ];
        if let Some(map) = wrapper.as_object_mut() {
            for (key, present, value) in optional {
                if present {
                    map.insert(key.to_string(), value?);
                }
            }
        }
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
    let mut doc = Document {
        board,
        kerf,
        layers,
        next_shape_id,
        board_setup: optional_field(&value, "board-setup", path)?,
        pages: optional_field(&value, "pages", path)?,
        guides: optional_field(&value, "guides", path)?,
        grid: optional_field(&value, "grid", path)?,
        snap: optional_field(&value, "snap", path)?,
        groups: optional_field(&value, "groups", path)?,
        meta: optional_field(&value, "meta", path)?,
    };
    doc.validate()?;
    doc.groups = doc.pruned_groups();
    Ok(doc)
}

/// A key added after version 1: absent means its default (older files stay readable).
fn optional_field<T: serde::de::DeserializeOwned + Default>(
    value: &serde_json::Value,
    key: &str,
    path: &str,
) -> Result<T, DocError> {
    match value.get(key) {
        None => Ok(T::default()),
        Some(_) => field(value, key, path),
    }
}

fn json<T: Serialize>(value: &T) -> Result<serde_json::Value, DocError> {
    serde_json::to_value(value).map_err(|e| structure_err("output", &e))
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

    fn canvas_sample() -> Document {
        let mut doc = sample();
        doc.push_shape(0, rect_path(50.0, 50.0, 5.0, 5.0), CutStyle::Cut)
            .unwrap();
        doc.board_setup.mat = Some("12x12".to_string());
        doc.pages.push(Page {
            x_mm: 120.0,
            y_mm: 0.0,
            width_mm: 210.0,
            height_mm: 297.0,
            setup: PageSetup {
                name: "A4".to_string(),
                color: Some("#ffeedd".to_string()),
                ..PageSetup::default()
            },
        });
        doc.guides.push(Guide {
            axis: GuideAxis::Vertical,
            position_mm: 12.5,
            locked: true,
        });
        doc.grid.spacing_mm = 5.0;
        doc.grid.subdivisions = 4;
        doc.snap.grid = true;
        doc.layers[0].color = Some("#112233".to_string());
        doc.layers.push(Layer {
            template: true,
            locked: true,
            ..Layer::new("guide", CutStyle::Cut)
        });
        doc.groups = vec![
            Group {
                id: 1,
                name: "inner".to_string(),
                children: vec![GroupChild::Shape(1)],
            },
            Group {
                id: 2,
                name: String::new(),
                children: vec![GroupChild::Group(1), GroupChild::Shape(2)],
            },
        ];
        doc.meta.title = "Card".to_string();
        doc.meta.notes = "for mum".to_string();
        doc.meta.thumbnail_svg = Some(doc.render_thumbnail(4096));
        doc
    }

    #[test]
    fn canvas_fields_round_trip() {
        let doc = canvas_sample();
        let back = Document::load_json(&doc.save_json().unwrap()).unwrap();
        assert_eq!(doc, back);
        assert_eq!(back.group_chain(GroupChild::Shape(1)), vec![2, 1]);
        assert_eq!(back.group_shapes(2), vec![1, 2]);
    }

    #[test]
    fn plain_documents_save_without_canvas_keys() {
        let json = sample().save_json().unwrap();
        for key in ["pages", "guides", "grid", "snap", "groups", "meta", "board-setup", "template"] {
            assert!(!json.contains(key), "{key} written for a plain document");
        }
    }

    #[test]
    fn version_1_files_without_canvas_fields_still_load() {
        for name in ["starter", "poster", "stencil-frame"] {
            let path = format!("{}/../../examples/{name}.stencilcraft", env!("CARGO_MANIFEST_DIR"));
            let doc = load_from(&path).unwrap();
            assert!(doc.pages.is_empty() && doc.groups.is_empty() && doc.guides.is_empty());
            assert_eq!(doc.grid, Grid::default());
            assert!(doc.layers.iter().all(|l| !l.template && !l.locked));
        }
    }

    #[test]
    fn dangling_group_members_are_pruned_not_fatal() {
        let mut doc = canvas_sample();
        doc.layers[0].shapes.retain(|s| s.id != 1);
        // Group 1 loses its only member and disappears; group 2 keeps shape 2.
        let back = Document::load_json(&doc.save_json().unwrap()).unwrap();
        assert_eq!(back.groups.len(), 1);
        assert_eq!(back.groups[0].children, vec![GroupChild::Shape(2)]);
    }

    #[test]
    fn bad_canvas_data_is_rejected_calmly() {
        let cases: Vec<Box<dyn Fn(&mut Document)>> = vec![
            Box::new(|d| d.groups[0].children.push(GroupChild::Group(2))),
            Box::new(|d| d.groups[1].children.push(GroupChild::Shape(1))),
            Box::new(|d| d.groups[0].id = 2),
            Box::new(|d| d.pages[0].width_mm = f64::NAN),
            Box::new(|d| d.guides[0].position_mm = f64::INFINITY),
            Box::new(|d| d.grid.subdivisions = 0),
            Box::new(|d| d.layers[0].color = Some("red".to_string())),
            Box::new(|d| d.meta.notes = "x".repeat(canvas::MAX_NOTES_BYTES + 1)),
            Box::new(|d| d.snap.tolerance_mm = f64::NAN),
        ];
        for (i, corrupt) in cases.iter().enumerate() {
            let mut doc = canvas_sample();
            corrupt(&mut doc);
            assert!(doc.validate().is_err(), "case {i} validated");
            assert!(doc.save_json().is_err(), "case {i} saved");
        }
        // The same faults in a file are load errors.
        let json = canvas_sample().save_json().unwrap().replace("#112233", "#zz");
        assert!(Document::load_json(&json).is_err());
    }

    #[test]
    fn page_document_takes_the_shapes_on_that_page() {
        let mut doc = canvas_sample();
        doc.push_shape(0, rect_path(130.0, 10.0, 10.0, 10.0), CutStyle::Cut)
            .unwrap();
        let page = doc.page_document(1).unwrap();
        assert_eq!((page.board.width_mm, page.board.height_mm), (210.0, 297.0));
        assert_eq!(page.shape_count(), 1);
        let b = page.layers[0].shapes[0].bounds().unwrap();
        assert!((b.x0 - 10.0).abs() < 1e-9 && (b.y0 - 10.0).abs() < 1e-9);
        assert_eq!(doc.page_document(0).unwrap().shape_count(), 2);
        assert!(doc.page_document(5).is_none());
    }

    #[test]
    fn thumbnail_respects_its_budget() {
        let doc = canvas_sample();
        let full = doc.render_thumbnail(64 * 1024);
        assert!(full.starts_with("<svg") && full.ends_with("</svg>"));
        assert_eq!(full.matches("<path").count(), 2);
        let small = doc.render_thumbnail(full.len() - 1);
        assert!(small.len() < full.len() && small.ends_with("</svg>"));
    }
}
