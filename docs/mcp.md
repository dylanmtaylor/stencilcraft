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
(see [Replicate and offset](#replicate-and-offset)), and the cut-production tools (see
[Production](#production)). Their
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

## Production

Cut-production commands (`crates/engine/src/production.rs`, algorithms in `crates/layout`)
cover Cricut Design Space's layer actions and Make screen and Silhouette Studio's Send
settings, from our own design. Each is an engine command (one undo step) with a dedicated MCP
tool (the name with `.`/`-` turned into `_`); the `production.*` queries change nothing. Shape
production data (operation, colours, material, attach group, manual mat, hidden contours,
Combine/Flatten members, send-preview tool) is saved in the `.stencilcraft` file under each
shape's `production` key and follows moves, rotations and scaling.

| Command / tool | Params | Effect |
| --- | --- | --- |
| `shapes.slice` | `{a, b}` | Two overlapping closed shapes become every piece (a − b, b − a, a ∩ b; each separate region its own shape). The overlap takes the top shape's settings. `ShapesAdded`. |
| `shapes.weld` | `{ids}` (≥ 2) | One outline for good; the first shape in document order keeps its id. |
| `shapes.attach` / `shapes.detach` | `{ids}` | Attached shapes stay in their arrangement on one mat and cut together (draw/score layers ride with their cut layer). Detach releases the whole group. |
| `shapes.flatten` / `shapes.unflatten` | `{ids}` / `{id}` | Flatten: one Print Then Cut image (members kept as artwork) cut along the members' outline. Unflatten restores them where the image is now. |
| `shape.contour` | `{id, contours, hidden}` | Hide or show contours (subpath indices). Hidden contours stay in the path but every export (SVG, DXF, G-code, tiling, cut, cut jobs) leaves them out. |
| `shapes.combine` | `{ids, operation}` | Non-destructive `unite`, `subtract` (back-most minus the rest), `intersect`, `exclude`; the result keeps its members. `ShapeAdded`. |
| `shape.set-combine` / `shapes.uncombine` | `{id, operation}` / `{id}` | Change the operation; or restore the members (they follow any move of the group). |
| `shapes.create-sticker` | `{ids, border_mm?, cut?}` | Flatten plus a welded offset border (0.5–50 mm, default 3; holes filled), cut `kiss-cut` (default, backing intact) or `die-cut`. |
| `shapes.set-production` | `{ids, operation?, line_color?, fill_color?, material?, send?, clear?}` | `operation`: `cut`, `kiss-cut`, `die-cut`, `draw`, `score`, `engrave`, `perforate`, `print-then-cut`, `guide` (never sent). Colours take CSS forms and are stored as `#rrggbb`. `send`: `tool1`, `tool1-outline`, `tool2`, `tool2-outline`, `ignore`. `clear` lists fields to reset. |
| `shapes.move-to-mat` | `{ids, mat}` | Put pieces on mat `mat` (an existing mat is re-laid with them; a higher number makes a new mat); `null` is automatic. |
| `production.machines` | `{}` | Machine profiles: mat cut areas, roll width/length limits and roller margins, Print Then Cut maximum, mark style. |
| `production.mats` | mat options | `{"Production": {"Mats": …}}`: the mat layout. |
| `production.job` | job options | `{"Production": {"Job": …}}`: per-mat cut operations for the machine layer. |
| `production.print-sheet` | print options | `{"Production": {"PrintSheets": […]}}`: one page per Print Then Cut mat. |

**Mat options** (all optional): `machine` (`cricut-maker` default, `cricut-explore`,
`cricut-joy`, `cricut-joy-xtra`, `cricut-venture`, `silhouette-cameo`,
`silhouette-cameo-plus`, `silhouette-cameo-pro`, `silhouette-portrait`), `mat` (size name;
default the smallest that fits the largest piece), `mat_free` (roll feed: length reported
with roller margins and the minimum length), `copies` (1–100), `spacing_mm` (0–100, default
3), `nest` (skyline packing, big pieces first; off is Cricut's down-the-mat columns),
`rotation_step_deg` (0 or 1–180, default 90; at most 36 angles), `mirror_on` / `mirror_off`
(mat indices; iron-on, HTV and Infusible Ink materials mirror automatically), `page`
(`width_mm`, `height_mm` default US Letter; `marks` `silhouette`/`cricut`/`four-corner`;
`mark_length_mm` 20, `mark_thickness_mm` 0.5, `mark_inset_mm` 15.9). Pieces are grouped onto
mats by print-then-cut / material / colour. A piece too big for the mat (or for the machine's
Print Then Cut maximum) is an error naming the shape and the limit. Mat coordinates are mm
from the top-left of the cut area, y down the mat; each placement carries its
document → mat `transform`.

**Job options**: `mats` (above), `cut_by` (`operation`, `line-color`, `fill-color`, `layer`),
`groups` (`[{key, tool 1|2, enabled, passes?, overcut_mm?}]` per group key), `order`
(`minimize-roller` default, `document`, `by-color`, `by-layer`, `shortest-travel`),
`inside_first` (default true: counters before outlines), `overcut_mm` (0–5),
`passes` (1–10), `test_cut` (`{mat, x_mm, y_mm, size_mm}`; must be on the mat and clear of
pieces), `weed` (`{margin_mm, lines}`: a box and lines through the gaps), `origin`
(`top-left` …), `feed` (`length` or `width`), `after` (`return-to-origin` or `feed`),
`stipple_order` (`default`, `edge-to-center`, `center-to-edge` for dots under 0.5 mm).
Each op has `shape_id`, `copy`, `contour`, `group`, `operation`, `tool`, `passes`, `closed`
and `path`; Print Then Cut mats also carry their `registration` marks.

**Print options**: `mats`, `bleed` (default on), `bleed_mm` (0–10, default 1.27). The
`export_print_sheet` tool (legacy `export.print-sheet`) writes them: `{path, format: pdf|png,
dpi?}` plus the print options; PDF is one page per sheet at actual size; PNG is one file per
sheet (`-NN` suffix when there are several, 50–1200 dpi).

The desktop app's `--control` socket also answers two window methods the headless session
cannot (legacy envelope only):

| Method | Params | Result |
| --- | --- | --- |
| `app.screenshot` | `{path}` | `{path, size: [w, h]}`: the next painted frame as a PNG |
| `app.select` | `{ids}` | `{selected}`: sets the canvas selection (unknown ids are dropped) |

A successful `doc.open` or `doc.save` through the socket also updates the window title and
the recent-files list, as File › Open and Save do.

