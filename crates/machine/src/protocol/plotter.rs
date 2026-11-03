//! Generic plotter protocols for cutters outside the Silhouette family:
//! HP-GL (the HP-GL/2 subset `IN SP PU PD` — Roland, USCutter, Vinyl Express,
//! many Chinese plotters, Graphtec in HP-GL mode), plain GP-GL, and DMPL.

use super::Protocol;
use crate::job::{Paths, bbox_rect, find_bbox, prepared_paths};

/// (holder, speed, prepared paths) for one operation.
type OpBody = (u8, Option<u32>, Paths);
use crate::{Capabilities, Command, CutJob, CutProgram, Framing, ProtocolId};

/// HP-GL plotter units per mm (1 plu = 0.025 mm).
pub const HPGL_PER_MM: f64 = 40.0;
/// Plain GP-GL step: 0.1 mm (the panel's GP-GL step size must match).
pub const GPGL_PER_MM: f64 = 10.0;
/// DMPL `EC1` resolution: 0.001 in.
pub const DMPL_PER_MM: f64 = 1.0 / 0.0254;

fn units(mm: f64, per_mm: f64) -> i64 {
    (mm * per_mm).round() as i64
}

/// Per-operation path lists (passes/overcut/registration applied), the bbox
/// over all of them, and the bbox-only substitution.
fn bodies(job: &CutJob) -> Option<(Vec<OpBody>, crate::BBoxMm)> {
    let reg = job.registration.as_ref();
    let mut ops: Vec<OpBody> = job
        .operations
        .iter()
        .map(|op| (op.holder, op.settings.speed, prepared_paths(op, reg)))
        .collect();
    let all: Vec<Vec<(f64, f64)>> = ops.iter().flat_map(|o| o.2.iter().cloned()).collect();
    let bb = find_bbox(&all)?;
    if job.bbox_only {
        ops.truncate(1);
        if let Some(first) = ops.first_mut() {
            first.2 = vec![bbox_rect(bb)];
        }
    }
    Some((ops, bb))
}

/// HP-GL and DMPL are y-up: flip about the design's bottom edge so the cut
/// reads the right way round with the origin at the bottom-left.
fn y_up(job: &CutJob, bottom: f64, per: f64) -> impl Fn(&(f64, f64)) -> (i64, i64) {
    let (dm, rm) = (job.origin_down_mm, job.origin_right_mm);
    move |&(x, y)| (units(x + rm, per), units(bottom + dm - (y + dm), per))
}

fn program(commands: Vec<Command>, bb: crate::BBoxMm, framing: Framing) -> CutProgram {
    CutProgram {
        commands,
        bbox_mm: Some(bb),
        framing,
        registration_wait: None,
    }
}

pub struct HpglProtocol;

impl Protocol for HpglProtocol {
    fn id(&self) -> ProtocolId {
        ProtocolId::Hpgl
    }

    fn encode(&self, _caps: &Capabilities, job: &CutJob) -> CutProgram {
        let Some((ops, bb)) = bodies(job) else {
            return CutProgram::default();
        };
        let p = y_up(job, bb.2, HPGL_PER_MM);
        let mut c = vec![Command::Raw("IN;".to_string())];
        for (holder, _, paths) in &ops {
            c.push(Command::Raw(format!("SP{holder};")));
            for path in paths {
                let Some((first, rest)) = path.split_first() else {
                    continue;
                };
                let (x, y) = p(first);
                c.push(Command::Raw(format!("PU{x},{y};")));
                let pts: Vec<String> = rest
                    .iter()
                    .map(|q| {
                        let (x, y) = p(q);
                        format!("{x},{y}")
                    })
                    .collect();
                c.push(Command::Raw(format!("PD{};", pts.join(","))));
            }
        }
        c.push(Command::Raw("PU;".to_string()));
        c.push(Command::Raw("SP0;".to_string()));
        program(c, bb, Framing::Newline)
    }
}

pub struct DmplProtocol;

impl Protocol for DmplProtocol {
    fn id(&self) -> ProtocolId {
        ProtocolId::Dmpl
    }

    fn encode(&self, _caps: &Capabilities, job: &CutJob) -> CutProgram {
        let Some((ops, bb)) = bodies(job) else {
            return CutProgram::default();
        };
        let p = y_up(job, bb.2, DMPL_PER_MM);
        let mut c = vec![
            Command::Raw(";:".to_string()),
            Command::Raw("EC1".to_string()),
        ];
        for path in ops.iter().flat_map(|o| o.2.iter()) {
            let Some((first, rest)) = path.split_first() else {
                continue;
            };
            let (x, y) = p(first);
            c.push(Command::Raw(format!("U{x},{y}")));
            let pts: Vec<String> = rest
                .iter()
                .map(|q| {
                    let (x, y) = p(q);
                    format!("{x},{y}")
                })
                .collect();
            c.push(Command::Raw(format!("D{}", pts.join(","))));
        }
        c.push(Command::Raw("U0,0".to_string()));
        c.push(Command::Raw("@".to_string()));
        program(c, bb, Framing::Newline)
    }
}

/// Plain GP-GL: same axis order as the Silhouette dialect (down, right).
pub struct GpglProtocol;

impl Protocol for GpglProtocol {
    fn id(&self) -> ProtocolId {
        ProtocolId::Gpgl
    }

    fn encode(&self, _caps: &Capabilities, job: &CutJob) -> CutProgram {
        let Some((ops, bb)) = bodies(job) else {
            return CutProgram::default();
        };
        let (dm, rm) = (job.origin_down_mm, job.origin_right_mm);
        let g = |&(x, y): &(f64, f64)| (units(y + dm, GPGL_PER_MM), units(x + rm, GPGL_PER_MM));
        let mut c = Vec::new();
        for (_, speed, paths) in &ops {
            if let Some(s) = speed {
                c.push(Command::Raw(format!("!{s}")));
            }
            for path in paths {
                let Some((first, rest)) = path.split_first() else {
                    continue;
                };
                let (d, r) = g(first);
                c.push(Command::Raw(format!("M{d},{r}")));
                for q in rest {
                    let (d, r) = g(q);
                    c.push(Command::Raw(format!("D{d},{r}")));
                }
            }
        }
        // Park below the cut, head up.
        c.push(Command::Raw(format!(
            "M{},0",
            units(bb.2 + job.end_feed_mm.max(0.0), GPGL_PER_MM)
        )));
        program(c, bb, Framing::Etx)
    }
}
