//! Bluetooth Classic RFCOMM transport on Linux. Pair with the OS first.
use super::{DEFAULT_TIMEOUT, Transport};
use crate::MachineError;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// An RFCOMM stream to a paired device. Opening does not send cutter commands.
pub struct BluetoothTransport {
    runtime: Option<tokio::runtime::Runtime>,
    stream: bluer::rfcomm::Stream,
    name: String,
}

impl BluetoothTransport {
    /// Connect to a Bluetooth address and explicit RFCOMM service channel.
    pub fn open(address: &str, channel: u8) -> Result<Self, MachineError> {
        super::ensure_blocking_context()?;
        if !(1..=30).contains(&channel) {
            return Err(MachineError::Transport(
                "RFCOMM channel must be 1–30".into(),
            ));
        }
        let addr = address
            .parse::<bluer::Address>()
            .map_err(|e| MachineError::Transport(format!("invalid Bluetooth address: {e}")))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| MachineError::Transport(format!("Bluetooth runtime: {e}")))?;
        let stream = runtime
            .block_on(async {
                tokio::time::timeout(
                    Duration::from_secs(15),
                    bluer::rfcomm::Stream::connect(bluer::rfcomm::SocketAddr::new(addr, channel)),
                )
                .await
            })
            .map_err(|_| MachineError::Transport("Bluetooth connection timed out".into()))?
            .map_err(|e| MachineError::Transport(format!("Bluetooth connection: {e}")))?;
        Ok(Self {
            runtime: Some(runtime),
            stream,
            name: format!("Bluetooth {address} channel {channel}"),
        })
    }
}

impl Drop for BluetoothTransport {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

impl Transport for BluetoothTransport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError> {
        super::ensure_blocking_context()?;
        self.runtime.as_ref().ok_or_else(|| MachineError::Transport("Bluetooth runtime is closed".into()))?
            .block_on(async {
                tokio::time::timeout(DEFAULT_TIMEOUT, self.stream.write_all(data)).await
            })
            .map_err(|_| MachineError::Transport("Bluetooth write timed out; data may have been partially sent. Check the cutter before retrying.".into()))?
            .map_err(|e| MachineError::Transport(format!("Bluetooth write: {e}")))?;
        Ok(data.len())
    }
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, MachineError> {
        self.read_timeout(buf, DEFAULT_TIMEOUT)
    }
    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, MachineError> {
        super::ensure_blocking_context()?;
        match self
            .runtime
            .as_ref()
            .ok_or_else(|| MachineError::Transport("Bluetooth runtime is closed".into()))?
            .block_on(async { tokio::time::timeout(timeout, self.stream.read(buf)).await })
        {
            Ok(Ok(n)) => Ok(n),
            Ok(Err(e)) => Err(MachineError::Transport(format!("Bluetooth read: {e}"))),
            Err(_) => Ok(0),
        }
    }
    fn describe(&self) -> String {
        self.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_endpoints_before_connecting() {
        assert!(BluetoothTransport::open("not-an-address", 1).is_err());
        assert!(BluetoothTransport::open("00:11:22:33:44:55", 0).is_err());
        assert!(BluetoothTransport::open("00:11:22:33:44:55", 31).is_err());
    }
}
