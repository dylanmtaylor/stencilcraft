//! USB bulk transport for the Silhouette/Graphtec vendor family, via `nusb`
//! (MIT OR Apache-2.0, pure Rust). Behind the `usb` feature. Opening a device
//! never sends anything; writes happen only when a caller explicitly runs a job.

use std::io::{Read as _, Write as _};
use std::time::Duration;

use nusb::MaybeFuture as _;
use nusb::descriptors::TransferType;
use nusb::io::{EndpointRead, EndpointWrite};
use nusb::transfer::{Bulk, Direction, In, Out};

use super::{DEFAULT_TIMEOUT, Transport};
use crate::{MachineError, USB_VENDORS, UsbIds};

const BUFFER: usize = 4096;

fn err(context: &str) -> impl Fn(nusb::Error) -> MachineError + '_ {
    move |e| MachineError::Transport(format!("{context}: {e}"))
}

fn location(d: &nusb::DeviceInfo) -> String {
    format!("{}-{}", d.bus_id(), d.device_address())
}

/// Every USB device from the cutter vendor ids (catalogued or not).
pub fn list_usb_devices() -> Result<Vec<UsbIds>, MachineError> {
    let devices = nusb::list_devices()
        .wait()
        .map_err(err("list usb devices"))?;
    Ok(devices
        .filter(|d| USB_VENDORS.contains(&d.vendor_id()))
        .map(|d| UsbIds {
            vendor_id: d.vendor_id(),
            product_id: d.product_id(),
            location: location(&d),
            product: d.product_string().map(str::to_string),
        })
        .collect())
}

/// Bulk OUT/IN pipes on interface 0 of a cutter.
pub struct UsbTransport {
    writer: EndpointWrite<Bulk>,
    reader: EndpointRead<Bulk>,
    name: String,
}

impl UsbTransport {
    /// Open the device at `location` (`bus-address` from `list_usb_devices`),
    /// or the first cutter-vendor device when `None`.
    pub fn open(at: Option<&str>) -> Result<Self, MachineError> {
        let info = nusb::list_devices()
            .wait()
            .map_err(err("list usb devices"))?
            .filter(|d| USB_VENDORS.contains(&d.vendor_id()))
            .find(|d| at.is_none_or(|l| location(d) == l))
            .ok_or_else(|| {
                MachineError::Transport(match at {
                    Some(l) => format!("no cutter at usb {l}"),
                    None => "no cutter found on usb".to_string(),
                })
            })?;
        let name = format!(
            "usb:{} {:04x}:{:04x}",
            location(&info),
            info.vendor_id(),
            info.product_id()
        );
        let device = info.open().wait().map_err(err("open usb device"))?;
        // The kernel may bind a printer driver to these devices; detach it.
        let interface = device
            .detach_and_claim_interface(0)
            .wait()
            .map_err(err("claim usb interface 0"))?;
        let (mut out_ep, mut in_ep) = (None, None);
        if let Some(desc) = interface.descriptor() {
            for ep in desc.endpoints() {
                if ep.transfer_type() != TransferType::Bulk {
                    continue;
                }
                match ep.direction() {
                    Direction::Out => out_ep = out_ep.or(Some(ep.address())),
                    Direction::In => in_ep = in_ep.or(Some(ep.address())),
                }
            }
        }
        let (Some(out_addr), Some(in_addr)) = (out_ep, in_ep) else {
            return Err(MachineError::Transport(
                "usb interface 0 has no bulk in/out endpoint pair".to_string(),
            ));
        };
        let writer = interface
            .endpoint::<Bulk, Out>(out_addr)
            .map_err(err("open bulk out"))?
            .writer(BUFFER)
            .with_write_timeout(DEFAULT_TIMEOUT);
        let reader = interface
            .endpoint::<Bulk, In>(in_addr)
            .map_err(err("open bulk in"))?
            .reader(BUFFER)
            .with_read_timeout(DEFAULT_TIMEOUT);
        Ok(UsbTransport {
            writer,
            reader,
            name,
        })
    }
}

impl Transport for UsbTransport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError> {
        self.writer
            .write_all(data)
            .and_then(|()| self.writer.flush())
            .map(|()| data.len())
            .map_err(|e| MachineError::Transport(format!("usb write: {e}")))
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, MachineError> {
        match self.reader.read(buf) {
            Ok(n) => Ok(n),
            // No reply within the timeout reads as "nothing".
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(MachineError::Transport(format!("usb read: {e}"))),
        }
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, MachineError> {
        self.reader.set_read_timeout(timeout);
        let n = self.read(buf);
        self.reader.set_read_timeout(DEFAULT_TIMEOUT);
        n
    }

    fn describe(&self) -> String {
        self.name.clone()
    }
}
