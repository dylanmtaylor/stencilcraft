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

pub(crate) fn to_plotter_job(job: &CutJob) -> craft_plotter::Job {
    craft_plotter::Job {
        operations: job.operations.iter().map(to_plotter_operation).collect(),
        origin_down_mm: job.origin_down_mm,
        origin_right_mm: job.origin_right_mm,
        end_feed_mm: job.end_feed_mm,
        bbox_only: job.bbox_only,
        registration: job.registration.map(|reg| craft_plotter::Registration {
            width_mm: reg.width_mm,
            length_mm: reg.length_mm,
            origin_right_mm: reg.origin_right_mm,
            origin_down_mm: reg.origin_down_mm,
            search: reg.search,
            quad: reg.quad,
        }),
    }
}

pub(crate) fn to_plotter_operation(op: &crate::Operation) -> craft_plotter::Operation {
    craft_plotter::Operation {
        pen: op.is_pen(),
        holder: op.holder,
        paths: op.paths.clone(),
        settings: craft_plotter::ToolSettings {
            force: op.settings.force,
            speed: op.settings.speed,
            passes: op.settings.passes,
            depth: op.settings.depth,
            overcut_mm: op.settings.overcut_mm,
            blade_offset_mm: op.settings.blade_offset_mm,
            sharpen_corners: op.settings.sharpen_corners,
            sharpen_start_mm: op.settings.sharpen_start_mm,
            sharpen_end_mm: op.settings.sharpen_end_mm,
        },
    }
}

pub(crate) fn to_plotter_setup(
    dialect: ProtocolId,
    caps: &Capabilities,
    job: &CutJob,
) -> craft_plotter::gpgl::silhouette::Setup {
    use craft_plotter::gpgl::silhouette::{Generation, Setup};
    let generation = match dialect {
        ProtocolId::SilhouetteCameo4 => Generation::Cameo4,
        ProtocolId::SilhouetteCameo3 => Generation::Cameo3,
        _ => Generation::Legacy,
    };
    let media = job.media_size_mm;
    Setup {
        generation,
        two_holders: caps.tool_holders == Some(2),
        area_down_mm: caps.cut_length_mm.or(media.map(|m| m.1)).unwrap_or(0.0),
        area_right_mm: caps.cut_width_mm.or(media.map(|m| m.0)).unwrap_or(0.0),
        mat: job.mat.into(),
        landscape: job.landscape,
        track_enhancing: job.track_enhancing,
        media_id: job.media_id,
    }
}

pub(crate) fn from_plotter_program(program: craft_plotter::Program) -> CutProgram {
    let commands = program
        .commands
        .into_iter()
        .map(from_plotter_command)
        .collect();
    CutProgram {
        commands,
        bbox_mm: program.bbox_mm,
        framing: match program.framing {
            craft_plotter::Framing::Etx => crate::Framing::Etx,
            craft_plotter::Framing::Newline => crate::Framing::Newline,
        },
        registration_wait: program.registration_wait,
    }
}

pub(crate) fn from_plotter_command(command: craft_plotter::Command) -> crate::Command {
    use crate::Command as A;
    use craft_plotter::Command as C;
    match command {
        C::SelectTool(t) => A::SelectTool(t),
        C::Pressure(p, t) => A::Pressure(p, t),
        C::Speed(s, t) => A::Speed(s, t),
        C::Depth(d, t) => A::Depth(d, t),
        C::CutterOffset {
            x_mm,
            y_mm,
            toolholder,
        } => A::CutterOffset {
            x_mm,
            y_mm,
            toolholder,
        },
        C::Lift { lift, toolholder } => A::Lift { lift, toolholder },
        C::SharpenCorners {
            start_tenths,
            end_tenths,
            toolholder,
        } => A::SharpenCorners {
            start_tenths,
            end_tenths,
            toolholder,
        },
        C::Acceleration(a) => A::Acceleration(a),
        C::Move { down_mm, right_mm } => A::Move { down_mm, right_mm },
        C::Draw { down_mm, right_mm } => A::Draw { down_mm, right_mm },
        C::BoundaryTopLeft { down_mm, right_mm } => A::BoundaryTopLeft { down_mm, right_mm },
        C::BoundaryBottomRight { down_mm, right_mm } => {
            A::BoundaryBottomRight { down_mm, right_mm }
        }
        C::TrackEnhancing(on) => A::TrackEnhancing(on),
        C::SelectMat(mat) => A::SelectMat(mat.into()),
        C::Reset => A::Reset,
        C::PortraitMode(landscape) => A::PortraitMode(landscape),
        C::PenUp => A::PenUp,
        C::Origin => A::Origin,
        C::Home => A::Home,
        C::MediaSelect(id) => A::MediaSelect(id),
        C::PressureLegacy(p) => A::PressureLegacy(p),
        C::SpeedLegacy(s) => A::SpeedLegacy(s),
        C::CutterOffsetLegacy(d) => A::CutterOffsetLegacy(d),
        C::Raw(text) => A::Raw(text),
    }
}

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
