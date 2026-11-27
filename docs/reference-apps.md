# Reference apps: Silhouette Studio and Cricut Design Space

This is the feature inventory of the two consumer cutting apps StencilCraft is measured
against. For each feature it gives our own description, StencilCraft's status and how we
plan to handle it. It replaces the old "Silhouette Studio feature TODO" list in
[`ROADMAP.md`](../ROADMAP.md). The ranked work list stays in [`gaps.md`](gaps.md), and the menu bar that exposes these
features is designed in [`menu-map.md`](menu-map.md). Owner goal: no feature of either
app is left undocumented or unreproduced. Won't-do is kept for proprietary stores and
content, accounts, DRM, telemetry and vendor firmware.

## Clean-room posture

StencilCraft is clean-room. We study the reference apps from the outside: we drive them
to learn what they do, then write our own implementation from our own design. We never
use their source code.

**Allowed**

- Installing the reference apps (Silhouette Studio, Cricut Design Space) on a licensed
  machine or VM.
- Using and driving them through their normal user interface.
- Observing their behaviour, features, workflows, UI layout and outputs (for example, a
  cut file they write or the command stream a machine receives during a normal job).
- Reading their public documentation: user manuals, vendor help centres and release notes,
  cited by URL.
- Recording what we observe, in our own words, in this document.

**Not allowed**

- Their source code. We never obtain it or use it.
- Disassembling, decompiling or debugging their binaries.
- Extracting or copying their assets: icons, artwork, fonts, sample designs, preset data
  files, or UI text.
- Copying their trade dress or branding.
- Bypassing licensing, activation or sign-in.
- Copying GPL/AGPL code (Inkscape, lib2geom, LaserGRBL, VisiCut, the GPL Silhouette
  drivers). We may mirror behaviour, but never copy code.

Graphtec protocol facts come from community reverse-engineered GPL drivers, read as
*specifications only*:
[inkscape-silhouette](https://github.com/fablabnbg/inkscape-silhouette),
[inkscape-cutcutgo](https://github.com/virtualabs/inkscape-cutcutgo) and
[pklaus/Silhouette-Cutting-Plotters](https://github.com/pklaus/Silhouette-Cutting-Plotters)
(see [`machine.md`](machine.md#provenance--licensing)). No GPL code goes into this MIT/Apache
codebase.

Every observation names the date and the app version it was made against. A row with
`pending` in the Observed column is based only on public documentation so far.

## Versions and sources

Raw notes from driving Silhouette Studio are in [`observations/silhouette-studio.md`](observations/silhouette-studio.md).

| App | Version under study | Platform | Public documentation |
| --- | --- | --- | --- |
| Silhouette Studio | 5.0.x; observed: 5.0.539 Basic edition, Windows 11 VM, 2026-11-02; 5.0.556 Basic edition, Windows 11 Pro hardware, 2026-11-23 | Windows | [Silhouette Studio overview](https://www.silhouetteamerica.com/silhouette-studio), [software suite and editions](https://www.silhouetteamerica.com/software), [support knowledge base: Studio features](https://silhouetteamerica.freshdesk.com/support/solutions/folders/35000233809), [file types and features by edition](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000282776-file-types-and-features), [Designer Edition features](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000273383-designer-edition-features), the in-app Help › User's Manual |
| Cricut Design Space | 9.8x; installed: 9.83.76, Windows 11 VM, 2026-11-02 | Windows | [Cricut help centre](https://help.cricut.com/hc/en-us), [Canvas screen guide](https://help.cricut.com/hc/en-us/articles/26751604105879-Canvas-screen-Design-Space-Guide), [release notes](https://help.cricut.com/hc/en-us/articles/27054006457111-Design-Space-Release-Notes) |

Silhouette Studio splits features across editions (Basic, Designer, Designer Plus,
Business). Rows note the edition when the public documentation names it. Cricut Design
Space puts some content behind a subscription (Cricut Access). Neither split matters for
StencilCraft: every feature we build is free.

Silhouette Studio 5.0.539 was driven in the Basic edition only. Rows tagged with a paid
edition say "not present in Basic" until someone observes that edition.

Cricut Design Space 9.83.76 installs, but it requires a Cricut account sign-in before any
design screen opens. We do not bypass sign-in, so no DS row has been observed. Every DS
observation reads `docs only (sign-in required)` until the owner decides whether to sign
in with an account. The DS rows are built from Cricut's public documentation, collected in
the [Cricut Design Space feature library](cricut-design-space/README.md).

## Key

- **Status**: what StencilCraft has today, checked against the code at the time of
  writing.
  - **have**: usable end to end.
  - **partial**: some of it works; the note says what is missing.
  - **missing**: not implemented.
  - **out of scope**: we will not build it.
- **Posture**: how we handle the feature.
  - **replicate**: build the same behaviour.
  - **reimagine**: meet the same need in our own way.
  - **won't do**: we will not build it; the note gives the reason.
- **Observed**: `pending` until someone has driven the app. Then it holds the date, app
  version and the observation, in our own words. Rows for both apps give one note per
  app (SS, then DS).

Code references are to this repository: `engine` is `crates/engine/src/lib.rs` (the command
set), `app` is `apps/stencilcraft/src/main.rs` (the desktop shell) and `cli` is
`apps/stencilcraft-cli/src/`.

## Document, canvas and page setup

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Page/media size | SS | Set the media width and height, or choose a preset size; the design area matches the material | have | replicate | 2026-11-02, SS 5.0.539: Page Setup panel (Media tab): machine, feed type, mat and media size, each defaulting to Auto (12×12 in); custom width/height, an option to keep the media inside the mat, page colour and transparency. 2026-11-23, SS 5.0.556: a Page Setup Guide wizard asks machine series → model → feed type (manual, electrostatic mat, auto sheet feeder) → mat → media size, each with a preview and "change this later in Page Setup" |
| Home screen | SS | A start screen with new-document choices, recent files and a vendor news feed | have: the title bar's Home shows the start screen (New, Open, recent files) | reimagine: new/recent only; no vendor news feed | 2026-11-02, SS 5.0.539: a New Document carousel, Recent Library Files and a News & Updates feed; a first-run dialog asks if you are a beginner or experienced. 2026-11-23, SS 5.0.556: first launch also asks about update checks and a data-collection opt-in that is checked by default, then shows a 7-page "what's new" tour ending in the Page Setup Guide |
| Starter mode (simplified UI toggle) | SS | A switch that hides most toolbars and panels for beginners | have: Panels › Starter Mode keeps only the everyday panels (remembered; Reset Panels follows it) | reimagine as a workspace preset | 2026-11-02, SS 5.0.539: a top-bar toggle; on, it hides the quick-access toolbar and most panels; off, it shows the full toolbar, the options bar and about 15 panel icons. 2026-11-23, SS 5.0.556: changed — on keeps the toolbar with large icons and shows 8 labelled panels plus "Show more…"; off uses compact icons, about 15 panels and adds zoom-to-selection, pan and fit buttons |
| Design page settings | SS | A quick dialog for the page and mat settings | have: Panels › Page Setup: Machine, Cutting Mat, Constrain Media to Cutting Mat (on), Media Size presets, Width/Height, Portrait/Landscape, page colour and Transparency (view only), Rotate View, borders, grid and snapping | replicate | 2026-11-02, SS 5.0.539: View › Show Design Page Settings (Ctrl+F1) |
| Cutting mat selection and backdrop | both | Choose the mat (12×12, 12×24, …); the canvas shows it behind the design | have: `Mat` table in `crates/machine`, mat backdrop in the app | replicate | 2026-11-02, SS 5.0.539: a grey mat is drawn around the white page, with a feed-direction arrow at the top; the mat is picked in Page Setup; a preference keeps the mat always on screen. DS: docs only (sign-in required) |
| Document tabs | SS | Several open documents as tabs, with a button for a new tab | have: tabs above the design area (the framework's DocumentTabs): New and Open add a tab (Open shows a file already open), each tab keeps its own undo history, selection and view; Close Tab (Ctrl+W) asks before discarding changes; tabs reorder; engine: several pages per document (`page.add/set/remove`) | replicate | 2026-11-02, SS 5.0.539: tabs above the canvas (one per document) with a + button; Close Tab is in the File menu; only one page per tab was seen |
| Infinite canvas with auto mat layout | DS | Design on a free canvas; the Make screen lays shapes out onto mats per colour/material | missing | reimagine: bridge to nesting and tiling | docs only (sign-in required) |
| Orientation and rotated view | SS | Portrait/landscape feed and a rotated view of the mat | partial: Page Setup has Portrait/Landscape (swaps the page) and Rotate View 0/90/180/270; the encoder can emit the feed direction (`TB50`), not yet tied to orientation | replicate | 2026-11-02, SS 5.0.539: Page Setup has portrait/landscape and a rotated view (0/90/180/270); View menu has Rotate Page; page orientation defaults to automatic in Preferences |
| Cut border / safe area | SS | A border around the media that the blade will not enter | have: View › Show Cut Border (on, Cmd+Shift+L) draws it 1.5 mm inside the page, as measured in SS 5.0.556, or at the page edge with Preferences › Cut to edge of page; sends clip every line to it (`clip_inset_mm`), cutting only the parts inside, as SS's Send view shows; per-machine margins are not modelled | replicate | 2026-11-02, SS 5.0.539: Show Cut Border (on by default) outlines the area the blade can reach; a preference allows cutting to the edge of the page |
| Rulers, grid, guides | both | Rulers on the canvas edges, a background grid, draggable guides | partial: rulers, grid (the document's spacing/subdivisions) and guides dragged from the rulers, with Show/Lock/Make/Release/Clear; Page Setup (View › Grid Options…) sets grid spacing, divisions, Snap to Grid, Smart Snapping and tolerance; isometric grids and grid colours are missing | replicate | 2026-11-02, SS 5.0.539: View menu has Show Grid and a Grid Options dialog; Page Setup has a Grid tab; rulers and guides not checked yet. DS: docs only (sign-in required) |
| Snap to grid | both | Objects snap to grid lines | have: View › Snap to Grid; move drags snap through `snap.point` | replicate | 2026-11-02, SS 5.0.539: Snap to Grid in the View menu; object snapping is a separate row (Smart Snapping). DS: docs only (sign-in required) |
| Smart snapping (to objects) | SS | Snap to other objects' edges and centres while moving | have: View › Smart Snapping; move drags snap to edges, centres, pages, spacing and guides, with Smart Guides lines | replicate | 2026-11-02, SS 5.0.539: View › Enable Smart Snapping, on by default |
| Zoom, pan, fit | both | Zoom tools, hand pan, fit to page/selection | have: Hand, Zoom, Fit (F), Center (C), Zoom to Selection, 25–400 % presets, status-bar zoom, Rotate Page, canvas scroll bars | replicate | 2026-11-02, SS 5.0.539: full toolbar has zoom in/out, zoom to fit, zoom to selection and a pan hand; View menu has a Zoom submenu. DS: docs only (sign-in required) |
| Live dimension callouts while drawing | SS | Width and height labels follow the shape as you drag, and stay on the selection | have: W/H (or length and angle) follow the pointer while drawing; View › Show Dimensions labels the selection | replicate | 2026-11-02, SS 5.0.539: width shown below and height on the left, in inches; they update during an offset preview; Show Dimensions is a preference (on) |
| Measure | SS | Measure a distance on the canvas | have: the Measure tool (ruler, at the foot of the tool rail): drag a line to read its length, width, height and angle; it stays shown until the next one and never enters the document | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Units | both | Choose inches or millimetres; the default is inches | have: Preferences › Units mm, cm or in; length fields show and take the chosen unit (`length_drag`), designs stay in mm | replicate (display-only conversion at the UI boundary) | 2026-11-02, SS 5.0.539: default units are inches (preference set to Auto (Inches)); page size, dimension callouts and offset distances all show inches. DS: docs only (sign-in required) |
| Kerf/compensation preview | — | Not a reference-app feature; StencilCraft's own | have | — | — |
| Oversized design tiling | SS (Business) | Split a design larger than the media into tiles that are cut separately | have: `tiling` crate, `tile-preview`, overlap and marks | replicate | 2026-11-02, SS 5.0.539: not present in Basic; needs a paid edition to observe |

## Drawing tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Rectangle, rounded rectangle | both | Drag to draw; the corner radius can be changed | have: `shape.add-rect`, `shape.add-rounded-rect`; desktop drag: Shift = square/circle, Alt = from the centre; rounded rectangle Corner radius in the options bar (capped at half the shorter side); no on-canvas radius handles yet | replicate | 2026-11-02, SS 5.0.539: Shape flyout has rectangle and rounded rectangle; width and height callouts follow the drag. DS: docs only (sign-in required) |
| Ellipse/circle | both | Drag to draw; hold a modifier for a circle | have: `shape.add-ellipse`; desktop drag: Shift = square/circle, Alt = from the centre | replicate | 2026-11-02, SS 5.0.539: ellipse in the Shape flyout. DS: docs only (sign-in required) |
| Polygon, star | both | Regular polygon or star; the number of sides or points can be changed | have: `shape.add-polygon`, `shape.add-star`; desktop options bar while the tool is active: polygon Sides (default 5, as Silhouette), star Points and Inner % | replicate | 2026-11-02, SS 5.0.539: polygon in the Shape flyout; no star tool was seen. DS: docs only (sign-in required) |
| Basic shape palette (heart, triangle, score line, …) | DS | A panel of ready-made shapes | have (engine/MCP, no UI yet): `generate.shape` heart, triangle, arrow, banner and more; `generate.score-line` | reimagine with shapes generated in code; never copy their shape artwork | docs only (sign-in required) |
| Line | SS | Straight line tool | have: Line tool (`\\`, in the Pen flyout) → `shape.add-path`; Shift snaps to 45° steps; length/angle readout | replicate | 2026-11-02, SS 5.0.539: Line Tools flyout: line, polyline, curve, arc. 2026-11-24, SS 5.0.556: drag start→end; Shift snaps to 0/45/90° |
| Polyline/Bezier pen | both | Click to place points, drag for curves | have: `PenDraft` → `shape.add-path`; Polyline and Curve tools (Pen flyout): click to place points, click the start to close, double-click/Enter/Esc to finish, Backspace takes back a point, Shift snaps segments to 45°; the curve passes through every point | replicate | 2026-11-02, SS 5.0.539: polyline and curve (Bezier) tools in the Line Tools flyout. DS: docs only (sign-in required). 2026-11-24, SS 5.0.556: separate polyline (click corners; click start closes, double-click ends) and curve tools (curve passes through each clicked point); see observations |
| Freehand draw and smooth freehand | SS | Draw by hand, with optional smoothing | have: Freehand (`N`) and Smooth Freehand tools: the stroke follows the pointer, closes when it ends on its start; Freehand drops redundant points, Smooth simplifies and fits curves through what is left | replicate | 2026-11-02, SS 5.0.539: Freehand Tools flyout: freehand and smooth freehand; the tool stays active after a stroke. 2026-11-24, SS 5.0.556: stroke until release, closes when the end meets the start (manual) |
| Arc | SS | Draw an arc segment | have: Arc tool (Pen flyout): drag centre → start for the radius, the pointer sweeps (past 180°, either way), click or Enter places it, Shift snaps to 15°, Esc drops it | replicate | 2026-11-02, SS 5.0.539: arc in the Line Tools flyout. 2026-11-24, SS 5.0.556: drag centre→start sets the radius, then the pointer sweeps the arc with a live angle readout; click commits an open arc |
| Knife | both | Cut a shape apart along a drawn line | have: `shapes.knife` + Knife tool (`K`): drag a line across the selection; Outline (default) splits lines into open pieces exactly where it crosses, Solid separates closed shapes into closed pieces; optional gap; Shift snaps to 45° | replicate | 2026-11-02, SS 5.0.539: knife tool at the bottom of the tool rail; the tool stays active after use. DS: docs only (sign-in required). 2026-11-24, SS 5.0.556 Basic: acts on the selection; Outline mode (default) splits into open paths, Solid into closed pieces; gap width, Auto Apply |
| Eraser | SS | Erase part of a closed shape with a brush | have: `shapes.erase` + Eraser tool (`E`): the stroke erases from the selection (or every unlocked visible shape); Solid (default) removes area and keeps each piece, Outline removes the lines under the brush; round or square brush, size | replicate | 2026-11-02, SS 5.0.539: eraser tool in the tool rail; the tool stays active after use. 2026-11-24, SS 5.0.556 Basic: Solid (default) or Outline, round or square brush, Size; a Solid stroke across a rectangle leaves two closed pieces |
| Flexishapes | SS (Designer Plus) | Parametric shape presets you can edit | have (engine/MCP, no UI yet): `generate.shape` with editable params (`replace`) | reimagine (parametric shapes) | 2026-11-02, SS 5.0.539: not present in Basic |
| Rhinestone, cross-stitch, puzzle generator | SS (Designer+) | Special pattern generators | have: Effect › Rhinestone…, Puzzle… and Sketch Fill › Cross-stitch (`generate.rhinestone`, `generate.puzzle`, `generate.fill` pattern `cross-stitch`) | reimagine later, low priority | 2026-11-02, SS 5.0.539: not present in Basic (Sketch and Stipple are in Basic: see the Fill and line styles rows) |
| Stencil bridges | StencilCraft (SS Business has a Bridge tool, not observed) | Keep islands (the middle of O, A, B…) joined to the stencil sheet | have: Effect › Bridges… (Effects panel, live preview; `shapes.bridge`): strips of a chosen width through each island, above/below, left/right or both | own feature for stencils | — |

## Text

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Text object with system fonts | both | Type text in an installed font; edit it later | have: desktop Text tool types live text on the canvas (click, type, caret, Esc) in installed fonts or loaded font files; double-click or Type › Edit Selected Text edits it; Text Style picks the family (with a search), the style, and B / I toggles for the family's bold and italic faces | replicate with user fonts read through `CRAFT_FONTS_DIR`/system fonts; no fonts bundled | 2026-11-02, SS 5.0.539: text tool with a font picker (frequently used, recently used, all fonts), size in pt (72 by default), bold/italic/underline; a text box/notes tool is also in the rail. DS: docs only (sign-in required) |
| Vendor/licensed font libraries | both | Fonts from the vendor store or subscription | out of scope | won't do: licensed fonts and assets | SS: pending. DS: docs only (sign-in required) |
| Size, letter spacing, line spacing, alignment | both | Typography controls | have: Text Style panel: size (pt), justification, line spacing %, character spacing % on SS's scale, kerning and vertical text | replicate | 2026-11-02, SS 5.0.539: Text Style panel: line spacing %, character spacing %, a kerning toggle, left/centre/right/justify, horizontal or vertical text, a preview-text field. DS: docs only (sign-in required) |
| Text on path / curve text | both | Lay text along a path (SS) or bend it into an arc (DS Curve) | have: Type › Type on a Path (text + shape) and Curve Text…, with a Flow section in Text Style (straight / curve diameter / on-path start, offset, flip); each letter is placed and rotated along the path, not bent | replicate | 2026-11-02, SS 5.0.539: a text-on-path control in the text options bar. DS: docs only (sign-in required) |
| Spelling check | SS | Check the spelling of text objects | have: Type › Spelling… (F7): the framework's Check Spelling dialog over all live text (English word list, suggestions, Change / Change All / Ignore / Add; added words kept in preferences) | reimagine later, low priority | 2026-11-02, SS 5.0.539: a Spelling tab in the Text Style panel and Panels › Spelling (F7) |
| Convert text to path / ungroup to letters | both | Turn letters into editable outlines | have: Type › Create Outlines and Ungroup to Letters (`text.to-outlines`, `text.ungroup`); ungroup to lines is engine only | replicate | 2026-11-02, SS 5.0.539: Object menu has Convert to Path. DS: docs only (sign-in required) |
| Writing fonts (single-line) | both | Single-stroke fonts for pens | partial: engine ships "StencilCraft Single Line", an original single-stroke font drawn in code (printable ASCII); no user single-line font format yet | replicate (user-supplied single-line fonts) | SS: pending. DS: docs only (sign-in required) |
| Glyphs panel | SS (Designer) | Insert special characters from a font | have: Type › Glyphs (the framework's Glyphs panel): every character of the text font by category (Punctuation, Currency, Arrows, …), recently used, zoom; double-click or Enter inserts | replicate | 2026-11-02, SS 5.0.539: not in Basic's Panels menu |

## Editing and node tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Select, multi-select, box/lasso | both | Pick objects; add to the selection with a modifier | have: click/box/lasso, additive multi-select | replicate | 2026-11-02, SS 5.0.539: 8-handle bounding box; Select All and Deselect All in the Edit menu; a drag box selects shapes it touches by default (a preference); many selected shapes share one bounding box. DS: docs only (sign-in required) |
| Move, rotate, scale, exact size/position fields | both | Transform panel with numeric fields | have: `shape.move/rotate/scale/set-bounds`, `shapes.transform`; on the canvas: corner handles scale in proportion, edge handles stretch, the band outside a corner rotates (Shift 15° steps), one `shapes.affine` per drag | replicate | 2026-11-02, SS 5.0.539: Transform panel with Align, Scale, Rotate and Move tabs; the options bar also has W/H/X/Y fields (own row). DS: docs only (sign-in required) |
| Arrow-key nudge | both | Move the selection with the arrow keys | have: 2 screen px per press, 16 px with Shift (step follows the zoom) | replicate | 2026-11-25, SS 5.0.556: ~2 px per press and ~16 px with Shift at any zoom (0.021/0.176 in at the default zoom, 0.009/0.078 in two zoom steps in) |
| Flip/mirror | both | Mirror horizontally/vertically | have: Object › Mirror › Flip Horizontal/Vertical (`transform.each`, scale −1) | replicate | 2026-11-02, SS 5.0.539: Object menu has a Mirror submenu. DS: docs only (sign-in required) |
| Duplicate, cut/copy/paste | both | Copy objects; paste in place | have: desktop Cut/Copy/Paste (Cmd+X/C/V) through `clipboard.*`; Paste lands 80 px down-right and Duplicate (Cmd+D) 40 px right, in screen pixels like Silhouette | replicate | 2026-11-02, SS 5.0.539: Cut/Copy/Paste and Duplicate (Ctrl+D) in the Edit menu; Paste in Front is its own row. DS: docs only (sign-in required). 2026-11-25, SS 5.0.556: Paste offset +0.875 in right and down at the default zoom, Duplicate +0.4375 in right; both shrink with zoom (screen-pixel offsets); Ctrl+Arrow duplicates flush to that side |
| Group/ungroup | both | Treat objects as one unit | have: Object › Group (Cmd+G) / Ungroup (Cmd+Shift+G); a click or marquee on any member selects the whole outermost group; Ungroup on a compound path releases it (Silhouette manual ch. 06) | replicate | 2026-11-02, SS 5.0.539: Group and Ungroup in the Object menu, the toolbar and the selection options bar. DS: docs only (sign-in required) |
| Compound path make/release | SS | Join paths into one shape with holes | have: Object › Make / Release Compound Path | replicate | 2026-11-02, SS 5.0.539: Make/Release Compound Path in the Object menu and the Modify panel |
| Point editing | both | Move, add and delete points; change smooth/corner points | have: `BezierOverlay` direct select and convert-point; desktop: click a line to add a point, right-click for Delete Point / Break Path / Corner / Smooth / Make Flat / Make Curve, drag an open end onto another to join, Delete removes points; Edit › Simplify (Cmd+J) and Modify › Convert to Path | replicate | 2026-11-02, SS 5.0.539: point edit tool in the rail; Bezier control handles show for selected points only by default (a preference); Point Editing panel: delete point, break path, corner, smooth, make a segment flat or curved, simplify, convert to path. DS: docs only (sign-in required). 2026-11-24, SS 5.0.556: point toolbar + Point Editing panel; click a segment to add a point; right-click a point for Delete/Break; drag an end onto the other end to rejoin |
| Node count readout | SS | Counts for the selection: shapes, contours, nodes, straight and curved nodes | have: the Point Editing panel counts the selection's nodes | replicate (cheap, helps judge cut complexity) | 2026-11-02, SS 5.0.539: shown in the Point Editing panel |
| Break/join path, close path | SS | Split a path at a point; join two endpoints | have: Object › Path › Join, Break at Point, Close Path, plus drag-an-end-to-join in point editing | replicate | 2026-11-02, SS 5.0.539: the Point Editing panel has Break Path; no join-path command was found in Basic |
| Simplify / reduce points | SS | Remove extra points within a tolerance | have: Object › Simplify (Cmd+J) and the Points panel | replicate | 2026-11-02, SS 5.0.539: Object menu has Simplify (Ctrl+J) |
| Warp / distort | SS (Designer), DS | Bend a shape with an envelope or perspective | have: Effect › Warp › Arc/Flag/Bulge/Perspective/Conical… open the Effects panel (`generate.warp`, 15 envelope styles, perspective, and Conical: the design wrapped onto a flattened cone from the cup's top and bottom diameters and slant height) | reimagine, low priority | 2026-11-02, SS 5.0.539: not present in Basic. DS: docs only (sign-in required) |
| Undo/redo history | both | Multi-step undo | have: journal-based undo/redo | replicate | 2026-11-02, SS 5.0.539: Undo/Redo in the Edit menu and the toolbar. DS: docs only (sign-in required) |
| Selection options bar (W/H with aspect lock, X/Y with reference point) | SS | Size and position fields for the selection above the canvas | have: X/Y/W/H in the options bar and Inspector, an aspect link and a reference-point picker | replicate | 2026-11-02, SS 5.0.539: fill/line swatches and line width, then W/H with an aspect lock and X/Y with a reference-point picker, then align, group, arrange, weld, offset, replicate and delete |
| Rotation knob on the selection | SS | A handle above the bounding box to rotate by dragging | have: a knob on a stem above the selection box rotates it (Shift snaps 15°), as in Silhouette | replicate | 2026-11-02, SS 5.0.539: a green round handle above the 8-handle box |
| Paste in Front | SS | Paste a copy at the same place, on top | have: Paste in Front (Cmd+F, exactly on top) and Paste in Place (Cmd+Shift+V) and Paste in Back (Cmd+B, each copy directly behind its original) | replicate | 2026-11-02, SS 5.0.539: Edit › Paste in Front (Ctrl+F) |
| Right-click menu on the selection | SS | Context menu with the common object commands | have: Select tool right-click (selects the shape first): Undo/Redo, Cut/Copy/Paste/Paste in Front/Duplicate/Delete, Flip, Edit Points/Weld/Offset, Group/Ungroup, arrange; with nothing selected Undo/Redo, Paste, Select All | replicate | 2026-11-03, SS 5.0.539: the same groups in the same order (see observations); undo/redo items name the action, ours do not yet |
| Select by Color | SS | Select every shape with a given line or fill colour | have: Select by Color panel lists the line and fill colours in use and selects their shapes | reimagine: select by operation/colour | 2026-11-02, SS 5.0.539: Edit › Select by Color and a Select by Color panel |
| Tool shortcuts | SS | Single-letter keys switch tools | have: rail tools have single-key shortcuts | replicate | 2026-11-02, SS 5.0.539: typing on the canvas changes the active tool |
| Action after tool use | SS | Whether a tool stays active or returns to Select after use | have: Preferences › Tools: after creating a shape, drawing freehand, the Eraser, the Knife and Zoom, continue or Choose Select (Silhouette's defaults: continue, and Select after Zoom) | replicate as a preference | 2026-11-02, SS 5.0.539: Tools preferences: shape tools keep drawing; freehand, eraser and knife stay active; zoom and eyedropper return to Select; right-click on later shapes selects them |
| Default fill style for new shapes | SS | Whether drawn shapes start filled or outline only, and in which colours | have: Preferences › Defaults: new shapes: Outline only (default) or Solid fill with a fill swatch, and a line colour; applied in the shape's own undo step (`Engine::new_shape_style`); imports keep their own look | replicate | Preferences › Defaults (2026-11-26, SS 5.0.556) |
| Drag-select mode (touching vs enclosed) | SS | Whether a drag box selects shapes it touches or only those inside it | have: Preferences › Tools › When drag selecting: shapes touching the box (default) or fully inside, for the box and the lasso | reimagine (by drag direction, not a preference) | 2026-11-02, SS 5.0.539: a Tools preference, defaulting to shapes touching the box |
| Lock objects | SS | Stop an object from being selected or moved | have: Object › Lock / Unlock All and lock toggles per object and per layer | replicate | not present in Basic (2026-11-02, SS 5.0.539) |

## Offset

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Outward/inward offset of a shape | both | Make a new outline a set distance outside or inside the shape | have: desktop Offset panel previews and applies `shapes.offset` (internal/external, distance, corner, Weld checkbox uniting overlapping results) | replicate as an engine command on top of `kerf` | 2026-11-02, SS 5.0.539: Offset panel shows a live preview on the selection before Apply; the dimension callouts update to the offset size; Cancel discards it. DS: docs only (sign-in required) |
| Corner style (round/sharp) | both | Choose rounded or sharp corners, with a limit on how far sharp corners reach | have: Offset panel corner choice (round, miter with limit, bevel), engine `JoinStyle` | replicate | 2026-11-02, SS 5.0.539: sharp or round corners plus a corner limit value (1.0 default); the live test defaulted to round at 0.125 in, the panel showed 0.079 in. DS: docs only (sign-in required) |
| Weld offsets / per-letter offsets | DS | Merge the offset outlines into one, or keep one per piece | missing | replicate | docs only (sign-in required) |
| Internal offset | SS | Offset inward, used for frames and borders | have: Offset panel › Internal Offset; thin features close up and pinched shapes split instead of inverting | replicate | 2026-11-02, SS 5.0.539: Internal Offset is a mode beside Offset in the same panel |

Sources: [Cricut, *How to use the Offset feature*](https://help.cricut.com/hc/en-us/articles/360061650414-How-to-use-the-Offset-feature-in-Design-Space).

## Trace

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Threshold trace of a raster image | both | Choose an area or the whole image; turn dark pixels into cut outlines | have: Trace panel: Select Trace Area box, live solid-yellow/outline preview, Threshold, Despeckle, High/Low Pass, Trace / Trace Outer Edge / Trace and Detach to one compound path; CLI/engine trace PNG, JPEG and other raster formats. Missing: resizing the box, Scale and Trace Quality | replicate | 2026-11-02, SS 5.0.539: Trace panel: select the trace area, then threshold % and despeckle threshold %; PNG files are auto-traced on import by default (a preference). 2026-11-23, SS 5.0.556: no auto-trace on Open or Merge of a PNG; the result is one red, unfilled compound path, the image is kept, and edges land within ~0.003 in of the pixel edges. DS: docs only (sign-in required) |
| High/low-pass filter, scale and colour-range trace | SS | Image filters and colour selection that guide the trace | have: Trace panel › Filters (High Pass, Low Pass, threshold, despeckle) and Trace by Colour | replicate | 2026-11-02, SS 5.0.539: an Advanced section in the Trace panel holds the filters (not expanded yet). 2026-11-23, SS 5.0.556: High Pass and Low Pass (off, 0.00), Scale (10), a 5-stop Trace Quality slider (Low…Max, middle default) and the preview display choice; no colour-range control in Basic |
| Image effects | SS | Brightness, colour and other adjustments on placed images | have: Effect › Image Effects… (brightness, contrast, saturation, greyscale, invert; `image.adjust`, one undo step) | reimagine later, low priority | 2026-11-02, SS 5.0.539: the panel needs GPU support and showed only an unsupported-graphics notice in the VM |
| Trace outer edge only / trace & detach | SS | Keep only the outline, or split the image along the trace | have: Trace panel › Trace, Trace Outer Edge, Trace and Detach | replicate | 2026-11-02, SS 5.0.539: Trace Outer Edge and Trace and Detach are actions next to Trace |
| Background remover / "complexity" upload flow | DS | A guided upload that removes the background, then cleans up with erase tools | partial: engine/MCP have deterministic background removal; guided upload and erase tools remain open | reimagine (local, deterministic; no cloud ML) | docs only (sign-in required) |
| Live preview of the trace on screen | both | Highlights the area that will be traced | have: Trace panel: a resizable trace box with a live solid-yellow or outline preview | replicate | 2026-11-23, SS 5.0.556: after Select Trace Area you drag a resizable box (with an × to remove it); what will be traced is painted solid yellow and updates live as Threshold moves (a darkness cutoff, default 45 %); Trace Preview Display switches between Solid Fill and Outline (dimmed image, yellow would-be cut lines). DS: docs only (sign-in required) |
| Full-colour/multi-layer trace | both | Trace each colour into its own layer | have: Trace panel › Find Colours, tick colours, Trace by Colour: each ticked colour becomes its own filled layer (`image.trace-colours`); not in Silhouette Basic, so unobserved | replicate | 2026-11-23, SS 5.0.556: not present in Basic (one-tab Trace panel); paid editions unobserved. DS: docs only (sign-in required) |

Sources: [Cricut, *How to upload images*](https://help.cricut.com/hc/en-us/articles/360009556313-How-to-upload-images-into-Design-Space).

## Weld and pathfinder

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Weld / Unite | both | Merge overlapping shapes into one outline | have: Pathfinder `union`/`merge` | replicate | 2026-11-02, SS 5.0.539: Weld in the Modify panel and the selection options bar. DS: docs only (sign-in required) |
| Subtract / Subtract All / Minus Back | both | Remove the top shape(s) from another | have: `difference`, `minus-back` | replicate | 2026-11-02, SS 5.0.539: Subtract and Subtract All in the Modify panel. DS: docs only (sign-in required) |
| Intersect | both | Keep only the overlap | have: `intersect` | replicate | 2026-11-02, SS 5.0.539: Intersect in the Modify panel. DS: docs only (sign-in required) |
| Exclude / XOR | both | Keep everything except the overlap | have: `xor` | replicate | 2026-11-02, SS 5.0.539: no exclude/XOR action in Basic's Modify panel. DS: docs only (sign-in required) |
| Divide / Slice | both | Split overlapping shapes into separate pieces | have: `divide`, `trim`, `crop`, `outline` | replicate | 2026-11-02, SS 5.0.539: Divide and Crop in the Modify panel. DS: docs only (sign-in required) |
| Detach Lines | SS | Separate a shape's lines from it | partial: Object › Modify › Detach Lines (`shapes.detach-lines`) splits a filled shape into its fill and its line (the stroke outlined, filled with the line colour); the width is the Outline Stroke width until shapes carry their own (craft-libs #3610) | replicate once its behaviour is confirmed | 2026-11-02, SS 5.0.539: a Modify panel action; its behaviour not tested yet |
| Attach | DS | Keep pieces in their relative positions on the mat without merging them | partial: Object › Attach / Detach in the desktop; engine/MCP attach/detach; the mat preview does not show attach groups yet | reimagine (groups that keep positions when sent to the mat) | docs only (sign-in required) |
| Flatten (print layers into one printable image) | DS | Merge layers into one Print Then Cut image | partial: production engine/MCP support flatten/unflatten metadata; end-to-end printed artwork and desktop workflow remain open | reimagine as part of Print & Cut | docs only (sign-in required) |
| Contour (hide parts of a shape) | DS | Hide individual sub-paths so they are not cut | have: Object › Contour… panel hides or shows each contour (hidden ones are drawn faint and not cut) | replicate (per-subpath enable) | docs only (sign-in required) |

## Replicate and array

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Duplicate in a direction (left/right/above/below) | SS | One-click copies next to the original | have: desktop Replicate panel and engine commands | replicate | 2026-11-02, SS 5.0.539: Replicate panel: duplicate left, right, above, below |
| Row/column/grid array | SS | Preset rows or columns of copies | have: Object › Replicate › Row of Three/Four, Column of Three/Four (touching, as in SS) and Fill Page with Silhouette's shortcuts; Replicate panel for any count and gap | replicate | 2026-11-02, SS 5.0.539: presets only: row of 3, row of 4, column of 3, column of 4; no free count seen |
| Rotated copies (radial array) | SS | N copies rotated around a centre | have: Object › Replicate › Rotate One/Two/Three/Five Copies (180°/(n+1) apart about the shape's centre, Ctrl+Shift+F1/F2/F3/F5); the Replicate panel's full-turn radial array | replicate | 2026-11-02, SS 5.0.539: Rotate Copies presets (a small set of radial counts) on a Radial tab |
| Advanced Replicate | SS | Any number of copies on top, to a side or at an angle, with a gap | have: Object › Replicate › Advanced Replicate… (Effects panel, `generate.replicate`) | replicate | Replicate panel tab 2 (2026-11-25, SS 5.0.556) |
| Object on Path | SS | Repeat an object along another shape's outline | have: Object › Replicate › Object on Path… (the largest selected shape is the path; Perpendicular, start/increment angle, start position, section length, repeats; `generate.object-on-path`) | replicate | Replicate panel tab 3 (2026-11-25, SS 5.0.556) |
| Fill page | SS | Repeat a shape until the media is full | have: desktop Replicate panel and `shapes.replicate-fill` | replicate (pairs with nesting) | 2026-11-02, SS 5.0.539: Fill Page in the Replicate panel |
| Mirror copy | SS | A mirrored duplicate | have: desktop Replicate panel and `shapes.mirror-copy` | replicate | 2026-11-02, SS 5.0.539: mirrored copy below, above, left or right |
| Nesting (auto pack shapes to save material) | SS (Business) | Pack shapes tightly onto the media | have: Object › Replicate › Nesting… (Effects panel, live preview): packs the selected shapes onto the page, spacing and rotation step (`shapes.nest`; attach groups move as one); the production mat planner nests across mats | replicate | 2026-11-02, SS 5.0.539: the Replicate panel may have a Nesting tab; not confirmed usable in Basic |

## Alignment

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Align left/centre/right/top/middle/bottom | both | Align the selection | have: `shapes.align` | replicate | 2026-11-02, SS 5.0.539: Transform panel, Align tab: horizontal left/centre/right, vertical top/middle/bottom. DS: docs only (sign-in required) |
| Align to page/mat | SS | Align against the media instead of the selection | have: `to_board` | replicate | 2026-11-02, SS 5.0.539: centre to page in the Align tab and the options bar |
| Distribute horizontally/vertically | both | Even spacing | have: `shapes.distribute` | replicate | 2026-11-02, SS 5.0.539: Distribute horizontally/vertically in the Align tab. DS: docs only (sign-in required) |
| Centre to page | SS | Put the selection in the middle of the media | have: one move of the whole selection to the board centre (works for one shape, one undo step) | replicate | 2026-11-02, SS 5.0.539: centre to page and centre on each other in the Align tab |
| Spacing by a set value | SS | Exact gap between objects | have: `shapes.distribute-spacing`, Transform panel Spacing field | replicate | 2026-11-02, SS 5.0.539: Basic has only even distribution (Space Horizontally / Space Vertically); no spacing-by-value control |

## Layers and objects

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Layers panel: add/remove/show/hide | both | List of layers or objects with visibility toggles | have: `layer.add/remove/set-visible`, Layers & Objects panel | replicate | 2026-11-02, SS 5.0.539: no layers panel in Basic's panel column (vendor docs list layers as a Designer feature). DS: docs only (sign-in required) |
| Reorder (bring forward/send back, drag in list) | both | Change the stacking order | have: Object › Arrange (Bring to Front, Bring Forward, Send Backward, Send to Back, Send to Layer) and layers reordered in the Layers panel (`layer.move`) | replicate | 2026-11-02, SS 5.0.539: Object menu has an Arrange submenu; the options bar has arrange too; Arrange: bring to front, send to back, bring forward, send backward, each with a shortcut. DS: docs only (sign-in required) |
| Arrange shortcuts | SS | Keyboard shortcuts for bring to front/forward and send to back/backward | have: Object › Arrange with Cmd+Shift+] / Cmd+] / Cmd+[ / Cmd+Shift+[ through `shapes.arrange` | replicate with the reorder command | 2026-11-02, SS 5.0.539: Ctrl+Shift+] / Ctrl+Shift+[ for front/back, Ctrl+] / Ctrl+[ for one step |
| Lock layer | SS | Stop a layer from being edited | have: the lock column of the Layers panel (`layer.set-locked`; a locked layer's shapes cannot be selected or edited) | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Rename layer/object | both | Give a layer or object a label | have: Layers panel Rename for layers and groups (`layer.rename`, `group.rename`) | replicate | SS: pending. DS: docs only (sign-in required) |
| Move object between layers | SS | Drag an object to another layer | have: Object › Arrange › Send to Layer (`shapes.move-to-layer`) | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Colour Sync | DS | Merge layers of similar colours so fewer mats are used | engine: layer colours + `layer.color-sync` (tolerance, optional merge); UI not wired | reimagine (merge by operation/material) | docs only (sign-in required) |

## Fill and line styles

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Line colour and width | both | Stroke colour, used to group cut operations | partial: a shape is `cut` or `engrave` (`CutStyle`); the Line Style panel sets a line colour (drawn on the canvas) and the style; no line width | reimagine: operation-first, colour as a label | 2026-11-02, SS 5.0.539: Line Style panel: a line-colour tab and thickness in pt; new shapes default to outline only, with a default line colour (preferences). DS: docs only (sign-in required) |
| Line style (dashed, perforated) | SS | Solid or dashed lines, with thickness, corner joins, end caps and stroke position | partial: Line Style › Perforate Lines turns lines into cut dashes (cut and gap lengths); no dashed print style, thickness, joins or caps | replicate (perforation as a cut setting) | 2026-11-02, SS 5.0.539: solid/dash style, thickness (pt), corners miter/round/bevel, end caps butt/round/square, position centre or inside |
| Fill colour, gradient, pattern fill | both | Fill shapes for Print & Cut | partial: the Fill Color panel sets a solid fill (swatches or #rrggbb), drawn on the canvas with holes; no gradients or patterns, and fills are not printed yet | replicate colour/gradient fill for printing; user-supplied patterns only | 2026-11-02, SS 5.0.539: Fill panel tabs Solid/Gradient/Pattern; swatch grid with a no-fill swatch, recent colours, eyedropper, named custom palettes, transparency %, colour spectrum and a hex field. DS: docs only (sign-in required) |
| Print cut lines | SS | Print the outlines of chosen shapes too (zero-width lines print as hairlines) | have: Panels › Print & Cut › Print Cut Lines | replicate | 2026-11-02, SS 5.0.539: a Line Style option to print lines of the selected shapes; 0 pt lines print as 0.1 pt, also in SVG/PDF exports |
| Sketch and Stipple fills | SS | Fill a shape with hatch lines or dots for pens | have: Effect › Sketch Fill… / Stipple… (Effects panel with live preview); `generate.fill`, `generate.release` | replicate (hatch fills for pen/engrave) | 2026-11-02, SS 5.0.539: Sketch fill patterns: cross-hatch, grid, lines, concentric, spiral; spacing and angle; Release Effects; Stipple on its own tab |
| Specialty tools (emboss/deboss, foil) | SS | Patterns and settings for emboss, deboss and foil tools | partial: Effect › Emboss / Deboss Patterns… (Effects panel); `generate.fill`/`generate.tool-lines` tool tags; machine output not wired | replicate the hatch patterns; tool support follows the machine work | 2026-11-02, SS 5.0.539: a panel with tabs per tool type; deboss/emboss toggle; the same five hatch patterns; spacing, angle, Release Effects |
| Leather Punch | SS | Turn a line into evenly spaced punch points | have: Effect › Leather Punch… (punch size, spacing; `generate.punch`); Effect › Release Effects turns generated fills and points back into plain shapes | replicate | Specialty Tools tab (2026-11-25, SS 5.0.556) |
| Pop-up card generator | SS | Generate the struts for a pop-up card from a design | partial: Effect › Pop-up Card… (Effects panel, `generate.popup`), a standalone card with struts; Silhouette's Pop up converts the selected shapes themselves (base strip above, dashed folds; see the observations of 2026-11-26), not yet | reimagine later, low priority | 2026-11-02, SS 5.0.539: on/off with base width %, strut width and dash pitch |
| Vendor pattern libraries | both | Store pattern artwork | out of scope | won't do: licensed assets | 2026-11-02, SS 5.0.539: a Pattern tab exists in the Fill panel; where its patterns come from was not checked. DS: docs only (sign-in required) |
| Operation type (cut, draw/pen, score, deboss, engrave, wave, perforate, foil, print then cut) | DS | Each layer says what the machine does with it | partial: cut and engrave only | replicate the common ones (cut, pen, score, perforate) | docs only (sign-in required) |

## Print & Cut and registration marks

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Registration marks on the printed page | both | Printed marks the cutter's sensor uses to find the artwork | have: Print & Cut panel: Type 1 / Type 2 marks with length, thickness and inset, Print Cut Lines, drawn on the page and printed, as Silhouette's panel | replicate | 2026-11-02, SS 5.0.539: Print & Cut panel (View › Registration Marks) turns marks on; marks are off by default (a preference). DS: docs only (sign-in required) |
| Show print border | SS | Draw the printer's printable area on the page | have: View › Show Print Border (Cmd+Shift+P, off) draws a dashed border a quarter inch inside the page (no printer-specific margins) | replicate | 2026-11-02, SS 5.0.539: View › Show Print Border (Ctrl+Shift+P), off by default |
| Mark style, length, inset choices | SS | Change the size and position of the marks | have: Type 1 (three marks) or Type 2 (four corners), length, thickness, inset (Panels › Print & Cut) | replicate | 2026-11-02, SS 5.0.539: length 0.787 in, thickness 0.020 in, inset 0.625 in by default; More Settings; Restore Defaults |
| Send to printer from the app | both | System print dialog, with bleed and actual-size checks | partial: File › Print (with Print Preview) and the Print & Cut panel's Print write the page (PDF, or the Print & Cut SVG with marks) and open it in the system viewer to print at 100 %; there is no in-app print dialog | replicate | 2026-11-02, SS 5.0.539: File › Print and Print Page Setup; a Print button in the Print & Cut panel; print resolution and print preview are preferences. DS: docs only (sign-in required) |
| Bleed | both | Extend the print colour past the cut line so a slightly-off cut shows no white edge | have: Print & Cut panel › Bleed with a radius (0.05 in default, as Silhouette): each filled shape grows by the radius in its fill colour behind the cut line (`with_bleed`) | replicate | 2026-11-02, SS 5.0.539: Enable Print Bleed with a bleed radius (0.050 in default) in the Print & Cut panel. DS: docs only (sign-in required) |
| Optical sensor registration (automatic alignment) | both | The machine scans the marks before it cuts | missing (`TB5x` commands not driven) | replicate from protocol facts | SS: pending. DS: docs only (sign-in required) |
| Print & Cut calibration | both | Adjust for sensor/printer offset with a test sheet | missing | replicate | SS: pending (2026-11-23, 5.0.556: not found in the Send tab or its gear menu with no machine connected). DS: docs only (sign-in required) |
| Print size limits | DS | Maximum printable area per mat/page | missing | replicate (validation) | docs only (sign-in required) |
| PixScan / scan-to-cut | SS | Photograph the material on a special mat and cut to it | have: File › Scan… / Panels › PixScan: Detect Corners finds the page in a photo of it on a darker surface (or pick the four corners by hand), then Straighten to Page warps it to the page at true size; no camera capture or vendor mat-pattern calibration | done by hand; auto-detect later | 2026-11-02, SS 5.0.539: the PixScan panel opens a camera/scan calibration dialog; File › Scan is also present |

Sources: [Cricut, *How to Print Then Cut*](https://help.cricut.com/hc/en-us/articles/360009387274-How-to-Print-Then-Cut-in-Design-Space), [Cricut, *How large can I Print Then Cut?*](https://help.cricut.com/hc/en-us/articles/360009429814-Design-Space-says-printable-image-is-too-large), [Silhouette, *Print and Cut troubleshooting*](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000276219-print-and-cut-troubleshooting). See also [`print-cut.md`](print-cut.md).

## Cut settings, materials and tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Material presets | both | Choose the material; force, speed and passes fill in from the preset | have: `MEDIA_PRESETS` (27 entries) with a Send-panel chooser | replicate with our own numbers; never import their preset data files | 2026-11-02, SS 5.0.539: Standard sub-tab: material, action and tool dropdowns per tool holder; a long built-in material list; a notice when the material has no action for that holder. DS: docs only (sign-in required) |
| Force, speed, passes override | both | Change the preset values for this job | have: Send tab pressure, speed and passes (bounded by the model), per line colour/fill/layer in those send modes | replicate | 2026-11-02, SS 5.0.539: Material Settings: force, speed, passes, acceleration and a blade-depth dial, each a slider plus a number. DS: docs only (sign-in required) |
| Advanced material settings | SS | Per-material settings beyond force and speed: tracking, kraft-blade loop corners, smart cut | partial: Send › Advanced: track enhancing, overcut, weld on cut, cut order; no kraft-blade loop settings (loop force, height, overcut, radius, smart cut) | replicate the documented behaviours from our own testing | 2026-11-02, SS 5.0.539: Track Enhancing checkbox; the kraft blade adds loop force, loop blade height, loop overcut, loop radius and a smart-cut toggle |
| Blade depth / tool type | SS | Pick the tool (ratchet, auto blade, kraft, rotary, pen) and blade depth | have: Send › Tool (AutoBlade, ratchet, deep-cut, pen) and Set AutoBlade depth | replicate | 2026-11-02, SS 5.0.539: the tool is detected from the holder (it shows awaiting connection without a machine); blade-depth dial for ratchet/auto blades; the kraft blade has its own sub-tab; blade type defaults to automatic |
| Custom user materials | both | Save your own material presets | have: Send › My materials saves, applies and deletes named cut settings | replicate | 2026-11-02, SS 5.0.539: +/− buttons in the material list, Create New…, and Revert / Save As… in Material Settings. DS: docs only (sign-in required) |
| Per-layer/per-colour settings | SS | Different settings per line colour or layer | have: Send › mode by Line Color, Fill Color or Layer with a settings row each (send modes) | replicate | 2026-11-02, SS 5.0.539: Cut by Line Color / Cut by Fill Color tables, one row per colour (own row: Send sub-modes) |
| Test cut | both | Cut a small test shape first | have (not tried on hardware): Machine › Test Cut sends a 10 mm square with a triangle inside to the chosen cutter with the Send settings | replicate | SS: pending (2026-11-23, 5.0.556: Send shows no test control without a connected machine). DS: docs only (sign-in required) |
| Overcut / line segment overcut | SS | Cut a little past corners or ends so pieces lift cleanly | have: Send › Advanced › Overcut (closed contours, 0–10 mm); no kraft-blade loop corners | replicate | 2026-11-02, SS 5.0.539: a Line Segment Overcut checkbox per material; kraft blade adds loop overcut; Preferences › Advanced has Software Overcut (on) |
| Cut order / path sorting | SS | Cut inner shapes first and keep travel short | have: Send › Advanced › Cut order (least roller movement, fastest path, document order) and Cut inner contours first | replicate | 2026-11-02, SS 5.0.539: an Intelligent Path checkbox per material; its behaviour not tested yet |
| Mat-free (carrier sheet free) cutting strategy | SS | Path ordering that lets material be cut without a mat | missing | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Weed lines and weed borders | SS (Business) | Extra cut lines that make weeding easier | have: Effect › Weed Lines and Border… (Effects panel); `generate.weed` | replicate | 2026-11-02, SS 5.0.539: not present in Basic |
| Laser/G-code profiles | — | Not in these apps; StencilCraft's own | partial | — | — |

## Send panel and machine connection

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Design → Send/Make split | both | A separate production screen before cutting | have: Design/Send split with a dry-run program summary | replicate | 2026-11-02, SS 5.0.539: the SEND tab turns the canvas into a cut preview with settings on the right, in Guide / Standard / By Line Color / By Fill Color sub-tabs plus a gear for send preferences. DS: docs only (sign-in required) |
| Send sub-modes: Standard / by line colour / by fill colour | SS | Settings for the whole job, or one row of settings per line or fill colour | have: Machine › Send Mode and the Send tab: Standard, Line Color, Fill Color or Layer; each colour/layer is a row with its own on/off, cut or draw, pressure, speed and passes, cut one after another (not tried on hardware) | reimagine: per-layer/operation settings | 2026-11-02, SS 5.0.539: By Line Color table: tool number, on/off, outline-only, colour, material, action, tool; By Fill Color is the same keyed by fill |
| Assign lines to tool holders on the cut preview | SS | Send the selected lines (or only their outline) to tool 1 or tool 2, or skip them | have: Send view › Selected lines: Tool 1 / Tool 2 › Use all selected lines or Use outline of selection, Ignore selected lines, Clear tool choice (`shapes.set-production` `send`; the cut job honours it) | replicate (dual-holder machines) | 2026-11-02, SS 5.0.539: overlay buttons: tool 1 all lines, tool 1 outline, tool 2 all, tool 2 outline, ignore selected lines |
| Mat preview on the Make screen | DS | Shows how pieces sit on each mat; you can move them there | missing | reimagine | docs only (sign-in required) |
| Machine selection | both | Choose the cutter model | have: model chooser (`Model`, Cameo 3–5, Portrait, Pro) | replicate | 2026-11-02, SS 5.0.539: machine card with a model dropdown (CAMEO 5 by default) and an illustration (own row: machine card). DS: docs only (sign-in required) |
| Machine card (model, firmware, status) | SS | A card showing the chosen machine, its firmware and its state | have: Send › Model chooser, Firmware and Check status (ready, busy, no media; `hardware.status`) for the chosen USB cutter; not tried on a cutter here | replicate | 2026-11-02, SS 5.0.539: model dropdown, firmware (unknown without a machine), status (none), an illustration |
| USB connection | both | Send the job over USB | have: Send tab Find USB cutters, pick one, Send cut over USB (`UsbTransport`, packet size), with the serial transport in the CLI | replicate (`nusb`) | 2026-11-02, SS 5.0.539: Guide sub-tab lists detected USB and Bluetooth devices with Connect; no machine was attached in the VM. DS: docs only (sign-in required) |
| Bluetooth connection | both | Wireless send | partial: Linux paired BLE discovery and send from the Send tab; Classic RFCOMM and other platforms unverified | replicate | 2026-11-02, SS 5.0.539: Bluetooth devices share the same list; a banner said Bluetooth was unavailable in the VM. DS: docs only (sign-in required) |
| Device discovery and status | both | Find connected cutters; show ready/loaded/tool status | have: Send tab Find USB cutters / Bluetooth devices, and Check status for the chosen USB cutter (`hardware.status`); not tried on a cutter here | replicate | 2026-11-02, SS 5.0.539: Rescan for Devices button; with no machine the card shows unknown firmware and no status; a preference releases the device automatically when not in use (off by default). DS: docs only (sign-in required) |
| Packet size | SS | How much data is sent to the machine per write | have: Preferences › Sending › Packet size, Auto (1000 bytes) or 64–65536, applied to every USB write (`PacketTransport`) | replicate if real hardware needs it | 2026-11-02, SS 5.0.539: Preferences › Advanced: packet size Auto (1000) |
| Load/unload media, pause, cancel | both | Machine controls during a job | missing | replicate | SS: pending (2026-11-23, 5.0.556: hidden until a machine is connected). DS: docs only (sign-in required) |
| Multi-cutter support | SS (Business) | Send jobs to several machines at once | have: Send tab › Send to all N cutters (shown with two or more recognised USB cutters): Send cut and Send Queue run the job on each cutter in turn, each with its own model's limits; Cancel stops before the next; not observed (Business), not tried on cutters | replicate later | 2026-11-02, SS 5.0.539: not present in Basic |
| Batch / queued jobs | SS (Business) | Queue several designs or repeats as one job | have: Send tab › Job queue: Add to Queue with a copy count (each design's send jobs as they were when added, up to 100), Remove, Clear Queue, Send Queue cuts them in order and stops between jobs on Cancel; not observed (Business), not tried on a cutter | replicate | 2026-11-02, SS 5.0.539: not present in Basic |
| Cricut machine protocol | DS | Driving Cricut machines | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | docs only (sign-in required) |
| Firmware updates | both | Update the machine firmware from the app | out of scope | won't do: vendor firmware | SS: pending. DS: docs only (sign-in required) |

## Library and store

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Local design library | SS | Organise designs on disk in folders | have: File › Library… (Library panel): folders (New Folder, open, Up), designs saved into the shown folder, search, favourites (listed first), Delete to the Recycle Bin with Restore and Empty (`stencilcraft_doc::library::Library`); no drag-and-drop moves between folders | reimagine: plain folders of `.stencilcraft` files | 2026-11-02, SS 5.0.539: LIBRARY tab; a folder button on the tool rail opens a library sidebar; Save to Library and Update Library in the File menu; the default save location (disk or library) is a preference |
| Design store / Cricut Access images and projects | both | Buy or subscribe to designs | out of scope | won't do: proprietary cloud store and licensed assets | 2026-11-02, SS 5.0.539: STORE tab present; not explored. DS: docs only (sign-in required) |
| Ready-to-make projects | DS | Project templates from the vendor | out of scope | won't do: licensed assets; we may ship original example files | docs only (sign-in required) |
| Templates (mockups like shirts and mugs) | DS | Background images to size designs against | missing | reimagine with original, code-generated templates | docs only (sign-in required) |

## File import and export formats

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| SVG import | SS (Designer+), DS | Open vector artwork | have: a working subset (see [`gaps.md`](gaps.md) 1) | replicate | 2026-11-02, SS 5.0.539: Basic's Open dialog has no SVG. DS: docs only (sign-in required) |
| Merge (import into current document) | SS | Open a file's contents into the document already open | have: Import SVG/Image add to the open document | replicate | 2026-11-02, SS 5.0.539: File › Merge (Ctrl+Shift+O) |
| Open Recovered (autosave recovery) | SS | Reopen work saved automatically before a crash | have: the desktop snapshots a changed document every 30 s (saving discards it); File › Open Recovered… and the Library panel open or discard earlier sessions' snapshots | replicate | 2026-11-02, SS 5.0.539: File › Open Recovered… |
| DXF import | both | Open CAD outlines | partial: CLI/MCP import supported entities; complete fidelity and desktop workflow remain open | replicate | 2026-11-02, SS 5.0.539: DXF is in Basic's Open types; import options: fit to page, convert white lines to black, join lines into polylines. DS: docs only (sign-in required) |
| PDF import | SS (Designer+) | Open vector PDFs | partial: File › Import › PDF… in the desktop, CLI/MCP; vector paths only (images, text and fills are not preserved) | replicate | 2026-11-02, SS 5.0.539: Basic's Open dialog has no PDF |
| AI/EPS/CDR import | SS (Business) | Open Illustrator/CorelDRAW files | partial: File › Import › AI / EPS… brings in vector paths; CDR unsupported | replicate AI (its PDF-compatible content) and EPS (a PostScript subset); CDR only if its structure can be learned clean-room | 2026-11-02, SS 5.0.539: not present in Basic |
| Raster import (PNG/JPEG/BMP/GIF/TIFF) | both | Place a photo or clip art to trace or print | partial: engine places bounded raster images; the desktop places PNG/JPEG at 300 dpi as SS does, draws them under the cut lines, and picks, drags and deletes them; printing them is missing | replicate | 2026-11-02, SS 5.0.539: PNG, JPEG, BMP, GIF and TIFF in Basic's Open types; PNG autotrace on import is on by default. DS: docs only (sign-in required) |
| Embroidery formats (PES, DST, …) | SS (Designer Plus) | Import embroidery outlines | missing | replicate (outline only) | 2026-11-02, SS 5.0.539: not present in Basic |
| Native `.studio`/`.studio3` | SS | The vendor's own formats: Basic saves `.studio3`, `.studio` and GSP and opens those plus GSD | partial: File › Open reads `.studio` and `.studio3` (every outline, the page size) upright, as a new unsaved document; no writer yet (decoded from scratch, see docs/observations), GSD or GSP | reimagine: import needs a public spec or a clean-room study of files users own (owner decision); until then users export DXF | 2026-11-02, SS 5.0.539: Basic's save types: Studio V3 (.studio3), V2 (.studio), GSP; GSD open options: as-is, convert white lines to black, join lines into polylines |
| SVG export | SS (Business) | Save as SVG | have | replicate | 2026-11-02, SS 5.0.539: Basic has no SVG export (and no DXF, PDF or PNG) |
| PDF/PNG/JPG export | SS (Business) | Save as PDF or image | partial: File › Export › PDF…, PNG…, JPEG… (and CLI/MCP); PNG and JPEG render fills and placed images; PDF has the lines only (placed images: craft-libs #3655; fills to do) | replicate (PDF first) | 2026-11-02, SS 5.0.539: not present in Basic |
| DXF export | — | StencilCraft's own | have (R14, flattened) | — | — |
| G-code export | — | StencilCraft's own | have | — | — |
| Plugins for external design tools | SS (Silhouette Connect) | Send straight from Illustrator/CorelDRAW to the cutter | have (reimagined): `stencilcraft-cli cut` and the MCP server take SVG, PDF, DXF and AI/EPS from any design tool and send them to the cutter; no plug-in inside proprietary apps, by design | reimagine: CLI/MCP and SVG/PDF interchange; never tie the core to proprietary software | pending |
| Cloud-only save | DS | Projects saved to the vendor cloud | out of scope | won't do: account system; we save open, local files | docs only (sign-in required) |

Sources: [Silhouette, *File Types and Features*](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000282776-file-types-and-features), [Cricut, *Image uploads: unsupported items*](https://help.cricut.com/hc/en-us/articles/360009553213-Image-uploads-unsupported-items).

## Preferences

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Theme / appearance | both | Light or dark interface | have: suite light/dark themes, saved between sessions | replicate | 2026-11-02, SS 5.0.539: Display tab: theme (Light by default, with a choice), background colour, button size, animation speed, antialiasing and curve quality (the last two need a restart). DS: docs only (sign-in required) |
| Print resolution and preview | SS | The dpi used for printing and whether a preview is shown first | have: Preferences › Print: Show print preview (on: File › Print and Print & Cut › Print… open a preview of the page with its marks first) and Print resolution 300 / 600 (default) / 1200 dpi or custom, used for the printed PDF | replicate with printing | 2026-11-02, SS 5.0.539: General tab: print resolution 300/600 (default)/1200/custom dpi; print preview on |
| Reset and maintenance options | SS | Restore defaults, show hidden dialogs, graphics settings | have: Preferences › Maintenance › Restore Factory Defaults… (asks first; keeps recent files, presets, styles, actions and dictionary words) and Panels › Reset Panels to Default; no Restore Pre-loaded Designs or Unhide All Dialogs (StencilCraft ships neither) | replicate | 2026-11-02, SS 5.0.539: Advanced tab: restore factory defaults, restore pre-loaded designs, unhide dialogs, library permissions, OpenGL settings |
| Usage data collection | SS | Opt-in performance data sent to the vendor | out of scope: StencilCraft sends nothing | won't do (owner decision if this ever changes) | 2026-11-02, SS 5.0.539: an opt-in setting, off by default |
| Units and grid spacing | both | Default units; grid size | have: Preferences › Units (mm/cm/in) and Page Setup grid spacing and divisions | replicate | 2026-11-02, SS 5.0.539: General tab: unit of measurement, Auto (Inches); Show Dimensions is on; grid spacing in the Grid Options dialog. DS: docs only (sign-in required) |
| Default material/machine | both | Settings that start each document and job: orientation, blade, page colour, fill style, text style | have: the Send tab remembers the machine model, media and mat between runs (`remember_machine_prefs`) | replicate | 2026-11-02, SS 5.0.539: Defaults tab: page orientation and blade type are automatic; default page colour; default fill style outline-only with default line/fill colours; default text style; always show the mat; cut to page edge; registration marks off. DS: docs only (sign-in required) |
| Import defaults (DPI, scale) | SS | How imported files are sized | have: Preferences › Import: DXF opens Fit to Page or As-is, Join lines into polylines, PNG Autotrace (a PNG with transparency comes in with a cut line round its opaque area); no white-to-black line conversion (GSD/DXF colours) | replicate | 2026-11-02, SS 5.0.539: Import tab: options for GSD and DXF (white lines to black, join into polylines, DXF fit to page) and PNG autotrace on import. 2026-11-23, SS 5.0.556: PNGs are placed at 300 dpi at the page's top-left, ignoring the file's own DPI (a 600 × 400 px PNG tagged 150 dpi still came in at 2.000 × 1.333 in) |
| Workspace layout | SS | Panel arrangement | have: docked panels with tabs, Reset Panels to Default, and Preferences › Panel mode: Flexible (tabs), Single (one panel at a time) or Multiple (panels stack in a column), as Silhouette's Panel Mode; no scrollbar-side or button-size preferences | replicate | 2026-11-02, SS 5.0.539: panels open from a column of icons on the right as floating, collapsible, closable docked panels; scrollbar side is a preference; Starter mode hides most of them (own row) |
| Recent files | both | Reopen recent documents | have: eight-entry Recent menu | replicate | 2026-11-02, SS 5.0.539: File › Open Recent; the Home screen lists recent library files. DS: docs only (sign-in required) |
| Language | both | UI language | missing | replicate later | 2026-11-02, SS 5.0.539: General tab: language, default English. DS: docs only (sign-in required) |

## Account and cloud

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Sign-in / account | both | A vendor account: DS needs one before any design screen opens; in SS it is optional (store and cloud) | out of scope | won't do: account systems; StencilCraft works offline and needs no account | 2026-11-02, SS 5.0.539: not asked to sign in during this session. DS: docs only (sign-in required) |
| Cloud library sync | both | Sync designs across devices | out of scope | won't do: no hosted service; users sync files with their own tools | SS: pending. DS: docs only (sign-in required) |
| Purchased-design management | both | Manage licences for bought designs | out of scope | won't do: proprietary store | SS: pending. DS: docs only (sign-in required) |
| Offline mode | DS | Work without a connection (limited) | have: StencilCraft is offline by design | — | docs only (sign-in required) |
| Mobile apps | both | Phone and tablet clients | missing | reimagine as the WASM/browser build (M5) | SS: pending. DS: docs only (sign-in required) |
| Machine registration | both | Register a machine to an account | out of scope | won't do: account system | SS: pending. DS: docs only (sign-in required) |

## File formats

Every format either app opens, imports, saves or exports, per edition. SS rows marked
"observed" were seen in 5.0.539 Basic; the rest come from the vendor pages cited above.
Owner goal: support every format the source apps support.

| Format | App / edition | Direction | StencilCraft status | Posture |
| --- | --- | --- | --- | --- |
| `.stencilcraft` (JSON) | StencilCraft | open, save | have | ours |
| `.studio3` (Studio V3) | SS Basic+ | open, save (observed 5.0.539) | partial: lossy reader (all outlines incl. text and compound paths, page size); no writer; writer: decoded from scratch for pages, layers and rectangles, and generated files open in Silhouette (docs/observations); the Rust writer is next | reimagine: bounded import of unprotected documents; writing remains open |
| `.studio` (Studio V2) | SS Basic+ | open, save (observed 5.0.539) | partial: lossy reader (all outlines incl. text and compound paths, page size); no writer | reimagine: bounded import of unprotected documents; writing remains open |
| GSP | SS Basic+ | save (observed 5.0.539) | missing: layout mapped (header, XOR 0xA9 body, GP-GL program with binary BE/BD draws, see docs/observations/silhouette-studio.md); the vertical half of the binary step is not decoded yet | reimagine: needs a public spec or a clean-room study of files users own (owner decision); until then users export DXF/SVG |
| GSD (older Graphtec Studio) | SS Basic+ | open (observed 5.0.539) | missing | reimagine: needs a public spec or a clean-room study of files users own (owner decision); until then users export DXF/SVG |
| DXF | SS Basic+ | open (observed 5.0.539) | partial: import supported entities; R14 flattened export | replicate |
| DXF | DS desktop | upload (not on iOS/Android) | partial: import supported entities; R14 flattened export | replicate |
| SVG | SS Designer+ | open | import partial (subset) | replicate |
| SVG | SS Business | export | have | replicate |
| SVG | DS | upload | import partial (subset) | replicate |
| PDF | SS Designer+ | open | partial: vector-path import and stroke export; artwork fidelity incomplete | replicate |
| PDF | SS Business | export | partial: vector-path import and stroke export; artwork fidelity incomplete | replicate |
| PDF | DS | not accepted for upload (help centre) | partial: vector-path import and stroke export; artwork fidelity incomplete | replicate (for SS parity) |
| AI (Illustrator) | SS Business | open | partial: vector paths only | replicate via its PDF-compatible content |
| EPS | SS Business | open | partial: vector paths only | replicate (PostScript subset) |
| CDR (CorelDRAW) | SS Business | open | missing | replicate only if its structure can be learned clean-room |
| PNG | SS Basic+ | open (observed 5.0.539; autotrace on import) | partial: placed raster import, shown on the canvas, traced, exported in PNG/JPEG; not yet in printed PDF or Print & Cut sheets (craft-libs #3594, #3655) | replicate (also placed raster for printing) |
| PNG | SS Business | export | have: CLI/MCP rendered image export | replicate |
| PNG | DS | upload | partial: placed raster import, shown on the canvas, traced, exported in PNG/JPEG; not yet in printed PDF or Print & Cut sheets (craft-libs #3594, #3655) | replicate |
| JPEG | SS Basic+ | open (observed 5.0.539) | partial: placed raster import, shown on the canvas, traced, exported in PNG/JPEG; not yet in printed PDF or Print & Cut sheets (craft-libs #3594, #3655) | replicate |
| JPEG | SS Business | export | have: CLI/MCP rendered image export | replicate |
| JPEG | DS | upload | partial: placed raster import, shown on the canvas, traced, exported in PNG/JPEG; not yet in printed PDF or Print & Cut sheets (craft-libs #3594, #3655) | replicate |
| BMP | SS Basic+, DS | open / upload | partial: placed raster import, shown on the canvas, traced, exported in PNG/JPEG; not yet in printed PDF or Print & Cut sheets (craft-libs #3594, #3655) | replicate |
| GIF | SS Basic+, DS | open / upload | partial: placed raster import, shown on the canvas, traced, exported in PNG/JPEG; not yet in printed PDF or Print & Cut sheets (craft-libs #3594, #3655) | replicate |
| TIFF | SS Basic+ | open (observed 5.0.539) | partial: placed raster import, shown on the canvas, traced, exported in PNG/JPEG; not yet in printed PDF or Print & Cut sheets (craft-libs #3594, #3655) | replicate |
| TIFF | DS | not accepted for upload (help centre) | partial: placed raster import, shown on the canvas, traced, exported in PNG/JPEG; not yet in printed PDF or Print & Cut sheets (craft-libs #3594, #3655) | replicate (for SS parity) |
| HEIC | DS | upload (listed in one help article) | missing | replicate |
| WebP | DS | not documented in the help articles we found | partial: placed raster import; desktop display/print incomplete | verify, then replicate if supported |
| Embroidery: PES, DST, EXP, JEF, XXX | SS Designer Plus | open | missing | replicate (outline only) |
| Fonts: TTF/OTF (system fonts) | SS, DS | used for text | have: installed families, the built-in single-line font, and Text Style › Load Font File… (TTF, OTF, TTC); text made in a loaded family records the file and reloads it when the design is opened | replicate through system/user fonts; none bundled |
| Pattern images (JPG/PNG) | SS Designer+, DS | imported as fill patterns | missing | replicate |
| Design Space projects | DS | cloud-only; no local project file | n/a (we save `.stencilcraft` locally) | won't do: account system |
| G-code (Grbl) | StencilCraft | export | have | ours |
## Cutters

Every machine either app drives. Silhouette USB IDs and widths come from the device table of
[inkscape-silhouette](https://github.com/fablabnbg/inkscape-silhouette) (GPL, read as a
specification only; see [`machine.md`](machine.md)). Tools and Cricut sizes are from vendor
pages and marked "to verify" where they are not settled. StencilCraft's `Model` table in
`crates/machine` currently covers CAMEO 3/4/4 Plus/4 Pro/5/5 Plus/Pro MK-II and Portrait 3/4,
with a dry-run encoder and an opt-in serial transport (no USB bulk or Bluetooth yet).
Owner goal: support every cutter the source apps drive.

| Machine | Connection | Protocol | Tools / holders | Max media / mats | StencilCraft status | Posture |
| --- | --- | --- | --- | --- | --- | --- |
| Silhouette SD | USB `0b4d:111c` / `0b4d:111d` | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 1 holder | to verify | missing | replicate |
| Silhouette CAMEO (1) | USB `0b4d:1121` | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 1 holder | 304 mm wide; 12×12 / 12×24 mat; rolls to 3 m | missing | replicate |
| Silhouette CAMEO 2 | USB `0b4d:112b` | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 1 holder | 304 mm wide; 12×12 / 12×24 mat; 3 m | missing | replicate |
| Silhouette CAMEO 3 | USB `0b4d:112f`, Bluetooth | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 2 holders; AutoBlade | 304.8 mm wide; 12×12 / 12×24 mat; 3 m | partial: in `Model`; dry-run encode; serial only | replicate |
| Silhouette CAMEO 4 | USB `0b4d:1137`, Bluetooth | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 2 holders; AutoBlade, ratchet, kraft, rotary, pens | 304.8 mm wide; 12×12 / 12×24 mat; 3 m | partial: in `Model`; dry-run encode; serial only | replicate |
| Silhouette CAMEO 4 Plus | USB `0b4d:1138`, Bluetooth | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 2 holders, as CAMEO 4 | 372 mm wide; 15×15 mat; 3 m | partial: in `Model`; dry-run encode; serial only | replicate |
| Silhouette CAMEO 4 Pro | USB `0b4d:1139`, Bluetooth | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 2 holders, as CAMEO 4 | 600 mm wide; 24×24 mat; 3 m | partial: in `Model`; dry-run encode; serial only | replicate |
| Silhouette CAMEO 5 | USB `0b4d:1140`, Bluetooth | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 2 holders (tool range per vendor docs, to verify) | 330.2 mm wide; 12×12 / 12×24 mat; 3 m | partial: in `Model`; dry-run encode; serial only | replicate |
| Silhouette CAMEO 5 Plus | USB `0b4d:1141`, Bluetooth | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 2 holders (to verify) | 372 mm wide; 15×15 mat; 3 m | partial: in `Model`; dry-run encode; serial only | replicate |
| Silhouette CAMEO 5 Alpha / Alpha Plus | USB `3844:0001` / `3844:0002` | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification); four-mark registration | to verify | 330.2 / 372 mm wide; 3 m | missing | replicate |
| Silhouette CAMEO Pro MK-II | USB `0b4d:1146` | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification); four-mark registration | to verify | 609 mm wide; rolls to 18.29 m | partial: in `Model`; dry-run encode; serial only | replicate |
| Silhouette Portrait (1) | USB `0b4d:1123` | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 1 holder | 206 mm wide; 8×12 mat | missing | replicate |
| Silhouette Portrait 2 | USB `0b4d:1132`, Bluetooth | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 1 holder | 203 mm wide; 8×12 mat | missing | replicate |
| Silhouette Portrait 3 | USB `0b4d:113a`, Bluetooth | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 1 holder; AutoBlade | 203 mm wide; 8×12 mat; long media | partial: in `Model`; dry-run encode; serial only | replicate |
| Silhouette Portrait 4 | USB `0b4d:113f`, Bluetooth | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 2 holders (to verify) | 216 mm wide; 8.5×12 mat; long media | partial: in `Model`; dry-run encode; serial only | replicate |
| Silhouette Curio | USB (ID not in our sources) | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) (to verify) | 2 holders; emboss/etch/stipple tools; platform base | 8.5×6 in base, 8.5×12 extended (to verify) | missing | replicate once its IDs and commands are confirmed from public sources or observation |
| Silhouette Curio 2 | USB, Bluetooth (ID not in our sources) | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) (to verify) | 2 holders; emboss/etch tools | to verify | missing | replicate once its IDs and commands are confirmed |
| Graphtec Craft ROBO CC200-20 | USB `0b4d:110a` | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 1 holder | 200 mm wide; 1 m | missing | replicate |
| Graphtec Craft ROBO CC300-20 | USB `0b4d:111a` | Graphtec GPGL-style ASCII (facts from GPL drivers used as a specification) | 1 holder | to verify | missing | replicate |
| Graphtec CE/CB professional plotters | USB, serial, network (vendor docs) | Graphtec GP-GL / HP-GL (vendor-published command references) | 1–2 tools | to verify per model | missing | replicate from Graphtec's published command references; whether SS drives them is to verify |
| "Silhouette Joy" | — | — | — | — | n/a | no such Silhouette machine found; Joy is a Cricut machine (below) |
| Cricut Explore / Explore One | USB; Bluetooth via adapter | Cricut proprietary | Explore: 2 clamps (A accessory, B blade); One: 1 | 12×12 / 12×24 mat | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review |
| Cricut Explore Air / Air 2 | USB, Bluetooth | Cricut proprietary | 2 clamps; fine/deep-point blades, pens, scoring stylus, foil | 12×12 / 12×24 mat | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review |
| Cricut Explore 3 | USB, Bluetooth | Cricut proprietary | 2 clamps, as Air 2 | 12×12 / 12×24 mat; Smart Materials without a mat up to 12 ft | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review |
| Cricut Maker / Maker 3 | USB, Bluetooth | Cricut proprietary | Adaptive tools: rotary, knife (multi-pass), scoring wheel, debossing, engraving, perforation, wave, foil | 12×12 / 12×24 mat; Maker 3: Smart Materials up to 12 ft | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review |
| Cricut Joy | Bluetooth | Cricut proprietary | 1 tool: blade, pens, foil | 4.5×6.5 / 4.5×12 mat, Card Mat; Smart Materials up to 20 ft | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review |
| Cricut Joy Xtra | Bluetooth | Cricut proprietary | 1 tool: blade, pens, foil; Print Then Cut | 8.5×12 mat; Smart Materials 9.5 in wide | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review |
| Cricut Venture | USB, Bluetooth | Cricut proprietary | Performance blade, pens (to verify) | 24×28 mat; rolls up to 25 in wide | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review |

## Silhouette Studio paid editions

Features the paid editions (Designer, Designer Plus, Business) add over Basic, from
the [edition comparison](https://www.silhouetteamerica.com/software), the
[file types and features](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000282776-file-types-and-features)
and [Designer Edition features](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000273383-designer-edition-features)
pages. "to confirm" means the public pages do not settle which edition adds it. Some rows
repeat a row in the area tables above, so this table can be read on its own.

| Feature | Edition | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| SVG import | Designer | Open SVG files | partial: a tested subset (see `gaps.md` 1) | replicate | not present in Basic; from vendor docs (not observed) |
| PDF import | Designer | Open the vector content of PDF files | partial: File › Import › PDF… in the desktop, CLI/MCP; vector paths only (images, text and fills are not preserved) | replicate | not present in Basic; from vendor docs (not observed) |
| Layers panel | Designer | Create, rename, reorder, hide and lock layers; move objects between layers | partial: `layer.add/remove/set-visible` only | replicate | not present in Basic; from vendor docs (not observed) |
| Cut by layer | Designer (to confirm) | Send settings and cut order chosen per layer | partial: per-layer skip, holder, passes, force, speed (`cut --layer`); layers cut in document order | replicate | not present in Basic; from vendor docs (not observed) |
| Rhinestone tool | Designer | Place stones along a line, around an outline or as a fill; stone size, spacing and shape | have: Effect › Rhinestone… (Effects panel with live preview); `generate.rhinestone` | replicate (stone templates are just circles we generate) | not present in Basic; from vendor docs (not observed) |
| Sketch effect | Designer (a Sketch panel was also visible in Basic 5.0.539) | Turn fills into hatch lines a pen can draw | have: Effect › Sketch Fill… (Effects panel); `generate.fill` | replicate | vendor docs list it as Designer; a Sketch panel was visible in Basic 5.0.539 (2026-11-02) |
| Custom gradient and pattern fills | Designer | Edit gradients, scale and rotate patterns, add your own pattern images (Basic has solid fill and transparency) | missing | replicate with user-supplied patterns only | not present in Basic; from vendor docs (not observed) |
| Eraser and knife options | SS (Basic, observed 5.0.556) | Eraser size and shape, knife in straight, curved or polyline mode | have: options bar Solid/Outline, knife gap, eraser Round/Square and size | replicate | not present in Basic; from vendor docs (not observed) |
| Eyedropper tool | Designer (to confirm) | Copy fill and line style from one shape to another | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Rulers and measure tool | Designer | Measure distances and angles on the canvas | partial: rulers only | replicate | not present in Basic; from vendor docs (not observed) |
| Shear and distort | Designer | Slant a shape or drag its corners freely | have: shear (Transform panel); Effect › Distort › Free Distort and Lattice Distort drag a grid of handles over the selection (`shapes.distort`, bilinear per cell, live preview) | replicate | not present in Basic; from vendor docs (not observed) |
| Lattice | Designer (to confirm) | Turn a shape into a lattice of cut-out holes | have: Effect › Lattice… (squares, circles, diamonds or hexagons of a chosen size and wall, inside a border; `generate.lattice`, live preview) | replicate | not present in Basic; from vendor docs (not observed) |
| Font creator | Designer (to confirm) | Draw your own font, one glyph at a time, including single-line fonts | missing | replicate (writes a user font file; nothing bundled) | not present in Basic; from vendor docs (not observed) |
| Glyphs panel | Designer | Insert special characters of the current font | have: Type › Glyphs (the framework's Glyphs panel): every character of the text font by category (Punctuation, Currency, Arrows, …), recently used, zoom; double-click or Enter inserts | replicate | not present in Basic; from vendor docs (not observed) |
| Weld on cut | paid (edition to confirm) | Merge overlapping shapes automatically when sending, so overlaps are not cut twice | partial: `cut --weld`, MCP `weld` (job layer); not in the desktop Send panel | replicate | not present in Basic; from vendor docs (not observed) |
| Roll feed / mat-free cutting | paid (edition to confirm) | Cut long jobs straight from a roll without a mat, with path ordering that keeps the material steady | partial: model tables carry roll lengths; no roll-feed choice or mat-free ordering | replicate | not present in Basic; from vendor docs (not observed) |
| Cross-stitch patterns | Designer (to confirm) | Turn a design into a cross-stitch pattern for pens | missing | replicate, low priority | not present in Basic; from vendor docs (not observed) |
| Embroidery import (PES, DST, EXP, JEF, XXX) | Designer Plus | Open embroidery files as outlines | missing | replicate (outline only) | not present in Basic; from vendor docs (not observed) |
| Advanced warp (conical, perspective) | Designer Plus | Warp onto a cone (cups, tumblers) or into perspective | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Flexishapes | Designer Plus | Parametric shapes with handles you can drag | have (engine/MCP, no UI yet): `generate.shape` with editable params (`replace`) | replicate | not present in Basic; from vendor docs (not observed) |
| Puzzle generator | Designer Plus | Cut a design or photo into jigsaw pieces | have: Effect › Puzzle… (Effects panel); `generate.puzzle` (over a shape, not a photo) | replicate, low priority | not present in Basic; from vendor docs (not observed) |
| SVG export | Business | Save as SVG | have | replicate | not present in Basic; from vendor docs (not observed) |
| PDF export | Business | Save as PDF | have: File › Export › PDF… (also PNG and JPEG at 300 dpi) | replicate | not present in Basic; from vendor docs (not observed) |
| PNG/JPG export | Business | Save as an image | missing | replicate | not present in Basic; from vendor docs (not observed) |
| AI, EPS and CDR import | Business | Open Illustrator, EPS and CorelDRAW files | missing | replicate AI (its PDF-compatible content) and EPS (a PostScript subset); CDR only if its structure can be learned clean-room | not present in Basic; from vendor docs (not observed) |
| Multi-cutter support | Business | Drive several machines from one app, each with its own job | have: Send tab › Send to all N cutters (shown with two or more recognised USB cutters): Send cut and Send Queue run the job on each cutter in turn, each with its own model's limits; Cancel stops before the next; not observed (Business), not tried on cutters | replicate | not present in Basic; from vendor docs (not observed) |
| Nesting | Business | Pack shapes tightly onto the media to save material | partial: production mat planner supports nesting; desktop and hardware workflow remain open | replicate | not present in Basic; from vendor docs (not observed) |
| Tiling | Business | Split a design larger than the media into tiles | have: `tiling` crate | replicate | not present in Basic; from vendor docs (not observed) |
| Weed lines and borders | Business | Add cut lines around and between shapes so waste lifts in pieces | have: Effect › Weed Lines and Border… (Effects panel); `generate.weed` | replicate | not present in Basic; from vendor docs (not observed) |
| Job queue / batch | Business (to confirm) | Queue several designs or copies as one production run | missing | replicate | not present in Basic; from vendor docs (not observed) |

## Cricut Design Space, full inventory

Every Design Space feature found in Cricut's public documentation. The details, options,
platform differences and sources for each are in the
[Cricut Design Space feature library](cricut-design-space/README.md); each row links to
its entry. Design Space needs sign-in, so every row is "docs only". Status is checked
against the engine command set on main (2026-11-02).

| Feature | Area | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Home screen | Home | Browse and search projects, images, fonts ([library](cricut-design-space/home-and-projects.md#home-screen)) | have: the title bar's Home shows the start screen (New, Open, recent files) | reimagine: new/recent local files; vendor content won't do | docs only (sign-in required) |
| New Project / change base type | Home | Empty Canvas; base type changeable from the Canvas preview ([library](cricut-design-space/home-and-projects.md#new-project)) | have: `document.new`; no base types | replicate (document presets) | docs only (sign-in required) |
| Ready-to-Make projects | Home | Vendor projects to customise ([library](cricut-design-space/home-and-projects.md#ready-to-make-projects)) | out of scope | won't do: store content | docs only (sign-in required) |
| My Stuff | Home | Saved and bookmarked projects, uploads ([library](cricut-design-space/home-and-projects.md#my-stuff)) | partial: Recent menu | reimagine: local folders | docs only (sign-in required) |
| Collections | Home | Folders of projects/uploads, 1,000 per collection ([library](cricut-design-space/home-and-projects.md#collections)) | missing | reimagine: local folders | docs only (sign-in required) |
| Save / Save As (cloud) | Home | Save to the account cloud; copy under a new name ([library](cricut-design-space/home-and-projects.md#save-save-as-auto-save)) | have: local Save / Save As | won't do cloud (account); local save kept | docs only (sign-in required) |
| Auto Save | Home | Automatic saving, on by default for new projects ([library](cricut-design-space/home-and-projects.md#save-save-as-auto-save)) | missing | replicate (local autosave and recovery) | docs only (sign-in required) |
| Project sharing | Home | Publish a project or share a link, with photos and tags ([library](cricut-design-space/home-and-projects.md#project-sharing)) | out of scope | won't do: account-bound cloud service | docs only (sign-in required) |
| Heat Guide link | Home | Link to vendor heat-press settings ([library](cricut-design-space/home-and-projects.md#heat-guide-link)) | out of scope | won't do: store content | docs only (sign-in required) |
| AI Project Designer | Home | Conversational project builder using credits ([library](cricut-design-space/home-and-projects.md#ai-project-designer)) | out of scope | won't do: account-bound cloud service with paid credits; MCP lets any agent drive StencilCraft instead | docs only (sign-in required) |
| Help chatbot | Home | In-app help chat, docked or floating ([library](cricut-design-space/home-and-projects.md#help-chatbot)) | out of scope | won't do: account-bound cloud service | docs only (sign-in required) |
| Canvas grid and units | Canvas | Grid on/off; inches or cm ([library](cricut-design-space/canvas.md#canvas-grid-and-units)) | partial: grid toggle; mm only | replicate | docs only (sign-in required) |
| Snap to Align | Canvas | Alignment guides while dragging ([library](cricut-design-space/canvas.md#snap-to-align-alignment-guides)) | missing | replicate | docs only (sign-in required) |
| Smart Guides: equal distance | Canvas | Spacing markers for equal gaps while moving ([library](cricut-design-space/canvas.md#smart-guides-equal-distance)) | missing | replicate | docs only (sign-in required) |
| Operation-type display | Canvas | How operation types are shown on the Canvas ([library](cricut-design-space/canvas.md#operation-type-display)) | partial: cut/engrave production colours | replicate | docs only (sign-in required) |
| Light mode | Canvas | Light or dark interface (iOS) ([library](cricut-design-space/canvas.md#light-mode)) | have: light/dark themes | replicate | docs only (sign-in required) |
| Zoom and pan | Canvas | Zoom and pan the Canvas ([library](cricut-design-space/canvas.md#zoom-and-pan)) | have | replicate | docs only (sign-in required) |
| Selection | Canvas | Click, Shift/Ctrl-click to add ([library](cricut-design-space/canvas.md#selection)) | have | replicate | docs only (sign-in required) |
| Undo/redo | Canvas | Step back and forward ([library](cricut-design-space/canvas.md#undo-and-redo)) | have: `edit.undo`/`edit.redo` | replicate | docs only (sign-in required) |
| Live preview on real objects | Canvas | Preview the project on real objects (2026) ([library](cricut-design-space/canvas.md#live-preview-on-real-objects)) | missing | reimagine: mock-up preview from user photos | docs only (sign-in required) |
| Templates (Guide layers) | Design panel | Sizing backgrounds that are never cut ([library](cricut-design-space/design-panel.md#templates)) | missing | reimagine with original templates; Guide operation replicate | docs only (sign-in required) |
| Projects panel | Design panel | Open vendor projects ([library](cricut-design-space/design-panel.md#projects-from-the-canvas)) | out of scope | won't do: store content | docs only (sign-in required) |
| Monogram Maker | Design panel | Monogram from up to 4 initials, style and frame ([library](cricut-design-space/design-panel.md#monogram-maker)) | partial: engine only (`text.monogram`, circle/square/diamond/scallop frames, one size for all initials) | replicate with user fonts and our own frames | docs only (sign-in required) |
| Editable images | Design panel | Library images with editable text ([library](cricut-design-space/design-panel.md#editable-images)) | out of scope (vendor images) | reimagine: templates with editable text placeholders | docs only (sign-in required) |
| Phrases | Design panel | Unconfirmed in public docs ([library](cricut-design-space/design-panel.md#phrases)) | n/a | verify | docs only (sign-in required) |
| AI Create / AI Edit / AI Styles | Design panel | Generate or restyle images with credits ([library](cricut-design-space/design-panel.md#ai-create-and-ai-edit)) | out of scope | won't do: account-bound cloud service with paid credits | docs only (sign-in required) |
| AI Generated image filter | Design panel | Filter for AI-made images ([library](cricut-design-space/design-panel.md#ai-generated-image-filter)) | out of scope | won't do: account-bound cloud service | docs only (sign-in required) |
| Add text | Text | Text box on the Canvas ([library](cricut-design-space/text.md#add-text)) | have: click the canvas with the Text tool and type (`text.add`, then `text.edit` per key) | replicate | docs only (sign-in required) |
| Font picker with filters | Text | System/Cricut tabs, search, keywords, kerned filter ([library](cricut-design-space/text.md#font-picker)) | partial: desktop family picker uses `text.fonts`; no search, filter or vendor fonts | replicate for system/user fonts; vendor fonts won't do (store) | docs only (sign-in required) |
| Font style incl. writing | Text | Regular, bold, italic, single-line writing ([library](cricut-design-space/text.md#font-style)) | partial: engine styles of installed families plus the built-in single-line font | replicate | docs only (sign-in required) |
| Font size | Text | Point size ([library](cricut-design-space/text.md#font-size)) | partial: desktop dialog edits point size; no contextual toolbar | replicate | docs only (sign-in required) |
| Letter space | Text | Spacing between letters ([library](cricut-design-space/text.md#letter-space)) | partial: engine only, no UI yet | replicate | docs only (sign-in required) |
| Line space | Text | Spacing between lines ([library](cricut-design-space/text.md#line-space)) | partial: engine only, no UI yet | replicate | docs only (sign-in required) |
| Text alignment | Text | Left, centre, right ([library](cricut-design-space/text.md#text-alignment)) | partial: engine only, no UI yet | replicate | docs only (sign-in required) |
| Curve | Text | Bend one line into an arc by diameter ([library](cricut-design-space/text.md#curve)) | partial: engine only, no UI yet | replicate | docs only (sign-in required) |
| Ungroup to lines / letters | Text | Split text into lines, then letters ([library](cricut-design-space/text.md#ungroup-to-lines--ungroup-to-letters)) | partial: engine only, no UI yet | replicate | docs only (sign-in required) |
| Text clipboard | Text | Cut/copy/paste letters in a box ([library](cricut-design-space/text.md#text-editing-clipboard)) | missing | replicate | docs only (sign-in required) |
| Shapes panel | Shapes | Basic shapes plus a score line ([library](cricut-design-space/shapes.md#shapes-panel)) | partial: rect, rounded rect, ellipse, polygon, star; no heart/triangle presets or score line | replicate (shapes generated in code) | docs only (sign-in required) |
| More Shapes | Shapes | Library shapes ([library](cricut-design-space/shapes.md#more-shapes)) | out of scope | won't do: store content | docs only (sign-in required) |
| Score line | Shapes | A line for fold scoring ([library](cricut-design-space/shapes.md#score-line)) | missing | replicate | docs only (sign-in required) |
| Unlocked proportions | Shapes | Stretch with the size lock open ([library](cricut-design-space/shapes.md#unlocked-proportions)) | have: aspect link off by default | replicate | docs only (sign-in required) |
| Images panel | Upload | Vendor library search and filters ([library](cricut-design-space/images-and-upload.md#images-panel)) | out of scope | won't do: store content; reimagine as browsing the user's folders | docs only (sign-in required) |
| Upload panel | Upload | Upload images, SVGs, other files ([library](cricut-design-space/images-and-upload.md#upload-panel-v108-desktop)) | partial: SVG import and PNG/JPEG trace | replicate | docs only (sign-in required) |
| Upload formats (JPG, PNG, GIF, BMP, HEIC, SVG, DXF) | Upload | Accepted formats; no TIFF/PDF; SVG max 5,000 paths ([library](cricut-design-space/images-and-upload.md#supported-upload-formats)) | partial: PNG, JPEG, SVG (bounded import) | replicate | docs only (sign-in required) |
| Automatic background remover | Upload | One-click background removal (Access) ([library](cricut-design-space/images-and-upload.md#automatic-background-remover)) | missing | reimagine (local, deterministic) | docs only (sign-in required) |
| Manual Select / Erase / Restore | Upload | Remove same-colour areas; erase/restore brush ([library](cricut-design-space/images-and-upload.md#manual-cleanup-select-erase-restore)) | missing | replicate | docs only (sign-in required) |
| Reduce Colors / Color Tolerance | Upload | Simplify an upload's colours ([library](cricut-design-space/images-and-upload.md#manual-cleanup-select-erase-restore)) | partial: trace threshold | replicate | docs only (sign-in required) |
| Despeckle and Smooth | Upload | Clean specks, smooth edges (iOS) ([library](cricut-design-space/images-and-upload.md#despeckle-and-smooth)) | missing | replicate | docs only (sign-in required) |
| Preview single layer | Upload | Show the upload as one cut layer ([library](cricut-design-space/images-and-upload.md#preview-single-layer)) | missing | replicate | docs only (sign-in required) |
| Save as Cut vs Print Then Cut image | Upload | Upload becomes outline or printable image ([library](cricut-design-space/images-and-upload.md#save-as-cut-image-or-print-then-cut-image)) | partial: uploads become cut outlines | replicate | docs only (sign-in required) |
| Upload patterns | Upload | User images as pattern fills ([library](cricut-design-space/images-and-upload.md#upload-patterns)) | missing | replicate | docs only (sign-in required) |
| Download for offline | Upload | Download images/fonts for offline use, with time limits ([library](cricut-design-space/images-and-upload.md#downloading-images-for-offline-use)) | have: everything is local | n/a (no DRM time limits) | docs only (sign-in required) |
| Operation | Edit bar | Per-layer machine action ([library](cricut-design-space/edit-bar.md#operation)) | partial: cut/engrave | replicate | docs only (sign-in required) |
| Fill: original / colour / pattern | Edit bar | Print fill for Print Then Cut layers ([library](cricut-design-space/edit-bar.md#fill)) | missing | replicate | docs only (sign-in required) |
| Edit Pattern | Edit bar | Scale, move, rotate a pattern fill ([library](cricut-design-space/edit-bar.md#edit-pattern)) | missing | replicate | docs only (sign-in required) |
| Linetype colour | Edit bar | Colour that groups layers onto mats ([library](cricut-design-space/edit-bar.md#linetype-colour)) | partial: fixed production colours | reimagine: operation first, colour label | docs only (sign-in required) |
| Select all / deselect | Edit bar | Select everything or nothing ([library](cricut-design-space/edit-bar.md#select-all--deselect)) | have | replicate | docs only (sign-in required) |
| Cut, copy, paste | Edit bar | Clipboard ([library](cricut-design-space/edit-bar.md#edit-menu-cut-copy-paste)) | have: Cut/Copy/Paste/Paste in Front | replicate | docs only (sign-in required) |
| Align and distribute | Edit bar | Align 2+ objects; distribute evenly ([library](cricut-design-space/edit-bar.md#align)) | have: `shapes.align`, `shapes.distribute` | replicate | docs only (sign-in required) |
| Arrange | Edit bar | To back, backward, forward, to front ([library](cricut-design-space/edit-bar.md#arrange)) | have: Object › Arrange and shortcuts (`shapes.arrange`) | replicate | docs only (sign-in required) |
| Flip | Edit bar | Flip horizontal/vertical ([library](cricut-design-space/edit-bar.md#flip--mirror)) | have: Object › Mirror (`transform.each` scale −1); `shapes.mirror-copy` makes a copy | replicate | docs only (sign-in required) |
| Size | Edit bar | Exact W/H, 0.1 steps, proportion lock ([library](cricut-design-space/edit-bar.md#size)) | have: size fields with aspect link | replicate | docs only (sign-in required) |
| Rotate | Edit bar | Exact angle, 1° steps ([library](cricut-design-space/edit-bar.md#rotate)) | have: `shape.rotate`, `shapes.transform` | replicate | docs only (sign-in required) |
| Position | Edit bar | Exact X/Y ([library](cricut-design-space/edit-bar.md#position)) | have: `shape.set-bounds` | replicate | docs only (sign-in required) |
| Offset | Edit bar | Outline at a distance; round/sharp corners; weld offsets ([library](cricut-design-space/edit-bar.md#offset)) | partial: desktop Offset panel previews and applies offsets; no weld-offsets option | replicate | docs only (sign-in required) |
| Warp (30 styles) | Edit bar | Preset warps with a strength slider (Access) ([library](cricut-design-space/edit-bar.md#warp)) | partial (engine/MCP): `generate.warp`, 15 styles with bend and distortion | replicate | docs only (sign-in required) |
| Create Sticker (die/kiss cut, border) | Edit bar | One-step sticker: flatten, border, die or kiss cut (Access) ([library](cricut-design-space/edit-bar.md#create-sticker)) | partial: engine/MCP `shapes.create-sticker`; desktop and hardware depth validation remain open | replicate (offset + flatten + cut depth) | docs only (sign-in required) |
| Group / ungroup | Layers panel | Move and size together ([library](cricut-design-space/layers-panel.md#group--ungroup)) | have: Group/Ungroup (menu, Cmd+G / Cmd+Shift+G); groups select and move as one | replicate | docs only (sign-in required) |
| Duplicate | Layers panel | Copy the selection ([library](cricut-design-space/layers-panel.md#duplicate)) | have: `shapes.duplicate` | replicate | docs only (sign-in required) |
| Delete | Layers panel | Remove the selection ([library](cricut-design-space/layers-panel.md#delete)) | have: `shapes.remove` | replicate | docs only (sign-in required) |
| Hide layer / object | Layers panel | Hidden items are not cut or printed ([library](cricut-design-space/layers-panel.md#hide--show-layer)) | have: `layer.set-visible`, `shapes.set-hidden` | replicate | docs only (sign-in required) |
| Rename and reorder layers | Layers panel | Rename; drag to reorder ([library](cricut-design-space/layers-panel.md#rename-and-reorder-layers)) | partial: engine/MCP layer rename and move commands; desktop drag reordering remains open | replicate | docs only (sign-in required) |
| Slice | Layers panel | Split two overlapping layers ([library](cricut-design-space/layers-panel.md#slice)) | have: Pathfinder `divide` | replicate | docs only (sign-in required) |
| Weld | Layers panel | Merge permanently into one outline ([library](cricut-design-space/layers-panel.md#weld)) | have: Pathfinder `union` | replicate | docs only (sign-in required) |
| Combine (non-destructive Unite/Subtract/Intersect/Exclude) | Layers panel | Shape operations that stay editable ([library](cricut-design-space/layers-panel.md#combine-unite-subtract-intersect-exclude)) | partial: engine/MCP preserve source members and support uncombine; desktop workflow remains open | replicate (live boolean groups) | docs only (sign-in required) |
| Attach / detach | Layers panel | Keep positions on the mat; fasten draw/score to cut ([library](cricut-design-space/layers-panel.md#attach--detach)) | partial: engine/MCP support attach/detach; desktop workflow remains open | replicate | docs only (sign-in required) |
| Flatten / unflatten | Layers panel | Merge into one printable layer ([library](cricut-design-space/layers-panel.md#flatten--unflatten)) | partial: engine/MCP commands exist; complete print rendering and desktop workflow remain open | replicate | docs only (sign-in required) |
| Contour | Layers panel | Hide individual cut lines ([library](cricut-design-space/layers-panel.md#contour)) | have: Object › Contour… panel (`shape.contour`) | replicate | docs only (sign-in required) |
| Color Sync | Layers panel | Merge colours to use fewer mats ([library](cricut-design-space/layers-panel.md#color-sync-panel)) | partial: engine/MCP `layer.color-sync`; desktop workflow remains open | replicate | docs only (sign-in required) |
| Basic Cut | Operations | Cut the outline ([library](cricut-design-space/operations-and-linetypes.md)) | have: `cut` style | replicate | docs only (sign-in required) |
| Pen / Draw | Operations | Draw with a pen ([library](cricut-design-space/operations-and-linetypes.md)) | partial: `engrave` style | replicate | docs only (sign-in required) |
| Score | Operations | Fold line with stylus or wheel ([library](cricut-design-space/operations-and-linetypes.md)) | partial: `engrave` style | replicate | docs only (sign-in required) |
| Engrave | Operations | Engraving tip on blanks up to 3 mm ([library](cricut-design-space/operations-and-linetypes.md)) | partial: `engrave` style | replicate | docs only (sign-in required) |
| Deboss | Operations | Press a line in ([library](cricut-design-space/operations-and-linetypes.md)) | missing | replicate | docs only (sign-in required) |
| Wave | Operations | Wavy cut ([library](cricut-design-space/operations-and-linetypes.md)) | missing | replicate | docs only (sign-in required) |
| Perforate | Operations | Dashed tear line ([library](cricut-design-space/operations-and-linetypes.md)) | missing | replicate | docs only (sign-in required) |
| Foil | Operations | Foil transfer tool ([library](cricut-design-space/operations-and-linetypes.md)) | missing | replicate | docs only (sign-in required) |
| Print Then Cut | Operations | Print, then cut around ([library](cricut-design-space/operations-and-linetypes.md)) | partial: printable sheet; no sensor | replicate | docs only (sign-in required) |
| Guide | Operations | Shown only, never sent ([library](cricut-design-space/operations-and-linetypes.md)) | missing | replicate | docs only (sign-in required) |
| Mat preview | Make | One mat per material/colour ([library](cricut-design-space/make-screen.md#prepare-screen-mat-preview)) | missing | replicate | docs only (sign-in required) |
| Arrange on mat; move to another mat | Make | Drag, hide, move pieces between mats ([library](cricut-design-space/make-screen.md#move-object-to-another-mat)) | missing | replicate | docs only (sign-in required) |
| Project copies | Make | Cut N copies, spilling onto new mats ([library](cricut-design-space/make-screen.md#project-copies)) | partial: engine `shapes.replicate-array`/`-fill` on the design; no per-job copies | replicate | docs only (sign-in required) |
| Mirror per mat (auto for iron-on) | Make | Mirror one mat; automatic for iron-on/Infusible Ink ([library](cricut-design-space/make-screen.md#mirror-per-mat)) | missing | replicate | docs only (sign-in required) |
| Material size | Make | Mat or sheet size filtered by machine and design ([library](cricut-design-space/make-screen.md#material-size)) | partial: `Mat` table | replicate | docs only (sign-in required) |
| Material load type | Make | On mat / without mat; default per machine ([library](cricut-design-space/make-screen.md#material-load-type)) | partial: `Mat::None` | replicate | docs only (sign-in required) |
| Connect machine | Make | USB or Bluetooth ([library](cricut-design-space/make-screen.md#connect-machine)) | partial: serial in CLI | replicate (Silhouette); Cricut pending owner decision | docs only (sign-in required) |
| Base material selection | Make | Popular, bookmarked, or full list ([library](cricut-design-space/make-screen.md#base-material-selection)) | partial: 27 presets | replicate | docs only (sign-in required) |
| Pressure: default / more / less | Make | Per-cut adjustment, not saved ([library](cricut-design-space/make-screen.md#pressure-default--more--less)) | partial: force override | replicate | docs only (sign-in required) |
| Remember material settings | Make | Keep material between mats of the project ([library](cricut-design-space/make-screen.md#remember-material-settings)) | missing | replicate | docs only (sign-in required) |
| Load tools and materials list | Make | Which tool and material for each mat ([library](cricut-design-space/make-screen.md#load-tools-and-materials-list)) | missing | replicate | docs only (sign-in required) |
| Machine buttons (Load, Go, Pause) | Make | Machine controls light up when ready ([library](cricut-design-space/make-screen.md#machine-buttons-load-go-pause)) | missing | replicate | docs only (sign-in required) |
| Custom materials | Materials | Pressure, multi-cut, blade; steps of 2–4 ([library](cricut-design-space/materials-and-tools.md#custom-materials)) | missing | replicate | docs only (sign-in required) |
| Smart Materials matless limits | Materials | Min and max lengths per machine ([library](cricut-design-space/materials-and-tools.md#smart-materials-matless-cutting)) | missing | replicate (validation) | docs only (sign-in required) |
| Card mats and insert cards | Materials | Card Mat 2×2, 1×2, Joy/Joy Xtra card mats ([library](cricut-design-space/materials-and-tools.md#mats)) | missing | replicate (mat geometry) | docs only (sign-in required) |
| Blade and tool catalogue | Materials | Blades and tips with machine compatibility ([library](cricut-design-space/materials-and-tools.md#blades-and-tools)) | partial: Silhouette `ToolKind` | replicate | docs only (sign-in required) |
| Print Then Cut workflow | Print Then Cut | Print with sensor marks, scan, cut ([library](cricut-design-space/print-then-cut.md#print-then-cut-workflow)) | partial: `print-cut` sheet | replicate | docs only (sign-in required) |
| Bleed toggle | Print Then Cut | Print past the cut line; on by default ([library](cricut-design-space/print-then-cut.md#bleed)) | missing | replicate | docs only (sign-in required) |
| System print dialog | Print Then Cut | Print through the OS, no scaling ([library](cricut-design-space/print-then-cut.md#system-print-dialog)) | missing | replicate | docs only (sign-in required) |
| Sensor marks rules | Print Then Cut | Marks must not be scaled; same session ([library](cricut-design-space/print-then-cut.md#sensor-marks)) | partial: fixed marks, actual-size printing documented | replicate | docs only (sign-in required) |
| Print Then Cut size limits and page size | Print Then Cut | Usable area per machine and page ([library](cricut-design-space/print-then-cut.md#maximum-size)) | missing | replicate (validation) | docs only (sign-in required) |
| Print Then Cut calibration | Print Then Cut | Test sheet aligning sensor and printer ([library](cricut-design-space/print-then-cut.md#calibration)) | missing | replicate | docs only (sign-in required) |
| StickerPix printers | Print Then Cut | Cricut photo/sticker printers (2026) ([library](cricut-design-space/print-then-cut.md#stickerpix-printers)) | missing | reimagine: system printing | docs only (sign-in required) |
| Cricut Explore Air 2 / Explore 3 | Machines | Two-clamp mid-size cutters ([library](cricut-design-space/machines.md)) | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | docs only (sign-in required) |
| Cricut Explore 5 (2026) | Machines | New Explore with Scoring Tool and card mats ([library](cricut-design-space/machines.md)) | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | docs only (sign-in required) |
| Cricut Maker / Maker 3 | Machines | Adaptive-tool cutters ([library](cricut-design-space/machines.md)) | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | docs only (sign-in required) |
| Cricut Maker 5 (2026) | Machines | Adaptive tools, materials to 3 mm ([library](cricut-design-space/machines.md)) | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | docs only (sign-in required) |
| Cricut Joy / Joy Xtra / Joy 2 | Machines | Small Bluetooth cutters ([library](cricut-design-space/machines.md)) | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | docs only (sign-in required) |
| Cricut Venture | Machines | Large-format roll cutter ([library](cricut-design-space/machines.md)) | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | docs only (sign-in required) |
| Machine setup and firmware | Machines | Register and update machines ([library](cricut-design-space/machines.md#machine-setup-and-firmware)) | out of scope | won't do: account registration and vendor firmware | docs only (sign-in required) |
| Settings: machine defaults | Settings | Default load type and Print Then Cut page size per machine ([library](cricut-design-space/settings-and-account.md#settings--machines)) | missing | replicate | docs only (sign-in required) |
| Keyboard shortcuts list | Settings | In-app shortcut list ([library](cricut-design-space/shortcuts.md)) | have: Help › Keyboard Shortcuts | replicate | docs only (sign-in required) |
| Cricut ID sign-in | Account | Required before designing ([library](cricut-design-space/settings-and-account.md#cricut-id-sign-in)) | out of scope | won't do: account system | docs only (sign-in required) |
| Cricut Access subscription | Account | Vendor content and some tools ([library](cricut-design-space/settings-and-account.md#cricut-access)) | out of scope | won't do: proprietary store; the gated tools themselves are replicated | docs only (sign-in required) |
| Cricut Credits | Account | Credits for AI generation ([library](cricut-design-space/settings-and-account.md#cricut-credits-ai)) | out of scope | won't do: account-bound cloud service | docs only (sign-in required) |
| Offline mode | Account | Downloaded content with time limits ([library](cricut-design-space/settings-and-account.md#offline-mode)) | have: offline by design | replicate (no limits) | docs only (sign-in required) |
| Desktop platforms | Account | Windows and Mac only ([library](cricut-design-space/settings-and-account.md#desktop-platforms)) | have: also Linux | replicate and extend | docs only (sign-in required) |
| Clear application cache | Account | Troubleshooting step ([library](cricut-design-space/settings-and-account.md#clear-application-cache-desktop)) | n/a | n/a | docs only (sign-in required) |

## Remaining work, by area

The tables above distinguish usable workflows from engine-only and incomplete features.
The ranked, implementation-level list is in [`gaps.md`](gaps.md). The most significant
remaining work includes desktop integration of live text, placed images, production and
multi-page controls; print and cutter hardware validation; full native-format fidelity;
and broader import/export fidelity. Cloud services, vendor assets and protected designs
remain outside the project’s implementation scope. Entries based solely on vendor
materials are not treated as observed behaviour.
