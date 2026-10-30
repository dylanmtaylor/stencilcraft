//! The command model: every user-visible behaviour is a `Command` variant, run
//! through [`Engine::run`], reached from the UI, the CLI and the control/MCP
//! surface alike. Commands are serde-tagged so a JSON message can name one
//! (`"command": "shape.add-rect", "params": {...}`), matching the CLI's `mcp`.
//!
//! Undo/redo snapshots whole documents for now (documents are small;
//! structural sharing is on the roadmap — `docs/gaps.md` 6) — but the history
//! API is already the one the actors use, so swapping in a shared-structure
//! stack later must not touch any caller.

use std::collections::BTreeMap;
use std::sync::Arc;

use kurbo::BezPath;
use serde::{Deserialize, Serialize};
use serde_json::json;
use stencilcraft_doc::{CutStyle, Document};
use vectorcraft_engine::Session as VectorCraftSession;
use vectorcraft_engine::doc::color::{Color, Paint};
use vectorcraft_engine::doc::geom::PathData;
use vectorcraft_engine::doc::{Appearance, AppearanceItem, FillLayer, Node, NodeId, NodeKind};

const POINTS_PER_MM: f64 = 72.0 / 25.4;

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
/// (`"command": "shape.add-rect"`).
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
    /// Add a VectorCraft ellipse inside the given bounds, in millimetres.
    #[serde(rename = "shape.add-ellipse")]
    ShapeAddEllipse {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    /// Add a VectorCraft rounded rectangle, with a uniform corner radius in millimetres.
    #[serde(rename = "shape.add-rounded-rect")]
    ShapeAddRoundedRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: f64,
    },
    /// Add a regular VectorCraft polygon centred at the given point, in millimetres.
    #[serde(rename = "shape.add-polygon")]
    ShapeAddPolygon {
        cx: f64,
        cy: f64,
        radius: f64,
        sides: u32,
        rotation: f64,
    },
    /// Add a VectorCraft star centred at the given point, in millimetres.
    #[serde(rename = "shape.add-star")]
    ShapeAddStar {
        cx: f64,
        cy: f64,
        radius: f64,
        inner_radius: f64,
        points: u32,
        rotation: f64,
    },
    /// Undo the latest command through VectorCraft's journal and restore StencilCraft metadata.
    #[serde(rename = "edit.undo")]
    Undo,
    /// Redo the latest undone command through VectorCraft's journal and restore StencilCraft metadata.
    #[serde(rename = "edit.redo")]
    Redo,
    /// Remove a shape by id.
    #[serde(rename = "shape.remove")]
    ShapeRemove { id: u64 },
    /// Remove multiple shapes as one undoable document edit.
    #[serde(rename = "shapes.remove")]
    ShapesRemove { ids: Vec<u64> },
    /// Duplicate one or more shapes at their exact positions as one undoable edit.
    #[serde(rename = "shapes.duplicate")]
    ShapesDuplicate { ids: Vec<u64> },
    /// Translate a shape by a finite millimetre offset.
    #[serde(rename = "shape.move")]
    ShapeMove { id: u64, dx_mm: f64, dy_mm: f64 },
    /// Rotate a shape counter-clockwise around its centre, in degrees.
    #[serde(rename = "shape.rotate")]
    ShapeRotate { id: u64, degrees: f64 },
    /// Scale a shape around its centre. Factors must be positive and finite.
    #[serde(rename = "shape.scale")]
    ShapeScale { id: u64, scale_x: f64, scale_y: f64 },
    /// Set a shape's axis-aligned bounds in millimetres.
    #[serde(rename = "shape.set-bounds")]
    ShapeSetBounds {
        id: u64,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    /// Add an imported vector path to the matching production layer.
    #[serde(rename = "shape.add-path")]
    ShapeAddPath { path: BezPath, style: CutStyle },
    /// Replace a shape's vector path in-place (e.g. from interactive node editing).
    #[serde(rename = "shape.set-path")]
    ShapeSetPath { id: u64, path: BezPath },
    /// Change the production style (Cut / Engrave) of one or more shapes.
    #[serde(rename = "shapes.set-style")]
    ShapesSetStyle { ids: Vec<u64>, style: CutStyle },
    /// Add imported vector paths together as one undoable document edit.
    #[serde(rename = "shape.add-paths")]
    ShapeAddPaths { shapes: Vec<(BezPath, CutStyle)> },
    /// Apply a curve-preserving Pathfinder operation to two filled shapes.
    #[serde(rename = "shape.boolean")]
    ShapeBoolean {
        a: u64,
        b: u64,
        operation: BooleanOperation,
    },
    /// Apply a VectorCraft Pathfinder operation to selected shapes in document order.
    #[serde(rename = "shape.pathfinder")]
    ShapePathfinder {
        ids: Vec<u64>,
        operation: BooleanOperation,
    },
    /// Transform multiple shapes together about their combined bounds center.
    #[serde(rename = "shapes.transform")]
    ShapesTransform {
        ids: Vec<u64>,
        dx_mm: f64,
        dy_mm: f64,
        scale_x: f64,
        scale_y: f64,
        degrees: f64,
    },
    /// Align multiple shapes by their overall bounds or to the board edge/center.
    #[serde(rename = "shapes.align")]
    ShapesAlign {
        ids: Vec<u64>,
        alignment: Alignment,
        to_board: bool,
    },
    /// Evenly distribute the selected shapes along one axis.
    #[serde(rename = "shapes.distribute")]
    ShapesDistribute {
        ids: Vec<u64>,
        axis: DistributionAxis,
    },
    /// Add a layer with a name and a production kind.
    #[serde(rename = "layer.add")]
    LayerAdd { name: String, kind: CutStyle },
    /// Set visibility of a layer by index.
    #[serde(rename = "layer.set-visible")]
    LayerSetVisible { index: usize, visible: bool },
    /// Remove a layer by index (at least one layer must remain).
    #[serde(rename = "layer.remove")]
    LayerRemove { index: usize },
    /// Set the kerf width (millimetres; exports compensate when asked).
    #[serde(rename = "kerf.set")]
    KerfSet { width_mm: f64 },
    /// Resize the document board without clearing existing layers or shapes.
    #[serde(rename = "document.set-board")]
    DocumentSetBoard { width_mm: f64, height_mm: f64 },
    /// Clear the document to a fresh board.
    #[serde(rename = "document.new")]
    DocumentNew { width_mm: f64, height_mm: f64 },
}

/// VectorCraft Pathfinder operations supported for StencilCraft's filled cut paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BooleanOperation {
    Union,
    Difference,
    Intersect,
    Xor,
    Divide,
    Trim,
    Merge,
    Crop,
    Outline,
    MinusBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Alignment {
    Left,
    HorizontalCenter,
    Right,
    Top,
    VerticalCenter,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistributionAxis {
    Horizontal,
    Vertical,
}

/// The result of a command: what changed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Effect {
    ShapeAdded { id: u64 },
    ShapesAdded { ids: Vec<u64> },
    ShapeRemoved { id: u64 },
    ShapesRemoved { ids: Vec<u64> },
    ShapeTransformed { id: u64 },
    ShapesStyled { ids: Vec<u64> },
    HistoryChanged,
    LayerAdded,
    LayerVisibilityChanged { index: usize, visible: bool },
    LayerRemoved { index: usize },
    KerfSet { width_mm: f64 },
    BoardChanged { width_mm: f64, height_mm: f64 },
    DocumentReset,
}

/// Undo/redo snapshots StencilCraft's production document; VectorCraft still owns the reusable
/// editing operations and its own copy-on-write history for each adapted operation.
#[derive(Debug)]
struct History {
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    limit: usize,
}

#[derive(Debug, Clone)]
struct MappingSnapshot {
    shape_nodes: BTreeMap<u64, u64>,
    node_shapes: BTreeMap<u64, u64>,
    node_layers: BTreeMap<u64, usize>,
    layer_nodes: Vec<NodeId>,
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    snapshot: Document,
    mappings: MappingSnapshot,
    vectorcraft: bool,
    rebuild_session: bool,
}

impl History {
    fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit,
        }
    }

    fn push_snapshot(&mut self, entry: HistoryEntry) {
        self.undo.push(entry);
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn peek_undo(&self) -> Option<&HistoryEntry> {
        self.undo.last()
    }

    fn peek_redo(&self) -> Option<&HistoryEntry> {
        self.redo.last()
    }

    fn undo(&mut self, current: HistoryEntry) -> Option<HistoryEntry> {
        let previous = self.undo.pop()?;
        self.redo.push(HistoryEntry {
            vectorcraft: previous.vectorcraft,
            rebuild_session: previous.rebuild_session,
            ..current
        });
        Some(previous)
    }

    fn redo(&mut self, current: HistoryEntry) -> Option<HistoryEntry> {
        let next = self.redo.pop()?;
        self.undo.push(HistoryEntry {
            vectorcraft: next.vectorcraft,
            rebuild_session: next.rebuild_session,
            ..current
        });
        Some(next)
    }

    fn rebase_to_snapshots(&mut self) {
        for entry in self.undo.iter_mut().chain(&mut self.redo) {
            entry.vectorcraft = false;
            entry.rebuild_session = true;
        }
    }
}

/// VectorCraft supplies the actual editing session and reusable geometry commands. StencilCraft
/// keeps its production document as the stable public model and adapts operations at this boundary.
pub struct Engine {
    pub doc: Document,
    history: History,
    session: VectorCraftSession,
    shape_nodes: BTreeMap<u64, u64>,
    node_shapes: BTreeMap<u64, u64>,
    node_layers: BTreeMap<u64, usize>,
    layer_nodes: Vec<NodeId>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(Document::default())
    }
}

impl Engine {
    pub fn new(doc: Document) -> Self {
        let mut engine = Self {
            doc,
            history: History::new(100),
            session: VectorCraftSession::new(),
            shape_nodes: BTreeMap::new(),
            node_shapes: BTreeMap::new(),
            node_layers: BTreeMap::new(),
            layer_nodes: Vec::new(),
        };
        engine.sync_session_from_doc();
        engine
    }

    /// Runs a StencilCraft command through the VectorCraft session where its document semantics
    /// fit. StencilCraft-specific metadata stays in `doc`, while VectorCraft owns IDs for its
    /// richer transient tree and records all editable-tree operations in its undo journal.
    pub fn run(&mut self, command: Command) -> Result<Effect, EngineError> {
        match command {
            Command::Undo => {
                self.undo();
                return Ok(Effect::HistoryChanged);
            }
            Command::Redo => {
                self.redo();
                return Ok(Effect::HistoryChanged);
            }
            _ => {}
        }

        let rebuild_session = matches!(
            command,
            Command::DocumentNew { .. }
                | Command::DocumentSetBoard { .. }
                | Command::ShapeSetPath { .. }
                | Command::ShapesSetStyle { .. }
                | Command::LayerSetVisible { .. }
                | Command::LayerRemove { .. }
        );
        let before = self.capture_history_entry(false, false);
        let document_before = self.session.active().map(|state| Arc::clone(&state.doc));
        if !rebuild_session {
            self.session
                .execute(
                    "edit.undoGroup.begin",
                    &json!({"label": "StencilCraft command"}),
                )
                .map_err(|error| {
                    EngineError::Internal(format!("VectorCraft undo group: {error}"))
                })?;
        }
        match self.execute(command) {
            Ok(effect) => {
                if !rebuild_session {
                    self.session
                        .execute("edit.undoGroup.end", &json!({}))
                        .map_err(|error| {
                            EngineError::Internal(format!("VectorCraft undo group: {error}"))
                        })?;
                }
                if rebuild_session {
                    self.sync_session_from_doc();
                    self.history.rebase_to_snapshots();
                }
                let vectorcraft = !rebuild_session
                    && document_before.as_ref().is_some_and(|before| {
                        self.session
                            .active()
                            .is_some_and(|state| !Arc::ptr_eq(before, &state.doc))
                    });
                self.history.push_snapshot(HistoryEntry {
                    vectorcraft,
                    rebuild_session,
                    ..before
                });
                Ok(effect)
            }
            Err(error) => {
                if !rebuild_session {
                    let canceled = self
                        .session
                        .execute("edit.undoGroup.end", &json!({"cancel": true}))
                        .is_ok();
                    if !canceled {
                        self.doc = before.snapshot.clone();
                        self.restore_mappings(before.mappings);
                        self.sync_session_from_doc();
                        self.history.rebase_to_snapshots();
                        return Err(EngineError::Internal(format!(
                            "{error}; VectorCraft could not cancel the failed command"
                        )));
                    }
                }
                self.doc = before.snapshot;
                self.restore_mappings(before.mappings);
                if rebuild_session {
                    self.sync_session_from_doc();
                    self.history.rebase_to_snapshots();
                }
                Err(error)
            }
        }
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
                let ids = self.vector_add_paths(vec![(path, CutStyle::Cut)], "shape.add-rect")?;
                Ok(Effect::ShapeAdded { id: ids[0] })
            }
            Command::ShapeAddEllipse {
                x,
                y,
                width,
                height,
            } => {
                validate_shape_bounds(x, y, width, height, "shape.add-ellipse")?;
                let params = json!({"x": x * POINTS_PER_MM, "y": y * POINTS_PER_MM, "width": width * POINTS_PER_MM, "height": height * POINTS_PER_MM});
                self.vector_add_primitive(
                    "shape.ellipse",
                    &params,
                    CutStyle::Cut,
                    "shape.add-ellipse",
                )
            }
            Command::ShapeAddRoundedRect {
                x,
                y,
                width,
                height,
                radius,
            } => {
                validate_shape_bounds(x, y, width, height, "shape.add-rounded-rect")?;
                if !(radius.is_finite() && radius >= 0.0) {
                    return Err(EngineError::Invalid(
                        "shape.add-rounded-rect: radius must be non-negative and finite"
                            .to_string(),
                    ));
                }
                let params = json!({"x": x * POINTS_PER_MM, "y": y * POINTS_PER_MM, "width": width * POINTS_PER_MM, "height": height * POINTS_PER_MM, "radius": radius * POINTS_PER_MM});
                self.vector_add_primitive(
                    "shape.rectangle",
                    &params,
                    CutStyle::Cut,
                    "shape.add-rounded-rect",
                )
            }
            Command::ShapeAddPolygon {
                cx,
                cy,
                radius,
                sides,
                rotation,
            } => {
                validate_radial_primitive(cx, cy, radius, rotation, sides, 3, "shape.add-polygon")?;
                let params = json!({"cx": cx * POINTS_PER_MM, "cy": cy * POINTS_PER_MM, "radius": radius * POINTS_PER_MM, "sides": sides, "rotation": rotation});
                self.vector_add_primitive(
                    "shape.polygon",
                    &params,
                    CutStyle::Cut,
                    "shape.add-polygon",
                )
            }
            Command::ShapeAddStar {
                cx,
                cy,
                radius,
                inner_radius,
                points,
                rotation,
            } => {
                validate_radial_primitive(cx, cy, radius, rotation, points, 2, "shape.add-star")?;
                if !(inner_radius.is_finite() && inner_radius >= 0.0 && inner_radius <= radius) {
                    return Err(EngineError::Invalid("shape.add-star: inner radius must be finite and between zero and the outer radius".to_string()));
                }
                let outer = radius * POINTS_PER_MM;
                let inner = inner_radius * POINTS_PER_MM;
                let params = json!({"cx": cx * POINTS_PER_MM, "cy": cy * POINTS_PER_MM, "radius1": outer, "radius2": inner, "points": points, "rotation": rotation});
                self.vector_add_primitive("shape.star", &params, CutStyle::Cut, "shape.add-star")
            }
            Command::Undo => {
                self.undo();
                Ok(Effect::HistoryChanged)
            }
            Command::Redo => {
                self.redo();
                Ok(Effect::HistoryChanged)
            }
            Command::ShapeRemove { id } => self.remove_shapes(&[id], "shape.remove"),
            Command::ShapesRemove { ids } => self.remove_shapes(&ids, "shapes.remove"),
            Command::ShapesDuplicate { ids } => self.duplicate_shapes(&ids),
            Command::ShapeMove { id, dx_mm, dy_mm } => {
                if !(dx_mm.is_finite() && dy_mm.is_finite()) {
                    return Err(EngineError::Invalid(
                        "shape.move: offsets must be finite millimetres".to_string(),
                    ));
                }
                let transform = kurbo::Affine::translate((dx_mm, dy_mm));
                self.vector_transform(&[id], transform, "shape.move")
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
                self.vector_transform(&[id], transform, "shape.rotate")
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
                self.vector_transform(&[id], transform, "shape.scale")
            }
            Command::ShapeSetBounds {
                id,
                x,
                y,
                width,
                height,
            } => {
                if !(x.is_finite()
                    && y.is_finite()
                    && width.is_finite()
                    && height.is_finite()
                    && width > 0.0
                    && height > 0.0
                    && (x + width).is_finite()
                    && (y + height).is_finite())
                {
                    return Err(EngineError::Invalid(
                        "shape.set-bounds: position must be finite and size positive and finite"
                            .to_string(),
                    ));
                }
                let bounds = self.shape_bounds(id, "shape.set-bounds")?;
                if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
                    return Err(EngineError::Invalid(
                        "shape.set-bounds: cannot resize geometry with a zero-size bound"
                            .to_string(),
                    ));
                }
                let transform = kurbo::Affine::translate((x, y))
                    * kurbo::Affine::scale_non_uniform(
                        width / bounds.width(),
                        height / bounds.height(),
                    )
                    * kurbo::Affine::translate((-bounds.x0, -bounds.y0));
                self.vector_transform(&[id], transform, "shape.set-bounds")
            }
            Command::ShapeAddPath { path, style } => {
                if path.elements().is_empty() || !path.elements().iter().all(path_element_is_finite)
                {
                    return Err(EngineError::Invalid(
                        "shape.add-path: path must contain finite geometry".to_string(),
                    ));
                }
                let ids = self.vector_add_paths(vec![(path, style)], "shape.add-path")?;
                Ok(Effect::ShapeAdded { id: ids[0] })
            }
            Command::ShapeSetPath { id, path } => {
                if path.elements().is_empty() || !path.elements().iter().all(path_element_is_finite)
                {
                    return Err(EngineError::Invalid(
                        "shape.set-path: path must contain finite geometry".to_string(),
                    ));
                }
                let Some(shape) = self.doc.get_shape_mut(id) else {
                    return Err(EngineError::Invalid(format!(
                        "shape.set-path: no shape has id {id}"
                    )));
                };
                shape.path = path;
                Ok(Effect::ShapeTransformed { id })
            }
            Command::ShapesSetStyle { ids, style } => {
                if ids.is_empty()
                    || std::collections::BTreeSet::from_iter(ids.iter().copied()).len() != ids.len()
                {
                    return Err(EngineError::Invalid(
                        "shapes.set-style: expected one or more unique shape ids".to_string(),
                    ));
                }
                for id in &ids {
                    if self.doc.get_shape(*id).is_none() {
                        return Err(EngineError::Invalid(format!(
                            "shapes.set-style: no shape has id {id}"
                        )));
                    }
                }
                let target_layer = self
                    .doc
                    .layers
                    .iter()
                    .rposition(|l| l.kind == style)
                    .unwrap_or_else(|| {
                        let name = match style {
                            CutStyle::Cut => "cut",
                            CutStyle::Engrave => "engrave",
                        };
                        self.doc
                            .layers
                            .push(stencilcraft_doc::Layer::new(name, style));
                        self.doc.layers.len() - 1
                    });
                let requested: std::collections::BTreeSet<u64> = ids.iter().copied().collect();
                let mut moved = Vec::new();
                for (li, layer) in self.doc.layers.iter_mut().enumerate() {
                    if li == target_layer || layer.kind == style {
                        for shape in &mut layer.shapes {
                            if requested.contains(&shape.id) {
                                shape.style = style;
                            }
                        }
                    } else {
                        let mut kept = Vec::with_capacity(layer.shapes.len());
                        for mut shape in layer.shapes.drain(..) {
                            if requested.contains(&shape.id) {
                                shape.style = style;
                                moved.push(shape);
                            } else {
                                kept.push(shape);
                            }
                        }
                        layer.shapes = kept;
                    }
                }
                if let Some(layer) = self.doc.layers.get_mut(target_layer) {
                    layer.shapes.extend(moved);
                }
                Ok(Effect::ShapesStyled { ids })
            }
            Command::ShapeAddPaths { shapes } => {
                if shapes.is_empty()
                    || shapes.iter().any(|(path, _)| {
                        path.elements().is_empty()
                            || !path.elements().iter().all(path_element_is_finite)
                    })
                {
                    return Err(EngineError::Invalid(
                        "shape.add-paths: expected non-empty paths with finite geometry"
                            .to_string(),
                    ));
                }
                let ids = self.vector_add_paths(shapes, "shape.add-paths")?;
                Ok(Effect::ShapesAdded { ids })
            }
            Command::ShapeBoolean { a, b, operation } => {
                if !matches!(
                    operation,
                    BooleanOperation::Union
                        | BooleanOperation::Difference
                        | BooleanOperation::Intersect
                        | BooleanOperation::Xor
                ) {
                    return Err(EngineError::Invalid(
                        "shape.boolean: operation must be union, difference, intersect, or xor"
                            .to_string(),
                    ));
                }
                if a == b {
                    return Err(EngineError::Invalid(
                        "shape.boolean: select two different shapes".to_string(),
                    ));
                }
                self.apply_pathfinder(&[a, b], operation, "shape.boolean")
            }
            Command::ShapePathfinder { ids, operation } => {
                let requested: std::collections::BTreeSet<u64> = ids.iter().copied().collect();
                if requested.len() != ids.len() {
                    return Err(EngineError::Invalid(
                        "shape.pathfinder: shape ids must be unique".to_string(),
                    ));
                }
                let ordered_ids: Vec<u64> = self
                    .doc
                    .shapes()
                    .filter_map(|(_, shape)| requested.contains(&shape.id).then_some(shape.id))
                    .collect();
                if ordered_ids.len() != ids.len() {
                    let missing = ids
                        .iter()
                        .find(|id| self.doc.get_shape(**id).is_none())
                        .copied()
                        .unwrap_or_default();
                    return Err(EngineError::Invalid(format!(
                        "shape.pathfinder: no shape has id {missing}"
                    )));
                }
                self.apply_pathfinder(&ordered_ids, operation, "shape.pathfinder")
            }
            Command::ShapesTransform {
                ids,
                dx_mm,
                dy_mm,
                scale_x,
                scale_y,
                degrees,
            } => {
                if ids.is_empty()
                    || std::collections::BTreeSet::from_iter(ids.iter().copied()).len() != ids.len()
                    || !dx_mm.is_finite()
                    || !dy_mm.is_finite()
                    || !scale_x.is_finite()
                    || !scale_y.is_finite()
                    || scale_x <= 0.0
                    || scale_y <= 0.0
                    || !degrees.is_finite()
                {
                    return Err(EngineError::Invalid("shapes.transform: parameters must be finite, with positive scales and at least one id".to_string()));
                }
                let bounds = ids
                    .iter()
                    .try_fold(None, |acc, id| {
                        let bounds = self.shape_bounds(*id, "shapes.transform")?;
                        Ok::<_, EngineError>(Some(
                            acc.map_or(bounds, |previous: kurbo::Rect| previous.union(bounds)),
                        ))
                    })?
                    .ok_or_else(|| {
                        EngineError::Invalid("shapes.transform: no shapes selected".to_string())
                    })?;
                let center = bounds.center();
                let transform = kurbo::Affine::translate((center.x + dx_mm, center.y + dy_mm))
                    * kurbo::Affine::rotate(degrees.to_radians())
                    * kurbo::Affine::scale_non_uniform(scale_x, scale_y)
                    * kurbo::Affine::translate((-center.x, -center.y));
                self.vector_transform(&ids, transform, "shapes.transform")
            }
            Command::ShapesAlign {
                ids,
                alignment,
                to_board,
            } => {
                if ids.len() < 2
                    || std::collections::BTreeSet::from_iter(ids.iter().copied()).len() != ids.len()
                {
                    return Err(EngineError::Invalid(
                        "shapes.align: select at least two unique shapes".to_string(),
                    ));
                }
                self.vector_align(&ids, alignment, to_board)
            }
            Command::ShapesDistribute { ids, axis } => {
                if ids.len() < 3
                    || std::collections::BTreeSet::from_iter(ids.iter().copied()).len() != ids.len()
                {
                    return Err(EngineError::Invalid(
                        "shapes.distribute: select at least three unique shapes".to_string(),
                    ));
                }
                self.vector_distribute(&ids, axis)
            }
            Command::LayerAdd { name, kind } => {
                if name.trim().is_empty() {
                    return Err(EngineError::Invalid(
                        "layer.add: name cannot be empty".to_string(),
                    ));
                }
                self.add_vector_layer(&name, kind, "layer.add")?;
                Ok(Effect::LayerAdded)
            }
            Command::LayerSetVisible { index, visible } => {
                let Some(layer) = self.doc.layers.get_mut(index) else {
                    return Err(EngineError::Invalid(format!(
                        "layer.set-visible: no layer at index {index}"
                    )));
                };
                layer.visible = visible;
                Ok(Effect::LayerVisibilityChanged { index, visible })
            }
            Command::LayerRemove { index } => {
                if index >= self.doc.layers.len() {
                    return Err(EngineError::Invalid(format!(
                        "layer.remove: no layer at index {index}"
                    )));
                }
                if self.doc.layers.len() <= 1 {
                    return Err(EngineError::Invalid(
                        "layer.remove: document must keep at least one layer".to_string(),
                    ));
                }
                self.doc.layers.remove(index);
                Ok(Effect::LayerRemoved { index })
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
            Command::DocumentSetBoard {
                width_mm,
                height_mm,
            } => {
                if !(width_mm.is_finite()
                    && height_mm.is_finite()
                    && width_mm > 0.0
                    && height_mm > 0.0)
                {
                    return Err(EngineError::Invalid(format!(
                        "document.set-board: {width_mm}×{height_mm} is not a positive finite board"
                    )));
                }
                self.doc.board.width_mm = width_mm;
                self.doc.board.height_mm = height_mm;
                Ok(Effect::BoardChanged {
                    width_mm,
                    height_mm,
                })
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

    fn apply_pathfinder(
        &mut self,
        ids: &[u64],
        operation: BooleanOperation,
        command: &str,
    ) -> Result<Effect, EngineError> {
        if ids.len() < 2 {
            return Err(EngineError::Invalid(format!(
                "{command}: select at least two shapes"
            )));
        }
        let unique_ids: std::collections::BTreeSet<u64> = ids.iter().copied().collect();
        if unique_ids.len() != ids.len() {
            return Err(EngineError::Invalid(format!(
                "{command}: shape ids must be unique"
            )));
        }
        // `ids` is a back-to-front stack. `shape.pathfinder` normalizes to document
        // paint order; the two-object `shape.boolean` API preserves explicit a/b order.
        let inputs: Vec<_> = ids
            .iter()
            .filter_map(|id| self.doc.get_shape(*id).cloned())
            .collect();
        if inputs.len() != ids.len() {
            let missing = ids
                .iter()
                .find(|id| self.doc.get_shape(**id).is_none())
                .copied()
                .unwrap_or_default();
            return Err(EngineError::Invalid(format!(
                "{command}: no shape has id {missing}"
            )));
        }

        for shape in &inputs {
            if shape.path.elements().is_empty()
                || !shape.path.elements().iter().all(path_element_is_finite)
            {
                return Err(EngineError::Invalid(format!(
                    "{command}: selected paths must contain finite geometry"
                )));
            }
        }
        let vc_ids = ids
            .iter()
            .map(|id| self.node_for_shape(*id, command))
            .collect::<Result<Vec<_>, _>>()?;
        let command_id = match operation {
            BooleanOperation::Union => "object.pathfinder.unite",
            BooleanOperation::Difference if command == "shape.boolean" => {
                let paint_order: BTreeMap<u64, usize> = self
                    .doc
                    .shapes()
                    .enumerate()
                    .map(|(index, (_, shape))| (shape.id, index))
                    .collect();
                let a_is_front = ids.first().zip(ids.get(1)).is_some_and(|(a, b)| {
                    paint_order
                        .get(a)
                        .zip(paint_order.get(b))
                        .is_some_and(|(a, b)| a > b)
                });
                if a_is_front {
                    "object.pathfinder.minusBack"
                } else {
                    "object.pathfinder.minusFront"
                }
            }
            BooleanOperation::Difference => "object.pathfinder.minusFront",
            BooleanOperation::Intersect => "object.pathfinder.intersect",
            BooleanOperation::Xor => "object.pathfinder.exclude",
            BooleanOperation::Divide => "object.pathfinder.divide",
            BooleanOperation::Trim => "object.pathfinder.trim",
            BooleanOperation::Merge => "object.pathfinder.merge",
            BooleanOperation::Crop => "object.pathfinder.crop",
            BooleanOperation::Outline => "object.pathfinder.outline",
            BooleanOperation::MinusBack => "object.pathfinder.minusBack",
        };
        self.select_nodes(&vc_ids)?;
        let first_new_shape_id = self.doc.next_shape_id;
        if let Err(error) = self.session.execute(command_id, &json!({"ids": vc_ids})) {
            // VectorCraft treats a mathematically empty boolean result as an error. For a cut
            // document, the corresponding result is simply no output shapes, so remove the inputs
            // in the same session rather than leaving the source shapes behind.
            if error.to_string().contains("produced an empty result") {
                let node_ids = vc_ids.iter().map(|id| id.0).collect::<Vec<_>>();
                self.session
                    .execute("edit.clear", &json!({"ids": node_ids}))
                    .map_err(|clear_error| {
                        EngineError::Invalid(format!("{command}: {clear_error}"))
                    })?;
            } else {
                return Err(EngineError::Invalid(format!("{command}: {error}")));
            }
        }
        self.sync_doc_from_session()?;
        let result_ids = self
            .doc
            .shapes()
            .filter_map(|(_, shape)| {
                (shape.id >= first_new_shape_id && !unique_ids.contains(&shape.id))
                    .then_some(shape.id)
            })
            .collect();
        Ok(Effect::ShapesAdded { ids: result_ids })
    }

    fn duplicate_shapes(&mut self, ids: &[u64]) -> Result<Effect, EngineError> {
        if ids.is_empty()
            || std::collections::BTreeSet::from_iter(ids.iter().copied()).len() != ids.len()
        {
            return Err(EngineError::Invalid(
                "shapes.duplicate: expected one or more unique shape ids".to_string(),
            ));
        }
        let requested: std::collections::BTreeSet<u64> = ids.iter().copied().collect();
        let shapes = self
            .doc
            .shapes()
            .filter(|(_, shape)| requested.contains(&shape.id))
            .map(|(_, shape)| (shape.path.clone(), shape.style))
            .collect::<Vec<_>>();
        if shapes.len() != ids.len() {
            let missing = ids
                .iter()
                .find(|id| self.doc.get_shape(**id).is_none())
                .copied()
                .unwrap_or_default();
            return Err(EngineError::Invalid(format!(
                "shapes.duplicate: no shape has id {missing}"
            )));
        }
        let new_ids = self.vector_add_paths(shapes, "shapes.duplicate")?;
        Ok(Effect::ShapesAdded { ids: new_ids })
    }

    fn remove_shapes(&mut self, ids: &[u64], command: &str) -> Result<Effect, EngineError> {
        if ids.is_empty()
            || std::collections::BTreeSet::from_iter(ids.iter().copied()).len() != ids.len()
        {
            return Err(EngineError::Invalid(format!(
                "{command}: expected one or more unique shape ids"
            )));
        }
        for id in ids {
            if self.doc.get_shape(*id).is_none() {
                return Err(EngineError::Invalid(format!(
                    "{command}: no shape has id {id}"
                )));
            }
        }
        let ids: std::collections::BTreeSet<u64> = ids.iter().copied().collect();
        let ordered_ids = self
            .doc
            .shapes()
            .filter_map(|(_, shape)| ids.contains(&shape.id).then_some(shape.id))
            .collect::<Vec<_>>();
        let node_ids = ordered_ids
            .iter()
            .map(|id| self.node_for_shape(*id, command))
            .collect::<Result<Vec<_>, _>>()?;
        self.select_nodes(&node_ids)?;
        let value = node_ids.iter().map(|id| id.0).collect::<Vec<_>>();
        self.session
            .execute("edit.clear", &json!({"ids": value}))
            .map_err(|error| EngineError::Invalid(format!("{command}: {error}")))?;
        self.sync_doc_from_session()?;
        if ordered_ids.len() == 1 {
            Ok(Effect::ShapeRemoved { id: ordered_ids[0] })
        } else {
            Ok(Effect::ShapesRemoved { ids: ordered_ids })
        }
    }

    fn shape_bounds(&self, id: u64, command: &str) -> Result<kurbo::Rect, EngineError> {
        self.doc
            .get_shape(id)
            .and_then(stencilcraft_doc::Shape::bounds)
            .ok_or_else(|| EngineError::Invalid(format!("{command}: shape {id} has no bounds")))
    }

    fn vector_transform(
        &mut self,
        ids: &[u64],
        transform: kurbo::Affine,
        command: &str,
    ) -> Result<Effect, EngineError> {
        let nodes = ids
            .iter()
            .map(|id| self.node_for_shape(*id, command))
            .collect::<Result<Vec<_>, _>>()?;
        if ids.is_empty() {
            return Err(EngineError::Invalid(format!(
                "{command}: select at least one shape"
            )));
        }
        // Point-space coordinates use the same affine as millimetres because the unit scaling
        // cancels around every point; scale only the translation terms.
        let [a, b, c, d, e, f] = transform.as_coeffs();
        // Linear terms remain dimensionless; translation and stored geometry are converted
        // between StencilCraft millimetres and VectorCraft points.
        let params = json!({"ids": nodes.iter().map(|id| id.0).collect::<Vec<_>>(), "matrix": [a, b, c, d, e * POINTS_PER_MM, f * POINTS_PER_MM]});
        self.session
            .execute("object.transform", &params)
            .map_err(|error| EngineError::Invalid(format!("{command}: {error}")))?;
        self.sync_doc_from_session()?;
        Ok(Effect::ShapeTransformed {
            id: ids.first().copied().unwrap_or_default(),
        })
    }

    fn vector_align(
        &mut self,
        ids: &[u64],
        alignment: Alignment,
        to_board: bool,
    ) -> Result<Effect, EngineError> {
        let nodes = ids
            .iter()
            .map(|id| self.node_for_shape(*id, "shapes.align"))
            .collect::<Result<Vec<_>, _>>()?;
        let direction = match alignment {
            Alignment::Left => (Some("left"), None),
            Alignment::HorizontalCenter => (Some("center"), None),
            Alignment::Right => (Some("right"), None),
            Alignment::Top => (None, Some("top")),
            Alignment::VerticalCenter => (None, Some("center")),
            Alignment::Bottom => (None, Some("bottom")),
        };
        self.select_nodes(&nodes)?;
        let mut params =
            json!({"to": if to_board {"artboard"} else {"selection"}, "bounds": "geometric"});
        if let Some(horizontal) = direction.0 {
            params["horizontal"] = json!(horizontal);
        }
        if let Some(vertical) = direction.1 {
            params["vertical"] = json!(vertical);
        }
        self.session
            .execute("object.align", &params)
            .map_err(|error| EngineError::Invalid(format!("shapes.align: {error}")))?;
        self.sync_doc_from_session()?;
        Ok(Effect::ShapeTransformed {
            id: ids.first().copied().unwrap_or_default(),
        })
    }

    fn vector_distribute(
        &mut self,
        ids: &[u64],
        axis: DistributionAxis,
    ) -> Result<Effect, EngineError> {
        let nodes = ids
            .iter()
            .map(|id| self.node_for_shape(*id, "shapes.distribute"))
            .collect::<Result<Vec<_>, _>>()?;
        self.select_nodes(&nodes)?;
        let axis_name = match axis {
            DistributionAxis::Horizontal => "horizontal",
            DistributionAxis::Vertical => "vertical",
        };
        let params = json!({"axis": axis_name, "bounds": "geometric"});
        self.session
            .execute("object.distributeSpacing", &params)
            .map_err(|error| EngineError::Invalid(format!("shapes.distribute: {error}")))?;
        self.sync_doc_from_session()?;
        Ok(Effect::ShapeTransformed {
            id: ids.first().copied().unwrap_or_default(),
        })
    }

    fn vector_add_primitive(
        &mut self,
        primitive: &str,
        params: &serde_json::Value,
        style: CutStyle,
        command: &str,
    ) -> Result<Effect, EngineError> {
        let layer_index = self.top_layer_for_style(style, command)?;
        let layer_id = self.layer_nodes.get(layer_index).copied().ok_or_else(|| {
            EngineError::Internal(format!("{command}: missing VectorCraft layer mapping"))
        })?;
        self.session
            .execute("layer.setCurrent", &json!({"id": layer_id.0}))
            .map_err(|error| EngineError::Invalid(format!("{command}: {error}")))?;
        let created = self
            .session
            .execute(primitive, params)
            .map_err(|error| EngineError::Invalid(format!("{command}: {error}")))?;
        let node_id = created["id"].as_u64().ok_or_else(|| {
            EngineError::Internal(format!("{command}: VectorCraft returned no node id"))
        })?;
        let color = match style {
            CutStyle::Cut => "#000000",
            CutStyle::Engrave => "#c02020",
        };
        self.session
            .execute("paint.setFill", &json!({"ids": [node_id], "color": color}))
            .map_err(|error| EngineError::Invalid(format!("{command}: {error}")))?;
        self.sync_doc_from_session()?;
        let id = self.node_shapes.get(&node_id).copied().ok_or_else(|| {
            EngineError::Internal(format!("{command}: created shape was not synchronized"))
        })?;
        Ok(Effect::ShapeAdded { id })
    }

    fn add_vector_layer(
        &mut self,
        name: &str,
        kind: CutStyle,
        command: &str,
    ) -> Result<(), EngineError> {
        let created = self
            .session
            .execute("layer.new", &json!({"name": name}))
            .map_err(|error| EngineError::Invalid(format!("{command}: {error}")))?;
        let node_id = created["id"].as_u64().ok_or_else(|| {
            EngineError::Internal(format!("{command}: VectorCraft returned no layer id"))
        })?;
        let position = self
            .session
            .active()
            .and_then(|state| {
                state
                    .doc
                    .layers
                    .iter()
                    .position(|layer| layer.id.0 == node_id)
            })
            .ok_or_else(|| EngineError::Internal(format!("{command}: new layer is missing")))?;
        if position > self.doc.layers.len() || position > self.layer_nodes.len() {
            return Err(EngineError::Internal(format!(
                "{command}: VectorCraft layer order is inconsistent"
            )));
        }
        self.doc
            .layers
            .insert(position, stencilcraft_doc::Layer::new(name, kind));
        self.layer_nodes.insert(position, NodeId(node_id));
        for layer_index in self.node_layers.values_mut() {
            if *layer_index >= position {
                *layer_index += 1;
            }
        }
        Ok(())
    }

    fn vector_add_paths(
        &mut self,
        shapes: Vec<(BezPath, CutStyle)>,
        command: &str,
    ) -> Result<Vec<u64>, EngineError> {
        for (_, style) in &shapes {
            self.top_layer_for_style(*style, command)?;
        }

        let first_shape_id = self.doc.next_shape_id;
        let mut node_ids = Vec::with_capacity(shapes.len());
        for (path, style) in shapes.iter().rev() {
            let layer_index = self
                .doc
                .layers
                .iter()
                .rposition(|layer| layer.kind == *style)
                .ok_or_else(|| EngineError::Internal(format!("{command}: missing style layer")))?;
            let layer_id = self.layer_nodes.get(layer_index).copied().ok_or_else(|| {
                EngineError::Internal(format!("{command}: missing VectorCraft layer mapping"))
            })?;
            self.session
                .execute("layer.setCurrent", &json!({"id": layer_id.0}))
                .map_err(|error| EngineError::Invalid(format!("{command}: {error}")))?;
            let path_points = transform_path(path, kurbo::Affine::scale(POINTS_PER_MM));
            let path_data = path_to_svg_data(&path_points);
            let created = self
                .session
                .execute("path.create", &json!({"d": path_data, "closed": false}))
                .map_err(|error| EngineError::Invalid(format!("{command}: {error}")))?;
            let node_id = created["id"].as_u64().ok_or_else(|| {
                EngineError::Internal(format!("{command}: VectorCraft returned no node id"))
            })?;
            let color = match style {
                CutStyle::Cut => "#000000",
                CutStyle::Engrave => "#c02020",
            };
            self.session
                .execute("paint.setFill", &json!({"ids": [node_id], "color": color}))
                .map_err(|error| EngineError::Invalid(format!("{command}: {error}")))?;
            node_ids.push(NodeId(node_id));
        }
        // VectorCraft inserts at the top of a layer. Create paths in reverse input order, then
        // restore the input order for mapping back to StencilCraft's append order.
        node_ids.reverse();

        self.sync_doc_from_session()?;
        let mut remapped_shapes = self.shape_nodes.clone();
        for node_id in &node_ids {
            if let Some(old_id) = self.node_shapes.get(&node_id.0) {
                remapped_shapes.remove(old_id);
            }
        }
        for (offset, (node_id, (path, _))) in node_ids.iter().zip(shapes.iter()).enumerate() {
            let old_id = self.node_shapes.get(&node_id.0).copied().ok_or_else(|| {
                EngineError::Internal(format!("{command}: created path was not synchronized"))
            })?;
            let shape = self
                .doc
                .layers
                .iter_mut()
                .flat_map(|layer| &mut layer.shapes)
                .find(|shape| shape.id == old_id)
                .ok_or_else(|| {
                    EngineError::Internal(format!(
                        "{command}: synchronized shape {old_id} is missing"
                    ))
                })?;
            let id = first_shape_id + offset as u64;
            shape.id = id;
            shape.path = path.clone();
            remapped_shapes.insert(id, node_id.0);
        }
        self.shape_nodes = remapped_shapes.clone();
        self.node_shapes = remapped_shapes
            .into_iter()
            .map(|(shape_id, node_id)| (node_id, shape_id))
            .collect();
        for layer in &mut self.doc.layers {
            layer.shapes.sort_by_key(|shape| shape.id);
        }
        node_ids
            .into_iter()
            .map(|node_id| {
                self.node_shapes.get(&node_id.0).copied().ok_or_else(|| {
                    EngineError::Internal(format!("{command}: created path was not synchronized"))
                })
            })
            .collect()
    }

    fn node_for_shape(&self, id: u64, command: &str) -> Result<NodeId, EngineError> {
        self.shape_nodes
            .get(&id)
            .copied()
            .map(NodeId)
            .ok_or_else(|| EngineError::Invalid(format!("{command}: no shape has id {id}")))
    }

    fn select_nodes(&mut self, ids: &[NodeId]) -> Result<(), EngineError> {
        let ids = ids.iter().map(|id| id.0).collect::<Vec<_>>();
        self.session
            .execute("select.set", &json!({"ids": ids}))
            .map(|_| ())
            .map_err(|error| EngineError::Internal(error.to_string()))
    }

    fn sync_session_from_doc(&mut self) {
        let mut vdoc = vectorcraft_engine::doc::Document::new(
            self.doc.board.width_mm * POINTS_PER_MM,
            self.doc.board.height_mm * POINTS_PER_MM,
        );
        vdoc.layers.clear();
        self.shape_nodes.clear();
        self.node_shapes.clear();
        self.node_layers.clear();
        self.layer_nodes.clear();
        for (layer_index, layer) in self.doc.layers.iter().enumerate() {
            let layer_id = vdoc.add_layer(Some(&layer.name));
            self.layer_nodes.push(layer_id);
            let mut layer_children = Vec::with_capacity(layer.shapes.len());
            for shape in &layer.shapes {
                let node_id = vdoc.alloc_id();
                let fill = FillLayer::new(Paint::solid(match shape.style {
                    CutStyle::Cut => Color::BLACK,
                    CutStyle::Engrave => Color::rgb8(0xc0, 0x20, 0x20),
                }));
                let path = transform_path(&shape.path, kurbo::Affine::scale(POINTS_PER_MM));
                let mut node = Node::path(
                    node_id,
                    PathData::from_bezpath(&path),
                    Appearance {
                        items: vec![AppearanceItem::Fill(fill)],
                        ..Appearance::default()
                    },
                );
                node.name = Some(format!("shape-{}", shape.id));
                self.shape_nodes.insert(shape.id, node_id.0);
                self.node_shapes.insert(node_id.0, shape.id);
                self.node_layers.insert(node_id.0, layer_index);
                layer_children.push(Arc::new(node));
            }
            if let Some(layer_node) = vdoc.node_mut(layer_id) {
                layer_node.visible = layer.visible;
                if let NodeKind::Layer { children, .. } = &mut layer_node.kind {
                    *children = layer_children;
                }
            }
        }
        vdoc.fix_next_id();
        let mut session = VectorCraftSession::new();
        session.add_document(vdoc, None);
        self.session = session;
    }

    fn sync_doc_from_session(&mut self) -> Result<(), EngineError> {
        let state = self.session.active().ok_or_else(|| {
            EngineError::Internal("VectorCraft session lost its active document".to_string())
        })?;
        let vector_doc = &state.doc;
        let mut paths = Vec::new();
        for (layer_index, layer) in vector_doc.layers.iter().enumerate() {
            collect_vector_paths(layer, layer_index, &mut paths);
        }

        let old_shapes: BTreeMap<u64, stencilcraft_doc::Shape> = self
            .doc
            .shapes()
            .map(|(_, shape)| (shape.id, shape.clone()))
            .collect();
        let mut updated = self.doc.clone();
        for layer in &mut updated.layers {
            layer.shapes.clear();
        }
        let mut active_nodes = BTreeMap::new();
        let mut new_shapes = Vec::with_capacity(paths.len());
        for (node_id, path, layer_index, inferred_style, source_name) in paths {
            let existing_shape_id = self.node_shapes.get(&node_id.0).copied();
            let source_style = source_name
                .as_deref()
                .and_then(|name| name.strip_prefix("shape-"))
                .and_then(|id| id.parse::<u64>().ok())
                .and_then(|id| old_shapes.get(&id).map(|shape| shape.style));
            let style = source_style
                .or_else(|| {
                    existing_shape_id.and_then(|id| old_shapes.get(&id).map(|shape| shape.style))
                })
                .unwrap_or(inferred_style);
            let shape_id =
                if let Some(id) = existing_shape_id.filter(|id| old_shapes.contains_key(id)) {
                    id
                } else {
                    let id = updated.next_shape_id;
                    updated.next_shape_id = updated
                        .next_shape_id
                        .checked_add(1)
                        .ok_or_else(|| EngineError::Internal("shape ids exhausted".to_string()))?;
                    id
                };
            let target_layer = updated
                .layers
                .get(layer_index)
                .filter(|layer| layer.kind == style)
                .map(|_| layer_index)
                .or_else(|| updated.layers.iter().rposition(|layer| layer.kind == style))
                .unwrap_or_else(|| {
                    updated.layers.push(stencilcraft_doc::Layer::new(
                        match style {
                            CutStyle::Cut => "cut",
                            CutStyle::Engrave => "engrave",
                        },
                        style,
                    ));
                    updated.layers.len() - 1
                });
            new_shapes.push((
                target_layer,
                stencilcraft_doc::Shape {
                    id: shape_id,
                    path: snap_mm_path(&transform_path(
                        &path,
                        kurbo::Affine::scale(1.0 / POINTS_PER_MM),
                    )),
                    style,
                },
            ));
            active_nodes.insert(shape_id, node_id.0);
            self.node_shapes.insert(node_id.0, shape_id);
        }
        for (layer_index, shape) in new_shapes {
            updated.layers[layer_index].shapes.push(shape);
        }
        self.shape_nodes = active_nodes
            .iter()
            .map(|(shape_id, node_id)| (*shape_id, *node_id))
            .collect();
        self.node_shapes = active_nodes
            .iter()
            .map(|(shape_id, node_id)| (*node_id, *shape_id))
            .collect();
        self.node_layers.clear();
        for (layer_index, layer) in updated.layers.iter().enumerate() {
            for shape in &layer.shapes {
                if let Some(node_id) = self.shape_nodes.get(&shape.id) {
                    self.node_layers.insert(*node_id, layer_index);
                }
            }
        }
        self.doc = updated;
        Ok(())
    }

    fn top_layer_for_style(
        &mut self,
        style: CutStyle,
        command: &str,
    ) -> Result<usize, EngineError> {
        if let Some(index) = self
            .doc
            .layers
            .iter()
            .rposition(|layer| layer.kind == style)
        {
            return Ok(index);
        }
        let name = match style {
            CutStyle::Cut => "cut",
            CutStyle::Engrave => "engrave",
        };
        self.add_vector_layer(name, style, command)?;
        self.doc
            .layers
            .iter()
            .rposition(|layer| layer.kind == style)
            .ok_or_else(|| EngineError::Internal(format!("{command}: missing new style layer")))
    }

    fn capture_history_entry(&self, vectorcraft: bool, rebuild_session: bool) -> HistoryEntry {
        HistoryEntry {
            snapshot: self.doc.clone(),
            mappings: MappingSnapshot {
                shape_nodes: self.shape_nodes.clone(),
                node_shapes: self.node_shapes.clone(),
                node_layers: self.node_layers.clone(),
                layer_nodes: self.layer_nodes.clone(),
            },
            vectorcraft,
            rebuild_session,
        }
    }

    fn restore_mappings(&mut self, mappings: MappingSnapshot) {
        self.shape_nodes = mappings.shape_nodes;
        self.node_shapes = mappings.node_shapes;
        self.node_layers = mappings.node_layers;
        self.layer_nodes = mappings.layer_nodes;
    }

    fn restore_history_entry(&mut self, entry: &HistoryEntry) {
        self.doc = entry.snapshot.clone();
        self.restore_mappings(entry.mappings.clone());
        if entry.rebuild_session {
            self.sync_session_from_doc();
            self.history.rebase_to_snapshots();
        }
    }

    pub fn can_undo(&self) -> bool {
        self.history.peek_undo().is_some()
    }

    pub fn can_redo(&self) -> bool {
        self.history.peek_redo().is_some()
    }

    /// Undo the last StencilCraft command through VectorCraft's journal when the command edited
    /// its document; snapshot-only metadata changes restore directly. Returns false when empty.
    pub fn undo(&mut self) -> bool {
        let Some(entry) = self.history.peek_undo().cloned() else {
            return false;
        };
        if entry.vectorcraft && self.session.execute("edit.undo", &json!({})).is_err() {
            return false;
        }
        let current = self.capture_history_entry(entry.vectorcraft, entry.rebuild_session);
        let Some(previous) = self.history.undo(current) else {
            return false;
        };
        self.restore_history_entry(&previous);
        true
    }

    /// Redo the last StencilCraft command through VectorCraft's journal when applicable.
    pub fn redo(&mut self) -> bool {
        let Some(entry) = self.history.peek_redo().cloned() else {
            return false;
        };
        if entry.vectorcraft && self.session.execute("edit.redo", &json!({})).is_err() {
            return false;
        }
        let current = self.capture_history_entry(entry.vectorcraft, entry.rebuild_session);
        let Some(next) = self.history.redo(current) else {
            return false;
        };
        self.restore_history_entry(&next);
        true
    }
}

fn path_to_svg_data(path: &BezPath) -> String {
    path.elements()
        .iter()
        .map(|element| match *element {
            kurbo::PathEl::MoveTo(point) => format!("M{} {}", point.x, point.y),
            kurbo::PathEl::LineTo(point) => format!("L{} {}", point.x, point.y),
            kurbo::PathEl::QuadTo(control, point) => {
                format!("Q{} {} {} {}", control.x, control.y, point.x, point.y)
            }
            kurbo::PathEl::CurveTo(control1, control2, point) => format!(
                "C{} {} {} {} {} {}",
                control1.x, control1.y, control2.x, control2.y, point.x, point.y
            ),
            kurbo::PathEl::ClosePath => "Z".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn transform_path(path: &BezPath, transform: kurbo::Affine) -> BezPath {
    let mut transformed = BezPath::new();
    for element in path.elements() {
        match *element {
            kurbo::PathEl::MoveTo(point) => transformed.move_to(transform * point),
            kurbo::PathEl::LineTo(point) => transformed.line_to(transform * point),
            kurbo::PathEl::QuadTo(control, point) => {
                transformed.quad_to(transform * control, transform * point)
            }
            kurbo::PathEl::CurveTo(control1, control2, point) => transformed.curve_to(
                transform * control1,
                transform * control2,
                transform * point,
            ),
            kurbo::PathEl::ClosePath => transformed.close_path(),
        }
    }
    transformed
}
fn snap_mm_path(path: &BezPath) -> BezPath {
    const GRID: f64 = 1_000_000_000_000.0;
    let snap = |value: f64| {
        let rounded = (value * GRID).round() / GRID;
        if rounded.is_finite()
            && (rounded - value).abs() <= value.abs().max(1.0) * f64::EPSILON * 8.0
        {
            rounded
        } else {
            value
        }
    };
    let point = |point: kurbo::Point| kurbo::Point::new(snap(point.x), snap(point.y));
    let mut snapped = BezPath::new();
    for element in path.elements() {
        match *element {
            kurbo::PathEl::MoveTo(p) => snapped.move_to(point(p)),
            kurbo::PathEl::LineTo(p) => snapped.line_to(point(p)),
            kurbo::PathEl::QuadTo(control, p) => snapped.quad_to(point(control), point(p)),
            kurbo::PathEl::CurveTo(a, b, p) => snapped.curve_to(point(a), point(b), point(p)),
            kurbo::PathEl::ClosePath => snapped.close_path(),
        }
    }
    snapped
}

fn collect_vector_paths(
    node: &Node,
    layer_index: usize,
    out: &mut Vec<(NodeId, BezPath, usize, CutStyle, Option<String>)>,
) {
    if let Some(path) = node.path_data() {
        out.push((
            node.id,
            path.to_bezpath(),
            layer_index,
            style_from_node(node),
            node.name.clone(),
        ));
    } else if let Some(children) = node.children() {
        for child in children {
            collect_vector_paths(child, layer_index, out);
        }
    }
}

fn style_from_node(node: &Node) -> CutStyle {
    match node.appearance.fill_paint().color() {
        Some(color) if color == Color::rgb8(0xc0, 0x20, 0x20) => CutStyle::Engrave,
        _ => CutStyle::Cut,
    }
}

fn validate_shape_bounds(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    command: &str,
) -> Result<(), EngineError> {
    if !(x.is_finite()
        && y.is_finite()
        && width.is_finite()
        && height.is_finite()
        && width > 0.0
        && height > 0.0
        && (x + width).is_finite()
        && (y + height).is_finite()
        && (x * POINTS_PER_MM).is_finite()
        && (y * POINTS_PER_MM).is_finite()
        && (width * POINTS_PER_MM).is_finite()
        && (height * POINTS_PER_MM).is_finite())
    {
        return Err(EngineError::Invalid(format!(
            "{command}: position must be finite and size positive and finite"
        )));
    }
    Ok(())
}

fn validate_radial_primitive(
    cx: f64,
    cy: f64,
    radius: f64,
    rotation: f64,
    count: u32,
    min_count: u32,
    command: &str,
) -> Result<(), EngineError> {
    if !(cx.is_finite()
        && cy.is_finite()
        && radius.is_finite()
        && radius > 0.0
        && rotation.is_finite()
        && (min_count..=1000).contains(&count)
        && (cx - radius).is_finite()
        && (cx + radius).is_finite()
        && (cy - radius).is_finite()
        && (cy + radius).is_finite()
        && (cx * POINTS_PER_MM).is_finite()
        && (cy * POINTS_PER_MM).is_finite()
        && (radius * POINTS_PER_MM).is_finite())
    {
        return Err(EngineError::Invalid(format!(
            "{command}: center and rotation must be finite, radius positive, and count between {min_count} and 1000"
        )));
    }
    Ok(())
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
    fn duplicate_shapes_preserves_exact_geometry_style_order_and_history() {
        let first = rect_path(1.0, 2.0, 3.0, 4.0);
        let second = rect_path(10.0, 20.0, 30.0, 40.0);
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddPaths {
                shapes: vec![
                    (first.clone(), CutStyle::Cut),
                    (second.clone(), CutStyle::Engrave),
                ],
            })
            .unwrap();

        let effect = engine
            .run(Command::ShapesDuplicate { ids: vec![2, 1] })
            .unwrap();
        assert_eq!(effect, Effect::ShapesAdded { ids: vec![3, 4] });
        assert_eq!(engine.doc.shape_count(), 4);
        assert_eq!(engine.doc.get_shape(3).unwrap().path, first);
        assert_eq!(engine.doc.get_shape(4).unwrap().path, second);
        assert_eq!(engine.doc.get_shape(3).unwrap().style, CutStyle::Cut);
        assert_eq!(engine.doc.get_shape(4).unwrap().style, CutStyle::Engrave);
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 2);
        assert!(engine.redo());
        assert_eq!(engine.doc.shape_count(), 4);
    }

    #[test]
    fn duplicate_shapes_rejects_empty_duplicate_or_missing_ids_atomically() {
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        let before = engine.doc.clone();
        for ids in [vec![], vec![1, 1], vec![1, 99]] {
            assert!(engine.run(Command::ShapesDuplicate { ids }).is_err());
            assert_eq!(engine.doc, before);
        }
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
    fn set_bounds_edits_position_and_size_in_one_undoable_command() {
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddRect {
                x: 5.0,
                y: 7.0,
                width: 10.0,
                height: 20.0,
            })
            .unwrap();
        engine
            .run(Command::ShapeSetBounds {
                id: 1,
                x: 12.0,
                y: 15.0,
                width: 40.0,
                height: 5.0,
            })
            .unwrap();
        assert_eq!(
            engine.doc.get_shape(1).unwrap().bounds(),
            Some(kurbo::Rect::new(12.0, 15.0, 52.0, 20.0))
        );
        assert!(engine.undo());
        assert_eq!(
            engine.doc.get_shape(1).unwrap().bounds(),
            Some(kurbo::Rect::new(5.0, 7.0, 15.0, 27.0))
        );
    }

    #[test]
    fn imported_paths_are_added_to_the_matching_layer_and_undoable() {
        let mut engine = Engine::default();
        let mut path = BezPath::new();
        path.move_to(kurbo::Point::new(1.0, 2.0));
        path.line_to(kurbo::Point::new(3.0, 4.0));
        engine
            .run(Command::ShapeAddPath {
                path,
                style: CutStyle::Engrave,
            })
            .unwrap();
        assert_eq!(engine.doc.get_shape(1).unwrap().style, CutStyle::Engrave);
        assert_eq!(engine.doc.layers[0].kind, CutStyle::Cut);
        assert_eq!(engine.doc.layers[1].kind, CutStyle::Engrave);
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 0);
    }

    #[test]
    fn vectorcraft_path_creation_preserves_shape_order_style_and_geometry() {
        let first = rect_path(1.0, 2.0, 3.0, 4.0);
        let second = rect_path(10.0, 20.0, 30.0, 40.0);
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddPaths {
                shapes: vec![
                    (first.clone(), CutStyle::Cut),
                    (second.clone(), CutStyle::Engrave),
                ],
            })
            .unwrap();

        assert_eq!(engine.doc.get_shape(1).unwrap().path, first);
        assert_eq!(engine.doc.get_shape(2).unwrap().path, second);
        assert_eq!(engine.doc.get_shape(1).unwrap().style, CutStyle::Cut);
        assert_eq!(engine.doc.get_shape(2).unwrap().style, CutStyle::Engrave);
        assert_eq!(
            engine
                .doc
                .shapes()
                .map(|(_, shape)| shape.id)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn imported_style_layers_keep_existing_shapes_and_history_editable() {
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            })
            .unwrap();
        engine
            .run(Command::ShapeAddPath {
                path: rect_path(10.0, 10.0, 5.0, 5.0),
                style: CutStyle::Engrave,
            })
            .unwrap();
        assert_eq!(engine.doc.shape_count(), 2);
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 1);
        assert_eq!(engine.doc.layers.len(), 1);
        assert!(engine.redo());
        assert_eq!(engine.doc.shape_count(), 2);
        engine
            .run(Command::ShapeMove {
                id: 1,
                dx_mm: 2.0,
                dy_mm: 0.0,
            })
            .unwrap();
        assert_eq!(engine.doc.get_shape(1).unwrap().bounds().unwrap().x0, 2.0);
        assert!(engine.undo());
        assert_eq!(engine.doc.get_shape(1).unwrap().bounds().unwrap().x0, 0.0);
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 1);
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 0);
    }

    #[test]
    fn imported_paths_are_one_undoable_edit() {
        let mut first = BezPath::new();
        first.move_to(kurbo::Point::new(1.0, 2.0));
        first.line_to(kurbo::Point::new(3.0, 4.0));
        let mut second = BezPath::new();
        second.move_to(kurbo::Point::new(5.0, 6.0));
        second.line_to(kurbo::Point::new(7.0, 8.0));
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddPaths {
                shapes: vec![(first, CutStyle::Cut), (second, CutStyle::Engrave)],
            })
            .unwrap();
        assert_eq!(engine.doc.shape_count(), 2);
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 0);
    }

    #[test]
    fn pathfinder_unite_preserves_curves_and_is_one_undoable_edit() {
        use kurbo::Shape as _;

        let mut first = kurbo::Circle::new((20.0, 20.0), 10.0).to_path(0.01);
        let second = kurbo::Circle::new((32.0, 20.0), 10.0).to_path(0.01);
        let original = [first.clone(), second.clone()];
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddPaths {
                shapes: vec![
                    (std::mem::take(&mut first), CutStyle::Cut),
                    (second, CutStyle::Cut),
                ],
            })
            .unwrap();

        let effect = engine
            .run(Command::ShapePathfinder {
                ids: vec![2, 1],
                operation: BooleanOperation::Union,
            })
            .unwrap();
        let Effect::ShapesAdded { ids } = effect else {
            panic!("Pathfinder should return result shape ids");
        };
        assert_eq!(ids.len(), 1);
        assert_eq!(engine.doc.shape_count(), 1);
        let result = &engine.doc.get_shape(ids[0]).unwrap().path;
        assert!(
            result
                .elements()
                .iter()
                .any(|element| matches!(element, kurbo::PathEl::CurveTo(..)))
        );

        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 2);
        assert_eq!(engine.doc.get_shape(1).unwrap().path, original[0]);
        assert_eq!(engine.doc.get_shape(2).unwrap().path, original[1]);

        let mut reversed = Engine::default();
        reversed
            .run(Command::ShapeAddPaths {
                shapes: vec![
                    (rect_path(0.0, 0.0, 20.0, 20.0), CutStyle::Cut),
                    (rect_path(10.0, 0.0, 20.0, 20.0), CutStyle::Engrave),
                ],
            })
            .unwrap();
        reversed
            .run(Command::ShapeBoolean {
                a: 2,
                b: 1,
                operation: BooleanOperation::Difference,
            })
            .unwrap();
        assert_eq!(reversed.doc.shape_count(), 1);
        assert_eq!(reversed.doc.get_shape(3).unwrap().style, CutStyle::Engrave);
        assert_eq!(
            reversed.doc.get_shape(3).unwrap().bounds(),
            Some(kurbo::Rect::new(20.0, 0.0, 30.0, 20.0))
        );
    }

    #[test]
    fn pathfinder_minus_front_keeps_backmost_style_and_empty_results_are_allowed() {
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddPaths {
                shapes: vec![
                    (rect_path(0.0, 0.0, 20.0, 20.0), CutStyle::Cut),
                    (rect_path(10.0, 0.0, 20.0, 20.0), CutStyle::Engrave),
                ],
            })
            .unwrap();
        engine
            .run(Command::ShapePathfinder {
                ids: vec![2, 1],
                operation: BooleanOperation::Difference,
            })
            .unwrap();
        assert_eq!(engine.doc.shape_count(), 1);
        let result = engine.doc.get_shape(3).unwrap();
        assert_eq!(result.style, CutStyle::Cut);
        assert_eq!(
            result.bounds(),
            Some(kurbo::Rect::new(0.0, 0.0, 10.0, 20.0))
        );

        let mut disjoint = Engine::default();
        disjoint
            .run(Command::ShapeAddPaths {
                shapes: vec![
                    (rect_path(0.0, 0.0, 10.0, 10.0), CutStyle::Cut),
                    (rect_path(20.0, 0.0, 10.0, 10.0), CutStyle::Cut),
                ],
            })
            .unwrap();
        disjoint
            .run(Command::ShapePathfinder {
                ids: vec![1, 2],
                operation: BooleanOperation::Intersect,
            })
            .unwrap();
        assert_eq!(disjoint.doc.shape_count(), 0);
        assert!(disjoint.undo());
        assert_eq!(disjoint.doc.shape_count(), 2);
        assert!(
            disjoint
                .run(Command::ShapePathfinder {
                    ids: vec![1, 2, 99],
                    operation: BooleanOperation::Union,
                })
                .is_err()
        );
        assert_eq!(disjoint.doc.shape_count(), 2);
    }

    #[test]
    fn multi_shape_align_distribute_move_together_and_undo_atomically() {
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddPaths {
                shapes: vec![
                    (rect_path(0.0, 0.0, 10.0, 10.0), CutStyle::Cut),
                    (rect_path(30.0, 20.0, 20.0, 10.0), CutStyle::Cut),
                    (rect_path(100.0, 40.0, 10.0, 10.0), CutStyle::Cut),
                ],
            })
            .unwrap();
        engine
            .run(Command::ShapesAlign {
                ids: vec![1, 2],
                alignment: Alignment::Left,
                to_board: false,
            })
            .unwrap();
        assert_eq!(engine.doc.get_shape(1).unwrap().bounds().unwrap().x0, 0.0);
        assert_eq!(engine.doc.get_shape(2).unwrap().bounds().unwrap().x0, 0.0);
        assert_eq!(engine.doc.get_shape(2).unwrap().bounds().unwrap().y0, 20.0);
        assert!(engine.undo());
        assert_eq!(engine.doc.get_shape(2).unwrap().bounds().unwrap().x0, 30.0);

        engine
            .run(Command::ShapesDistribute {
                ids: vec![1, 2, 3],
                axis: DistributionAxis::Horizontal,
            })
            .unwrap();
        let a = engine.doc.get_shape(1).unwrap().bounds().unwrap();
        let b = engine.doc.get_shape(2).unwrap().bounds().unwrap();
        let c = engine.doc.get_shape(3).unwrap().bounds().unwrap();
        assert!((b.x0 - a.x1 - (c.x0 - b.x1)).abs() < 1e-9);
        assert!(engine.undo());
        assert_eq!(engine.doc.get_shape(2).unwrap().bounds().unwrap().x0, 30.0);
    }

    #[test]
    fn multi_shape_commands_reject_duplicate_or_missing_ids_without_partial_edits() {
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        engine
            .run(Command::ShapeAddRect {
                x: 20.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        let before = engine.doc.clone();
        assert!(
            engine
                .run(Command::ShapesAlign {
                    ids: vec![1, 1],
                    alignment: Alignment::Left,
                    to_board: false,
                })
                .is_err()
        );
        assert!(
            engine
                .run(Command::ShapesTransform {
                    ids: vec![1, 99],
                    dx_mm: 5.0,
                    dy_mm: 0.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    degrees: 0.0,
                })
                .is_err()
        );
        assert_eq!(engine.doc, before);
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
    fn metadata_snapshots_and_vectorcraft_history_stay_in_lockstep() {
        let mut engine = Engine::default();
        engine.run(Command::KerfSet { width_mm: 0.25 }).unwrap();
        engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            })
            .unwrap();
        engine.run(Command::KerfSet { width_mm: 0.5 }).unwrap();
        assert!(engine.undo());
        assert_eq!(engine.doc.kerf.width_mm, 0.25);
        assert_eq!(engine.doc.shape_count(), 1);
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 0);
        assert_eq!(engine.doc.kerf.width_mm, 0.25);
        assert!(engine.redo());
        assert_eq!(engine.doc.shape_count(), 1);
        assert!(engine.redo());
        assert_eq!(engine.doc.kerf.width_mm, 0.5);
    }

    #[test]
    fn vectorcraft_history_limit_preserves_oldest_retained_state() {
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            })
            .unwrap();
        for _ in 0..101 {
            engine
                .run(Command::ShapeMove {
                    id: 1,
                    dx_mm: 1.0,
                    dy_mm: 0.0,
                })
                .unwrap();
        }
        assert_eq!(engine.doc.get_shape(1).unwrap().bounds().unwrap().x0, 101.0);
        for x in (1..=100).rev() {
            assert!(engine.undo(), "undo to x={x}");
            assert_eq!(
                engine.doc.get_shape(1).unwrap().bounds().unwrap().x0,
                f64::from(x)
            );
        }
        assert!(!engine.undo());
        for x in 2..=101 {
            assert!(engine.redo(), "redo to x={x}");
            assert_eq!(
                engine.doc.get_shape(1).unwrap().bounds().unwrap().x0,
                f64::from(x)
            );
        }
        assert!(!engine.redo());
    }

    #[test]
    fn vectorcraft_branch_discards_redo_and_keeps_metadata_in_sync() {
        let mut engine = Engine::default();
        engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            })
            .unwrap();
        engine.run(Command::KerfSet { width_mm: 0.25 }).unwrap();
        engine
            .run(Command::ShapeMove {
                id: 1,
                dx_mm: 10.0,
                dy_mm: 0.0,
            })
            .unwrap();
        assert!(engine.undo());
        assert!(engine.undo());
        engine
            .run(Command::ShapeMove {
                id: 1,
                dx_mm: 3.0,
                dy_mm: 0.0,
            })
            .unwrap();
        assert!(!engine.redo());
        assert_eq!(engine.doc.get_shape(1).unwrap().bounds().unwrap().x0, 3.0);
        assert_eq!(engine.doc.kerf.width_mm, 0.0);
        assert!(engine.undo());
        assert_eq!(engine.doc.get_shape(1).unwrap().bounds().unwrap().x0, 0.0);
        assert!(engine.redo());
        assert_eq!(engine.doc.get_shape(1).unwrap().bounds().unwrap().x0, 3.0);
        engine
            .run(Command::ShapeMove {
                id: 1,
                dx_mm: 2.0,
                dy_mm: 0.0,
            })
            .unwrap();
        assert_eq!(engine.doc.get_shape(1).unwrap().bounds().unwrap().x0, 5.0);
    }

    #[test]
    fn document_new_rebases_both_history_stacks_to_snapshots() {
        let mut engine = Engine::default();
        engine.run(Command::KerfSet { width_mm: 0.25 }).unwrap();
        engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            })
            .unwrap();
        engine
            .run(Command::DocumentNew {
                width_mm: 80.0,
                height_mm: 90.0,
            })
            .unwrap();
        assert_eq!(engine.doc.shape_count(), 0);
        assert_eq!(engine.doc.board.width_mm, 80.0);
        assert_eq!(engine.doc.kerf.width_mm, 0.25);
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 1);
        assert_eq!(engine.doc.board.width_mm, 100.0);
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 0);
        assert_eq!(engine.doc.kerf.width_mm, 0.25);
        assert!(engine.redo());
        assert_eq!(engine.doc.shape_count(), 1);
        assert!(engine.redo());
        assert_eq!(engine.doc.board.width_mm, 80.0);
        assert_eq!(engine.doc.shape_count(), 0);
    }

    #[test]
    fn branching_after_undoing_reset_discards_reset_and_keeps_restored_shapes_editable() {
        let mut engine = Engine::default();
        engine.run(Command::KerfSet { width_mm: 0.25 }).unwrap();
        engine
            .run(Command::ShapeAddRect {
                x: 2.0,
                y: 3.0,
                width: 5.0,
                height: 5.0,
            })
            .unwrap();
        let original = engine.doc.clone();
        engine
            .run(Command::DocumentNew {
                width_mm: 80.0,
                height_mm: 90.0,
            })
            .unwrap();
        engine
            .run(Command::ShapeAddRect {
                x: 20.0,
                y: 20.0,
                width: 4.0,
                height: 4.0,
            })
            .unwrap();
        assert!(engine.undo());
        assert!(engine.undo());
        assert_eq!(engine.doc, original);

        engine
            .run(Command::ShapeMove {
                id: 1,
                dx_mm: 7.0,
                dy_mm: 0.0,
            })
            .unwrap();
        let branch = engine.doc.clone();
        assert_eq!(branch.get_shape(1).unwrap().bounds().unwrap().x0, 9.0);
        assert_eq!(branch.kerf.width_mm, 0.25);
        assert!(!engine.redo());
        assert!(engine.undo());
        assert_eq!(engine.doc, original);
        assert!(engine.redo());
        assert_eq!(engine.doc, branch);
        assert!(!engine.redo());
        engine
            .run(Command::ShapeMove {
                id: 1,
                dx_mm: 1.0,
                dy_mm: 0.0,
            })
            .unwrap();
        assert_eq!(engine.doc.get_shape(1).unwrap().bounds().unwrap().x0, 10.0);
    }

    #[test]
    fn reset_after_undo_discards_old_redo_and_new_branch_roundtrips() {
        let mut engine = Engine::default();
        engine.run(Command::KerfSet { width_mm: 0.25 }).unwrap();
        engine
            .run(Command::ShapeAddRect {
                x: 2.0,
                y: 3.0,
                width: 5.0,
                height: 5.0,
            })
            .unwrap();
        let original = engine.doc.clone();
        engine
            .run(Command::ShapeMove {
                id: 1,
                dx_mm: 10.0,
                dy_mm: 0.0,
            })
            .unwrap();
        assert!(engine.undo());
        assert_eq!(engine.doc, original);
        engine
            .run(Command::DocumentNew {
                width_mm: 80.0,
                height_mm: 90.0,
            })
            .unwrap();
        let reset = engine.doc.clone();
        assert!(!engine.redo());
        assert_eq!(reset.shape_count(), 0);
        assert_eq!((reset.board.width_mm, reset.board.height_mm), (80.0, 90.0));
        assert_eq!(reset.kerf.width_mm, 0.25);
        engine
            .run(Command::ShapeAddRect {
                x: 6.0,
                y: 7.0,
                width: 4.0,
                height: 4.0,
            })
            .unwrap();
        let branch = engine.doc.clone();
        assert!(engine.undo());
        assert_eq!(engine.doc, reset);
        assert!(engine.undo());
        assert_eq!(engine.doc, original);
        assert!(engine.redo());
        assert_eq!(engine.doc, reset);
        assert!(engine.redo());
        assert_eq!(engine.doc, branch);
        assert!(!engine.redo());
        let id = branch.shapes().next().unwrap().1.id;
        engine
            .run(Command::ShapeMove {
                id,
                dx_mm: 2.0,
                dy_mm: 0.0,
            })
            .unwrap();
        assert_eq!(engine.doc.get_shape(id).unwrap().bounds().unwrap().x0, 8.0);
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
    fn vectorcraft_shape_primitives_add_cut_shapes_and_roundtrip_through_history() {
        let mut engine = Engine::default();
        for command in [
            Command::ShapeAddEllipse {
                x: 5.0,
                y: 8.0,
                width: 12.0,
                height: 6.0,
            },
            Command::ShapeAddPolygon {
                cx: 30.0,
                cy: 20.0,
                radius: 8.0,
                sides: 6,
                rotation: 15.0,
            },
            Command::ShapeAddStar {
                cx: 50.0,
                cy: 20.0,
                radius: 10.0,
                inner_radius: 4.0,
                points: 5,
                rotation: 0.0,
            },
        ] {
            assert!(matches!(
                engine.run(command).unwrap(),
                Effect::ShapeAdded { .. }
            ));
        }
        assert_eq!(engine.doc.shape_count(), 3);
        assert_eq!(engine.doc.get_shape(1).unwrap().style, CutStyle::Cut);
        assert_eq!(
            engine.doc.get_shape(1).unwrap().bounds(),
            Some(kurbo::Rect::new(5.0, 8.0, 17.0, 14.0))
        );
        assert_eq!(
            engine.doc.get_shape(2).unwrap().bounds().unwrap().center(),
            kurbo::Point::new(30.0, 20.0)
        );
        let star_bounds = engine.doc.get_shape(3).unwrap().bounds().unwrap();
        assert!(star_bounds.width() > 15.0);
        assert!(star_bounds.height() > 15.0);
        assert!(engine.undo());
        assert_eq!(engine.doc.shape_count(), 2);
        assert!(engine.redo());
        assert_eq!(engine.doc.shape_count(), 3);
    }

    #[test]
    fn vectorcraft_shape_primitives_reject_out_of_range_geometry() {
        let mut engine = Engine::default();
        for command in [
            Command::ShapeAddEllipse {
                x: 0.0,
                y: 0.0,
                width: f64::INFINITY,
                height: 1.0,
            },
            Command::ShapeAddPolygon {
                cx: 0.0,
                cy: 0.0,
                radius: 1.0,
                sides: 2,
                rotation: 0.0,
            },
            Command::ShapeAddStar {
                cx: 0.0,
                cy: 0.0,
                radius: 5.0,
                inner_radius: 6.0,
                points: 5,
                rotation: 0.0,
            },
            Command::ShapeAddRoundedRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
                radius: -1.0,
            },
        ] {
            assert!(engine.run(command).is_err());
            assert_eq!(engine.doc.shape_count(), 0);
        }
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

    #[test]
    fn board_path_style_and_layer_commands_roundtrip_undo_redo() {
        let mut engine = Engine::default();
        assert!(!engine.can_undo());
        engine
            .run(Command::DocumentSetBoard {
                width_mm: 304.8,
                height_mm: 609.6,
            })
            .unwrap();
        assert_eq!(engine.doc.board.width_mm, 304.8);
        assert_eq!(engine.doc.board.height_mm, 609.6);
        assert!(engine.can_undo());

        engine
            .run(Command::ShapeAddRect {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 20.0,
            })
            .unwrap();
        let mut updated_path = BezPath::new();
        updated_path.move_to(kurbo::Point::new(5.0, 5.0));
        updated_path.line_to(kurbo::Point::new(35.0, 5.0));
        updated_path.line_to(kurbo::Point::new(20.0, 35.0));
        updated_path.close_path();
        engine
            .run(Command::ShapeSetPath {
                id: 1,
                path: updated_path.clone(),
            })
            .unwrap();
        assert_eq!(engine.doc.get_shape(1).unwrap().path, updated_path);

        engine
            .run(Command::ShapesSetStyle {
                ids: vec![1],
                style: CutStyle::Engrave,
            })
            .unwrap();
        assert_eq!(engine.doc.get_shape(1).unwrap().style, CutStyle::Engrave);
        assert_eq!(engine.doc.layers.len(), 2);

        engine
            .run(Command::LayerSetVisible {
                index: 1,
                visible: false,
            })
            .unwrap();
        assert!(!engine.doc.layers[1].visible);

        engine.run(Command::LayerRemove { index: 0 }).unwrap();
        assert_eq!(engine.doc.layers.len(), 1);
        assert!(engine.run(Command::LayerRemove { index: 0 }).is_err());

        assert!(engine.undo());
        assert_eq!(engine.doc.layers.len(), 2);
        assert!(engine.can_redo());
        assert!(engine.redo());
        assert_eq!(engine.doc.layers.len(), 1);
    }
}
