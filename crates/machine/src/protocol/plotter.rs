//! Generic plotter protocols for cutters outside the Silhouette family:
//! HP-GL (the HP-GL/2 subset `IN SP PU PD`: Roland, USCutter, Vinyl Express,
//! many Chinese plotters, Graphtec in HP-GL mode), plain GP-GL, and DMPL.
//! The encoders are `craft-plotter`'s; none of these has a session beyond
//! streaming the program.

use super::{Protocol, refused};
use crate::{Capabilities, CutJob, CutProgram, MachineError, ProtocolId};

pub struct HpglProtocol;

impl Protocol for HpglProtocol {
    fn id(&self) -> ProtocolId {
        ProtocolId::Hpgl
    }

    fn encode(&self, _caps: &Capabilities, job: &CutJob) -> Result<CutProgram, MachineError> {
        craft_plotter::hpgl::encode(&job.neutral()).map_err(refused)
    }
}

pub struct DmplProtocol;

impl Protocol for DmplProtocol {
    fn id(&self) -> ProtocolId {
        ProtocolId::Dmpl
    }

    fn encode(&self, _caps: &Capabilities, job: &CutJob) -> Result<CutProgram, MachineError> {
        craft_plotter::dmpl::encode(&job.neutral()).map_err(refused)
    }
}

/// Plain GP-GL: same axis order as the Silhouette dialect (down, right).
pub struct GpglProtocol;

impl Protocol for GpglProtocol {
    fn id(&self) -> ProtocolId {
        ProtocolId::Gpgl
    }

    fn encode(&self, _caps: &Capabilities, job: &CutJob) -> Result<CutProgram, MachineError> {
        craft_plotter::gpgl::encode(&job.neutral()).map_err(refused)
    }
}
