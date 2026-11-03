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

Legacy requests use one line in and one line out; logging goes to stderr only.

## Document and canvas commands

Engine commands in `crates/engine/src/document.rs`, each with a dedicated MCP tool (the
command name with `.`/`-` turned into `_`, e.g. `group.create` → `group_create`) whose
arguments are the command's params; `command_execute` takes them too. Edits are one
undoable step each; queries (marked Q) and `clipboard.copy` add no history. A refused
command changes nothing. Millimetres throughout; the canvas draws y downwards, so `top`
means the smaller y.

| Command | Params | Behaviour |
| --- | --- | --- |
| `group.create` | `{ids?, groups?, name?}` | Groups two or more shapes/groups that share one parent (or are all top-level); the new group takes their place. Answers `{GroupCreated: {id}}`. |
| `group.ungroup` | `{id}` | Members move up to the group's parent. Answers the shapes it held. |
| `group.rename` | `{id, name}` | |
| `group.members` (Q) | `{id}` | Every shape in the group, nested groups included. |
| `group.of` (Q) | `{id}` | `{GroupOf: {group, ids}}`: the shape's outermost group and its shapes (what a click selects). |
| `shapes.arrange` | `{ids, order}` | `front`, `back`, `forward`, `backward`, within each shape's layer; a selected block moves as one. |
| `shapes.move-to-layer` | `{ids, layer}` | To the top of the layer; shapes take its operation. A locked layer refuses. |
| `layer.move` | `{from, to}` | Reorder layers. |
| `layer.rename` | `{index, name}` | Name must not be blank. |
| `layer.set-locked` | `{index, locked}` | Locked layers' shapes cannot be selected or edited (also by `lib.rs` commands); new shapes skip them. |
| `layer.set-template` | `{index, template}` | Template (guide) layers show on the canvas but are left out of SVG/DXF/G-code export, tiling, cutting and thumbnails; new shapes skip them. |
| `layer.set-color` | `{index, color}` | `#rrggbb` (normalised to lowercase) or `null`. |
| `layer.color-sync` | `{tolerance, merge?}` | Color Sync: each coloured, non-template layer within `tolerance` (RGB distance 0-442) of an earlier one takes its colour; `merge` also merges unlocked layers that then share colour and operation into the first. |
| `page.add` | `{preset?, width_mm?, height_mm?, orientation?, name?, mat?, media?, color?}` | Adds a page right of the others (10 mm gap). Presets: `letter`, `a4`, `mat-12x12`, `mat-12x24`, `joy-mat-4.5x6.5`, `joy-mat-4.5x12`, `joy-xtra-mat-8.5x12`, `custom` (needs width and height). Mat presets fill `mat`. Answers `{PageAdded: {index}}`. |
| `page.set` | `{index, …page.add fields}` | Page 0 is the main board (its size is `board`). Omitted fields keep their value; an empty label clears it. |
| `page.remove` | `{index}` | Extra pages only (index ≥ 1); shapes stay where they are. |
| `guide.add` | `{axis, position_mm}` | `horizontal` is the line y = position, `vertical` x = position. At most 1000. |
| `guide.move` / `guide.remove` | `{index, position_mm}` / `{index}` | Refused for a locked guide. |
| `guide.set-locked` | `{index, locked}` | |
| `guide.clear` | `{}` | Removes unlocked guides; answers `{GuidesCleared: {removed}}`. |
| `grid.set` | `{spacing_mm, subdivisions, visible?}` | Spacing 0.01-10000 mm, subdivisions 1-100; snapping uses spacing / subdivisions. |
| `snap.set` | `{grid?, guides?, objects?, spacing?, tolerance_mm?}` | Defaults: grid off, the rest on, 2 mm. |
| `snap.point` (Q) | `{x, y, width?, height?, exclude?, tolerance_mm?}` | A point, or a moving box whose left/centre/right and top/centre/bottom snap. Answers `{Snap: {x, y, dx, dy, snapped_x, snapped_y, lines}}`; each axis snaps to its nearest target (`guide`, `edge`, `center`, `page`, `spacing`, `grid`) within tolerance, and `lines` are the alignment or equal-spacing markers to draw. Also callable as the pure function `stencilcraft_engine::document::snap`. |
| `measure.distance` (Q) | `{from: [x, y], to: [x, y]}` | `{Measure: {distance_mm, dx_mm, dy_mm, angle_degrees}}`, angle from +x towards +y. |
| `query.dimensions` (Q) | `{ids}` | `{Dimensions: {x, y, width, height}}` of the combined bounds. |
| `select.all` / `select.none` (Q) | `{}` | Selectable shapes outside template layers / nothing. |
| `select.by-color` (Q) | `{color, tolerance?}` | Shapes on layers with a colour within `tolerance` of `color`. |
| `select.by-operation` (Q) | `{style}` | `cut` or `engrave`, outside template layers. |
| `select.by-layer` (Q) | `{index}` | |
| `clipboard.copy` | `{ids}` | Engine clipboard (not saved, not undone); groups wholly inside the copy come along. |
| `clipboard.cut` | `{ids}` | Copy, then remove. |
| `clipboard.paste` | `{mode?, offset_mm?}` | `offset` (default, right and down by `offset_mm`, default 5), `in-place` (top of the layer), `in-front` (directly above each original). New ids; copied groups are recreated. |
| `transform.each` | `{ids, dx_mm?, dy_mm?, scale_x?, scale_y?, degrees?, each?}` | Scale and rotate about the selection's centre, or with `each` about each object's own centre (a group is one object), then move. Remembered. |
| `transform.again` | `{ids}` | Repeats the last `transform.each` on `ids`. |
| `shapes.align-to-key` | `{ids, key, alignment}` | The key (with its group) stays still; others align to it. |
| `shapes.distribute-spacing` | `{ids, axis, gap_mm}` | The first object stays; the rest follow `gap_mm` apart. |
| `document.set-meta` | `{title?, notes?}` | Title up to 1 KiB, notes up to 64 KiB. |
| `document.update-thumbnail` | `{}` | Stores an SVG thumbnail (≤ 64 KiB) in the file. |
| `templates.list` (Q) | `{}` | `{Templates: [{id, name, description, category, params}]}` for a new-project wizard: `wizard` types (`blank`, `card`, `emboss`, `gift-tag`, `pixscan`, `print-and-cut`, `sketch`, `stamping`, `stipple`) and `starter`s (`vinyl-sticker`, `heat-transfer-tshirt`, `sticker-sheet`, `greetings-card`, `stencil`, `sketched-message`, `foil-stamp-card`, `foil-stamp-notebook`, `acrylic-light`, `metal-engraved-decor`, `embossed-card`, `embossed-sticker`, `coin-case`, `faux-leather-tray`, `leather-earrings`, `leather-keychain`). |
| `document.new-from-template` | `{template, params?}` | New document from a template (`crates/doc/src/templates.rs`, our own geometry): page/mat/media, operation layers (score, pen, foil, emboss, stipple and engrave are named `engrave` layers), `mirror` and `registration_marks` on the page setup, and the main pieces grouped. E.g. `card`: 10 × 7 in outline with a dashed score fold on a 12×12 page. `params`: `preset`, `width_mm`, `height_mm`, `fold` (`vertical`/`horizontal`), `hole_mm`; the page grows when the item does not fit. Keeps the kerf width. |

Format: these fields are stored in `.stencilcraft` version 1 under optional keys
(`board-setup`, `pages`, `guides`, `grid`, `snap`, `groups`, `meta`, and per layer
`locked`, `template`, `color`), written only when set, so older files load unchanged and a
plain document saves exactly as before. Group members that no longer exist are dropped on
save and load; cycles, a member in two groups, bad colours and over-cap values are load
errors.

Not engine commands (filesystem, called by the desktop app directly):
`stencilcraft_doc::recovery::Recovery` (autosave snapshots: `snapshot`, `list`, `restore`,
`discard`; atomic writes, bounded count and bytes), `recovery::RecentFiles`, and
`stencilcraft_doc::library::Library` (local design library: folders, favourites, search,
thumbnails, recycle bin). Display units live in `stencilcraft_geom::Unit`
(`mm`/`cm`/`in`/`pt`: `to_mm`, `from_mm`, `format`, `parse_to_mm`).

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

