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
`export_dxf`, `export_gcode`, `tile_run`, `tile_preview`, and `command_execute`. Their
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

## Path editing, compound paths, flags and queries

Each command below is also a dedicated MCP tool (the command name with `.`/`-` turned into
`_`, e.g. `path_simplify`) whose arguments are the command's params, and can be applied
headlessly with `stencilcraft-cli run --in IN --command '<json>' … --export OUT`
(repeatable; each effect is printed as one JSON line). Every editing command is one undo
step (`edit.undo`); queries are not recorded. Bad ids, indices or numbers return an error
and leave the document unchanged. A shape that is locked rejects every editing command
until `shapes.set-locked {locked: false}`.

A node is addressed as `{"contour": c, "node": n}`: contour `c` of the shape's path and
node `n` along it, counted as `query.node-count` counts them (a closed contour does not
repeat its first node). Segment `s` runs from node `s` to the next.

| Command / tool | Params | Effect |
| --- | --- | --- |
| `path.break` / `path_break` | `{id, contour, node}` | Closed contour opens at the node; open one splits in two (pieces stay in the shape). |
| `path.join` / `path_join` | `{a, b, a_contour?, b_contour?, a_end?, b_end?}` | Joins two open contours (`b` consumed; may be another contour of `a`). Ends `start`/`end` default to the nearest pair; coincident ends merge, otherwise a line joins them. |
| `path.close` / `path_close` | `{id, contour?}` | Closes an open contour. |
| `path.average-nodes` / `path_average_nodes` | `{id, nodes, axis}` | `axis`: `horizontal` (same y), `vertical` (same x), `both`. |
| `path.merge-nodes` / `path_merge_nodes` | `{id, nodes}` | Moves nodes to their centroid; neighbours on a contour fuse into one corner node. |
| `path.delete-nodes` / `path_delete_nodes` | `{id, nodes}` | Removes nodes, refitting the neighbouring curve. Deleting the whole path is an error. |
| `path.set-node-type` / `path_set_node_type` | `{id, nodes, kind}` | `corner` retracts handles; `smooth` makes collinear handles. |
| `path.set-segment` / `path_set_segment` | `{id, contour, segment, kind}` | `line` or `curve` (handles placed on the line, so the shape is unchanged until edited). |
| `path.simplify` / `path_simplify` | `{ids, tolerance_mm}` | Refits with fewer nodes within `tolerance_mm` (0 < t ≤ 1000) of the original. |
| `path.outline-stroke` / `path_outline_stroke` | `{ids, width_mm, join?, cap?, miter_limit?}` | Replaces each path with the filled outline of its stroke. `join`: `miter` (default), `round`, `bevel`; `cap`: `butt` (default), `round`, `square`; miter limit 1–100 (default 4). |
| `shapes.to-path` / `shapes_to_path` | `{ids}` | Live rectangle/ellipse/polygon/star primitives become plain paths. |
| `shapes.make-compound` / `shapes_make_compound` | `{ids}` | One compound path in the bottom-most shape's id and style. |
| `shapes.release-compound` / `shapes_release_compound` | `{ids}` | Release Compound Path / Detach: one shape per contour; returns `ShapesAdded` with every id. |
| `shape.pathfinder` / `shape_pathfinder` | `{ids, operation}` | Adds `subtract-all` (every other shape subtracted from the bottom one) to `union`, `difference`, `intersect`, `xor` (exclude), `divide`, `trim`, `merge`, `crop`, `outline`, `minus-back`. |
| `shapes.set-locked` / `shapes_set_locked` | `{ids, locked}` | Locked shapes reject edits and are left out of selections. |
| `shapes.set-hidden` / `shapes_set_hidden` | `{ids, hidden}` | Hidden shapes are not exported (SVG, DXF, G-code, tiling, cut) or selected. |
| `query.node-count` / `query_node_count` | `{ids?}` | `{"NodeCount": {shapes, contours, nodes, linear, bezier}}`; all shapes when `ids` is empty. A node with any handle counts as bezier. |
| `select.same-fill` / `select_same_fill` | `{id}` | `{"Selection": {ids}}`: selectable shapes with the same operation colour that are likewise filled (closed) or open. |
| `select.same-stroke` / `select_same_stroke` | `{id}` | Selectable shapes with the same operation colour (every outline is stroked in it). |
| `select.inverse` / `select_inverse` | `{ids}` | Selectable shapes not in `ids`. |

"Selectable" means not hidden, not locked, and on a visible layer. The document stores one
operation colour (cut/engrave) per shape, so fill and stroke differ only in whether open
paths count.

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

