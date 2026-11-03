//! Protocols turn a validated `CutJob` into a `CutProgram` (the exact bytes)
//! and, for interactive transports, run the send session: identify, wait for
//! ready, stream, report status, and pause/cancel where the protocol has it.
//!
//! Implementations: Silhouette/Graphtec GP-GL (three generations) and plain
//! GP-GL (`gpgl`), HP-GL/HP-GL/2 and DMPL (`plotter`), Grbl G-code (`gcode`),
//! and `Unsupported` for driver slots that must not encode (Cricut, Curio).

use crate::{Capabilities, CutJob, CutProgram, MachineError, ProtocolId, Transport};

mod gcode;
mod gpgl;
mod plotter;

pub use gcode::{GcodeProtocol, parse_grbl_status};
pub use gpgl::{SilhouetteProtocol, encode_registration};
pub(crate) use gpgl::{park as gpgl_park, plot_body as gpgl_plot_body, setup as gpgl_setup};
pub use plotter::{
    DMPL_PER_MM, DmplProtocol, GPGL_PER_MM, GpglProtocol, HPGL_PER_MM, HpglProtocol,
};

/// Machine state as a protocol reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineStatus {
    Ready,
    Busy,
    Paused,
    /// No media loaded.
    MediaUnloaded,
    Error(String),
    /// The reply could not be decoded (raw text kept for diagnostics).
    Unknown(String),
}

/// Things a send session reports while it runs.
#[derive(Debug, Clone, PartialEq)]
pub enum DriverEvent {
    Identified(String),
    Status(MachineStatus),
    /// Waiting for the operator / registration-mark search.
    Waiting(&'static str),
    Sent {
        bytes: usize,
        total: usize,
    },
    Done,
}

/// Callbacks a session uses: a pause between polls (a real sleep, or nothing
/// in tests) and an event sink.
pub trait SessionHooks {
    fn wait(&mut self) {}
    fn event(&mut self, _event: DriverEvent) {}
}

/// No-op hooks.
pub struct NoHooks;
impl SessionHooks for NoHooks {}

/// Hooks from a plain wait closure (the original `send_job` signature).
pub struct WaitFn<'a>(pub &'a mut dyn FnMut());
impl SessionHooks for WaitFn<'_> {
    fn wait(&mut self) {
        (self.0)();
    }
}

/// What a completed real send reports.
#[derive(Debug, Clone, PartialEq)]
pub struct SendReport {
    pub version: Option<String>,
    pub bytes: usize,
}

/// A wire protocol. `encode` assumes `job` already passed `validate(caps, job)`.
pub trait Protocol {
    fn id(&self) -> ProtocolId;

    fn encode(&self, caps: &Capabilities, job: &CutJob) -> CutProgram;

    /// Query status, where the protocol defines a query.
    fn status(&self, _t: &mut dyn Transport) -> Result<MachineStatus, MachineError> {
        Err(self.unsupported("status query"))
    }

    fn pause(&self, _t: &mut dyn Transport) -> Result<(), MachineError> {
        Err(self.unsupported("pause"))
    }

    fn resume(&self, _t: &mut dyn Transport) -> Result<(), MachineError> {
        Err(self.unsupported("resume"))
    }

    fn cancel(&self, _t: &mut dyn Transport) -> Result<(), MachineError> {
        Err(self.unsupported("cancel"))
    }

    /// Run the send session. Default: stream the program as-is.
    fn send(
        &self,
        t: &mut dyn Transport,
        _caps: &Capabilities,
        program: &CutProgram,
        hooks: &mut dyn SessionHooks,
    ) -> Result<SendReport, MachineError> {
        let stream = program.to_stream();
        let bytes = t.write(&stream)?;
        hooks.event(DriverEvent::Sent {
            bytes,
            total: stream.len(),
        });
        hooks.event(DriverEvent::Done);
        Ok(SendReport {
            version: None,
            bytes,
        })
    }

    fn unsupported(&self, what: &str) -> MachineError {
        MachineError::Unsupported(format!(
            "{what} is not supported by the {} protocol",
            self.id().id()
        ))
    }
}

/// A driver slot whose protocol is not implemented; it never encodes.
pub struct UnsupportedProtocol(pub ProtocolId);

impl Protocol for UnsupportedProtocol {
    fn id(&self) -> ProtocolId {
        self.0
    }

    fn encode(&self, _caps: &Capabilities, _job: &CutJob) -> CutProgram {
        CutProgram::default()
    }

    fn send(
        &self,
        _t: &mut dyn Transport,
        caps: &Capabilities,
        _program: &CutProgram,
        _hooks: &mut dyn SessionHooks,
    ) -> Result<SendReport, MachineError> {
        Err(MachineError::Unsupported(format!(
            "{}: no implemented protocol; export SVG/DXF instead",
            caps.id
        )))
    }
}

/// The protocol implementation for an id.
pub fn protocol_for(id: ProtocolId) -> Box<dyn Protocol> {
    match id {
        ProtocolId::SilhouetteLegacy
        | ProtocolId::SilhouetteCameo3
        | ProtocolId::SilhouetteCameo4 => Box::new(SilhouetteProtocol(id)),
        ProtocolId::Gpgl => Box::new(GpglProtocol),
        ProtocolId::Hpgl => Box::new(HpglProtocol),
        ProtocolId::Dmpl => Box::new(DmplProtocol),
        ProtocolId::Gcode => Box::new(GcodeProtocol),
        ProtocolId::ExportOnly | ProtocolId::Unsupported => Box::new(UnsupportedProtocol(id)),
    }
}
