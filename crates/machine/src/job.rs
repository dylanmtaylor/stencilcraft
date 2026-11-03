//! `CutJob`: the vendor-neutral description of what to cut, and its
//! validation against a machine's `Capabilities`. Every protocol encodes
//! from a `CutJob`; nothing reaches a protocol without passing `validate`.

use crate::{Capabilities, MachineError, Mat, ProtocolId, ToolKind};

/// Hard caps on job size from untrusted documents.
pub const MAX_POINTS: usize = 2_000_000;
pub const MAX_PASSES: u8 = 10;
pub const MAX_OPERATIONS: usize = 64;
/// Largest line-segment overcut the encoder accepts, mm.
pub const MAX_OVERCUT_MM: f64 = 2.0;
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

/// Tool settings for one operation. `None` = use the machine panel's value.
#[derive(Debug, Clone, PartialEq)]
pub struct OpSettings {
    /// Device force steps (Silhouette 1..33) or laser power (G-code S).
    pub force: Option<u32>,
    /// Device speed steps or mm/min, per the machine's `speed.unit`.
    pub speed: Option<u32>,
    pub passes: u8,
    /// AutoBlade depth 0..10.
    pub depth: Option<u8>,
    /// Line-segment overcut at sharp corners, mm (0 = off).
    pub overcut_mm: f64,
    /// Blade swivel offset, mm (0.9 for the stock Silhouette blade).
    pub blade_offset_mm: f64,
    pub sharpen_corners: bool,
    pub sharpen_start_mm: f64,
    pub sharpen_end_mm: f64,
}

impl Default for OpSettings {
    fn default() -> Self {
        OpSettings {
            force: None,
            speed: None,
            passes: 1,
            depth: None,
            overcut_mm: 0.0,
            blade_offset_mm: 0.9,
            sharpen_corners: false,
            sharpen_start_mm: 0.1,
            sharpen_end_mm: 0.1,
        }
    }
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
}

impl Operation {
    pub fn is_pen(&self) -> bool {
        self.kind == OpKind::Draw || self.tool == Some(ToolKind::Pen)
    }
}

/// Print-and-cut registration: the optical sensor finds the printed marks
/// before cutting. All mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Registration {
    /// Distance between the marks across the media (right axis).
    pub width_mm: f64,
    /// Distance between the marks along the feed (down axis).
    pub length_mm: f64,
    /// Position of the top-left mark on the page; the design is shifted so
    /// that the mark becomes the origin.
    pub origin_right_mm: f64,
    pub origin_down_mm: f64,
    /// Automatic search; `false` = manual positioning.
    pub search: bool,
    /// Four-mark search (only machines whose capabilities say so).
    pub quad: bool,
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
        }
    }
}

/// Bounding box in mm `(down-min, right-min, down-max, right-max)`.
pub type BBoxMm = (f64, f64, f64, f64);

/// A list of polylines in mm, `(x right, y down)`.
pub type Paths = Vec<Vec<(f64, f64)>>;

/// Compute the mm bounding box of a path list, or `None` when empty.
pub fn find_bbox(paths: &[Vec<(f64, f64)>]) -> Option<BBoxMm> {
    let mut bbox: Option<BBoxMm> = None;
    for &(x, y) in paths.iter().flatten() {
        bbox = Some(match bbox {
            None => (y, x, y, x),
            Some((d0, r0, d1, r1)) => (d0.min(y), r0.min(x), d1.max(y), r1.max(x)),
        });
    }
    bbox
}

impl CutJob {
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

/// The closed rectangle path `(x, y)` around a bbox.
pub(crate) fn bbox_rect(bb: BBoxMm) -> Vec<(f64, f64)> {
    vec![
        (bb.1, bb.0),
        (bb.3, bb.0),
        (bb.3, bb.2),
        (bb.1, bb.2),
        (bb.1, bb.0),
    ]
}

/// Corners sharper than this turn (30°) get a line-segment overcut; gentler
/// joints are flattened curves and must not be overcut.
const OVERCUT_MIN_TURN_COS: f64 = 0.866;

/// The paths an operation actually cuts: overcut applied, each path repeated
/// `passes` times back to back, shifted by the registration origin.
pub(crate) fn prepared_paths(op: &Operation, reg: Option<&Registration>) -> Vec<Vec<(f64, f64)>> {
    let passes = op.settings.passes.max(1);
    let (sd, sr) = reg.map_or((0.0, 0.0), |r| (r.origin_down_mm, r.origin_right_mm));
    let mut out = Vec::with_capacity(op.paths.len() * usize::from(passes));
    for path in op.paths.iter().filter(|p| p.len() >= 2) {
        let cut = if op.settings.overcut_mm > 0.0 {
            overcut_path(path, op.settings.overcut_mm)
        } else {
            path.clone()
        };
        let cut: Vec<(f64, f64)> = cut.into_iter().map(|(x, y)| (x - sr, y - sd)).collect();
        for _ in 0..passes {
            out.push(cut.clone());
        }
    }
    out
}

pub(crate) fn overcut_path(path: &[(f64, f64)], o: f64) -> Vec<(f64, f64)> {
    let closed = path.len() > 2 && path.first() == path.last();
    let n = path.len();
    let mut out = Vec::with_capacity(n * 3);
    for (i, &b) in path.iter().enumerate() {
        out.push(b);
        // The segment arriving at b, and the one leaving it (wrapping on closed paths).
        let Some(&a) = i.checked_sub(1).and_then(|j| path.get(j)) else {
            continue;
        };
        let next = if i + 1 < n {
            path.get(i + 1)
        } else if closed {
            path.get(1)
        } else {
            None
        };
        let Some(&c) = next else { continue };
        let (ux, uy, ul) = (b.0 - a.0, b.1 - a.1, (b.0 - a.0).hypot(b.1 - a.1));
        let (vx, vy, vl) = (c.0 - b.0, c.1 - b.1, (c.0 - b.0).hypot(c.1 - b.1));
        if ul <= f64::EPSILON || vl <= f64::EPSILON {
            continue;
        }
        let cos = (ux * vx + uy * vy) / (ul * vl);
        if cos < OVERCUT_MIN_TURN_COS {
            out.push((b.0 + ux / ul * o, b.1 + uy / ul * o));
            out.push(b);
        }
    }
    out
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
    let holders = caps.tool_holders.unwrap_or(1);
    let mut count = 0usize;
    for op in &job.operations {
        validate_op(caps, op, holders)?;
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
