//! Byte pipes to machines. A `Transport` knows nothing about protocols: it
//! opens, writes, reads with a timeout, reports whether it is interactive
//! (can answer queries) and closes. `TransportSpec` names one from a string
//! (`usb`, `usb:1-4`, `serial:/dev/ttyUSB0@115200`, `tcp:host:9100`,
//! `file:job.plt`, `stdout`).
//!
//! Bluetooth is a documented future implementation (see
//! `docs/machine-architecture.md`); today an OS-paired RFCOMM port works
//! through the serial transport.

use std::collections::VecDeque;
use std::io::Write as _;
use std::time::Duration;

use crate::MachineError;

#[cfg(all(feature = "bluetooth", target_os = "linux"))]
mod ble;
#[cfg(all(feature = "bluetooth", target_os = "linux"))]
mod bluetooth;
mod net;
#[cfg(feature = "serial")]
mod serial;
#[cfg(all(feature = "usb", not(target_arch = "wasm32")))]
mod usb;

#[cfg(all(feature = "bluetooth", target_os = "linux"))]
pub use ble::{BleTransport, list_ble_devices};
#[cfg(all(feature = "bluetooth", target_os = "linux"))]
pub use bluetooth::BluetoothTransport;
pub use net::TcpTransport;
#[cfg(feature = "serial")]
pub use serial::SerialTransport;
#[cfg(all(feature = "usb", not(target_arch = "wasm32")))]
pub use usb::{UsbTransport, list_usb_devices};

#[cfg(all(feature = "bluetooth", target_os = "linux"))]
fn ensure_blocking_context() -> Result<(), crate::MachineError> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(crate::MachineError::Transport(
            "Bluetooth transport must run outside an async runtime".into(),
        ));
    }
    Ok(())
}

/// Default read timeout for interactive transports.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// A byte pipe to a machine. Never panics; everything returns.
pub trait Transport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError>;
    /// Read what is available within the transport's default timeout; `Ok(0)`
    /// means nothing arrived.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, MachineError>;
    /// Read with an explicit timeout (transports without a clock ignore it).
    fn read_timeout(&mut self, buf: &mut [u8], _timeout: Duration) -> Result<usize, MachineError> {
        self.read(buf)
    }
    /// Whether the other end can answer queries. Spool files and stdout
    /// cannot, so sessions skip the handshake and just stream.
    fn interactive(&self) -> bool {
        true
    }
    /// Whether the transport is still usable.
    fn is_open(&self) -> bool {
        true
    }
    fn close(&mut self) -> Result<(), MachineError> {
        Ok(())
    }
    /// Human-readable endpoint, for logs and reports.
    fn describe(&self) -> String {
        "transport".to_string()
    }
}

/// Dry-run transport: records every write exactly as it would be sent.
/// (Queries answer nothing; callers handle empty reads.)
#[derive(Debug, Clone, Default)]
pub struct RecordingTransport {
    pub bytes: Vec<u8>,
}

impl RecordingTransport {
    pub fn new() -> Self {
        RecordingTransport { bytes: Vec::new() }
    }

    /// Human-readable transcript: one command per line, ETX becomes newline.
    pub fn transcript(&self) -> String {
        String::from_utf8_lossy(&self.bytes).replace('\u{3}', "\n")
    }
}

impl Transport for RecordingTransport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError> {
        self.bytes.extend_from_slice(data);
        Ok(data.len())
    }

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, MachineError> {
        Ok(0)
    }

    fn interactive(&self) -> bool {
        false
    }

    fn describe(&self) -> String {
        "dry-run recording".to_string()
    }
}

/// Scripted transport for tests and simulations: records writes and answers
/// each read with the next canned reply (empty once the script runs out).
#[derive(Debug, Clone, Default)]
pub struct MockTransport {
    pub written: Vec<u8>,
    pub replies: VecDeque<Vec<u8>>,
    pub closed: bool,
}

impl MockTransport {
    pub fn with_replies(replies: &[&[u8]]) -> Self {
        MockTransport {
            replies: replies.iter().map(|r| r.to_vec()).collect(),
            ..MockTransport::default()
        }
    }
}

impl Transport for MockTransport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError> {
        if self.closed {
            return Err(MachineError::Transport("mock: closed".to_string()));
        }
        self.written.extend_from_slice(data);
        Ok(data.len())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, MachineError> {
        let Some(reply) = self.replies.pop_front() else {
            return Ok(0);
        };
        let n = reply.len().min(buf.len());
        if let (Some(dst), Some(src)) = (buf.get_mut(..n), reply.get(..n)) {
            dst.copy_from_slice(src);
        }
        Ok(n)
    }

    fn is_open(&self) -> bool {
        !self.closed
    }

    fn close(&mut self) -> Result<(), MachineError> {
        self.closed = true;
        Ok(())
    }

    fn describe(&self) -> String {
        "mock".to_string()
    }
}

/// Spool transport: the stream goes to a file (or stdout) for a print
/// spooler, `lp`, or a cutter that reads jobs from a share. Not interactive.
pub struct FileTransport {
    out: Box<dyn std::io::Write>,
    name: String,
}

impl FileTransport {
    pub fn create(path: &str) -> Result<Self, MachineError> {
        let file = std::fs::File::create(path)
            .map_err(|e| MachineError::Transport(format!("{path}: {e}")))?;
        Ok(FileTransport {
            out: Box::new(file),
            name: format!("file:{path}"),
        })
    }

    pub fn stdout() -> Self {
        FileTransport {
            out: Box::new(std::io::stdout()),
            name: "stdout".to_string(),
        }
    }
}

impl Transport for FileTransport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError> {
        self.out
            .write_all(data)
            .and_then(|()| self.out.flush())
            .map(|()| data.len())
            .map_err(|e| MachineError::Transport(format!("{}: {e}", self.name)))
    }

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, MachineError> {
        Ok(0)
    }

    fn interactive(&self) -> bool {
        false
    }

    fn close(&mut self) -> Result<(), MachineError> {
        self.out
            .flush()
            .map_err(|e| MachineError::Transport(format!("{}: {e}", self.name)))
    }

    fn describe(&self) -> String {
        self.name.clone()
    }
}

/// A transport named by a string; see the module docs for the forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportSpec {
    /// USB bulk; `None` = first cutter-vendor device, else `bus-address`.
    Usb(Option<String>),
    Serial {
        path: String,
        baud: u32,
    },
    Tcp(String),
    File(String),
    Stdout,
}

impl TransportSpec {
    /// Parse `usb[:LOC]`, `serial:PATH[@BAUD]`, `tcp:HOST:PORT`, `file:PATH`,
    /// `stdout`, or a bare device path (serial at 9600 baud).
    pub fn parse(s: &str) -> Result<TransportSpec, MachineError> {
        let s = s.trim();
        let bad = || {
            MachineError::Config(format!(
                "device `{s}`: expected usb[:BUS-ADDR], serial:PATH[@BAUD], tcp:HOST:PORT, file:PATH or stdout"
            ))
        };
        if s == "usb" {
            return Ok(TransportSpec::Usb(None));
        }
        if s == "stdout" || s == "-" {
            return Ok(TransportSpec::Stdout);
        }
        if let Some(loc) = s.strip_prefix("usb:") {
            return Ok(TransportSpec::Usb(Some(loc.to_string())));
        }
        if let Some(addr) = s.strip_prefix("tcp:") {
            return if addr.contains(':') {
                Ok(TransportSpec::Tcp(addr.to_string()))
            } else {
                Err(bad())
            };
        }
        if let Some(path) = s.strip_prefix("file:") {
            return if path.is_empty() {
                Err(bad())
            } else {
                Ok(TransportSpec::File(path.to_string()))
            };
        }
        let serial = s.strip_prefix("serial:").unwrap_or(s);
        if serial.is_empty() {
            return Err(bad());
        }
        let (path, baud) = match serial.rsplit_once('@') {
            Some((p, b)) => (p, b.parse::<u32>().map_err(|_| bad())?),
            None => (serial, 9600),
        };
        Ok(TransportSpec::Serial {
            path: path.to_string(),
            baud,
        })
    }

    /// Open it. Opening never sends anything.
    pub fn open(&self) -> Result<Box<dyn Transport>, MachineError> {
        match self {
            TransportSpec::Usb(loc) => open_usb(loc.as_deref()),
            TransportSpec::Serial { path, baud } => open_serial(path, *baud),
            TransportSpec::Tcp(addr) => Ok(Box::new(TcpTransport::connect(addr, DEFAULT_TIMEOUT)?)),
            TransportSpec::File(path) => Ok(Box::new(FileTransport::create(path)?)),
            TransportSpec::Stdout => Ok(Box::new(FileTransport::stdout())),
        }
    }
}

#[cfg(all(feature = "usb", not(target_arch = "wasm32")))]
fn open_usb(location: Option<&str>) -> Result<Box<dyn Transport>, MachineError> {
    Ok(Box::new(UsbTransport::open(location)?))
}

#[cfg(not(all(feature = "usb", not(target_arch = "wasm32"))))]
fn open_usb(_location: Option<&str>) -> Result<Box<dyn Transport>, MachineError> {
    Err(MachineError::Config(
        "USB support is not built in: rebuild with --features usb".to_string(),
    ))
}

#[cfg(feature = "serial")]
fn open_serial(path: &str, baud: u32) -> Result<Box<dyn Transport>, MachineError> {
    Ok(Box::new(SerialTransport::open(path, baud)?))
}

#[cfg(not(feature = "serial"))]
fn open_serial(_path: &str, _baud: u32) -> Result<Box<dyn Transport>, MachineError> {
    Err(MachineError::Config(
        "serial support is not built in: rebuild with --features serial".to_string(),
    ))
}
