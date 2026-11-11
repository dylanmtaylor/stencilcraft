//! Silhouette / Graphtec GP-GL, three generations (legacy, Cameo 3, Cameo 4
//! line): the mapping of a job and a machine onto `craft-plotter`'s Silhouette
//! dialect, and the send session. See `docs/machine.md`.

use super::{DriverEvent, MachineStatus, Protocol, SendReport, SessionHooks, refused};
use crate::{
    Capabilities, Command, CutJob, CutProgram, DeviceRegistry, DeviceStatus, ESC_INIT,
    MachineError, Operation, ProtocolId, Transport, parse_status, query_status, query_version,
};
use craft_plotter::gpgl::silhouette::{self, Generation, Setup};
use craft_plotter::stream;

/// Silhouette-family GP-GL; the id picks the generation.
pub struct SilhouetteProtocol(pub ProtocolId);

/// What the preamble announces for this machine and job. The cutting region
/// (down, right) is the machine's documented area, else the loaded media.
fn setup_for(dialect: ProtocolId, caps: &Capabilities, job: &CutJob) -> Setup {
    let media = job.media_size_mm;
    Setup {
        generation: match dialect {
            ProtocolId::SilhouetteCameo4 => Generation::Cameo4,
            ProtocolId::SilhouetteCameo3 => Generation::Cameo3,
            _ => Generation::Legacy,
        },
        two_holders: caps.tool_holders == Some(2),
        area_down_mm: caps.cut_length_mm.or(media.map(|m| m.1)).unwrap_or(0.0),
        area_right_mm: caps.cut_width_mm.or(media.map(|m| m.0)).unwrap_or(0.0),
        mat: job.mat,
        landscape: job.landscape,
        track_enhancing: job.track_enhancing,
        media_id: job.media_id,
    }
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
    silhouette::setup(&setup_for(dialect, caps, job), &op.neutral(), first)
}

/// Status polls before giving up on a busy device.
const READY_POLLS: usize = 15;
/// Reads to wait for the registration result (each read times out on its own).
const REGISTRATION_READS: usize = 20;

impl Protocol for SilhouetteProtocol {
    fn id(&self) -> ProtocolId {
        self.0
    }

    fn encode(&self, caps: &Capabilities, job: &CutJob) -> Result<CutProgram, MachineError> {
        silhouette::encode(&setup_for(self.0, caps, job), &job.neutral()).map_err(refused)
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
        let (head, rest) = program.split_at_registration();
        let mut bytes = 0;
        if !head.is_empty() {
            bytes += t.write(&stream(head, program.framing))?;
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
        bytes += t.write(&stream(rest, program.framing))?;
        hooks.event(DriverEvent::Sent { bytes, total });
        hooks.event(DriverEvent::Done);
        Ok(SendReport {
            version: Some(version),
            bytes,
        })
    }
}
