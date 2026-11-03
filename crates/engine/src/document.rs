//! Document and canvas commands: groups, stacking order, layer management and Color Sync,
//! pages, ruler guides and the grid, snapping, measuring, selection helpers, the engine
//! clipboard and repeatable transforms.
//!
//! These commands edit the StencilCraft document directly (no VectorCraft operation fits
//! them), so each successful edit is one snapshot in the shared history and the VectorCraft
//! session is rebuilt from the document afterwards, as for the other snapshot commands in
//! `lib.rs`. Queries answer without touching the document or history. Coordinates are
//! millimetres; the canvas shows y growing downwards, so "top" is the smaller y.

use std::collections::BTreeSet;

use kurbo::{Affine, Rect};
use serde::{Deserialize, Serialize};
use stencilcraft_doc::canvas::{self, color_distance, format_hex_color, parse_hex_color};
use stencilcraft_doc::templates::{
    ProjectTemplate, TemplateInfo, TemplateParams, list_templates, new_from_template,
};
use stencilcraft_doc::{
    CutStyle, Group, GroupChild, Guide, GuideAxis, Orientation, Page, PagePreset, PageSetup,
};

use super::{
    Alignment, DistributionAxis, Effect, Engine, EngineError, HistoryEntry, POINTS_PER_MM,
    path_element_is_finite, transform_path, unique_ids,
};

/// Largest page side accepted, millimetres (100 m).
const MAX_PAGE_MM: f64 = 100_000.0;
/// Space left between a new page and the pages before it, millimetres.
const PAGE_GAP_MM: f64 = 10.0;
/// Largest offset, gap or position a command accepts, millimetres.
const MAX_DISTANCE_MM: f64 = 1_000_000.0;
/// Most shapes the clipboard holds.
const MAX_CLIPBOARD_SHAPES: usize = 10_000;
/// Thumbnail budget for `document.update-thumbnail`, bytes.
const THUMBNAIL_BYTES: usize = 64 * 1024;

/// Commands of this module. Serialised exactly like [`super::Command`] (`{"command":
/// "group.create", "params": {...}}`); `Command::Document` carries them untagged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", content = "params")]
pub enum DocumentCommand {
    /// Group shapes `ids` and groups `groups` (two or more members, all directly in the same
    /// parent group or all top-level). The new group takes their place in that parent.
    #[serde(rename = "group.create")]
    GroupCreate {
        #[serde(default)]
        ids: Vec<u64>,
        #[serde(default)]
        groups: Vec<u64>,
        #[serde(default)]
        name: String,
    },
    /// Dissolve group `id`; its members move up to its parent. Answers the shapes it held.
    #[serde(rename = "group.ungroup")]
    GroupUngroup { id: u64 },
    #[serde(rename = "group.rename")]
    GroupRename { id: u64, name: String },
    /// Query: every shape inside group `id`, nested groups included.
    #[serde(rename = "group.members")]
    GroupMembers { id: u64 },
    /// Query: the outermost group holding shape `id` and all its shapes (what a click on
    /// the shape selects); just the shape when it is not grouped.
    #[serde(rename = "group.of")]
    GroupOf { id: u64 },
    /// Change stacking order within each shape's layer.
    #[serde(rename = "shapes.arrange")]
    ShapesArrange { ids: Vec<u64>, order: ArrangeOrder },
    /// Move shapes to the top of layer `layer`; they take that layer's operation.
    #[serde(rename = "shapes.move-to-layer")]
    ShapesMoveToLayer { ids: Vec<u64>, layer: usize },
    /// Move layer `from` to position `to` (both indices into the current list).
    #[serde(rename = "layer.move")]
    LayerMove { from: usize, to: usize },
    #[serde(rename = "layer.rename")]
    LayerRename { index: usize, name: String },
    /// Lock or unlock a layer: its shapes cannot be selected or edited while locked.
    #[serde(rename = "layer.set-locked")]
    LayerSetLocked { index: usize, locked: bool },
    /// Make a layer a template (guide) layer: shown, never exported or cut.
    #[serde(rename = "layer.set-template")]
    LayerSetTemplate { index: usize, template: bool },
    /// Set or clear (`null`) a layer's material colour, `#rrggbb`.
    #[serde(rename = "layer.set-color")]
    LayerSetColor { index: usize, color: Option<String> },
    /// Color Sync: every coloured, non-template layer within `tolerance` (RGB distance,
    /// 0-442) of an earlier one takes its colour. With `merge`, layers that then share
    /// colour and operation are merged into the first.
    #[serde(rename = "layer.color-sync")]
    LayerColorSync {
        tolerance: f64,
        #[serde(default)]
        merge: bool,
    },
    /// Add a page beside the existing ones. Size comes from `preset`, or from `width_mm`
    /// and `height_mm` for `custom` (which also override a preset); `orientation` swaps
    /// sides. Answers the new page index.
    #[serde(rename = "page.add")]
    PageAdd(PageRequest),
    /// Change page `index` (0 is the main board); fields left out keep their value, an
    /// empty string clears a label.
    #[serde(rename = "page.set")]
    PageSet {
        index: usize,
        #[serde(flatten)]
        request: PageRequest,
    },
    /// Remove extra page `index` (≥ 1); its shapes stay where they are.
    #[serde(rename = "page.remove")]
    PageRemove { index: usize },
    #[serde(rename = "guide.add")]
    GuideAdd { axis: GuideAxis, position_mm: f64 },
    #[serde(rename = "guide.move")]
    GuideMove { index: usize, position_mm: f64 },
    #[serde(rename = "guide.set-locked")]
    GuideSetLocked { index: usize, locked: bool },
    #[serde(rename = "guide.remove")]
    GuideRemove { index: usize },
    /// Remove every unlocked guide.
    #[serde(rename = "guide.clear")]
    GuideClear {},
    #[serde(rename = "grid.set")]
    GridSet {
        spacing_mm: f64,
        subdivisions: u32,
        #[serde(default)]
        visible: Option<bool>,
    },
    /// Change which targets snapping uses; fields left out keep their value.
    #[serde(rename = "snap.set")]
    SnapSet {
        #[serde(default)]
        grid: Option<bool>,
        #[serde(default)]
        guides: Option<bool>,
        #[serde(default)]
        objects: Option<bool>,
        #[serde(default)]
        spacing: Option<bool>,
        #[serde(default)]
        tolerance_mm: Option<f64>,
    },
    /// Query: snap a point or a moving box ([`snap`]).
    #[serde(rename = "snap.point")]
    SnapPoint(SnapQuery),
    /// Query: distance and angle from `from` to `to` ([`measure`]).
    #[serde(rename = "measure.distance")]
    MeasureDistance { from: [f64; 2], to: [f64; 2] },
    /// Query: combined bounds of shapes `ids`.
    #[serde(rename = "query.dimensions")]
    QueryDimensions { ids: Vec<u64> },
    /// Query: every selectable shape outside template layers.
    #[serde(rename = "select.all")]
    SelectAll {},
    /// Query: the empty selection.
    #[serde(rename = "select.none")]
    SelectNone {},
    /// Query: selectable shapes on layers whose colour is within `tolerance` of `color`.
    #[serde(rename = "select.by-color")]
    SelectByColor {
        color: String,
        #[serde(default)]
        tolerance: f64,
    },
    /// Query: selectable shapes with operation `style`, outside template layers.
    #[serde(rename = "select.by-operation")]
    SelectByOperation { style: CutStyle },
    /// Query: selectable shapes on layer `index`.
    #[serde(rename = "select.by-layer")]
    SelectByLayer { index: usize },
    /// Copy shapes (and the groups wholly inside them) to the engine clipboard. Not undoable.
    #[serde(rename = "clipboard.copy")]
    ClipboardCopy { ids: Vec<u64> },
    /// Copy, then remove the shapes.
    #[serde(rename = "clipboard.cut")]
    ClipboardCut { ids: Vec<u64> },
    /// Paste the clipboard: `offset` (default; moved right and down by `offset_mm`),
    /// `in-place` (same position, top of the layer) or `in-front` (same position, directly
    /// above each original).
    #[serde(rename = "clipboard.paste")]
    ClipboardPaste {
        #[serde(default)]
        mode: PasteMode,
        #[serde(default = "default_paste_offset")]
        offset_mm: f64,
    },
    /// Move, scale and rotate shapes, about their combined centre or (`each`) about each
    /// object's own centre; a group counts as one object. Remembered for `transform.again`.
    #[serde(rename = "transform.each")]
    TransformEach(TransformRequest),
    /// Repeat the last `transform.each` on shapes `ids`.
    #[serde(rename = "transform.again")]
    TransformAgain { ids: Vec<u64> },
    /// Align objects to the key shape `key` (which, with its group, stays still).
    #[serde(rename = "shapes.align-to-key")]
    ShapesAlignToKey {
        ids: Vec<u64>,
        key: u64,
        alignment: Alignment,
    },
    /// Space objects `gap_mm` apart along `axis`, keeping the first (leftmost/topmost).
    #[serde(rename = "shapes.distribute-spacing")]
    ShapesDistributeSpacing {
        ids: Vec<u64>,
        axis: DistributionAxis,
        gap_mm: f64,
    },
    /// Set the title and/or notes stored in the file.
    #[serde(rename = "document.set-meta")]
    DocumentSetMeta {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        notes: Option<String>,
    },
    /// Store a fresh SVG thumbnail of the main board in the file.
    #[serde(rename = "document.update-thumbnail")]
    DocumentUpdateThumbnail {},
    /// Query: the project templates `document.new-from-template` offers (id, name,
    /// description, category, params), for a new-project wizard.
    #[serde(rename = "templates.list")]
    TemplatesList {},
    /// Replace the document with a new one from a project template (keeps the kerf
    /// setting, like `document.new`; undoable).
    #[serde(rename = "document.new-from-template")]
    DocumentNewFromTemplate {
        template: ProjectTemplate,
        #[serde(default)]
        params: TemplateParams,
    },
}

fn default_paste_offset() -> f64 {
    5.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArrangeOrder {
    Front,
    Back,
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PasteMode {
    #[default]
    Offset,
    InPlace,
    InFront,
}

/// Page size and labels for `page.add` / `page.set`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PageRequest {
    #[serde(default)]
    pub preset: Option<PagePreset>,
    #[serde(default)]
    pub width_mm: Option<f64>,
    #[serde(default)]
    pub height_mm: Option<f64>,
    #[serde(default)]
    pub orientation: Option<Orientation>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mat: Option<String>,
    #[serde(default)]
    pub media: Option<String>,
    /// Page colour, `#rrggbb`.
    #[serde(default)]
    pub color: Option<String>,
}

/// A transform for `transform.each`: scale and rotation about a centre, then a move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformRequest {
    #[serde(default)]
    pub ids: Vec<u64>,
    #[serde(default)]
    pub dx_mm: f64,
    #[serde(default)]
    pub dy_mm: f64,
    #[serde(default = "one")]
    pub scale_x: f64,
    #[serde(default = "one")]
    pub scale_y: f64,
    #[serde(default)]
    pub degrees: f64,
    /// About each object's own centre instead of the selection's.
    #[serde(default)]
    pub each: bool,
}

fn one() -> f64 {
    1.0
}

/// A snapping request. With zero `width`/`height`, (`x`, `y`) is a point; otherwise it is
/// the top-left (smallest x and y) corner of a moving box whose edges and centre snap.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SnapQuery {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    /// Shapes not to snap to (usually the ones being moved).
    #[serde(default)]
    pub exclude: Vec<u64>,
    /// Overrides the document's tolerance (the UI converts a screen distance at its zoom).
    #[serde(default)]
    pub tolerance_mm: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapKind {
    Grid,
    Guide,
    Edge,
    Center,
    Page,
    Spacing,
}

/// What one axis snapped to: `value_mm` is the coordinate matched.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SnapTarget {
    pub kind: SnapKind,
    pub value_mm: f64,
}

/// A line segment for the UI to draw while snapping (alignment or spacing marker).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SnapLine {
    pub kind: SnapKind,
    pub from: [f64; 2],
    pub to: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SnapResult {
    /// The snapped point (or box corner).
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    pub snapped_x: Option<SnapTarget>,
    pub snapped_y: Option<SnapTarget>,
    pub lines: Vec<SnapLine>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measurement {
    pub distance_mm: f64,
    pub dx_mm: f64,
    pub dy_mm: f64,
    /// Angle of the line from `from` to `to` in document coordinates: degrees from +x
    /// towards +y, in (-180, 180].
    pub angle_degrees: f64,
}

/// Results of this module's commands.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DocumentEffect {
    /// Document or canvas settings changed (layers, pages, guides, grid, metadata).
    Changed,
    GroupCreated {
        id: u64,
    },
    /// The outermost group of a shape (if any) and the shapes a click selects.
    GroupOf {
        group: Option<u64>,
        ids: Vec<u64>,
    },
    PageAdded {
        index: usize,
    },
    GuideAdded {
        index: usize,
    },
    GuidesCleared {
        removed: usize,
    },
    Snap(SnapResult),
    Measure(Measurement),
    Dimensions {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    Copied {
        count: usize,
    },
    Templates(Vec<TemplateInfo>),
}

/// Per-engine state that is not part of the document or its history.
#[derive(Debug, Clone, Default)]
pub struct EditorState {
    clipboard: Option<Clip>,
    last_transform: Option<TransformRequest>,
}

#[derive(Debug, Clone)]
struct Clip {
    /// (layer index, shape) in document order.
    shapes: Vec<(usize, stencilcraft_doc::Shape)>,
    /// Groups wholly inside the copied shapes.
    groups: Vec<Group>,
}

fn invalid(message: String) -> EngineError {
    EngineError::Invalid(message)
}

/// Entry point from [`Engine::run`].
pub(crate) fn run(engine: &mut Engine, command: DocumentCommand) -> Result<Effect, EngineError> {
    if let Some(result) = query(engine, &command) {
        return result;
    }
    if let DocumentCommand::ClipboardCopy { ids } = &command {
        let count = copy(engine, ids, "clipboard.copy")?;
        return Ok(Effect::Document(DocumentEffect::Copied { count }));
    }
    let before = engine.capture_history_entry(false, true);
    let state = engine.document_state.clone();
    let result = apply(engine, command).and_then(|effect| {
        engine.doc.groups = engine.doc.pruned_groups();
        engine.doc.validate().map_err(|e| invalid(e.to_string()))?;
        Ok(effect)
    });
    match result {
        Ok(effect) => {
            engine.sync_session_from_doc();
            engine.history.rebase_to_snapshots();
            engine.history.push_snapshot(HistoryEntry {
                vectorcraft: false,
                rebuild_session: true,
                ..before
            });
            Ok(effect)
        }
        Err(error) => {
            // Only the document and this module's state were touched: restore both.
            engine.doc = before.snapshot;
            engine.restore_mappings(before.mappings);
            engine.document_state = state;
            Err(error)
        }
    }
}

fn selection(ids: Vec<u64>) -> Result<Effect, EngineError> {
    Ok(Effect::Selection { ids })
}

/// Shapes a selection may contain: on a visible, unlocked layer, not hidden or locked.
fn selectable(engine: &Engine, include_templates: bool) -> Vec<(usize, u64)> {
    engine
        .doc
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.visible && !l.locked && (include_templates || !l.template))
        .flat_map(|(i, l)| {
            l.shapes
                .iter()
                .filter(|s| !s.hidden && !s.locked)
                .map(move |s| (i, s.id))
        })
        .collect()
}

fn query(engine: &Engine, command: &DocumentCommand) -> Option<Result<Effect, EngineError>> {
    let doc = &engine.doc;
    Some(match command {
        DocumentCommand::GroupMembers { id } => match doc.group(*id) {
            Some(_) => selection(doc.group_shapes(*id)),
            None => Err(invalid(format!("group.members: no group has id {id}"))),
        },
        DocumentCommand::GroupOf { id } => {
            if doc.get_shape(*id).is_none() {
                return Some(Err(invalid(format!("group.of: no shape has id {id}"))));
            }
            let group = doc.group_chain(GroupChild::Shape(*id)).first().copied();
            let ids = group.map_or_else(|| vec![*id], |g| doc.group_shapes(g));
            Ok(Effect::Document(DocumentEffect::GroupOf { group, ids }))
        }
        DocumentCommand::SnapPoint(q) => {
            snap(doc, q).map(|r| Effect::Document(DocumentEffect::Snap(r)))
        }
        DocumentCommand::MeasureDistance { from, to } => {
            measure(*from, *to).map(|m| Effect::Document(DocumentEffect::Measure(m)))
        }
        DocumentCommand::QueryDimensions { ids } => units_bounds(engine, ids, "query.dimensions")
            .map(|r| {
                Effect::Document(DocumentEffect::Dimensions {
                    x: r.x0,
                    y: r.y0,
                    width: r.width(),
                    height: r.height(),
                })
            }),
        DocumentCommand::SelectAll {} => selection(
            selectable(engine, false)
                .into_iter()
                .map(|(_, id)| id)
                .collect(),
        ),
        DocumentCommand::SelectNone {} => selection(Vec::new()),
        DocumentCommand::TemplatesList {} => Ok(Effect::Document(DocumentEffect::Templates(
            list_templates(),
        ))),
        DocumentCommand::SelectByColor { color, tolerance } => {
            let Some(rgb) = parse_hex_color(color) else {
                return Some(Err(invalid(
                    "select.by-color: color must be #rrggbb".to_string(),
                )));
            };
            if !(0.0..=442.0).contains(tolerance) {
                return Some(Err(invalid(
                    "select.by-color: tolerance must be 0-442".to_string(),
                )));
            }
            let near = |li: usize| {
                doc.layers
                    .get(li)
                    .and_then(|l| l.color.as_deref())
                    .and_then(parse_hex_color)
                    .is_some_and(|c| color_distance(c, rgb) <= *tolerance)
            };
            selection(
                selectable(engine, false)
                    .into_iter()
                    .filter(|(li, _)| near(*li))
                    .map(|(_, id)| id)
                    .collect(),
            )
        }
        DocumentCommand::SelectByOperation { style } => selection(
            selectable(engine, false)
                .into_iter()
                .filter(|(_, id)| doc.get_shape(*id).is_some_and(|s| s.style == *style))
                .map(|(_, id)| id)
                .collect(),
        ),
        DocumentCommand::SelectByLayer { index } => {
            if *index >= doc.layers.len() {
                return Some(Err(invalid(format!(
                    "select.by-layer: no layer {index} (document has {})",
                    doc.layers.len()
                ))));
            }
            selection(
                selectable(engine, true)
                    .into_iter()
                    .filter(|(li, _)| li == index)
                    .map(|(_, id)| id)
                    .collect(),
            )
        }
        _ => return None,
    })
}

fn layer_index(engine: &Engine, index: usize, command: &str) -> Result<usize, EngineError> {
    if index < engine.doc.layers.len() {
        Ok(index)
    } else {
        Err(invalid(format!(
            "{command}: no layer {index} (document has {})",
            engine.doc.layers.len()
        )))
    }
}

/// Unique, existing, unlocked shape ids.
fn editable_ids(engine: &Engine, ids: &[u64], command: &str) -> Result<Vec<u64>, EngineError> {
    let ids = unique_ids(ids, command)?;
    for id in &ids {
        if engine.doc.get_shape(*id).is_none() {
            return Err(invalid(format!("{command}: no shape has id {id}")));
        }
        if engine.doc.shape_locked(*id) {
            return Err(invalid(format!(
                "{command}: shape {id} is locked; unlock it first"
            )));
        }
    }
    Ok(ids)
}

fn apply(engine: &mut Engine, command: DocumentCommand) -> Result<Effect, EngineError> {
    let changed = Ok(Effect::Document(DocumentEffect::Changed));
    match command {
        DocumentCommand::GroupCreate { ids, groups, name } => {
            group_create(engine, &ids, &groups, name)
        }
        DocumentCommand::GroupUngroup { id } => {
            let doc = &mut engine.doc;
            let shapes = doc.group_shapes(id);
            let Some(position) = doc.groups.iter().position(|g| g.id == id) else {
                return Err(invalid(format!("group.ungroup: no group has id {id}")));
            };
            let group = doc.groups.remove(position);
            if let Some(parent) = doc
                .groups
                .iter_mut()
                .find(|g| g.children.contains(&GroupChild::Group(id)))
            {
                let at = parent
                    .children
                    .iter()
                    .position(|c| *c == GroupChild::Group(id))
                    .unwrap_or(parent.children.len());
                parent.children.splice(at..=at, group.children);
            }
            selection(shapes)
        }
        DocumentCommand::GroupRename { id, name } => {
            let Some(group) = engine.doc.groups.iter_mut().find(|g| g.id == id) else {
                return Err(invalid(format!("group.rename: no group has id {id}")));
            };
            group.name = name;
            changed
        }
        DocumentCommand::ShapesArrange { ids, order } => {
            let ids = editable_ids(engine, &ids, "shapes.arrange")?;
            let selected: BTreeSet<u64> = ids.iter().copied().collect();
            for layer in &mut engine.doc.layers {
                arrange(&mut layer.shapes, &selected, order);
            }
            Ok(Effect::ShapesEdited { ids })
        }
        DocumentCommand::ShapesMoveToLayer { ids, layer } => {
            let ids = editable_ids(engine, &ids, "shapes.move-to-layer")?;
            let target = layer_index(engine, layer, "shapes.move-to-layer")?;
            let doc = &mut engine.doc;
            if doc.layers.get(target).is_some_and(|l| l.locked) {
                return Err(invalid(format!(
                    "shapes.move-to-layer: layer {target} is locked"
                )));
            }
            let selected: BTreeSet<u64> = ids.iter().copied().collect();
            let mut moved = Vec::new();
            for l in &mut doc.layers {
                let (take, keep): (Vec<_>, Vec<_>) =
                    l.shapes.drain(..).partition(|s| selected.contains(&s.id));
                l.shapes = keep;
                moved.extend(take);
            }
            let Some(l) = doc.layers.get_mut(target) else {
                return Err(EngineError::Internal("layer disappeared".to_string()));
            };
            for mut shape in moved {
                shape.style = l.kind;
                l.shapes.push(shape);
            }
            Ok(Effect::ShapesStyled { ids })
        }
        DocumentCommand::LayerMove { from, to } => {
            let from = layer_index(engine, from, "layer.move")?;
            let to = layer_index(engine, to, "layer.move")?;
            let layer = engine.doc.layers.remove(from);
            engine.doc.layers.insert(to, layer);
            changed
        }
        DocumentCommand::LayerRename { index, name } => {
            let index = layer_index(engine, index, "layer.rename")?;
            if name.trim().is_empty() {
                return Err(invalid("layer.rename: name cannot be empty".to_string()));
            }
            if let Some(l) = engine.doc.layers.get_mut(index) {
                l.name = name;
            }
            changed
        }
        DocumentCommand::LayerSetLocked { index, locked } => {
            let index = layer_index(engine, index, "layer.set-locked")?;
            if let Some(l) = engine.doc.layers.get_mut(index) {
                l.locked = locked;
            }
            changed
        }
        DocumentCommand::LayerSetTemplate { index, template } => {
            let index = layer_index(engine, index, "layer.set-template")?;
            if let Some(l) = engine.doc.layers.get_mut(index) {
                l.template = template;
            }
            changed
        }
        DocumentCommand::LayerSetColor { index, color } => {
            let index = layer_index(engine, index, "layer.set-color")?;
            let color = match color {
                None => None,
                Some(text) => Some(format_hex_color(parse_hex_color(&text).ok_or_else(
                    || invalid("layer.set-color: color must be #rrggbb".to_string()),
                )?)),
            };
            if let Some(l) = engine.doc.layers.get_mut(index) {
                l.color = color;
            }
            changed
        }
        DocumentCommand::LayerColorSync { tolerance, merge } => {
            color_sync(engine, tolerance, merge)
        }
        DocumentCommand::PageAdd(request) => {
            if engine.doc.pages.len() + 1 >= canvas::MAX_PAGES {
                return Err(invalid(format!(
                    "page.add: a document holds at most {} pages",
                    canvas::MAX_PAGES
                )));
            }
            let (width_mm, height_mm) = page_size(&request, None, "page.add")?;
            let x_mm = engine
                .doc
                .all_pages()
                .iter()
                .map(|p| p.x_mm + p.width_mm)
                .fold(0.0, f64::max)
                + PAGE_GAP_MM;
            let mut setup = PageSetup::default();
            apply_labels(&mut setup, &request, "page.add")?;
            engine.doc.pages.push(Page {
                x_mm,
                y_mm: 0.0,
                width_mm,
                height_mm,
                setup,
            });
            Ok(Effect::Document(DocumentEffect::PageAdded {
                index: engine.doc.pages.len(),
            }))
        }
        DocumentCommand::PageSet { index, request } => {
            let doc = &mut engine.doc;
            let current = doc.all_pages();
            let Some(page) = current.get(index) else {
                return Err(invalid(format!(
                    "page.set: no page {index} (document has {})",
                    current.len()
                )));
            };
            let size = page_size(&request, Some((page.width_mm, page.height_mm)), "page.set")?;
            let (setup, (w, h)) = if index == 0 {
                (
                    &mut doc.board_setup,
                    (&mut doc.board.width_mm, &mut doc.board.height_mm),
                )
            } else {
                let Some(p) = doc.pages.get_mut(index - 1) else {
                    return Err(EngineError::Internal("page disappeared".to_string()));
                };
                (&mut p.setup, (&mut p.width_mm, &mut p.height_mm))
            };
            (*w, *h) = size;
            apply_labels(setup, &request, "page.set")?;
            changed
        }
        DocumentCommand::PageRemove { index } => {
            if index == 0 {
                return Err(invalid(
                    "page.remove: page 0 is the main board and cannot be removed".to_string(),
                ));
            }
            if index > engine.doc.pages.len() {
                return Err(invalid(format!("page.remove: no page {index}")));
            }
            engine.doc.pages.remove(index - 1);
            changed
        }
        DocumentCommand::GuideAdd { axis, position_mm } => {
            finite_distance(position_mm, "guide.add: position")?;
            if engine.doc.guides.len() >= canvas::MAX_GUIDES {
                return Err(invalid(format!(
                    "guide.add: a document holds at most {} guides",
                    canvas::MAX_GUIDES
                )));
            }
            engine.doc.guides.push(Guide {
                axis,
                position_mm,
                locked: false,
            });
            Ok(Effect::Document(DocumentEffect::GuideAdded {
                index: engine.doc.guides.len() - 1,
            }))
        }
        DocumentCommand::GuideMove { index, position_mm } => {
            finite_distance(position_mm, "guide.move: position")?;
            guide_mut(engine, index, "guide.move", true)?.position_mm = position_mm;
            changed
        }
        DocumentCommand::GuideSetLocked { index, locked } => {
            guide_mut(engine, index, "guide.set-locked", false)?.locked = locked;
            changed
        }
        DocumentCommand::GuideRemove { index } => {
            guide_mut(engine, index, "guide.remove", true)?;
            engine.doc.guides.remove(index);
            changed
        }
        DocumentCommand::GuideClear {} => {
            let before = engine.doc.guides.len();
            engine.doc.guides.retain(|g| g.locked);
            Ok(Effect::Document(DocumentEffect::GuidesCleared {
                removed: before - engine.doc.guides.len(),
            }))
        }
        DocumentCommand::GridSet {
            spacing_mm,
            subdivisions,
            visible,
        } => {
            let grid = &mut engine.doc.grid;
            grid.spacing_mm = spacing_mm;
            grid.subdivisions = subdivisions;
            if let Some(v) = visible {
                grid.visible = v;
            }
            // `validate` (run after every edit) enforces the ranges.
            changed
        }
        DocumentCommand::SnapSet {
            grid,
            guides,
            objects,
            spacing,
            tolerance_mm,
        } => {
            let snap = &mut engine.doc.snap;
            for (field, value) in [
                (&mut snap.grid, grid),
                (&mut snap.guides, guides),
                (&mut snap.objects, objects),
                (&mut snap.spacing, spacing),
            ] {
                if let Some(v) = value {
                    *field = v;
                }
            }
            if let Some(t) = tolerance_mm {
                snap.tolerance_mm = t;
            }
            changed
        }
        DocumentCommand::ClipboardCut { ids } => {
            let ids = editable_ids(engine, &ids, "clipboard.cut")?;
            copy(engine, &ids, "clipboard.cut")?;
            let selected: BTreeSet<u64> = ids.iter().copied().collect();
            for layer in &mut engine.doc.layers {
                layer.shapes.retain(|s| !selected.contains(&s.id));
            }
            Ok(Effect::ShapesRemoved { ids })
        }
        DocumentCommand::ClipboardPaste { mode, offset_mm } => paste(engine, mode, offset_mm),
        DocumentCommand::TransformEach(request) => transform(engine, request),
        DocumentCommand::TransformAgain { ids } => {
            let Some(last) = engine.document_state.last_transform.clone() else {
                return Err(invalid(
                    "transform.again: no transform to repeat yet".to_string(),
                ));
            };
            transform(engine, TransformRequest { ids, ..last })
        }
        DocumentCommand::ShapesAlignToKey {
            ids,
            key,
            alignment,
        } => align_to_key(engine, &ids, key, alignment),
        DocumentCommand::ShapesDistributeSpacing { ids, axis, gap_mm } => {
            distribute_spacing(engine, &ids, axis, gap_mm)
        }
        DocumentCommand::DocumentSetMeta { title, notes } => {
            if let Some(title) = title {
                engine.doc.meta.title = title;
            }
            if let Some(notes) = notes {
                engine.doc.meta.notes = notes;
            }
            changed
        }
        DocumentCommand::DocumentUpdateThumbnail {} => {
            engine.doc.meta.thumbnail_svg = Some(engine.doc.render_thumbnail(THUMBNAIL_BYTES));
            changed
        }
        DocumentCommand::DocumentNewFromTemplate { template, params } => {
            let mut doc =
                new_from_template(template, &params).map_err(|e| invalid(e.to_string()))?;
            doc.kerf = engine.doc.kerf.clone();
            engine.doc = doc;
            Ok(Effect::DocumentReset)
        }
        DocumentCommand::GroupMembers { .. }
        | DocumentCommand::GroupOf { .. }
        | DocumentCommand::SnapPoint(_)
        | DocumentCommand::MeasureDistance { .. }
        | DocumentCommand::QueryDimensions { .. }
        | DocumentCommand::SelectAll {}
        | DocumentCommand::SelectNone {}
        | DocumentCommand::TemplatesList {}
        | DocumentCommand::SelectByColor { .. }
        | DocumentCommand::SelectByOperation { .. }
        | DocumentCommand::SelectByLayer { .. }
        | DocumentCommand::ClipboardCopy { .. } => Err(EngineError::Internal(
            "a query reached the edit path".to_string(),
        )),
    }
}

fn group_create(
    engine: &mut Engine,
    ids: &[u64],
    groups: &[u64],
    name: String,
) -> Result<Effect, EngineError> {
    const NAME: &str = "group.create";
    let doc = &mut engine.doc;
    let mut members: Vec<GroupChild> = Vec::new();
    for id in ids {
        if doc.get_shape(*id).is_none() {
            return Err(invalid(format!("{NAME}: no shape has id {id}")));
        }
        members.push(GroupChild::Shape(*id));
    }
    for id in groups {
        if doc.group(*id).is_none() {
            return Err(invalid(format!("{NAME}: no group has id {id}")));
        }
        members.push(GroupChild::Group(*id));
    }
    if members.len() < 2 || BTreeSet::from_iter(members.iter()).len() != members.len() {
        return Err(invalid(format!(
            "{NAME}: expected two or more distinct shapes or groups"
        )));
    }
    let parents: BTreeSet<Option<u64>> = members.iter().map(|m| doc.group_parent(*m)).collect();
    let [parent] = parents.into_iter().collect::<Vec<_>>()[..] else {
        return Err(invalid(format!(
            "{NAME}: members sit in different groups; group their outer groups instead"
        )));
    };
    let id = doc.next_group_id();
    if let Some(parent) = parent.and_then(|p| doc.groups.iter_mut().find(|g| g.id == p)) {
        let at = parent
            .children
            .iter()
            .position(|c| members.contains(c))
            .unwrap_or(parent.children.len());
        parent.children.retain(|c| !members.contains(c));
        parent
            .children
            .insert(at.min(parent.children.len()), GroupChild::Group(id));
    }
    doc.groups.push(Group {
        id,
        name,
        children: members,
    });
    Ok(Effect::Document(DocumentEffect::GroupCreated { id }))
}

/// Reorders one layer's shapes; `selected` may name shapes of other layers.
fn arrange(
    shapes: &mut Vec<stencilcraft_doc::Shape>,
    selected: &BTreeSet<u64>,
    order: ArrangeOrder,
) {
    let picked = |s: &stencilcraft_doc::Shape| selected.contains(&s.id);
    match order {
        ArrangeOrder::Front | ArrangeOrder::Back => {
            let (sel, rest): (Vec<_>, Vec<_>) = shapes.drain(..).partition(picked);
            *shapes = if order == ArrangeOrder::Front {
                rest.into_iter().chain(sel).collect()
            } else {
                sel.into_iter().chain(rest).collect()
            };
        }
        // One step past the nearest unselected neighbour; a selected block moves as one.
        ArrangeOrder::Forward => {
            for i in (0..shapes.len().saturating_sub(1)).rev() {
                if picked(&shapes[i]) && !picked(&shapes[i + 1]) {
                    shapes.swap(i, i + 1);
                }
            }
        }
        ArrangeOrder::Backward => {
            for i in 1..shapes.len() {
                if picked(&shapes[i]) && !picked(&shapes[i - 1]) {
                    shapes.swap(i, i - 1);
                }
            }
        }
    }
}

fn color_sync(engine: &mut Engine, tolerance: f64, merge: bool) -> Result<Effect, EngineError> {
    if !(0.0..=442.0).contains(&tolerance) {
        return Err(invalid(
            "layer.color-sync: tolerance must be 0-442 (RGB distance)".to_string(),
        ));
    }
    let doc = &mut engine.doc;
    let mut representatives: Vec<[u8; 3]> = Vec::new();
    for layer in doc.layers.iter_mut().filter(|l| !l.template) {
        let Some(rgb) = layer.color.as_deref().and_then(parse_hex_color) else {
            continue;
        };
        match representatives
            .iter()
            .find(|r| color_distance(**r, rgb) <= tolerance)
        {
            Some(r) => layer.color = Some(format_hex_color(*r)),
            None => representatives.push(rgb),
        }
    }
    if merge {
        let mut i = 0;
        while i < doc.layers.len() {
            let key = |l: &stencilcraft_doc::Layer| {
                (!l.template && !l.locked && l.color.is_some()).then(|| (l.color.clone(), l.kind))
            };
            let Some(target) = doc.layers.get(i).and_then(key) else {
                i += 1;
                continue;
            };
            let mut j = i + 1;
            while j < doc.layers.len() {
                if doc.layers.get(j).and_then(key) == Some(target.clone()) {
                    let merged = doc.layers.remove(j);
                    if let Some(into) = doc.layers.get_mut(i) {
                        into.shapes.extend(merged.shapes);
                        into.visible |= merged.visible;
                    }
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
    Ok(Effect::Document(DocumentEffect::Changed))
}

fn positive_mm(value: f64, what: &str) -> Result<f64, EngineError> {
    if value.is_finite() && value > 0.0 && value <= MAX_PAGE_MM {
        Ok(value)
    } else {
        Err(invalid(format!(
            "{what}: {value} is not a positive size up to {MAX_PAGE_MM} mm"
        )))
    }
}

fn finite_distance(value: f64, what: &str) -> Result<f64, EngineError> {
    if value.is_finite() && value.abs() <= MAX_DISTANCE_MM {
        Ok(value)
    } else {
        Err(invalid(format!(
            "{what}: {value} is not a finite distance within {MAX_DISTANCE_MM} mm"
        )))
    }
}

/// The page size a request asks for; `current` is kept when it names no size.
fn page_size(
    request: &PageRequest,
    current: Option<(f64, f64)>,
    command: &str,
) -> Result<(f64, f64), EngineError> {
    let preset = request.preset.and_then(PagePreset::size_mm);
    let base = preset.or(current);
    let width = request.width_mm.or(base.map(|b| b.0));
    let height = request.height_mm.or(base.map(|b| b.1));
    let (Some(w), Some(h)) = (width, height) else {
        return Err(invalid(format!(
            "{command}: give a preset, or width_mm and height_mm"
        )));
    };
    let size = (
        positive_mm(w, &format!("{command}: width"))?,
        positive_mm(h, &format!("{command}: height"))?,
    );
    Ok(request.orientation.map_or(size, |o| o.apply(size)))
}

fn apply_labels(
    setup: &mut PageSetup,
    request: &PageRequest,
    command: &str,
) -> Result<(), EngineError> {
    let label = |value: &Option<String>| value.as_ref().map(|v| (!v.is_empty()).then(|| v.clone()));
    if let Some(name) = &request.name {
        setup.name = name.clone();
    }
    if let Some(mat) = label(&request.mat) {
        setup.mat = mat;
    } else if let Some(mat) = request.preset.and_then(PagePreset::mat_label) {
        setup.mat = Some(mat.to_string());
    }
    if let Some(media) = label(&request.media) {
        setup.media = media;
    }
    if let Some(color) = label(&request.color) {
        setup.color = match color {
            None => None,
            Some(text) => Some(format_hex_color(
                parse_hex_color(&text)
                    .ok_or_else(|| invalid(format!("{command}: color must be #rrggbb")))?,
            )),
        };
    }
    Ok(())
}

fn guide_mut<'a>(
    engine: &'a mut Engine,
    index: usize,
    command: &str,
    refuse_locked: bool,
) -> Result<&'a mut Guide, EngineError> {
    let count = engine.doc.guides.len();
    let guide = engine.doc.guides.get_mut(index).ok_or_else(|| {
        invalid(format!(
            "{command}: no guide {index} (document has {count})"
        ))
    })?;
    if refuse_locked && guide.locked {
        return Err(invalid(format!(
            "{command}: guide {index} is locked; unlock it first"
        )));
    }
    Ok(guide)
}

/// Copies shapes to the clipboard; returns how many.
fn copy(engine: &mut Engine, ids: &[u64], command: &str) -> Result<usize, EngineError> {
    let ids = unique_ids(ids, command)?;
    if ids.len() > MAX_CLIPBOARD_SHAPES {
        return Err(invalid(format!(
            "{command}: at most {MAX_CLIPBOARD_SHAPES} shapes at a time"
        )));
    }
    let doc = &engine.doc;
    if let Some(id) = ids.iter().find(|id| doc.get_shape(**id).is_none()) {
        return Err(invalid(format!("{command}: no shape has id {id}")));
    }
    let selected: BTreeSet<u64> = ids.iter().copied().collect();
    let shapes: Vec<_> = doc
        .layers
        .iter()
        .enumerate()
        .flat_map(|(li, l)| l.shapes.iter().map(move |s| (li, s)))
        .filter(|(_, s)| selected.contains(&s.id))
        .map(|(li, s)| (li, s.clone()))
        .collect();
    let whole: BTreeSet<u64> = doc
        .groups
        .iter()
        .filter(|g| {
            let members = doc.group_shapes(g.id);
            !members.is_empty() && members.iter().all(|s| selected.contains(s))
        })
        .map(|g| g.id)
        .collect();
    let groups = doc
        .groups
        .iter()
        .filter(|g| whole.contains(&g.id))
        .map(|g| Group {
            children: g
                .children
                .iter()
                .copied()
                .filter(|c| match c {
                    GroupChild::Shape(s) => selected.contains(s),
                    GroupChild::Group(g) => whole.contains(g),
                })
                .collect(),
            ..g.clone()
        })
        .collect();
    let count = shapes.len();
    engine.document_state.clipboard = Some(Clip { shapes, groups });
    Ok(count)
}

fn paste(engine: &mut Engine, mode: PasteMode, offset_mm: f64) -> Result<Effect, EngineError> {
    const NAME: &str = "clipboard.paste";
    let Some(clip) = engine.document_state.clipboard.clone() else {
        return Err(invalid(format!("{NAME}: the clipboard is empty")));
    };
    let offset = match mode {
        PasteMode::Offset => finite_distance(offset_mm, "clipboard.paste: offset_mm")?,
        PasteMode::InPlace | PasteMode::InFront => 0.0,
    };
    let shift = Affine::translate((offset, offset));
    let doc = &mut engine.doc;
    let mut new_ids = Vec::with_capacity(clip.shapes.len());
    let mut remap = std::collections::BTreeMap::new();
    for (layer, shape) in &clip.shapes {
        let target = doc
            .layers
            .get(*layer)
            .filter(|l| !l.locked)
            .map(|_| *layer)
            .or_else(|| {
                doc.layers
                    .iter()
                    .rposition(|l| l.takes_new_shapes() && l.kind == shape.style)
            })
            .or_else(|| doc.layers.iter().rposition(|l| l.takes_new_shapes()))
            .ok_or_else(|| invalid(format!("{NAME}: no unlocked layer to paste into")))?;
        let id = doc.next_shape_id;
        doc.next_shape_id = id
            .checked_add(1)
            .ok_or_else(|| EngineError::Internal("shape ids exhausted".to_string()))?;
        let path = transform_path(&shape.path, shift);
        check_finite_path(&path, NAME)?;
        let Some(l) = doc.layers.get_mut(target) else {
            return Err(EngineError::Internal("layer disappeared".to_string()));
        };
        let mut copy = shape.clone();
        copy.id = id;
        copy.path = path;
        copy.style = l.kind;
        copy.locked = false;
        let above = (mode == PasteMode::InFront)
            .then(|| l.shapes.iter().position(|s| s.id == shape.id))
            .flatten();
        match above {
            // Directly above the original, after copies already placed there.
            Some(at) => {
                let at = l.shapes[at + 1..]
                    .iter()
                    .take_while(|s| remap.values().any(|v| *v == s.id))
                    .count()
                    + at
                    + 1;
                l.shapes.insert(at, copy);
            }
            None => l.shapes.push(copy),
        }
        remap.insert(shape.id, id);
        new_ids.push(id);
    }
    let mut group_ids = std::collections::BTreeMap::new();
    for group in &clip.groups {
        let id = doc.next_group_id().saturating_add(group_ids.len() as u64);
        group_ids.insert(group.id, id);
    }
    for group in &clip.groups {
        let children = group
            .children
            .iter()
            .filter_map(|c| match c {
                GroupChild::Shape(s) => remap.get(s).map(|n| GroupChild::Shape(*n)),
                GroupChild::Group(g) => group_ids.get(g).map(|n| GroupChild::Group(*n)),
            })
            .collect();
        doc.groups.push(Group {
            id: group_ids.get(&group.id).copied().unwrap_or_default(),
            name: group.name.clone(),
            children,
        });
    }
    Ok(Effect::ShapesAdded { ids: new_ids })
}

fn check_finite_path(path: &kurbo::BezPath, command: &str) -> Result<(), EngineError> {
    // VectorCraft stores points; the path must stay finite in both units.
    let ok = transform_path(path, Affine::scale(POINTS_PER_MM))
        .elements()
        .iter()
        .chain(path.elements())
        .all(path_element_is_finite);
    if ok {
        Ok(())
    } else {
        Err(invalid(format!(
            "{command}: the result lands outside the finite coordinate range"
        )))
    }
}

/// Selected shapes bundled into objects: a shape's outermost group is one object.
fn units(engine: &Engine, ids: &[u64], command: &str) -> Result<Vec<Vec<u64>>, EngineError> {
    let ids = editable_ids(engine, ids, command)?;
    let parents = engine.doc.group_parent_map();
    let mut out: Vec<(Option<u64>, Vec<u64>)> = Vec::new();
    for id in ids {
        let outer = canvas::group_chain_in(&parents, GroupChild::Shape(id))
            .first()
            .copied();
        match out.iter_mut().find(|(g, _)| outer.is_some() && *g == outer) {
            Some((_, members)) => members.push(id),
            None => out.push((outer, vec![id])),
        }
    }
    Ok(out.into_iter().map(|(_, members)| members).collect())
}

fn bounds_of(engine: &Engine, ids: &[u64], command: &str) -> Result<Rect, EngineError> {
    ids.iter()
        .filter_map(|id| {
            engine
                .doc
                .get_shape(*id)
                .and_then(stencilcraft_doc::Shape::bounds)
        })
        .reduce(|a, b| a.union(b))
        .ok_or_else(|| invalid(format!("{command}: the shapes have no bounds")))
}

fn units_bounds(engine: &Engine, ids: &[u64], command: &str) -> Result<Rect, EngineError> {
    let ids = unique_ids(ids, command)?;
    if let Some(id) = ids.iter().find(|id| engine.doc.get_shape(**id).is_none()) {
        return Err(invalid(format!("{command}: no shape has id {id}")));
    }
    bounds_of(engine, &ids, command)
}

fn apply_affine(
    engine: &mut Engine,
    ids: &[u64],
    affine: Affine,
    command: &str,
) -> Result<(), EngineError> {
    for id in ids {
        let shape = engine
            .doc
            .get_shape_mut(*id)
            .ok_or_else(|| invalid(format!("{command}: no shape has id {id}")))?;
        let path = transform_path(&shape.path, affine);
        check_finite_path(&path, command)?;
        shape.path = path;
    }
    Ok(())
}

fn transform(engine: &mut Engine, request: TransformRequest) -> Result<Effect, EngineError> {
    const NAME: &str = "transform.each";
    let scale_ok = |s: f64| s.is_finite() && (1e-6..=1e6).contains(&s.abs());
    if !(scale_ok(request.scale_x)
        && scale_ok(request.scale_y)
        && request.degrees.is_finite()
        && request.dx_mm.is_finite()
        && request.dy_mm.is_finite()
        && request.dx_mm.abs() <= MAX_DISTANCE_MM
        && request.dy_mm.abs() <= MAX_DISTANCE_MM)
    {
        return Err(invalid(format!(
            "{NAME}: moves must be finite, scales finite and non-zero (|s| 1e-6 to 1e6), degrees finite"
        )));
    }
    let objects = if request.each {
        units(engine, &request.ids, NAME)?
    } else {
        vec![editable_ids(engine, &request.ids, NAME)?]
    };
    for object in &objects {
        let c = bounds_of(engine, object, NAME)?.center();
        let affine = Affine::translate((request.dx_mm + c.x, request.dy_mm + c.y))
            * Affine::rotate(request.degrees.to_radians())
            * Affine::scale_non_uniform(request.scale_x, request.scale_y)
            * Affine::translate((-c.x, -c.y));
        apply_affine(engine, object, affine, NAME)?;
    }
    let ids = request.ids.clone();
    engine.document_state.last_transform = Some(TransformRequest {
        ids: Vec::new(),
        ..request
    });
    Ok(Effect::ShapesEdited { ids })
}

fn align_to_key(
    engine: &mut Engine,
    ids: &[u64],
    key: u64,
    alignment: Alignment,
) -> Result<Effect, EngineError> {
    const NAME: &str = "shapes.align-to-key";
    let objects = units(engine, ids, NAME)?;
    let Some(key_object) = objects.iter().find(|o| o.contains(&key)) else {
        return Err(invalid(format!(
            "{NAME}: key {key} must be one of the selected shapes"
        )));
    };
    let k = bounds_of(engine, key_object, NAME)?;
    for object in objects.iter().filter(|o| !o.contains(&key)) {
        let b = bounds_of(engine, object, NAME)?;
        let (dx, dy) = match alignment {
            Alignment::Left => (k.x0 - b.x0, 0.0),
            Alignment::HorizontalCenter => (k.center().x - b.center().x, 0.0),
            Alignment::Right => (k.x1 - b.x1, 0.0),
            Alignment::Top => (0.0, k.y0 - b.y0),
            Alignment::VerticalCenter => (0.0, k.center().y - b.center().y),
            Alignment::Bottom => (0.0, k.y1 - b.y1),
        };
        apply_affine(engine, object, Affine::translate((dx, dy)), NAME)?;
    }
    Ok(Effect::ShapesEdited { ids: ids.to_vec() })
}

fn distribute_spacing(
    engine: &mut Engine,
    ids: &[u64],
    axis: DistributionAxis,
    gap_mm: f64,
) -> Result<Effect, EngineError> {
    const NAME: &str = "shapes.distribute-spacing";
    finite_distance(gap_mm, "shapes.distribute-spacing: gap_mm")?;
    let mut objects = units(engine, ids, NAME)?
        .into_iter()
        .map(|o| Ok((bounds_of(engine, &o, NAME)?, o)))
        .collect::<Result<Vec<_>, EngineError>>()?;
    if objects.len() < 2 {
        return Err(invalid(format!("{NAME}: select two or more objects")));
    }
    let horizontal = axis == DistributionAxis::Horizontal;
    let lo = |r: &Rect| if horizontal { r.x0 } else { r.y0 };
    objects.sort_by(|a, b| lo(&a.0).total_cmp(&lo(&b.0)));
    let mut next = objects
        .first()
        .map(|(r, _)| if horizontal { r.x1 } else { r.y1 })
        .unwrap_or_default()
        + gap_mm;
    for (bounds, object) in objects.iter().skip(1) {
        let d = next - lo(bounds);
        let shift = if horizontal { (d, 0.0) } else { (0.0, d) };
        apply_affine(engine, object, Affine::translate(shift), NAME)?;
        next += if horizontal {
            bounds.width()
        } else {
            bounds.height()
        } + gap_mm;
    }
    Ok(Effect::ShapesEdited { ids: ids.to_vec() })
}

/// Distance and angle from `from` to `to` (millimetres, document coordinates).
pub fn measure(from: [f64; 2], to: [f64; 2]) -> Result<Measurement, EngineError> {
    if !from.iter().chain(&to).all(|v| v.is_finite()) {
        return Err(invalid(
            "measure.distance: points must be finite".to_string(),
        ));
    }
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let mut angle = dy.atan2(dx).to_degrees();
    if angle <= -180.0 {
        angle += 360.0;
    }
    Ok(Measurement {
        distance_mm: dx.hypot(dy),
        dx_mm: dx,
        dy_mm: dy,
        angle_degrees: angle,
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Axis {
    X,
    Y,
}

fn span(r: &Rect, axis: Axis) -> (f64, f64) {
    match axis {
        Axis::X => (r.x0, r.x1),
        Axis::Y => (r.y0, r.y1),
    }
}

/// A segment at main-axis coordinate pair (`a`, `b`) and cross coordinates (`c`, `d`).
fn line(axis: Axis, kind: SnapKind, (a, c): (f64, f64), (b, d): (f64, f64)) -> SnapLine {
    match axis {
        Axis::X => SnapLine {
            kind,
            from: [a, c],
            to: [b, d],
        },
        Axis::Y => SnapLine {
            kind,
            from: [c, a],
            to: [d, b],
        },
    }
}

struct Hit {
    offset: f64,
    target: SnapTarget,
    lines: Vec<SnapLine>,
}

/// Snaps a point or moving box to the grid, guides, other shapes' edges and centres, page
/// edges and centres, and equal spacing between neighbours, per the document's
/// [`stencilcraft_doc::SnapSettings`]. Each axis snaps independently to its nearest target
/// within the tolerance; ties go to guides, then objects and pages, then spacing, then grid.
/// A pure function of the document and the query.
pub fn snap(
    doc: &stencilcraft_doc::Document,
    query: &SnapQuery,
) -> Result<SnapResult, EngineError> {
    let settings = &doc.snap;
    let tolerance = query.tolerance_mm.unwrap_or(settings.tolerance_mm);
    let finite = [query.x, query.y, query.width, query.height, tolerance]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= MAX_DISTANCE_MM);
    if !finite || query.width < 0.0 || query.height < 0.0 || tolerance < 0.0 {
        return Err(invalid(
            "snap.point: position, size and tolerance must be finite, size and tolerance non-negative"
                .to_string(),
        ));
    }
    let moving = Rect::new(
        query.x,
        query.y,
        query.x + query.width,
        query.y + query.height,
    );
    let exclude: BTreeSet<u64> = query.exclude.iter().copied().collect();
    let others: Vec<Rect> = doc
        .layers
        .iter()
        .filter(|l| l.visible)
        .flat_map(|l| &l.shapes)
        .filter(|s| !s.hidden && !exclude.contains(&s.id))
        .filter_map(stencilcraft_doc::Shape::bounds)
        .collect();
    let pages: Vec<Rect> = doc.all_pages().iter().map(Page::rect).collect();
    let extent = pages
        .iter()
        .chain(&others)
        .fold(moving, |acc, r| acc.union(*r));
    let x = snap_axis(doc, Axis::X, moving, &others, &pages, extent, tolerance);
    let y = snap_axis(doc, Axis::Y, moving, &others, &pages, extent, tolerance);
    let dx = x.as_ref().map_or(0.0, |h| h.offset);
    let dy = y.as_ref().map_or(0.0, |h| h.offset);
    let mut lines = Vec::new();
    let mut targets = [None, None];
    for (slot, hit) in targets.iter_mut().zip([x, y]) {
        if let Some(hit) = hit {
            lines.extend(hit.lines);
            *slot = Some(hit.target);
        }
    }
    let [snapped_x, snapped_y] = targets;
    Ok(SnapResult {
        x: query.x + dx,
        y: query.y + dy,
        dx,
        dy,
        snapped_x,
        snapped_y,
        lines,
    })
}

fn snap_axis(
    doc: &stencilcraft_doc::Document,
    axis: Axis,
    moving: Rect,
    others: &[Rect],
    pages: &[Rect],
    extent: Rect,
    tolerance: f64,
) -> Option<Hit> {
    let settings = &doc.snap;
    let cross = match axis {
        Axis::X => Axis::Y,
        Axis::Y => Axis::X,
    };
    let (lo, hi) = span(&moving, axis);
    let (clo, chi) = span(&moving, cross);
    let refs: Vec<f64> = if hi > lo {
        vec![lo, (lo + hi) / 2.0, hi]
    } else {
        vec![lo]
    };
    let (elo, ehi) = span(&extent, cross);
    // Candidates in tie-break order; the nearest within tolerance wins, the first on ties.
    let mut hits: Vec<Hit> = Vec::new();
    let hit = |offset: f64, target: SnapTarget, lines: Vec<SnapLine>| Hit {
        offset,
        target,
        lines,
    };
    // A target at `value`: each reference may snap to it; the line spans `cross_span`.
    let at = |value: f64, kind: SnapKind, cross_span: (f64, f64)| {
        let (a, b) = (cross_span.0.min(clo), cross_span.1.max(chi));
        refs.iter()
            .map(|r| {
                hit(
                    value - r,
                    SnapTarget {
                        kind,
                        value_mm: value,
                    },
                    vec![line(axis, kind, (value, a), (value, b))],
                )
            })
            .collect::<Vec<_>>()
    };
    if settings.guides {
        let wanted = match axis {
            Axis::X => GuideAxis::Vertical,
            Axis::Y => GuideAxis::Horizontal,
        };
        for g in doc.guides.iter().filter(|g| g.axis == wanted) {
            hits.extend(at(g.position_mm, SnapKind::Guide, (elo, ehi)));
        }
    }
    if settings.objects {
        for (rects, edge, center) in [
            (others, SnapKind::Edge, SnapKind::Center),
            (pages, SnapKind::Page, SnapKind::Page),
        ] {
            for r in rects {
                let (a, b) = span(r, axis);
                let s = span(r, cross);
                hits.extend(at(a, edge, s));
                hits.extend(at((a + b) / 2.0, center, s));
                hits.extend(at(b, edge, s));
            }
        }
    }
    if settings.spacing && hi > lo {
        // Neighbours level with the moving box along the cross axis, in main-axis order.
        let mut row: Vec<(f64, f64)> = others
            .iter()
            .filter(|r| {
                let (a, b) = span(r, cross);
                a < chi && b > clo
            })
            .map(|r| span(r, axis))
            .collect();
        row.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mid = (clo.max(elo) + chi.min(ehi)) / 2.0;
        let w = hi - lo;
        for pair in row.windows(2) {
            let [(a0, a1), (b0, b1)] = [pair[0], pair[1]];
            let gap = b0 - a1;
            if gap < 0.0 {
                continue;
            }
            let marks = |segments: [(f64, f64); 2]| {
                segments
                    .iter()
                    .map(|(s, e)| line(axis, SnapKind::Spacing, (*s, mid), (*e, mid)))
                    .collect::<Vec<_>>()
            };
            let target = |value| SnapTarget {
                kind: SnapKind::Spacing,
                value_mm: value,
            };
            // After the pair, before it, or centred in its gap.
            let after = b1 + gap;
            hits.push(hit(
                after - lo,
                target(after),
                marks([(a1, b0), (b1, after)]),
            ));
            let before = a0 - gap;
            hits.push(hit(
                before - hi,
                target(before - w),
                marks([(before, a0), (a1, b0)]),
            ));
            if gap >= w {
                let start = a1 + (gap - w) / 2.0;
                hits.push(hit(
                    start - lo,
                    target(start),
                    marks([(a1, start), (start + w, b0)]),
                ));
            }
        }
    }
    if settings.grid {
        let step = doc.grid.step_mm();
        if step.is_finite() && step > 0.0 {
            for r in &refs {
                let value = (r / step).round() * step;
                hits.extend(at(value, SnapKind::Grid, (elo, ehi)));
            }
        }
    }
    let mut best: Option<Hit> = None;
    for h in hits {
        if h.offset.abs() <= tolerance
            && best
                .as_ref()
                .is_none_or(|b| h.offset.abs() < b.offset.abs())
        {
            best = Some(h);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::super::Command;
    use super::*;
    use stencilcraft_doc::Document;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> kurbo::BezPath {
        super::super::rect_path(x, y, w, h)
    }

    /// An engine with three 10 mm squares at x = 0, 20 and 40 (ids 1, 2, 3).
    fn three() -> Engine {
        let mut doc = Document::default();
        for x in [0.0, 20.0, 40.0] {
            doc.push_shape(0, rect(x, 0.0, 10.0, 10.0), CutStyle::Cut)
                .unwrap();
        }
        Engine::new(doc)
    }

    fn run(engine: &mut Engine, command: DocumentCommand) -> Effect {
        engine.run(Command::Document(command)).unwrap()
    }

    fn bounds(engine: &Engine, id: u64) -> Rect {
        engine.doc.get_shape(id).unwrap().bounds().unwrap()
    }

    fn order(engine: &Engine) -> Vec<u64> {
        engine.doc.layers[0].shapes.iter().map(|s| s.id).collect()
    }

    #[test]
    fn commands_round_trip_through_the_command_json() {
        let samples = [
            DocumentCommand::GroupCreate {
                ids: vec![1, 2],
                groups: vec![],
                name: "g".to_string(),
            },
            DocumentCommand::GuideClear {},
            DocumentCommand::PageAdd(PageRequest {
                preset: Some(PagePreset::A4),
                ..PageRequest::default()
            }),
            DocumentCommand::PageSet {
                index: 0,
                request: PageRequest {
                    width_mm: Some(100.0),
                    ..PageRequest::default()
                },
            },
            DocumentCommand::SnapPoint(SnapQuery::default()),
            DocumentCommand::TransformEach(TransformRequest {
                ids: vec![1],
                dx_mm: 1.0,
                dy_mm: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                degrees: 0.0,
                each: true,
            }),
        ];
        for command in samples {
            let wrapped = Command::Document(command);
            let json = serde_json::to_value(&wrapped).unwrap();
            assert!(json["command"].is_string(), "{json}");
            let back: Command = serde_json::from_value(json).unwrap();
            assert_eq!(back, wrapped);
        }
        // Old commands still parse to their own variants; MCP-style `{}` params work.
        let undo: Command =
            serde_json::from_value(serde_json::json!({"command": "edit.undo"})).unwrap();
        assert_eq!(undo, Command::Undo);
        let clear: Command =
            serde_json::from_value(serde_json::json!({"command": "guide.clear", "params": {}}))
                .unwrap();
        assert_eq!(clear, Command::Document(DocumentCommand::GuideClear {}));
        let paste: Command =
            serde_json::from_value(serde_json::json!({"command": "clipboard.paste", "params": {}}))
                .unwrap();
        assert_eq!(
            paste,
            Command::Document(DocumentCommand::ClipboardPaste {
                mode: PasteMode::Offset,
                offset_mm: 5.0
            })
        );
        assert!(
            serde_json::from_value::<Command>(serde_json::json!({"command": "no.such"})).is_err()
        );
    }

    #[test]
    fn groups_nest_ungroup_and_undo() {
        let mut e = three();
        let Effect::Document(DocumentEffect::GroupCreated { id: inner }) = run(
            &mut e,
            DocumentCommand::GroupCreate {
                ids: vec![1, 2],
                groups: vec![],
                name: "pair".to_string(),
            },
        ) else {
            panic!("no group");
        };
        let Effect::Document(DocumentEffect::GroupCreated { id: outer }) = run(
            &mut e,
            DocumentCommand::GroupCreate {
                ids: vec![3],
                groups: vec![inner],
                name: String::new(),
            },
        ) else {
            panic!("no group");
        };
        assert_eq!(
            run(&mut e, DocumentCommand::GroupOf { id: 1 }),
            Effect::Document(DocumentEffect::GroupOf {
                group: Some(outer),
                ids: vec![1, 2, 3]
            })
        );
        // Members in different parents are refused, and nothing changes.
        let err = e.run(Command::Document(DocumentCommand::GroupCreate {
            ids: vec![1, 3],
            groups: vec![],
            name: String::new(),
        }));
        assert!(err.is_err());
        // Save/load keeps the nesting; a vectorcraft edit keeps groups too.
        let back = Document::load_json(&e.doc.save_json().unwrap()).unwrap();
        assert_eq!(back.group_chain(GroupChild::Shape(1)), vec![outer, inner]);
        e.run(Command::ShapeMove {
            id: 1,
            dx_mm: 1.0,
            dy_mm: 0.0,
        })
        .unwrap();
        assert_eq!(e.doc.group_chain(GroupChild::Shape(1)), vec![outer, inner]);
        e.run(Command::Undo).unwrap();
        // Ungroup the outer group: the inner one becomes top-level.
        run(&mut e, DocumentCommand::GroupUngroup { id: outer });
        assert_eq!(e.doc.group_chain(GroupChild::Shape(1)), vec![inner]);
        assert_eq!(e.doc.group_chain(GroupChild::Shape(3)), Vec::<u64>::new());
        e.run(Command::Undo).unwrap();
        assert_eq!(e.doc.group_chain(GroupChild::Shape(1)), vec![outer, inner]);
        // Removing grouped shapes prunes the groups on the next save.
        e.run(Command::ShapesRemove { ids: vec![1, 2] }).unwrap();
        let back = Document::load_json(&e.doc.save_json().unwrap()).unwrap();
        assert_eq!(back.groups.len(), 1);
        assert_eq!(back.groups[0].children, vec![GroupChild::Shape(3)]);
    }

    #[test]
    fn arrange_orders_and_undo() {
        let mut e = three();
        let arrange = |e: &mut Engine, ids: Vec<u64>, order| {
            run(e, DocumentCommand::ShapesArrange { ids, order });
        };
        arrange(&mut e, vec![1], ArrangeOrder::Front);
        assert_eq!(order(&e), vec![2, 3, 1]);
        arrange(&mut e, vec![1], ArrangeOrder::Backward);
        assert_eq!(order(&e), vec![2, 1, 3]);
        arrange(&mut e, vec![2, 1], ArrangeOrder::Forward);
        assert_eq!(order(&e), vec![3, 2, 1]);
        arrange(&mut e, vec![1], ArrangeOrder::Back);
        assert_eq!(order(&e), vec![1, 3, 2]);
        e.run(Command::Undo).unwrap();
        assert_eq!(order(&e), vec![3, 2, 1]);
        // The VectorCraft session follows the new order: a later edit keeps it.
        e.run(Command::ShapeMove {
            id: 3,
            dx_mm: 1.0,
            dy_mm: 1.0,
        })
        .unwrap();
        assert_eq!(order(&e), vec![3, 2, 1]);
    }

    #[test]
    fn layers_move_rename_lock_template_and_shapes_change_layer() {
        let mut e = three();
        e.run(Command::LayerAdd {
            name: "score".to_string(),
            kind: CutStyle::Engrave,
        })
        .unwrap();
        run(
            &mut e,
            DocumentCommand::ShapesMoveToLayer {
                ids: vec![2],
                layer: 1,
            },
        );
        assert_eq!(e.doc.layers[1].shapes[0].id, 2);
        assert_eq!(e.doc.layers[1].shapes[0].style, CutStyle::Engrave);
        run(&mut e, DocumentCommand::LayerMove { from: 1, to: 0 });
        assert_eq!(e.doc.layers[0].name, "score");
        run(
            &mut e,
            DocumentCommand::LayerRename {
                index: 0,
                name: "Score lines".to_string(),
            },
        );
        assert_eq!(e.doc.layers[0].name, "Score lines");
        // A locked layer refuses edits from both command sets.
        run(
            &mut e,
            DocumentCommand::LayerSetLocked {
                index: 1,
                locked: true,
            },
        );
        assert!(
            e.run(Command::ShapeMove {
                id: 1,
                dx_mm: 1.0,
                dy_mm: 0.0
            })
            .is_err()
        );
        assert!(
            e.run(Command::Document(DocumentCommand::ShapesArrange {
                ids: vec![1],
                order: ArrangeOrder::Front
            }))
            .is_err()
        );
        let Effect::Selection { ids } = run(&mut e, DocumentCommand::SelectAll {}) else {
            panic!()
        };
        assert_eq!(ids, vec![2], "locked layer shapes are not selectable");
        // New shapes skip locked and template layers.
        run(
            &mut e,
            DocumentCommand::LayerSetTemplate {
                index: 0,
                template: true,
            },
        );
        let Effect::ShapeAdded { id } = e
            .run(Command::ShapeAddPath {
                path: rect(0.0, 50.0, 5.0, 5.0),
                style: CutStyle::Engrave,
            })
            .unwrap()
        else {
            panic!()
        };
        let layer = e.doc.layer_of_shape(id).unwrap();
        assert!(
            e.doc.layers[layer].takes_new_shapes(),
            "landed on layer {layer}"
        );
        let undo_count = (0..10).filter(|_| e.undo()).count();
        assert!(undo_count >= 6);
        assert_eq!(e.doc.layers.len(), 1);
        assert_eq!(order(&e), vec![1, 2, 3]);
        assert!(
            e.run(Command::Document(DocumentCommand::LayerRename {
                index: 5,
                name: "x".to_string()
            }))
            .is_err()
        );
    }

    #[test]
    fn templates_are_not_exported() {
        let mut e = three();
        e.run(Command::LayerAdd {
            name: "template".to_string(),
            kind: CutStyle::Cut,
        })
        .unwrap();
        run(
            &mut e,
            DocumentCommand::ShapesMoveToLayer {
                ids: vec![3],
                layer: 1,
            },
        );
        run(
            &mut e,
            DocumentCommand::LayerSetTemplate {
                index: 1,
                template: true,
            },
        );
        assert!(!e.doc.layers[1].is_output());
        let back = Document::load_json(&e.doc.save_json().unwrap()).unwrap();
        assert!(back.layers[1].template);
        let thumb = back.render_thumbnail(64 * 1024);
        assert_eq!(thumb.matches("<path").count(), 2);
    }

    #[test]
    fn color_sync_merges_similar_colours() {
        let mut e = three();
        for (name, color) in [("b", "#ff0505"), ("c", "#0000ff"), ("d", "#fa0000")] {
            e.run(Command::LayerAdd {
                name: name.to_string(),
                kind: CutStyle::Cut,
            })
            .unwrap();
            let index = e.doc.layers.len() - 1;
            run(
                &mut e,
                DocumentCommand::LayerSetColor {
                    index,
                    color: Some(color.to_string()),
                },
            );
        }
        run(
            &mut e,
            DocumentCommand::LayerSetColor {
                index: 0,
                color: Some("#FF0000".to_string()),
            },
        );
        assert_eq!(e.doc.layers[0].color.as_deref(), Some("#ff0000"));
        run(
            &mut e,
            DocumentCommand::ShapesMoveToLayer {
                ids: vec![3],
                layer: 3,
            },
        );
        run(
            &mut e,
            DocumentCommand::LayerColorSync {
                tolerance: 20.0,
                merge: false,
            },
        );
        let colors: Vec<_> = e
            .doc
            .layers
            .iter()
            .map(|l| l.color.clone().unwrap())
            .collect();
        assert_eq!(colors, ["#ff0000", "#ff0000", "#0000ff", "#ff0000"]);
        let Effect::Selection { ids } = run(
            &mut e,
            DocumentCommand::SelectByColor {
                color: "#ff0000".to_string(),
                tolerance: 0.0,
            },
        ) else {
            panic!()
        };
        assert_eq!(ids, vec![1, 2, 3]);
        e.run(Command::Undo).unwrap();
        run(
            &mut e,
            DocumentCommand::LayerColorSync {
                tolerance: 20.0,
                merge: true,
            },
        );
        assert_eq!(e.doc.layers.len(), 2);
        assert_eq!(e.doc.layers[0].shapes.len(), 3);
        assert!(
            e.run(Command::Document(DocumentCommand::LayerColorSync {
                tolerance: f64::NAN,
                merge: false
            }))
            .is_err()
        );
    }

    #[test]
    fn pages_add_set_remove_with_presets() {
        let mut e = three();
        let Effect::Document(DocumentEffect::PageAdded { index }) = run(
            &mut e,
            DocumentCommand::PageAdd(PageRequest {
                preset: Some(PagePreset::Mat12x24),
                orientation: Some(Orientation::Landscape),
                color: Some("#eeeeee".to_string()),
                ..PageRequest::default()
            }),
        ) else {
            panic!()
        };
        assert_eq!(index, 1);
        let page = &e.doc.pages[0];
        assert!((page.width_mm - 609.6).abs() < 1e-9 && (page.height_mm - 304.8).abs() < 1e-9);
        assert_eq!(
            page.x_mm, 110.0,
            "placed right of the 100 mm board with a gap"
        );
        assert_eq!(page.setup.mat.as_deref(), Some("12x24"));
        run(
            &mut e,
            DocumentCommand::PageSet {
                index: 0,
                request: PageRequest {
                    preset: Some(PagePreset::Letter),
                    media: Some("cardstock".to_string()),
                    ..PageRequest::default()
                },
            },
        );
        assert!((e.doc.board.width_mm - 215.9).abs() < 1e-9);
        assert_eq!(e.doc.board_setup.media.as_deref(), Some("cardstock"));
        run(
            &mut e,
            DocumentCommand::PageAdd(PageRequest {
                preset: Some(PagePreset::Custom),
                width_mm: Some(50.0),
                height_mm: Some(60.0),
                ..PageRequest::default()
            }),
        );
        let back = Document::load_json(&e.doc.save_json().unwrap()).unwrap();
        assert_eq!(back.pages, e.doc.pages);
        assert_eq!(back.board_setup, e.doc.board_setup);
        for bad in [
            DocumentCommand::PageRemove { index: 0 },
            DocumentCommand::PageRemove { index: 9 },
            DocumentCommand::PageAdd(PageRequest {
                preset: Some(PagePreset::Custom),
                ..PageRequest::default()
            }),
            DocumentCommand::PageAdd(PageRequest {
                width_mm: Some(-1.0),
                height_mm: Some(5.0),
                ..PageRequest::default()
            }),
            DocumentCommand::PageAdd(PageRequest {
                preset: Some(PagePreset::A4),
                color: Some("blue".to_string()),
                ..PageRequest::default()
            }),
        ] {
            assert!(e.run(Command::Document(bad)).is_err());
        }
        run(&mut e, DocumentCommand::PageRemove { index: 1 });
        assert_eq!(e.doc.pages.len(), 1);
        e.run(Command::Undo).unwrap();
        assert_eq!(e.doc.pages.len(), 2);
    }

    #[test]
    fn guides_and_grid() {
        let mut e = three();
        run(
            &mut e,
            DocumentCommand::GuideAdd {
                axis: GuideAxis::Vertical,
                position_mm: 33.0,
            },
        );
        run(
            &mut e,
            DocumentCommand::GuideAdd {
                axis: GuideAxis::Horizontal,
                position_mm: 5.0,
            },
        );
        run(
            &mut e,
            DocumentCommand::GuideMove {
                index: 0,
                position_mm: 34.0,
            },
        );
        run(
            &mut e,
            DocumentCommand::GuideSetLocked {
                index: 0,
                locked: true,
            },
        );
        assert!(
            e.run(Command::Document(DocumentCommand::GuideMove {
                index: 0,
                position_mm: 1.0
            }))
            .is_err()
        );
        assert_eq!(
            run(&mut e, DocumentCommand::GuideClear {}),
            Effect::Document(DocumentEffect::GuidesCleared { removed: 1 })
        );
        assert_eq!(e.doc.guides.len(), 1);
        assert_eq!(e.doc.guides[0].position_mm, 34.0);
        run(
            &mut e,
            DocumentCommand::GridSet {
                spacing_mm: 25.4,
                subdivisions: 4,
                visible: Some(true),
            },
        );
        assert!((e.doc.grid.step_mm() - 6.35).abs() < 1e-12);
        for bad in [
            DocumentCommand::GridSet {
                spacing_mm: 0.0,
                subdivisions: 4,
                visible: None,
            },
            DocumentCommand::GridSet {
                spacing_mm: 10.0,
                subdivisions: 1000,
                visible: None,
            },
            DocumentCommand::GuideAdd {
                axis: GuideAxis::Vertical,
                position_mm: f64::NAN,
            },
            DocumentCommand::SnapSet {
                grid: None,
                guides: None,
                objects: None,
                spacing: None,
                tolerance_mm: Some(-1.0),
            },
        ] {
            assert!(e.run(Command::Document(bad)).is_err());
        }
        assert!(e.doc.grid.visible);
        let back = Document::load_json(&e.doc.save_json().unwrap()).unwrap();
        assert_eq!(
            (back.guides, back.grid),
            (e.doc.guides.clone(), e.doc.grid.clone())
        );
    }

    fn snap_at(doc: &Document, x: f64, y: f64, w: f64, h: f64, exclude: Vec<u64>) -> SnapResult {
        snap(
            doc,
            &SnapQuery {
                x,
                y,
                width: w,
                height: h,
                exclude,
                tolerance_mm: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn snapping_to_guides_grid_edges_and_centres() {
        let mut doc = three().doc;
        doc.snap.objects = false;
        doc.snap.spacing = false;
        // Nothing enabled near: unchanged.
        let r = snap_at(&doc, 61.3, 71.7, 0.0, 0.0, vec![]);
        assert_eq!((r.x, r.y, r.snapped_x.clone()), (61.3, 71.7, None));
        // Grid: 5 mm step.
        doc.snap.grid = true;
        doc.grid.spacing_mm = 10.0;
        doc.grid.subdivisions = 2;
        let r = snap_at(&doc, 61.3, 71.7, 0.0, 0.0, vec![]);
        assert!((r.x - 60.0).abs() < 1e-9 && (r.y - 70.0).abs() < 1e-9);
        assert_eq!(r.snapped_x.unwrap().kind, SnapKind::Grid);
        assert_eq!(r.lines.len(), 2);
        // A guide wins over the grid when it is nearer.
        doc.guides.push(Guide {
            axis: GuideAxis::Vertical,
            position_mm: 61.0,
            locked: false,
        });
        let r = snap_at(&doc, 61.3, 71.7, 0.0, 0.0, vec![]);
        assert!((r.x - 61.0).abs() < 1e-9);
        assert_eq!(r.snapped_x.unwrap().kind, SnapKind::Guide);
        // Object edges and centres: a box's centre lines up with square 2's centre (25).
        doc.snap.grid = false;
        doc.snap.objects = true;
        let r = snap_at(&doc, 19.0, 40.0, 10.0, 10.0, vec![]);
        assert!((r.dx - 1.0).abs() < 1e-9, "{r:?}");
        let target = r.snapped_x.unwrap();
        assert!(matches!(target.kind, SnapKind::Edge | SnapKind::Center));
        // Excluded shapes are not targets; out of tolerance means no snap.
        let r = snap_at(&doc, 85.0, 40.0, 0.0, 0.0, vec![]);
        assert!(r.snapped_x.is_none(), "{r:?}");
        let r = snap_at(&doc, 41.0, 40.0, 0.0, 0.0, vec![3]);
        assert!(r.snapped_x.is_none() || r.snapped_x.unwrap().kind == SnapKind::Page);
        // Bad input is an error, not a panic.
        assert!(
            snap(
                &doc,
                &SnapQuery {
                    x: f64::NAN,
                    ..SnapQuery::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn smart_guides_offer_equal_spacing() {
        let mut doc = three().doc;
        doc.layers[0].shapes.truncate(2); // squares at 0..10 and 20..30: gap 10
        doc.snap.objects = false;
        // A 10 mm box dropped near x = 41 snaps to x = 40, repeating the 10 mm gap.
        let r = snap_at(&doc, 41.2, 0.0, 10.0, 10.0, vec![]);
        assert!((r.x - 40.0).abs() < 1e-9, "{r:?}");
        assert_eq!(r.snapped_x.unwrap().kind, SnapKind::Spacing);
        let marks: Vec<_> = r
            .lines
            .iter()
            .filter(|l| l.kind == SnapKind::Spacing)
            .collect();
        assert_eq!(marks.len(), 2);
        let len = |l: &SnapLine| (l.to[0] - l.from[0]).abs();
        assert!((len(marks[0]) - 10.0).abs() < 1e-9 && (len(marks[1]) - 10.0).abs() < 1e-9);
        // Before the pair too: right edge at -10.
        let r = snap_at(&doc, -21.0, 0.0, 10.0, 10.0, vec![]);
        assert!((r.x + 20.0).abs() < 1e-9, "{r:?}");
        // A box not level with the row gets no spacing snap.
        let r = snap_at(&doc, 41.2, 50.0, 10.0, 10.0, vec![]);
        assert!(r.snapped_x.is_none(), "{r:?}");
    }

    #[test]
    fn measure_and_dimensions() {
        let m = measure([0.0, 0.0], [3.0, -4.0]).unwrap();
        assert!((m.distance_mm - 5.0).abs() < 1e-12);
        assert!(
            (m.angle_degrees + 53.130_102_354_155_98).abs() < 1e-9,
            "{m:?}"
        );
        assert_eq!(
            measure([0.0, 0.0], [-1.0, 0.0]).unwrap().angle_degrees,
            180.0
        );
        assert!(measure([f64::NAN, 0.0], [0.0, 0.0]).is_err());
        let mut e = three();
        assert_eq!(
            run(&mut e, DocumentCommand::QueryDimensions { ids: vec![1, 3] }),
            Effect::Document(DocumentEffect::Dimensions {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 10.0
            })
        );
        assert!(!e.can_undo(), "queries add no history");
        assert!(
            e.run(Command::Document(DocumentCommand::QueryDimensions {
                ids: vec![9]
            }))
            .is_err()
        );
    }

    #[test]
    fn select_by_operation_layer_and_none() {
        let mut e = three();
        e.run(Command::ShapesSetStyle {
            ids: vec![2],
            style: CutStyle::Engrave,
        })
        .unwrap();
        let sel = |e: &mut Engine, c| match run(e, c) {
            Effect::Selection { ids } => ids,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            sel(
                &mut e,
                DocumentCommand::SelectByOperation {
                    style: CutStyle::Engrave
                }
            ),
            vec![2]
        );
        assert_eq!(
            sel(&mut e, DocumentCommand::SelectByLayer { index: 0 }),
            vec![1, 3]
        );
        assert_eq!(sel(&mut e, DocumentCommand::SelectAll {}).len(), 3);
        assert!(sel(&mut e, DocumentCommand::SelectNone {}).is_empty());
        assert!(
            e.run(Command::Document(DocumentCommand::SelectByColor {
                color: "red".to_string(),
                tolerance: 0.0
            }))
            .is_err()
        );
    }

    #[test]
    fn clipboard_copy_cut_paste_modes_keep_groups() {
        let mut e = three();
        run(
            &mut e,
            DocumentCommand::GroupCreate {
                ids: vec![1, 2],
                groups: vec![],
                name: "pair".to_string(),
            },
        );
        assert!(
            e.run(Command::Document(DocumentCommand::ClipboardPaste {
                mode: PasteMode::Offset,
                offset_mm: 5.0
            }))
            .is_err(),
            "empty clipboard"
        );
        let history = e.can_undo();
        run(&mut e, DocumentCommand::ClipboardCopy { ids: vec![1, 2] });
        assert_eq!(e.can_undo(), history);
        let Effect::ShapesAdded { ids } = run(
            &mut e,
            DocumentCommand::ClipboardPaste {
                mode: PasteMode::Offset,
                offset_mm: 5.0,
            },
        ) else {
            panic!()
        };
        assert_eq!(ids, vec![4, 5]);
        assert_eq!(bounds(&e, 4).x0, 5.0);
        assert_eq!(bounds(&e, 4).y0, 5.0);
        let chain = e.doc.group_chain(GroupChild::Shape(4));
        assert_eq!(chain.len(), 1, "the pasted pair is grouped");
        assert_eq!(e.doc.group_shapes(chain[0]), vec![4, 5]);
        // In front: directly above each original.
        run(
            &mut e,
            DocumentCommand::ClipboardPaste {
                mode: PasteMode::InFront,
                offset_mm: 5.0,
            },
        );
        assert_eq!(order(&e), vec![1, 6, 2, 7, 3, 4, 5]);
        assert_eq!(bounds(&e, 6), bounds(&e, 1));
        // In place: same position, top of the layer.
        run(
            &mut e,
            DocumentCommand::ClipboardPaste {
                mode: PasteMode::InPlace,
                offset_mm: 5.0,
            },
        );
        assert_eq!(*order(&e).last().unwrap(), 9);
        assert_eq!(bounds(&e, 9), bounds(&e, 2));
        // Cut removes and is undoable.
        run(&mut e, DocumentCommand::ClipboardCut { ids: vec![3] });
        assert!(e.doc.get_shape(3).is_none());
        e.run(Command::Undo).unwrap();
        assert!(e.doc.get_shape(3).is_some());
        assert!(
            e.run(Command::Document(DocumentCommand::ClipboardCopy {
                ids: vec![42]
            }))
            .is_err()
        );
    }

    #[test]
    fn transform_each_and_again() {
        let mut e = three();
        run(
            &mut e,
            DocumentCommand::TransformEach(TransformRequest {
                ids: vec![1, 2],
                dx_mm: 0.0,
                dy_mm: 0.0,
                scale_x: 2.0,
                scale_y: 2.0,
                degrees: 0.0,
                each: true,
            }),
        );
        // Each square doubled about its own centre.
        let b = bounds(&e, 1);
        assert!((b.x0 + 5.0).abs() < 1e-9 && (b.width() - 20.0).abs() < 1e-9);
        let b = bounds(&e, 2);
        assert!((b.x0 - 15.0).abs() < 1e-9);
        run(
            &mut e,
            DocumentCommand::TransformEach(TransformRequest {
                ids: vec![3],
                dx_mm: 3.0,
                dy_mm: -1.0,
                scale_x: 1.0,
                scale_y: 1.0,
                degrees: 0.0,
                each: false,
            }),
        );
        run(&mut e, DocumentCommand::TransformAgain { ids: vec![3] });
        let b = bounds(&e, 3);
        assert!(
            (b.x0 - 46.0).abs() < 1e-9 && (b.y0 + 2.0).abs() < 1e-9,
            "{b:?}"
        );
        e.run(Command::Undo).unwrap();
        assert!((bounds(&e, 3).x0 - 43.0).abs() < 1e-9);
        // Grouped shapes transform as one object.
        run(
            &mut e,
            DocumentCommand::GroupCreate {
                ids: vec![1, 2],
                groups: vec![],
                name: String::new(),
            },
        );
        let before = bounds(&e, 1).union(bounds(&e, 2));
        run(
            &mut e,
            DocumentCommand::TransformEach(TransformRequest {
                ids: vec![1, 2],
                dx_mm: 0.0,
                dy_mm: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                degrees: 180.0,
                each: true,
            }),
        );
        let after = bounds(&e, 1).union(bounds(&e, 2));
        assert!((after.x0 - before.x0).abs() < 1e-9 && (after.x1 - before.x1).abs() < 1e-9);
        assert!(
            bounds(&e, 1).x0 > bounds(&e, 2).x0,
            "rotated about the group centre"
        );
        let bad = TransformRequest {
            ids: vec![3],
            dx_mm: f64::INFINITY,
            dy_mm: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            degrees: 0.0,
            each: false,
        };
        assert!(
            e.run(Command::Document(DocumentCommand::TransformEach(bad)))
                .is_err()
        );
        assert!(
            three()
                .run(Command::Document(DocumentCommand::TransformAgain {
                    ids: vec![1]
                }))
                .is_err()
        );
    }

    #[test]
    fn align_to_key_and_distribute_by_spacing() {
        let mut e = three();
        e.run(Command::ShapeMove {
            id: 2,
            dx_mm: 0.0,
            dy_mm: 7.0,
        })
        .unwrap();
        run(
            &mut e,
            DocumentCommand::ShapesAlignToKey {
                ids: vec![1, 2, 3],
                key: 2,
                alignment: Alignment::Top,
            },
        );
        for id in [1, 2, 3] {
            assert!((bounds(&e, id).y0 - 7.0).abs() < 1e-9);
        }
        run(
            &mut e,
            DocumentCommand::ShapesDistributeSpacing {
                ids: vec![3, 1, 2],
                axis: DistributionAxis::Horizontal,
                gap_mm: 2.5,
            },
        );
        let xs: Vec<f64> = [1, 2, 3].iter().map(|id| bounds(&e, *id).x0).collect();
        assert_eq!(xs, vec![0.0, 12.5, 25.0]);
        assert!(
            e.run(Command::Document(DocumentCommand::ShapesAlignToKey {
                ids: vec![1, 3],
                key: 2,
                alignment: Alignment::Left
            }))
            .is_err()
        );
        assert!(
            e.run(Command::Document(
                DocumentCommand::ShapesDistributeSpacing {
                    ids: vec![1],
                    axis: DistributionAxis::Vertical,
                    gap_mm: 1.0
                }
            ))
            .is_err()
        );
    }

    #[test]
    fn new_from_template_replaces_the_document_undoably() {
        let mut e = three();
        e.run(Command::KerfSet { width_mm: 0.2 }).unwrap();
        let command: Command = serde_json::from_value(serde_json::json!({
            "command": "document.new-from-template",
            "params": {"template": "card", "params": {"fold": "horizontal"}}
        }))
        .unwrap();
        assert_eq!(e.run(command).unwrap(), Effect::DocumentReset);
        assert_eq!(e.doc.shape_count(), 2);
        assert_eq!(e.doc.kerf.width_mm, 0.2);
        assert_eq!(e.doc.groups.len(), 1);
        // The new shapes are editable through the VectorCraft path.
        e.run(Command::ShapeMove {
            id: 1,
            dx_mm: 1.0,
            dy_mm: 0.0,
        })
        .unwrap();
        e.run(Command::Undo).unwrap();
        e.run(Command::Undo).unwrap();
        assert_eq!(order(&e), vec![1, 2, 3]);
        assert!(
            e.run(Command::Document(
                DocumentCommand::DocumentNewFromTemplate {
                    template: ProjectTemplate::GiftTag,
                    params: TemplateParams {
                        hole_mm: Some(-1.0),
                        ..TemplateParams::default()
                    },
                }
            ))
            .is_err()
        );
    }

    #[test]
    fn metadata_and_thumbnail_round_trip_with_caps() {
        let mut e = three();
        run(
            &mut e,
            DocumentCommand::DocumentSetMeta {
                title: Some("Birthday card".to_string()),
                notes: Some("12x12 cardstock".to_string()),
            },
        );
        run(&mut e, DocumentCommand::DocumentUpdateThumbnail {});
        let thumb = e.doc.meta.thumbnail_svg.clone().unwrap();
        assert_eq!(thumb.matches("<path").count(), 3);
        let back = Document::load_json(&e.doc.save_json().unwrap()).unwrap();
        assert_eq!(back.meta, e.doc.meta);
        assert!(
            e.run(Command::Document(DocumentCommand::DocumentSetMeta {
                title: Some("x".repeat(5000)),
                notes: None
            }))
            .is_err()
        );
        assert_eq!(e.doc.meta.title, "Birthday card");
        e.run(Command::Undo).unwrap();
        e.run(Command::Undo).unwrap();
        assert!(e.doc.meta.is_empty());
    }
}
