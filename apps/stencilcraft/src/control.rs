//! The desktop app's control channel: JSON lines on `127.0.0.1:PORT`, one line
//! per request, one line per response — the same session core the CLI's `mcp`
//! mode uses (`crates/mcp`). Agents drive the running app without window tricks.
//!
//! Sessions are per-connection. A thread per connection is plenty for a control
//! channel; heavy work happens on the engine without a window dependency.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

/// Serve the control port in background threads for the life of the process.
/// Failures to bind are reported to stderr (the app keeps running; agents read
/// the message and pick another port, matching the Crafting Apps convention).
pub fn serve(port: u16) {
    let address = format!("127.0.0.1:{port}");
    let listener = match TcpListener::bind(&address) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("control: could not bind {address}: {e} (pick another port)");
            return;
        }
    };
    eprintln!("control: listening on {address}");
    std::thread::spawn(move || {
        for incoming in listener.incoming() {
            match incoming {
                Ok(stream) => handle_connection(stream),
                Err(e) => {
                    eprintln!("control: accept failed: {e}");
                    return;
                }
            }
        }
    });
}

fn handle_connection(stream: TcpStream) {
    let peer = stream.peer_addr().ok();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("control: clone failed: {e}");
                return;
            }
        });
        let mut out = stream;
        let mut session = stencilcraft_mcp::Session::default();
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => return, // closed
                Ok(_) => {}
                Err(e) => {
                    eprintln!("control: read failed: {e}");
                    return;
                }
            }
            match session.handle_line(&line) {
                Ok(None) => continue, // blank line
                Ok(Some(response)) => {
                    if let Err(e) = writeln!(out, "{response}") {
                        eprintln!("control: write failed: {e}");
                        return;
                    }
                }
                Err(e) => {
                    eprintln!("control: {e}");
                    if let Err(write_err) = writeln!(
                        out,
                        r#"{{"id":null,"ok":false,"error":"internal control error"}}"#
                    ) {
                        eprintln!("control: write failed: {write_err}");
                        return;
                    }
                }
            }
        }
    });
    let _ = peer;
}
