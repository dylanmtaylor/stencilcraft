//! Stencil job adapters for shared GP-GL, HP-GL and DMPL command encoders.

use super::{Protocol, from_plotter_program, to_plotter_job};
use crate::{Capabilities, CutJob, CutProgram, ProtocolId};

pub const HPGL_PER_MM: f64 = craft_plotter::hpgl::UNITS_PER_MM;
pub const GPGL_PER_MM: f64 = craft_plotter::gpgl::UNITS_PER_MM;
pub const DMPL_PER_MM: f64 = craft_plotter::dmpl::UNITS_PER_MM;

pub struct HpglProtocol;

impl Protocol for HpglProtocol {
    fn id(&self) -> ProtocolId {
        ProtocolId::Hpgl
    }

    fn encode(&self, _caps: &Capabilities, job: &CutJob) -> CutProgram {
        craft_plotter::hpgl::encode(&to_plotter_job(job))
            .map(from_plotter_program)
            .unwrap_or_default()
    }
}

pub struct DmplProtocol;

impl Protocol for DmplProtocol {
    fn id(&self) -> ProtocolId {
        ProtocolId::Dmpl
    }

    fn encode(&self, _caps: &Capabilities, job: &CutJob) -> CutProgram {
        craft_plotter::dmpl::encode(&to_plotter_job(job))
            .map(from_plotter_program)
            .unwrap_or_default()
    }
}

pub struct GpglProtocol;

impl Protocol for GpglProtocol {
    fn id(&self) -> ProtocolId {
        ProtocolId::Gpgl
    }

    fn encode(&self, _caps: &Capabilities, job: &CutJob) -> CutProgram {
        craft_plotter::gpgl::encode(&to_plotter_job(job))
            .map(from_plotter_program)
            .unwrap_or_default()
    }
}
