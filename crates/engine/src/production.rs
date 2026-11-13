//! Cut-production commands: Cricut-style layer actions (Slice, Weld, Attach, Flatten, Contour,
//! non-destructive Combine, Create Sticker), per-shape production settings, and the queries
//! that hand mat layouts, cut jobs and Print Then Cut sheets to the machine layer and printers.
//!
//! Edits change `Engine::doc` directly and are undone through the engine's document snapshots;
//! queries change nothing. Geometry runs in millimetres through VectorCraft's path operations.

use std::collections::BTreeSet;

use kurbo::BezPath;
use serde::{Deserialize, Serialize};
use stencilcraft_doc::{CombineOp, Document, GroupKind, Operation, SendAssign, Shape, SourceGroup};
use stencilcraft_layout as layout;
use vectorcraft_geom::{FillRule, PathData};

use crate::{Effect, EngineError};

const MAX_IDS: usize = 10_000;
/// Matches the layout's mat cap.
const MAX_MAT: u32 = 500;

/// Sticker cut depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StickerCut {
    /// Through the sticker only; the backing sheet stays whole.
    #[default]
    KissCut,
    /// Through sticker and backing.
    DieCut,
}

/// Production fields `shapes.set-production` can reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductionField {
    Operation,
    LineColor,
    FillColor,
    Material,
    Send,
}

/// Production commands, serialised with the same `command`/`params` envelope as [`crate::Command`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", content = "params")]
pub enum ProductionCommand {
    /// Split two overlapping closed shapes into every piece: a − b, b − a and a ∩ b, each
    /// separate region its own shape. The two originals are removed.
    #[serde(rename = "shapes.slice")]
    Slice { a: u64, b: u64 },
    /// Join shapes into one outline for good (the first shape in document order keeps its id).
    #[serde(rename = "shapes.weld")]
    Weld { ids: Vec<u64> },
    /// Keep shapes in their arrangement on one mat and cut them together.
    #[serde(rename = "shapes.attach")]
    Attach { ids: Vec<u64> },
    /// Release the Attach groups the shapes belong to.
    #[serde(rename = "shapes.detach")]
    Detach { ids: Vec<u64> },
    /// Merge shapes into one Print Then Cut image whose cut line is their outline.
    #[serde(rename = "shapes.flatten")]
    Flatten { ids: Vec<u64> },
    /// Split a flattened image or sticker back into its shapes.
    #[serde(rename = "shapes.unflatten")]
    Unflatten { id: u64 },
    /// Hide (or show) contours of a shape: they stay in the path but are not cut or exported.
    #[serde(rename = "shape.contour")]
    Contour {
        id: u64,
        contours: Vec<usize>,
        hidden: bool,
    },
    /// Non-destructive Combine: the result keeps its members, so it can be changed or undone.
    #[serde(rename = "shapes.combine")]
    Combine { ids: Vec<u64>, operation: CombineOp },
    /// Undo a Combine, restoring its members where the combined shape now is.
    #[serde(rename = "shapes.uncombine")]
    Uncombine { id: u64 },
    /// Change a Combine's operation.
    #[serde(rename = "shape.set-combine")]
    SetCombine { id: u64, operation: CombineOp },
    /// Flatten the shapes and give them an offset border, welded, as a sticker.
    #[serde(rename = "shapes.create-sticker")]
    CreateSticker {
        ids: Vec<u64>,
        #[serde(default = "default_border")]
        border_mm: f64,
        #[serde(default)]
        cut: StickerCut,
    },
    /// Set operation, colours, material or send-preview tool assignment; `clear` resets fields.
    #[serde(rename = "shapes.set-production")]
    SetProduction {
        ids: Vec<u64>,
        #[serde(default)]
        operation: Option<Operation>,
        #[serde(default)]
        line_color: Option<String>,
        #[serde(default)]
        fill_color: Option<String>,
        #[serde(default)]
        material: Option<String>,
        #[serde(default)]
        send: Option<SendAssign>,
        #[serde(default)]
        clear: Vec<ProductionField>,
    },
    /// Put pieces on a given mat (`null` returns them to automatic layout).
    #[serde(rename = "shapes.move-to-mat")]
    MoveToMat { ids: Vec<u64>, mat: Option<u32> },
    /// Query: the machine profiles (mats, roll and Print Then Cut limits).
    #[serde(rename = "production.machines")]
    Machines {},
    /// Query: the mat layout (Prepare screen).
    #[serde(rename = "production.mats")]
    Mats(layout::MatOptions),
    /// Query: the cut job for the machine layer (Make / Send).
    #[serde(rename = "production.job")]
    Job(layout::JobOptions),
    /// Query: the Print Then Cut sheets (marks, artwork, bleed).
    #[serde(rename = "production.print-sheet")]
    PrintSheet(layout::PrintOptions),
}

fn default_border() -> f64 {
    3.0
}

/// What a production query returns.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ProductionEffect {
    Machines(&'static [layout::MachineProfile]),
    Mats(layout::MatLayout),
    Job(layout::CutJob),
    PrintSheets(Vec<layout::PrintSheet>),
}

impl ProductionCommand {
    pub fn is_query(&self) -> bool {
        matches!(
            self,
            Self::Machines {} | Self::Mats(_) | Self::Job(_) | Self::PrintSheet(_)
        )
    }

    /// Shapes the command edits (a locked one refuses it).
    pub fn targets(&self) -> Vec<u64> {
        match self {
            Self::Slice { a, b } => vec![*a, *b],
            Self::Weld { ids }
            | Self::Attach { ids }
            | Self::Detach { ids }
            | Self::Flatten { ids }
            | Self::Combine { ids, .. }
            | Self::CreateSticker { ids, .. }
            | Self::SetProduction { ids, .. }
            | Self::MoveToMat { ids, .. } => ids.clone(),
            Self::Unflatten { id }
            | Self::Contour { id, .. }
            | Self::Uncombine { id }
            | Self::SetCombine { id, .. } => vec![*id],
            _ => Vec::new(),
        }
    }
}

fn bad<T>(name: &str, message: impl std::fmt::Display) -> Result<T, EngineError> {
    Err(EngineError::Invalid(format!("{name}: {message}")))
}

pub(crate) fn query(doc: &Document, command: &ProductionCommand) -> Result<Effect, EngineError> {
    let wrap = |name: &str, e: layout::LayoutError| EngineError::Invalid(format!("{name}: {e}"));
    let effect = match command {
        ProductionCommand::Machines {} => ProductionEffect::Machines(layout::MACHINES),
        ProductionCommand::Mats(o) => ProductionEffect::Mats(
            layout::layout_mats(doc, o).map_err(|e| wrap("production.mats", e))?,
        ),
        ProductionCommand::Job(o) => {
            ProductionEffect::Job(layout::build_job(doc, o).map_err(|e| wrap("production.job", e))?)
        }
        ProductionCommand::PrintSheet(o) => ProductionEffect::PrintSheets(
            layout::print_sheets(doc, o).map_err(|e| wrap("production.print-sheet", e))?,
        ),
        _ => return bad("production", "not a query"),
    };
    Ok(Effect::Production(effect))
}

/// Runs an editing production command on `doc` (the engine has taken the undo snapshot).
pub(crate) fn execute(
    doc: &mut Document,
    command: ProductionCommand,
) -> Result<Effect, EngineError> {
    match command {
        ProductionCommand::Slice { a, b } => slice(doc, a, b),
        ProductionCommand::Weld { ids } => weld(doc, &ids),
        ProductionCommand::Attach { ids } => {
            let ids = checked(doc, &ids, "shapes.attach", 2)?;
            let group = ids.first().copied();
            for id in &ids {
                if let Some(s) = doc.get_shape_mut(*id) {
                    s.production.attach = group;
                }
            }
            Ok(Effect::ShapesEdited { ids })
        }
        ProductionCommand::Detach { ids } => {
            let ids = checked(doc, &ids, "shapes.detach", 1)?;
            let groups: BTreeSet<u64> = ids
                .iter()
                .filter_map(|id| doc.get_shape(*id).and_then(|s| s.production.attach))
                .collect();
            if groups.is_empty() {
                return bad("shapes.detach", "none of the shapes is attached");
            }
            let mut edited = Vec::new();
            for layer in &mut doc.layers {
                for s in &mut layer.shapes {
                    if s.production.attach.is_some_and(|g| groups.contains(&g)) {
                        s.production.attach = None;
                        edited.push(s.id);
                    }
                }
            }
            Ok(Effect::ShapesEdited { ids: edited })
        }
        ProductionCommand::Flatten { ids } => {
            let name = "shapes.flatten";
            let ids = checked(doc, &ids, name, 1)?;
            let (at, members) = take(doc, &ids)?;
            let members = expand_flattened(members);
            let outline = combine_paths(&members, CombineOp::Unite, name)?;
            let template = members.first().cloned();
            let id = group_shape(doc, at, template, outline, GroupKind::Flatten, members)?;
            if let Some(s) = doc.get_shape_mut(id) {
                s.production.operation = Some(Operation::PrintThenCut);
            }
            Ok(Effect::ShapeAdded { id })
        }
        ProductionCommand::Unflatten { id } => {
            ungroup(doc, id, "shapes.unflatten", |k| k == GroupKind::Flatten)
        }
        ProductionCommand::Contour {
            id,
            contours,
            hidden,
        } => {
            let name = "shape.contour";
            let Some(shape) = doc.get_shape_mut(id) else {
                return bad(name, format!("no shape has id {id}"));
            };
            let count = shape
                .path
                .elements()
                .iter()
                .filter(|el| matches!(el, kurbo::PathEl::MoveTo(_)))
                .count();
            if contours.is_empty() {
                return bad(name, "name at least one contour");
            }
            if let Some(c) = contours.iter().find(|c| **c >= count) {
                return bad(
                    name,
                    format!("shape {id} has {count} contours, no contour {c}"),
                );
            }
            let mut set: BTreeSet<usize> =
                shape.production.hidden_contours.iter().copied().collect();
            for c in contours {
                if hidden {
                    set.insert(c);
                } else {
                    set.remove(&c);
                }
            }
            shape.production.hidden_contours = set.into_iter().collect();
            Ok(Effect::ShapesEdited { ids: vec![id] })
        }
        ProductionCommand::Combine { ids, operation } => {
            let name = "shapes.combine";
            let ids = checked(doc, &ids, name, 2)?;
            let (at, members) = take(doc, &ids)?;
            let path = combine_paths(&members, operation, name)?;
            let template = members.first().cloned();
            let id = group_shape(
                doc,
                at,
                template,
                path,
                GroupKind::Combine(operation),
                members,
            )?;
            Ok(Effect::ShapeAdded { id })
        }
        ProductionCommand::Uncombine { id } => ungroup(doc, id, "shapes.uncombine", |k| {
            matches!(k, GroupKind::Combine(_))
        }),
        ProductionCommand::SetCombine { id, operation } => {
            let name = "shape.set-combine";
            let Some(shape) = doc.get_shape(id) else {
                return bad(name, format!("no shape has id {id}"));
            };
            let Some(group) = shape
                .production
                .group
                .as_deref()
                .filter(|g| matches!(g.kind, GroupKind::Combine(_)))
            else {
                return bad(name, format!("shape {id} is not a Combine"));
            };
            let members = group.members_now(&shape.path);
            let path = combine_paths(&members, operation, name)?;
            if let Some(s) = doc.get_shape_mut(id) {
                s.path = path.clone();
                s.production.group = Some(Box::new(SourceGroup {
                    kind: GroupKind::Combine(operation),
                    members,
                    path,
                }));
            }
            Ok(Effect::ShapesEdited { ids: vec![id] })
        }
        ProductionCommand::CreateSticker {
            ids,
            border_mm,
            cut,
        } => {
            let name = "shapes.create-sticker";
            if !(border_mm.is_finite() && (0.5..=50.0).contains(&border_mm)) {
                return bad(name, format!("border_mm must be 0.5–50, got {border_mm}"));
            }
            let ids = checked(doc, &ids, name, 1)?;
            let (at, members) = take(doc, &ids)?;
            let members = expand_flattened(members);
            let union = combine_paths(&members, CombineOp::Unite, name)?;
            let grown = vectorcraft_pathops::offset_path(
                &PathData::from_bezpath(&union),
                border_mm,
                vectorcraft_pathops::Join::Round,
                4.0,
            )
            .to_bezpath();
            // A sticker is solid: keep each piece's outer contour, drop the holes.
            let mut outline = BezPath::new();
            for region in layout::split_regions(&grown) {
                if let Some((_, outer)) = layout::split_contours(&region).into_iter().next() {
                    outline.extend(outer.iter());
                }
            }
            if outline.elements().is_empty() {
                return bad(name, "the border came out empty");
            }
            let template = members.first().cloned();
            let id = group_shape(doc, at, template, outline, GroupKind::Flatten, members)?;
            if let Some(s) = doc.get_shape_mut(id) {
                s.production.operation = Some(match cut {
                    StickerCut::KissCut => Operation::KissCut,
                    StickerCut::DieCut => Operation::DieCut,
                });
            }
            Ok(Effect::ShapeAdded { id })
        }
        ProductionCommand::SetProduction {
            ids,
            operation,
            line_color,
            fill_color,
            material,
            send,
            clear,
        } => {
            let name = "shapes.set-production";
            let ids = checked(doc, &ids, name, 1)?;
            let color = |c: Option<String>| -> Result<Option<String>, EngineError> {
                c.map(|c| {
                    craft_svg::color::parse_color(&c)
                        .map(|c| c.to_css())
                        .map_err(|e| EngineError::Invalid(format!("{name}: {e}")))
                })
                .transpose()
            };
            let (line_color, fill_color) = (color(line_color)?, color(fill_color)?);
            let material = material.map(|m| m.trim().to_string());
            if material
                .as_ref()
                .is_some_and(|m| m.is_empty() || m.chars().count() > 100)
            {
                return bad(name, "material must be 1–100 characters");
            }
            for id in &ids {
                let Some(s) = doc.get_shape_mut(*id) else {
                    continue;
                };
                let p = &mut s.production;
                for f in &clear {
                    match f {
                        ProductionField::Operation => p.operation = None,
                        ProductionField::LineColor => p.line_color = None,
                        ProductionField::FillColor => p.fill_color = None,
                        ProductionField::Material => p.material = None,
                        ProductionField::Send => p.send = None,
                    }
                }
                p.operation = operation.or(p.operation);
                p.line_color = line_color.clone().or(p.line_color.take());
                p.fill_color = fill_color.clone().or(p.fill_color.take());
                p.material = material.clone().or(p.material.take());
                p.send = send.or(p.send);
            }
            Ok(Effect::ShapesEdited { ids })
        }
        ProductionCommand::MoveToMat { ids, mat } => {
            let name = "shapes.move-to-mat";
            let ids = checked(doc, &ids, name, 1)?;
            if mat.is_some_and(|m| m >= MAX_MAT) {
                return bad(name, format!("mat must be below {MAX_MAT}"));
            }
            for id in &ids {
                if let Some(s) = doc.get_shape_mut(*id) {
                    s.production.mat = mat;
                }
            }
            Ok(Effect::ShapesEdited { ids })
        }
        ProductionCommand::Machines {}
        | ProductionCommand::Mats(_)
        | ProductionCommand::Job(_)
        | ProductionCommand::PrintSheet(_) => query(doc, &command),
    }
}

/// Unique, existing ids (at least `min`), in document order.
fn checked(doc: &Document, ids: &[u64], name: &str, min: usize) -> Result<Vec<u64>, EngineError> {
    let set: BTreeSet<u64> = ids.iter().copied().collect();
    if set.len() != ids.len() || ids.len() < min || ids.len() > MAX_IDS {
        return bad(
            name,
            format!(
                "expected {min}–{MAX_IDS} unique shape ids, got {}",
                ids.len()
            ),
        );
    }
    if let Some(missing) = ids.iter().find(|id| doc.get_shape(**id).is_none()) {
        return bad(name, format!("no shape has id {missing}"));
    }
    Ok(doc
        .shapes()
        .filter(|(_, s)| set.contains(&s.id))
        .map(|(_, s)| s.id)
        .collect())
}

/// Removes the shapes, returning where the first one was `(layer, index)` and the shapes.
fn take(doc: &mut Document, ids: &[u64]) -> Result<((usize, usize), Vec<Shape>), EngineError> {
    let mut at = None;
    let mut taken = Vec::new();
    for (li, layer) in doc.layers.iter_mut().enumerate() {
        let mut i = 0;
        while i < layer.shapes.len() {
            if layer.shapes.get(i).is_some_and(|s| ids.contains(&s.id)) {
                at.get_or_insert((li, i));
                taken.push(layer.shapes.remove(i));
            } else {
                i += 1;
            }
        }
    }
    match at {
        Some(at) => Ok((at, taken)),
        None => Err(EngineError::Internal(
            "production: no shapes to take".to_string(),
        )),
    }
}

fn alloc_id(doc: &mut Document) -> Result<u64, EngineError> {
    let id = doc.next_shape_id;
    doc.next_shape_id = id
        .checked_add(1)
        .ok_or_else(|| EngineError::Internal("shape ids exhausted".to_string()))?;
    Ok(id)
}

fn insert(
    doc: &mut Document,
    (layer, index): (usize, usize),
    shapes: Vec<Shape>,
) -> Result<(), EngineError> {
    let Some(layer) = doc.layers.get_mut(layer) else {
        return Err(EngineError::Internal(
            "production: layer vanished".to_string(),
        ));
    };
    let index = index.min(layer.shapes.len());
    layer.shapes.splice(index..index, shapes);
    Ok(())
}

/// Flattened members print their own artwork, so flattening again keeps that artwork.
fn expand_flattened(members: Vec<Shape>) -> Vec<Shape> {
    members
        .into_iter()
        .flat_map(|m| match m.production.group.as_deref() {
            Some(g) if g.kind == GroupKind::Flatten => g.members_now(&m.path),
            _ => vec![m],
        })
        .collect()
}

fn group_shape(
    doc: &mut Document,
    at: (usize, usize),
    template: Option<Shape>,
    path: BezPath,
    kind: GroupKind,
    members: Vec<Shape>,
) -> Result<u64, EngineError> {
    let Some(template) = template else {
        return Err(EngineError::Internal("production: empty group".to_string()));
    };
    let id = alloc_id(doc)?;
    let mut production = template.production.clone();
    production.attach = None;
    production.mat = None;
    production.hidden_contours.clear();
    production.group = Some(Box::new(SourceGroup {
        kind,
        members,
        path: path.clone(),
    }));
    let shape = Shape {
        id,
        path,
        production,
        hidden: false,
        locked: false,
        ..template
    };
    insert(doc, at, vec![shape])?;
    Ok(id)
}

fn ungroup(
    doc: &mut Document,
    id: u64,
    name: &str,
    accepts: impl Fn(GroupKind) -> bool,
) -> Result<Effect, EngineError> {
    let Some(shape) = doc.get_shape(id) else {
        return bad(name, format!("no shape has id {id}"));
    };
    let Some(group) = shape
        .production
        .group
        .as_deref()
        .filter(|g| accepts(g.kind))
    else {
        return bad(name, format!("shape {id} has nothing to separate"));
    };
    let mut members = group.members_now(&shape.path);
    let (at, _) = take(doc, &[id])?;
    // Ids may be taken meanwhile (a duplicated group restores twice): give those fresh ones.
    let mut used: BTreeSet<u64> = doc.shapes().map(|(_, s)| s.id).collect();
    for m in &mut members {
        if !used.insert(m.id) {
            m.id = alloc_id(doc)?;
            used.insert(m.id);
        }
        doc.next_shape_id = doc.next_shape_id.max(m.id.saturating_add(1));
    }
    let ids = members.iter().map(|m| m.id).collect();
    insert(doc, at, members)?;
    Ok(Effect::ShapesAdded { ids })
}

fn data(path: &BezPath) -> PathData {
    PathData::from_bezpath(path)
}

fn closed_only(shapes: &[Shape], name: &str) -> Result<(), EngineError> {
    for s in shapes {
        let open = layout::split_contours(&s.cut_path())
            .iter()
            .any(|(_, c)| !matches!(c.elements().last(), Some(kurbo::PathEl::ClosePath)));
        if open {
            return bad(
                name,
                format!("shape {} has an open path; close it first", s.id),
            );
        }
    }
    Ok(())
}

fn combine_paths(shapes: &[Shape], op: CombineOp, name: &str) -> Result<BezPath, EngineError> {
    closed_only(shapes, name)?;
    let paths: Vec<PathData> = shapes.iter().map(|s| data(&s.cut_path())).collect();
    let inputs: Vec<(&PathData, FillRule)> = paths.iter().map(|p| (p, FillRule::EvenOdd)).collect();
    let result = vectorcraft_pathops::try_boolean_n(&inputs, |inside: &[bool]| match op {
        CombineOp::Unite => inside.iter().any(|b| *b),
        CombineOp::Intersect => inside.iter().all(|b| *b),
        CombineOp::Subtract => {
            inside.first().copied().unwrap_or(false) && !inside.iter().skip(1).any(|b| *b)
        }
        CombineOp::Exclude => inside.iter().filter(|b| **b).count() % 2 == 1,
    })
    .map_err(|e| EngineError::Invalid(format!("{name}: {e}")))?
    .to_bezpath();
    if result.elements().is_empty() {
        return bad(name, "the result is empty");
    }
    Ok(result)
}

fn slice(doc: &mut Document, a: u64, b: u64) -> Result<Effect, EngineError> {
    let name = "shapes.slice";
    if a == b {
        return bad(name, "select two different shapes");
    }
    let ids = checked(doc, &[a, b], name, 2)?;
    let (Some(sa), Some(sb)) = (doc.get_shape(a).cloned(), doc.get_shape(b).cloned()) else {
        return bad(name, "both shapes must exist");
    };
    closed_only(&[sa.clone(), sb.clone()], name)?;
    let (pa, pb) = (data(&sa.cut_path()), data(&sb.cut_path()));
    let boolean = |x: &PathData, y: &PathData, op| {
        vectorcraft_pathops::try_boolean(x, FillRule::EvenOdd, y, FillRule::EvenOdd, op, 0.001)
            .map(|p| p.to_bezpath())
            .map_err(|e| EngineError::Invalid(format!("{name}: {e}")))
    };
    let both = boolean(&pa, &pb, vectorcraft_pathops::BoolOp::Intersect)?;
    if both.elements().is_empty() {
        return bad(name, format!("shapes {a} and {b} do not overlap"));
    }
    // The overlap takes the colour of the shape on top (the later one in document order).
    let top = if ids.last() == Some(&a) { &sa } else { &sb };
    let pieces = [
        (
            boolean(&pa, &pb, vectorcraft_pathops::BoolOp::Difference)?,
            &sa,
        ),
        (
            boolean(&pb, &pa, vectorcraft_pathops::BoolOp::Difference)?,
            &sb,
        ),
        (both, top),
    ];
    let (at, _) = take(doc, &[a, b])?;
    let mut added = Vec::new();
    let mut shapes = Vec::new();
    for (path, source) in pieces {
        for region in layout::split_regions(&path) {
            let id = alloc_id(doc)?;
            let mut production = source.production.clone();
            production.group = None;
            production.attach = None;
            production.hidden_contours.clear();
            shapes.push(Shape {
                id,
                path: region,
                production,
                ..source.clone()
            });
            added.push(id);
        }
    }
    insert(doc, at, shapes)?;
    Ok(Effect::ShapesAdded { ids: added })
}

fn weld(doc: &mut Document, ids: &[u64]) -> Result<Effect, EngineError> {
    let name = "shapes.weld";
    let ids = checked(doc, ids, name, 2)?;
    let shapes: Vec<Shape> = ids
        .iter()
        .filter_map(|id| doc.get_shape(*id).cloned())
        .collect();
    let path = combine_paths(&shapes, CombineOp::Unite, name)?;
    let Some(keep) = ids.first().copied() else {
        return bad(name, "nothing to weld");
    };
    let rest: Vec<u64> = ids.iter().skip(1).copied().collect();
    take(doc, &rest)?;
    if let Some(s) = doc.get_shape_mut(keep) {
        s.path = path;
        s.production.group = None;
        s.production.hidden_contours.clear();
    }
    Ok(Effect::ShapesEdited { ids: vec![keep] })
}
