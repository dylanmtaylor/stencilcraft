//! The JSON-lines control protocol / MCP session core.
//!
//! One request per line in, one response per line out. The CLI (`stencilcraft-cli
//! mcp`) and the desktop app's control channel both wrap a `Session`. A bad
//! parameter must never abort the session (`docs/mcp.md`): every request with a
//! parseable id gets exactly one response.

use stencilcraft_doc::FORMAT_NAME;
use stencilcraft_engine::{Command, Engine, EngineError};

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("mcp: {0}")]
    Bad(String),
    #[error("mcp: {0}")]
    Internal(String),
}

/// A session: one document, plus the engine and the helpers that act on it.
pub struct Session {
    pub engine: Engine,
    /// Where the last `doc.open` loaded from, so Save can default there.
    last_path: Option<String>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
            last_path: None,
        }
    }

    /// Handle an MCP stdio message, or a legacy control request for compatibility.
    pub fn handle_stdio_line(&mut self, line: &str) -> Result<Option<String>, ProtocolError> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => {
                return Ok(Some(
                    rpc_error(serde_json::Value::Null, -32700, "Parse error").to_string(),
                ));
            }
        };
        if value.is_object() && value.get("jsonrpc").is_none() {
            return self.handle_line(line);
        }
        let response = if let Some(batch) = value.as_array() {
            if batch.is_empty() {
                Some(rpc_error(
                    serde_json::Value::Null,
                    -32600,
                    "Invalid Request",
                ))
            } else {
                let responses: Vec<_> = batch
                    .iter()
                    .filter_map(|request| self.handle_rpc(request))
                    .collect();
                if responses.is_empty() {
                    None
                } else {
                    Some(serde_json::Value::Array(responses))
                }
            }
        } else {
            self.handle_rpc(&value)
        };
        Ok(response.map(|value| value.to_string()))
    }

    fn handle_rpc(&mut self, request: &serde_json::Value) -> Option<serde_json::Value> {
        let id = request.get("id").cloned();
        let valid_id = id
            .as_ref()
            .is_none_or(|id| id.is_string() || id.is_i64() || id.is_u64() || id.is_null());
        let method = request.get("method").and_then(serde_json::Value::as_str);
        if !request.is_object()
            || request.get("jsonrpc").and_then(serde_json::Value::as_str) != Some("2.0")
            || method.is_none()
            || !valid_id
        {
            return Some(rpc_error(
                serde_json::Value::Null,
                -32600,
                "Invalid Request",
            ));
        }
        // Notifications must not receive a response or invoke a tool.
        let id = id?;
        let method = method?;
        let params = request
            .get("params")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        if !params.is_object() {
            return Some(rpc_error(id, -32602, "Invalid params"));
        }
        let result = match method {
            "initialize" => {
                let version = params
                    .get("protocolVersion")
                    .and_then(serde_json::Value::as_str);
                if version.is_none()
                    || !params
                        .get("capabilities")
                        .is_some_and(serde_json::Value::is_object)
                    || !params
                        .get("clientInfo")
                        .is_some_and(serde_json::Value::is_object)
                {
                    return Some(rpc_error(id, -32602, "Invalid initialize parameters"));
                }
                let version = "2025-03-26";
                serde_json::json!({"protocolVersion": version, "capabilities": {"tools": {}}, "serverInfo": {"name": "stencilcraft", "version": env!("CARGO_PKG_VERSION")}})
            }
            "ping" => serde_json::json!({}),
            "tools/list" => serde_json::json!({"tools": mcp_tools()}),
            "tools/call" => {
                let Some(name) = params.get("name").and_then(serde_json::Value::as_str) else {
                    return Some(rpc_error(id, -32602, "Missing tool name"));
                };
                let Some(method) = tool_method(name) else {
                    return Some(rpc_error(id, -32602, "Unknown tool"));
                };
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                if !arguments.is_object() {
                    return Some(rpc_error(id, -32602, "Tool arguments must be an object"));
                }
                match self.dispatch(method, &arguments) {
                    Ok(value) => {
                        serde_json::json!({"content": [{"type": "text", "text": value.to_string()}], "isError": false})
                    }
                    Err(error) => {
                        serde_json::json!({"content": [{"type": "text", "text": error.to_string()}], "isError": true})
                    }
                }
            }
            _ => return Some(rpc_error(id, -32601, "Method not found")),
        };
        Some(serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }

    /// Interpret one request line and produce the response line, if the request
    /// carried a parseable envelope. Returns `Ok(None)` on blank lines.
    pub fn handle_line(&mut self, line: &str) -> Result<Option<String>, ProtocolError> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                // Malformed JSON cannot name an id reliably: respond with null.
                return Ok(Some(error_response(
                    None,
                    &format!("bad request envelope: {e}"),
                )));
            }
        };
        let id = value.get("id").cloned();
        let method = value.get("method").and_then(serde_json::Value::as_str);
        let Some(method) = method else {
            return Ok(Some(error_response(id, "missing `method`")));
        };
        let params = value
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        match self.dispatch(method, &params) {
            Ok(result) => Ok(Some(ok_response(id, result))),
            Err(e) => Ok(Some(error_response(id, &e.to_string()))),
        }
    }

    fn dispatch(
        &mut self,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, ProtocolError> {
        match method {
            "doc.open" => {
                let path = str_param(params, "path")?;
                let doc = stencilcraft_doc::load_from(path.as_str())
                    .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                let result = serde_json::json!({
                    "layers": doc.layers.len(),
                    "shapes": doc.shape_count(),
                });
                self.engine = Engine::new(doc);
                self.last_path = Some(path.to_string());
                Ok(result)
            }
            "doc.save" => {
                let path = str_param(params, "path")?;
                stencilcraft_doc::save_to(&self.engine.doc, path.as_str())
                    .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                self.last_path = Some(path.to_string());
                Ok(serde_json::json!({ "path": path }))
            }
            "doc.summary" => Ok(serde_json::json!({
                "format": FORMAT_NAME,
                "layers": self.engine.doc.layers.len(),
                "shapes": self.engine.doc.shape_count(),
                "kerf_mm": self.engine.doc.kerf.width_mm,
                "board": [
                    self.engine.doc.board.width_mm,
                    self.engine.doc.board.height_mm,
                ],
            })),
            "kerf.apply" => {
                let width_mm = f64_param(params, "width_mm")?;
                self.engine
                    .run(Command::KerfSet { width_mm })
                    .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                Ok(serde_json::json!({ "width_mm": width_mm }))
            }
            "export.svg" => {
                let path = str_param(params, "path")?;
                let text = stencilcraft_svg::export(&self.engine.doc)
                    .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                std::fs::write(&path, text)
                    .map_err(|e| ProtocolError::Bad(format!("{path}: {e}")))?;
                Ok(serde_json::json!({ "path": path }))
            }
            "export.dxf" => {
                let path = str_param(params, "path")?;
                let text = stencilcraft_dxf::export(&self.engine.doc)
                    .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                std::fs::write(&path, text)
                    .map_err(|e| ProtocolError::Bad(format!("{path}: {e}")))?;
                Ok(serde_json::json!({ "path": path }))
            }
            "export.gcode" => {
                let path = str_param(params, "path")?;
                let settings = stencilcraft_gcode::GcodeSettings::default();
                let text = stencilcraft_gcode::export(&self.engine.doc, &settings)
                    .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                std::fs::write(&path, text)
                    .map_err(|e| ProtocolError::Bad(format!("{path}: {e}")))?;
                Ok(serde_json::json!({ "path": path }))
            }
            "tile.run" => {
                let path = str_param(params, "path")?;
                let sheet_w = f64_param(params, "sheet_w")?;
                let sheet_h = f64_param(params, "sheet_h")?;
                let overlap = params
                    .get("overlap")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let pieces =
                    stencilcraft_tiling::tile_document(&self.engine.doc, sheet_w, sheet_h, overlap)
                        .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                let count = pieces.len();
                for (i, piece) in pieces.iter().enumerate() {
                    let tile_path = format!("{path}-{i:02}.svg");
                    let text = stencilcraft_svg::export(piece)
                        .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                    std::fs::write(&tile_path, text)
                        .map_err(|e| ProtocolError::Bad(format!("{tile_path}: {e}")))?;
                }
                Ok(serde_json::json!({ "tiles": count }))
            }
            "command.execute" => {
                let command: Command = serde_json::from_value(params.clone())
                    .map_err(|e| ProtocolError::Bad(format!("bad command parameters: {e}")))?;
                self.engine
                    .run(command)
                    .map_err(|e: EngineError| ProtocolError::Bad(e.to_string()))
                    .map(stencilcraft_effect_json)
            }
            other => Err(ProtocolError::Bad(format!("unknown method: {other}"))),
        }
    }
}

fn rpc_error(id: serde_json::Value, code: i32, message: &str) -> serde_json::Value {
    serde_json::json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn tool_method(name: &str) -> Option<&'static str> {
    match name {
        "doc_open" => Some("doc.open"),
        "doc_save" => Some("doc.save"),
        "doc_summary" => Some("doc.summary"),
        "kerf_apply" => Some("kerf.apply"),
        "export_svg" => Some("export.svg"),
        "export_dxf" => Some("export.dxf"),
        "export_gcode" => Some("export.gcode"),
        "tile_run" => Some("tile.run"),
        "command_execute" => Some("command.execute"),
        _ => None,
    }
}

fn mcp_tools() -> Vec<serde_json::Value> {
    use serde_json::json;
    let path = json!({"type": "string", "description": "File path on the server's filesystem"});
    let number = json!({"type": "number"});
    let specs = [
        (
            "doc_open",
            "Open a native StencilCraft document, replacing the current document.",
            json!({"path": path}),
            json!(["path"]),
        ),
        (
            "doc_save",
            "Save the current document to a native StencilCraft file.",
            json!({"path": path}),
            json!(["path"]),
        ),
        (
            "doc_summary",
            "Summarize the current document.",
            json!({}),
            json!([]),
        ),
        (
            "kerf_apply",
            "Set the kerf width in millimetres.",
            json!({"width_mm": number}),
            json!(["width_mm"]),
        ),
        (
            "export_svg",
            "Export the current document as SVG.",
            json!({"path": path}),
            json!(["path"]),
        ),
        (
            "export_dxf",
            "Export the current document as DXF.",
            json!({"path": path}),
            json!(["path"]),
        ),
        (
            "export_gcode",
            "Export the current document as G-code using default settings.",
            json!({"path": path}),
            json!(["path"]),
        ),
        (
            "tile_run",
            "Export tiled SVGs using a filename prefix and sheet dimensions in millimetres.",
            json!({"path": path, "sheet_w": number, "sheet_h": number, "overlap": number}),
            json!(["path", "sheet_w", "sheet_h"]),
        ),
        (
            "command_execute",
            "Execute an engine command. For example, command shape.add-rect uses params x, y, width, height in millimetres. See docs/mcp.md for the command format.",
            json!({"command": {"type": "string"}, "params": {"type": ["object", "null"]}}),
            json!(["command"]),
        ),
    ];
    specs
        .into_iter()
        .map(|(name, description, properties, required)| {
            json!({
                "name": name, "description": description,
                "inputSchema": {"type": "object", "properties": properties, "required": required}
            })
        })
        .collect()
}

fn stencilcraft_effect_json(effect: stencilcraft_engine::Effect) -> serde_json::Value {
    serde_json::to_value(effect).unwrap_or(serde_json::Value::Null)
}

fn str_param(params: &serde_json::Value, key: &str) -> Result<String, ProtocolError> {
    params
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ProtocolError::Bad(format!("missing parameter `{key}` (a string)")))
}

fn f64_param(params: &serde_json::Value, key: &str) -> Result<f64, ProtocolError> {
    params
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| ProtocolError::Bad(format!("missing parameter `{key}` (a number)")))
}

fn ok_response(id: Option<serde_json::Value>, result: serde_json::Value) -> String {
    serde_json::json!({ "id": id, "ok": true, "result": result }).to_string()
}

fn error_response(id: Option<serde_json::Value>, error: &str) -> String {
    serde_json::json!({ "id": id, "ok": false, "error": error }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_json() -> String {
        stencilcraft_testkit::simple_square().save_json().unwrap()
    }

    fn write_tmp(name: &str, content: &str) -> String {
        let dir = std::env::temp_dir().join("stencilcraft-mcp-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn json_rpc_errors_notifications_and_batches() {
        let mut session = Session::default();
        let parse: serde_json::Value =
            serde_json::from_str(&session.handle_stdio_line("{").unwrap().unwrap()).unwrap();
        assert_eq!(parse["error"]["code"], -32700);
        assert!(
            session
                .handle_stdio_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .unwrap()
                .is_none()
        );
        let batch: serde_json::Value = serde_json::from_str(&session.handle_stdio_line(r#"[{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":"a","method":"ping"},{"jsonrpc":"2.0","id":2,"method":"unknown"}]"#).unwrap().unwrap()).unwrap();
        assert_eq!(batch.as_array().unwrap().len(), 2);
        assert_eq!(batch[0]["id"], "a");
        assert_eq!(batch[1]["error"]["code"], -32601);
        let bad_tool: serde_json::Value = serde_json::from_str(&session.handle_stdio_line(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"doc_open","arguments":{}}}"#).unwrap().unwrap()).unwrap();
        assert_eq!(bad_tool["result"]["isError"], true);
        let bad_params: serde_json::Value = serde_json::from_str(&session.handle_stdio_line(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"doc_summary","arguments":[]}}"#).unwrap().unwrap()).unwrap();
        assert_eq!(bad_params["error"]["code"], -32602);
    }

    #[test]
    fn open_then_summary() {
        let path = write_tmp("simple.stencilcraft", &doc_json());
        let mut session = Session::default();
        let open = session
            .handle_line(&format!(
                r#"{{"id":1,"method":"doc.open","params":{{"path":"{path}"}}}}"#
            ))
            .unwrap()
            .unwrap();
        assert!(open.contains("\"ok\":true"), "{open}");
        let summary = session
            .handle_line(r#"{"id":2,"method":"doc.summary"}"#)
            .unwrap()
            .unwrap();
        assert!(summary.contains("\"shapes\":1"), "{summary}");
    }

    #[test]
    fn unknown_method_says_so() {
        let mut session = Session::default();
        let r = session
            .handle_line(r#"{"id":3,"method":"doc.nope"}"#)
            .unwrap()
            .unwrap();
        assert!(r.contains("\"ok\":false"), "{r}");
        assert!(r.contains("unknown method"), "{r}");
    }

    #[test]
    fn malformed_json_gets_null_id_error() {
        let mut session = Session::default();
        let r = session.handle_line("not json").unwrap().unwrap();
        assert!(r.contains("\"id\":null"), "{r}");
        assert!(r.contains("\"ok\":false"), "{r}");
    }

    #[test]
    fn bad_params_never_abort() {
        let mut session = Session::default();
        session
            .handle_line(r#"{"id":4,"method":"doc.open","params":{"path":"missing.no"}}"#)
            .unwrap();
        let still = session
            .handle_line(r#"{"id":5,"method":"doc.summary"}"#)
            .unwrap()
            .unwrap();
        assert!(still.contains("\"ok\":true"), "session survives: {still}");
    }

    #[test]
    fn command_execute_runs_the_engine() {
        let mut session = Session::default();
        let r = session
            .handle_line(
                r#"{"id":6,"method":"command.execute","params":{"command":"shape.add-rect","params":{"x":0,"y":0,"width":5,"height":5}}}"#,
            )
            .unwrap()
            .unwrap();
        assert!(r.contains("\"ok\":true"), "{r}");
        assert_eq!(session.engine.doc.shape_count(), 1);
    }

    #[test]
    fn export_roundtrip() {
        let src = write_tmp("start.stencilcraft", &doc_json());
        let out = std::env::temp_dir()
            .join("stencilcraft-mcp-tests")
            .join("start.svg")
            .to_string_lossy()
            .into_owned();
        let mut session = Session::default();
        session
            .handle_line(&format!(
                r#"{{"id":1,"method":"doc.open","params":{{"path":"{src}"}}}}"#
            ))
            .unwrap();
        let r = session
            .handle_line(&format!(
                r#"{{"id":2,"method":"export.svg","params":{{"path":"{out}"}}}}"#
            ))
            .unwrap()
            .unwrap();
        assert!(r.contains("\"ok\":true"), "{r}");
        let svg = std::fs::read_to_string(&out).unwrap();
        assert!(svg.contains("stencilcraft-cut"));
    }

    #[test]
    fn blank_lines_are_skipped() {
        let mut session = Session::default();
        assert!(session.handle_line("   ").unwrap().is_none());
        assert!(session.handle_line("\t\n").unwrap().is_none());
    }
}
