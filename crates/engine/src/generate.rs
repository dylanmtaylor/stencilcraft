//! Generator and effect commands (`generate.*`): thin adapters from engine shapes to the
//! pure geometry in `stencilcraft-generate`. Every command runs inside the VectorCraft undo
//! group `Engine::run` opens, so it is one undo step; new shapes go through
//! `vector_add_paths`, in-place edits through VectorCraft's `path.setAnchors` (ids stay),
//! and conversions remove the sources and add the results (new ids).

use kurbo::{Affine, BezPath, Shape as _};
use serde::{Deserialize, Serialize};
use serde_json::json;
use stencilcraft_doc::{CutStyle, Tip, ToolOp};
use stencilcraft_generate as generate;

use crate::{Command, Effect, Engine, EngineError, POINTS_PER_MM, transform_path, unique_ids};

/// `generate.shape`: a parametric shape in a box; `replace` regenerates an existing shape
/// in place (same id), which is how its parameters are edited.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapeArgs {
    pub shape: generate::shapes::ShapeKind,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub replace: Option<u64>,
}

/// `generate.score-line`: a straight score (fold) line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreLineArgs {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    #[serde(default)]
    pub tip: Tip,
    #[serde(default = "one")]
    pub passes: u32,
}

fn one() -> u32 {
    1
}

/// `generate.fill`: sketch/stipple/tool pattern fill of each shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FillArgs {
    pub ids: Vec<u64>,
    #[serde(flatten)]
    pub params: generate::fill::FillParams,
    /// Tool tag for the fill paths (`power-engrave`, `heat-pen`, `emboss`…); none = pen.
    #[serde(default)]
    pub tool: Option<ToolOp>,
}

/// `generate.tool-lines`: turn outlines into score/emboss (or other tool) lines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolLinesArgs {
    pub ids: Vec<u64>,
    pub tool: ToolOp,
    /// Also fill the inside with this pattern, tagged with the same tool.
    #[serde(default)]
    pub fill: Option<generate::fill::FillParams>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RhinestoneArgs {
    pub ids: Vec<u64>,
    #[serde(flatten)]
    pub params: generate::rhinestone::RhinestoneParams,
}

/// `generate.punch`: replace each path with punch points along it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PunchArgs {
    pub ids: Vec<u64>,
    #[serde(default = "punch_spacing")]
    pub spacing_mm: f64,
    #[serde(default = "punch_size")]
    pub diameter_mm: f64,
}

fn punch_spacing() -> f64 {
    3.5
}
fn punch_size() -> f64 {
    3.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PuzzleArgs {
    pub id: u64,
    #[serde(flatten)]
    pub params: generate::puzzle::PuzzleParams,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeedArgs {
    pub ids: Vec<u64>,
    #[serde(flatten)]
    pub params: generate::weed::WeedParams,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistrationArgs {
    pub ids: Vec<u64>,
    #[serde(default = "mark_margin")]
    pub margin_mm: f64,
    #[serde(default = "mark_arm")]
    pub arm_mm: f64,
    #[serde(default)]
    pub mark: generate::weed::MarkStyle,
    /// `cut` (default: layered vinyl) or `engrave` (drawn/printed guides).
    #[serde(default = "cut")]
    pub style: CutStyle,
}

fn mark_margin() -> f64 {
    5.0
}
fn mark_arm() -> f64 {
    8.0
}
fn cut() -> CutStyle {
    CutStyle::Cut
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WarpArgs {
    pub ids: Vec<u64>,
    #[serde(flatten)]
    pub params: generate::warp::WarpParams,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerforateArgs {
    pub ids: Vec<u64>,
    pub cut_mm: f64,
    pub gap_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WavyArgs {
    pub ids: Vec<u64>,
    pub amplitude_mm: f64,
    pub wavelength_mm: f64,
}

/// `generate.object-on-path`: copies of `ids` (as a group) along `path_id`'s outline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectOnPathArgs {
    pub ids: Vec<u64>,
    pub path_id: u64,
    #[serde(flatten)]
    pub params: generate::along::AlongParams,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReplicateDirection {
    /// Stacked exactly on the selection.
    OnTop,
    Left,
    Right,
    Above,
    Below,
    /// Along `angle_deg` (counter-clockwise from +x).
    Angle,
}

/// `generate.replicate`: Advanced Replicate — `count` copies in a direction, `gap_mm` apart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicateArgs {
    pub ids: Vec<u64>,
    pub direction: ReplicateDirection,
    #[serde(default)]
    pub angle_deg: f64,
    #[serde(default = "one")]
    pub count: u32,
    #[serde(default)]
    pub gap_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdsArgs {
    pub ids: Vec<u64>,
}

/// Most copies one replicate/object-on-path command adds.
const MAX_COPIES: usize = 10_000;

impl Engine {
    pub(crate) fn generate(&mut self, command: Command) -> Result<Effect, EngineError> {
        match command {
            Command::GenerateShape(a) => {
                let path = generate::shapes::shape(&a.shape, a.x, a.y, a.width, a.height)
                    .map_err(|e| invalid("generate.shape", e))?;
                match a.replace {
                    Some(id) => {
                        self.sources(&[id], "generate.shape")?;
                        self.set_paths(vec![(id, path)], "generate.shape")
                    }
                    None => {
                        let ids =
                            self.vector_add_paths(vec![(path, CutStyle::Cut)], "generate.shape")?;
                        Ok(Effect::ShapesAdded { ids })
                    }
                }
            }
            Command::GenerateScoreLine(a) => {
                const NAME: &str = "generate.score-line";
                let ends = [a.x0, a.y0, a.x1, a.y1];
                if ends.iter().any(|v| !v.is_finite() || v.abs() > 100_000.0)
                    || (a.x0, a.y0) == (a.x1, a.y1)
                {
                    return Err(EngineError::Invalid(format!(
                        "{NAME}: expected two distinct finite end points"
                    )));
                }
                let tool = ToolOp::Score {
                    tip: a.tip,
                    passes: a.passes,
                };
                check_tool(&tool, NAME)?;
                let mut p = BezPath::new();
                p.move_to((a.x0, a.y0));
                p.line_to((a.x1, a.y1));
                self.add_tagged(vec![(p, CutStyle::Engrave, Some(tool))], NAME)
            }
            Command::GenerateFill(a) => {
                const NAME: &str = "generate.fill";
                if let Some(t) = &a.tool {
                    check_tool(t, NAME)?;
                }
                let mut out = Vec::new();
                for (_, path, _) in self.sources(&a.ids, NAME)? {
                    let fill =
                        generate::fill::fill(&path, &a.params).map_err(|e| invalid(NAME, e))?;
                    if !fill.elements().is_empty() {
                        out.push((fill, CutStyle::Engrave, a.tool.clone()));
                    }
                }
                self.add_tagged(out, NAME)
            }
            Command::GenerateToolLines(a) => {
                const NAME: &str = "generate.tool-lines";
                check_tool(&a.tool, NAME)?;
                if matches!(a.tool, ToolOp::Punch { .. }) {
                    return Err(EngineError::Invalid(format!(
                        "{NAME}: use generate.punch for punch points"
                    )));
                }
                let sources = self.sources(&a.ids, NAME)?;
                let mut out = Vec::new();
                for (_, path, _) in &sources {
                    out.push((path.clone(), CutStyle::Engrave, Some(a.tool.clone())));
                    if let Some(params) = &a.fill {
                        let fill =
                            generate::fill::fill(path, params).map_err(|e| invalid(NAME, e))?;
                        if !fill.elements().is_empty() {
                            out.push((fill, CutStyle::Engrave, Some(a.tool.clone())));
                        }
                    }
                }
                self.remove_shapes(&a.ids, NAME)?;
                self.add_tagged(out, NAME)
            }
            Command::GenerateRhinestone(a) => {
                const NAME: &str = "generate.rhinestone";
                let mut out = Vec::new();
                for (_, path, _) in self.sources(&a.ids, NAME)? {
                    let stones = generate::rhinestone::rhinestones(&path, &a.params)
                        .map_err(|e| invalid(NAME, e))?;
                    if !stones.elements().is_empty() {
                        out.push((stones, CutStyle::Cut, None));
                    }
                }
                self.add_tagged(out, NAME)
            }
            Command::GeneratePunch(a) => {
                const NAME: &str = "generate.punch";
                let tool = ToolOp::Punch {
                    diameter_mm: a.diameter_mm,
                };
                check_tool(&tool, NAME)?;
                let mut out = Vec::new();
                for (_, path, _) in self.sources(&a.ids, NAME)? {
                    let points = generate::along::punch_points(&path, a.spacing_mm)
                        .map_err(|e| invalid(NAME, e))?;
                    let mut holes = BezPath::new();
                    for p in points {
                        holes
                            .extend(kurbo::Circle::new(p, a.diameter_mm / 2.0).path_elements(1e-3));
                    }
                    out.push((holes, CutStyle::Cut, Some(tool.clone())));
                }
                self.remove_shapes(&a.ids, NAME)?;
                self.add_tagged(out, NAME)
            }
            Command::GeneratePopup(params) => {
                const NAME: &str = "generate.popup";
                let card = generate::popup::popup(&params).map_err(|e| invalid(NAME, e))?;
                let tool = ToolOp::Score {
                    tip: Tip::Fine,
                    passes: 1,
                };
                self.add_tagged(
                    vec![
                        (card.cuts, CutStyle::Cut, None),
                        (card.scores, CutStyle::Engrave, Some(tool)),
                    ],
                    NAME,
                )
            }
            Command::GeneratePuzzle(a) => {
                const NAME: &str = "generate.puzzle";
                let (_, path, _) = self.one_source(a.id, NAME)?;
                let lines =
                    generate::puzzle::puzzle(&path, &a.params).map_err(|e| invalid(NAME, e))?;
                self.add_tagged(vec![(lines, CutStyle::Cut, None)], NAME)
            }
            Command::GenerateWeed(a) => {
                const NAME: &str = "generate.weed";
                let paths: Vec<BezPath> = self
                    .sources(&a.ids, NAME)?
                    .into_iter()
                    .map(|s| s.1)
                    .collect();
                let (boxed, lines) =
                    generate::weed::weed(&paths, &a.params).map_err(|e| invalid(NAME, e))?;
                let mut out = vec![(boxed, CutStyle::Cut, None)];
                if !lines.elements().is_empty() {
                    out.push((lines, CutStyle::Cut, None));
                }
                self.add_tagged(out, NAME)
            }
            Command::GenerateRegistration(a) => {
                const NAME: &str = "generate.registration";
                let bounds = self.bounds_of(&a.ids, NAME)?;
                let marks = generate::weed::registration(bounds, a.margin_mm, a.arm_mm, a.mark)
                    .map_err(|e| invalid(NAME, e))?;
                self.add_tagged(vec![(marks, a.style, None)], NAME)
            }
            Command::GenerateWarp(a) => self.edit_in_place(&a.ids, "generate.warp", |p| {
                generate::warp::warp(p, &a.params)
            }),
            Command::GeneratePerforate(a) => {
                self.edit_in_place(&a.ids, "generate.perforate", |p| {
                    generate::lines::perforate(p, a.cut_mm, a.gap_mm)
                })
            }
            Command::GenerateWavy(a) => self.edit_in_place(&a.ids, "generate.wavy", |p| {
                generate::lines::wavy(p, a.amplitude_mm, a.wavelength_mm)
            }),
            Command::GenerateObjectOnPath(a) => {
                const NAME: &str = "generate.object-on-path";
                if a.ids.contains(&a.path_id) {
                    return Err(EngineError::Invalid(format!(
                        "{NAME}: the path cannot be one of the objects"
                    )));
                }
                let (_, along, _) = self.one_source(a.path_id, NAME)?;
                let objects = self.sources(&a.ids, NAME)?;
                let centre = self.bounds_of(&a.ids, NAME)?.center();
                let placements = generate::along::object_on_path(&along, centre, &a.params)
                    .map_err(|e| invalid(NAME, e))?;
                self.add_copies(&objects, &placements, NAME)
            }
            Command::GenerateReplicate(a) => {
                const NAME: &str = "generate.replicate";
                if a.count == 0 || a.count as usize > MAX_COPIES {
                    return Err(EngineError::Invalid(format!(
                        "{NAME}: count must be between 1 and {MAX_COPIES}"
                    )));
                }
                if !a.gap_mm.is_finite() || a.gap_mm < 0.0 || !a.angle_deg.is_finite() {
                    return Err(EngineError::Invalid(format!(
                        "{NAME}: gap_mm must be finite and ≥ 0, angle_deg finite"
                    )));
                }
                let objects = self.sources(&a.ids, NAME)?;
                let b = self.bounds_of(&a.ids, NAME)?;
                let deg = match a.direction {
                    ReplicateDirection::OnTop => None,
                    ReplicateDirection::Right => Some(0.0),
                    ReplicateDirection::Above => Some(90.0),
                    ReplicateDirection::Left => Some(180.0),
                    ReplicateDirection::Below => Some(270.0),
                    ReplicateDirection::Angle => Some(a.angle_deg),
                };
                let step = deg.map_or(kurbo::Vec2::ZERO, |d: f64| {
                    let u = kurbo::Vec2::from_angle(d.to_radians());
                    // The selection's extent along `u`, so neighbours just clear each other.
                    u * (b.width() * u.x.abs() + b.height() * u.y.abs() + a.gap_mm)
                });
                let placements: Vec<Affine> = (1..=a.count)
                    .map(|k| Affine::translate(step * k as f64))
                    .collect();
                self.add_copies(&objects, &placements, NAME)
            }
            Command::GenerateRelease(a) => {
                const NAME: &str = "generate.release";
                let sources = self.sources(&a.ids, NAME)?;
                let mut out = Vec::new();
                for (_, path, style) in sources {
                    for contour in split_contours(&path) {
                        out.push((contour, style, None));
                    }
                }
                self.remove_shapes(&a.ids, NAME)?;
                self.add_tagged(out, NAME)
            }
            other => Err(EngineError::Internal(format!(
                "not a generator command: {other:?}"
            ))),
        }
    }

    /// The shapes behind `ids`: unique, existing and unlocked, in the order given.
    fn sources(
        &self,
        ids: &[u64],
        name: &str,
    ) -> Result<Vec<(u64, BezPath, CutStyle)>, EngineError> {
        unique_ids(ids, name)?
            .into_iter()
            .map(|id| {
                let shape = self
                    .doc
                    .get_shape(id)
                    .ok_or_else(|| EngineError::Invalid(format!("{name}: no shape has id {id}")))?;
                if shape.locked {
                    return Err(EngineError::Invalid(format!(
                        "shape {id} is locked; unlock it first"
                    )));
                }
                Ok((id, shape.path.clone(), shape.style))
            })
            .collect()
    }

    fn one_source(&self, id: u64, name: &str) -> Result<(u64, BezPath, CutStyle), EngineError> {
        self.sources(&[id], name)?
            .into_iter()
            .next()
            .ok_or_else(|| EngineError::Invalid(format!("{name}: no shape has id {id}")))
    }

    fn bounds_of(&self, ids: &[u64], name: &str) -> Result<kurbo::Rect, EngineError> {
        let mut acc: Option<kurbo::Rect> = None;
        for (_, path, _) in self.sources(ids, name)? {
            let b = generate::path_bounds(&path).map_err(|e| invalid(name, e))?;
            acc = Some(acc.map_or(b, |a| a.union(b)));
        }
        acc.ok_or_else(|| EngineError::Invalid(format!("{name}: nothing selected")))
    }

    /// Adds paths with their styles, then tags the new shapes with their tools.
    fn add_tagged(
        &mut self,
        paths: Vec<(BezPath, CutStyle, Option<ToolOp>)>,
        name: &str,
    ) -> Result<Effect, EngineError> {
        if paths.is_empty() {
            return Err(EngineError::Invalid(format!("{name}: the result is empty")));
        }
        if paths
            .iter()
            .any(|(p, _, _)| !p.elements().iter().all(crate::path_element_is_finite))
        {
            return Err(EngineError::Internal(format!(
                "{name}: generated non-finite geometry"
            )));
        }
        let tools: Vec<Option<ToolOp>> = paths.iter().map(|(_, _, t)| t.clone()).collect();
        let ids =
            self.vector_add_paths(paths.into_iter().map(|(p, s, _)| (p, s)).collect(), name)?;
        for (id, tool) in ids.iter().zip(tools) {
            if let Some(shape) = self.doc.get_shape_mut(*id) {
                shape.tool = tool;
            }
        }
        Ok(Effect::ShapesAdded { ids })
    }

    fn add_copies(
        &mut self,
        objects: &[(u64, BezPath, CutStyle)],
        placements: &[Affine],
        name: &str,
    ) -> Result<Effect, EngineError> {
        if objects.len().saturating_mul(placements.len()) > MAX_COPIES {
            return Err(EngineError::Invalid(format!(
                "{name}: more than {MAX_COPIES} copies"
            )));
        }
        let tools: Vec<Option<ToolOp>> = objects
            .iter()
            .map(|(id, _, _)| self.doc.get_shape(*id).and_then(|s| s.tool.clone()))
            .collect();
        let mut out = Vec::new();
        for t in placements {
            for ((_, path, style), tool) in objects.iter().zip(&tools) {
                out.push((*t * path.clone(), *style, tool.clone()));
            }
        }
        self.add_tagged(out, name)
    }

    /// Replaces each shape's path with `edit(path)`, keeping ids, style and tool.
    fn edit_in_place(
        &mut self,
        ids: &[u64],
        name: &str,
        edit: impl Fn(&BezPath) -> generate::Result<BezPath>,
    ) -> Result<Effect, EngineError> {
        let mut edits = Vec::new();
        for (id, path, _) in self.sources(ids, name)? {
            let out = edit(&path).map_err(|e| invalid(name, e))?;
            if out.elements().is_empty() {
                return Err(EngineError::Invalid(format!(
                    "{name}: shape {id} would be left empty"
                )));
            }
            edits.push((id, out));
        }
        self.set_paths(edits, name)
    }

    fn set_paths(&mut self, edits: Vec<(u64, BezPath)>, name: &str) -> Result<Effect, EngineError> {
        for (id, path) in &edits {
            if !path.elements().iter().all(crate::path_element_is_finite) {
                return Err(EngineError::Internal(format!(
                    "{name}: generated non-finite geometry"
                )));
            }
            let node = self.node_for_shape(*id, name)?;
            let data = vectorcraft_geom::PathData::from_bezpath(&transform_path(
                path,
                Affine::scale(POINTS_PER_MM),
            ));
            let subpaths: Vec<serde_json::Value> = data
                .subpaths
                .iter()
                .map(|sp| {
                    let anchors: Vec<serde_json::Value> = sp
                        .anchors
                        .iter()
                        .map(|a| json!({"x": a.p.x, "y": a.p.y, "in": [a.h_in.x, a.h_in.y], "out": [a.h_out.x, a.h_out.y]}))
                        .collect();
                    json!({"anchors": anchors, "closed": sp.closed})
                })
                .collect();
            self.session
                .execute(
                    "path.setAnchors",
                    &json!({"id": node.0, "subpaths": subpaths}),
                )
                .map_err(|e| EngineError::Invalid(format!("{name}: {e}")))?;
        }
        self.sync_doc_from_session()?;
        let ids = edits.iter().map(|(id, _)| *id).collect();
        // Keep the exact millimetre geometry rather than the session's point-snapped copy.
        for (id, path) in edits {
            if let Some(shape) = self.doc.get_shape_mut(id) {
                shape.path = path;
            }
        }
        Ok(Effect::ShapesEdited { ids })
    }
}

fn invalid(name: &str, e: generate::GenError) -> EngineError {
    EngineError::Invalid(format!("{name}: {e}"))
}

/// Score/emboss passes 1–10; punch diameter 0.5–20 mm.
fn check_tool(tool: &ToolOp, name: &str) -> Result<(), EngineError> {
    match *tool {
        ToolOp::Score { passes, .. } | ToolOp::Emboss { passes, .. }
            if !(1..=10).contains(&passes) =>
        {
            Err(EngineError::Invalid(format!(
                "{name}: passes must be between 1 and 10"
            )))
        }
        ToolOp::Punch { diameter_mm } if !(0.5..=20.0).contains(&diameter_mm) => Err(
            EngineError::Invalid(format!("{name}: punch diameter must be 0.5–20 mm")),
        ),
        _ => Ok(()),
    }
}

fn split_contours(path: &BezPath) -> Vec<BezPath> {
    let mut out: Vec<BezPath> = Vec::new();
    for el in path.elements() {
        if matches!(el, kurbo::PathEl::MoveTo(_)) || out.is_empty() {
            out.push(BezPath::new());
        }
        if let Some(last) = out.last_mut() {
            last.push(*el);
        }
    }
    out.retain(|p| p.elements().len() > 1);
    out
}
