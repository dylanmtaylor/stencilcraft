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

    fn push(&mut self, doc: &Document) {
        self.undo.push(doc.clone());
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
        let effect = self.execute(command)?;
        Ok(effect)
    }

    /// Runs a command without touching history (internal use: history is pushed
    /// before the workspace copy is mutated).
    fn execute(&mut self, command: Command) -> Result<Effect, EngineError> {
        self.history.push(&self.doc);
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
