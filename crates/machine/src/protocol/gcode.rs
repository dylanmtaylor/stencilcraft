//! Grbl-style G-code for lasers and knife/pen CNCs, from a `CutJob`. Force is
//! the spindle/laser power `S` (Grbl `$30`, default 0–1000), speed the feed in
//! mm/min. The lines are written by `craft-gcode`; its `grbl` module has the
//! real-time commands (`?` status, `!` feed hold, `~` resume, `0x18` soft
//! reset) and the reply parsers. This file maps the job onto them (y-up flip,
//! per-operation feed and power) and runs the send-response session.

use super::{DriverEvent, MachineStatus, Protocol, SendReport, SessionHooks, refused};
use crate::{
    Capabilities, Command, CutJob, CutProgram, Framing, MachineError, ProtocolId, Transport,
};
use craft_gcode::grbl::{self, Ack, State};
use craft_gcode::{Settings, Writer};

/// Feed used when an operation leaves speed to the machine (the G-code
/// exporter's documented default); `G1` without a feed is a Grbl error.
const DEFAULT_FEED: u32 = 1000;
/// Reads to wait for one line's `ok`.
const ACK_READS: usize = 30;

/// Laser/spindle off, sent when a job is abandoned mid-stream.
fn tool_off(t: &mut dyn Transport) {
    let _ = t.write(format!("{}\n", grbl::TOOL_OFF).as_bytes());
}

pub struct GcodeProtocol;

/// Decode a Grbl status report (`<Idle|MPos:…>`).
pub fn parse_grbl_status(reply: &str) -> MachineStatus {
    match grbl::parse_status(reply) {
        None => MachineStatus::Unknown(reply.trim().to_string()),
        Some(State::Idle) => MachineStatus::Ready,
        Some(State::Run | State::Jog | State::Home) => MachineStatus::Busy,
        Some(State::Hold | State::Door) => MachineStatus::Paused,
        Some(State::Alarm) => MachineStatus::Error("alarm".to_string()),
        Some(State::Check) => MachineStatus::Unknown("Check".to_string()),
        Some(State::Sleep) => MachineStatus::Unknown("Sleep".to_string()),
        Some(State::Other(word)) => MachineStatus::Unknown(word),
    }
}

impl Protocol for GcodeProtocol {
    fn id(&self) -> ProtocolId {
        ProtocolId::Gcode
    }

    fn encode(&self, _caps: &Capabilities, job: &CutJob) -> Result<CutProgram, MachineError> {
        let Some(prepared) = job.neutral().prepare().map_err(refused)? else {
            return Ok(CutProgram::default());
        };
        let bb = prepared.bbox_mm;
        let (dm, rm) = (job.origin_down_mm, job.origin_right_mm);
        let mut w = Writer::bare(Settings::default()).map_err(refused)?;
        for (op, paths) in job.operations.iter().zip(&prepared.operations) {
            let feed = op.settings.speed.unwrap_or(DEFAULT_FEED);
            w.set_feed(f64::from(feed)).map_err(refused)?;
            let power = op.settings.force.unwrap_or(0);
            for path in paths {
                // G-code is y-up: flip about the design's bottom edge.
                let pts: Vec<(f64, f64)> =
                    path.iter().map(|&(x, y)| (x + rm, bb.2 - y + dm)).collect();
                w.contour(&pts, false, power).map_err(refused)?;
            }
        }
        let commands = w
            .finish()
            .lines()
            .map(|l| Command::Raw(l.to_string()))
            .collect();
        Ok(CutProgram {
            commands,
            bbox_mm: Some(bb),
            framing: Framing::Newline,
            registration_wait: None,
        })
    }

    fn status(&self, t: &mut dyn Transport) -> Result<MachineStatus, MachineError> {
        t.write(&[grbl::STATUS_QUERY])?;
        let mut buf = [0u8; 128];
        let n = t.read(&mut buf)?;
        Ok(parse_grbl_status(&String::from_utf8_lossy(
            buf.get(..n).unwrap_or_default(),
        )))
    }

    fn pause(&self, t: &mut dyn Transport) -> Result<(), MachineError> {
        t.write(&[grbl::FEED_HOLD]).map(|_| ())
    }

    fn resume(&self, t: &mut dyn Transport) -> Result<(), MachineError> {
        t.write(&[grbl::CYCLE_START]).map(|_| ())
    }

    fn cancel(&self, t: &mut dyn Transport) -> Result<(), MachineError> {
        t.write(&[grbl::SOFT_RESET]).map(|_| ())
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
                match grbl::parse_ack(&reply) {
                    Some(Ack::Rejected) => {
                        tool_off(t);
                        return Err(MachineError::Protocol(format!(
                            "controller rejected `{}`: {}",
                            cmd.to_ascii(),
                            reply.trim()
                        )));
                    }
                    Some(Ack::Ok) => {
                        acked = true;
                        break;
                    }
                    None => {}
                }
            }
            if !acked {
                tool_off(t);
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
