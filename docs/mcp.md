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
`export_dxf`, `export_gcode`, `export_pdf`, `export_image`, `import_file`, `image_trace`,
`image_remove_background`, `tile_run`, `tile_preview`, and `command_execute`. Their
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
| `export.pdf` | `{path}` | `{path}` |
| `export.image` | `{path, dpi?}` (`.png`, `.jpg`/`.jpeg`; dpi default 300, up to 4800) | `{path, dpi}` |
| `import.file` | `{path, fit_to_page?, join?, join_tolerance?, dpi?}` | `{shapes: [id], images: [id]}` |
| `image.trace` | `{id, options?, style?}` | `{ShapesAdded: {ids}}` |
| `image.remove_background` | `{id, tolerance}` | `{ImageChanged: {id}}` |
| `command.execute` | `{command: <command name>, params: <command parameters>}` | engine result |

## Import and trace

`import_file` adds a file to the **current** document (undoable): SVG, DXF, PDF, EPS/PS
and Illustrator `.ai` vector paths become cut shapes (as is, 1:1, unless `fit_to_page`
scales them to the board), with open paths whose ends meet within `join_tolerance` mm
(default 0.05) joined into polylines unless `join: false`. Raster files (PNG, JPEG, BMP,
GIF first frame, TIFF, WebP) are placed as an image object sized at `dpi` (default 96) to
trace; HEIC/HEIF is refused with a message. Files over 64 MiB, images over 10,000 px a
side or 16 million pixels, and more than 200,000 paths are errors, never crashes. Only
path geometry is read from vector files: fills, text, embedded images and symbol
instances are dropped.

`image_trace` turns a placed image into closed paths. `options` (all optional):

| Option | Range | Default | Meaning |
| --- | --- | --- | --- |
| `threshold` | 0–100 % | 50 | pixels darker than this (alpha over white) are traced |
| `despeckle` | px area | 4 | islands and pinholes smaller than this are dropped |
| `low_pass` | 0–50 px | 0 | blur radius before thresholding |
| `high_pass` | 0–50 px | 0 | edge filter radius (traces outlines of large dark areas) |
| `colour_range` | `{hue_min, hue_max, min_saturation}` | none | trace by hue (degrees, wraps when min > max) instead of brightness |
| `smoothing` | 0–50 px | 1 | simplification tolerance (0 keeps the pixel staircase) |
| `corner_threshold` | 0–180° | 60 | vertices turning at least this much stay sharp; others are rounded |
| `mode` | `all-edges`, `outer-edge`, `detach` | `all-edges` | outlines and holes; outlines only; or trace and remove the traced pixels from the image |

`style` is `cut` (default) or `engrave`. Outer contours have positive shoelace area in
document coordinates (y down) and holes negative. `image_remove_background` flood-fills
from the border through colours within `tolerance` % of the border's dominant colour and
makes them transparent. The underlying engine commands are `image.place`, `image.remove`,
`image.set-bounds {id, x, y, width, height}`, `image.trace` and `image.remove-background`,
all reachable through `command_execute` too.

`export_pdf` writes one vector page the size of the board (cut red, engrave blue; placed
images are not written yet). `export_image` renders the board, images included, to PNG or
JPEG at `dpi` (render capped at 64 million pixels).

