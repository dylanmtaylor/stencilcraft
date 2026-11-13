//! Stencil machine and registration adapters for the shared Silhouette GP-GL encoder.

use super::{
    DriverEvent, MachineStatus, Protocol, SendReport, SessionHooks, from_plotter_command,
    from_plotter_program, to_plotter_job, to_plotter_operation, to_plotter_setup,
};
use crate::{
    Capabilities, Command, CutJob, CutProgram, DeviceRegistry, DeviceStatus, ESC_INIT,
    MachineError, Operation, ProtocolId, Registration, Transport, parse_status, query_status,
    query_version, stream_of,
};

const READY_POLLS: usize = 15;
const REGISTRATION_READS: usize = 20;

/// Silhouette-family GP-GL; the id picks the generation.
pub struct SilhouetteProtocol(pub ProtocolId);

/// Encodes the shared setup preamble as Stencil's public command type.
pub(crate) fn setup(
    dialect: ProtocolId,
    caps: &Capabilities,
    job: &CutJob,
    op: &Operation,
    first: bool,
) -> Vec<Command> {
    craft_plotter::gpgl::silhouette::setup(
        &to_plotter_setup(dialect, caps, job),
        &to_plotter_operation(op),
        first,
    )
    .into_iter()
    .map(from_plotter_command)
    .collect()
}

/// Shared registration-mark geometry and search commands.
pub fn encode_registration(reg: &Registration) -> Vec<Command> {
    craft_plotter::gpgl::silhouette::registration(&craft_plotter::Registration {
        width_mm: reg.width_mm,
        length_mm: reg.length_mm,
        origin_right_mm: reg.origin_right_mm,
        origin_down_mm: reg.origin_down_mm,
        search: reg.search,
        quad: reg.quad,
    })
    .into_iter()
    .map(from_plotter_command)
    .collect()
}

pub(crate) fn plot_body(paths: &[Vec<(f64, f64)>], down: f64, right: f64) -> Vec<Command> {
    craft_plotter::gpgl::silhouette::plot_body(paths, down, right)
        .into_iter()
        .map(from_plotter_command)
        .collect()
}

pub(crate) fn park(bb: crate::BBoxMm, end_feed_mm: f64) -> Vec<Command> {
    craft_plotter::gpgl::silhouette::park(bb, end_feed_mm)
        .into_iter()
        .map(from_plotter_command)
        .collect()
}

impl Protocol for SilhouetteProtocol {
    fn id(&self) -> ProtocolId {
        self.0
    }

    fn encode(&self, caps: &Capabilities, job: &CutJob) -> CutProgram {
        craft_plotter::gpgl::silhouette::encode(
            &to_plotter_setup(self.0, caps, job),
            &to_plotter_job(job),
        )
        .map(from_plotter_program)
        .unwrap_or_default()
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
