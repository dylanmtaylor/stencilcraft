//! TCP transport: network cutters and plotters behind a raw socket (port
//! 9100-style print servers, serial-to-Ethernet bridges, Grbl over Wi-Fi).

use std::io::{Read as _, Write as _};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use super::Transport;
use crate::MachineError;

pub struct TcpTransport {
    stream: Option<TcpStream>,
    addr: String,
    timeout: Duration,
}

impl TcpTransport {
    pub fn connect(addr: &str, timeout: Duration) -> Result<Self, MachineError> {
        let err = |e: std::io::Error| MachineError::Transport(format!("tcp {addr}: {e}"));
        let target = addr
            .to_socket_addrs()
            .map_err(err)?
            .next()
            .ok_or_else(|| MachineError::Transport(format!("tcp {addr}: no address")))?;
        let stream = TcpStream::connect_timeout(&target, timeout).map_err(err)?;
        stream.set_read_timeout(Some(timeout)).map_err(err)?;
        stream.set_write_timeout(Some(timeout)).map_err(err)?;
        Ok(TcpTransport {
            stream: Some(stream),
            addr: addr.to_string(),
            timeout,
        })
    }

    fn stream(&mut self) -> Result<&mut TcpStream, MachineError> {
        self.stream
            .as_mut()
            .ok_or_else(|| MachineError::Transport(format!("tcp {}: closed", self.addr)))
    }
}

fn timed_out(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    )
}

impl Transport for TcpTransport {
    fn write(&mut self, data: &[u8]) -> Result<usize, MachineError> {
        let addr = self.addr.clone();
        self.stream()?
            .write_all(data)
            .map(|()| data.len())
            .map_err(|e| MachineError::Transport(format!("tcp {addr}: {e}")))
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, MachineError> {
        let timeout = self.timeout;
        self.read_timeout(buf, timeout)
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, MachineError> {
        let addr = self.addr.clone();
        let stream = self.stream()?;
        let err = |e: std::io::Error| MachineError::Transport(format!("tcp {addr}: {e}"));
        stream.set_read_timeout(Some(timeout)).map_err(err)?;
        match stream.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if timed_out(&e) => Ok(0),
            Err(e) => Err(err(e)),
        }
    }

    fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    fn close(&mut self) -> Result<(), MachineError> {
        if let Some(s) = self.stream.take() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        Ok(())
    }

    fn describe(&self) -> String {
        format!("tcp:{}", self.addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_round_trip_with_a_local_echo_peer() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let peer = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 3];
            s.read_exact(&mut buf).unwrap();
            s.write_all(b"0\x03").unwrap();
            buf
        });
        let mut t = TcpTransport::connect(&addr, Duration::from_secs(2)).unwrap();
        t.write(b"\x1b\x05\x03").unwrap();
        let mut buf = [0u8; 8];
        let n = t.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"0\x03");
        assert_eq!(peer.join().unwrap(), *b"\x1b\x05\x03");
        t.close().unwrap();
        assert!(!t.is_open());
        assert!(t.write(b"x").is_err());
    }
}
