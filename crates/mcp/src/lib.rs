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

    /// Wrap an engine the caller already owns, so a host (the desktop app) can run
    /// requests against the document it is displaying.
    pub fn with_engine(engine: Engine) -> Self {
        Self {
            engine,
            last_path: None,
        }
    }

    /// Take the engine back out after handling requests.
    pub fn into_engine(self) -> Engine {
        self.engine
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
                let settings = gcode_settings(params)?;
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
                let marks = params
                    .get("marks")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let pieces = stencilcraft_tiling::tile_document_with(
                    &self.engine.doc,
                    &stencilcraft_tiling::SheetOptions {
                        sheet_w,
                        sheet_h,
                        overlap_mm: overlap,
                        marks,
                    },
                )
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
            "tile.preview" => {
                let path = str_param(params, "path")?;
                let sheet_w = f64_param(params, "sheet_w")?;
                let sheet_h = f64_param(params, "sheet_h")?;
                let overlap = params
                    .get("overlap")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let marks = params
                    .get("marks")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true);
                let columns = params
                    .get("columns")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|n| usize::try_from(n).ok())
                    .unwrap_or(stencilcraft_svg::TilePreviewOptions::default().columns);
                let scale = params
                    .get("scale")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(stencilcraft_svg::TilePreviewOptions::default().scale);
                let pieces = stencilcraft_tiling::tile_document_with(
                    &self.engine.doc,
                    &stencilcraft_tiling::SheetOptions {
                        sheet_w,
                        sheet_h,
                        overlap_mm: overlap,
                        marks,
                    },
                )
                .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                let text = stencilcraft_svg::export_tile_preview(
                    &pieces,
                    &stencilcraft_svg::TilePreviewOptions { columns, scale },
                )
                .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                std::fs::write(&path, text)
                    .map_err(|e| ProtocolError::Bad(format!("{path}: {e}")))?;
                Ok(serde_json::json!({ "path": path, "tiles": pieces.len() }))
            }
            "machines.list" => Ok(machines_json()),
            "machine.capabilities" => machine_capabilities(params),
            "machine.dry_run" => machine_dry_run(&self.engine.doc, params),
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
        "tile_preview" => Some("tile.preview"),
        "command_execute" => Some("command.execute"),
        "machines_list" => Some("machines.list"),
        "machine_capabilities" => Some("machine.capabilities"),
        "machine_dry_run" => Some("machine.dry_run"),
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
            "Export the current document as G-code. Optional profile knobs: feed (mm/min), power and engrave_power (0-1000), passes (1-10), ramp (boolean), mode (\"dynamic\" for M4 or \"constant\" for M3).",
            json!({
                "path": path,
                "feed": number,
                "power": json!({"type": "integer"}),
                "engrave_power": json!({"type": "integer"}),
                "passes": json!({"type": "integer"}),
                "ramp": json!({"type": "boolean"}),
                "mode": json!({"type": "string", "enum": ["dynamic", "constant"]}),
            }),
            json!(["path"]),
        ),
        (
            "tile_run",
            "Export tiled SVGs using a filename prefix and sheet dimensions in millimetres. Set marks to true to stamp registration marks on each sheet.",
            json!({"path": path, "sheet_w": number, "sheet_h": number, "overlap": number, "marks": json!({"type": "boolean"})}),
            json!(["path", "sheet_w", "sheet_h"]),
        ),
        (
            "tile_preview",
            "Write one SVG page showing every sheet of the tiling, each in a framed cell, so the whole job can be inspected before cutting.",
            json!({"path": path, "sheet_w": number, "sheet_h": number, "overlap": number, "columns": json!({"type": "integer"}), "scale": number, "marks": json!({"type": "boolean"})}),
            json!(["path", "sheet_w", "sheet_h"]),
        ),
        (
            "machines_list",
            "List every catalogued cutting machine with its dialect, USB id, cut area, mats, tool holders, tools, force/speed ranges and Bluetooth support. null means not publicly documented.",
            json!({}),
            json!([]),
        ),
        (
            "machine_capabilities",
            "Full capabilities of one machine: protocol, support level, USB id, cut area, mats, tool holders, tools, force/speed ranges with units, roll feed, registration marks, overcut, multi-pass, notes.",
            json!({"model": json!({"type": "string"})}),
            json!(["model"]),
        ),
        (
            "machine_dry_run",
            "Encode the current document for a cutter without touching hardware and return the command transcript. Settings outside the model's documented limits are rejected. model is a machines_list id; optional: media (preset name/id), force, speed, tool, holder, depth, passes, overcut (mm), mat, media_size [w, l] mm, pen, track_enhancing, bbox, regmarks {width, length, x, y, search, quad}, path (also write the byte stream there).",
            json!({
                "model": json!({"type": "string"}),
                "media": json!({"type": ["string", "integer"]}),
                "force": json!({"type": "integer"}),
                "speed": json!({"type": "integer"}),
                "tool": json!({"type": "string"}),
                "holder": json!({"type": "integer"}),
                "depth": json!({"type": "integer"}),
                "passes": json!({"type": "integer"}),
                "overcut": number,
                "mat": json!({"type": "string"}),
                "media_size": json!({"type": "array", "items": number}),
                "pen": json!({"type": "boolean"}),
                "track_enhancing": json!({"type": "boolean"}),
                "bbox": json!({"type": "boolean"}),
                "regmarks": json!({"type": "object"}),
                "path": path,
            }),
            json!(["model"]),
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

fn u32_param(params: &serde_json::Value, key: &str) -> Result<Option<u32>, ProtocolError> {
    match params.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| ProtocolError::Bad(format!("`{key}` must be a non-negative integer"))),
    }
}

/// G-code profile knobs from a request, with every unset one at its default. Ranges
/// are the exporter's business, so an out-of-range value comes back as a normal
/// tool error naming the knob.
fn gcode_settings(
    params: &serde_json::Value,
) -> Result<stencilcraft_gcode::GcodeSettings, ProtocolError> {
    let mut settings = stencilcraft_gcode::GcodeSettings::default();
    if let Some(feed) = params.get("feed").and_then(serde_json::Value::as_f64) {
        settings.feed_mm_per_min = feed;
    }
    if let Some(power) = u32_param(params, "power")? {
        settings.power = power;
    }
    if let Some(power) = u32_param(params, "engrave_power")? {
        settings.engrave_power = Some(power);
    }
    if let Some(passes) = u32_param(params, "passes")? {
        settings.passes = passes;
    }
    if let Some(ramp) = params.get("ramp").and_then(serde_json::Value::as_bool) {
        settings.ramp_power = ramp;
    }
    if let Some(mode) = params.get("mode").and_then(serde_json::Value::as_str) {
        settings.mode = stencilcraft_gcode::LaserMode::parse(mode).ok_or_else(|| {
            ProtocolError::Bad(format!(
                "export.gcode: mode `{mode}` is neither dynamic nor constant"
            ))
        })?;
    }
    Ok(settings)
}

/// The machine registry as JSON (`machines.list`): every `Capabilities` row
/// from `machines.toml`; `null` = not publicly documented.
pub fn machines_json() -> serde_json::Value {
    serde_json::to_value(stencilcraft_machine::DeviceRegistry::builtin().machines())
        .unwrap_or(serde_json::Value::Null)
}

/// `machine.capabilities`: one machine's full capability row.
fn machine_capabilities(params: &serde_json::Value) -> Result<serde_json::Value, ProtocolError> {
    let id = str_param(params, "model")?;
    let caps = stencilcraft_machine::DeviceRegistry::builtin()
        .by_id(&id)
        .ok_or_else(|| {
            ProtocolError::Bad(format!(
                "machine.capabilities: unknown model `{id}` (see machines_list)"
            ))
        })?;
    serde_json::to_value(caps).map_err(|e| ProtocolError::Internal(e.to_string()))
}

/// Longest transcript a dry run returns inline.
const DRY_RUN_LINES: usize = 5000;

/// `machine.dry_run`: validate and encode the document for a model. Never
/// sends: real sends are only `stencilcraft-cli cut --device`.
fn machine_dry_run(
    doc: &stencilcraft_doc::Document,
    params: &serde_json::Value,
) -> Result<serde_json::Value, ProtocolError> {
    use stencilcraft_machine as m;
    let bad = |e: String| ProtocolError::Bad(format!("machine.dry_run: {e}"));
    let id = str_param(params, "model")?;
    let caps = m::DeviceRegistry::builtin()
        .by_id(&id)
        .ok_or_else(|| bad(format!("unknown model `{id}` (see machines_list)")))?;
    let media = match params.get("media") {
        None | Some(serde_json::Value::Null) => m::Media::by_id(102),
        Some(v) => match (v.as_u64(), v.as_str()) {
            (Some(n), _) => u16::try_from(n).ok().and_then(m::Media::by_id),
            (_, Some(name)) => {
                let lower = name.to_ascii_lowercase();
                m::MEDIA_PRESETS
                    .iter()
                    .copied()
                    .find(|p| p.name.to_ascii_lowercase().starts_with(&lower))
            }
            _ => None,
        },
    }
    .ok_or_else(|| bad("unknown media preset".to_string()))?;
    let mut s = m::CutSettings::for_caps(caps, &media);
    let small = |key: &str| -> Result<Option<u8>, ProtocolError> {
        u32_param(params, key)?
            .map(|n| u8::try_from(n).map_err(|_| bad(format!("`{key}` {n} is out of range"))))
            .transpose()
    };
    let flag = |key: &str| {
        params
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    };
    if let Some(v) = small("depth")? {
        s.depth = Some(v);
    }
    if let Some(v) = small("holder")? {
        s.toolholder = v;
    }
    if let Some(v) = small("passes")? {
        s.passes = v;
    }
    if let Some(v) = params.get("overcut").and_then(serde_json::Value::as_f64) {
        s.overcut_mm = v;
    }
    if let Some(t) = params.get("tool").and_then(serde_json::Value::as_str) {
        s.tool = Some(m::ToolKind::parse(t).ok_or_else(|| bad(format!("unknown tool `{t}`")))?);
    }
    if let Some(v) = params.get("mat").and_then(serde_json::Value::as_str) {
        s.mat = m::Mat::parse(v).ok_or_else(|| bad(format!("unknown mat `{v}`")))?;
    }
    if let Some(arr) = params
        .get("media_size")
        .and_then(serde_json::Value::as_array)
    {
        let (Some(w), Some(l)) = (
            arr.first().and_then(serde_json::Value::as_f64),
            arr.get(1).and_then(serde_json::Value::as_f64),
        ) else {
            return Err(bad("media_size must be [width, length] in mm".to_string()));
        };
        s.media_size_mm = Some((w, l));
    }
    s.pen = flag("pen");
    s.track_enhancing = flag("track_enhancing");
    if let Some(r) = params.get("regmarks").filter(|r| !r.is_null()) {
        let f = |k: &str| {
            r.get(k)
                .and_then(serde_json::Value::as_f64)
                .ok_or_else(|| bad(format!("regmarks.{k} must be a number")))
        };
        let b = |k: &str| {
            r.get(k)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        };
        s.registration = Some(m::Registration {
            width_mm: f("width")?,
            length_mm: f("length")?,
            origin_right_mm: f("x")?,
            origin_down_mm: f("y")?,
            search: b("search") || b("quad"),
            quad: b("quad"),
        });
    }
    let mut paths = Vec::new();
    for layer in doc.layers.iter().filter(|l| l.visible) {
        for shape in &layer.shapes {
            let pl = stencilcraft_geom::path_to_polylines(
                &shape.path,
                stencilcraft_geom::DEFAULT_FLATTEN_STEPS,
            );
            paths.extend(m::paths_from_polylines(&pl));
        }
    }
    if paths.is_empty() {
        return Err(bad("document has no visible shapes to cut".to_string()));
    }
    let mut job = s.to_job(&paths, &m::PlotOptions::default(), flag("bbox"));
    // Force/speed in the machine's own units (steps, laser power, mm/min).
    let (force, speed) = (u32_param(params, "force")?, u32_param(params, "speed")?);
    for op in &mut job.operations {
        if force.is_some() {
            op.settings.force = force;
        }
        if speed.is_some() {
            op.settings.speed = speed;
        }
    }
    let program = m::dry_run(caps, &job).map_err(|e| bad(e.to_string()))?;
    let stream = program.to_stream();
    let written = match params.get("path").and_then(serde_json::Value::as_str) {
        Some(path) => {
            std::fs::write(path, &stream).map_err(|e| bad(format!("{path}: {e}")))?;
            Some(path.to_string())
        }
        None => None,
    };
    let transcript = program.transcript();
    Ok(serde_json::json!({
        "model": caps.id,
        "protocol": caps.protocol.id(),
        "commands": transcript.len(),
        "bytes": stream.len(),
        "bbox_mm": program.bbox_mm.map(|b| [b.1, b.0, b.3, b.2]),
        "registration_wait": program.registration_wait,
        "truncated": transcript.len() > DRY_RUN_LINES,
        "transcript": transcript.iter().take(DRY_RUN_LINES).collect::<Vec<_>>(),
        "path": written,
    }))
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
    fn machines_list_and_dry_run() {
        let mut session = Session::default();
        let r: serde_json::Value = serde_json::from_str(
            &session
                .handle_line(r#"{"id":1,"method":"machines.list"}"#)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let list = r["result"].as_array().unwrap();
        assert!(
            list.iter()
                .any(|m| m["id"] == "cameo-4-pro" && m["usb"] == "0b4d:1139")
        );
        assert!(
            list.iter()
                .any(|m| m["id"] == "curio" && m["usb"].is_null())
        );

        let src = write_tmp("dry.stencilcraft", &doc_json());
        session
            .handle_line(&format!(
                r#"{{"id":2,"method":"doc.open","params":{{"path":"{src}"}}}}"#
            ))
            .unwrap();
        let r = session
            .handle_line(r#"{"id":3,"method":"machine.dry_run","params":{"model":"cameo-4","mat":"12x12","tool":"autoblade","depth":3}}"#)
            .unwrap()
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v["ok"], true, "{r}");
        let lines = v["result"]["transcript"].as_array().unwrap();
        assert!(lines.iter().any(|l| l == "TF3,1"));
        let r = session
            .handle_line(
                r#"{"id":4,"method":"machine.dry_run","params":{"model":"cameo-4","force":50}}"#,
            )
            .unwrap()
            .unwrap();
        assert!(r.contains("\"ok\":false") && r.contains("force 50"), "{r}");
        let r = session
            .handle_line(r#"{"id":5,"method":"machine.dry_run","params":{"model":"cricut-maker"}}"#)
            .unwrap()
            .unwrap();
        // A Cricut slot exists but never encodes; it points at the export workaround.
        assert!(
            r.contains("\"ok\":false") && r.contains("not implemented") && r.contains("SVG"),
            "{r}"
        );
        let r = session
            .handle_line(r#"{"id":6,"method":"machine.dry_run","params":{"model":"warp-plotter"}}"#)
            .unwrap()
            .unwrap();
        assert!(r.contains("unknown model"), "{r}");
    }

    #[test]
    fn blank_lines_are_skipped() {
        let mut session = Session::default();
        assert!(session.handle_line("   ").unwrap().is_none());
        assert!(session.handle_line("\t\n").unwrap().is_none());
    }
}
