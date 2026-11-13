//! The desktop app's control channel: JSON lines on `127.0.0.1:PORT`, one line
//! per request, one line per response — the same session core the CLI's `mcp`
//! mode uses (`crates/mcp`). Agents drive the running app without window tricks.
//!
//! Requests are not executed on the socket thread: each one is queued for the UI
//! thread, which runs it against the engine backing the visible canvas and hands
//! the response back. That way an agent edits the document the user is looking at,
//! undo history and all, instead of a private copy.
//!
//! With the screen locked or the window hidden no frames run, so the socket thread
//! waits at most [`REQUEST_TIMEOUT`] and then answers with [`NO_FRAMES_ERROR`];
//! `app.screenshot` instead falls back to rendering the last document the window
//! published ([`Jobs::publish`]), which needs no presented frame.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use stencilcraft_doc::Document;

/// How long the socket thread waits for the UI thread before giving up.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// The error a request gets when the UI thread does not pick it up in time.
pub const NO_FRAMES_ERROR: &str = "the window is not drawing frames (screen locked, minimized or covered); try again when it is visible";

/// Resolution of the headless `app.screenshot` fallback render.
const FALLBACK_DPI: f64 = 96.0;

/// One control request waiting for the UI thread, with the channel to answer on.
pub struct Job {
    pub line: String,
    pub reply: Sender<String>,
    /// After this the socket thread has already answered with a timeout.
    deadline: Instant,
}

/// The receiving half of the control queue, drained once per frame by the app.
pub struct Jobs {
    rx: Receiver<Job>,
    /// The port actually bound (useful when the caller asked for `0`).
    port: u16,
    /// The document as of the last frame, for the headless screenshot fallback.
    snapshot: Arc<Mutex<Document>>,
}

impl Jobs {
    /// Next queued request, if any. Never blocks: the UI thread has a window to
    /// keep responsive. Requests the socket thread already timed out are dropped,
    /// so a late edit is not applied after the agent was told it failed.
    pub fn try_recv(&self) -> Option<Job> {
        loop {
            let job = self.rx.try_recv().ok()?;
            if Instant::now() < job.deadline {
                return Some(job);
            }
        }
    }

    /// Record the document on screen for the screenshot fallback (only clones
    /// when it changed since the last call).
    pub fn publish(&self, doc: &Document) {
        if let Ok(mut snapshot) = self.snapshot.lock()
            && *snapshot != *doc
        {
            *snapshot = doc.clone();
        }
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
    serve_with_timeout(port, REQUEST_TIMEOUT, notify)
}

/// [`serve`] with a chosen wait for the UI thread (tests use a short one).
fn serve_with_timeout(
    port: u16,
    timeout: Duration,
    notify: impl Fn() + Send + Sync + 'static,
) -> Option<Jobs> {
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
    let snapshot = Arc::new(Mutex::new(Document::default()));
    let shared = Arc::clone(&snapshot);
    std::thread::spawn(move || {
        for incoming in listener.incoming() {
            match incoming {
                Ok(stream) => handle_connection(
                    stream,
                    tx.clone(),
                    Arc::clone(&notify),
                    Arc::clone(&shared),
                    timeout,
                ),
                Err(e) => {
                    eprintln!("control: accept failed: {e}");
                    return;
                }
            }
        }
    });
    Some(Jobs { rx, port, snapshot })
}

fn handle_connection(
    stream: TcpStream,
    jobs: Sender<Job>,
    notify: Arc<dyn Fn() + Send + Sync>,
    snapshot: Arc<Mutex<Document>>,
    timeout: Duration,
) {
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
            let value = serde_json::from_str::<serde_json::Value>(&request).ok();
            if jobs
                .send(Job {
                    line: request,
                    reply,
                    deadline: Instant::now() + timeout,
                })
                .is_err()
            {
                eprintln!("control: the app is no longer accepting requests");
                return;
            }
            notify();
            // `answered` is dropped with this iteration, so a late answer is discarded.
            let response = match answered.recv_timeout(timeout) {
                Ok(response) => response,
                Err(mpsc::RecvTimeoutError::Timeout) => no_frame_answer(value.as_ref(), &snapshot),
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            };
            if let Err(e) = writeln!(out, "{response}") {
                eprintln!("control: write failed: {e}");
                return;
            }
        }
    });
}

/// The answer to a request the UI thread did not pick up in time: `app.screenshot`
/// renders the last published document headlessly, everything else gets
/// [`NO_FRAMES_ERROR`].
fn no_frame_answer(request: Option<&serde_json::Value>, snapshot: &Mutex<Document>) -> String {
    let id = request
        .and_then(|v| v.get("id"))
        .cloned()
        .unwrap_or_default();
    let path = request
        .filter(|v| v.get("method").and_then(|m| m.as_str()) == Some("app.screenshot"))
        .and_then(|v| v.pointer("/params/path"))
        .and_then(|p| p.as_str());
    let Some(path) = path else {
        return serde_json::json!({"id": id, "ok": false, "error": NO_FRAMES_ERROR}).to_string();
    };
    let doc = match snapshot.lock() {
        Ok(doc) => doc.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    let rendered = stencilcraft_import::render(&doc, FALLBACK_DPI)
        .map_err(|e| e.to_string())
        .and_then(|img| {
            let size = [img.width(), img.height()];
            craft_raster_codecs::common::encode_for_path(path, &img)
                .map_err(|e| format!("screenshot: {path}: {e}"))
                .and_then(|bytes| {
                    std::fs::write(path, bytes).map_err(|e| format!("screenshot: {path}: {e}"))
                })
                .map(|()| size)
        });
    match rendered {
        Ok(size) => serde_json::json!({"id": id, "ok": true, "result": {
            "path": path, "size": size, "source": "render", "dpi": FALLBACK_DPI,
        }}),
        Err(e) => serde_json::json!({"id": id, "ok": false,
            "error": format!("{NO_FRAMES_ERROR}; headless render failed: {e}")}),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(port: u16, request: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        writeln!(stream, "{request}").expect("write");
        let mut response = String::new();
        BufReader::new(stream)
            .read_line(&mut response)
            .expect("read");
        response
    }

    #[test]
    fn unanswered_requests_time_out_with_their_id() {
        let jobs = serve_with_timeout(0, Duration::from_millis(200), || {}).expect("bind");
        let started = Instant::now();
        let response = ask(jobs.port(), r#"{"id":"q7","method":"doc.summary"}"#);
        let waited = started.elapsed();
        assert!(waited >= Duration::from_millis(200), "{waited:?}");
        assert!(waited < Duration::from_secs(5), "{waited:?}");
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["id"], "q7");
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"], NO_FRAMES_ERROR);
        // The stale request is not handed to a UI thread that wakes up late.
        assert!(jobs.try_recv().is_none());
    }

    #[test]
    fn screenshots_fall_back_to_a_headless_render() {
        let jobs = serve_with_timeout(0, Duration::from_millis(200), || {}).expect("bind");
        let mut doc = Document::default();
        doc.board.width_mm = 25.4;
        doc.board.height_mm = 50.8;
        jobs.publish(&doc);
        let dir = std::env::temp_dir().join(format!("stencilcraft-ctl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("shot.png");
        let request =
            serde_json::json!({"id": 3, "method": "app.screenshot", "params": {"path": path}});
        let response = ask(jobs.port(), &request.to_string());
        let value: serde_json::Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["ok"], true, "{response}");
        assert_eq!(value["id"], 3);
        assert_eq!(value["result"]["source"], "render");
        let bytes = std::fs::read(&path).expect("png written");
        let img = craft_raster_codecs::common::decode_rgba(
            &bytes,
            craft_raster_codecs::common::DecodeLimits::default(),
        )
        .expect("png decodes");
        assert_eq!((img.width(), img.height()), (96, 192));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn requests_reach_the_queue_and_answers_come_back() {
        let jobs = serve(0, || {}).expect("bind an ephemeral port");
        let port = jobs.port();
        let client = std::thread::spawn(move || ask(port, r#"{"id":1}"#));

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
