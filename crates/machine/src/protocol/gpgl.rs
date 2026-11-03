//! Silhouette / Graphtec GP-GL, three generations (legacy, Cameo 3, Cameo 4
//! line). Facts from the community drivers' specification; see
//! `docs/machine.md`.

use super::{DriverEvent, MachineStatus, Protocol, SendReport, SessionHooks};
use crate::job::{BBoxMm, Paths, bbox_rect, find_bbox, prepared_paths};
use crate::{
    Capabilities, Command, CutJob, CutProgram, DeviceRegistry, DeviceStatus, ESC_INIT, Framing,
    MachineError, Operation, ProtocolId, Registration, ToolKind, Transport, parse_status,
    query_status, query_version, stream_of, su,
};

/// Silhouette-family GP-GL; the id picks the generation.
pub struct SilhouetteProtocol(pub ProtocolId);

/// The cutting region (down, right) in mm the boundary commands announce:
/// the machine's documented area, else the loaded media.
fn boundary_area(caps: &Capabilities, job: &CutJob) -> (f64, f64) {
    let media = job.media_size_mm;
    let right = caps.cut_width_mm.or(media.map(|m| m.0)).unwrap_or(0.0);
    let down = caps.cut_length_mm.or(media.map(|m| m.1)).unwrap_or(0.0);
    (down, right)
}

/// `FF` corner-sharpening pair: tenths of a millimetre (not device units),
/// truncated, start-only first then start+end; pens send zeros.
fn sharpen_pair(op: &Operation, t: u8) -> [Command; 2] {
    let s = &op.settings;
    let (start, end) = if op.is_pen() {
        (0, 0)
    } else {
        (
            ((s.sharpen_start_mm + 0.05) * 10.0) as i64,
            ((s.sharpen_end_mm + 0.05) * 10.0) as i64,
        )
    };
    [
        Command::SharpenCorners {
            start_tenths: start,
            end_tenths: 0,
            toolholder: t,
        },
        Command::SharpenCorners {
            start_tenths: start,
            end_tenths: end,
            toolholder: t,
        },
    ]
}

/// Mat select, reset, feed direction and cutting region — what the Cameo 3
/// and newer receive before any tool command.
fn mat_block(caps: &Capabilities, job: &CutJob) -> Vec<Command> {
    let (down, right) = boundary_area(caps, job);
    vec![
        Command::SelectMat(job.mat),
        Command::Reset,
        Command::PortraitMode(job.landscape),
        Command::BoundaryTopLeft {
            down_mm: 0.0,
            right_mm: 0.0,
        },
        Command::BoundaryBottomRight {
            down_mm: down,
            right_mm: right,
        },
    ]
}

/// AutoBlade depth (`TF`, holder 1 only).
fn depth_cmd(op: &Operation) -> Option<Command> {
    (op.tool == Some(ToolKind::AutoBlade) && op.holder == 1)
        .then_some(op.settings.depth)
        .flatten()
        .map(|d| Command::Depth(d, 1))
}

fn step(v: Option<u32>) -> Option<u8> {
    v.and_then(|v| u8::try_from(v).ok())
}

/// The preamble for one operation. `first` adds the job-level block (mat,
/// region, media, track enhancing); later operations only re-tool.
pub(crate) fn setup(
    dialect: ProtocolId,
    caps: &Capabilities,
    job: &CutJob,
    op: &Operation,
    first: bool,
) -> Vec<Command> {
    let t = op.holder;
    let pen = op.is_pen();
    let (force, speed) = (step(op.settings.force), step(op.settings.speed));
    let offset = if pen {
        0.0
    } else {
        op.settings.blade_offset_mm
    };
    let mut c = Vec::new();
    match dialect {
        ProtocolId::SilhouetteCameo4 => {
            if first {
                c.extend(mat_block(caps, job));
            }
            if caps.tool_holders == Some(2) {
                c.push(Command::SelectTool(t));
            }
            if let Some(p) = force {
                c.push(Command::Pressure(p, t));
                // On first connect acceleration is set to 0, then bumped later.
                c.push(Command::Acceleration(0));
            }
            if let Some(s) = speed {
                c.push(Command::Speed(s, t));
            }
            // The first cutter-offset command always reads 0 mm × 0.05 mm.
            c.push(Command::CutterOffset {
                x_mm: 0.0,
                y_mm: 0.05,
                toolholder: t,
            });
            c.push(Command::Lift {
                lift: op.settings.sharpen_corners,
                toolholder: t,
            });
            c.extend(sharpen_pair(op, t));
            // Force and acceleration go out a second time (observed behaviour).
            if let Some(p) = force {
                c.push(Command::Pressure(p, t));
                c.push(Command::Acceleration(3));
            }
            c.push(Command::CutterOffset {
                x_mm: offset,
                y_mm: 0.05,
                toolholder: t,
            });
            c.extend(depth_cmd(op));
            if first && job.track_enhancing {
                c.push(Command::TrackEnhancing(true));
            }
        }
        ProtocolId::SilhouetteCameo3 => {
            if first {
                c.extend(mat_block(caps, job));
            }
            c.push(Command::SelectTool(t));
            if let Some(s) = speed {
                c.push(Command::Speed(s, t));
            }
            if let Some(p) = force {
                c.push(Command::Pressure(p, t));
            }
            let zero = Command::CutterOffset {
                x_mm: 0.0,
                y_mm: 0.05,
                toolholder: t,
            };
            if pen {
                c.push(zero.clone());
            }
            c.push(Command::Lift {
                lift: op.settings.sharpen_corners,
                toolholder: t,
            });
            c.extend(sharpen_pair(op, t));
            if !pen {
                c.push(zero);
                c.push(Command::CutterOffset {
                    x_mm: offset,
                    y_mm: 0.05,
                    toolholder: t,
                });
            }
            c.extend(depth_cmd(op));
            if first && job.track_enhancing {
                c.push(Command::TrackEnhancing(true));
            }
        }
        _ => {
            if first && let Some(id) = job.media_id {
                c.push(Command::MediaSelect(if (100..=300).contains(&id) {
                    id
                } else {
                    300
                }));
            }
            if let Some(s) = speed {
                c.push(Command::SpeedLegacy(s));
            }
            if let Some(p) = force {
                c.push(Command::PressureLegacy(p));
            }
            c.push(Command::CutterOffsetLegacy(offset));
            if first {
                c.push(Command::TrackEnhancing(job.track_enhancing));
                c.push(Command::Reset);
                c.push(Command::PortraitMode(job.landscape));
                let (down, right) = boundary_area(caps, job);
                c.push(Command::BoundaryTopLeft {
                    down_mm: 0.0,
                    right_mm: 0.0,
                });
                c.push(Command::BoundaryBottomRight {
                    down_mm: down,
                    right_mm: right,
                });
                c.push(Command::PenUp);
                c.push(Command::Raw("FE0,0".to_string()));
                c.push(Command::Raw("FF0,0,0".to_string()));
            }
        }
    }
    c
}

/// Registration-mark commands: mark geometry, then the search (or manual
/// positioning) request. The device answers `    0\x03` once it has the marks.
pub fn encode_registration(reg: &Registration) -> Vec<Command> {
    let raw = |s: &str| Command::Raw(s.to_string());
    let mut c = vec![
        Command::PortraitMode(false),
        raw("TB99"),
        // Mark type 2 = Cameo/Portrait marks; arm length 400 SU (20 mm); stroke 10 SU.
        raw("TB52,2"),
        raw("TB51,400"),
        raw("TB53,10"),
        raw("TB55,1"),
    ];
    let (h, w) = (su(reg.length_mm), su(reg.width_mm));
    if reg.search {
        // Search 10 mm around the expected first-mark position.
        let top = su((reg.origin_down_mm - 10.0).max(0.0));
        let left = su((reg.origin_right_mm - 10.0).max(0.0));
        let code = if reg.quad { 124 } else { 123 };
        c.push(Command::Raw(format!("TB{code},{h},{w},{top},{left}")));
    } else {
        c.push(Command::Raw(format!("TB23,{h},{w}")));
    }
    c
}

/// Move/draw commands for paths, offset by the job origin.
pub(crate) fn plot_body(paths: &[Vec<(f64, f64)>], down: f64, right: f64) -> Vec<Command> {
    let mut commands = Vec::new();
    for path in paths {
        let Some((first, rest)) = path.split_first() else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        commands.push(Command::Move {
            down_mm: first.1 + down,
            right_mm: first.0 + right,
        });
        for &(x, y) in rest {
            commands.push(Command::Draw {
                down_mm: y + down,
                right_mm: x + right,
            });
        }
    }
    commands
}

/// Park below the cut so the next job (or the cross-cutter) starts there.
pub(crate) fn park(bb: BBoxMm, end_feed_mm: f64) -> [Command; 2] {
    [
        Command::Move {
            down_mm: bb.2 + end_feed_mm,
            right_mm: 0.0,
        },
        Command::Origin,
    ]
}

/// Status polls before giving up on a busy device.
const READY_POLLS: usize = 15;
/// Reads to wait for the registration result (each read times out on its own).
const REGISTRATION_READS: usize = 20;

impl Protocol for SilhouetteProtocol {
    fn id(&self) -> ProtocolId {
        self.0
    }

    fn encode(&self, caps: &Capabilities, job: &CutJob) -> CutProgram {
        let reg = job.registration.as_ref();
        let ops: Vec<(&Operation, Paths)> = job
            .operations
            .iter()
            .map(|op| (op, prepared_paths(op, reg)))
            .collect();
        let all: Vec<Vec<(f64, f64)>> = ops.iter().flat_map(|(_, p)| p.iter().cloned()).collect();
        let Some(bb) = find_bbox(&all) else {
            return CutProgram::default();
        };
        let mut commands = Vec::new();
        let mut registration_wait = None;
        for (i, (op, paths)) in ops.iter().enumerate() {
            commands.extend(setup(self.0, caps, job, op, i == 0));
            if i == 0
                && let Some(r) = reg
            {
                commands.extend(encode_registration(r));
                registration_wait = Some(commands.len());
            }
            if job.bbox_only {
                commands.extend(plot_body(
                    &[bbox_rect(bb)],
                    job.origin_down_mm,
                    job.origin_right_mm,
                ));
                break;
            }
            commands.extend(plot_body(paths, job.origin_down_mm, job.origin_right_mm));
        }
        commands.extend(park(bb, job.end_feed_mm));
        CutProgram {
            commands,
            bbox_mm: Some(bb),
            framing: Framing::Etx,
            registration_wait,
        }
    }

    fn status(&self, t: &mut dyn Transport) -> Result<MachineStatus, MachineError> {
        Ok(match query_status(t)? {
            DeviceStatus::Ready => MachineStatus::Ready,
            DeviceStatus::Moving => MachineStatus::Busy,
            DeviceStatus::Unloaded => MachineStatus::MediaUnloaded,
            DeviceStatus::Unknown(b) => MachineStatus::Unknown(format!("status byte {b}")),
        })
    }

    /// Initialize, check the firmware names this machine (a different known
    /// model aborts — the limits we validated would be the wrong ones), poll
    /// until ready, stream the setup, wait for registration marks when the
    /// program asks, then the plot. Non-interactive transports just stream.
    fn send(
        &self,
        t: &mut dyn Transport,
        caps: &Capabilities,
        program: &CutProgram,
        hooks: &mut dyn SessionHooks,
    ) -> Result<SendReport, MachineError> {
        let total = program.to_stream().len();
        if !t.interactive() {
            let bytes = t.write(&program.to_stream())?;
            hooks.event(DriverEvent::Sent { bytes, total });
            hooks.event(DriverEvent::Done);
            return Ok(SendReport {
                version: None,
                bytes,
            });
        }
        t.write(ESC_INIT.as_bytes())?;
        let version = query_version(t)?;
        hooks.event(DriverEvent::Identified(version.clone()));
        if let Some(actual) = DeviceRegistry::builtin().by_firmware(&version)
            && actual.id != caps.id
        {
            return Err(MachineError::Protocol(format!(
                "device reports {version:?} ({}) but the job was encoded for {}",
                actual.id, caps.id
            )));
        }
        let mut ready = false;
        for _ in 0..READY_POLLS {
            let status = self.status(t)?;
            hooks.event(DriverEvent::Status(status.clone()));
            match status {
                MachineStatus::Ready => {
                    ready = true;
                    break;
                }
                MachineStatus::MediaUnloaded => {
                    return Err(MachineError::Protocol(
                        "media not loaded: load material and retry".to_string(),
                    ));
                }
                _ => hooks.wait(),
            }
        }
        if !ready {
            return Err(MachineError::Protocol(
                "device never reported ready".to_string(),
            ));
        }
        let split = program
            .registration_wait
            .unwrap_or(0)
            .min(program.commands.len());
        let (head, rest) = program.commands.split_at(split);
        let mut bytes = 0;
        if !head.is_empty() {
            bytes += t.write(&stream_of(head, program.framing))?;
            hooks.event(DriverEvent::Waiting("registration marks"));
            let mut found = false;
            for _ in 0..REGISTRATION_READS {
                let mut buf = [0u8; 64];
                let n = t.read(&mut buf)?;
                let reply = buf.get(..n).unwrap_or_default();
                if reply.is_empty() {
                    hooks.wait();
                    continue;
                }
                found = parse_status(reply) == DeviceStatus::Ready;
                break;
            }
            if !found {
                return Err(MachineError::Protocol(
                    "registration marks not found: nothing was cut".to_string(),
                ));
            }
        }
        bytes += t.write(&stream_of(rest, program.framing))?;
        hooks.event(DriverEvent::Sent { bytes, total });
        hooks.event(DriverEvent::Done);
        Ok(SendReport {
            version: Some(version),
            bytes,
        })
    }
}
