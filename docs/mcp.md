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
(see [Replicate and offset](#replicate-and-offset)), and the text tools `text_add`,
`text_edit`, `text_to_outlines`, `text_ungroup`, `text_fonts` and `text_monogram` (see
[Text](#text)). Their
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

## Text

Live text objects (`crates/engine/src/text.rs`). A text shape's `path` holds the laid-out
outlines (what every export cuts); its `text` field keeps the parameters so the text stays
editable. Each command is a dedicated MCP tool (`text_add`, …), a `command_execute`
command and a `--command` for `stencilcraft-cli run`; editing commands are one undo step.

| Command / tool | Params | Effect |
| --- | --- | --- |
| `text.add` / `text_add` | `{text, font, size, x, y, alignment?, letter_spacing?, line_spacing?, kerning?, vertical?, weld?, flow?, letters?, style?}` | `ShapeAdded`. A live text shape on the top layer of `style` (default `cut`; `engrave` for the single-line font). |
| `text.edit` / `text_edit` | `{id, …any text.add field but style}` | `ShapesEdited`. Lays the text out again; moves, turns, scales and mirrors made to the shape since are kept. |
| `text.to-outlines` / `text_to_outlines` | `{ids}` | `ShapesEdited`. The outlines stay as plain paths; the text is no longer editable. |
| `text.ungroup` / `text_ungroup` | `{id, to}` | `ShapesAdded` with every id (the first keeps `id`). `to: lines` makes one text object per line (straight text only); `letters` one per letter, each live text in its exact place (nudges, curve and path placement become its transform). |
| `text.fonts` / `text_fonts` | `{query?}` | `{"Fonts": {families: [{family, styles, single_line}]}}`: the built-in single-line family first, then installed fonts and loaded font files whose name contains `query`. A query: not recorded. |
| `text.monogram` / `text_monogram` | `{initials, font, size?, x, y, frame?, padding_mm?}` | `ShapesAdded` `[text, frame]`: 1–4 initials (live text) centred on `x, y`, with a built-in frame (`none`, `circle` (default), `square`, `diamond`, `scallop`) `padding_mm` (default 3) clear of them. |

Fields:

- `font`: `{family, style?, file?}`. `family` is an installed family (`text.fonts`), or
  `"StencilCraft Single Line"`, the built-in single-stroke lettering for pens and engraving
  (printable ASCII; other characters draw `?`). `style` defaults to `Regular`; a missing style
  takes the family's nearest one, a missing family is an error. `file` loads a TTF/OTF/TTC
  (at most 64 MiB; unreadable files are an error) and is read again whenever the text is laid
  out; with `file`, `family` may be empty (the file's first family).
- `size` is the em size in points (1 pt = 25.4/72 mm), 0.01–5000. `x, y` (mm) is the anchor:
  the first baseline, at its left end, centre or right end by `alignment`.
- `alignment`: `left` (default), `centre` (`center` also accepted), `right`, `justify` (every
  line but the last stretched to the widest line).
- `letter_spacing`: space added between letters in percent of the em (0 = the font's own
  spacing; −100 to 1000). `line_spacing`: percent of the normal line distance (1.2 em; 100 =
  normal; 1 to 1000). `kerning: false` turns the font's pair kerning off.
- `vertical`: letters run top to bottom in columns from right to left.
- `weld`: overlapping letters are merged into one cut outline (connected script); counters
  stay. Ignored for the single-line font.
- `flow` (one horizontal line only; ungroup to lines first):
  - `{"kind": "straight"}` (default);
  - `{"kind": "curve", "diameter_mm": D}`: the line bent round a circle of diameter |D| with
    its middle at the anchor; positive D arches up, negative dips; 0 straightens;
  - `{"kind": "on-path", "shape": id | "path": <path>, "start": 0.0, "offset_mm": 0, "flip":
    false}`: the letters follow the shape's outline (its first contour, copied when the
    command runs). `start` is the fraction of the path's length where the anchor sits (so
    `alignment` centres or ends the text there); `offset_mm` lifts the letters off the path;
    `flip` runs the text the other way round, on the other side. Closed paths wrap; open
    ones continue straight past their ends.
  Letters are placed rigidly (turned to the path, not bent).
- `letters`: `[{index, dx_mm, dy_mm, rotate_deg}]`: per-letter nudges by character index into
  `text` (newlines count), moved and turned about the letter's baseline origin (before any
  curve or path placement, so `dy_mm` lifts a letter off its path).

`text.edit` works out the move made to the shape by fitting an affine from a fresh layout of
its old parameters to its current path; when the outline was edited as a path (node edits,
pathfinder) it no longer fits and `text.edit`/`text.ungroup` refuse instead of discarding the
edit: undo it, or keep the outlines with `text.to-outlines`. The same happens when the font
the text was set in is no longer installed. Release Compound Path on a text shape makes plain
paths.

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

