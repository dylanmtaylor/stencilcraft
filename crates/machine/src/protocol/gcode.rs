//! Grbl-style G-code for lasers and knife/pen CNCs, from a `CutJob`. Force is
//! the spindle/laser power `S` (Grbl `$30`, default 0–1000), speed the feed in
//! mm/min. Real-time commands from the Grbl documentation
//! (<https://github.com/gnea/grbl/wiki/Grbl-v1.1-Interface>): `?` status,
//! `!` feed hold, `~` resume, `0x18` soft reset.

use super::{DriverEvent, MachineStatus, Protocol, SendReport, SessionHooks};
use crate::job::{bbox_rect, find_bbox, prepared_paths};
use crate::{
    Capabilities, Command, CutJob, CutProgram, Framing, MachineError, ProtocolId, Transport,
};

/// Feed used when an operation leaves speed to the machine; `G1` without a
/// feed is a Grbl error.
const DEFAULT_FEED: u32 = 1000;
/// Reads to wait for one line's `ok`.
const ACK_READS: usize = 30;

pub struct GcodeProtocol;

/// Decode a Grbl status report (`<Idle|MPos:…>`).
pub fn parse_grbl_status(reply: &str) -> MachineStatus {
    let Some(body) = reply.trim().strip_prefix('<') else {
        return MachineStatus::Unknown(reply.trim().to_string());
    };
    let state = body.split(['|', '>', ':']).next().unwrap_or("");
    match state {
        "Idle" => MachineStatus::Ready,
        "Run" | "Jog" | "Home" => MachineStatus::Busy,
        "Hold" | "Door" => MachineStatus::Paused,
        "Alarm" => MachineStatus::Error("alarm".to_string()),
        other => MachineStatus::Unknown(other.to_string()),
    }
}

impl Protocol for GcodeProtocol {
    fn id(&self) -> ProtocolId {
        ProtocolId::Gcode
    }

    fn encode(&self, _caps: &Capabilities, job: &CutJob) -> CutProgram {
        let reg = job.registration.as_ref();
        let ops: Vec<_> = job
            .operations
            .iter()
            .map(|op| (op, prepared_paths(op, reg)))
            .collect();
        let all: Vec<Vec<(f64, f64)>> = ops.iter().flat_map(|(_, p)| p.iter().cloned()).collect();
        let Some(bb) = find_bbox(&all) else {
            return CutProgram::default();
        };
        let (dm, rm) = (job.origin_down_mm, job.origin_right_mm);
        let settings = craft_gcode::Settings {
            feed_mm_per_min: f64::from(DEFAULT_FEED),
            ..craft_gcode::Settings::default()
        };
        let Ok(mut writer) = craft_gcode::Writer::bare(settings) else {
            return CutProgram::default();
        };
        for (op, paths) in &ops {
            let feed = op.settings.speed.unwrap_or(DEFAULT_FEED);
            let power = op.settings.force.unwrap_or(0);
            if writer.set_feed(f64::from(feed)).is_err() {
                return CutProgram::default();
            }
            let body: Vec<Vec<(f64, f64)>> = if job.bbox_only {
                vec![bbox_rect(bb)]
            } else {
                paths.clone()
            };
            for path in &body {
                // G-code is y-up: flip about the design's bottom edge. The
                // shared writer owns coordinate formatting and command syntax.
                let points: Vec<_> = path.iter().map(|&(x, y)| (x + rm, bb.2 - y + dm)).collect();
                if writer.contour(&points, false, power).is_err() {
                    return CutProgram::default();
                }
            }
            if job.bbox_only {
                break;
            }
        }
        let commands = writer
            .finish()
            .lines()
            .map(|line| Command::Raw(line.to_owned()))
            .collect();
        CutProgram {
            commands,
            bbox_mm: Some(bb),
            framing: Framing::Newline,
            registration_wait: None,
        }
    }

    fn status(&self, t: &mut dyn Transport) -> Result<MachineStatus, MachineError> {
        t.write(b"?")?;
        let mut buf = [0u8; 128];
        let n = t.read(&mut buf)?;
        Ok(parse_grbl_status(&String::from_utf8_lossy(
            buf.get(..n).unwrap_or_default(),
        )))
    }

    fn pause(&self, t: &mut dyn Transport) -> Result<(), MachineError> {
        t.write(b"!").map(|_| ())
    }

    fn resume(&self, t: &mut dyn Transport) -> Result<(), MachineError> {
        t.write(b"~").map(|_| ())
    }

    fn cancel(&self, t: &mut dyn Transport) -> Result<(), MachineError> {
        t.write(&[0x18]).map(|_| ())
    }

    /// Send-response streaming: one line, wait for `ok`; an `error` reply
    /// turns the laser/spindle off and aborts.
    fn send(
        &self,
        t: &mut dyn Transport,
        _caps: &Capabilities,
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
        let mut bytes = 0;
        for cmd in &program.commands {
            let line = format!("{}\n", cmd.to_ascii());
            bytes += t.write(line.as_bytes())?;
            let mut reply = String::new();
            let mut acked = false;
            for _ in 0..ACK_READS {
                let mut buf = [0u8; 128];
                let n = t.read(&mut buf)?;
                if n == 0 {
                    hooks.wait();
                    continue;
                }
                reply.push_str(&String::from_utf8_lossy(buf.get(..n).unwrap_or_default()));
                if reply.contains("error") || reply.contains("ALARM") {
                    let _ = t.write(b"M5\n");
                    return Err(MachineError::Protocol(format!(
                        "controller rejected `{}`: {}",
                        cmd.to_ascii(),
                        reply.trim()
                    )));
                }
                if reply.contains("ok") {
                    acked = true;
                    break;
                }
            }
            if !acked {
                let _ = t.write(b"M5\n");
                return Err(MachineError::Protocol(format!(
                    "no `ok` for `{}`; laser/spindle switched off",
                    cmd.to_ascii()
                )));
            }
            hooks.event(DriverEvent::Sent { bytes, total });
        }
        hooks.event(DriverEvent::Done);
        Ok(SendReport {
            version: None,
            bytes,
        })
    }
}
