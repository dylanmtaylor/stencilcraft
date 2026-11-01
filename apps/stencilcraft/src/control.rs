//! The desktop app's control channel: JSON lines on `127.0.0.1:PORT`, one line
//! per request, one line per response — the same session core the CLI's `mcp`
//! mode uses (`crates/mcp`). Agents drive the running app without window tricks.
//!
//! Requests are not executed on the socket thread: each one is queued for the UI
//! thread, which runs it against the engine backing the visible canvas and hands
//! the response back. That way an agent edits the document the user is looking at,
//! undo history and all, instead of a private copy.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

/// One control request waiting for the UI thread, with the channel to answer on.
pub struct Job {
    pub line: String,
    pub reply: Sender<String>,
}

/// The receiving half of the control queue, drained once per frame by the app.
pub struct Jobs {
    rx: Receiver<Job>,
    /// The port actually bound (useful when the caller asked for `0`).
    port: u16,
}

impl Jobs {
    /// Next queued request, if any. Never blocks: the UI thread has a window to
    /// keep responsive.
    pub fn try_recv(&self) -> Option<Job> {
        self.rx.try_recv().ok()
    }

    /// The listening port.
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Serve the control port in background threads for the life of the process.
/// Failures to bind are reported to stderr (the app keeps running; agents read
/// the message and pick another port, matching the Crafting Apps convention).
///
/// `notify` is called whenever a request is queued, so an idle app wakes up and
/// applies it.
pub fn serve(port: u16, notify: impl Fn() + Send + Sync + 'static) -> Option<Jobs> {
    let address = format!("127.0.0.1:{port}");
    let listener = match TcpListener::bind(&address) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("control: could not bind {address}: {e} (pick another port)");
            return None;
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    eprintln!("control: listening on 127.0.0.1:{port}");
    let (tx, rx) = mpsc::channel();
    let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(notify);
    std::thread::spawn(move || {
        for incoming in listener.incoming() {
            match incoming {
                Ok(stream) => handle_connection(stream, tx.clone(), Arc::clone(&notify)),
                Err(e) => {
                    eprintln!("control: accept failed: {e}");
                    return;
                }
            }
        }
    });
    Some(Jobs { rx, port })
}

fn handle_connection(stream: TcpStream, jobs: Sender<Job>, notify: Arc<dyn Fn() + Send + Sync>) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("control: clone failed: {e}");
                return;
            }
        });
        let mut out = stream;
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
            if line.trim().is_empty() {
                continue;
            }
            let (reply, answered) = mpsc::channel();
            let request = std::mem::take(&mut line);
            if jobs
                .send(Job {
                    line: request,
                    reply,
                })
                .is_err()
            {
                eprintln!("control: the app is no longer accepting requests");
                return;
            }
            notify();
            match answered.recv() {
                Ok(response) => {
                    if let Err(e) = writeln!(out, "{response}") {
                        eprintln!("control: write failed: {e}");
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn requests_reach_the_queue_and_answers_come_back() {
        let jobs = serve(0, || {}).expect("bind an ephemeral port");
        let port = jobs.port();
        let client = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
            stream.write_all(b"{\"id\":1}\n").expect("write");
            let mut response = String::new();
            BufReader::new(stream)
                .read_line(&mut response)
                .expect("read");
            response
        });

        // Play the UI thread: pick the request up off the queue and answer it.
        let deadline = Instant::now() + Duration::from_secs(5);
        let job = loop {
            if let Some(job) = jobs.try_recv() {
                break job;
            }
            assert!(Instant::now() < deadline, "no request reached the queue");
            std::thread::sleep(Duration::from_millis(5));
        };
        let received = job.line.clone();
        job.reply
            .send(r#"{"id":1,"ok":true}"#.to_string())
            .expect("send the answer");

        assert!(received.contains("\"id\""), "request line: {received}");
        let response = client.join().expect("client thread");
        assert!(response.contains("\"ok\":true"), "response: {response}");
    }
}
