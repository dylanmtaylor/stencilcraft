# Control protocol (CLI JSON lines / MCP)

Two agent-facing surfaces share one core (`crates/mcp`):

- an **MCP stdio server** (`stencilcraft-cli mcp`), also accepting legacy JSON-lines requests;
- the desktop app's `--control` socket, which accepts both envelopes and runs each request
  against the document on screen.

The MCP server supports protocol version `2025-03-26`, initialization, ping, tool discovery,
and tool calls. Add it to an MCP client with `stencilcraft-cli mcp` as the server command.
Messages are newline-delimited JSON-RPC; notifications receive no response. Tool results
are JSON encoded as text content, and operation errors are returned with `isError: true`.
The stdio server's session is headless. The desktop app's `--control` socket is different: it
queues each request onto the UI thread and handles it against the engine behind the visible
canvas, so `doc.summary` reports the open document, edits are undoable in the app, and the
window repaints. Requests are answered between frames, and only while the window is running.
`stencilcraft-cli mcp --control 7979` runs the same server as a bridge: tool calls are
forwarded to the app's `--control` socket instead of a private headless session, so an agent
opening `stencilcraft-cli mcp --control …` in an MCP client edits the document the user can
see. File paths refer to the server's filesystem; tools can read and write files accessible
to the process. The control socket listens on `127.0.0.1` only, but it has no authentication: keep it
on a trusted machine and review what connected clients do.

The MCP tool names are `doc_open`, `doc_save`, `doc_summary`, `kerf_apply`, `export_svg`,
`export_dxf`, `export_gcode`, `tile_run`, `tile_preview`, `machines_list`, `machine_capabilities`,
`machine_dry_run`,
and `command_execute`. Their
arguments correspond to the legacy method parameters below. `tile_run` takes an optional
`marks: true` to stamp corner registration brackets and edge overlap guides on each sheet;
`tile_preview` writes the whole tiling to one labelled SVG page (`columns`, `scale`,
`marks`) so a client can look at the sheets before anything is cut. `export_gcode` takes
the cut profile: `feed` (mm/min), `power` and `engrave_power` (0–1000), `passes` (1–10),
`ramp` (power climbs to the last pass) and `mode` (`dynamic` for `M4`, `constant` for
`M3`). Unset knobs keep the documented defaults; out-of-range values come back as a normal
tool error naming the knob. For example, `command_execute` accepts
`{"command":"shape.add-rect","params":{"x":0,"y":0,"width":5,"height":5}}`;
see `crates/engine/src/lib.rs` for the complete serde-tagged command set. Undo and redo
accept `{"command":"edit.undo"}` and `{"command":"edit.redo"}`.

Legacy requests use one line in and one line out; logging goes to stderr only.

## Legacy envelope

```json
{"id": 1, "method": "doc.open", "params": {"path": "examples/starter.stencilcraft"}}
```

Response:

```json
{"id": 1, "ok": true, "result": {"path": "examples/starter.stencilcraft"}}
```

On failure:

```json
{"id": 1, "ok": false, "error": "open: examples/x.stencilcraft: no such file"}
```

Every request with a known `method` must produce exactly one response with the same `id`.
Unknown methods produce `{"ok": false, "error": "unknown method: ..."}`. Malformed JSON
produces a response with `id: null`. No request may abort the session on a bad parameter.
(Vectors: `crates/mcp/tests/`.)

## Methods

| Method | Params | Result |
| --- | --- | --- |
| `doc.open` | `{path}` | `{layers: n, shapes: n}` |
| `doc.save` | `{path}` | `{path}` |
| `doc.summary` | `{}` | `{layers, shapes, kerf_mm, board: [w, h]}` |
| `kerf.apply` | `{width_mm}` | `{width_mm}` |
| `export.svg` | `{path}` | `{path}` |
| `export.dxf` | `{path}` | `{path}` |
| `export.gcode` | `{path, feed?, power?, engrave_power?, passes?, ramp?, mode?}` | `{path}` |
| `tile.run` | `{path, sheet_w, sheet_h, overlap, marks?}` | `{tiles: n}` |
| `tile.preview` | `{path, sheet_w, sheet_h, overlap, columns?, scale?, marks?}` | `{path, tiles: n}` |
| `machines.list` | `{}` | array of machines: `{id, name, vendor, protocol, support, usb, firmware, cut_width_mm, cut_length_mm, media_width_mm, mats, tool_holders, tools, max_force_gf, force{min,max,unit}, speed{min,max,unit}, roll_feed, bluetooth, regmarks, quad_regmarks, overcut, multi_pass, notes}` (`null` = undocumented) |
| `machine.capabilities` | `{model}` | that machine's full capabilities row |
| `machine.dry_run` | `{model, media?, force?, speed?, tool?, holder?, depth?, passes?, overcut?, mat?, media_size?: [w, l], pen?, track_enhancing?, bbox?, regmarks?: {width, length, x, y, search?, quad?}, path?}` | `{model, protocol, commands, bytes, bbox_mm, registration_wait, truncated, transcript: [..], path}` |
| `command.execute` | `{command: <command name>, params: <command parameters>}` | engine result |


## Cutting machines

`machines_list` returns every registry row (`crates/machine/machines.toml`, see
`docs/machine-architecture.md`), including Cricut slots marked `unsupported`;
`machine_capabilities` returns one. Force/speed are in the machine's own units
(`force.unit` / `speed.unit`). `machine_dry_run` encodes the
current document for one model and returns the transcript (first 5000 commands; `truncated`
says if there were more); with `path` it also writes the exact byte stream (HP-GL/DMPL files
for a spooler, for example). Every value is validated against the model: an out-of-range
force, speed, depth, tool, holder, mat or a design outside the cut area is a normal tool error
naming the knob and range — never clamped. There is deliberately **no** MCP method that sends
to hardware; a real send is only `stencilcraft-cli cut --device …`, run by a person.
