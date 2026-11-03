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
`export_dxf`, `export_gcode`, `tile_run`, `tile_preview`, `command_execute`, and the
replicate/offset tools `shapes_replicate_side`, `shapes_replicate_array`,
`shapes_replicate_fill`, `shapes_mirror_copy`, `shapes_rotate_copies` and `shapes_offset`
(see [Replicate and offset](#replicate-and-offset)). Their
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
| `command.execute` | `{command: <command name>, params: <command parameters>}` | engine result |
| `shapes.replicate-side` | `{ids, side, gap_mm}` | `{ShapesAdded: {ids}}` |
| `shapes.replicate-array` | `{ids, axis, count, gap_mm}` | `{ShapesAdded: {ids}}` |
| `shapes.replicate-fill` | `{ids, gap_mm}` | `{ShapesAdded: {ids}}` |
| `shapes.mirror-copy` | `{ids, side, gap_mm}` | `{ShapesAdded: {ids}}` |
| `shapes.rotate-copies` | `{ids, count}` | `{ShapesAdded: {ids}}` |
| `shapes.offset` | `{ids, distance_mm, join?, miter_limit?}` | `{ShapesAdded: {ids}}` |

## Replicate and offset

These are engine commands (`crates/engine/src/lib.rs`), so each is one undoable edit
(`edit.undo` removes every copy it added). Each has a dedicated MCP tool whose arguments
are the command's params, and can also be sent through `command_execute` or as a legacy
method of the same name. Copies keep each shape's cut/engrave style. Documents are y-up,
so `above` means larger y.

| Command | Behaviour |
| --- | --- |
| `shapes.replicate-side` | One copy of the selection on `side` (`left`, `right`, `above`, `below`): its bounds touch the selection's bounds, `gap_mm` apart. |
| `shapes.replicate-array` | A row (`axis: horizontal`, growing right) or column (`vertical`, growing up) of `count` items; the selection is the first, so `count - 1` copies are added. |
| `shapes.replicate-fill` | Every cell of the selection's grid (pitch = selection size + `gap_mm`) that lies wholly on the board gets a copy; the selection keeps its own cell. |
| `shapes.mirror-copy` | A mirrored copy on `side`: `left`/`right` copies flip horizontally, `above`/`below` vertically, then sit edge to edge `gap_mm` away. |
| `shapes.rotate-copies` | A radial array of `count` items (the selection included) rotated in steps of 360°/`count` about the selection's bounds centre. |
| `shapes.offset` | Adds an offset outline of each shape, keeping the original. Positive `distance_mm` grows the solid; negative is an internal offset (outlines shrink, holes grow). `join` is `miter` (default), `round` or `bevel`; `miter_limit` (default 4, at least 1) is the corner limit as a multiple of the distance. The outline is flattened to line segments. |

Limits: `gap_mm` must be finite and non-negative; `count` is 2–1000; one command adds at
most 10,000 shapes (a fill that would need more is refused — raise the gap); ids must be
unique and exist; copies must stay within finite coordinates. A refused command changes
nothing and adds no history.

The CLI reaches the same commands with `stencilcraft-cli run --command JSON` (repeatable,
run in order on the loaded document before kerf and export), for example
`--command '{"command":"shapes.replicate-fill","params":{"ids":[1],"gap_mm":2}}'`. CLI
kerf compensation takes `--kerf-join miter|round|bevel` and `--kerf-limit N` beside
`--apply-kerf MM`; miter with limit 4 stays the default. The MCP `kerf_apply` tool only
records the kerf width on the document; offsetting with a chosen join is `shapes_offset`.

The desktop app's `--control` socket also answers two window methods the headless session
cannot (legacy envelope only):

| Method | Params | Result |
| --- | --- | --- |
| `app.screenshot` | `{path}` | `{path, size: [w, h]}`: the next painted frame as a PNG |
| `app.select` | `{ids}` | `{selected}`: sets the canvas selection (unknown ids are dropped) |

A successful `doc.open` or `doc.save` through the socket also updates the window title and
the recent-files list, as File › Open and Save do.

