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
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                if !arguments.is_object() {
                    return Some(rpc_error(id, -32602, "Tool arguments must be an object"));
                }
                // Engine tools take the command's own params as their arguments.
                let (method, arguments) = match ENGINE_TOOLS.iter().find(|t| t.0 == name) {
                    Some((_, command, _)) => (
                        "command.execute",
                        serde_json::json!({"command": command, "params": arguments}),
                    ),
                    None => match tool_method(name) {
                        Some(method) => (method, arguments),
                        None => return Some(rpc_error(id, -32602, "Unknown tool")),
                    },
                };
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
                let (doc, warnings) = open_document(path.as_str())?;
                let result = serde_json::json!({
                    "layers": doc.layers.len(),
                    "shapes": doc.shape_count(),
                    "warnings": warnings,
                });
                self.engine = Engine::new(doc);
                // An imported foreign file is never a save target.
                self.last_path = path
                    .to_ascii_lowercase()
                    .ends_with(".stencilcraft")
                    .then(|| path.to_string());
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
            "import.file" => {
                let path = str_param(params, "path")?;
                let mut options = stencilcraft_import::ImportOptions {
                    page_mm: (
                        self.engine.doc.board.width_mm,
                        self.engine.doc.board.height_mm,
                    ),
                    ..stencilcraft_import::ImportOptions::default()
                };
                let flag = |key: &str| params.get(key).and_then(serde_json::Value::as_bool);
                let num = |key: &str| params.get(key).and_then(serde_json::Value::as_f64);
                options.fit_to_page = flag("fit_to_page").unwrap_or(false);
                options.join = flag("join").unwrap_or(options.join);
                options.join_tolerance_mm =
                    num("join_tolerance").unwrap_or(options.join_tolerance_mm);
                options.dpi = num("dpi").unwrap_or(options.dpi);
                let imported = stencilcraft_import::import_file(&path, &options)
                    .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                let shapes: Vec<_> = imported
                    .layers
                    .iter()
                    .flat_map(|l| l.shapes.iter().map(|s| (s.path.clone(), s.style)))
                    .collect();
                let mut shape_ids = Vec::new();
                if !shapes.is_empty() {
                    match self.engine.run(Command::ShapeAddPaths { shapes }) {
                        Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) => shape_ids = ids,
                        Ok(_) => {}
                        Err(e) => return Err(ProtocolError::Bad(e.to_string())),
                    }
                }
                let mut image_ids = Vec::new();
                for image in imported.images {
                    if let stencilcraft_engine::Effect::ImageAdded { id } = self
                        .engine
                        .run(Command::ImagePlace { image })
                        .map_err(|e| ProtocolError::Bad(e.to_string()))?
                    {
                        image_ids.push(id);
                    }
                }
                Ok(serde_json::json!({ "shapes": shape_ids, "images": image_ids }))
            }
            "export.pdf" => {
                let path = str_param(params, "path")?;
                let bytes = stencilcraft_pdf::export(&self.engine.doc)
                    .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                std::fs::write(&path, bytes)
                    .map_err(|e| ProtocolError::Bad(format!("{path}: {e}")))?;
                Ok(serde_json::json!({ "path": path }))
            }
            "export.image" => {
                let path = str_param(params, "path")?;
                let dpi = params
                    .get("dpi")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(300.0);
                let ext = path.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
                let format =
                    stencilcraft_import::RenderFormat::from_extension(ext).ok_or_else(|| {
                        ProtocolError::Bad(format!(
                            "export.image: {path}: use a .png, .jpg or .jpeg name"
                        ))
                    })?;
                let bytes = stencilcraft_import::render_document(&self.engine.doc, dpi, format)
                    .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                std::fs::write(&path, bytes)
                    .map_err(|e| ProtocolError::Bad(format!("{path}: {e}")))?;
                Ok(serde_json::json!({ "path": path, "dpi": dpi }))
            }
            "export.print-sheet" => self.export_print_sheet(params),
            "command.execute" => {
                let command: Command = serde_json::from_value(params.clone())
                    .map_err(|e| ProtocolError::Bad(format!("bad command parameters: {e}")))?;
                self.engine
                    .run(command)
                    .map_err(|e: EngineError| ProtocolError::Bad(e.to_string()))
                    .map(stencilcraft_effect_json)
            }
            command if ENGINE_TOOLS.iter().any(|(_, name, _)| *name == command) => {
                let command: Command = serde_json::from_value(
                    serde_json::json!({"command": command, "params": params}),
                )
                .map_err(|e| ProtocolError::Bad(format!("bad {command} parameters: {e}")))?;
                self.engine
                    .run(command)
                    .map_err(|e: EngineError| ProtocolError::Bad(e.to_string()))
                    .map(stencilcraft_effect_json)
            }
            other => Err(ProtocolError::Bad(format!("unknown method: {other}"))),
        }
    }
}

impl Session {
    fn export_print_sheet(
        &mut self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, ProtocolError> {
        let path = str_param(params, "path")?;
        let mut options = params.clone();
        if let Some(map) = options.as_object_mut() {
            for key in ["path", "format", "dpi"] {
                map.remove(key);
            }
        }
        let options: stencilcraft_layout::PrintOptions = serde_json::from_value(options)
            .map_err(|e| ProtocolError::Bad(format!("bad print options: {e}")))?;
        let sheets = stencilcraft_layout::print_sheets(&self.engine.doc, &options)
            .map_err(|e| ProtocolError::Bad(e.to_string()))?;
        let format = params
            .get("format")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("pdf");
        let mut written = Vec::new();
        match format {
            "pdf" => {
                std::fs::write(&path, stencilcraft_layout::sheets_to_pdf(&sheets))
                    .map_err(|e| ProtocolError::Bad(format!("{path}: {e}")))?;
                written.push(path.clone());
            }
            "png" => {
                let dpi = params
                    .get("dpi")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(300.0);
                for (i, sheet) in sheets.iter().enumerate() {
                    let png = stencilcraft_layout::sheet_to_png(sheet, dpi)
                        .map_err(|e| ProtocolError::Bad(e.to_string()))?;
                    let file = if sheets.len() == 1 {
                        path.clone()
                    } else {
                        let stem = path.strip_suffix(".png").unwrap_or(&path);
                        format!("{stem}-{i:02}.png")
                    };
                    std::fs::write(&file, png)
                        .map_err(|e| ProtocolError::Bad(format!("{file}: {e}")))?;
                    written.push(file);
                }
            }
            other => {
                return Err(ProtocolError::Bad(format!(
                    "export.print-sheet: format must be pdf or png, got {other}"
                )));
            }
        }
        Ok(serde_json::json!({ "paths": written, "sheets": sheets.len() }))
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
        "export_print_sheet" => Some("export.print-sheet"),
        "command_execute" => Some("command.execute"),
        "machines_list" => Some("machines.list"),
        "machine_capabilities" => Some("machine.capabilities"),
        "machine_dry_run" => Some("machine.dry_run"),
        "import_file" => Some("import.file"),
        "export_pdf" => Some("export.pdf"),
        "export_image" => Some("export.image"),
        _ => ENGINE_TOOLS
            .iter()
            .find(|(tool, _, _)| *tool == name)
            .map(|(_, command, _)| *command),
    }
}

fn engine_tool_schema(command: &str) -> (serde_json::Value, serde_json::Value) {
    use serde_json::json;
    let ids = json!({"type": "array", "items": {"type": "integer"}});
    let number = json!({"type": "number"});
    let side = json!({"type": "string", "enum": ["left", "right", "above", "below"]});
    let count = json!({"type": "integer", "minimum": 2, "maximum": 1000});
    let tip = json!({"type": "string", "enum": ["fine", "wide"]});
    let passes = json!({"type": "integer", "minimum": 1, "maximum": 10});
    let tool = json!({"type": "object", "description": "{op: score|emboss|power-engrave|heat-pen, tip?: fine|wide, passes?: 1-10, deboss?: bool}"});
    let fill = json!({"pattern": {"type": "string", "enum": ["lines", "cross-hatch", "grid", "concentric", "spiral", "stipple-grid", "stipple-random"]}, "spacing_mm": number, "angle_deg": number, "inset_mm": number, "dot_mm": number, "seed": {"type": "integer"}});
    let integer = json!({"type": "integer", "minimum": 0});
    let string = json!({"type": "string"});
    let boolean = json!({"type": "boolean"});
    let combine = json!({"type": "string", "enum": ["unite", "subtract", "intersect", "exclude"]});
    let mats = mats_schema();
    let print = print_schema();
    match command {
        "shapes.replicate-side" | "shapes.mirror-copy" => (
            json!({"ids": ids, "side": side, "gap_mm": number}),
            json!(["ids", "side", "gap_mm"]),
        ),
        "shapes.replicate-array" => (
            json!({"ids": ids, "axis": {"type": "string", "enum": ["horizontal", "vertical"]}, "count": count, "gap_mm": number}),
            json!(["ids", "axis", "count", "gap_mm"]),
        ),
        "shapes.replicate-fill" => (
            json!({"ids": ids, "gap_mm": number}),
            json!(["ids", "gap_mm"]),
        ),
        "shapes.rotate-copies" => (json!({"ids": ids, "count": count}), json!(["ids", "count"])),
        "shapes.offset" => (
            json!({"ids": ids, "distance_mm": number, "join": {"type": "string", "enum": ["miter", "round", "bevel"]}, "miter_limit": number}),
            json!(["ids", "distance_mm"]),
        ),
        "generate.shape" => (
            json!({"shape": {"type": "object", "description": "{kind: heart|arrow|speech-bubble|ring|crescent|gear|spiral|banner|scallop-circle|scallop-square|triangle, ...kind params}"}, "x": number, "y": number, "width": number, "height": number, "replace": {"type": "integer"}}),
            json!(["shape", "x", "y", "width", "height"]),
        ),
        "generate.score-line" => (
            json!({"x0": number, "y0": number, "x1": number, "y1": number, "tip": tip, "passes": passes}),
            json!(["x0", "y0", "x1", "y1"]),
        ),
        "generate.fill" => {
            let mut p = fill.clone();
            p["ids"] = ids.clone();
            p["tool"] = tool.clone();
            (p, json!(["ids"]))
        }
        "generate.tool-lines" => (
            json!({"ids": ids, "tool": tool, "fill": {"type": "object", "properties": fill}}),
            json!(["ids", "tool"]),
        ),
        "generate.rhinestone" => (
            json!({"ids": ids, "size": {"type": "string", "enum": ["ss6", "ss8", "ss10", "ss12", "ss16", "ss20", "ss30"]}, "stone_mm": number, "spacing_mm": number, "layout": {"type": "string", "enum": ["outline", "fill", "outline-fill"]}, "lines": {"type": "integer", "minimum": 1, "maximum": 2}, "pattern": {"type": "string", "enum": ["grid", "hex"]}}),
            json!(["ids"]),
        ),
        "generate.punch" => (
            json!({"ids": ids, "spacing_mm": number, "diameter_mm": number}),
            json!(["ids"]),
        ),
        "generate.popup" => (
            json!({"x": number, "y": number, "width": number, "height": number, "struts": {"type": "integer", "minimum": 1, "maximum": 50}, "strut_width_mm": number, "base_width_pct": number, "dash_pitch_mm": number, "outline": {"type": "boolean"}}),
            json!([]),
        ),
        "generate.puzzle" => (
            json!({"id": {"type": "integer"}, "rows": {"type": "integer"}, "columns": {"type": "integer"}, "tab_size": number, "seed": {"type": "integer"}}),
            json!(["id"]),
        ),
        "generate.weed" => (
            json!({"ids": ids, "margin_mm": number, "spacing_mm": number, "direction": {"type": "string", "enum": ["vertical", "horizontal", "both"]}, "clearance_mm": number}),
            json!(["ids"]),
        ),
        "generate.registration" => (
            json!({"ids": ids, "margin_mm": number, "arm_mm": number, "mark": {"type": "string", "enum": ["corners", "crosshair"]}, "style": {"type": "string", "enum": ["cut", "engrave"]}}),
            json!(["ids"]),
        ),
        "generate.warp" => (
            json!({"ids": ids, "style": {"type": "string", "enum": ["none", "arc", "arc-lower", "arc-upper", "arch", "bulge", "shell-lower", "shell-upper", "flag", "wave", "fish", "rise", "fisheye", "inflate", "squeeze", "twist"]}, "bend": number, "horizontal": number, "vertical": number}),
            json!(["ids"]),
        ),
        "generate.perforate" => (
            json!({"ids": ids, "cut_mm": number, "gap_mm": number}),
            json!(["ids", "cut_mm", "gap_mm"]),
        ),
        "generate.wavy" => (
            json!({"ids": ids, "amplitude_mm": number, "wavelength_mm": number}),
            json!(["ids", "amplitude_mm", "wavelength_mm"]),
        ),
        "generate.object-on-path" => (
            json!({"ids": ids, "path_id": {"type": "integer"}, "perpendicular": {"type": "boolean"}, "start_angle_deg": number, "increment_angle_deg": number, "start_position_mm": number, "section_length_mm": number, "count": {"type": "integer"}, "step_mm": number}),
            json!(["ids", "path_id"]),
        ),
        "generate.replicate" => (
            json!({"ids": ids, "direction": {"type": "string", "enum": ["on-top", "left", "right", "above", "below", "angle"]}, "angle_deg": number, "count": {"type": "integer", "minimum": 1, "maximum": 10000}, "gap_mm": number}),
            json!(["ids", "direction"]),
        ),
        "generate.release" => (json!({"ids": ids}), json!(["ids"])),
        "image.trace" => (
            json!({
                "id": {"type": "integer"},
                "options": {"type": "object", "properties": {
                    "threshold": number, "despeckle": {"type": "integer"}, "low_pass": number, "high_pass": number,
                    "colour_range": {"type": ["object", "null"], "properties": {"hue_min": number, "hue_max": number, "min_saturation": number}},
                    "smoothing": number, "corner_threshold": number,
                    "mode": {"type": "string", "enum": ["all-edges", "outer-edge", "detach"]},
                }},
                "style": {"type": "string", "enum": ["cut", "engrave"]},
            }),
            json!(["id"]),
        ),
        "image.remove-background" => (
            json!({"id": {"type": "integer"}, "tolerance": number}),
            json!(["id", "tolerance"]),
        ),
        "image.set-bounds" => (
            json!({"id": {"type": "integer"}, "x": number, "y": number, "width": number, "height": number}),
            json!(["id", "x", "y", "width", "height"]),
        ),
        "image.remove" => (json!({"id": {"type": "integer"}}), json!(["id"])),
        "text.add" | "text.edit" => {
            let string = json!({"type": "string"});
            let boolean = json!({"type": "boolean"});
            let mut fields = json!({
                "text": string,
                "font": {"type": "object", "properties": {"family": string, "style": string, "file": string}},
                "size": number, "x": number, "y": number,
                "alignment": {"type": "string", "enum": ["left", "centre", "right", "justify"]},
                "letter_spacing": number, "line_spacing": number,
                "kerning": boolean, "vertical": boolean, "weld": boolean,
                "flow": {"type": "object", "properties": {
                    "kind": {"type": "string", "enum": ["straight", "curve", "on-path"]},
                    "diameter_mm": number, "shape": {"type": "integer"}, "start": number,
                    "offset_mm": number, "flip": boolean}},
                "letters": {"type": "array", "items": {"type": "object", "properties": {
                    "index": {"type": "integer"}, "dx_mm": number, "dy_mm": number, "rotate_deg": number}}},
            });
            if command == "text.add" {
                fields["style"] = json!({"type": "string", "enum": ["cut", "engrave"]});
                (fields, json!(["text", "font", "size", "x", "y"]))
            } else {
                fields["id"] = json!({"type": "integer"});
                (fields, json!(["id"]))
            }
        }
        "text.to-outlines" => (json!({"ids": ids}), json!(["ids"])),
        "text.ungroup" => (
            json!({"id": {"type": "integer"}, "to": {"type": "string", "enum": ["lines", "letters"]}}),
            json!(["id", "to"]),
        ),
        "text.fonts" => (json!({"query": {"type": "string"}}), json!([])),
        "text.monogram" => (
            json!({"initials": {"type": "string"},
                   "font": {"type": "object", "properties": {"family": {"type": "string"}, "style": {"type": "string"}, "file": {"type": "string"}}},
                   "size": number, "x": number, "y": number,
                   "frame": {"type": "string", "enum": ["none", "circle", "square", "diamond", "scallop"]},
                   "padding_mm": number}),
            json!(["initials", "font", "x", "y"]),
        ),
        "group.create" => (
            json!({"ids": ids, "groups": ids, "name": string}),
            json!([]),
        ),
        "group.ungroup" | "group.members" | "group.of" | "page.remove" | "guide.remove"
        | "select.by-layer" => {
            let key = if command.starts_with("group") {
                "id"
            } else {
                "index"
            };
            let mut properties = json!({});
            properties[key] = integer.clone();
            (properties, json!([key]))
        }
        "group.rename" => (
            json!({"id": integer, "name": string}),
            json!(["id", "name"]),
        ),
        "shapes.arrange" => (
            json!({"ids": ids, "order": {"type": "string", "enum": ["front", "back", "forward", "backward"]}}),
            json!(["ids", "order"]),
        ),
        "shapes.move-to-layer" => (
            json!({"ids": ids, "layer": integer}),
            json!(["ids", "layer"]),
        ),
        "layer.move" => (
            json!({"from": integer, "to": integer}),
            json!(["from", "to"]),
        ),
        "layer.rename" => (
            json!({"index": integer, "name": string}),
            json!(["index", "name"]),
        ),
        "layer.set-locked" | "guide.set-locked" => (
            json!({"index": integer, "locked": boolean}),
            json!(["index", "locked"]),
        ),
        "layer.set-template" => (
            json!({"index": integer, "template": boolean}),
            json!(["index", "template"]),
        ),
        "layer.set-color" => (
            json!({"index": integer, "color": {"type": ["string", "null"]}}),
            json!(["index", "color"]),
        ),
        "layer.color-sync" => (
            json!({"tolerance": number, "merge": boolean}),
            json!(["tolerance"]),
        ),
        "page.add" | "page.set" => {
            let mut properties = json!({
                "preset": {"type": "string", "enum": ["letter", "a4", "mat-12x12", "mat-12x24", "joy-mat-4.5x6.5", "joy-mat-4.5x12", "joy-xtra-mat-8.5x12", "custom"]},
                "width_mm": number, "height_mm": number,
                "orientation": {"type": "string", "enum": ["portrait", "landscape"]},
                "name": string, "mat": string, "media": string, "color": string,
            });
            if command == "page.set" {
                properties["index"] = integer.clone();
                (properties, json!(["index"]))
            } else {
                (properties, json!([]))
            }
        }
        "guide.add" => (
            json!({"axis": {"type": "string", "enum": ["horizontal", "vertical"]}, "position_mm": number}),
            json!(["axis", "position_mm"]),
        ),
        "guide.move" => (
            json!({"index": integer, "position_mm": number}),
            json!(["index", "position_mm"]),
        ),
        "guide.clear"
        | "select.all"
        | "select.none"
        | "document.update-thumbnail"
        | "templates.list" => (json!({}), json!([])),
        "grid.set" => (
            json!({"spacing_mm": number, "subdivisions": integer, "visible": boolean}),
            json!(["spacing_mm", "subdivisions"]),
        ),
        "snap.set" => (
            json!({"grid": boolean, "guides": boolean, "objects": boolean, "spacing": boolean, "tolerance_mm": number}),
            json!([]),
        ),
        "snap.point" => (
            json!({"x": number, "y": number, "width": number, "height": number, "exclude": ids, "tolerance_mm": number}),
            json!(["x", "y"]),
        ),
        "measure.distance" => {
            let point = json!({"type": "array", "items": number, "minItems": 2, "maxItems": 2});
            (json!({"from": point, "to": point}), json!(["from", "to"]))
        }
        "query.dimensions" | "clipboard.copy" | "clipboard.cut" | "transform.again" => {
            (json!({"ids": ids}), json!(["ids"]))
        }
        "select.by-color" => (
            json!({"color": string, "tolerance": number}),
            json!(["color"]),
        ),
        "select.by-operation" => (
            json!({"style": {"type": "string", "enum": ["cut", "engrave"]}}),
            json!(["style"]),
        ),
        "clipboard.paste" => (
            json!({"mode": {"type": "string", "enum": ["offset", "in-place", "in-front"]}, "offset_mm": number}),
            json!([]),
        ),
        "transform.each" => (
            json!({"ids": ids, "dx_mm": number, "dy_mm": number, "scale_x": number, "scale_y": number, "degrees": number, "each": boolean}),
            json!(["ids"]),
        ),
        "shapes.align-to-key" => (
            json!({"ids": ids, "key": integer, "alignment": {"type": "string", "enum": ["left", "horizontal-center", "right", "top", "vertical-center", "bottom"]}}),
            json!(["ids", "key", "alignment"]),
        ),
        "shapes.distribute-spacing" => (
            json!({"ids": ids, "axis": {"type": "string", "enum": ["horizontal", "vertical"]}, "gap_mm": number}),
            json!(["ids", "axis", "gap_mm"]),
        ),
        "document.set-meta" => (json!({"title": string, "notes": string}), json!([])),
        "document.new-from-template" => (
            json!({
                "template": {"type": "string", "description": "a template id from templates_list"},
                "params": {"type": "object", "properties": {
                    "preset": string, "width_mm": number, "height_mm": number,
                    "fold": {"type": "string", "enum": ["vertical", "horizontal"]}, "hole_mm": number,
                }},
            }),
            json!(["template"]),
        ),
        "shapes.slice" => (json!({"a": integer, "b": integer}), json!(["a", "b"])),
        "shapes.weld" | "shapes.attach" | "shapes.detach" | "shapes.flatten" => {
            (json!({"ids": ids}), json!(["ids"]))
        }
        "shapes.unflatten" | "shapes.uncombine" => (json!({"id": integer}), json!(["id"])),
        "shape.contour" => (
            json!({"id": integer, "contours": {"type": "array", "items": integer}, "hidden": boolean}),
            json!(["id", "contours", "hidden"]),
        ),
        "shapes.combine" => (
            json!({"ids": ids, "operation": combine}),
            json!(["ids", "operation"]),
        ),
        "shape.set-combine" => (
            json!({"id": integer, "operation": combine}),
            json!(["id", "operation"]),
        ),
        "shapes.create-sticker" => (
            json!({"ids": ids, "border_mm": number, "cut": {"type": "string", "enum": ["kiss-cut", "die-cut"]}}),
            json!(["ids"]),
        ),
        "shapes.set-production" => (
            json!({
                "ids": ids,
                "operation": {"type": "string", "enum": ["cut", "kiss-cut", "die-cut", "draw", "score", "engrave", "perforate", "print-then-cut", "guide"]},
                "line_color": string, "fill_color": string, "material": string,
                "send": {"type": "string", "enum": ["tool1", "tool1-outline", "tool2", "tool2-outline", "ignore"]},
                "clear": {"type": "array", "items": {"type": "string", "enum": ["operation", "line-color", "fill-color", "material", "send"]}},
            }),
            json!(["ids"]),
        ),
        "shapes.move-to-mat" => (
            json!({"ids": ids, "mat": {"type": ["integer", "null"]}}),
            json!(["ids", "mat"]),
        ),
        "production.machines" => (json!({}), json!([])),
        "production.mats" => (mats, json!([])),
        "production.job" => (
            json!({
                "mats": {"type": "object", "properties": mats},
                "cut_by": {"type": "string", "enum": ["operation", "line-color", "fill-color", "layer"]},
                "groups": {"type": "array", "items": {"type": "object", "properties": {
                    "key": string, "tool": {"type": "integer", "enum": [1, 2]}, "enabled": boolean,
                    "passes": integer, "overcut_mm": number}, "required": ["key"]}},
                "order": {"type": "string", "enum": ["document", "minimize-roller", "by-color", "by-layer", "shortest-travel"]},
                "inside_first": boolean, "overcut_mm": number, "passes": integer,
                "test_cut": {"type": "object", "properties": {"mat": integer, "x_mm": number, "y_mm": number, "size_mm": number}, "required": ["x_mm", "y_mm"]},
                "weed": {"type": "object", "properties": {"margin_mm": number, "lines": boolean}},
                "origin": {"type": "string", "enum": ["top-left", "top-right", "bottom-left", "bottom-right"]},
                "feed": {"type": "string", "enum": ["length", "width"]},
                "after": {"type": "string", "enum": ["return-to-origin", "feed"]},
                "stipple_order": {"type": "string", "enum": ["default", "edge-to-center", "center-to-edge"]},
            }),
            json!([]),
        ),
        "production.print-sheet" => (print, json!([])),
        _ => (json!({}), json!([])),
    }
}

/// Properties of `production.mats` (also `mats` inside job and print options).
fn mats_schema() -> serde_json::Value {
    use serde_json::json;
    let number = json!({"type": "number"});
    let ints = json!({"type": "array", "items": {"type": "integer"}});
    json!({
        "machine": {"type": "string", "description": "Machine profile id (production_machines)"},
        "mat": {"type": "string", "description": "Mat size name, e.g. 12x12; smallest that fits when unset"},
        "mat_free": {"type": "boolean"},
        "copies": {"type": "integer", "minimum": 1, "maximum": 100},
        "spacing_mm": number,
        "nest": {"type": "boolean"},
        "rotation_step_deg": number,
        "mirror_on": ints,
        "mirror_off": ints,
        "page": {"type": "object", "properties": {
            "width_mm": number, "height_mm": number,
            "marks": {"type": "string", "enum": ["silhouette", "cricut", "four-corner"]},
            "mark_length_mm": number, "mark_thickness_mm": number, "mark_inset_mm": number,
        }},
    })
}

/// Properties of `production.print-sheet` and `export_print_sheet`.
fn print_schema() -> serde_json::Value {
    use serde_json::json;
    json!({
        "mats": {"type": "object", "properties": mats_schema()},
        "bleed": {"type": "boolean"},
        "bleed_mm": {"type": "number"},
    })
}

/// Dedicated MCP tools for engine commands: (tool, command, description). Arguments are the
/// command's params (`docs/mcp.md`); a node is `{"contour": c, "node": n}`.
const ENGINE_TOOLS: &[(&str, &str, &str)] = &[
    (
        "path_break",
        "path.break",
        "Break a shape's path at a node. Args: id, contour, node.",
    ),
    (
        "path_join",
        "path.join",
        "Join two open contours end to end. Args: a, b (shape ids, may be equal), a_contour?, b_contour? (default 0), a_end?, b_end? (\"start\"/\"end\", default nearest).",
    ),
    (
        "path_close",
        "path.close",
        "Close an open contour. Args: id, contour? (default 0).",
    ),
    (
        "path_average_nodes",
        "path.average-nodes",
        "Average node positions. Args: id, nodes [{contour, node}], axis (\"horizontal\", \"vertical\", \"both\").",
    ),
    (
        "path_merge_nodes",
        "path.merge-nodes",
        "Merge nodes into one at their centroid. Args: id, nodes [{contour, node}].",
    ),
    (
        "path_delete_nodes",
        "path.delete-nodes",
        "Delete nodes. Args: id, nodes [{contour, node}].",
    ),
    (
        "path_set_node_type",
        "path.set-node-type",
        "Convert nodes to corner or smooth. Args: id, nodes [{contour, node}], kind (\"corner\", \"smooth\").",
    ),
    (
        "path_set_segment",
        "path.set-segment",
        "Make a segment straight or curved. Args: id, contour, segment (index of its first node), kind (\"line\", \"curve\").",
    ),
    (
        "path_simplify",
        "path.simplify",
        "Simplify paths to fewer nodes within a tolerance. Args: ids, tolerance_mm.",
    ),
    (
        "path_outline_stroke",
        "path.outline-stroke",
        "Convert stroked paths to filled cut contours. Args: ids, width_mm, join? (\"miter\", \"round\", \"bevel\"), cap? (\"butt\", \"round\", \"square\"), miter_limit?.",
    ),
    (
        "shapes_to_path",
        "shapes.to-path",
        "Convert live shape primitives to plain paths. Args: ids.",
    ),
    (
        "shapes_make_compound",
        "shapes.make-compound",
        "Make a compound path from shapes. Args: ids.",
    ),
    (
        "shapes_release_compound",
        "shapes.release-compound",
        "Release compound paths / detach: one shape per contour. Args: ids.",
    ),
    (
        "shape_pathfinder",
        "shape.pathfinder",
        "Pathfinder on shapes in document order. Args: ids, operation (union, difference, subtract-all, intersect, xor, divide, trim, merge, crop, outline, minus-back).",
    ),
    (
        "shapes_set_locked",
        "shapes.set-locked",
        "Lock or unlock shapes. Args: ids, locked.",
    ),
    (
        "shapes_set_hidden",
        "shapes.set-hidden",
        "Hide or show shapes (hidden shapes are not exported). Args: ids, hidden.",
    ),
    (
        "query_node_count",
        "query.node-count",
        "Count shapes, contours, nodes, linear and bezier nodes. Args: ids? (default all).",
    ),
    (
        "select_same_fill",
        "select.same-fill",
        "Ids of selectable shapes with the same fill as a shape. Args: id.",
    ),
    (
        "select_same_stroke",
        "select.same-stroke",
        "Ids of selectable shapes with the same stroke as a shape. Args: id.",
    ),
    (
        "select_inverse",
        "select.inverse",
        "Ids of selectable shapes not in the given selection. Args: ids.",
    ),
    (
        "shapes_replicate_side",
        "shapes.replicate-side",
        "Copy the shapes `ids` once to the `side` (left, right, above, below), edge to edge with `gap_mm` between.",
    ),
    (
        "shapes_replicate_array",
        "shapes.replicate-array",
        "Make a row (axis horizontal) or column (axis vertical) of `count` items (2-1000, the selection included), `gap_mm` apart.",
    ),
    (
        "shapes_replicate_fill",
        "shapes.replicate-fill",
        "Fill the board with as many copies of the selection as fit whole on its grid, `gap_mm` apart.",
    ),
    (
        "shapes_mirror_copy",
        "shapes.mirror-copy",
        "Add a mirrored copy of the selection on `side` (left/right flip horizontally, above/below vertically), `gap_mm` away.",
    ),
    (
        "shapes_rotate_copies",
        "shapes.rotate-copies",
        "Radial array: `count` items (2-1000, the selection included) rotated evenly about the selection's centre.",
    ),
    (
        "shapes_offset",
        "shapes.offset",
        "Add an offset outline of each shape: positive `distance_mm` grows, negative is an internal offset. Optional `join` (miter, round, bevel; default miter) and `miter_limit` (corner limit, >= 1, default 4).",
    ),
    (
        "generate_shape",
        "generate.shape",
        "Add a parametric shape (heart, arrow, speech-bubble, ring, crescent, gear, spiral, banner, scallop-circle, scallop-square, triangle) fitted to a box; `replace: id` regenerates an existing shape in place to edit its parameters.",
    ),
    (
        "generate_score_line",
        "generate.score-line",
        "Add a straight score (fold) line from (x0, y0) to (x1, y1); tip fine/wide, passes 1-10.",
    ),
    (
        "generate_fill",
        "generate.fill",
        "Sketch/stipple/tool fill of closed shapes as engrave paths: pattern lines, cross-hatch, grid, concentric, spiral, stipple-grid, stipple-random; spacing_mm (default 0.4064), angle_deg, inset_mm, dot_mm, seed; optional tool tag (power-engrave, heat-pen, emboss, score).",
    ),
    (
        "generate_tool_lines",
        "generate.tool-lines",
        "Convert outlines into score/emboss (or power-engrave/heat-pen) lines, optionally with an interior fill using the same tool.",
    ),
    (
        "generate_rhinestone",
        "generate.rhinestone",
        "Rhinestone template: stone-sized holes along the outline (1 or 2 lines) and/or as a grid/hex fill. size ss6-ss30 or stone_mm, spacing_mm gap.",
    ),
    (
        "generate_punch",
        "generate.punch",
        "Replace paths with leather punch points every spacing_mm (default 3.5) of diameter_mm (1, 3 or 5; default 3), tagged as punch operations.",
    ),
    (
        "generate_popup",
        "generate.popup",
        "Pop-up card: card outline and box struts across a vertical centre fold (cuts) plus fold lines (scores). struts, strut_width_mm, base_width_pct (strut depth), dash_pitch_mm.",
    ),
    (
        "generate_puzzle",
        "generate.puzzle",
        "Jigsaw puzzle cut lines over a shape: rows x columns, tab_size (0-0.35 of a cell), seed.",
    ),
    (
        "generate_weed",
        "generate.weed",
        "Weeding box around shapes at margin_mm, with internal weed lines every spacing_mm (vertical, horizontal or both) stopping clearance_mm short of the design.",
    ),
    (
        "generate_registration",
        "generate.registration",
        "Registration marks (corners or crosshair) around the shapes' bounds at margin_mm, arms arm_mm long.",
    ),
    (
        "generate_warp",
        "generate.warp",
        "Warp shapes in place: style (arc, arc-lower, arc-upper, arch, bulge, shell-lower, shell-upper, flag, wave, fish, rise, fisheye, inflate, squeeze, twist, none), bend, horizontal, vertical (each -1..1).",
    ),
    (
        "generate_perforate",
        "generate.perforate",
        "Turn paths into perforated cut lines in place: dashes cut_mm long, gap_mm apart.",
    ),
    (
        "generate_wavy",
        "generate.wavy",
        "Turn paths into wavy lines in place: amplitude_mm either side, one period per wavelength_mm.",
    ),
    (
        "generate_object_on_path",
        "generate.object-on-path",
        "Copies of the shapes `ids` along path_id's outline: perpendicular, start_angle_deg, increment_angle_deg, start_position_mm, section_length_mm (0 = whole), count (default 50) or step_mm.",
    ),
    (
        "generate_replicate",
        "generate.replicate",
        "Advanced replicate: `count` copies on-top, left, right, above, below or along angle_deg, gap_mm apart.",
    ),
    (
        "generate_release",
        "generate.release",
        "Release a generated effect (fill, punch points, stones, copies) into plain paths: one shape per contour, tool tags cleared.",
    ),
    (
        "image_trace",
        "image.trace",
        "Trace a placed image into closed cut paths. options: threshold (0-100 %), despeckle (px area), low_pass and high_pass (radius px, 0-50), colour_range {hue_min, hue_max, min_saturation}, smoothing (px tolerance), corner_threshold (degrees), mode (all-edges, outer-edge, detach). style: cut or engrave.",
    ),
    (
        "image_remove_background",
        "image.remove-background",
        "Make a placed image's background transparent: flood fill from the border through colours within tolerance (0-100 %) of the border's dominant colour.",
    ),
    (
        "image_set_bounds",
        "image.set-bounds",
        "Move/resize a placed image. Args: id, x, y, width, height (mm).",
    ),
    (
        "image_remove",
        "image.remove",
        "Remove a placed image. Args: id.",
    ),
    (
        "text_add",
        "text.add",
        "Add live, editable text. Args: text, font {family, style?, file?} (\"StencilCraft Single Line\" is the built-in pen font), size (pt), x, y (mm, first baseline), alignment? (left, centre, right, justify), letter_spacing? (% of em, 0 normal), line_spacing? (%, 100 normal), kerning? (default true), vertical?, weld?, flow? ({kind: straight} | {kind: curve, diameter_mm} | {kind: on-path, shape or path, start (0-1), offset_mm, flip}), letters? [{index, dx_mm, dy_mm, rotate_deg}], style? (cut, engrave).",
    ),
    (
        "text_edit",
        "text.edit",
        "Change a text object's parameters and lay it out again (moves and turns made since are kept). Args: id, then any text_add field.",
    ),
    (
        "text_to_outlines",
        "text.to-outlines",
        "Keep text shapes as plain cut paths (no longer editable). Args: ids.",
    ),
    (
        "text_ungroup",
        "text.ungroup",
        "Split a text object in place into one text object per line or per letter. Args: id, to (lines, letters).",
    ),
    (
        "text_fonts",
        "text.fonts",
        "List font families with their styles: the built-in single-line font, installed fonts and loaded font files. Args: query? (name contains).",
    ),
    (
        "text_monogram",
        "text.monogram",
        "Add a monogram: 1-4 initials centred on x, y with a frame. Args: initials, font, size? (pt, default 72), x, y, frame? (none, circle, square, diamond, scallop; default circle), padding_mm? (default 3).",
    ),
    (
        "group_create",
        "group.create",
        "Group shapes and/or groups (two or more members sharing one parent). Args: ids?, groups?, name?.",
    ),
    (
        "group_ungroup",
        "group.ungroup",
        "Dissolve a group; members move to its parent. Args: id.",
    ),
    (
        "group_rename",
        "group.rename",
        "Rename a group. Args: id, name.",
    ),
    (
        "group_members",
        "group.members",
        "Ids of every shape inside a group, nested groups included. Args: id.",
    ),
    (
        "group_of",
        "group.of",
        "The outermost group of a shape and the shapes a click on it selects. Args: id.",
    ),
    (
        "shapes_arrange",
        "shapes.arrange",
        "Stacking order within each shape's layer. Args: ids, order (front, back, forward, backward).",
    ),
    (
        "shapes_move_to_layer",
        "shapes.move-to-layer",
        "Move shapes to the top of a layer; they take its operation. Args: ids, layer.",
    ),
    (
        "layer_move",
        "layer.move",
        "Reorder layers. Args: from, to (indices).",
    ),
    (
        "layer_rename",
        "layer.rename",
        "Rename a layer. Args: index, name.",
    ),
    (
        "layer_set_locked",
        "layer.set-locked",
        "Lock or unlock a layer. Args: index, locked.",
    ),
    (
        "layer_set_template",
        "layer.set-template",
        "Make a layer a template (guide) layer: shown, never exported or cut. Args: index, template.",
    ),
    (
        "layer_set_color",
        "layer.set-color",
        "Set (#rrggbb) or clear (null) a layer's material colour. Args: index, color.",
    ),
    (
        "layer_color_sync",
        "layer.color-sync",
        "Color Sync: layers within `tolerance` (RGB distance 0-442) of an earlier layer take its colour; `merge` also merges layers that then share colour and operation.",
    ),
    (
        "page_add",
        "page.add",
        "Add a page beside the others. Args: preset? (letter, a4, mat-12x12, mat-12x24, joy-mat-4.5x6.5, joy-mat-4.5x12, joy-xtra-mat-8.5x12, custom), width_mm?, height_mm?, orientation? (portrait, landscape), name?, mat?, media?, color?.",
    ),
    (
        "page_set",
        "page.set",
        "Change a page (0 is the main board); omitted fields keep their value, an empty label clears it. Args: index plus page_add's args.",
    ),
    (
        "page_remove",
        "page.remove",
        "Remove an extra page (index >= 1). Args: index.",
    ),
    (
        "guide_add",
        "guide.add",
        "Add a ruler guide. Args: axis (horizontal: y = position, vertical: x = position), position_mm.",
    ),
    (
        "guide_move",
        "guide.move",
        "Move an unlocked guide. Args: index, position_mm.",
    ),
    (
        "guide_set_locked",
        "guide.set-locked",
        "Lock or unlock a guide. Args: index, locked.",
    ),
    (
        "guide_remove",
        "guide.remove",
        "Remove an unlocked guide. Args: index.",
    ),
    ("guide_clear", "guide.clear", "Remove every unlocked guide."),
    (
        "grid_set",
        "grid.set",
        "Grid spacing (0.01-10000 mm) and subdivisions (1-100). Args: spacing_mm, subdivisions, visible?.",
    ),
    (
        "snap_set",
        "snap.set",
        "Snapping targets. Args: grid?, guides?, objects?, spacing?, tolerance_mm?.",
    ),
    (
        "snap_point",
        "snap.point",
        "Snap a point, or a moving box (width/height > 0, x/y its smallest corner), to the grid, guides, shape and page edges/centres and equal spacing. Args: x, y, width?, height?, exclude? (ids), tolerance_mm?.",
    ),
    (
        "measure_distance",
        "measure.distance",
        "Distance and angle between two points. Args: from [x, y], to [x, y].",
    ),
    (
        "query_dimensions",
        "query.dimensions",
        "Combined bounds of shapes. Args: ids.",
    ),
    (
        "select_all",
        "select.all",
        "Ids of every selectable shape outside template layers.",
    ),
    ("select_none", "select.none", "The empty selection."),
    (
        "select_by_color",
        "select.by-color",
        "Selectable shapes on layers whose colour is within `tolerance` of `color` (#rrggbb).",
    ),
    (
        "select_by_operation",
        "select.by-operation",
        "Selectable shapes with an operation. Args: style (cut, engrave).",
    ),
    (
        "select_by_layer",
        "select.by-layer",
        "Selectable shapes on a layer. Args: index.",
    ),
    (
        "clipboard_copy",
        "clipboard.copy",
        "Copy shapes (and groups wholly inside them) to the engine clipboard. Args: ids.",
    ),
    (
        "clipboard_cut",
        "clipboard.cut",
        "Copy shapes, then remove them. Args: ids.",
    ),
    (
        "clipboard_paste",
        "clipboard.paste",
        "Paste. Args: mode? (offset (default), in-place, in-front), offset_mm? (default 5).",
    ),
    (
        "transform_each",
        "transform.each",
        "Move/scale/rotate shapes about their combined centre, or with `each` about each object's own centre (a group is one object); remembered for transform_again. Args: ids, dx_mm?, dy_mm?, scale_x?, scale_y?, degrees?, each?.",
    ),
    (
        "transform_again",
        "transform.again",
        "Repeat the last transform_each on shapes. Args: ids.",
    ),
    (
        "shapes_align_to_key",
        "shapes.align-to-key",
        "Align objects to a key shape that stays still. Args: ids, key, alignment (left, horizontal-center, right, top, vertical-center, bottom).",
    ),
    (
        "shapes_distribute_spacing",
        "shapes.distribute-spacing",
        "Space objects a fixed gap apart, keeping the first. Args: ids, axis (horizontal, vertical), gap_mm.",
    ),
    (
        "document_set_meta",
        "document.set-meta",
        "Set the document title and/or notes. Args: title?, notes?.",
    ),
    (
        "document_update_thumbnail",
        "document.update-thumbnail",
        "Store a fresh SVG thumbnail in the document.",
    ),
    (
        "templates_list",
        "templates.list",
        "The project templates for document_new_from_template: id, name, description, category (wizard, starter) and the params each reads.",
    ),
    (
        "document_new_from_template",
        "document.new-from-template",
        "Replace the document with a project template (ids from templates_list). Args: template, params? {preset?, width_mm?, height_mm?, fold? (vertical, horizontal), hole_mm?}.",
    ),
    (
        "shapes_slice",
        "shapes.slice",
        "Slice two overlapping closed shapes into every piece (a-b, b-a, a∩b; each region its own shape). Args: a, b.",
    ),
    (
        "shapes_weld",
        "shapes.weld",
        "Weld shapes into one outline for good. Args: ids.",
    ),
    (
        "shapes_attach",
        "shapes.attach",
        "Attach shapes: they keep their arrangement on one mat and cut together. Args: ids.",
    ),
    (
        "shapes_detach",
        "shapes.detach",
        "Detach the Attach groups the shapes are in. Args: ids.",
    ),
    (
        "shapes_flatten",
        "shapes.flatten",
        "Flatten shapes into one Print Then Cut image cut along their outline. Args: ids.",
    ),
    (
        "shapes_unflatten",
        "shapes.unflatten",
        "Split a flattened image or sticker back into its shapes. Args: id.",
    ),
    (
        "shape_contour",
        "shape.contour",
        "Hide or show contours (subpath indices) of a shape; hidden ones stay in the path but are not cut or exported. Args: id, contours, hidden.",
    ),
    (
        "shapes_combine",
        "shapes.combine",
        "Non-destructive Combine (unite, subtract, intersect, exclude), undone with shapes_uncombine. Args: ids, operation.",
    ),
    (
        "shapes_uncombine",
        "shapes.uncombine",
        "Restore a Combine's members where the combined shape is now. Args: id.",
    ),
    (
        "shape_set_combine",
        "shape.set-combine",
        "Change a Combine's operation. Args: id, operation.",
    ),
    (
        "shapes_create_sticker",
        "shapes.create-sticker",
        "Flatten shapes and add a welded offset border, cut as kiss-cut (default) or die-cut. Args: ids, border_mm? (default 3), cut?.",
    ),
    (
        "shapes_set_production",
        "shapes.set-production",
        "Set operation, line/fill colour, material or send tool assignment of shapes; clear resets fields. Args: ids, operation?, line_color?, fill_color?, material?, send?, clear?.",
    ),
    (
        "shapes_move_to_mat",
        "shapes.move-to-mat",
        "Put pieces on a given mat index (null: automatic). Args: ids, mat.",
    ),
    (
        "production_machines",
        "production.machines",
        "List machine profiles: mats, roll limits, Print Then Cut limits.",
    ),
    (
        "production_mats",
        "production.mats",
        "Lay pieces onto mats by material/colour/operation: copies, spacing, nesting with rotation steps, mirror per mat, mat-free.",
    ),
    (
        "production_job",
        "production.job",
        "Build the cut job for the machine: per-mat ops with tool holders, passes, cut order, overcut, test cut, weeding, origin, feed.",
    ),
    (
        "production_print_sheet",
        "production.print-sheet",
        "Print Then Cut sheets: registration marks, artwork and bleed per print mat (export_print_sheet writes PDF/PNG).",
    ),
];

fn mcp_tools() -> Vec<serde_json::Value> {
    use serde_json::json;
    let path = json!({"type": "string", "description": "File path on the server's filesystem"});
    let number = json!({"type": "number"});
    let specs = [
        (
            "doc_open",
            "Open a native StencilCraft document, or import a Silhouette Studio .studio/.studio3 document, replacing the current document.",
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
            "Encode the current document for a cutter without touching hardware and return the command transcript. Settings outside the model's documented limits are rejected. model is a machines_list id; optional: media (preset name/id), force, speed, tool, holder, depth, passes, overcut (mm), mat, media_size [w, l] mm, pen, track_enhancing, bbox, regmarks {width, length, x, y, search, quad}, order (none | speed | roller, default roller), interior_first (default true), closed_overcut (mm past the start of each closed contour, 0..5), mirror (flip about the media centre for heat-transfer vinyl), path (also write the byte stream there).",
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
                "order": json!({"type": "string", "enum": ["none", "speed", "roller"]}),
                "interior_first": json!({"type": "boolean"}),
                "closed_overcut": number,
                "mirror": json!({"type": "boolean"}),
                "path": path,
            }),
            json!(["model"]),
        ),
        (
            "import_file",
            "Import a file into the current document: SVG, DXF, PDF, EPS/PS/AI vector paths (added as cut shapes), or a PNG/JPEG/BMP/GIF/TIFF/WebP image (placed as an image object to trace). fit_to_page scales vectors to the board (else 1:1); join (default true) joins open paths whose ends meet within join_tolerance mm (default 0.05); dpi sizes images (default 96).",
            json!({"path": path, "fit_to_page": json!({"type": "boolean"}), "join": json!({"type": "boolean"}), "join_tolerance": number, "dpi": number}),
            json!(["path"]),
        ),
        (
            "export_pdf",
            "Export the current document as a one-page vector PDF (cut lines red, engrave blue).",
            json!({"path": path}),
            json!(["path"]),
        ),
        (
            "export_image",
            "Render the current document to PNG or JPEG (by the path's extension) at dpi pixels per inch (default 300; 600, 1200 or custom up to 4800).",
            json!({"path": path, "dpi": number}),
            json!(["path"]),
        ),
        (
            "export_print_sheet",
            "Write the Print Then Cut sheets (registration marks, artwork, bleed) as a PDF (all pages) or PNGs (one per sheet, -NN suffix when several). Print at 100%. Other args are production_print_sheet's options.",
            {
                let mut props = print_schema();
                if let Some(map) = props.as_object_mut() {
                    map.insert("path".to_string(), path.clone());
                    map.insert(
                        "format".to_string(),
                        json!({"type": "string", "enum": ["pdf", "png"]}),
                    );
                    map.insert(
                        "dpi".to_string(),
                        json!({"type": "number", "minimum": 50, "maximum": 1200}),
                    );
                }
                props
            },
            json!(["path"]),
        ),
        (
            "command_execute",
            "Execute an engine command. For example, command shape.add-rect uses params x, y, width, height in millimetres. See docs/mcp.md for the command format.",
            json!({"command": {"type": "string"}, "params": {"type": ["object", "null"]}}),
            json!(["command"]),
        ),
    ];
    let engine_specs = ENGINE_TOOLS.iter().map(|(name, command, description)| {
        let (properties, required) = engine_tool_schema(command);
        (*name, *description, properties, required)
    });
    specs
        .into_iter()
        .chain(engine_specs)
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

/// Native documents load as is; Silhouette Studio documents are imported, with the
/// reader's notes about skipped records returned alongside.
fn open_document(path: &str) -> Result<(stencilcraft_doc::Document, Vec<String>), ProtocolError> {
    match stencilcraft_doc::format::read_path(&[&stencilcraft_studio::StudioHandler], path)
        .map_err(ProtocolError::Bad)?
    {
        Some(read) => Ok((read.value, read.warnings)),
        None => stencilcraft_doc::load_from(path)
            .map(|doc| (doc, Vec::new()))
            .map_err(|e| ProtocolError::Bad(e.to_string())),
    }
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
    if let Some(v) = params.get("order").filter(|v| !v.is_null()) {
        job.order = v
            .as_str()
            .and_then(m::CutOrder::parse)
            .ok_or_else(|| bad("order must be none, speed or roller".to_string()))?;
    }
    if let Some(v) = params.get("interior_first").filter(|v| !v.is_null()) {
        job.interior_first = v
            .as_bool()
            .ok_or_else(|| bad("interior_first must be a boolean".to_string()))?;
    }
    if let Some(v) = params.get("closed_overcut").filter(|v| !v.is_null()) {
        job.closed_overcut_mm = v
            .as_f64()
            .ok_or_else(|| bad("closed_overcut must be a number of mm".to_string()))?;
    }
    job.mirror = flag("mirror");
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
        "order": job.order.id(),
        "interior_first": job.interior_first,
        "closed_overcut_mm": job.closed_overcut_mm,
        "mirror": job.mirror,
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
    fn engine_tools_are_listed_and_run_commands() {
        let mut session = Session::default();
        let call = |session: &mut Session, id: u32, name: &str, args: &str| -> serde_json::Value {
            let line = format!(
                r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{name}","arguments":{args}}}}}"#
            );
            serde_json::from_str(&session.handle_stdio_line(&line).unwrap().unwrap()).unwrap()
        };
        let list: serde_json::Value = serde_json::from_str(
            &session
                .handle_stdio_line(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        for (tool, _, _) in ENGINE_TOOLS {
            assert!(names.contains(tool), "{tool} not listed");
        }
        call(
            &mut session,
            2,
            "command_execute",
            r#"{"command":"shape.add-rect","params":{"x":0,"y":0,"width":5,"height":5}}"#,
        );
        let counted = call(&mut session, 3, "query_node_count", "{}");
        assert_eq!(counted["result"]["isError"], false);
        let text = counted["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"nodes\":4"), "{text}");
        let bad = call(
            &mut session,
            4,
            "path_break",
            r#"{"id":1,"contour":9,"node":0}"#,
        );
        assert_eq!(bad["result"]["isError"], true);
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
    fn doc_open_rejects_a_damaged_studio_file_and_survives() {
        let path = write_tmp("damaged.studio3", "silhouette05;\0\0\0 not really");
        let mut session = Session::default();
        let r = session
            .handle_line(&format!(
                r#"{{"id":1,"method":"doc.open","params":{{"path":"{path}"}}}}"#
            ))
            .unwrap()
            .unwrap();
        assert!(
            r.contains("\"ok\":false") && r.contains("Silhouette"),
            "{r}"
        );
        assert!(session.last_path.is_none());
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
    fn document_tools_run_engine_commands() {
        let mut session = Session::default();
        let call = |session: &mut Session, name: &str, args: &str| -> serde_json::Value {
            let line = format!(
                r#"{{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{{"name":"{name}","arguments":{args}}}}}"#
            );
            let value: serde_json::Value =
                serde_json::from_str(&session.handle_stdio_line(&line).unwrap().unwrap()).unwrap();
            assert_eq!(value["result"]["isError"], false, "{name}: {value}");
            serde_json::from_str(value["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
        };
        let templates = call(&mut session, "templates_list", "{}");
        assert!(templates["Document"]["Templates"].as_array().unwrap().len() >= 20);
        call(
            &mut session,
            "document_new_from_template",
            r#"{"template":"card"}"#,
        );
        assert_eq!(session.engine.doc.shape_count(), 2);
        let group = call(&mut session, "group_of", r#"{"id":1}"#);
        assert_eq!(
            group["Document"]["GroupOf"]["ids"],
            serde_json::json!([1, 2])
        );
        call(
            &mut session,
            "page_add",
            r#"{"preset":"a4","orientation":"landscape"}"#,
        );
        assert_eq!(session.engine.doc.pages.len(), 1);
        call(
            &mut session,
            "guide_add",
            r#"{"axis":"vertical","position_mm":10}"#,
        );
        call(&mut session, "guide_clear", "{}");
        let snapped = call(
            &mut session,
            "snap_point",
            r#"{"x":25.7,"y":300,"tolerance_mm":1}"#,
        );
        assert!(snapped["Document"]["Snap"]["x"].is_number());
        let all = call(&mut session, "select_all", "{}");
        assert_eq!(all["Selection"]["ids"], serde_json::json!([1, 2]));
        call(&mut session, "clipboard_copy", r#"{"ids":[1]}"#);
        call(&mut session, "clipboard_paste", r#"{"mode":"in-front"}"#);
        assert_eq!(session.engine.doc.shape_count(), 3);
        // Bad arguments are tool errors, not crashes.
        let line = r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"page_remove","arguments":{"index":0}}}"#;
        let value: serde_json::Value =
            serde_json::from_str(&session.handle_stdio_line(line).unwrap().unwrap()).unwrap();
        assert_eq!(value["result"]["isError"], true);
    }

    #[test]
    fn replicate_and_offset_tools_run_engine_commands() {
        let mut session = Session::default();
        let call = |session: &mut Session, id: u32, name: &str, args: &str| {
            let line = format!(
                r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{name}","arguments":{args}}}}}"#
            );
            let value: serde_json::Value =
                serde_json::from_str(&session.handle_stdio_line(&line).unwrap().unwrap()).unwrap();
            value["result"]["isError"].as_bool().unwrap()
        };
        let list: serde_json::Value = serde_json::from_str(
            &session
                .handle_stdio_line(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        for (tool, _, _) in ENGINE_TOOLS {
            assert!(names.contains(tool), "{tool} listed");
        }
        session
            .engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        assert!(!call(
            &mut session,
            2,
            "shapes_replicate_side",
            r#"{"ids":[1],"side":"right","gap_mm":1}"#
        ));
        assert!(!call(
            &mut session,
            3,
            "shapes_replicate_array",
            r#"{"ids":[1],"axis":"vertical","count":3,"gap_mm":0}"#
        ));
        assert!(!call(
            &mut session,
            4,
            "shapes_mirror_copy",
            r#"{"ids":[1],"side":"left","gap_mm":0}"#
        ));
        assert!(!call(
            &mut session,
            5,
            "shapes_rotate_copies",
            r#"{"ids":[1],"count":6}"#
        ));
        assert!(!call(
            &mut session,
            6,
            "shapes_offset",
            r#"{"ids":[1],"distance_mm":-1,"join":"round"}"#
        ));
        assert!(!call(
            &mut session,
            7,
            "shapes_replicate_fill",
            r#"{"ids":[1],"gap_mm":50}"#
        ));
        let shapes = session.engine.doc.shape_count();
        assert!(call(
            &mut session,
            8,
            "shapes_rotate_copies",
            r#"{"ids":[1],"count":0}"#
        ));
        assert!(call(
            &mut session,
            9,
            "shapes_offset",
            r#"{"ids":[1],"distance_mm":1,"join":"spiky"}"#
        ));
        assert!(call(
            &mut session,
            10,
            "shapes_replicate_side",
            r#"{"ids":[]}"#
        ));
        assert_eq!(session.engine.doc.shape_count(), shapes);
        // The same commands answer as legacy methods.
        let r = session
            .handle_line(r#"{"id":11,"method":"shapes.replicate-side","params":{"ids":[1],"side":"below","gap_mm":0}}"#)
            .unwrap()
            .unwrap();
        assert!(r.contains("\"ok\":true"), "{r}");
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
            .handle_line(r#"{"id":7,"method":"machine.dry_run","params":{"model":"cameo-4","order":"speed","interior_first":false,"closed_overcut":1.0,"mirror":true}}"#)
            .unwrap()
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v["ok"], true, "{r}");
        assert_eq!(v["result"]["order"], "speed");
        assert_eq!(v["result"]["mirror"], true);
        assert_eq!(v["result"]["interior_first"], false);
        for bad in [
            r#"{"id":8,"method":"machine.dry_run","params":{"model":"cameo-4","order":"zigzag"}}"#,
            r#"{"id":9,"method":"machine.dry_run","params":{"model":"cameo-4","closed_overcut":50}}"#,
        ] {
            let r = session.handle_line(bad).unwrap().unwrap();
            assert!(r.contains("\"ok\":false"), "{r}");
        }
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
    fn import_trace_and_export_tools() {
        let dir = std::env::temp_dir().join("stencilcraft-mcp-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("square.png");
        image::RgbaImage::from_fn(30, 30, |x, y| {
            if (10..20).contains(&x) && (10..20).contains(&y) {
                image::Rgba([0, 0, 0, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        })
        .save(&png)
        .unwrap();
        let mut session = Session::default();
        let call = |session: &mut Session,
                    name: &str,
                    args: serde_json::Value|
         -> serde_json::Value {
            let line = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args}});
            let out: serde_json::Value = serde_json::from_str(
                &session
                    .handle_stdio_line(&line.to_string())
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(out["result"]["isError"], false, "{name}: {out}");
            serde_json::from_str(out["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
        };
        let imported = call(
            &mut session,
            "import_file",
            serde_json::json!({"path": png.to_string_lossy()}),
        );
        let id = imported["images"][0].as_u64().unwrap();
        call(
            &mut session,
            "image_remove_background",
            serde_json::json!({"id": id, "tolerance": 5}),
        );
        call(
            &mut session,
            "image_trace",
            serde_json::json!({"id": id, "options": {"threshold": 50, "mode": "outer-edge"}}),
        );
        assert_eq!(session.engine.doc.shape_count(), 1);
        for (tool, name) in [
            ("export_pdf", "out.pdf"),
            ("export_image", "out.png"),
            ("export_image", "out.jpg"),
        ] {
            let path = dir.join(name);
            call(
                &mut session,
                tool,
                serde_json::json!({"path": path.to_string_lossy(), "dpi": 96}),
            );
            assert!(std::fs::metadata(&path).unwrap().len() > 0);
        }
        let bad = session
            .handle_line(r#"{"id":9,"method":"image.trace","params":{"id":424242}}"#)
            .unwrap()
            .unwrap();
        assert!(bad.contains("\"ok\":false"), "{bad}");
        let bad = session
            .handle_line(r#"{"id":9,"method":"export.image","params":{"path":"x.gif"}}"#)
            .unwrap()
            .unwrap();
        assert!(bad.contains("\"ok\":false"), "{bad}");
    }

    #[test]
    fn blank_lines_are_skipped() {
        let mut session = Session::default();
        assert!(session.handle_line("   ").unwrap().is_none());
        assert!(session.handle_line("\t\n").unwrap().is_none());
    }
}
