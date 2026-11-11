//! `MachineDriver` = Capabilities + Protocol + Transport. The only path from
//! a `CutJob` to hardware, and it validates before any byte goes out.

use crate::protocol::{MachineStatus, Protocol, SendReport, SessionHooks, protocol_for};
use crate::{
    Capabilities, CutJob, CutProgram, MachineError, Transport, apply_job_settings, validate,
};

/// Validate, apply the job settings (order, overcut, mirror), validate the
/// result again (a mirrored design must still fit) and encode, without a
/// transport: the dry run.
pub fn dry_run(caps: &Capabilities, job: &CutJob) -> Result<CutProgram, MachineError> {
    validate(caps, job)?;
    let job = apply_job_settings(caps, job)?;
    validate(caps, &job)?;
    protocol_for(caps.protocol).encode(caps, &job)
}

pub struct MachineDriver {
    pub caps: Capabilities,
    protocol: Box<dyn Protocol>,
    transport: Box<dyn Transport>,
}

impl MachineDriver {
    /// Bind a machine to an open transport. Nothing is sent.
    pub fn new(caps: Capabilities, transport: Box<dyn Transport>) -> MachineDriver {
        let protocol = protocol_for(caps.protocol);
        MachineDriver {
            caps,
            protocol,
            transport,
        }
    }

    /// Validate, encode and encode only (no transport traffic).
    pub fn dry_run(&self, job: &CutJob) -> Result<CutProgram, MachineError> {
        dry_run(&self.caps, job)
    }

    /// The deliberate hardware step: validate against the capabilities, then
    /// run the protocol's send session over the transport.
    pub fn send(
        &mut self,
        job: &CutJob,
        hooks: &mut dyn SessionHooks,
    ) -> Result<SendReport, MachineError> {
        let program = self.dry_run(job)?;
        self.protocol
            .send(self.transport.as_mut(), &self.caps, &program, hooks)
    }

    pub fn status(&mut self) -> Result<MachineStatus, MachineError> {
        self.protocol.status(self.transport.as_mut())
    }

    pub fn pause(&mut self) -> Result<(), MachineError> {
        self.protocol.pause(self.transport.as_mut())
    }

    pub fn resume(&mut self) -> Result<(), MachineError> {
        self.protocol.resume(self.transport.as_mut())
    }

    pub fn cancel(&mut self) -> Result<(), MachineError> {
        self.protocol.cancel(self.transport.as_mut())
    }

    pub fn close(&mut self) -> Result<(), MachineError> {
        self.transport.close()
    }

    pub fn describe(&self) -> String {
        format!("{} via {}", self.caps.id, self.transport.describe())
    }
}
