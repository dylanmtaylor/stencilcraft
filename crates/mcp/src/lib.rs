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
    let integer = json!({"type": "integer"});
    let boolean = json!({"type": "boolean"});
    let string = json!({"type": "string"});
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
            "export_print_sheet",
            "Write the Print Then Cut sheets (registration marks, artwork, bleed) as a PDF (all pages) or PNGs (one per sheet, -NN suffix when several). Print at 100%. Other args are production_print_sheet's options.",
            {
                let mut props = print_schema();
                if let Some(map) = props.as_object_mut() {
                    map.insert("path".to_string(), path.clone());
                    map.insert("format".to_string(), json!({"type": "string", "enum": ["pdf", "png"]}));
                    map.insert("dpi".to_string(), json!({"type": "number", "minimum": 50, "maximum": 1200}));
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
            assert!(names.contains(&tool), "{tool} listed");
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
    fn blank_lines_are_skipped() {
        let mut session = Session::default();
        assert!(session.handle_line("   ").unwrap().is_none());
        assert!(session.handle_line("\t\n").unwrap().is_none());
    }
}
