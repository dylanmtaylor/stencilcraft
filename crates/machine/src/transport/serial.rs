//! Serial / USB-CDC transport (behind the `serial` feature): tty/COM devices
//! — Grbl/Marlin boards, many sign plotters, Graphtec CE serial ports — and
//! Bluetooth cutters once OS pairing exposes an RFCOMM port.

use std::io::{Read as _, Write as _};
use std::time::Duration;

use super::{DEFAULT_TIMEOUT, Transport};
use crate::MachineError;

pub struct SerialTransport {
    port: Box<dyn serialport::SerialPort>,
    name: String,
}

impl SerialTransport {
    pub fn open(path: &str, baud: u32) -> Result<Self, MachineError> {
        serialport::new(path, baud)
            .timeout(DEFAULT_TIMEOUT)
            .open()
            .map(|port| SerialTransport {
                port,
                name: format!("serial:{path}@{baud}"),
            })
            .map_err(|e| MachineError::Transport(format!("open {path}: {e}")))
    }
}

impl Transport for SerialTransport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError> {
        self.port
            .write_all(data)
            .map(|()| data.len())
            .map_err(|e| MachineError::Transport(format!("{}: {e}", self.name)))
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, MachineError> {
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            // No reply within the timeout reads as "nothing".
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(MachineError::Transport(format!("{}: {e}", self.name))),
        }
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, MachineError> {
        self.port
            .set_timeout(timeout)
            .map_err(|e| MachineError::Transport(format!("{}: {e}", self.name)))?;
        let n = self.read(buf);
        let _ = self.port.set_timeout(DEFAULT_TIMEOUT);
        n
    }

    fn describe(&self) -> String {
        self.name.clone()
    }
}
