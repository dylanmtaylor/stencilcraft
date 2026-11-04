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
`export_dxf`, `export_gcode`, `export_pdf`, `export_image`, `import_file`, `tile_run`,
`tile_preview`, `command_execute`, the machine tools `machines_list`, `machine_capabilities`
and `machine_dry_run` (see [Cutting machines](#cutting-machines)), the replicate/offset tools
`shapes_replicate_side`, `shapes_replicate_array`, `shapes_replicate_fill`,
`shapes_mirror_copy`, `shapes_rotate_copies` and `shapes_offset` (see
[Replicate and offset](#replicate-and-offset)), and the image tools `image_trace`,
`image_remove_background`, `image_set_bounds` and `image_remove` (see
[Import and trace](#import-and-trace)), and the text tools `text_add`, `text_edit`,
`text_to_outlines`, `text_ungroup`, `text_fonts` and `text_monogram` (see [Text](#text)). Their
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
| `guide.lock-all` | `{locked}` | Locks or unlocks every guide (View › Guides › Lock Guides). |
| `guide.make` | `{ids}` | Make Guides: replaces the shapes with guides along each one's bounds (two vertical, two horizontal; positions already guided are not doubled). Answers `ShapesRemoved`. |
| `guide.release` | `{}` | Release Guides: every unlocked guide becomes an open line shape across the main board on layer 0. Answers `ShapesAdded`. |
| `grid.set` | `{spacing_mm, subdivisions, visible?}` | Spacing 0.01-10000 mm, subdivisions 1-100; snapping uses spacing / subdivisions. |
| `snap.set` | `{grid?, guides?, objects?, spacing?, points?, tolerance_mm?}` | Defaults: grid and points off, the rest on, 2 mm. `points` is Snap to Point (other shapes' anchor points). |
| `snap.point` (Q) | `{x, y, width?, height?, exclude?, tolerance_mm?}` | A point, or a moving box whose left/centre/right and top/centre/bottom snap. Answers `{Snap: {x, y, dx, dy, snapped_x, snapped_y, lines}}`; each axis snaps to its nearest target (`guide`, `edge`, `center`, `page`, `spacing`, `grid`) within tolerance (with `points` on, an anchor point within tolerance on both axes wins first and moves both axes; its kind is `point` and its line is zero-length at the point), and `lines` are the alignment or equal-spacing markers to draw. Also callable as the pure function `stencilcraft_engine::document::snap`. |
| `measure.distance` (Q) | `{from: [x, y], to: [x, y]}` | `{Measure: {distance_mm, dx_mm, dy_mm, angle_degrees}}`, angle from +x towards +y. |
| `query.dimensions` (Q) | `{ids}` | `{Dimensions: {x, y, width, height}}` of the combined bounds. |
| `select.all` / `select.none` (Q) | `{}` | Selectable shapes outside template layers / nothing. |
| `select.by-color` (Q) | `{color, tolerance?}` | Shapes on layers with a colour within `tolerance` of `color`. |
| `select.by-operation` (Q) | `{style}` | `cut` or `engrave`, outside template layers. |
| `select.by-layer` (Q) | `{index}` | |
| `clipboard.copy` | `{ids}` | Engine clipboard (not saved, not undone); groups wholly inside the copy come along. |
| `clipboard.cut` | `{ids}` | Copy, then remove. |
| `clipboard.paste` | `{mode?, offset_mm?}` | `offset` (default, right and down by `offset_mm`, default 5), `in-place` (top of the layer), `in-front` (directly above each original). New ids; copied groups are recreated. |
| `transform.each` | `{ids, dx_mm?, dy_mm?, scale_x?, scale_y?, degrees?, each?, shear_degrees?, shear_axis?}` | Shear, scale and rotate about the selection's centre, or with `each` about each object's own centre (a group is one object), then move. A negative scale flips. Remembered. |
| `transform.again` | `{ids}` | Repeats the last `transform.each` or `shapes.shear` on `ids`. |
| `shapes.shear` | `{ids, degrees, axis}` | Slants about the selection's centre, \|degrees\| ≤ 89: `horizontal` shifts x by tan(degrees) × (y − centre y), `vertical` shifts y with x. Remembered. |
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
| `doc.open` | `{path}`: a `.stencilcraft` file, or a Silhouette Studio `.studio`/`.studio3` to import | `{layers: n, shapes: n, warnings: [..]}` |
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
| `machine.dry_run` | `{model, media?, force?, speed?, tool?, holder?, depth?, passes?, overcut?, mat?, media_size?: [w, l], pen?, track_enhancing?, bbox?, regmarks?: {width, length, x, y, search?, quad?}, order?: "none"\|"speed"\|"roller", interior_first?, closed_overcut?, mirror?, weld?, layers?: [{layer, skip?, holder?, passes?, force?, speed?}], path?}` | `{model, protocol, commands, bytes, bbox_mm, registration_wait, order, interior_first, closed_overcut_mm, mirror, weld, operations: [{layer, holder, passes, force, speed, paths}], truncated, transcript: [..], path}` |
| `export.pdf` | `{path}` | `{path}` |
| `export.image` | `{path, dpi?}` (`.png`, `.jpg`/`.jpeg`; dpi default 300, up to 4800) | `{path, dpi}` |
| `import.file` | `{path, fit_to_page?, join?, join_tolerance?, dpi?}` | `{shapes: [id], images: [id]}` |
| `image.trace` | `{id, options?, style?}` | `{ShapesAdded: {ids}}` |
| `image.remove-background` | `{id, tolerance}` | `{ImageChanged: {id}}` |
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

The desktop app's `--control` socket also answers three window methods the headless session
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
| `app.view` | `{kerf_preview?, theme?}` | `{kerf_preview, theme}`: turns the kerf preview on or off and sets the theme (`light`/`dark`) |

Every socket request is handed to the window, which runs it on its next frame. With the
screen locked or the window minimized or covered no frames run, so the socket waits at most
10 s and then answers `{"id":…,"ok":false,"error":"the window is not drawing frames (screen
locked, minimized or covered); try again when it is visible"}` (the request's `id` is kept;
the request is discarded, never applied later). `app.screenshot` instead falls back to a
headless render of the document as of the last drawn frame (board and shapes, as
`export_image`, 96 dpi, no window chrome) and answers `{path, size, source: "render", dpi:
96}`; a frame capture has no `source` field.

A successful `doc.open` or `doc.save` through the socket also updates the window title and
the recent-files list, as File › Open and Save do.


## Cutting machines

`machines_list` returns every registry row (`crates/machine/machines.toml`, see
`docs/machine-architecture.md`), including Cricut slots marked `unsupported`;
`machine_capabilities` returns one. Force/speed are in the machine's own units
(`force.unit` / `speed.unit`). `machine_dry_run` encodes the
current document for one model and returns the transcript (first 5000 commands; `truncated`
says if there were more); with `path` it also writes the exact byte stream (HP-GL/DMPL files
for a spooler, for example). Every value is validated against the model: an out-of-range
force, speed, depth, tool, holder, mat or a design outside the cut area is a normal tool error
naming the knob and range — never clamped. Job settings: `order` (`roller` default: sort
by the feed-axis top edge; `speed`: nearest-neighbour; `none`: document order),
`interior_first` (default true: inner contours before the closed path around them),
`closed_overcut` (mm, 0–5, past the start of each closed contour) and `mirror` (flip about
the media centre for heat-transfer vinyl; not with `regmarks`), `weld` (unite overlapping
closed contours so overlaps are cut once) and `layers` (cut by layer: per layer name or
`#rrggbb` line colour, `skip`, `holder` (alias `tool`) 1 or 2, `passes`, `force`,
`speed`; an unknown layer or field, or a value outside the machine's limits, is an
error). See
[`machine.md`](machine.md) › Job options. There is deliberately **no** MCP method that sends
to hardware; a real send is only `stencilcraft-cli cut --device …`, run by a person.

## Import and trace

`import_file` adds a file to the **current** document (undoable): SVG, DXF, PDF, EPS/PS
and Illustrator `.ai` vector paths become cut shapes (as is, 1:1, unless `fit_to_page`
scales them to the board), with open paths whose ends meet within `join_tolerance` mm
(default 0.05) joined into polylines unless `join: false`. Raster files (PNG, JPEG, BMP,
GIF first frame, TIFF, WebP) are placed as an image object sized at `dpi` (default 96) to
trace; HEIC/HEIF is refused with a message. Files over 64 MiB, images over 10,000 px a
side or 16 million pixels, and more than 200,000 paths are errors, never crashes. Only
path geometry is read from vector files: fills, text, embedded images and symbol
instances are dropped. An SVG with text, a mask, a filter or CSS the importer ignores
imports without those parts; what was skipped is printed to the server's stderr (the CLI
`import-svg` prints the same warnings). Convert text to paths first to keep it.

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


### Explicit hardware engine commands

`hardware.devices` enumerates connected cutter-vendor USB descriptors.
`hardware.cut` takes `model` (catalog id), `location` (from discovery), `mat`,
`force`, `speed`, optional AutoBlade `depth` (0–10), `tool`, `draw`, `holder`, `passes`, `mirror`, optional `bluetooth` ({`address`, `channel`, `ble`}, Linux BLE or experimental Classic RFCOMM), and `send` (defaults false). Matless jobs are bounded by the document board size. False validates visible document
paths; **true physically submits the cut**. These commands do not change document
history. USB requires the engine `usb` feature (enabled by the desktop).

`hardware.bluetooth_devices` lists paired recognized Linux BLE cutters without sending a job.
