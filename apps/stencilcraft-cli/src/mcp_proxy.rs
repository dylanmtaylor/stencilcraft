//! MCP stdio server that drives the running desktop app.
//!
//! MCP clients speak to a child process over stdin/stdout, so a desktop window can
//! never be the server itself. With `--control PORT` this mode relays: every line
//! from the client goes to the app's control socket (which runs it against the open
//! document) and the reply comes back on stdout. Without it, `mcp` keeps its own
//! headless session.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

/// Relay stdin/stdout to the desktop control socket on `127.0.0.1:port`.
pub fn run(port: u16) -> Result<(), String> {
    use std::io::BufWriter;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| format!("mcp: read: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let wants_response = expects_response(&line);
        match forward(port, &line, wants_response) {
            Ok(Some(response)) => {
                writeln!(out, "{response}").map_err(|e| format!("mcp: write: {e}"))?;
                out.flush().map_err(|e| format!("mcp: flush: {e}"))?;
            }
            Ok(None) => {}
            Err(e) => {
                // A client that asked a question must still get an answer: report the
                // broken link as a protocol error instead of hanging.
                if wants_response {
                    writeln!(out, "{}", error_line(&line, &e))
                        .map_err(|write| format!("mcp: write: {write}"))?;
                    out.flush()
                        .map_err(|write| format!("mcp: flush: {write}"))?;
                }
            }
        }
    }
    Ok(())
}

/// Send one request and, when the client expects an answer, read one reply.
///
/// Each request gets its own connection: the app keeps one session per connection
/// and a proxy restart should not leave half-open state behind.
fn forward(port: u16, line: &str, want_response: bool) -> Result<Option<String>, String> {
    let stream = TcpStream::connect(("127.0.0.1", port)).map_err(|e| {
        format!(
            "desktop control socket 127.0.0.1:{port} is not reachable: {e} (start the app with --control {port})"
        )
    })?;
    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("desktop control socket: {e}"))?;
    writer
        .write_all(format!("{line}\n").as_bytes())
        .map_err(|e| format!("desktop control socket: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("desktop control socket: {e}"))?;
    if !want_response {
        return Ok(None);
    }
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .map_err(|e| format!("desktop control socket: {e}"))?;
    if response.trim().is_empty() {
        return Err("the app closed the control connection without answering".to_string());
    }
    Ok(Some(response.trim_end().to_string()))
}

/// Does this line need a reply? JSON-RPC requests carry an `id`; notifications do
/// not. Anything unparseable gets an answer so the client is never left waiting.
fn expects_response(line: &str) -> bool {
    match serde_json::from_str::<serde_json::Value>(line) {
        Ok(serde_json::Value::Array(items)) => items.iter().any(|item| item.get("id").is_some()),
        Ok(serde_json::Value::Object(map)) => map.contains_key("id"),
        _ => true,
    }
}

/// Build the protocol-appropriate error for a request that could not be forwarded.
fn error_line(request: &str, message: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(request).unwrap_or(serde_json::Value::Null);
    let id = value.get("id").cloned().unwrap_or(serde_json::Value::Null);
    if value.get("jsonrpc").is_some() {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32000, "message": message },
        })
        .to_string()
    } else {
        serde_json::json!({ "id": id, "ok": false, "error": message }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_need_answers_and_notifications_do_not() {
        assert!(expects_response(
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#
        ));
        assert!(expects_response(
            r#"{"jsonrpc":"2.0","id":"abc","method":"tools/list"}"#
        ));
        assert!(!expects_response(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
        ));
        assert!(expects_response(r#"{"id":3,"method":"doc.summary"}"#));
        assert!(
            expects_response("not json"),
            "unparseable input still answers"
        );
        assert!(expects_response(
            r#"[{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":9,"method":"ping"}]"#
        ));
    }

    #[test]
    fn errors_match_the_envelope_style() {
        let rpc = error_line(r#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#, "down");
        let value: serde_json::Value = serde_json::from_str(&rpc).unwrap();
        assert_eq!(value["id"], 4);
        assert_eq!(value["error"]["code"], -32000);
        assert_eq!(value["error"]["message"], "down");

        let legacy = error_line(r#"{"id":7,"method":"doc.summary"}"#, "down");
        let value: serde_json::Value = serde_json::from_str(&legacy).unwrap();
        assert_eq!(value["id"], 7);
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"], "down");
    }
}
