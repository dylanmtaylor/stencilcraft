//! `CutJob`: the vendor-neutral description of what to cut, and its
//! validation against a machine's `Capabilities`. Every protocol encodes
//! from a `CutJob`; nothing reaches a protocol without passing `validate`.

use crate::{Capabilities, MachineError, Mat, ProtocolId, ToolKind};
use stencilcraft_geom as geom;
use vectorcraft_geom::{FillRule, PathData};

pub(crate) use craft_plotter::bbox_rect;
#[cfg(test)]
pub(crate) use craft_plotter::overcut_path;
/// Hard caps on job size from untrusted documents, and the job types the
/// protocol encoders share.
pub use craft_plotter::{
    BBoxMm, MAX_OPERATIONS, MAX_OVERCUT_MM, MAX_PASSES, MAX_POINTS, Registration,
    ToolSettings as OpSettings, find_bbox,
};

/// Largest closed-contour (software) overcut past the start point, mm.
pub const MAX_CLOSED_OVERCUT_MM: f64 = 5.0;
/// Cut-order sorting is quadratic in the path count; above this it falls back
/// to a plain feed-axis sort (no nesting check).
pub const MAX_SORT_PATHS: usize = 5_000;
/// `FC` (blade offset) tops out around 46 SU on the device (community note).
pub const MAX_BLADE_DIAMETER_MM: f64 = 2.3;

/// What an operation does to the material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OpKind {
    #[default]
    Cut,
    /// Pen/sketch: no blade offset, corners not sharpened.
    Draw,
    Score,
    Perforate,
    Emboss,
    Engrave,
    Foil,
}

/// One ordered step of a job: geometry plus how to cut it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Operation {
    pub kind: OpKind,
    /// The loaded tool; `None` = whatever is in the holder.
    pub tool: Option<ToolKind>,
    /// Tool holder, 1-based.
    pub holder: u8,
    /// Polylines in mm, `(x right, y down)`, origin at the media's top-left.
    pub paths: Vec<Vec<(f64, f64)>>,
    pub settings: OpSettings,
    /// Source document layer ("" = not split by layer).
    pub layer: String,
    /// The layer's line colour, `#rrggbb`.
    pub color: Option<String>,
}

impl Operation {
    pub fn is_pen(&self) -> bool {
        self.kind == OpKind::Draw || self.tool == Some(ToolKind::Pen)
    }

    /// The operation as the protocol encoders take it. Blade depth is only
    /// ever sent for the AutoBlade.
    pub(crate) fn neutral(&self) -> craft_plotter::Operation {
        let depth = (self.tool == Some(ToolKind::AutoBlade))
            .then_some(self.settings.depth)
            .flatten();
        craft_plotter::Operation {
            pen: self.is_pen(),
            holder: self.holder,
            paths: self.paths.clone(),
            settings: OpSettings {
                depth,
                ..self.settings.clone()
            },
        }
    }
}

/// Cut order within each operation (the reference's Send preferences:
/// No Sort | Maximize Speed | Minimize Roller Movement).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CutOrder {
    /// Document order.
    None,
    /// Nearest-neighbour: next path is the one starting closest to the head.
    Speed,
    /// Sort by the top (feed-axis) edge so the media moves one way. The
    /// reference's default.
    #[default]
    Roller,
}

impl CutOrder {
    pub fn parse(s: &str) -> Option<CutOrder> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" | "no-sort" => Some(CutOrder::None),
            "speed" | "maximize-speed" => Some(CutOrder::Speed),
            "roller" | "minimize-roller-movement" => Some(CutOrder::Roller),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            CutOrder::None => "none",
            CutOrder::Speed => "speed",
            CutOrder::Roller => "roller",
        }
    }
}

/// A complete, vendor-neutral cut job.
#[derive(Debug, Clone, PartialEq)]
pub struct CutJob {
    pub operations: Vec<Operation>,
    /// Offset of the job origin from the media's top-left corner, mm.
    pub origin_down_mm: f64,
    pub origin_right_mm: f64,
    /// Extra feed after the last cut (Silhouette park position), mm.
    pub end_feed_mm: f64,
    /// Landscape media feed (Silhouette `TB50,1`).
    pub landscape: bool,
    /// Loaded media (width across, length along the feed), mm.
    pub media_size_mm: Option<(f64, f64)>,
    pub mat: Mat,
    pub registration: Option<Registration>,
    /// Roller track enhancing (Silhouette `FY0`).
    pub track_enhancing: bool,
    /// Media preset id for the Silhouette legacy `FW` command.
    pub media_id: Option<u16>,
    /// Cut only the bounding rectangle (placement preview).
    pub bbox_only: bool,
    /// Path order within each operation.
    pub order: CutOrder,
    /// Cut contours inside a closed path before that path, so parts don't
    /// shift (reference default: on).
    pub interior_first: bool,
    /// Extend each closed contour past its start point by this much, mm
    /// (software overcut; 0 = off). Not applied to pen operations.
    pub closed_overcut_mm: f64,
    /// Flip horizontally about the media centre (heat-transfer vinyl is cut
    /// face down). Applied at job build time, never to the document.
    pub mirror: bool,
    /// Unite overlapping closed contours of each operation before cutting so
    /// overlaps aren't cut twice (weld on cut). The document is untouched.
    pub weld: bool,
}

impl Default for CutJob {
    fn default() -> Self {
        CutJob {
            operations: Vec::new(),
            origin_down_mm: 0.0,
            origin_right_mm: 0.0,
            end_feed_mm: 0.0,
            landscape: false,
            media_size_mm: None,
            mat: Mat::None,
            registration: None,
            track_enhancing: false,
            media_id: None,
            bbox_only: false,
            order: CutOrder::default(),
            interior_first: true,
            closed_overcut_mm: 0.0,
            mirror: false,
            weld: false,
        }
    }
}

/// Send settings for one document layer: the reference's Send › Cut by Line
/// Color row (on/off, tool holder, passes, force, speed). `None` keeps the
/// job's value; overrides are checked against the machine by `validate`.
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LayerSettings {
    /// Layer name, or `#rrggbb` to match the layer's line colour.
    pub layer: String,
    pub skip: bool,
    #[serde(alias = "tool")]
    pub holder: Option<u8>,
    pub passes: Option<u8>,
    pub force: Option<u32>,
    pub speed: Option<u32>,
}

impl LayerSettings {
    /// Parse `NAME:skip` or `NAME:tool=2,passes=2,force=20,speed=5`
    /// (`holder=` is an alias of `tool=`).
    pub fn parse(spec: &str) -> Result<LayerSettings, String> {
        let (name, rest) = spec
            .rsplit_once(':')
            .ok_or_else(|| format!("layer {spec}: expected NAME:SETTINGS"))?;
        let mut ls = LayerSettings {
            layer: name.trim().to_string(),
            ..LayerSettings::default()
        };
        for item in rest.split(',').map(str::trim) {
            let bad = || format!("layer {spec}: bad setting `{item}`");
            if item.eq_ignore_ascii_case("skip") || item.eq_ignore_ascii_case("off") {
                ls.skip = true;
                continue;
            }
            let (key, v) = item.split_once('=').ok_or_else(bad)?;
            let v = v.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "tool" | "holder" => ls.holder = Some(v.parse().map_err(|_| bad())?),
                "passes" => ls.passes = Some(v.parse().map_err(|_| bad())?),
                "force" => ls.force = Some(v.parse().map_err(|_| bad())?),
                "speed" => ls.speed = Some(v.parse().map_err(|_| bad())?),
                _ => return Err(bad()),
            }
        }
        Ok(ls)
    }

    fn matches(&self, op: &Operation) -> bool {
        op.layer == self.layer
            || (self.layer.starts_with('#')
                && op
                    .color
                    .as_deref()
                    .is_some_and(|c| c.eq_ignore_ascii_case(&self.layer)))
    }
}

/// One document layer's geometry for [`CutJob::split_layers`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayerPaths {
    pub name: String,
    pub color: Option<String>,
    pub paths: Paths,
}

impl CutJob {
    /// Cut by layer: replace the job's single operation with one per layer
    /// (same tool settings), apply `settings` (skip, holder, passes, force,
    /// speed), then merge neighbouring operations whose settings ended up
    /// equal, so a job without overrides stays one operation. A setting that
    /// names no layer is refused; out-of-range values are left to `validate`.
    pub fn split_layers(
        &mut self,
        layers: Vec<LayerPaths>,
        settings: &[LayerSettings],
    ) -> Result<(), MachineError> {
        let Some(template) = self.operations.first().cloned() else {
            return Ok(());
        };
        let ops: Vec<Operation> = layers
            .into_iter()
            .map(|l| Operation {
                layer: l.name,
                color: l.color,
                paths: l.paths,
                ..template.clone()
            })
            .collect();
        if let Some(ls) = settings
            .iter()
            .find(|ls| !ops.iter().any(|o| ls.matches(o)))
        {
            let names: Vec<&str> = ops.iter().map(|o| o.layer.as_str()).collect();
            return Err(MachineError::Limit(format!(
                "layer `{}` is not in the document (layers: {})",
                ls.layer,
                names.join(", ")
            )));
        }
        let mut out: Vec<Operation> = Vec::new();
        'ops: for mut op in ops {
            let matching: Vec<&LayerSettings> =
                settings.iter().filter(|ls| ls.matches(&op)).collect();
            for ls in matching {
                if ls.skip {
                    continue 'ops;
                }
                op.holder = ls.holder.unwrap_or(op.holder);
                op.settings.passes = ls.passes.unwrap_or(op.settings.passes);
                op.settings.force = ls.force.or(op.settings.force);
                op.settings.speed = ls.speed.or(op.settings.speed);
            }
            match out.last_mut() {
                Some(prev)
                    if prev.kind == op.kind
                        && prev.tool == op.tool
                        && prev.holder == op.holder
                        && prev.settings == op.settings =>
                {
                    prev.paths.append(&mut op.paths);
                }
                _ => out.push(op),
            }
        }
        self.operations = out;
        Ok(())
    }
}

/// A list of polylines in mm, `(x right, y down)`.
pub type Paths = Vec<Vec<(f64, f64)>>;

impl CutJob {
    /// The job as the protocol encoders take it: geometry and tool settings,
    /// without what only this crate's job layer uses (layers, order, weld).
    pub(crate) fn neutral(&self) -> craft_plotter::Job {
        craft_plotter::Job {
            operations: self.operations.iter().map(Operation::neutral).collect(),
            origin_down_mm: self.origin_down_mm,
            origin_right_mm: self.origin_right_mm,
            end_feed_mm: self.end_feed_mm,
            bbox_only: self.bbox_only,
            registration: self.registration,
        }
    }

    /// Bounding box over every operation's paths.
    pub fn bbox(&self) -> Option<BBoxMm> {
        let all: Vec<Vec<(f64, f64)>> = self
            .operations
            .iter()
            .flat_map(|o| o.paths.iter().cloned())
            .collect();
        find_bbox(&all)
    }
}

fn is_closed(path: &[(f64, f64)]) -> bool {
    path.len() > 2 && path.first() == path.last()
}

/// Even-odd point-in-polygon test.
fn point_in(p: (f64, f64), poly: &[(f64, f64)]) -> bool {
    let mut inside = false;
    for w in poly.windows(2) {
        let [a, b] = w else { continue };
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) {
            inside = !inside;
        }
    }
    inside
}

/// `outer` is closed and encloses `inner` (strict bbox containment plus the
/// first point inside).
fn encloses(outer: &[(f64, f64)], ob: BBoxMm, inner: &[(f64, f64)], ib: BBoxMm) -> bool {
    is_closed(outer)
        && ib.0 > ob.0
        && ib.1 > ob.1
        && ib.2 < ob.2
        && ib.3 < ob.3
        && inner.first().is_some_and(|&p| point_in(p, outer))
}

/// Order one operation's paths. Greedy: with `interior_first` a path is
/// ready once every path it encloses is cut; among ready paths `order` picks
/// document order, the nearest start to the head, or the top edge.
pub fn order_paths(paths: Paths, order: CutOrder, interior_first: bool) -> Paths {
    let n = paths.len();
    if n < 2 || (order == CutOrder::None && !interior_first) {
        return paths;
    }
    let bbs: Vec<BBoxMm> = paths
        .iter()
        .map(|p| find_bbox(core::slice::from_ref(p)).unwrap_or((0.0, 0.0, 0.0, 0.0)))
        .collect();
    let top = |i: usize| bbs.get(i).map_or((0.0, 0.0), |b| (b.0, b.1));
    let by_top = |a: &usize, b: &usize| {
        let (ta, tb) = (top(*a), top(*b));
        ta.0.total_cmp(&tb.0)
            .then(ta.1.total_cmp(&tb.1))
            .then(a.cmp(b))
    };
    let mut idx: Vec<usize> = (0..n).collect();
    if n > MAX_SORT_PATHS {
        // shortcut: no nesting check or nearest-neighbour above MAX_SORT_PATHS; upgrade with a spatial index.
        if order != CutOrder::None {
            idx.sort_by(by_top);
        }
        return pick(paths, &idx);
    }
    // containers[i] = closed paths enclosing i; blockers[j] = uncut paths inside j.
    let mut containers: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut blockers = vec![0usize; n];
    if interior_first {
        for (i, (inner, ib)) in paths.iter().zip(&bbs).enumerate() {
            for (j, (outer, ob)) in paths.iter().zip(&bbs).enumerate() {
                if i != j && encloses(outer, *ob, inner, *ib) {
                    if let Some(c) = containers.get_mut(i) {
                        c.push(j);
                    }
                    if let Some(b) = blockers.get_mut(j) {
                        *b += 1;
                    }
                }
            }
        }
    }
    let mut done = vec![false; n];
    let mut head = (0.0, 0.0);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let ready = |i: &usize| !done.get(*i).copied().unwrap_or(true);
        let dist = |i: usize| {
            paths
                .get(i)
                .and_then(|p| p.first())
                .map_or(f64::INFINITY, |&(x, y)| (x - head.0).hypot(y - head.1))
        };
        let candidates = idx
            .iter()
            .copied()
            .filter(|i| ready(i) && blockers.get(*i).copied().unwrap_or(0) == 0);
        let next = match order {
            CutOrder::None => candidates.min(),
            CutOrder::Roller => candidates.min_by(by_top),
            CutOrder::Speed => {
                candidates.min_by(|a, b| dist(*a).total_cmp(&dist(*b)).then(a.cmp(b)))
            }
        };
        // Strict bbox containment rules out cycles; fall back anyway.
        let Some(k) = next.or_else(|| idx.iter().copied().find(ready)) else {
            break;
        };
        if let Some(d) = done.get_mut(k) {
            *d = true;
        }
        for &c in containers.get(k).map_or(&[][..], Vec::as_slice) {
            if let Some(b) = blockers.get_mut(c) {
                *b = b.saturating_sub(1);
            }
        }
        if let Some(&p) = paths.get(k).and_then(|p| p.last()) {
            head = p;
        }
        out.push(k);
    }
    pick(paths, &out)
}

fn pick(paths: Paths, order: &[usize]) -> Paths {
    let mut slots: Vec<Option<Vec<(f64, f64)>>> = paths.into_iter().map(Some).collect();
    order
        .iter()
        .filter_map(|&i| slots.get_mut(i).and_then(Option::take))
        .collect()
}

/// Continue a closed contour past its start for `d` mm along its first
/// segments (at most one lap). Open paths are returned unchanged.
pub fn extend_closed(path: &[(f64, f64)], d: f64) -> Vec<(f64, f64)> {
    let mut out = path.to_vec();
    if !is_closed(path) || d <= 0.0 {
        return out;
    }
    let mut left = d;
    for w in path.windows(2) {
        let [a, b] = w else { continue };
        let l = (b.0 - a.0).hypot(b.1 - a.1);
        if l >= left {
            let t = left / l;
            out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
            break;
        }
        out.push(*b);
        left -= l;
    }
    out
}

/// Weld on cut: unite overlapping closed contours so shared regions are cut
/// once. Contours are grouped into shapes by nesting (a contour inside an odd
/// number of others is a hole of its innermost enclosing contour), each shape
/// is filled even-odd and the shapes are united with the Pathfinder/Weld
/// boolean engine. Open paths pass through; a union the engine refuses keeps
/// the paths as they are.
pub fn weld_paths(paths: Paths) -> Paths {
    // shortcut: the nesting pass is quadratic, so above MAX_SORT_PATHS weld is skipped; upgrade with a spatial index.
    if paths.len() < 2 || paths.len() > MAX_SORT_PATHS {
        return paths;
    }
    let (closed, open): (Paths, Paths) = paths.iter().cloned().partition(|p| is_closed(p));
    if closed.len() < 2 {
        return paths;
    }
    let bbs: Vec<BBoxMm> = closed
        .iter()
        .map(|p| find_bbox(core::slice::from_ref(p)).unwrap_or((0.0, 0.0, 0.0, 0.0)))
        .collect();
    let parents: Vec<Vec<usize>> = (0..closed.len())
        .map(|i| {
            (0..closed.len())
                .filter(|&j| {
                    i != j
                        && match (closed.get(j), bbs.get(j), closed.get(i), bbs.get(i)) {
                            (Some(o), Some(ob), Some(p), Some(pb)) => encloses(o, *ob, p, *pb),
                            _ => false,
                        }
                })
                .collect()
        })
        .collect();
    let depth = |i: usize| parents.get(i).map_or(0, Vec::len);
    // Each even-depth contour starts a shape; odd-depth contours join the
    // innermost enclosing one.
    let mut shapes: Vec<geom::BezPath> = vec![geom::BezPath::new(); closed.len()];
    for (i, p) in closed.iter().enumerate() {
        let owner = if depth(i) % 2 == 0 {
            i
        } else {
            parents
                .get(i)
                .and_then(|ps| ps.iter().copied().max_by_key(|&j| depth(j)))
                .unwrap_or(i)
        };
        let Some(bp) = shapes.get_mut(owner) else {
            continue;
        };
        for (k, &(x, y)) in p.iter().enumerate() {
            let pt = geom::Point::new(x, y);
            if k == 0 {
                bp.move_to(pt);
            } else {
                bp.line_to(pt);
            }
        }
        bp.close_path();
    }
    let data: Vec<PathData> = shapes
        .iter()
        .filter(|b| !b.elements().is_empty())
        .map(PathData::from_bezpath)
        .collect();
    let inputs: Vec<(&PathData, FillRule)> = data.iter().map(|d| (d, FillRule::EvenOdd)).collect();
    match vectorcraft_pathops::try_unite_all(&inputs) {
        Ok(united) if !united.subpaths.is_empty() => {
            let polylines =
                geom::path_to_polylines(&united.to_bezpath(), geom::DEFAULT_FLATTEN_STEPS);
            let mut out = crate::paths_from_polylines(&polylines);
            for p in &mut out {
                p.dedup();
            }
            out.extend(open);
            out
        }
        _ => paths,
    }
}

/// Width the mirror flips about: loaded media, then mat, then machine.
fn mirror_width(caps: &Capabilities, job: &CutJob) -> Option<f64> {
    job.media_size_mm
        .map(|(w, _)| w)
        .or_else(|| (job.mat != Mat::None).then(|| job.mat.extent_mm().1))
        .or(caps.cut_width_mm.or(caps.media_width_mm))
}

/// Apply the job-level settings (weld, cut order, closed-contour overcut, mirror)
/// to every operation's paths. Validate the result again: a mirrored design
/// must still fit.
pub fn apply_job_settings(caps: &Capabilities, job: &CutJob) -> Result<CutJob, MachineError> {
    let flip = if job.mirror {
        let Some(w) = mirror_width(caps, job) else {
            return Err(MachineError::Limit(format!(
                "{}: mirror needs a known media width: pass the loaded media size",
                caps.id
            )));
        };
        // About the media centre, in job coordinates (origin is offset from the media's left).
        Some(w - 2.0 * job.origin_right_mm)
    } else {
        None
    };
    let mut out = job.clone();
    for op in &mut out.operations {
        let pen = op.is_pen();
        let mut paths = core::mem::take(&mut op.paths);
        if job.weld {
            paths = weld_paths(paths);
        }
        let mut paths = order_paths(paths, job.order, job.interior_first);
        for p in &mut paths {
            if !pen && job.closed_overcut_mm > 0.0 {
                *p = extend_closed(p, job.closed_overcut_mm);
            }
            if let Some(w) = flip {
                for pt in p.iter_mut() {
                    pt.0 = w - pt.0;
                }
            }
        }
        op.paths = paths;
    }
    Ok(out)
}

/// Check a job against a machine. Each message names the knob and the
/// allowed range so CLI/MCP users can fix the request.
pub fn validate(caps: &Capabilities, job: &CutJob) -> Result<(), MachineError> {
    let limit = |m: String| Err(MachineError::Limit(format!("{}: {m}", caps.id)));
    if !caps.encodable() {
        return limit(match caps.protocol {
            ProtocolId::Unsupported => format!(
                "this machine's protocol is not implemented ({}). Export SVG/DXF instead.",
                caps.notes
            ),
            _ => format!(
                "no documented machine protocol ({}). Export SVG/DXF instead.",
                caps.notes
            ),
        });
    }
    if job.operations.is_empty() {
        return limit("job has no operations".to_string());
    }
    if job.operations.len() > MAX_OPERATIONS {
        return limit(format!("more than {MAX_OPERATIONS} operations"));
    }
    for v in [job.origin_down_mm, job.origin_right_mm, job.end_feed_mm] {
        if !v.is_finite() {
            return limit("non-finite origin offset".to_string());
        }
    }
    if job.origin_down_mm < 0.0 || job.origin_right_mm < 0.0 {
        return limit("negative origin offset".to_string());
    }
    if !job.closed_overcut_mm.is_finite()
        || !(0.0..=MAX_CLOSED_OVERCUT_MM).contains(&job.closed_overcut_mm)
    {
        return limit(format!(
            "closed-contour overcut {} mm outside 0..={MAX_CLOSED_OVERCUT_MM}",
            job.closed_overcut_mm
        ));
    }
    if job.mirror && job.registration.is_some() {
        return limit("mirror can't be combined with registration marks".to_string());
    }
    let holders = caps.tool_holders.unwrap_or(1);
    let mut count = 0usize;
    for op in &job.operations {
        validate_op(caps, op, holders).map_err(|e| match e {
            MachineError::Limit(m) if !op.layer.is_empty() => {
                MachineError::Limit(format!("{m} (layer {})", op.layer))
            }
            e => e,
        })?;
        for path in &op.paths {
            count = count.saturating_add(path.len());
            if path.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
                return limit("design has a non-finite coordinate".to_string());
            }
        }
    }
    if count > MAX_POINTS {
        return limit(format!("{count} points exceeds the {MAX_POINTS} cap"));
    }
    if job.track_enhancing && !caps.protocol.is_silhouette() {
        return limit("track enhancing is a Silhouette-only feature".to_string());
    }
    if job.mat != Mat::None && !caps.mats.contains(&job.mat) {
        return limit(format!(
            "mat {} is not supported by this machine",
            job.mat.id()
        ));
    }
    if let Some((w, l)) = job.media_size_mm
        && !(w.is_finite() && l.is_finite() && w > 0.0 && l > 0.0)
    {
        return limit("media size must be positive".to_string());
    }
    if let Some(reg) = &job.registration {
        if !caps.regmarks || !caps.protocol.is_silhouette() {
            return limit("registration marks are not supported".to_string());
        }
        if reg.quad && !caps.quad_regmarks {
            return limit("four-mark registration is not supported".to_string());
        }
        for v in [
            reg.width_mm,
            reg.length_mm,
            reg.origin_down_mm,
            reg.origin_right_mm,
        ] {
            if !v.is_finite() || v < 0.0 {
                return limit("registration geometry must be finite and non-negative".to_string());
            }
        }
        if reg.width_mm <= 0.0 || reg.length_mm <= 0.0 {
            return limit("registration area must be positive".to_string());
        }
    }

    // Area: the design (after origin offset and registration shift) must sit
    // inside every documented bound — cut area, mat, loaded media, mark area.
    let Some(bb) = job.bbox() else {
        return limit("job has no geometry".to_string());
    };
    let (shift_d, shift_r) = job
        .registration
        .map_or((0.0, 0.0), |r| (r.origin_down_mm, r.origin_right_mm));
    let top = bb.0 + job.origin_down_mm - shift_d;
    let left = bb.1 + job.origin_right_mm - shift_r;
    let bottom = bb.2 + job.origin_down_mm - shift_d;
    let right = bb.3 + job.origin_right_mm - shift_r;
    const EPS: f64 = 1e-6;
    if top < -EPS || left < -EPS {
        return limit(format!(
            "design starts at ({left:.2}, {top:.2}) mm, left of / above the origin"
        ));
    }
    let mut widths = vec![caps.cut_width_mm.or(caps.media_width_mm)];
    let mut lengths = vec![caps.cut_length_mm];
    if job.mat != Mat::None {
        let (d, r) = job.mat.extent_mm();
        widths.push(Some(r));
        lengths.push(Some(d));
    }
    if let Some((w, l)) = job.media_size_mm {
        widths.push(Some(w));
        lengths.push(Some(l));
    }
    if let Some(reg) = &job.registration {
        widths.push(Some(reg.width_mm));
        lengths.push(Some(reg.length_mm));
    }
    let Some(max_w) = widths.into_iter().flatten().reduce(f64::min) else {
        return limit("cut width is not documented: pass the loaded media size".to_string());
    };
    if right > max_w + EPS {
        return limit(format!(
            "design is {right:.2} mm wide; the limit is {max_w:.2} mm"
        ));
    }
    // Roll-fed machines with no documented length stop at the media end.
    if let Some(max_l) = lengths.into_iter().flatten().reduce(f64::min)
        && bottom > max_l + EPS
    {
        return limit(format!(
            "design is {bottom:.2} mm long; the limit is {max_l:.2} mm"
        ));
    }
    Ok(())
}

fn validate_op(caps: &Capabilities, op: &Operation, holders: u8) -> Result<(), MachineError> {
    let limit = |m: String| Err(MachineError::Limit(format!("{}: {m}", caps.id)));
    let s = &op.settings;
    let max_passes = if caps.multi_pass { MAX_PASSES } else { 1 };
    if !(1..=max_passes).contains(&s.passes) {
        return limit(format!("passes {} outside 1..={max_passes}", s.passes));
    }
    let max_overcut = if caps.overcut { MAX_OVERCUT_MM } else { 0.0 };
    if !s.overcut_mm.is_finite() || !(0.0..=max_overcut).contains(&s.overcut_mm) {
        return limit(format!(
            "overcut {} mm outside 0..={max_overcut}",
            s.overcut_mm
        ));
    }
    if !s.blade_offset_mm.is_finite() || !(0.0..=MAX_BLADE_DIAMETER_MM).contains(&s.blade_offset_mm)
    {
        return limit(format!(
            "blade offset {} mm outside 0..={MAX_BLADE_DIAMETER_MM}",
            s.blade_offset_mm
        ));
    }
    for (name, v) in [
        ("sharpen start", s.sharpen_start_mm),
        ("sharpen end", s.sharpen_end_mm),
    ] {
        if !v.is_finite() || !(0.0..=2.0).contains(&v) {
            return limit(format!("{name} {v} mm outside 0..=2"));
        }
    }
    if let Some(tool) = op.tool
        && !caps.tools.contains(&tool)
    {
        let known: Vec<&str> = caps.tools.iter().map(|t| t.id()).collect();
        return limit(format!(
            "tool {} is not documented for this machine (known: {})",
            tool.id(),
            known.join(", ")
        ));
    }
    if op.holder < 1 || op.holder > holders {
        return limit(format!("tool holder {} outside 1..={holders}", op.holder));
    }
    for (name, value, range) in [
        ("force", s.force, &caps.force),
        ("speed", s.speed, &caps.speed),
    ] {
        match (value, range) {
            (Some(v), Some(r)) if !r.contains(v) => {
                return limit(
                    format!("{name} {v} outside {}..={} {}", r.min, r.max, r.unit)
                        .trim_end()
                        .to_string(),
                );
            }
            (Some(_), None) => {
                return limit(format!(
                    "{name} range is not documented: set {name} on the machine's panel"
                ));
            }
            _ => {}
        }
    }
    if op.tool == Some(ToolKind::AutoBlade)
        && let Some(d) = s.depth
    {
        if d > 10 {
            return limit(format!("AutoBlade depth {d} outside 0..=10"));
        }
        if op.holder != 1 && caps.protocol != ProtocolId::SilhouetteLegacy {
            return limit("AutoBlade depth can only be set in tool holder 1".to_string());
        }
    }
    Ok(())
}
