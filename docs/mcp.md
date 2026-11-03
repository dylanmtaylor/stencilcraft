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

## Generators and effects

Pure geometry lives in `crates/generate`; the commands are in `crates/engine/src/generate.rs`.
Each is also an MCP tool (the command name with `.`/`-` turned into `_`, e.g.
`generate_fill`) and one undo step. Bad params, missing or locked ids, or results over the
caps (20,000 stones/dots/dashes/lines/pieces, 500,000 points, 10,000 copies) return an error
and change nothing. Shapes made by an effect are plain paths: one compound shape per source
(a fill, a stone template, the punch points), which `generate.release` splits into one plain
shape per contour with its tool tag cleared ("Release Effects"/"release fill"/"release
copies"). Nothing stays live-linked to its source.

Tool tags. A shape may carry `tool` (`{"op": ...}`) next to its cut/engrave style; a machine
backend is meant to run that operation instead of a plain pass (exports and the machine
crate do not read the tag yet; until they do, a tagged path goes out by its style):
`score {tip: fine|wide, passes 1-10}`, `emboss {tip, passes, deboss: bool}`,
`power-engrave`, `heat-pen` (foil), and `punch {diameter_mm}`. Tagged paths sit on an
engrave layer except `punch`, whose path is the hole circle (shown and, without a punch
tool, cut): a backend with a punch plunges once per contour at its bounds centre.

| Command / tool | Params | Effect |
| --- | --- | --- |
| `generate.shape` | `{shape: {kind, ...}, x, y, width, height, replace?}` | Adds the shape fitted to the box (`ShapesAdded`); with `replace: id` regenerates that shape in place (`ShapesEdited`, same id), which is how its parameters are edited. Kinds and params: `heart`; `arrow {head_length 0.4, shaft_width 0.4}`; `speech-bubble {tail_height 0.25, tail_width 0.2, corner_radius 0.15}`; `ring {inner_ratio 0.6}`; `crescent {thickness 0.35}`; `gear {teeth 12, tooth_depth 0.15, hole_ratio 0}`; `spiral {turns 3}` (open); `banner {notch 0.12}`; `scallop-circle {scallops 12}`; `scallop-square {scallops 4}` (per side); `triangle {variant: isosceles\|right\|equilateral}`. Rectangle, rounded rectangle, ellipse, polygon and star are the existing `shape.add-*` primitives. |
| `generate.score-line` | `{x0, y0, x1, y1, tip?, passes?}` | A score line (engrave, `score` tag). |
| `generate.fill` | `{ids, pattern?, spacing_mm?, angle_deg?, inset_mm?, dot_mm?, seed?, tool?}` | Sketch fill of each closed shape (even-odd: holes stay empty), as engrave paths. `pattern`: `lines` (default), `cross-hatch` (angle ± 45°), `grid` (angle and angle + 90°), `concentric`, `spiral`, `stipple-grid`, `stipple-random` (seeded jitter). `spacing_mm` default 0.4064 (0.016 in), `angle_deg` 45, `dot_mm` 0.5. `tool` tags the result for power engraver, heat pen (foil) or emboss/deboss fills. |
| `generate.tool-lines` | `{ids, tool, fill?}` | Score & emboss lines: each outline becomes an engrave path tagged `tool` (new id); `fill` (fill params) adds an interior fill with the same tool. |
| `generate.rhinestone` | `{ids, size?, stone_mm?, spacing_mm?, layout?, lines?, pattern?}` | Stone-hole circles (cut). `size` `ss6` 2.0, `ss8` 2.4, `ss10` 2.8 (default), `ss12` 3.1, `ss16` 3.9, `ss20` 4.7, `ss30` 6.4 mm, or `stone_mm`; `spacing_mm` gap (default 1); `layout` `outline` (default), `fill`, `outline-fill`; `lines` 1–2; `pattern` `hex` (default) or `grid`. Stones never overlap. |
| `generate.punch` | `{ids, spacing_mm?, diameter_mm?}` | Replaces each path with punch points every 3.5 mm (default) along it, `diameter_mm` 1, 3 (default) or 5 (0.5–20 accepted), tagged `punch`. |
| `generate.popup` | `{x?, y?, width?, height?, struts?, strut_width_mm?, base_width_pct?, dash_pitch_mm?, outline?}` | A pop-up card spread (default 200 × 140 mm) with a vertical centre fold: card outline and two cuts per strut (cut shape), fold lines (score-tagged shape). `base_width_pct` is how far struts reach from the fold as a % of half the card; `dash_pitch_mm` > 0 dashes the scores. |
| `generate.puzzle` | `{id, rows?, columns?, tab_size?, seed?}` | Jigsaw cut lines inside the shape (its outline is the border): rows × columns (default 4 × 5, up to 100 each), round knobs `tab_size` (0–0.35 of the smaller cell side, default 0.25) that never touch. |
| `generate.weed` | `{ids, margin_mm?, spacing_mm?, direction?, clearance_mm?}` | A weeding box `margin_mm` (default 3) round the shapes, plus weed lines every `spacing_mm` (0 = none) `vertical`, `horizontal` or `both`, broken around the design and stopping `clearance_mm` (default 1) short of it. |
| `generate.registration` | `{ids, margin_mm?, arm_mm?, mark?, style?}` | Alignment marks at the corners of the shapes' bounds grown by `margin_mm` (5): `corners` brackets or `crosshair`, arms `arm_mm` (8), `cut` (default) or `engrave`. |
| `generate.warp` | `{ids, style?, bend?, horizontal?, vertical?}` | Warps each shape in place (same id) inside its bounds. Styles: `arc` (default), `arc-lower`, `arc-upper`, `arch`, `bulge`, `shell-lower`, `shell-upper`, `flag`, `wave`, `fish`, `rise`, `fisheye`, `inflate`, `squeeze`, `twist`, `none`; `bend` (0.5), `horizontal`, `vertical` (perspective) each −1…1. The result is a dense polyline (`path.simplify` refits it). Preview: call `stencilcraft_generate::warp::warp` directly, or run then `edit.undo`. |
| `generate.perforate` | `{ids, cut_mm, gap_mm}` | Each path becomes dashes in place (perforated cut); closed contours get a whole number of dashes. |
| `generate.wavy` | `{ids, amplitude_mm, wavelength_mm}` | Each path becomes a sine wave around itself in place; closed contours hold whole periods. |
| `generate.object-on-path` | `{ids, path_id, perpendicular?, start_angle_deg?, increment_angle_deg?, start_position_mm?, section_length_mm?, count?, step_mm?}` | Copies of the shapes `ids` (as a group, by their bounds centre) along the first contour of `path_id`: rotated to the tangent when `perpendicular` (default true), plus `start_angle_deg` and `increment_angle_deg` per copy; from `start_position_mm` over `section_length_mm` (0 = the whole path); `count` copies (default 50) or one every `step_mm`. The copies are independent shapes. |
| `generate.replicate` | `{ids, direction, angle_deg?, count?, gap_mm?}` | Advanced Replicate: `count` copies (1–10,000) `on-top` (stacked exactly), `left`, `right`, `above`, `below` or along `angle_deg` (`direction: angle`), each clearing the last by `gap_mm`. |
| `generate.release` | `{ids}` | Splits each shape into one plain shape per contour and clears tool tags. |

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

