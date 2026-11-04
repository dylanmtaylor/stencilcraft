//! Linux BlueZ BLE transport for the Silhouette cutter GATT service.
//!
//! Characteristic roles are documented in the public discussion at
//! https://github.com/fablabnbg/inkscape-silhouette/issues/88 and were checked
//! against the connected Cameo 4 with a non-cutting firmware query.
use super::{DEFAULT_TIMEOUT, Transport};
use crate::{DeviceRegistry, MachineError};
use bluer::gatt::{
    WriteOp,
    remote::{Characteristic, CharacteristicWriteRequest},
};
use futures_util::{StreamExt, stream::BoxStream};
use std::{collections::VecDeque, time::Duration};

const SERVICE: &str = "e2088282-4fde-42f9-bb22-6ec3c7ed8f91";
const WRITE: &str = "6d92661d-f429-4d67-929b-28e7a9780912";
const INDICATE: &str = "8dcf199a-30e7-4bd4-beb6-beb57dca866c";
const PACKET: usize = 20; // fits the minimum BLE ATT MTU, including protocol overhead
const MAX_REPLY: usize = 65_536;

fn error(context: &str, err: impl std::fmt::Display) -> MachineError {
    MachineError::Transport(format!("Bluetooth LE {context}: {err}"))
}
fn runtime() -> Result<tokio::runtime::Runtime, MachineError> {
    super::ensure_blocking_context()?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| error("runtime", e))
}

/// Paired BlueZ devices whose names match a known Bluetooth-capable cutter.
/// Returns (address, model id, display name); no cutter commands are sent.
pub fn list_ble_devices() -> Result<Vec<(String, String, String)>, MachineError> {
    let rt = runtime()?;
    rt.block_on(async {
        let session = bluer::Session::new()
            .await
            .map_err(|e| error("session", e))?;
        let adapter = session
            .default_adapter()
            .await
            .map_err(|e| error("adapter", e))?;
        let addresses = adapter
            .device_addresses()
            .await
            .map_err(|e| error("discovery", e))?;
        let mut found = Vec::new();
        for address in addresses {
            let Ok(device) = adapter.device(address) else {
                continue;
            };
            if !device.is_paired().await.unwrap_or(false) {
                continue;
            }
            let has_service = device
                .uuids()
                .await
                .ok()
                .flatten()
                .is_some_and(|uuids| uuids.iter().any(|uuid| uuid.to_string() == SERVICE));
            if !has_service {
                continue;
            }
            let Some(name) = device.name().await.ok().flatten() else {
                continue;
            };
            let Some(caps) = DeviceRegistry::builtin().by_firmware(&name) else {
                continue;
            };
            if caps.bluetooth == Some(true) {
                found.push((address.to_string(), caps.id.clone(), name));
            }
        }
        Ok(found)
    })
}

/// A BLE command stream, constructed only after locating the verified GATT service.
pub struct BleTransport {
    runtime: Option<tokio::runtime::Runtime>,
    _session: bluer::Session,
    writer: Characteristic,
    notifications: BoxStream<'static, Vec<u8>>,
    pending: VecDeque<u8>,
    name: String,
}

impl BleTransport {
    /// Open a paired device by its BLE address and subscribe before writing.
    pub fn open(address: &str) -> Result<Self, MachineError> {
        let address = address
            .parse::<bluer::Address>()
            .map_err(|e| error("address", e))?;
        let runtime = runtime()?;
        let (session, writer, notifications) = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(20), async {
                let session = bluer::Session::new()
                    .await
                    .map_err(|e| error("session", e))?;
                let adapter = session
                    .default_adapter()
                    .await
                    .map_err(|e| error("adapter", e))?;
                let device = adapter.device(address).map_err(|e| error("device", e))?;
                if !device.is_paired().await.map_err(|e| error("pairing", e))? {
                    return Err(MachineError::Transport(
                        "Bluetooth LE device must be paired in system settings".into(),
                    ));
                }
                if !device
                    .is_connected()
                    .await
                    .map_err(|e| error("connection", e))?
                {
                    device.connect().await.map_err(|e| error("connect", e))?;
                }
                let mut writer = None;
                let mut reader = None;
                for service in device.services().await.map_err(|e| error("services", e))? {
                    if service
                        .uuid()
                        .await
                        .map_err(|e| error("service UUID", e))?
                        .to_string()
                        != SERVICE
                    {
                        continue;
                    }
                    for ch in service
                        .characteristics()
                        .await
                        .map_err(|e| error("characteristics", e))?
                    {
                        let id = ch
                            .uuid()
                            .await
                            .map_err(|e| error("characteristic UUID", e))?
                            .to_string();
                        if id == WRITE {
                            writer = Some(ch);
                        } else if id == INDICATE {
                            reader = Some(ch);
                        }
                    }
                }
                let writer = writer.ok_or_else(|| {
                    MachineError::Transport(
                        "Bluetooth LE cutter write characteristic not found".into(),
                    )
                })?;
                let reader = reader.ok_or_else(|| {
                    MachineError::Transport(
                        "Bluetooth LE cutter response characteristic not found".into(),
                    )
                })?;
                let notifications = reader
                    .notify()
                    .await
                    .map_err(|e| error("subscribe", e))?
                    .boxed();
                Ok((session, writer, notifications))
            })
            .await
            .map_err(|_| MachineError::Transport("Bluetooth LE connection timed out".into()))?
        })?;
        Ok(Self {
            runtime: Some(runtime),
            _session: session,
            writer,
            notifications,
            pending: VecDeque::new(),
            name: format!("Bluetooth LE {address}"),
        })
    }
}
impl Drop for BleTransport {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

impl Transport for BleTransport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError> {
        super::ensure_blocking_context()?;
        let req = CharacteristicWriteRequest {
            op_type: WriteOp::Request,
            ..Default::default()
        };
        for chunk in data.chunks(PACKET) {
            self.runtime.as_ref().ok_or_else(|| MachineError::Transport("Bluetooth runtime is closed".into()))?.block_on(async { tokio::time::timeout(DEFAULT_TIMEOUT, self.writer.write_ext(chunk, &req)).await })
                .map_err(|_| MachineError::Transport("Bluetooth LE write timed out; data may have been partially sent. Check the cutter before retrying.".into()))?
                .map_err(|e| error("write; data may have been partially sent", e))?;
        }
        Ok(data.len())
    }
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, MachineError> {
        self.read_timeout(buf, DEFAULT_TIMEOUT)
    }
    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, MachineError> {
        super::ensure_blocking_context()?;
        if self.pending.is_empty() && !buf.is_empty() {
            match self
                .runtime
                .as_ref()
                .ok_or_else(|| MachineError::Transport("Bluetooth runtime is closed".into()))?
                .block_on(async { tokio::time::timeout(timeout, self.notifications.next()).await })
            {
                Ok(Some(bytes)) if bytes.len() <= MAX_REPLY => self.pending.extend(bytes),
                Ok(Some(_)) => {
                    return Err(MachineError::Transport(
                        "Bluetooth LE reply too large".into(),
                    ));
                }
                Ok(None) => {
                    return Err(MachineError::Transport(
                        "Bluetooth LE notification stream closed".into(),
                    ));
                }
                Err(_) => return Ok(0),
            }
        }
        let n = buf.len().min(self.pending.len());
        for byte in buf.iter_mut().take(n) {
            if let Some(value) = self.pending.pop_front() {
                *byte = value;
            }
        }
        Ok(n)
    }
    fn describe(&self) -> String {
        self.name.clone()
    }
}
