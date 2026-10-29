//! The command model: every user-visible behaviour is a `Command` variant, run
//! through [`Engine::run`], reached from the UI, the CLI and the control/MCP
//! surface alike. Commands are serde-tagged so a JSON message can name one
//! (`"command": "shape.add-rect", "params": {...}`), matching the CLI's `mcp`.
//!
//! Undo/redo snapshots whole documents for now (documents are small;
//! structural sharing is on the roadmap — `docs/gaps.md` 6) — but the history
//! API is already the one the actors use, so swapping in a shared-structure
//! stack later must not touch any caller.

use kurbo::BezPath;
use serde::{Deserialize, Serialize};
use stencilcraft_doc::{CutStyle, Document};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Internal(String),
}

impl From<stencilcraft_doc::DocError> for EngineError {
    fn from(e: stencilcraft_doc::DocError) -> Self {
        EngineError::Internal(e.to_string())
    }
}

/// The command set. Every variant must carry enough data to execute AND to make
/// sense in a control-channel message: only plain, serde-serializable params.
/// Names use the dot convention the control protocol documents
/// (`"command": "shape.add-rect"`), matching VectorCraft's command API style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", content = "params")]
pub enum Command {
    /// Add a rectangle cut shape on the topmost cut layer.
    #[serde(rename = "shape.add-rect")]
    ShapeAddRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    /// Remove a shape by id.
    #[serde(rename = "shape.remove")]
    ShapeRemove { id: u64 },
    /// Translate a shape by a finite millimetre offset.
    #[serde(rename = "shape.move")]
    ShapeMove { id: u64, dx_mm: f64, dy_mm: f64 },
    /// Rotate a shape counter-clockwise around its centre, in degrees.
    #[serde(rename = "shape.rotate")]
    ShapeRotate { id: u64, degrees: f64 },
    /// Scale a shape around its centre. Factors must be positive and finite.
    #[serde(rename = "shape.scale")]
    ShapeScale { id: u64, scale_x: f64, scale_y: f64 },
    /// Add a layer with a name and a production kind.
    #[serde(rename = "layer.add")]
    LayerAdd { name: String, kind: CutStyle },
    /// Set the kerf width (millimetres; exports compensate when asked).
    #[serde(rename = "kerf.set")]
    KerfSet { width_mm: f64 },
    /// Clear the document to a fresh board.
    #[serde(rename = "document.new")]
    DocumentNew { width_mm: f64, height_mm: f64 },
}

/// The result of a command: what changed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Effect {
    ShapeAdded { id: u64 },
    ShapeRemoved { id: u64 },
    ShapeTransformed { id: u64 },
    LayerAdded,
    KerfSet { width_mm: f64 },
    DocumentReset,
}

/// Undo/redo history. Snapshots document clones; capped so a long session cannot
/// grow without bound.
#[derive(Debug)]
struct History {
    undo: Vec<Document>,
    redo: Vec<Document>,
    limit: usize,
}

impl History {
    fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit,
        }
    }

    fn push_snapshot(&mut self, snapshot: Document) {
        self.undo.push(snapshot);
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn undo(&mut self, doc: &mut Document) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(doc.clone());
                *doc = prev;
                true
            }
            None => false,
        }
    }

    fn redo(&mut self, doc: &mut Document) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(doc.clone());
                *doc = next;
                true
            }
            None => false,
        }
    }
}

/// The editing engine: a document plus its history and the command dispatcher.
#[derive(Debug)]
pub struct Engine {
    pub doc: Document,
    history: History,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(Document::default())
    }
}

impl Engine {
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            history: History::new(100),
        }
    }

    /// Runs a command, recording undo state. All command validation errors are
    /// printable messages; nothing here can panic on bad parameters.
    pub fn run(&mut self, command: Command) -> Result<Effect, EngineError> {
        let before = self.doc.clone();
        let effect = self.execute(command)?;
        self.history.push_snapshot(before);
        Ok(effect)
    }

    /// Runs a command without touching history (internal use: history is pushed
    /// before the workspace copy is mutated).
    fn execute(&mut self, command: Command) -> Result<Effect, EngineError> {
        match command {
            Command::ShapeAddRect {
                x,
                y,
                width,
                height,
            } => {
                if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
                    return Err(EngineError::Invalid(format!(
                        "shape.add-rect: {width}×{height} is not a positive finite size"
                    )));
                }
                if !(x.is_finite() && y.is_finite()) {
                    return Err(EngineError::Invalid(
                        "shape.add-rect: position must be finite".to_string(),
                    ));
                }
                let path = rect_path(x, y, width, height);
                let layer = self.top_cut_layer();
                let shape = self
                    .doc
                    .push_shape(layer, path, CutStyle::Cut)
                    .map_err(EngineError::from)?;
                Ok(Effect::ShapeAdded { id: shape.id })
            }
            Command::ShapeRemove { id } => {
                for layer in &mut self.doc.layers {
                    if let Some(pos) = layer.shapes.iter().position(|s| s.id == id) {
                        layer.shapes.remove(pos);
                        return Ok(Effect::ShapeRemoved { id });
                    }
                }
                Err(EngineError::Invalid(format!(
                    "shape.remove: no shape has id {id}"
                )))
            }
            Command::ShapeMove { id, dx_mm, dy_mm } => {
                if !(dx_mm.is_finite() && dy_mm.is_finite()) {
                    return Err(EngineError::Invalid(
                        "shape.move: offsets must be finite millimetres".to_string(),
                    ));
                }
                let transform = kurbo::Affine::translate((dx_mm, dy_mm));
                self.transform_shape(id, transform, "shape.move")
            }
            Command::ShapeRotate { id, degrees } => {
                if !degrees.is_finite() {
                    return Err(EngineError::Invalid(
                        "shape.rotate: degrees must be finite".to_string(),
                    ));
                }
                let bounds = self.shape_bounds(id, "shape.rotate")?;
                let center = bounds.center();
                let angle = degrees.to_radians();
                let transform = kurbo::Affine::translate((center.x, center.y))
                    * kurbo::Affine::rotate(angle)
                    * kurbo::Affine::translate((-center.x, -center.y));
                self.transform_shape(id, transform, "shape.rotate")
            }
            Command::ShapeScale {
                id,
                scale_x,
                scale_y,
            } => {
                if !(scale_x.is_finite() && scale_y.is_finite() && scale_x > 0.0 && scale_y > 0.0) {
                    return Err(EngineError::Invalid(
                        "shape.scale: scale factors must be positive and finite".to_string(),
                    ));
                }
                let bounds = self.shape_bounds(id, "shape.scale")?;
                let center = bounds.center();
                let transform = kurbo::Affine::translate((center.x, center.y))
                    * kurbo::Affine::scale_non_uniform(scale_x, scale_y)
                    * kurbo::Affine::translate((-center.x, -center.y));
                self.transform_shape(id, transform, "shape.scale")
            }
            Command::LayerAdd { name, kind } => {
                if name.trim().is_empty() {
                    return Err(EngineError::Invalid(
                        "layer.add: name cannot be empty".to_string(),
                    ));
                }
                self.doc
                    .layers
                    .push(stencilcraft_doc::Layer::new(name, kind));
                Ok(Effect::LayerAdded)
            }
            Command::KerfSet { width_mm } => {
                if !(width_mm.is_finite() && width_mm >= 0.0) {
                    return Err(EngineError::Invalid(format!(
                        "kerf.set: {width_mm} mm is not a non-negative finite width"
                    )));
                }
                self.doc.kerf.width_mm = width_mm;
                self.doc.kerf.apply_to_exports = width_mm > 0.0;
                Ok(Effect::KerfSet { width_mm })
            }
            Command::DocumentNew {
                width_mm,
                height_mm,
            } => {
                if !(width_mm.is_finite()
                    && height_mm.is_finite()
                    && width_mm > 0.0
                    && height_mm > 0.0)
                {
                    return Err(EngineError::Invalid(format!(
                        "document.new: {width_mm}×{height_mm} is not a positive finite board"
                    )));
                }
                self.doc = Document {
                    board: stencilcraft_doc::Board {
                        width_mm,
                        height_mm,
                    },
                    kerf: self.doc.kerf.clone(),
                    layers: vec![stencilcraft_doc::Layer::new("cut", CutStyle::Cut)],
                    next_shape_id: 1,
                };
                Ok(Effect::DocumentReset)
            }
        }
    }

    fn shape_bounds(&self, id: u64, command: &str) -> Result<kurbo::Rect, EngineError> {
        self.doc
            .get_shape(id)
            .and_then(stencilcraft_doc::Shape::bounds)
            .ok_or_else(|| EngineError::Invalid(format!("{command}: shape {id} has no bounds")))
    }

    fn transform_shape(
        &mut self,
        id: u64,
        transform: kurbo::Affine,
        command: &str,
    ) -> Result<Effect, EngineError> {
        let shape = self
            .doc
            .layers
            .iter_mut()
            .flat_map(|layer| layer.shapes.iter_mut())
            .find(|shape| shape.id == id)
            .ok_or_else(|| EngineError::Invalid(format!("{command}: no shape has id {id}")))?;
        let mut path = BezPath::new();
        for element in shape.path.elements() {
            match *element {
                kurbo::PathEl::MoveTo(point) => path.move_to(transform * point),
                kurbo::PathEl::LineTo(point) => path.line_to(transform * point),
                kurbo::PathEl::QuadTo(control, point) => {
                    path.quad_to(transform * control, transform * point)
                }
                kurbo::PathEl::CurveTo(control1, control2, point) => path.curve_to(
                    transform * control1,
                    transform * control2,
                    transform * point,
                ),
                kurbo::PathEl::ClosePath => path.close_path(),
            }
        }
        if !path.elements().iter().all(path_element_is_finite) {
            return Err(EngineError::Invalid(format!(
                "{command}: transformed geometry exceeds finite coordinate range"
            )));
        }
        shape.path = path;
        Ok(Effect::ShapeTransformed { id })
    }

    fn top_cut_layer(&self) -> usize {
        self.doc
            .layers
            .iter()
            .rposition(|l| l.kind == CutStyle::Cut)
            .unwrap_or(0)
    }

    /// Undo the last command; `false` when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        self.history.undo(&mut self.doc)
    }

    /// Redo the last undone command; `false` when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        self.history.redo(&mut self.doc)
    }
}

fn path_element_is_finite(element: &kurbo::PathEl) -> bool {
    let point_is_finite = |point: kurbo::Point| point.x.is_finite() && point.y.is_finite();
    match *element {
        kurbo::PathEl::MoveTo(point) | kurbo::PathEl::LineTo(point) => point_is_finite(point),
        kurbo::PathEl::QuadTo(control, point) => point_is_finite(control) && point_is_finite(point),
        kurbo::PathEl::CurveTo(control1, control2, point) => {
            point_is_finite(control1) && point_is_finite(control2) && point_is_finite(point)
        }
        kurbo::PathEl::ClosePath => true,
    }
}

fn rect_path(x: f64, y: f64, w: f64, h: f64) -> BezPath {
    use stencilcraft_geom::Point;
    let mut p = BezPath::new();
    p.move_to(Point::new(x, y));
    p.line_to(Point::new(x + w, y));
    p.line_to(Point::new(x + w, y + h));
    p.line_to(Point::new(x, y + h));
    p.close_path();
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_remove_roundtrip() {
        let mut engine = Engine::default();
        let effect = engine
            .run(Command::ShapeAddRect {
                x: 5.0,
                y: 5.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        assert_eq!(effect, Effect::ShapeAdded { id: 1 });
        assert_eq!(engine.doc.shape_count(), 1);

        engine.run(Command::ShapeRemove { id: 1 }).unwrap();
        assert_eq!(engine.doc.shape_count(), 0);
    }

    #[test]
    fn move_rotate_scale_preserve_path_geometry_and_undo() {
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 20.0,
            })
            .unwrap();
        engine
            .run(Command::ShapeMove {
                id: 1,
                dx_mm: 5.0,
                dy_mm: -3.0,
            })
            .unwrap();
        let moved = engine.doc.get_shape(1).unwrap().bounds().unwrap();
        assert_eq!(moved, kurbo::Rect::new(5.0, -3.0, 15.0, 17.0));

        engine
            .run(Command::ShapeRotate {
                id: 1,
                degrees: 90.0,
            })
            .unwrap();
        let rotated = engine.doc.get_shape(1).unwrap().bounds().unwrap();
        assert!((rotated.width() - 20.0).abs() < 1e-9);
        assert!((rotated.height() - 10.0).abs() < 1e-9);
        assert!((rotated.center().x - moved.center().x).abs() < 1e-9);
        assert!((rotated.center().y - moved.center().y).abs() < 1e-9);

        engine
            .run(Command::ShapeScale {
                id: 1,
                scale_x: 2.0,
                scale_y: 0.5,
            })
            .unwrap();
        let scaled = engine.doc.get_shape(1).unwrap().bounds().unwrap();
        assert!((scaled.width() - 40.0).abs() < 1e-9);
        assert!((scaled.height() - 5.0).abs() < 1e-9);
        assert!((scaled.center().x - rotated.center().x).abs() < 1e-9);
        assert!((scaled.center().y - rotated.center().y).abs() < 1e-9);
        assert!(engine.undo());
        assert_eq!(engine.doc.get_shape(1).unwrap().bounds(), Some(rotated));
    }

    #[test]
    fn shape_transforms_reject_invalid_parameters_without_changing_geometry() {
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        let original = engine.doc.get_shape(1).unwrap().path.clone();
        for command in [
            Command::ShapeMove {
                id: 1,
                dx_mm: f64::INFINITY,
                dy_mm: 0.0,
            },
            Command::ShapeRotate {
                id: 1,
                degrees: f64::NAN,
            },
            Command::ShapeScale {
                id: 1,
                scale_x: 0.0,
                scale_y: 1.0,
            },
        ] {
            assert!(engine.run(command).is_err());
            assert_eq!(engine.doc.get_shape(1).unwrap().path, original);
        }
        assert!(
            engine
                .run(Command::ShapeMove {
                    id: 99,
                    dx_mm: 1.0,
                    dy_mm: 1.0,
                })
                .is_err()
        );
    }

    #[test]
    fn undo_redo() {
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddRect {
                x: 5.0,
                y: 5.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 0);
        assert!(engine.redo());
        assert_eq!(engine.doc.shape_count(), 1);
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 0);
        assert!(!engine.undo(), "nothing more to undo");
    }

    #[test]
    fn bad_params_are_errors_not_panics() {
        let mut engine = Engine::default();
        assert!(
            engine
                .run(Command::ShapeAddRect {
                    x: 0.0,
                    y: 0.0,
                    width: -3.0,
                    height: 2.0
                })
                .is_err()
        );
        assert!(
            engine
                .run(Command::ShapeAddRect {
                    x: f64::NAN,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0
                })
                .is_err()
        );
        assert!(engine.run(Command::ShapeRemove { id: 42 }).is_err());
        assert!(engine.run(Command::KerfSet { width_mm: -1.0 }).is_err());
        assert!(
            engine
                .run(Command::DocumentNew {
                    width_mm: 0.0,
                    height_mm: 100.0
                })
                .is_err()
        );
        assert!(matches!(
            engine.run(Command::LayerAdd {
                name: "  ".to_string(),
                kind: CutStyle::Cut
            }),
            Err(EngineError::Invalid(_))
        ));
    }

    #[test]
    fn commands_roundtrip_json() {
        let command = Command::ShapeAddRect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        };
        let json = serde_json::to_string(&command).unwrap();
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(command, back);
        assert!(json.contains("\"command\""), "tagged: {json}");
    }

    #[test]
    fn kerf_set_flows_into_doc() {
        let mut engine = Engine::default();
        engine.run(Command::KerfSet { width_mm: 0.25 }).unwrap();
        assert_eq!(engine.doc.kerf.width_mm, 0.25);
        assert!(engine.doc.kerf.apply_to_exports);
    }
}
