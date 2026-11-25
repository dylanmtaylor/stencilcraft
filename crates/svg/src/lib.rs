//! StencilCraft SVG import and export policy on shared Craft-Libs SVG parsing and writing.

use std::collections::{BTreeSet, HashMap};

use kurbo::BezPath;
use stencilcraft_doc::{
    CutStyle, Document, Group, GroupChild, Layer, Operation, OperationKind, OperationSettings,
    Shape, ShapeProduction,
};

/// The production pass kind a legacy `CutStyle` maps to.
fn kind_of(style: CutStyle) -> OperationKind {
    match style {
        CutStyle::Cut => OperationKind::Cut,
        CutStyle::Engrave => OperationKind::Engrave,
    }
}

/// Finds the operation of `kind`, or creates it (named `name`) with the next free id.
fn ensure_operation(
    doc: &mut Document,
    name: &str,
    kind: OperationKind,
    color: Option<String>,
) -> u64 {
    if let Some(op) = doc
        .operations
        .iter()
        .find(|o| o.kind == kind && o.color == color)
    {
        return op.id;
    }
    let id = doc.next_operation_id();
    doc.operations.push(Operation {
        id,
        name: name.to_string(),
        kind,
        settings: OperationSettings::default(),
        color,
        visible: true,
        locked: false,
        members: Vec::new(),
        icon: None,
    });
    id
}
use stencilcraft_geom::{Polyline, path_to_polylines};

use craft_svg::paths::Kind as _;
use craft_svg::write::{Attribute as SvgAttribute, Element as SvgElement, Scene as SvgScene};

#[derive(Debug, thiserror::Error)]
pub enum SvgError {
    #[error("svg export: failure to build markup: {0}")]
    Write(String),
    #[error("svg import: {0}")]
    Import(String),
    #[error("svg: {0}")]
    Model(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TilePreviewOptions {
    pub columns: usize,
    pub scale: f64,
}
impl Default for TilePreviewOptions {
    fn default() -> Self {
        Self {
            columns: 3,
            scale: 0.25,
        }
    }
}
pub const TILE_PREVIEW_GAP_MM: f64 = 6.0;
pub const TILE_PREVIEW_MAX_MM: f64 = 100_000.0;

fn class_for(kind: Option<&OperationKind>) -> &'static str {
    match kind {
        Some(OperationKind::Engrave) => "stencilcraft-engrave",
        _ => "stencilcraft-cut",
    }
}
fn view_bounds(doc: &Document) -> kurbo::Rect {
    doc.operations
        .iter()
        .filter(|o| !o.kind.is_guide())
        .flat_map(|o| {
            o.members
                .iter()
                .filter_map(|id| doc.get_shape(*id))
                .filter_map(|s| s.bounds())
        })
        .fold(
            kurbo::Rect::new(0.0, 0.0, doc.board.width_mm, doc.board.height_mm),
            |bounds, layer| bounds.union(layer),
        )
}
// Groups are bounded before recursion to avoid exhausting the stack on hostile documents.
fn append_grouped(destination: &mut Vec<SvgElement>, chain: &[u64], element: SvgElement) {
    let Some((&id, rest)) = chain.split_first() else {
        destination.push(element);
        return;
    };
    let marker = SvgAttribute::text("data-group", id.to_string());
    if !matches!(destination.last(), Some(SvgElement::Group { attributes, .. }) if attributes.contains(&marker))
    {
        destination.push(SvgElement::Group {
            attributes: vec![SvgAttribute::text("class", "group"), marker],
            children: Vec::new(),
        });
    }
    if let Some(SvgElement::Group { children, .. }) = destination.last_mut() {
        append_grouped(children, rest, element);
    }
}

fn document_elements(
    doc: &Document,
    output_layers_only: bool,
    include_title: bool,
) -> Vec<SvgElement> {
    let mut elements = Vec::new();
    if include_title {
        elements.push(SvgElement::Title(format!(
            "stencilcraft {}: {} layers, {} shapes, kerf {:.3} mm",
            stencilcraft_doc::FORMAT_NAME,
            doc.layers.len(),
            doc.shape_count(),
            doc.kerf.width_mm
        )));
    }
    let parents = doc.group_parent_map();
    for op in doc.operations.iter().filter(|op| {
        if output_layers_only {
            op.is_output()
        } else {
            !op.kind.is_guide()
        }
    }) {
        let mut children = Vec::new();
        for id in &op.members {
            let Some(shape) = doc.get_shape(*id).filter(|shape| !shape.hidden) else {
                continue;
            };
            let chain = stencilcraft_doc::canvas::group_chain_in(
                &parents,
                stencilcraft_doc::GroupChild::Shape(shape.id),
            );
            let element = SvgElement::Path {
                attributes: vec![SvgAttribute::text("class", class_for(Some(&op.kind)))],
                path: shape.cut_path().into_owned(),
            };
            append_grouped(&mut children, &chain, element);
        }
        let mut attributes = Vec::new();
        if !output_layers_only {
            attributes.push(SvgAttribute::text("class", class_for(Some(&op.kind))));
        }
        attributes.extend([
            SvgAttribute::text("data-kind", op.kind.kind_name()),
            SvgAttribute::text(
                "style",
                if op.visible {
                    "display:visible"
                } else {
                    "display:hidden"
                },
            ),
        ]);
        elements.push(SvgElement::Group {
            attributes,
            children,
        });
    }
    elements
}

pub fn export(doc: &Document) -> Result<String, SvgError> {
    let bounds = view_bounds(doc);
    Ok(SvgScene {
        width_mm: bounds.width(),
        height_mm: bounds.height(),
        view_box: [bounds.x0, bounds.y0, bounds.width(), bounds.height()],
        elements: document_elements(doc, false, true),
    }
    .write_fixed_view_box())
}

pub fn export_tile_preview(
    pieces: &[Document],
    options: &TilePreviewOptions,
) -> Result<String, SvgError> {
    if pieces.is_empty() {
        return Err(SvgError::Model(
            "preview: the tiling produced no sheets to show".into(),
        ));
    }
    if options.columns == 0 {
        return Err(SvgError::Model(
            "preview: columns must be at least 1".into(),
        ));
    }
    if !options.scale.is_finite() || options.scale <= 0.0 {
        return Err(SvgError::Model(format!(
            "preview: scale {} is not a positive number",
            options.scale
        )));
    }
    let columns = options.columns.min(pieces.len());
    let rows = pieces.len().div_ceil(columns);
    let mut cell_w: f64 = 0.0;
    let mut cell_h: f64 = 0.0;
    for piece in pieces {
        let (w, h) = (piece.board.width_mm, piece.board.height_mm);
        if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
            return Err(SvgError::Model(format!(
                "preview: sheet {w}×{h} mm is not a usable size"
            )));
        }
        cell_w = cell_w.max(w * options.scale);
        cell_h = cell_h.max(h * options.scale);
    }
    let gap = TILE_PREVIEW_GAP_MM;
    let page_w = columns as f64 * cell_w + (columns as f64 + 1.0) * gap;
    let page_h = rows as f64 * cell_h + (rows as f64 + 1.0) * gap;
    if !(page_w.is_finite() && page_h.is_finite())
        || page_w > TILE_PREVIEW_MAX_MM
        || page_h > TILE_PREVIEW_MAX_MM
    {
        return Err(SvgError::Model(format!(
            "preview: a {page_w:.0}×{page_h:.0} mm page is too big; use a smaller scale or fewer sheets"
        )));
    }
    let mut elements = vec![
        SvgElement::Title(format!(
            "StencilCraft tiling preview: {} sheets, {} × {} cells",
            pieces.len(),
            columns,
            rows
        )),
        SvgElement::Style(
            concat!(
                ".preview-frame { fill: none; stroke: #9aa0a6; stroke-width: 0.25; }\n",
                ".stencilcraft-cut { fill: none; stroke: #c0392b; stroke-width: 0.3; }\n",
                ".stencilcraft-engrave { fill: none; stroke: #2d6cdf; stroke-width: 0.2; }\n",
                ".preview-label { fill: #5f6368; font-family: sans-serif; font-size: 3.2px; }"
            )
            .into(),
        ),
    ];
    for (index, piece) in pieces.iter().enumerate() {
        let col = index % columns;
        let row = index / columns;
        let x = gap + col as f64 * (cell_w + gap);
        let y = gap + (rows - 1 - row) as f64 * (cell_h + gap);
        let cell_height = piece.board.height_mm * options.scale;
        let children = vec![
            SvgElement::Text {
                attributes: vec![SvgAttribute::text("class", "preview-label")],
                x,
                y: y - 1.6,
                text: format!(
                    "sheet {} of {} · {:.0}×{:.0} mm",
                    index + 1,
                    pieces.len(),
                    piece.board.width_mm,
                    piece.board.height_mm
                ),
            },
            SvgElement::Rect {
                attributes: vec![SvgAttribute::text("class", "preview-frame")],
                x,
                y,
                width: piece.board.width_mm * options.scale,
                height: cell_height,
            },
            SvgElement::Group {
                attributes: vec![SvgAttribute::translate_scale_fixed(
                    x,
                    y + cell_height,
                    options.scale,
                    -options.scale,
                )],
                children: document_elements(piece, true, false),
            },
        ];
        elements.push(SvgElement::Group {
            attributes: vec![
                SvgAttribute::text("class", "sheet"),
                SvgAttribute::text("data-sheet", index.to_string()),
                SvgAttribute::text(
                    "data-size",
                    format!("{:.1}x{:.1}", piece.board.width_mm, piece.board.height_mm),
                ),
            ],
            children,
        });
    }
    Ok(SvgScene {
        width_mm: page_w,
        height_mm: page_h,
        view_box: [0.0, 0.0, page_w, page_h],
        elements,
    }
    .write_fixed_view_box())
}

pub fn export_print_and_cut(doc: &Document) -> Result<String, SvgError> {
    const MARGIN: f64 = 12.0;
    const MARK: f64 = 6.0;
    doc.validate().map_err(|e| SvgError::Model(e.to_string()))?;
    let page_w = doc.board.width_mm + 2.0 * MARGIN;
    let page_h = doc.board.height_mm + 2.0 * MARGIN;
    if !page_w.is_finite() || !page_h.is_finite() {
        return Err(SvgError::Model(
            "print-cut: page dimensions are not finite".into(),
        ));
    }
    let mut marks = vec![SvgElement::Rect {
        attributes: Vec::new(),
        x: 0.0,
        y: 0.0,
        width: MARK,
        height: MARK,
    }];
    let mut top = BezPath::new();
    top.move_to((page_w, 0.0));
    top.line_to((page_w - MARK, 0.0));
    top.line_to((page_w - MARK, MARK));
    top.line_to((page_w - 1.5, MARK));
    top.line_to((page_w - 1.5, 1.5));
    top.line_to((page_w - MARK, 1.5));
    top.close_path();
    marks.push(SvgElement::Path {
        attributes: Vec::new(),
        path: top,
    });
    let mut bottom = BezPath::new();
    bottom.move_to((0.0, page_h));
    bottom.line_to((MARK, page_h));
    bottom.line_to((MARK, page_h - MARK));
    bottom.line_to((1.5, page_h - MARK));
    bottom.line_to((1.5, page_h - 1.5));
    bottom.line_to((0.0, page_h - 1.5));
    bottom.close_path();
    marks.push(SvgElement::Path {
        attributes: Vec::new(),
        path: bottom,
    });
    let elements = vec![
        SvgElement::Title("StencilCraft print-and-cut alignment sheet".into()),
        SvgElement::Group {
            attributes: vec![
                SvgAttribute::text("id", "registration-marks"),
                SvgAttribute::text("fill", "#000000"),
                SvgAttribute::text(
                    "aria-label",
                    "Registration marks: top-left square, top-right and bottom-left corners",
                ),
            ],
            children: marks,
        },
        SvgElement::NestedSvg {
            attributes: vec![
                SvgAttribute::number("x", MARGIN),
                SvgAttribute::number("y", MARGIN),
                SvgAttribute::millimetres("width", doc.board.width_mm),
                SvgAttribute::millimetres("height", doc.board.height_mm),
                SvgAttribute::view_box([0.0, 0.0, doc.board.width_mm, doc.board.height_mm]),
                SvgAttribute::text("overflow", "hidden"),
            ],
            children: document_elements(doc, false, true),
        },
    ];
    Ok(SvgScene {
        width_mm: page_w,
        height_mm: page_h,
        view_box: [0.0, 0.0, page_w, page_h],
        elements,
    }
    .write_fixed_view_box())
}

#[derive(Debug, Clone, Copy)]
struct SvgKind(CutStyle);
impl Default for SvgKind {
    fn default() -> Self {
        Self(CutStyle::Cut)
    }
}
impl craft_svg::paths::Kind for SvgKind {
    const PROPERTIES: &'static [&'static str] = &["data-kind"];
    fn of(style: &craft_svg::paths::StyleMap, class: &str) -> Option<Self> {
        let kind = style.get("data-kind").map(String::as_str);
        if kind == Some("engrave")
            || class
                .split_ascii_whitespace()
                .any(|c| c == "stencilcraft-engrave")
        {
            Some(Self(CutStyle::Engrave))
        } else if kind == Some("cut")
            || class
                .split_ascii_whitespace()
                .any(|c| c == "stencilcraft-cut")
        {
            Some(Self(CutStyle::Cut))
        } else {
            None
        }
    }
}

/// How the importer maps source structure onto the document (ADR 0001).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportPolicy {
    /// Phase-2 semantics: `<g>` becomes an operation layer and source styling becomes
    /// the layer's operation. Kept reachable for one release (GEO-03).
    pub legacy_style_operations: bool,
}

/// A non-binding suggestion that source styling implies a production operation for a
/// group. Hints are collected on [`SvgImport`] and surfaced for explicit user
/// confirmation; the importer never applies them (GEO-02).
#[derive(Debug, Clone, PartialEq)]
pub struct OperationHint {
    /// Names of the `<g>` elements of the hinted element, outermost first; empty for a
    /// top-level shape.
    pub group_path: Vec<String>,
    /// The operation the element's `data-kind`/class suggests.
    pub operation: OperationKind,
}

#[derive(Debug)]
pub struct SvgImport {
    pub document: Document,
    pub warnings: Vec<String>,
    /// Source-styling suggestions, never applied by the importer.
    pub operation_hints: Vec<OperationHint>,
}
pub fn import(svg: &str, name: &str) -> Result<Document, SvgError> {
    let result = import_with_warnings(svg, name)?;
    if !result.warnings.is_empty() {
        return Err(SvgError::Import(result.warnings.join("; ")));
    }
    Ok(result.document)
}
/// Decode SVG or SVGZ with the shared bounded reader and apply Stencil's vector policy.
pub fn import_bytes_with_warnings(bytes: &[u8], name: &str) -> Result<SvgImport, SvgError> {
    import_bytes_with_warnings_policy(bytes, name, &ImportPolicy::default())
}
/// Like [`import_bytes_with_warnings`], with an explicit [`ImportPolicy`].
pub fn import_bytes_with_warnings_policy(
    bytes: &[u8],
    name: &str,
    policy: &ImportPolicy,
) -> Result<SvgImport, SvgError> {
    let text = craft_svg::svgz::text(bytes, craft_svg::paths::MAX_SVG_BYTES)
        .map_err(|error| SvgError::Import(error.to_string()))?;
    import_with_warnings_policy(&text, name, policy)
}

pub fn import_with_warnings(svg: &str, name: &str) -> Result<SvgImport, SvgError> {
    import_with_warnings_policy(svg, name, &ImportPolicy::default())
}

/// One source shape that survives clipping, with the styling the legacy layer mapping
/// needs. `None` marks an emitted shape whose clipped path is empty (dropped).
struct PendingShape {
    path: BezPath,
    style: CutStyle,
    color: Option<String>,
    layer: Option<String>,
}

pub fn import_with_warnings_policy(
    svg: &str,
    name: &str,
    policy: &ImportPolicy,
) -> Result<SvgImport, SvgError> {
    let imported = craft_svg::paths::import::<SvgKind>(svg, name)
        .map_err(|error| SvgError::Import(error.to_string()))?;
    let mut doc = Document {
        board: stencilcraft_doc::Board {
            width_mm: imported.width_mm,
            height_mm: imported.height_mm,
        },
        ..Document::default()
    };
    doc.layers.clear();
    let mut warnings = imported.warnings.clone();
    let mut dropped = imported.dropped_warnings;
    let mut pending: Vec<Option<PendingShape>> = Vec::with_capacity(imported.shapes.len());
    for shape in &imported.shapes {
        // Cutters retain open centerlines even when SVG's default fill would paint no area.
        // Ask the shared clipper to treat an entirely open path as linework.
        let mut cut_shape = shape.clone();
        if !cut_shape
            .path
            .elements()
            .iter()
            .any(|e| matches!(e, kurbo::PathEl::ClosePath))
        {
            cut_shape.has_fill = false;
            cut_shape.has_stroke = true;
        }
        let path = imported
            .clipped_path(&cut_shape)
            .map_err(|error| SvgError::Import(error.to_string()))?;
        if path.elements().is_empty() {
            pending.push(None);
            continue;
        }
        let color = shape
            .stroke
            .as_deref()
            .and_then(|stroke| match craft_svg::color::parse_color(stroke) {
                Ok(c) => Some(format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)),
                Err(_) => {
                    craft_svg::paths::push_warning(
                        &mut warnings,
                        &mut dropped,
                        format!("{}: unrecognised stroke colour", shape.label),
                    );
                    None
                }
            });
        pending.push(Some(PendingShape {
            path,
            style: shape.kind.0,
            color,
            layer: shape.layer.clone(),
        }));
    }
    if dropped > 0 {
        warnings.push(format!("{dropped} more SVG warnings omitted"));
    }

    let operation_hints = if policy.legacy_style_operations {
        build_legacy_layers(&mut doc, &pending)?;
        Vec::new()
    } else {
        // Geometry-preserving import (GEO-01): shapes land unassigned, `<g>` nesting
        // becomes `Document.groups`, and styling is collected as hints only.
        let mut shape_ids = Vec::with_capacity(pending.len());
        for item in &pending {
            let Some(p) = item else {
                shape_ids.push(None);
                continue;
            };
            let id = doc.next_shape_id;
            doc.next_shape_id = doc
                .next_shape_id
                .checked_add(1)
                .ok_or_else(|| SvgError::Model("shape ids exhausted".into()))?;
            doc.shapes.push(Shape {
                id,
                path: p.path.clone(),
                hidden: false,
                locked: false,
                text: None,
                production: ShapeProduction::default(),
            });
            shape_ids.push(Some(id));
        }
        attach_groups(&mut doc, svg, &shape_ids)
    };

    for shape in &doc.shapes {
        if let Some(bounds) = shape.bounds() {
            doc.board.width_mm = doc.board.width_mm.max(bounds.x1);
            doc.board.height_mm = doc.board.height_mm.max(bounds.y1);
        }
    }
    doc.validate().map_err(|e| SvgError::Model(e.to_string()))?;
    Ok(SvgImport {
        document: doc,
        warnings,
        operation_hints,
    })
}

/// The Phase-2 mapping, kept verbatim behind `ImportPolicy::legacy_style_operations`:
/// one geometry layer per (name, style, colour), with one operation per layer carrying
/// the style, colour and members.
fn build_legacy_layers(
    doc: &mut Document,
    pending: &[Option<PendingShape>],
) -> Result<(), SvgError> {
    let mut layers = HashMap::<(String, bool, Option<String>), usize>::new();
    doc.layers.push(Layer::new("cut"));
    ensure_operation(doc, "cut", kind_of(CutStyle::Cut), None);
    layers.insert(("cut".into(), false, None), 0);
    for item in pending {
        let Some(p) = item else { continue };
        let layer_name = p
            .layer
            .clone()
            .unwrap_or_else(|| match (&p.color, p.style) {
                (Some(color), CutStyle::Cut) => format!("Line {color}"),
                (Some(color), CutStyle::Engrave) => format!("engrave {color}"),
                (None, CutStyle::Cut) => "cut".into(),
                (None, CutStyle::Engrave) => "engrave".into(),
            });
        let key = (
            layer_name.clone(),
            p.style == CutStyle::Engrave,
            p.color.clone(),
        );
        let index = if let Some(index) = layers.get(&key) {
            *index
        } else {
            doc.layers.push(Layer::new(layer_name.clone()));
            let i = doc.layers.len() - 1;
            layers.insert(key, i);
            i
        };
        let op_id = ensure_operation(doc, &layer_name, kind_of(p.style), p.color.clone());
        let id = doc
            .push_shape(index, p.path.clone())
            .map_err(|e| SvgError::Model(e.to_string()))?
            .id;
        if let Some(op) = doc.operations.iter_mut().find(|o| o.id == op_id) {
            op.members.push(id);
        }
    }
    Ok(())
}

/// One `<g>` seen while walking the source tree, in document order (parents first).
struct WalkGroup {
    name: String,
    /// `data-group` written by [`export`], reused as the group id so an export→import
    /// round trip preserves group identity.
    data_group: Option<u64>,
    parent: Option<usize>,
}

#[derive(Clone, Default)]
struct WalkState {
    /// Inherited subset of the computed style the walk tracks: presentation attributes
    /// and inline declarations only (CSS-driven hiding can desynchronize the walk; the
    /// caller then skips grouping rather than misassign members).
    props: HashMap<String, String>,
}

const WALK_MAX_DEPTH: usize = 128;
const WALK_MAX_VISITS: usize = 1_000_000;

/// The result of walking the source tree: the `<g>` skeleton, one leaf chain (group
/// indices, outermost first) per shape the shared importer emits, and the declared
/// production kinds as `(group chain, style)` hints.
#[derive(Default)]
struct GroupWalk {
    groups: Vec<WalkGroup>,
    leaves: Vec<Vec<usize>>,
    hints: Vec<(Vec<usize>, CutStyle)>,
}

/// Best-effort reconstruction of the source `<g>` tree.
fn walk_groups(svg: &str) -> GroupWalk {
    let mut walk = GroupWalk {
        groups: Vec::new(),
        leaves: Vec::new(),
        hints: Vec::new(),
    };
    let Ok(xml) = roxmltree::Document::parse(svg) else {
        return walk;
    };
    let root = xml.root_element();
    let mut id_index: HashMap<String, roxmltree::Node> = HashMap::new();
    let mut nodes = 0usize;
    for node in root.descendants() {
        nodes += 1;
        if nodes > craft_svg::paths::MAX_SVG_NODES {
            return GroupWalk::default();
        }
        if let Some(id) = node.attribute("id") {
            id_index.entry(id.to_string()).or_insert(node);
        }
    }
    let mut ref_stack: Vec<String> = Vec::new();
    let mut visits = 0usize;
    walk_node(
        root,
        &id_index,
        &mut ref_stack,
        &mut visits,
        &mut walk,
        Vec::new(),
        WalkState::default(),
        0,
    );
    walk
}

#[allow(clippy::too_many_arguments)]
fn walk_node(
    node: roxmltree::Node,
    id_index: &HashMap<String, roxmltree::Node>,
    ref_stack: &mut Vec<String>,
    visits: &mut usize,
    walk: &mut GroupWalk,
    chain: Vec<usize>,
    inherited: WalkState,
    depth: usize,
) {
    *visits += 1;
    if depth > WALK_MAX_DEPTH || *visits > WALK_MAX_VISITS {
        return;
    }
    let tag = node.tag_name().name();
    // Not drawable content unless instantiated via `<use>`.
    if matches!(
        tag,
        "defs" | "symbol" | "clipPath" | "mask" | "pattern" | "marker"
    ) {
        return;
    }
    // The node's own declarations decide whether it *declares* a production kind
    // (a hint); everything else inherits down the tree.
    let mut own: HashMap<String, String> = HashMap::new();
    for key in ["data-kind"] {
        if let Some(value) = node.attribute(key) {
            own.insert(key.to_string(), value.to_string());
        }
    }
    if let Some(inline) = node.attribute("style") {
        for declaration in inline.split(';') {
            if let Some((key, value)) = declaration.split_once(':')
                && key.trim() == "data-kind"
            {
                own.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }
    let class = node.attribute("class").unwrap_or("");
    let declared = SvgKind::of(&own, class).map(|kind| kind.0);
    let mut props = inherited.props.clone();
    for key in ["data-kind", "display", "visibility", "fill", "stroke"] {
        if let Some(value) = node.attribute(key) {
            props.insert(key.to_string(), value.to_string());
        }
    }
    if let Some(inline) = node.attribute("style") {
        for declaration in inline.split(';') {
            if let Some((key, value)) = declaration.split_once(':') {
                props.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }
    let state = WalkState {
        props: props.clone(),
    };
    let display_none = props.get("display").is_some_and(|v| v == "none")
        || (props.get("stroke").is_some_and(|v| v == "none")
            && props.get("fill").is_some_and(|v| v == "none"));
    let visibility_hidden = props
        .get("visibility")
        .is_some_and(|v| v == "hidden" || v == "collapse");

    if tag == "g" {
        let name = node
            .attribute("id")
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("group-{}", walk.groups.len() + 1));
        let data_group = node
            .attribute("data-group")
            .and_then(|value| value.parse::<u64>().ok());
        let index = walk.groups.len();
        walk.groups.push(WalkGroup {
            name,
            data_group,
            parent: chain.last().copied(),
        });
        if let Some(kind) = declared {
            let mut path = chain.clone();
            path.push(index);
            walk.hints.push((path, kind));
        }
        let mut child_chain = chain;
        child_chain.push(index);
        for child in node.children().filter(|n| n.is_element()) {
            walk_node(
                child,
                id_index,
                ref_stack,
                visits,
                walk,
                child_chain.clone(),
                state.clone(),
                depth + 1,
            );
        }
        return;
    }
    if tag == "use" {
        if !display_none
            && let Some(href) = node
                .attribute("href")
                .or_else(|| node.attribute(("http://www.w3.org/1999/xlink", "href")))
            && let Some(id) = href.strip_prefix('#').filter(|s| !s.is_empty())
            && !ref_stack.iter().any(|active| active == id)
            && let Some(target) = id_index.get(id)
        {
            let target_tag = target.tag_name().name();
            if matches!(
                target_tag,
                "g" | "symbol"
                    | "path"
                    | "rect"
                    | "circle"
                    | "ellipse"
                    | "line"
                    | "polyline"
                    | "polygon"
            ) {
                ref_stack.push(id.to_string());
                if target_tag == "g" || target_tag == "symbol" {
                    // Each instance of a reused `<g>` is its own group at the use site;
                    // a `<symbol>` instantiates its children without a group of its own.
                    let mut child_chain = chain;
                    if target_tag == "g" {
                        let name = target
                            .attribute("id")
                            .map(str::to_string)
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| format!("group-{}", walk.groups.len() + 1));
                        let data_group = target
                            .attribute("data-group")
                            .and_then(|value| value.parse::<u64>().ok());
                        let index = walk.groups.len();
                        walk.groups.push(WalkGroup {
                            name,
                            data_group,
                            parent: child_chain.last().copied(),
                        });
                        let mut target_props = HashMap::new();
                        for key in ["data-kind", "display", "visibility", "fill", "stroke"] {
                            if let Some(value) = target.attribute(key) {
                                target_props.insert(key.to_string(), value.to_string());
                            }
                        }
                        child_chain.push(index);
                        if let Some(kind) =
                            SvgKind::of(&target_props, target.attribute("class").unwrap_or(""))
                        {
                            walk.hints.push((child_chain.clone(), kind.0));
                        }
                    }
                    for child in target.children().filter(|n| n.is_element()) {
                        walk_node(
                            child,
                            id_index,
                            ref_stack,
                            visits,
                            walk,
                            child_chain.clone(),
                            state.clone(),
                            depth + 1,
                        );
                    }
                } else {
                    walk_node(
                        *target,
                        id_index,
                        ref_stack,
                        visits,
                        walk,
                        chain,
                        state,
                        depth + 1,
                    );
                }
                ref_stack.pop();
            }
        }
        return;
    }
    if tag == "text" {
        return;
    }
    if matches!(
        tag,
        "path" | "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon"
    ) && !display_none
        && !visibility_hidden
    {
        if let Some(kind) = declared {
            walk.hints.push((chain.clone(), kind));
        }
        walk.leaves.push(chain);
        return;
    }
    for child in node.children().filter(|n| n.is_element()) {
        walk_node(
            child,
            id_index,
            ref_stack,
            visits,
            walk,
            chain.clone(),
            state.clone(),
            depth + 1,
        );
    }
}

/// Build `doc.groups` from the walked source tree and turn declared styling into
/// operation hints. When the walk's leaf count does not match the shapes the shared
/// importer emitted (CSS-driven hiding can desynchronize it), grouping is skipped and
/// only the hints are returned.
fn attach_groups(doc: &mut Document, svg: &str, shape_ids: &[Option<u64>]) -> Vec<OperationHint> {
    let walk = walk_groups(svg);
    let groups = &walk.groups;
    let mut hints = Vec::new();
    for (chain, kind) in &walk.hints {
        let group_path = chain
            .iter()
            .filter_map(|&i| groups.get(i).map(|g| g.name.clone()))
            .collect();
        hints.push(OperationHint {
            group_path,
            operation: kind_of(*kind),
        });
    }
    let leaves = &walk.leaves;
    if leaves.len() != shape_ids.len() {
        return hints;
    }
    // Keep only groups that hold at least one shape, directly or nested.
    let mut has_shape = vec![false; walk.groups.len()];
    for leaf in leaves {
        for &index in leaf {
            if let Some(slot) = has_shape.get_mut(index) {
                *slot = true;
            }
        }
    }
    let mut remap: Vec<Option<usize>> = Vec::with_capacity(walk.groups.len());
    let mut kept = 0usize;
    for (index, _) in groups.iter().enumerate() {
        if has_shape[index] {
            remap.push(Some(kept));
            kept += 1;
        } else {
            remap.push(None);
        }
    }
    // Allocate document group ids, preferring the exported `data-group` id (first
    // pass) so an export→import round trip preserves group identity.
    let mut used: BTreeSet<u64> = BTreeSet::new();
    let mut next_id = doc.next_group_id();
    let mut id_of = vec![0u64; walk.groups.len()];
    for (index, group) in groups.iter().enumerate() {
        if remap[index].is_none() {
            continue;
        }
        if let Some(data_group) = group.data_group
            && used.insert(data_group)
        {
            id_of[index] = data_group;
            if data_group >= next_id {
                next_id = data_group.saturating_add(1);
            }
        }
    }
    for (index, _) in groups.iter().enumerate() {
        if remap[index].is_none() || id_of[index] != 0 {
            continue;
        }
        while used.contains(&next_id) {
            next_id = next_id.saturating_add(1);
        }
        used.insert(next_id);
        id_of[index] = next_id;
        next_id = next_id.saturating_add(1);
    }
    // Parents are created before their children (pre-order walk), so pushing in index
    // order satisfies the "member group exists" invariant.
    for (index, group) in groups.iter().enumerate() {
        let Some(new_index) = remap[index] else {
            continue;
        };
        let mut children: Vec<GroupChild> = Vec::new();
        for (child, child_group) in groups.iter().enumerate() {
            if child_group.parent == Some(index) && remap[child].is_some() {
                children.push(GroupChild::Group(id_of[child]));
            }
        }
        for (leaf, shape_id) in leaves.iter().zip(shape_ids) {
            let Some(id) = shape_id else { continue };
            let mapped: Vec<usize> = leaf.iter().filter_map(|&i| remap[i]).collect();
            if mapped.last() == Some(&new_index) {
                children.push(GroupChild::Shape(*id));
            }
        }
        doc.groups.push(Group {
            id: id_of[index],
            name: group.name.clone(),
            children,
        });
    }
    hints
}
pub fn polylines_of(bez: &BezPath) -> Vec<Polyline> {
    path_to_polylines(bez, stencilcraft_geom::DEFAULT_FLATTEN_STEPS)
}

#[cfg(test)]
mod tests_group_walk {
    use super::*;

    #[test]
    fn groups_are_preserved() {
        let imported = import_with_warnings(
            r#"<svg><g class="stencilcraft-cut"><path d="M 0 0 L 5 0 L 5 5 Z"/></g></svg>"#,
            "grouped.svg",
        )
        .unwrap();
        let doc = imported.document;
        assert_eq!(doc.shape_count(), 1);
        // A plain import synthesizes no operation layers; the shape starts Unassigned.
        assert!(doc.layers.is_empty());
        assert_eq!(doc.unassigned_shapes().count(), 1);
        // The source <g> nesting is preserved as a document group referencing the shape.
        assert_eq!(doc.groups.len(), 1);
        assert_eq!(doc.groups[0].children, vec![GroupChild::Shape(1)]);
        // The group's cut class is collected as a hint, never applied.
        assert_eq!(imported.operation_hints.len(), 1);
        assert_eq!(imported.operation_hints[0].operation, OperationKind::Cut);
        assert_eq!(imported.operation_hints[0].group_path.len(), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_doc::Shape as DocShape;

    /// The `i`-th member shape of `layer` in `doc`.
    fn member(doc: &Document, layer: usize, i: usize) -> &DocShape {
        doc.get_shape(doc.layers[layer].members[i]).unwrap()
    }

    /// The `i`-th shape in document order.
    fn nth_shape(doc: &Document, i: usize) -> &DocShape {
        doc.shapes().nth(i).unwrap()
    }

    /// Import with the Phase-2 legacy semantics (groups→layers, style→operation).
    fn legacy(svg: &str) -> SvgImport {
        import_with_warnings_policy(
            svg,
            "legacy.svg",
            &ImportPolicy {
                legacy_style_operations: true,
            },
        )
        .unwrap()
    }
    use craft_svg::paths::{
        MAX_CSS_BYTES, MAX_SVG_BYTES, MAX_SVG_DEPTH, MAX_SVG_NODES, MAX_SVG_WARNINGS,
    };
    use stencilcraft_geom::Point;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(x, y));
        p.line_to(Point::new(x + w, y));
        p.line_to(Point::new(x + w, y + h));
        p.line_to(Point::new(x, y + h));
        p.close_path();
        p
    }

    /// Adds one Cut operation holding every shape of the base layer (test helper).
    fn cut_all(doc: &mut Document) {
        let members = doc.layers[0].members.clone();
        if members.is_empty() {
            return;
        }
        let id = doc.next_operation_id();
        doc.operations.push(Operation {
            id,
            name: "cut".to_string(),
            kind: OperationKind::Cut,
            settings: OperationSettings::default(),
            color: None,
            visible: true,
            locked: false,
            members,
            icon: None,
        });
    }

    #[test]
    fn export_includes_board_size_in_viewbox() {
        let mut doc = Document::default();
        doc.push_shape(0, rect(10.0, 10.0, 20.0, 20.0)).unwrap();
        cut_all(&mut doc);
        let svg = export(&doc).unwrap();
        assert!(
            svg.contains("viewBox=\"0.000 0.000 100.000 150.000\""),
            "{svg}"
        );
        assert!(svg.contains("width=\"100.000mm\""));
        // One cut operation writes the production kind.
        assert!(svg.contains("data-kind=\"cut\""));
    }

    #[test]
    fn export_shapes_as_paths() {
        let mut doc = Document::default();
        doc.push_shape(0, rect(10.0, 10.0, 20.0, 20.0)).unwrap();
        cut_all(&mut doc);
        let svg = export(&doc).unwrap();
        assert!(svg.contains("d=\"M10"), "{svg}");
        assert!(svg.contains("class=\"stencilcraft-cut\""));
    }

    #[test]
    fn hidden_shapes_are_not_exported() {
        let mut doc = Document::default();
        doc.push_shape(0, rect(10.0, 10.0, 20.0, 20.0)).unwrap();
        doc.push_shape(0, rect(50.0, 50.0, 5.0, 5.0)).unwrap();
        doc.get_shape_mut(doc.layers[0].members[1]).unwrap().hidden = true;
        cut_all(&mut doc);
        let svg = export(&doc).unwrap();
        assert!(svg.contains("d=\"M10"), "{svg}");
        assert!(!svg.contains("d=\"M50"), "{svg}");
    }

    fn preview_piece(w: f64, h: f64, style: CutStyle) -> Document {
        let mut doc = Document {
            board: stencilcraft_doc::Board {
                width_mm: w,
                height_mm: h,
            },
            ..Document::default()
        };
        doc.layers.clear();
        doc.layers.push(stencilcraft_doc::Layer::new("cut"));
        let id = doc.push_shape(0, rect(0.0, 0.0, w, h)).unwrap().id;
        let op_id = ensure_operation(&mut doc, "cut", kind_of(style), None);
        doc.operations
            .iter_mut()
            .find(|o| o.id == op_id)
            .unwrap()
            .members
            .push(id);
        doc
    }

    #[test]
    fn tile_preview_pages_every_sheet_with_stroked_frames() {
        let pieces = vec![
            preview_piece(60.0, 80.0, CutStyle::Cut),
            preview_piece(60.0, 80.0, CutStyle::Engrave),
        ];
        let svg = export_tile_preview(
            &pieces,
            &TilePreviewOptions {
                columns: 2,
                scale: 0.5,
            },
        )
        .unwrap();
        // Side by side: 2 cells of 30 × 40 mm plus three 6 mm gaps across, two 6 mm
        // gaps down.
        assert!(svg.contains("width=\"78.000mm\""), "{svg}");
        assert!(svg.contains("height=\"52.000mm\""), "{svg}");
        assert_eq!(svg.matches("class=\"sheet\"").count(), 2, "{svg}");
        assert_eq!(svg.matches("class=\"preview-frame\"").count(), 2, "{svg}");
        // One cut sheet and one engrave sheet, each drawn with its own class.
        assert_eq!(svg.matches("class=\"stencilcraft-cut\"").count(), 1);
        assert_eq!(svg.matches("class=\"stencilcraft-engrave\"").count(), 1);
        // Outlines, not fills: the whole point of the preview is to show the detail.
        assert!(svg.contains(".preview-frame { fill: none;"), "{svg}");
        assert!(svg.contains(".stencilcraft-cut { fill: none;"), "{svg}");
        // Content is flipped into the page's y-down space and anchored to the cell's
        // bottom edge: 6 mm gap + 40 mm cell = 46 mm.
        assert!(
            svg.contains("translate(6.000 46.000) scale(0.5000 -0.5000)"),
            "{svg}"
        );
        assert!(svg.contains("data-sheet=\"1\""), "{svg}");
    }

    #[test]
    fn tile_preview_labels_each_sheet() {
        let pieces = vec![
            preview_piece(60.0, 80.0, CutStyle::Cut),
            preview_piece(60.0, 80.0, CutStyle::Cut),
        ];
        let svg = export_tile_preview(
            &pieces,
            &TilePreviewOptions {
                columns: 1,
                scale: 0.5,
            },
        )
        .unwrap();
        assert!(svg.contains(">sheet 1 of 2 · 60×80 mm</text>"), "{svg}");
        assert!(svg.contains(">sheet 2 of 2 · 60×80 mm</text>"), "{svg}");
        // Column-major tiling stacks the first sheet at the bottom of the page.
        let first = svg.find("data-sheet=\"0\"").unwrap();
        let second = svg.find("data-sheet=\"1\"").unwrap();
        let first_y = &svg[first..second];
        assert!(
            first_y.contains("y=\"52.000\""),
            "sheet 0 should sit below sheet 1: {svg}"
        );
    }

    #[test]
    fn tile_preview_refuses_impossible_pages() {
        let pieces = vec![preview_piece(60.0, 80.0, CutStyle::Cut)];
        assert!(matches!(
            export_tile_preview(&[], &TilePreviewOptions::default()),
            Err(SvgError::Model(_))
        ));
        assert!(matches!(
            export_tile_preview(
                &pieces,
                &TilePreviewOptions {
                    columns: 0,
                    scale: 1.0
                }
            ),
            Err(SvgError::Model(_))
        ));
        assert!(matches!(
            export_tile_preview(
                &pieces,
                &TilePreviewOptions {
                    columns: 1,
                    scale: 0.0
                }
            ),
            Err(SvgError::Model(_))
        ));
        let huge = vec![preview_piece(200_000.0, 200_000.0, CutStyle::Cut)];
        assert!(matches!(
            export_tile_preview(
                &huge,
                &TilePreviewOptions {
                    columns: 1,
                    scale: 1.0
                }
            ),
            Err(SvgError::Model(_))
        ));
    }

    #[test]
    fn import_roundtrips_path_data() {
        let mut doc = Document::default();
        doc.push_shape(0, rect(0.0, 0.0, 30.0, 40.0)).unwrap();
        cut_all(&mut doc);
        let svg = export(&doc).unwrap();
        let back = import(&svg, "test.svg").unwrap();
        assert_eq!(back.shape_count(), 1);
        assert_eq!(
            nth_shape(&back, 0).path.to_svg(),
            member(&doc, 0, 0).path.to_svg()
        );
    }

    #[test]
    fn import_gives_calm_errors() {
        assert!(matches!(
            import("<path></path>", "nod.svg"),
            Err(SvgError::Import(ref m)) if m.contains("d attribute")
        ));
        assert!(matches!(
            import("<path d=\"M 0 0 Q\"/>", "bad.svg"),
            Err(SvgError::Import(_))
        ));
        let transformed = import(
            "<g transform=\"scale(nope)\"><path d=\"M0 0 L1 0 Z\"/></g>",
            "tx.svg",
        );
        assert!(matches!(
            transformed,
            Err(SvgError::Import(ref m)) if m.contains("transform")
        ));
    }

    #[test]
    fn import_grows_board_for_outside_art() {
        let back = import("<path d=\"M 200 250 L 210 250 L 210 260 Z\"/>", "big.svg").unwrap();
        assert!(back.board.width_mm >= 210.0);
        assert!(back.board.height_mm >= 260.0);
    }

    #[test]
    fn imports_all_common_basic_shapes() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <rect x="1" y="2" width="10" height="8"/>
          <circle cx="20" cy="20" r="5"/>
          <ellipse cx="40" cy="20" rx="8" ry="4"/>
          <line x1="1" y1="30" x2="20" y2="30"/>
          <polyline points="1,40 10,45 20,40"/>
          <polygon points="30,40 40,40 35,50"/>
          <path d="M50 50 C55 45 60 55 65 50 Z"/>
        </svg>"#;
        let doc = import(svg, "shapes.svg").unwrap();
        assert_eq!(doc.shape_count(), 7);
        assert!(doc.layers.is_empty());
        assert!(nth_shape(&doc, 0).path.elements().len() >= 5);
        assert!(nth_shape(&doc, 1).path.elements().len() >= 6);
    }

    #[test]
    fn imports_nested_group_transforms_and_inherited_production_style() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <g transform="translate(10,20)" data-kind="engrave">
            <g scale="2" transform="scale(2)">
              <rect x="1" y="2" width="3" height="4"/>
            </g>
          </g>
        </svg>"#;
        // New semantics: nesting becomes groups, transforms still resolve, and no
        // operation layers are synthesized — all shapes start Unassigned.
        let imported = import_with_warnings(svg, "groups.svg").unwrap();
        let doc = imported.document;
        assert_eq!(doc.shape_count(), 1);
        assert!(doc.layers.is_empty());
        assert_eq!(doc.unassigned_shapes().count(), 1);
        assert_eq!(doc.groups.len(), 2);
        let outer = &doc.groups[0];
        let inner = &doc.groups[1];
        assert!(matches!(outer.children.as_slice(), [GroupChild::Group(id)] if *id == inner.id));
        assert_eq!(inner.children, vec![GroupChild::Shape(1)]);
        // The inherited engrave styling is a hint on the outer group, not a layer.
        assert_eq!(imported.operation_hints.len(), 1);
        assert_eq!(
            imported.operation_hints[0].operation,
            OperationKind::Engrave
        );
        assert_eq!(imported.operation_hints[0].group_path.len(), 1);
        let b = nth_shape(&doc, 0).bounds().unwrap();
        assert!((b.x0 - 12.0).abs() < 1e-9, "{b:?}");
        assert!((b.y0 - 24.0).abs() < 1e-9, "{b:?}");
        assert!((b.width() - 6.0).abs() < 1e-9, "{b:?}");
        // Legacy branch: the old two-layer result (cut base + engrave layer).
        let legacy = legacy(svg).document;
        assert_eq!(legacy.layers.len(), 2);
        assert!(
            legacy
                .operations
                .iter()
                .any(|o| o.kind == OperationKind::Engrave)
        );
        let b = member(&legacy, 1, 0).bounds().unwrap();
        assert!((b.x0 - 12.0).abs() < 1e-9, "{b:?}");
        assert!((b.y0 - 24.0).abs() < 1e-9, "{b:?}");
        assert!((b.width() - 6.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn export_import_round_trips_the_group_tree() {
        let mut doc = Document::default();
        let first = doc.push_shape(0, rect(0.0, 0.0, 5.0, 5.0)).unwrap().id;
        let second = doc.push_shape(0, rect(10.0, 10.0, 5.0, 5.0)).unwrap().id;
        cut_all(&mut doc);
        let inner_id = 10;
        doc.groups.push(Group {
            id: inner_id,
            name: "inner".into(),
            children: vec![GroupChild::Shape(second)],
        });
        let outer_id = 11;
        doc.groups.push(Group {
            id: outer_id,
            name: "outer".into(),
            children: vec![GroupChild::Shape(first), GroupChild::Group(inner_id)],
        });
        let svg = export(&doc).unwrap();
        let back = import(&svg, "roundtrip.svg").unwrap();
        assert_eq!(back.shape_count(), 2);
        assert!(back.layers.is_empty());
        // The exported data-group ids come back as the same group ids (export carries
        // group identity, not names).
        let outer = back.group(outer_id).unwrap();
        assert!(outer.children.contains(&GroupChild::Shape(first)));
        assert!(outer.children.contains(&GroupChild::Group(inner_id)));
        assert_eq!(outer.children.len(), 2);
        let inner = back.group(inner_id).unwrap();
        assert_eq!(inner.children, vec![GroupChild::Shape(second)]);
    }

    #[test]
    fn imports_local_use_with_x_y_transform_and_inherited_style() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <defs>
            <g id="shape" data-kind="engrave" transform="scale(2)">
              <rect x="1" y="2" width="3" height="4"/>
            </g>
            <symbol id="badge" viewBox="10 20 20 10" preserveAspectRatio="xMidYMid meet" data-kind="engrave">
              <rect x="10" y="20" width="20" height="10"/>
            </symbol>
          </defs>
          <use href="#shape" x="10" y="20" transform="translate(5,0)"/>
          <use xlink:href="#shape" x="30" y="5"
            xmlns:xlink="http://www.w3.org/1999/xlink"/>
          <use href="#badge" x="50" y="10" width="40" height="40"/>
        </svg>"##;
        let imported = import_with_warnings(svg, "use.svg").unwrap();
        let doc = imported.document;
        assert_eq!(doc.shape_count(), 3);
        assert!(doc.layers.is_empty());
        let first = nth_shape(&doc, 0).bounds().unwrap();
        assert!((first.x0 - 17.0).abs() < 1e-9, "{first:?}");
        assert!((first.y0 - 24.0).abs() < 1e-9, "{first:?}");
        assert!((first.width() - 6.0).abs() < 1e-9, "{first:?}");
        let second = nth_shape(&doc, 1).bounds().unwrap();
        assert!((second.x0 - 32.0).abs() < 1e-9, "{second:?}");
        assert!((second.y0 - 9.0).abs() < 1e-9, "{second:?}");
        let badge = nth_shape(&doc, 2).bounds().unwrap();
        assert!((badge.x0 - 50.0).abs() < 1e-9, "{badge:?}");
        assert!((badge.y0 - 20.0).abs() < 1e-9, "{badge:?}");
        assert!((badge.width() - 40.0).abs() < 1e-9, "{badge:?}");
        assert!((badge.height() - 20.0).abs() < 1e-9, "{badge:?}");
        // The defs' engrave styling is collected as a hint, not applied to a layer.
        assert!(
            imported
                .operation_hints
                .iter()
                .any(|hint| hint.operation == OperationKind::Engrave)
        );
    }

    #[test]
    fn local_use_cycles_missing_and_external_references_fail_calmly() {
        for svg in [
            r##"<svg><defs><g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g></defs><use href="#a"/></svg>"##,
            r##"<svg><use href="#missing"/></svg>"##,
            r#"<svg><use href="other.svg#shape"/></svg>"#,
            r##"<svg><defs><text id="label">no</text></defs><use href="#label"/></svg>"##,
            r##"<svg><rect id="same"/><circle id="same"/></svg>"##,
            r##"<svg><defs><symbol id="cycle"><use href="#cycle"/></symbol></defs><use href="#cycle"/></svg>"##,
            r##"<svg><defs><symbol id="bad" viewBox="0 0 0 4"><rect width="2" height="2"/></symbol></defs><use href="#bad"/></svg>"##,
            r##"<svg><defs><symbol id="zero" viewBox="0 0 4 4"><rect width="2" height="2"/></symbol></defs><use href="#zero" width="-1"/></svg>"##,
        ] {
            assert!(import(svg, "use-error.svg").is_err(), "{svg}");
        }
    }

    #[test]
    fn viewbox_maps_physical_mm_and_preserves_aspect_ratio() {
        for preserve in ["xMidYMid meet extra", "notxMidYMid"] {
            let svg = format!(
                r#"<svg width="200mm" height="100mm" viewBox="0 0 100 100" preserveAspectRatio="{preserve}"/>"#
            );
            assert!(import(&svg, "aspect.svg").is_err(), "{preserve}");
        }
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200mm" height="100mm"
          viewBox="0 0 100 100" preserveAspectRatio="none">
          <rect x="10" y="20" width="5" height="4"/>
        </svg>"#;
        let doc = import(svg, "viewbox.svg").unwrap();
        assert_eq!(doc.board.width_mm, 200.0);
        assert_eq!(doc.board.height_mm, 100.0);
        let b = nth_shape(&doc, 0).bounds().unwrap();
        assert!((b.x0 - 20.0).abs() < 1e-9, "{b:?}");
        assert!((b.y0 - 20.0).abs() < 1e-9, "{b:?}");
        assert!((b.width() - 10.0).abs() < 1e-9, "{b:?}");
        assert!((b.height() - 4.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn imports_hidden_visibility_and_inline_styles() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <g style="display:none"><rect width="10" height="10"/></g>
          <g style="visibility:hidden" data-kind="engrave">
            <line style="visibility:visible" x1="1" y1="2" x2="3" y2="4"/>
            <line x1="5" y1="6" x2="7" y2="8"/>
          </g>
        </svg>"#;
        let imported = import_with_warnings(svg, "style.svg").unwrap();
        let doc = imported.document;
        // The display:none group and the hidden line drop out; one shape survives,
        // unassigned, inside the visibility group that carries the visible line.
        assert_eq!(doc.shape_count(), 1);
        assert!(doc.layers.is_empty());
        assert_eq!(doc.groups.len(), 1);
        assert_eq!(doc.groups[0].children.len(), 1);
        assert!(matches!(doc.groups[0].children[0], GroupChild::Shape(_)));
        assert_eq!(imported.operation_hints.len(), 1);
        assert_eq!(
            imported.operation_hints[0].operation,
            OperationKind::Engrave
        );
    }

    #[test]
    fn applies_css_selector_specificity_cascade_and_inherited_visibility() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <style>
            /* supported selector subset */
            path { data-kind: cut; }
            .mark { data-kind: engrave; visibility: hidden; }
            g#source > path.mark { visibility: visible; }
            path.mark[id="first"] { display: none; }
            path.mark[data-kind="engrave"] { stroke: red; }
            .mark { data-kind: cut !important; }
          </style>
          <g id="source">
            <path id="first" class="mark" d="M1 1L2 1"/>
            <path id="second" class="mark" style="data-kind:engrave" d="M3 3L4 3"/>
          </g>
        </svg>"##;
        // CSS-driven hiding is a legacy-mode concern: the new import preserves groups
        // and collects styling as hints instead of building layers.
        let doc = legacy(svg).document;
        assert_eq!(doc.shape_count(), 1);
        assert_eq!(doc.layers[0].members.len(), 1);
        assert_eq!(member(&doc, 0, 0).bounds().unwrap().x0, 3.0);
        assert_eq!(doc.layers.len(), 1);
    }

    #[test]
    fn rejects_unsupported_css_selectors_and_caps_stylesheet_size() {
        assert!(
            import(
                r##"<svg><style>path:hover { display:none; }</style><path d="M0 0L1 1"/></svg>"##,
                "pseudo.svg"
            )
            .is_err()
        );
        let large = format!(
            "<svg><style>{}</style></svg>",
            " ".repeat(MAX_CSS_BYTES + 1)
        );
        assert!(import(&large, "css-large.svg").is_err());
    }

    #[test]
    fn reports_unsupported_drawables_instead_of_dropping_them() {
        let svg =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text x="0" y="10">hello</text></svg>"#;
        assert!(matches!(
            import(svg, "text.svg"),
            Err(SvgError::Import(ref message)) if message.contains("<text> \"hello\" skipped")
        ));
        for svg in [
            r#"<svg><image href="a.png" width="1" height="1"/></svg>"#,
            r#"<svg><foreignObject/></svg>"#,
            r#"<svg><blink/></svg>"#,
        ] {
            assert!(import_with_warnings(svg, "drawable.svg").is_err(), "{svg}");
        }
    }

    #[test]
    fn rejects_nonfinite_geometry_and_malformed_number_lists() {
        assert!(import(r#"<svg viewBox="0 0 NaN 100"/>"#, "nan.svg").is_err());
        assert!(import(r#"<svg><polygon points="0,0,,1,1"/></svg>"#, "points.svg").is_err());
        assert!(
            import(
                r#"<svg><rect x="1e400" width="10" height="10"/></svg>"#,
                "huge.svg"
            )
            .is_err()
        );
        assert!(
            import(
                &format!("<svg>{}</svg>", " ".repeat(MAX_SVG_BYTES)),
                "large.svg"
            )
            .is_err()
        );
    }

    #[test]
    fn parses_transform_lists_and_keeps_physical_px_units() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="96px" height="48px">
          <g transform="translate(10 0) scale(2)"><line x1="0" y1="0" x2="4" y2="0"/></g>
        </svg>"#;
        let doc = import(svg, "px.svg").unwrap();
        assert!((doc.board.width_mm - 25.4).abs() < 1e-9);
        assert!((doc.board.height_mm - 12.7).abs() < 1e-9);
        let b = nth_shape(&doc, 0).bounds().unwrap();
        assert!((b.x0 - 10.0 * 25.4 / 96.0).abs() < 1e-9, "{b:?}");
        assert!((b.width() - 8.0 * 25.4 / 96.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn malformed_xml_and_resource_limits_are_errors_not_panics() {
        assert!(matches!(
            import("<svg><g></svg>", "bad-xml.svg"),
            Err(SvgError::Import(ref e)) if e.contains("invalid XML")
        ));
        let deep = format!(
            "{}<path d=\"M0 0L1 0\"/>{}",
            "<g>".repeat(MAX_SVG_DEPTH + 2),
            "</g>".repeat(MAX_SVG_DEPTH + 2)
        );
        assert!(matches!(
            import(&deep, "deep.svg"),
            Err(SvgError::Import(ref e)) if e.contains("nesting exceeds")
        ));
        let nodes = format!("<svg>{}</svg>", "<g/>".repeat(MAX_SVG_NODES + 1));
        assert!(matches!(
            import(&nodes, "nodes.svg"),
            Err(SvgError::Import(ref e)) if e.contains("100,000-node")
        ));
    }

    #[test]
    fn deterministically_mutated_svg_never_panics() {
        let seeds = [
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="2" y="3" width="7" height="8"/></svg>"#,
            // Colour, CSS, clip, text, mask and filter paths in one seed.
            r##"<svg viewBox="0 0 20 20"><style>@media print{g{x:y}} g>rect:not(.a)+rect[id^="r"]{stroke:hsl(9,50%,50%)}</style><clipPath id="c" transform="scale(1)"><circle cx="5" cy="5" r="4"/></clipPath><g clip-path="url(#c)" color="red"><rect width="9" height="9" stroke="currentColor"/><rect id="r2" width="3" height="3"/><path d="M0 0L20 20"/></g><text id="t">hi</text><g mask="url(#m)" filter="url(#f)"><rect width="1" height="1" stroke="#0f08"/></g></svg>"##,
        ];
        for (seed_index, seed) in seeds.iter().enumerate() {
            let bytes = seed.as_bytes();
            let mut state = 0x51_6f_9a_u64 + seed_index as u64;
            for _ in 0..1024 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let index = (state as usize) % bytes.len();
                let mut mutated = bytes.to_vec();
                mutated[index] = (state >> 32) as u8;
                if let Ok(text) = std::str::from_utf8(&mutated)
                    && let Ok(imported) = import_with_warnings(text, "mutated.svg")
                {
                    imported.document.validate().unwrap();
                    assert!(imported.warnings.len() <= MAX_SVG_WARNINGS + 1);
                }
            }
        }
    }

    fn lenient(svg: &str) -> SvgImport {
        import_with_warnings(svg, "test.svg").unwrap()
    }

    fn layer<'a>(doc: &'a Document, name: &str) -> &'a stencilcraft_doc::Layer {
        doc.layers
            .iter()
            .find(|layer| layer.name == name)
            .unwrap_or_else(|| panic!("no layer {name}: {:?}", doc.layers))
    }

    #[test]
    fn stroke_colours_become_one_layer_each() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <style>.blue { stroke: rgb(0 0 100%); }</style>
          <rect width="1" height="1" stroke="red"/>
          <rect width="1" height="1" style="stroke:#F00"/>
          <rect width="1" height="1" stroke="hsl(0, 100%, 50%)"/>
          <rect width="1" height="1" class="blue"/>
          <g color="rgba(0, 128, 0, 0.5)"><rect width="1" height="1" stroke="currentColor"/></g>
          <rect width="1" height="1" stroke="url(#gradient) cornflowerblue"/>
          <rect width="1" height="1" stroke="url(#gradient)"/>
          <rect width="1" height="1" fill="black"/>
          <rect width="1" height="1" stroke="none" fill="black"/>
          <g data-kind="engrave"><rect width="1" height="1" stroke="#00800080"/></g>
        </svg>"##;
        let imported = legacy(svg);
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        assert!(imported.operation_hints.is_empty());
        let doc = imported.document;
        assert_eq!(doc.shape_count(), 10);
        assert_eq!(doc.layers[0].name, "cut");
        assert_eq!(doc.layers[0].members.len(), 3);
        fn op<'a>(doc: &'a Document, name: &str) -> &'a Operation {
            doc.operations.iter().find(|o| o.name == name).unwrap()
        }
        let red = op(&doc, "Line #ff0000");
        assert_eq!(red.members.len(), 3);
        assert_eq!(red.color.as_deref(), Some("#ff0000"));
        assert_eq!(red.kind, OperationKind::Cut);
        assert_eq!(op(&doc, "Line #0000ff").members.len(), 1);
        assert_eq!(op(&doc, "Line #008000").members.len(), 1);
        assert_eq!(op(&doc, "Line #6495ed").members.len(), 1);
        let engrave = op(&doc, "engrave #008000");
        assert_eq!(engrave.kind, OperationKind::Engrave);
        assert_eq!(engrave.color.as_deref(), Some("#008000"));
        // The strict (new-semantics) import of the same file keeps every shape and
        // synthesizes no layers.
        let strict = import(svg, "colours.svg").unwrap();
        assert_eq!(strict.shape_count(), 10);
        assert!(strict.layers.is_empty());
    }

    #[test]
    fn unreadable_stroke_colour_warns_and_strict_import_refuses() {
        let svg = r#"<svg><rect id="odd" width="1" height="1" stroke="color(srgb 1 0 0)"/></svg>"#;
        let imported = legacy(svg);
        assert_eq!(imported.document.layers[0].members.len(), 1);
        assert_eq!(imported.warnings.len(), 1);
        assert!(
            imported.warnings[0].contains("<rect id=\"odd\">"),
            "{:?}",
            imported.warnings
        );
        assert!(import(svg, "odd.svg").is_err());
    }

    fn bounds_of(doc: &Document) -> Vec<kurbo::Rect> {
        doc.shapes().map(|shape| shape.bounds().unwrap()).collect()
    }

    fn near(a: kurbo::Rect, b: (f64, f64, f64, f64)) -> bool {
        (a.x0 - b.0).abs() < 1e-3
            && (a.y0 - b.1).abs() < 1e-3
            && (a.x1 - b.2).abs() < 1e-3
            && (a.y1 - b.3).abs() < 1e-3
    }

    #[test]
    fn clip_paths_clip_closed_and_open_geometry() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <defs>
            <clipPath id="half"><rect x="0" y="0" width="10" height="20"/></clipPath>
            <clipPath id="moved" transform="translate(50 0)"><rect width="5" height="5"/></clipPath>
            <clipPath id="disc"><circle cx="30" cy="30" r="5"/></clipPath>
            <clipPath id="nothing"/>
          </defs>
          <g clip-path="url(#half)">
            <rect width="20" height="20"/>
            <line x1="-5" y1="5" x2="25" y2="5"/>
          </g>
          <rect x="40" y="0" width="20" height="20" clip-path="url('#moved')"/>
          <rect x="20" y="20" width="20" height="20" style="clip-path: url(#disc)"/>
          <g transform="translate(0 60)"><rect width="20" height="20" clip-path="url(#half)"/></g>
          <rect width="20" height="20" clip-path="url(#nothing)"/>
          <rect x="80" y="80" width="5" height="5" clip-path="url(#half)"/>
        </svg>"##;
        let imported = lenient(svg);
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let bounds = bounds_of(&imported.document);
        assert_eq!(bounds.len(), 5, "{bounds:?}");
        assert!(near(bounds[0], (0.0, 0.0, 10.0, 20.0)), "{bounds:?}");
        assert!(near(bounds[1], (0.0, 5.0, 10.0, 5.0)), "{bounds:?}");
        assert!(near(bounds[2], (50.0, 0.0, 55.0, 5.0)), "{bounds:?}");
        assert!(near(bounds[3], (25.0, 25.0, 35.0, 35.0)), "{bounds:?}");
        // The clip region lives in the referencing element's user space.
        assert!(near(bounds[4], (0.0, 60.0, 10.0, 80.0)), "{bounds:?}");
        // The clipped closed shape stays closed (a cut contour, not an open run).
        let first = &nth_shape(&imported.document, 0).path;
        assert!(matches!(
            first.elements().last(),
            Some(kurbo::PathEl::ClosePath)
        ));
    }

    #[test]
    fn clip_paths_that_cannot_be_applied_warn() {
        for (svg, needle) in [
            (
                r##"<svg><clipPath id="c" clipPathUnits="objectBoundingBox"><rect width="1" height="1"/></clipPath><rect width="9" height="9" clip-path="url(#c)"/></svg>"##,
                "objectBoundingBox",
            ),
            (
                r##"<svg><rect width="9" height="9" clip-path="url(#missing)"/></svg>"##,
                "not a local <clipPath>",
            ),
            (
                r##"<svg><rect width="9" height="9" clip-path="inset(1px)"/></svg>"##,
                "not a local <clipPath>",
            ),
            (
                r##"<svg><rect id="r" width="1" height="1"/><rect width="9" height="9" clip-path="url(#r)"/></svg>"##,
                "not a <clipPath>",
            ),
            (
                r##"<svg><clipPath id="c"><text>A</text><rect width="1" height="1"/></clipPath><rect width="9" height="9" clip-path="url(#c)"/></svg>"##,
                "inside a clip path",
            ),
        ] {
            let imported = lenient(svg);
            assert!(
                imported.warnings.iter().any(|w| w.contains(needle)),
                "{svg}: {:?}",
                imported.warnings
            );
            assert!(imported.document.shape_count() >= 1, "{svg}");
            assert!(import(svg, "clip.svg").is_err(), "{svg}");
        }
    }

    #[test]
    fn advanced_css_selectors_match_per_selectors_level_3() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <style>
            rect[data-tag~="b"] { stroke: #000001; }
            rect[id^="pre"] { stroke: #000002; }
            rect[id$="post"] { stroke: #000003; }
            rect[id*="mid"] { stroke: #000004; }
            rect[lang|="en"] { stroke: #000005; }
            g > line:first-child { stroke: #000006; }
            rect:not(.keep):last-child { stroke: #000007; }
            #anchor + rect { stroke: #000008; }
            #anchor ~ circle { stroke: #000009; }
            :root > rect.top { stroke: #00000a; }
          </style>
          <rect class="top" width="1" height="1"/>
          <g>
            <line x2="1"/>
            <rect class="keep" data-tag="a b c" width="1" height="1"/>
            <rect id="prefix" width="1" height="1"/>
            <rect id="the-post" width="1" height="1"/>
            <rect id="amidb" width="1" height="1"/>
            <rect lang="en-GB" width="1" height="1"/>
            <rect id="anchor" class="keep" width="1" height="1"/>
            <rect width="1" height="1"/>
            <circle r="1"/>
            <rect class="keep" width="1" height="1"/>
            <rect width="1" height="1"/>
          </g>
        </svg>"##;
        let imported = legacy(svg);
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let names: Vec<_> = imported
            .document
            .layers
            .iter()
            .map(|layer| (layer.name.as_str(), layer.members.len()))
            .collect();
        for n in 1..=10 {
            let name = format!("Line #{n:06x}");
            assert!(
                names
                    .iter()
                    .any(|(layer, count)| *layer == name && *count == 1),
                "{name}: {names:?}"
            );
        }
        // Unmatched: the anchor, and the last-but-one `.keep` rect (excluded by :not).
        assert_eq!(imported.document.layers[0].members.len(), 2);
    }

    #[test]
    fn css_at_rules_and_unsupported_selectors_warn_instead_of_failing() {
        let svg = r##"<svg><style>
            @charset "utf-8";
            @import url(other.css);
            @font-face { font-family: X; src: url(x.woff); }
            @media (max-width: 10px) { rect { display: none; } }
            rect:hover, rect.ok { stroke: red; }
            a::before { stroke: blue; }
          </style><rect class="ok" width="1" height="1"/></svg>"##;
        let imported = legacy(svg);
        let warnings = imported.warnings.join("\n");
        assert!(warnings.contains("@import"), "{warnings}");
        assert!(warnings.contains("@media"), "{warnings}");
        assert!(!warnings.contains("font-face"), "{warnings}");
        assert!(!warnings.contains("charset"), "{warnings}");
        assert!(warnings.contains("rect:hover"), "{warnings}");
        assert!(warnings.contains("a::before"), "{warnings}");
        // The @media rule is not applied; the plain rule after it still is.
        assert_eq!(layer(&imported.document, "Line #ff0000").members.len(), 1);
        assert!(import(svg, "media.svg").is_err());
        for broken in [
            "<svg><style>@media screen { rect { x: y; }</style></svg>",
            "<svg><style>@import</style></svg>",
            "<svg><style>rect } {</style></svg>",
            "<svg><style>rect:not(:not(.a)) { stroke: red; }</style><rect/></svg>",
        ] {
            let result = import_with_warnings(broken, "broken.svg");
            assert!(
                result.is_err() || result.is_ok_and(|i| !i.warnings.is_empty()),
                "{broken}"
            );
        }
    }

    #[test]
    fn text_masks_and_filters_warn_by_name() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
          <text id="title" x="0" y="10" font-size="12">Hello <tspan>  wide   world</tspan></text>
          <text display="none">hidden</text>
          <g visibility="hidden"><text>also hidden</text></g>
          <rect id="masked" width="1" height="1" mask="url(#m)"/>
          <g filter="url(#shadow)"><rect width="1" height="1"/></g>
          <rect width="1" height="1" style="filter:none; mask:none"/>
        </svg>"##;
        let imported = lenient(svg);
        assert_eq!(imported.document.shape_count(), 3);
        assert_eq!(imported.warnings.len(), 3, "{:?}", imported.warnings);
        assert!(imported.warnings[0].contains(r#"<text id="title"> "Hello wide world""#));
        assert!(imported.warnings[0].contains("convert text to paths"));
        assert!(imported.warnings[1].contains(r#"<rect id="masked">: mask ignored"#));
        assert!(imported.warnings[2].contains("<g>: filter ignored"));
        // Masks and filters do not inherit, so the filtered group warns once.
        assert!(import(svg, "text.svg").is_err());
    }

    #[test]
    fn warnings_are_capped() {
        let svg = format!(
            "<svg>{}</svg>",
            (0..200)
                .map(|i| format!("<text id=\"t{i}\">x</text>"))
                .collect::<String>()
        );
        let imported = lenient(&svg);
        assert_eq!(imported.warnings.len(), MAX_SVG_WARNINGS + 1);
        assert!(imported.warnings[MAX_SVG_WARNINGS].contains("136 more"));
    }
}
