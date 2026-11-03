//! Cut jobs (the Send step): per-mat operations in cutting order, with tools and passes.

use std::collections::BTreeMap;

use kurbo::{Affine, BezPath, ParamCurve, ParamCurveArclen, PathSeg, Point, Rect};
use serde::{Deserialize, Serialize};
use stencilcraft_doc::{Document, Operation, SendAssign, Shape};

use crate::machines::{MarkStyle, machine};
use crate::mats::{MatOptions, MatPlan, layout_mats};
use crate::{Result, check_range, invalid, is_closed, nesting_depths, split_contours};

/// What groups operations into rows of settings (Silhouette's Send sub-modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CutBy {
    /// One row per operation (cut, draw, score, kiss-cut, …).
    #[default]
    Operation,
    LineColor,
    FillColor,
    /// One row per document layer (`layer 0`, `layer 1`, …).
    Layer,
}

/// Settings for one group of operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupSetting {
    /// The group's key: an operation name, a `#rrggbb` colour, or `layer N`.
    pub key: String,
    /// Tool holder 1 or 2.
    #[serde(default = "one_u8")]
    pub tool: u8,
    /// Off: the group is left out of the job.
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub passes: Option<u32>,
    #[serde(default)]
    pub overcut_mm: Option<f64>,
}

fn one_u8() -> u8 {
    1
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CutOrder {
    /// Document order.
    Document,
    /// Work down the mat in the feed direction so the rollers rarely move the material back
    /// (by each contour's far edge, which also keeps counters ahead of their outlines).
    #[default]
    MinimizeRoller,
    /// Grouped by colour (line colour, else fill colour), in order of first appearance.
    ByColor,
    /// Grouped by document layer.
    ByLayer,
    /// Nearest next contour first, to keep travel short.
    ShortestTravel,
}

/// What the machine does when the job ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AfterJob {
    /// The tool goes back to the origin.
    #[default]
    ReturnToOrigin,
    /// The media is fed out past the cut.
    Feed,
}

/// Order of dots (stipple points: contours smaller than half a millimetre).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StippleOrder {
    /// The cut order's own placement.
    #[default]
    Default,
    EdgeToCenter,
    CenterToEdge,
}

/// A small test shape (a square around a triangle) cut before the job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestCut {
    #[serde(default)]
    pub mat: usize,
    /// Top-left corner of the test shape on the mat.
    pub x_mm: f64,
    pub y_mm: f64,
    #[serde(default = "ten")]
    pub size_mm: f64,
}

fn ten() -> f64 {
    10.0
}

/// Weeding aids: a box around each mat's pieces, and lines through the gaps between them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weed {
    #[serde(default = "three")]
    pub margin_mm: f64,
    #[serde(default = "yes")]
    pub lines: bool,
}

fn three() -> f64 {
    3.0
}

/// Which mat corner is the machine origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// The axis the material feeds along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Feed {
    /// Down the mat (y), the usual case.
    #[default]
    Length,
    /// Across: x and y swap (landscape feed).
    Width,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobOptions {
    pub mats: MatOptions,
    pub cut_by: CutBy,
    pub groups: Vec<GroupSetting>,
    pub order: CutOrder,
    /// Cut contours that lie inside others first (counters before outlines).
    pub inside_first: bool,
    /// Line-segment overcut: closed contours continue this far past their start (0–5 mm).
    pub overcut_mm: f64,
    /// Passes for groups without their own (1–10).
    pub passes: u32,
    pub test_cut: Option<TestCut>,
    pub weed: Option<Weed>,
    pub origin: Origin,
    pub feed: Feed,
    pub after: AfterJob,
    pub stipple_order: StippleOrder,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            mats: MatOptions::default(),
            cut_by: CutBy::default(),
            groups: Vec::new(),
            order: CutOrder::default(),
            inside_first: true,
            overcut_mm: 0.0,
            passes: 1,
            test_cut: None,
            weed: None,
            origin: Origin::default(),
            feed: Feed::default(),
            after: AfterJob::default(),
            stipple_order: StippleOrder::default(),
        }
    }
}

/// One contour to cut, draw or score, in mat coordinates.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CutOp {
    /// Source shape (0 for test cuts and weed lines).
    pub shape_id: u64,
    pub copy: u32,
    pub contour: usize,
    pub group: String,
    pub operation: Operation,
    pub tool: u8,
    pub passes: u32,
    /// The contour is closed (an overcut path still counts as closed).
    pub closed: bool,
    pub path: BezPath,
}

/// The marks the machine's sensor looks for, on a Print Then Cut mat.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Registration {
    pub style: MarkStyle,
    /// Filled mark rectangles `[x, y, width, height]`.
    pub marks: Vec<[f64; 4]>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MatJob {
    pub index: usize,
    pub size: String,
    pub width_mm: f64,
    pub length_mm: f64,
    pub material: Option<String>,
    pub color: Option<String>,
    pub mirror: bool,
    pub registration: Option<Registration>,
    /// Cut before the job, when a test cut was asked for on this mat.
    pub test_cut: Vec<CutOp>,
    pub ops: Vec<CutOp>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CutJob {
    pub machine: String,
    pub origin: Origin,
    pub feed: Feed,
    pub after: AfterJob,
    pub mats: Vec<MatJob>,
}

fn operation_name(op: Operation) -> &'static str {
    match op {
        Operation::Cut => "cut",
        Operation::KissCut => "kiss-cut",
        Operation::DieCut => "die-cut",
        Operation::Draw => "draw",
        Operation::Score => "score",
        Operation::Engrave => "engrave",
        Operation::Perforate => "perforate",
        Operation::PrintThenCut => "print-then-cut",
        Operation::Guide => "guide",
    }
}

fn group_key(by: CutBy, shape: &Shape, layer: usize) -> String {
    let style = || operation_name(shape.operation()).to_string();
    match by {
        CutBy::Operation => style(),
        CutBy::LineColor => shape.production.line_color.clone().unwrap_or_else(style),
        CutBy::FillColor => shape
            .production
            .fill_color
            .clone()
            .unwrap_or_else(|| "none".to_string()),
        CutBy::Layer => format!("layer {layer}"),
    }
}

fn validate(options: &JobOptions) -> Result<()> {
    check_range(options.overcut_mm, 0.0, 5.0, "overcut_mm")?;
    if !(1..=10).contains(&options.passes) {
        return invalid(format!("passes must be 1–10, got {}", options.passes));
    }
    if options.groups.len() > 1000 {
        return invalid("at most 1000 group settings");
    }
    for g in &options.groups {
        if !(1..=2).contains(&g.tool) {
            return invalid(format!("group {:?}: tool must be 1 or 2", g.key));
        }
        if let Some(p) = g.passes
            && !(1..=10).contains(&p)
        {
            return invalid(format!("group {:?}: passes must be 1–10", g.key));
        }
        if let Some(o) = g.overcut_mm {
            check_range(o, 0.0, 5.0, "group overcut_mm")?;
        }
    }
    if let Some(t) = &options.test_cut {
        check_range(t.size_mm, 2.0, 50.0, "test cut size_mm")?;
        check_range(t.x_mm, 0.0, 10_000.0, "test cut x_mm")?;
        check_range(t.y_mm, 0.0, 10_000.0, "test cut y_mm")?;
    }
    if let Some(w) = &options.weed {
        check_range(w.margin_mm, 0.0, 50.0, "weed margin_mm")?;
    }
    Ok(())
}

/// Builds the cut job: the mat layout, then each mat's operations in cutting order.
pub fn build_job(doc: &Document, options: &JobOptions) -> Result<CutJob> {
    validate(options)?;
    let layout = layout_mats(doc, &options.mats)?;
    let layers: BTreeMap<u64, usize> = doc
        .layers
        .iter()
        .enumerate()
        .flat_map(|(li, l)| l.shapes.iter().map(move |s| (s.id, li)))
        .collect();
    let mut mats = Vec::new();
    for mat in &layout.mats {
        let mut ops = mat_ops(doc, &layers, mat, options);
        order_ops(&mut ops, doc, &layers, options);
        order_stipple(&mut ops, options.stipple_order);
        for op in &mut ops {
            let overcut = options
                .groups
                .iter()
                .find(|g| g.key == op.group)
                .and_then(|g| g.overcut_mm)
                .unwrap_or(options.overcut_mm);
            if op.closed && overcut > 0.0 {
                op.path = with_overcut(&op.path, overcut);
            }
        }
        if let Some(weed) = &options.weed {
            ops.extend(weed_ops(mat, weed, options.passes));
        }
        let mut test_cut = Vec::new();
        if let Some(t) = options.test_cut.as_ref().filter(|t| t.mat == mat.index) {
            test_cut = test_cut_ops(mat, t, options.passes)?;
        }
        let registration = if mat.key.print_then_cut {
            let profile = machine(&layout.machine);
            let style = options
                .mats
                .page
                .marks
                .or(profile.map(|p| p.marks))
                .unwrap_or_default();
            Some(Registration {
                style,
                marks: crate::print::registration_marks(&options.mats.page, style),
            })
        } else {
            None
        };
        let (w, l) = (mat.width_mm, mat.length_mm);
        let finish = origin_map(options.origin, options.feed, w, l);
        for op in ops.iter_mut().chain(test_cut.iter_mut()) {
            op.path = finish * op.path.clone();
        }
        let mut registration = registration;
        if let Some(r) = registration.as_mut() {
            for m in &mut r.marks {
                let b = finish.transform_rect_bbox(Rect::new(m[0], m[1], m[0] + m[2], m[1] + m[3]));
                *m = [b.x0, b.y0, b.width(), b.height()];
            }
        }
        let (width_mm, length_mm) = match options.feed {
            Feed::Length => (w, l),
            Feed::Width => (l, w),
        };
        mats.push(MatJob {
            index: mat.index,
            size: mat.size.clone(),
            width_mm,
            length_mm,
            material: mat.key.material.clone(),
            color: mat.key.color.clone(),
            mirror: mat.mirror,
            registration,
            test_cut,
            ops,
        });
    }
    if let Some(t) = &options.test_cut
        && t.mat >= mats.len()
    {
        return invalid(format!(
            "test cut: mat {} does not exist (the job has {})",
            t.mat,
            mats.len()
        ));
    }
    Ok(CutJob {
        machine: layout.machine,
        origin: options.origin,
        feed: options.feed,
        after: options.after,
        mats,
    })
}

fn mat_ops(
    doc: &Document,
    layers: &BTreeMap<u64, usize>,
    mat: &MatPlan,
    options: &JobOptions,
) -> Vec<CutOp> {
    let mut ops = Vec::new();
    for placement in &mat.placements {
        let map = placement.affine();
        for id in &placement.shape_ids {
            let Some(shape) = doc.get_shape(*id) else {
                continue;
            };
            let layer = layers.get(id).copied().unwrap_or_default();
            let group = group_key(options.cut_by, shape, layer);
            let setting = options.groups.iter().find(|g| g.key == group);
            let assign = shape.production.send;
            if setting.is_some_and(|g| !g.enabled) || assign == Some(SendAssign::Ignore) {
                continue;
            }
            let tool = match assign {
                Some(SendAssign::Tool1 | SendAssign::Tool1Outline) => 1,
                Some(SendAssign::Tool2 | SendAssign::Tool2Outline) => 2,
                _ => setting.map_or(1, |g| g.tool),
            };
            let outline_only = matches!(
                assign,
                Some(SendAssign::Tool1Outline | SendAssign::Tool2Outline)
            );
            let contours = split_contours(&shape.path);
            let depths = if outline_only {
                nesting_depths(&contours.iter().map(|(_, c)| c.clone()).collect::<Vec<_>>())
            } else {
                vec![0; contours.len()]
            };
            for ((contour, path), depth) in contours.into_iter().zip(depths) {
                if shape.production.hidden_contours.contains(&contour) || depth > 0 {
                    continue;
                }
                ops.push(CutOp {
                    shape_id: shape.id,
                    copy: placement.copy,
                    contour,
                    group: group.clone(),
                    operation: shape.operation(),
                    tool,
                    passes: setting.and_then(|g| g.passes).unwrap_or(options.passes),
                    closed: is_closed(&path),
                    path: map * path,
                });
            }
        }
    }
    ops
}

fn start_point(path: &BezPath) -> Point {
    path.elements()
        .first()
        .and_then(|el| el.end_point())
        .unwrap_or_default()
}

fn end_point(op: &CutOp) -> Point {
    if op.closed {
        return start_point(&op.path);
    }
    op.path
        .elements()
        .iter()
        .rev()
        .find_map(|el| el.end_point())
        .unwrap_or_default()
}

fn order_ops(
    ops: &mut Vec<CutOp>,
    doc: &Document,
    layers: &BTreeMap<u64, usize>,
    options: &JobOptions,
) {
    let depths = if options.inside_first {
        let paths: Vec<BezPath> = ops.iter().map(|o| o.path.clone()).collect();
        nesting_depths(&paths)
    } else {
        vec![0; ops.len()]
    };
    let mut colors: Vec<String> = Vec::new();
    let rank = |op: &CutOp, colors: &mut Vec<String>| -> usize {
        match options.order {
            CutOrder::ByLayer => layers.get(&op.shape_id).copied().unwrap_or_default(),
            CutOrder::ByColor => {
                let color = doc
                    .get_shape(op.shape_id)
                    .and_then(|s| {
                        s.production
                            .line_color
                            .clone()
                            .or_else(|| s.production.fill_color.clone())
                    })
                    .unwrap_or_default();
                colors.iter().position(|c| *c == color).unwrap_or_else(|| {
                    colors.push(color);
                    colors.len() - 1
                })
            }
            _ => 0,
        }
    };
    let mut keyed: Vec<((usize, std::cmp::Reverse<usize>), CutOp)> = ops
        .drain(..)
        .zip(depths)
        .map(|(op, depth)| ((rank(&op, &mut colors), std::cmp::Reverse(depth)), op))
        .collect();
    if options.order == CutOrder::MinimizeRoller {
        let far = |op: &CutOp| {
            stencilcraft_geom::bounds_of_shape(&op.path).map_or(0.0, |b| b.y1)
        };
        keyed.sort_by(|(ka, a), (kb, b)| {
            ka.0.cmp(&kb.0)
                .then(far(a).total_cmp(&far(b)))
                .then(ka.1.cmp(&kb.1))
        });
        ops.extend(keyed.into_iter().map(|(_, op)| op));
        return;
    }
    keyed.sort_by_key(|(k, _)| *k);
    if options.order != CutOrder::ShortestTravel {
        ops.extend(keyed.into_iter().map(|(_, op)| op));
        return;
    }
    // Greedy nearest neighbour inside each bucket of equal rank and depth.
    let mut at = Point::ZERO;
    let mut rest = keyed;
    while !rest.is_empty() {
        let key = rest.first().map(|(k, _)| *k);
        let bucket_end = rest.iter().take_while(|(k, _)| Some(*k) == key).count();
        let mut bucket: Vec<CutOp> = rest.drain(..bucket_end).map(|(_, op)| op).collect();
        while !bucket.is_empty() {
            let nearest = bucket
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    (start_point(&a.path) - at)
                        .hypot2()
                        .total_cmp(&(start_point(&b.path) - at).hypot2())
                })
                .map(|(i, _)| i)
                .unwrap_or_default();
            let op = bucket.swap_remove(nearest);
            at = end_point(&op);
            ops.push(op);
        }
    }
}

/// Reorders the dots (contours under 0.5 mm across) by distance from their common centre,
/// keeping them where the first dot was in the cutting order.
fn order_stipple(ops: &mut Vec<CutOp>, order: StippleOrder) {
    if order == StippleOrder::Default {
        return;
    }
    let is_dot = |op: &CutOp| {
        stencilcraft_geom::bounds_of_shape(&op.path)
            .is_some_and(|b| b.width() < 0.5 && b.height() < 0.5)
    };
    let Some(first) = ops.iter().position(is_dot) else {
        return;
    };
    let (mut dots, rest): (Vec<CutOp>, Vec<CutOp>) = ops.drain(..).partition(is_dot);
    let n = dots.len() as f64;
    let centre = dots
        .iter()
        .map(|d| start_point(&d.path).to_vec2())
        .fold(kurbo::Vec2::ZERO, |a, b| a + b)
        / n;
    let dist = |d: &CutOp| (start_point(&d.path).to_vec2() - centre).hypot2();
    dots.sort_by(|a, b| match order {
        StippleOrder::EdgeToCenter => dist(b).total_cmp(&dist(a)),
        _ => dist(a).total_cmp(&dist(b)),
    });
    let mut rest = rest.into_iter();
    ops.extend(rest.by_ref().take(first));
    ops.extend(dots);
    ops.extend(rest);
}

/// A closed contour that runs on past its start by `length` mm, so the cut meets cleanly.
pub(crate) fn with_overcut(path: &BezPath, length: f64) -> BezPath {
    let segs: Vec<PathSeg> = path.segments().collect();
    let Some(first) = segs.first() else {
        return path.clone();
    };
    let mut out = BezPath::new();
    out.move_to(first.start());
    let push = |out: &mut BezPath, seg: PathSeg| match seg {
        PathSeg::Line(l) => out.line_to(l.p1),
        PathSeg::Quad(q) => out.quad_to(q.p1, q.p2),
        PathSeg::Cubic(c) => out.curve_to(c.p1, c.p2, c.p3),
    };
    for seg in &segs {
        push(&mut out, *seg);
    }
    let mut remaining = length;
    // Contours shorter than the overcut wrap at most once.
    for seg in &segs {
        let len = seg.arclen(1e-4);
        if len >= remaining {
            let t = seg.inv_arclen(remaining, 1e-4);
            push(&mut out, seg.subsegment(0.0..t));
            break;
        }
        push(&mut out, *seg);
        remaining -= len;
    }
    out
}

fn placements_box(mat: &MatPlan) -> Option<Rect> {
    mat.placements
        .iter()
        .map(|p| Rect::new(p.bounds[0], p.bounds[1], p.bounds[2], p.bounds[3]))
        .reduce(|a, b| a.union(b))
}

fn line_op(path: BezPath, closed: bool, passes: u32) -> CutOp {
    CutOp {
        shape_id: 0,
        copy: 0,
        contour: 0,
        group: "weed".to_string(),
        operation: Operation::Cut,
        tool: 1,
        passes,
        closed,
        path,
    }
}

/// Gap midpoints along one axis: where no piece covers the coordinate.
fn gaps(mut spans: Vec<(f64, f64)>) -> Vec<f64> {
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut reach: Option<f64> = None;
    for (a, b) in spans {
        if let Some(r) = reach
            && a > r + 1e-6
        {
            out.push((r + a) / 2.0);
        }
        reach = Some(reach.map_or(b, |r| r.max(b)));
    }
    out
}

fn weed_ops(mat: &MatPlan, weed: &Weed, passes: u32) -> Vec<CutOp> {
    let Some(content) = placements_box(mat) else {
        return Vec::new();
    };
    let area = Rect::new(mat.area[0], mat.area[1], mat.area[2], mat.area[3]);
    let b = content.inflate(weed.margin_mm, weed.margin_mm).intersect(area);
    let mut ops = Vec::new();
    if weed.lines {
        let xs = gaps(mat.placements.iter().map(|p| (p.bounds[0], p.bounds[2])).collect());
        let ys = gaps(mat.placements.iter().map(|p| (p.bounds[1], p.bounds[3])).collect());
        for x in xs {
            let mut p = BezPath::new();
            p.move_to((x, b.y0));
            p.line_to((x, b.y1));
            ops.push(line_op(p, false, passes));
        }
        for y in ys {
            let mut p = BezPath::new();
            p.move_to((b.x0, y));
            p.line_to((b.x1, y));
            ops.push(line_op(p, false, passes));
        }
    }
    let mut boxed = BezPath::new();
    boxed.move_to((b.x0, b.y0));
    boxed.line_to((b.x1, b.y0));
    boxed.line_to((b.x1, b.y1));
    boxed.line_to((b.x0, b.y1));
    boxed.close_path();
    ops.push(line_op(boxed, true, passes));
    ops
}

fn test_cut_ops(mat: &MatPlan, t: &TestCut, passes: u32) -> Result<Vec<CutOp>> {
    let s = t.size_mm;
    let r = Rect::new(t.x_mm, t.y_mm, t.x_mm + s, t.y_mm + s);
    let area = Rect::new(mat.area[0], mat.area[1], mat.area[2], mat.area[3]);
    if r.x0 < area.x0 || r.y0 < area.y0 || r.x1 > area.x1 || r.y1 > area.y1 {
        return invalid(format!(
            "test cut at ({}, {}) size {s} is outside mat {}'s cut area",
            t.x_mm, t.y_mm, mat.index
        ));
    }
    if let Some(p) = mat.placements.iter().find(|p| {
        r.x0 < p.bounds[2] && p.bounds[0] < r.x1 && r.y0 < p.bounds[3] && p.bounds[1] < r.y1
    }) {
        return invalid(format!(
            "test cut overlaps shape {} on mat {}; move it",
            p.shape_ids.first().copied().unwrap_or_default(),
            mat.index
        ));
    }
    let mut tri = BezPath::new();
    tri.move_to((r.x0 + s * 0.5, r.y0 + s * 0.25));
    tri.line_to((r.x0 + s * 0.75, r.y0 + s * 0.7));
    tri.line_to((r.x0 + s * 0.25, r.y0 + s * 0.7));
    tri.close_path();
    let mut square = BezPath::new();
    square.move_to((r.x0, r.y0));
    square.line_to((r.x1, r.y0));
    square.line_to((r.x1, r.y1));
    square.line_to((r.x0, r.y1));
    square.close_path();
    Ok([tri, square]
        .into_iter()
        .enumerate()
        .map(|(i, path)| CutOp {
            contour: i,
            group: "test-cut".to_string(),
            ..line_op(path, true, passes)
        })
        .collect())
}

fn origin_map(origin: Origin, feed: Feed, w: f64, l: f64) -> Affine {
    let flip = match origin {
        Origin::TopLeft => Affine::IDENTITY,
        Origin::TopRight => Affine::new([-1.0, 0.0, 0.0, 1.0, w, 0.0]),
        Origin::BottomLeft => Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, l]),
        Origin::BottomRight => Affine::new([-1.0, 0.0, 0.0, -1.0, w, l]),
    };
    match feed {
        Feed::Length => flip,
        Feed::Width => Affine::new([0.0, 1.0, 1.0, 0.0, 0.0, 0.0]) * flip,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rect;
    use stencilcraft_doc::CutStyle;

    fn ring_doc() -> Document {
        let mut doc = Document::default();
        // A letter "O": outline first in the path, counter second.
        let mut o = rect(0.0, 0.0, 30.0, 30.0);
        o.extend(rect(10.0, 10.0, 10.0, 10.0).iter());
        doc.push_shape(0, o, CutStyle::Cut).unwrap();
        doc.push_shape(0, rect(100.0, 0.0, 5.0, 5.0), CutStyle::Engrave).unwrap();
        doc
    }

    #[test]
    fn inner_contours_cut_first() {
        let doc = ring_doc();
        let job = build_job(&doc, &JobOptions::default()).unwrap();
        let ops = &job.mats[0].ops;
        let first_o = ops.iter().position(|o| o.shape_id == 1).unwrap();
        assert_eq!(ops[first_o].contour, 1, "the counter comes before the outline");
        let outline = ops.iter().position(|o| o.shape_id == 1 && o.contour == 0).unwrap();
        assert!(first_o < outline);
        let plain = build_job(
            &doc,
            &JobOptions {
                inside_first: false,
                order: CutOrder::Document,
                ..JobOptions::default()
            },
        )
        .unwrap();
        assert_eq!(plain.mats[0].ops[0].contour, 0);
    }

    #[test]
    fn groups_pick_tools_and_switch_off() {
        let doc = ring_doc();
        let options = JobOptions {
            groups: vec![
                GroupSetting {
                    key: "engrave".to_string(),
                    tool: 2,
                    enabled: true,
                    passes: Some(3),
                    overcut_mm: None,
                },
            ],
            ..JobOptions::default()
        };
        let job = build_job(&doc, &options).unwrap();
        let engrave = job.mats[0].ops.iter().find(|o| o.shape_id == 2).unwrap();
        assert_eq!((engrave.tool, engrave.passes), (2, 3));
        let off = JobOptions {
            groups: vec![GroupSetting {
                key: "engrave".to_string(),
                tool: 1,
                enabled: false,
                passes: None,
                overcut_mm: None,
            }],
            ..JobOptions::default()
        };
        let job = build_job(&doc, &off).unwrap();
        assert!(job.mats[0].ops.iter().all(|o| o.shape_id != 2));
        let bad = JobOptions {
            groups: vec![GroupSetting {
                key: "x".into(),
                tool: 3,
                enabled: true,
                passes: None,
                overcut_mm: None,
            }],
            ..JobOptions::default()
        };
        assert!(build_job(&doc, &bad).is_err());
    }

    #[test]
    fn send_assignment_and_stipple() {
        let mut doc = ring_doc();
        doc.layers[0].shapes[0].production.send = Some(SendAssign::Tool2Outline);
        doc.layers[0].shapes[1].production.send = Some(SendAssign::Ignore);
        let job = build_job(&doc, &JobOptions::default()).unwrap();
        let ops = &job.mats[0].ops;
        assert_eq!(ops.len(), 1, "outline only, the ignored shape left out");
        assert_eq!((ops[0].tool, ops[0].contour), (2, 0));
        assert_eq!(job.after, AfterJob::ReturnToOrigin);

        let mut dots = Document::default();
        for x in [0.0, 10.0, 20.0] {
            dots.push_shape(0, rect(x, 0.0, 0.2, 0.2), CutStyle::Cut).unwrap();
        }
        dots.layers[0].shapes.iter_mut().for_each(|s| s.production.attach = Some(1));
        let job = build_job(
            &dots,
            &JobOptions {
                stipple_order: StippleOrder::CenterToEdge,
                ..JobOptions::default()
            },
        )
        .unwrap();
        assert_eq!(job.mats[0].ops[0].shape_id, 2, "the middle dot first");
    }

    #[test]
    fn overcut_extends_closed_contours() {
        let p = with_overcut(&rect(0.0, 0.0, 10.0, 10.0), 1.5);
        let len: f64 = p.segments().map(|s| s.arclen(1e-6)).sum();
        assert!((len - 41.5).abs() < 1e-6, "{len}");
    }

    #[test]
    fn shortest_travel_and_by_layer() {
        let mut doc = Document::default();
        for x in [200.0, 0.0, 100.0] {
            doc.push_shape(0, rect(x, 0.0, 5.0, 5.0), CutStyle::Cut).unwrap();
        }
        let options = JobOptions {
            order: CutOrder::ShortestTravel,
            mats: MatOptions {
                nest: false,
                ..MatOptions::default()
            },
            ..JobOptions::default()
        };
        let job = build_job(&doc, &options).unwrap();
        // All three sit in one column; travel goes top to bottom.
        let ys: Vec<f64> = job.mats[0]
            .ops
            .iter()
            .map(|o| start_point(&o.path).y)
            .collect();
        assert!(ys.windows(2).all(|w| w[0] <= w[1]), "{ys:?}");
        let by_layer = JobOptions {
            order: CutOrder::ByLayer,
            ..JobOptions::default()
        };
        assert_eq!(build_job(&doc, &by_layer).unwrap().mats[0].ops.len(), 3);
    }

    #[test]
    fn test_cut_weed_and_origin() {
        let doc = ring_doc();
        let options = JobOptions {
            test_cut: Some(TestCut {
                mat: 0,
                x_mm: 200.0,
                y_mm: 200.0,
                size_mm: 10.0,
            }),
            weed: Some(Weed {
                margin_mm: 2.0,
                lines: true,
            }),
            ..JobOptions::default()
        };
        let job = build_job(&doc, &options).unwrap();
        assert_eq!(job.mats[0].test_cut.len(), 2);
        let weed: Vec<&CutOp> = job.mats[0].ops.iter().filter(|o| o.group == "weed").collect();
        assert!(weed.iter().any(|o| o.closed), "weed box");
        assert!(weed.iter().any(|o| !o.closed), "a weed line between the two pieces");
        let overlapping = JobOptions {
            test_cut: Some(TestCut {
                mat: 0,
                x_mm: 0.0,
                y_mm: 0.0,
                size_mm: 10.0,
            }),
            ..JobOptions::default()
        };
        assert!(build_job(&doc, &overlapping).is_err());
        let flipped = JobOptions {
            origin: Origin::BottomRight,
            ..JobOptions::default()
        };
        let a = build_job(&doc, &JobOptions::default()).unwrap();
        let b = build_job(&doc, &flipped).unwrap();
        let (w, l) = (a.mats[0].width_mm, a.mats[0].length_mm);
        let pa = start_point(&a.mats[0].ops[0].path);
        let pb = start_point(&b.mats[0].ops[0].path);
        assert!((pb.x - (w - pa.x)).abs() < 1e-9 && (pb.y - (l - pa.y)).abs() < 1e-9);
    }
}
