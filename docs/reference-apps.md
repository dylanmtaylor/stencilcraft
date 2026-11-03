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
| Silhouette Studio | 5.0.x; observed: 5.0.539 Basic edition, Windows 11 VM, 2026-11-02 | Windows | [Silhouette Studio overview](https://www.silhouetteamerica.com/silhouette-studio), [software suite and editions](https://www.silhouetteamerica.com/software), [support knowledge base: Studio features](https://silhouetteamerica.freshdesk.com/support/solutions/folders/35000233809), [file types and features by edition](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000282776-file-types-and-features), [Designer Edition features](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000273383-designer-edition-features), the in-app Help › User's Manual |
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
| Page/media size | SS | Set the media width and height, or choose a preset size; the design area matches the material | have | replicate | 2026-11-02, SS 5.0.539: Page Setup panel (Media tab): machine, feed type, mat and media size, each defaulting to Auto (12×12 in); custom width/height, an option to keep the media inside the mat, page colour and transparency |
| Home screen | SS | A start screen with new-document choices, recent files and a vendor news feed | missing (the Home button fits the view) | reimagine: new/recent only; no vendor news feed | 2026-11-02, SS 5.0.539: a New Document carousel, Recent Library Files and a News & Updates feed; a first-run dialog asks if you are a beginner or experienced |
| Starter mode (simplified UI toggle) | SS | A switch that hides most toolbars and panels for beginners | missing (the Essentials workspace switcher is the nearest thing) | reimagine as a workspace preset | 2026-11-02, SS 5.0.539: a top-bar toggle; on, it hides the quick-access toolbar and most panels; off, it shows the full toolbar, the options bar and about 15 panel icons |
| Design page settings | SS | A quick dialog for the page and mat settings | partial: board size is set in the Properties panel | replicate | 2026-11-02, SS 5.0.539: View › Show Design Page Settings (Ctrl+F1) |
| Cutting mat selection and backdrop | both | Choose the mat (12×12, 12×24, …); the canvas shows it behind the design | have: `Mat` table in `crates/machine`, mat backdrop in the app | replicate | 2026-11-02, SS 5.0.539: a grey mat is drawn around the white page, with a feed-direction arrow at the top; the mat is picked in Page Setup; a preference keeps the mat always on screen. DS: docs only (sign-in required) |
| Document tabs | SS | Several open documents as tabs, with a button for a new tab | missing: one document per window, one board | replicate | 2026-11-02, SS 5.0.539: tabs above the canvas (one per document) with a + button; Close Tab is in the File menu; only one page per tab was seen |
| Infinite canvas with auto mat layout | DS | Design on a free canvas; the Make screen lays shapes out onto mats per colour/material | missing | reimagine: bridge to nesting and tiling | docs only (sign-in required) |
| Orientation and rotated view | SS | Portrait/landscape feed and a rotated view of the mat | partial: the encoder can emit the feed direction (`TB50`), but the UI does not expose it | replicate | 2026-11-02, SS 5.0.539: Page Setup has portrait/landscape and a rotated view (0/90/180/270); View menu has Rotate Page; page orientation defaults to automatic in Preferences |
| Cut border / safe area | SS | A border around the media that the blade will not enter | partial: plot margins exist in `PlotOptions`; nothing is drawn on the canvas | replicate | 2026-11-02, SS 5.0.539: Show Cut Border (on by default) outlines the area the blade can reach; a preference allows cutting to the edge of the page |
| Rulers, grid, guides | both | Rulers on the canvas edges, a background grid, draggable guides | partial: rulers (with zero reset) and grid toggle exist; no guides | replicate | 2026-11-02, SS 5.0.539: View menu has Show Grid and a Grid Options dialog; Page Setup has a Grid tab; rulers and guides not checked yet. DS: docs only (sign-in required) |
| Snap to grid | both | Objects snap to grid lines | missing | replicate | 2026-11-02, SS 5.0.539: Snap to Grid in the View menu; object snapping is a separate row (Smart Snapping). DS: docs only (sign-in required) |
| Smart snapping (to objects) | SS | Snap to other objects' edges and centres while moving | missing | replicate | 2026-11-02, SS 5.0.539: View › Enable Smart Snapping, on by default |
| Zoom, pan, fit | both | Zoom tools, hand pan, fit to page/selection | have: Hand, Zoom, Fit (F), Center (C), status-bar zoom | replicate | 2026-11-02, SS 5.0.539: full toolbar has zoom in/out, zoom to fit, zoom to selection and a pan hand; View menu has a Zoom submenu. DS: docs only (sign-in required) |
| Live dimension callouts while drawing | SS | Width and height labels follow the shape as you drag, and stay on the selection | missing | replicate | 2026-11-02, SS 5.0.539: width shown below and height on the left, in inches; they update during an offset preview; Show Dimensions is a preference (on) |
| Measure | SS | Measure a distance on the canvas | missing | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Units | both | Choose inches or millimetres; the default is inches | partial: everything is millimetres; no inch display option | replicate (display-only conversion at the UI boundary) | 2026-11-02, SS 5.0.539: default units are inches (preference set to Auto (Inches)); page size, dimension callouts and offset distances all show inches. DS: docs only (sign-in required) |
| Kerf/compensation preview | — | Not a reference-app feature; StencilCraft's own | have | — | — |
| Oversized design tiling | SS (Business) | Split a design larger than the media into tiles that are cut separately | have: `tiling` crate, `tile-preview`, overlap and marks | replicate | 2026-11-02, SS 5.0.539: not present in Basic; needs a paid edition to observe |

## Drawing tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Rectangle, rounded rectangle | both | Drag to draw; the corner radius can be changed | have: `shape.add-rect`, `shape.add-rounded-rect` | replicate | 2026-11-02, SS 5.0.539: Shape flyout has rectangle and rounded rectangle; width and height callouts follow the drag. DS: docs only (sign-in required) |
| Ellipse/circle | both | Drag to draw; hold a modifier for a circle | have: `shape.add-ellipse` | replicate | 2026-11-02, SS 5.0.539: ellipse in the Shape flyout. DS: docs only (sign-in required) |
| Polygon, star | both | Regular polygon or star; the number of sides or points can be changed | have: `shape.add-polygon`, `shape.add-star` | replicate | 2026-11-02, SS 5.0.539: polygon in the Shape flyout; no star tool was seen. DS: docs only (sign-in required) |
| Basic shape palette (heart, triangle, score line, …) | DS | A panel of ready-made shapes | have (engine/MCP, no UI yet): `generate.shape` heart, triangle, arrow, banner and more; `generate.score-line` | reimagine with shapes generated in code; never copy their shape artwork | docs only (sign-in required) |
| Line | SS | Straight line tool | partial: the pen draws open paths | replicate | 2026-11-02, SS 5.0.539: Line Tools flyout: line, polyline, curve, arc |
| Polyline/Bezier pen | both | Click to place points, drag for curves | have: `PenDraft` → `shape.add-path` | replicate | 2026-11-02, SS 5.0.539: polyline and curve (Bezier) tools in the Line Tools flyout. DS: docs only (sign-in required) |
| Freehand draw and smooth freehand | SS | Draw by hand, with optional smoothing | missing | replicate | 2026-11-02, SS 5.0.539: Freehand Tools flyout: freehand and smooth freehand; the tool stays active after a stroke |
| Arc | SS | Draw an arc segment | missing | replicate | 2026-11-02, SS 5.0.539: arc in the Line Tools flyout |
| Knife | both | Cut a shape apart along a drawn line | missing | replicate | 2026-11-02, SS 5.0.539: knife tool at the bottom of the tool rail; the tool stays active after use. DS: docs only (sign-in required) |
| Eraser | SS | Erase part of a closed shape with a brush | missing | replicate | 2026-11-02, SS 5.0.539: eraser tool in the tool rail; the tool stays active after use |
| Flexishapes | SS (Designer Plus) | Parametric shape presets you can edit | have (engine/MCP, no UI yet): `generate.shape` with editable params (`replace`) | reimagine (parametric shapes) | 2026-11-02, SS 5.0.539: not present in Basic |
| Rhinestone, cross-stitch, puzzle generator | SS (Designer+) | Special pattern generators | partial (engine/MCP): `generate.rhinestone`, `generate.puzzle`; no cross-stitch | reimagine later, low priority | 2026-11-02, SS 5.0.539: not present in Basic (Sketch and Stipple are in Basic: see the Fill and line styles rows) |

## Text

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Text object with system fonts | both | Type text in an installed font; edit it later | partial: the Text tool places single-line engraved lettering from a built-in bitmap glyph set (80 characters max); no font support, the text cannot be edited later | replicate with user fonts read through `CRAFT_FONTS_DIR`/system fonts; no fonts bundled | 2026-11-02, SS 5.0.539: text tool with a font picker (frequently used, recently used, all fonts), size in pt (72 by default), bold/italic/underline; a text box/notes tool is also in the rail. DS: docs only (sign-in required) |
| Vendor/licensed font libraries | both | Fonts from the vendor store or subscription | out of scope | won't do: licensed fonts and assets | SS: pending. DS: docs only (sign-in required) |
| Size, letter spacing, line spacing, alignment | both | Typography controls | missing | replicate | 2026-11-02, SS 5.0.539: Text Style panel: line spacing %, character spacing %, a kerning toggle, left/centre/right/justify, horizontal or vertical text, a preview-text field. DS: docs only (sign-in required) |
| Text on path / curve text | both | Lay text along a path (SS) or bend it into an arc (DS Curve) | missing | replicate | 2026-11-02, SS 5.0.539: a text-on-path control in the text options bar. DS: docs only (sign-in required) |
| Spelling check | SS | Check the spelling of text objects | missing | reimagine later, low priority | 2026-11-02, SS 5.0.539: a Spelling tab in the Text Style panel and Panels › Spelling (F7) |
| Convert text to path / ungroup to letters | both | Turn letters into editable outlines | partial: placed text is already a path | replicate once real text exists | 2026-11-02, SS 5.0.539: Object menu has Convert to Path. DS: docs only (sign-in required) |
| Writing fonts (single-line) | both | Single-stroke fonts for pens | partial: the built-in lettering is single-line engrave | replicate (user-supplied single-line fonts) | SS: pending. DS: docs only (sign-in required) |
| Glyphs panel | SS (Designer) | Insert special characters from a font | missing | replicate | 2026-11-02, SS 5.0.539: not in Basic's Panels menu |

## Editing and node tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Select, multi-select, box/lasso | both | Pick objects; add to the selection with a modifier | have: click/box/lasso, additive multi-select | replicate | 2026-11-02, SS 5.0.539: 8-handle bounding box; Select All and Deselect All in the Edit menu; a drag box selects shapes it touches by default (a preference); many selected shapes share one bounding box. DS: docs only (sign-in required) |
| Move, rotate, scale, exact size/position fields | both | Transform panel with numeric fields | have: `shape.move/rotate/scale/set-bounds`, `shapes.transform` | replicate | 2026-11-02, SS 5.0.539: Transform panel with Align, Scale, Rotate and Move tabs; the options bar also has W/H/X/Y fields (own row). DS: docs only (sign-in required) |
| Flip/mirror | both | Mirror horizontally/vertically | partial: possible through `shapes.transform` with negative scale; no menu item or button | replicate | 2026-11-02, SS 5.0.539: Object menu has a Mirror submenu. DS: docs only (sign-in required) |
| Duplicate, cut/copy/paste | both | Copy objects; paste in place | partial: `shapes.duplicate`; no clipboard | replicate | 2026-11-02, SS 5.0.539: Cut/Copy/Paste and Duplicate (Ctrl+D) in the Edit menu; Paste in Front is its own row. DS: docs only (sign-in required) |
| Group/ungroup | both | Treat objects as one unit | missing: the document has layers, but no groups | replicate | 2026-11-02, SS 5.0.539: Group and Ungroup in the Object menu, the toolbar and the selection options bar. DS: docs only (sign-in required) |
| Compound path make/release | SS | Join paths into one shape with holes | partial: Pathfinder Merge/Union makes compound shapes; no release | replicate | 2026-11-02, SS 5.0.539: Make/Release Compound Path in the Object menu and the Modify panel |
| Point editing | both | Move, add and delete points; change smooth/corner points | partial: `BezierOverlay` direct select and convert-point | replicate | 2026-11-02, SS 5.0.539: point edit tool in the rail; Bezier control handles show for selected points only by default (a preference); Point Editing panel: delete point, break path, corner, smooth, make a segment flat or curved, simplify, convert to path. DS: docs only (sign-in required) |
| Node count readout | SS | Counts for the selection: shapes, contours, nodes, straight and curved nodes | missing | replicate (cheap, helps judge cut complexity) | 2026-11-02, SS 5.0.539: shown in the Point Editing panel |
| Break/join path, close path | SS | Split a path at a point; join two endpoints | missing | replicate | 2026-11-02, SS 5.0.539: the Point Editing panel has Break Path; no join-path command was found in Basic |
| Simplify / reduce points | SS | Remove extra points within a tolerance | missing | replicate | 2026-11-02, SS 5.0.539: Object menu has Simplify (Ctrl+J) |
| Warp / distort | SS (Designer), DS | Bend a shape with an envelope or perspective | partial (engine/MCP): `generate.warp`, 15 envelope styles and perspective; no conical | reimagine, low priority | 2026-11-02, SS 5.0.539: not present in Basic. DS: docs only (sign-in required) |
| Undo/redo history | both | Multi-step undo | have: journal-based undo/redo | replicate | 2026-11-02, SS 5.0.539: Undo/Redo in the Edit menu and the toolbar. DS: docs only (sign-in required) |
| Selection options bar (W/H with aspect lock, X/Y with reference point) | SS | Size and position fields for the selection above the canvas | partial: position and size (mm) fields in Properties and an aspect-link setting; no reference-point picker | replicate | 2026-11-02, SS 5.0.539: fill/line swatches and line width, then W/H with an aspect lock and X/Y with a reference-point picker, then align, group, arrange, weld, offset, replicate and delete |
| Rotation knob on the selection | SS | A handle above the bounding box to rotate by dragging | missing (rotate fields and ±15° actions only) | replicate | 2026-11-02, SS 5.0.539: a green round handle above the 8-handle box |
| Paste in Front | SS | Paste a copy at the same place, on top | missing | replicate | 2026-11-02, SS 5.0.539: Edit › Paste in Front (Ctrl+F) |
| Select by Color | SS | Select every shape with a given line or fill colour | missing | reimagine: select by operation/colour | 2026-11-02, SS 5.0.539: Edit › Select by Color and a Select by Color panel |
| Tool shortcuts | SS | Single-letter keys switch tools | have: rail tools have single-key shortcuts | replicate | 2026-11-02, SS 5.0.539: typing on the canvas changes the active tool |
| Action after tool use | SS | Whether a tool stays active or returns to Select after use | missing: no preference | replicate as a preference | 2026-11-02, SS 5.0.539: Tools preferences: shape tools keep drawing; freehand, eraser and knife stay active; zoom and eyedropper return to Select; right-click on later shapes selects them |
| Drag-select mode (touching vs enclosed) | SS | Whether a drag box selects shapes it touches or only those inside it | have: left-to-right selects enclosed, right-to-left selects touching | reimagine (by drag direction, not a preference) | 2026-11-02, SS 5.0.539: a Tools preference, defaulting to shapes touching the box |
| Lock objects | SS | Stop an object from being selected or moved | missing | replicate | not present in Basic (2026-11-02, SS 5.0.539) |

## Offset

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Outward/inward offset of a shape | both | Make a new outline a set distance outside or inside the shape | partial: the `kerf` crate offsets contours for kerf compensation, but there is no user offset command that adds a new shape | replicate as an engine command on top of `kerf` | 2026-11-02, SS 5.0.539: Offset panel shows a live preview on the selection before Apply; the dimension callouts update to the offset size; Cancel discards it. DS: docs only (sign-in required) |
| Corner style (round/sharp) | both | Choose rounded or sharp corners, with a limit on how far sharp corners reach | partial: kerf uses a miter join only | replicate | 2026-11-02, SS 5.0.539: sharp or round corners plus a corner limit value (1.0 default); the live test defaulted to round at 0.125 in, the panel showed 0.079 in. DS: docs only (sign-in required) |
| Weld offsets / per-letter offsets | DS | Merge the offset outlines into one, or keep one per piece | missing | replicate | docs only (sign-in required) |
| Internal offset | SS | Offset inward, used for frames and borders | partial (kerf counters) | replicate | 2026-11-02, SS 5.0.539: Internal Offset is a mode beside Offset in the same panel |

Sources: [Cricut, *How to use the Offset feature*](https://help.cricut.com/hc/en-us/articles/360061650414-How-to-use-the-Offset-feature-in-Design-Space).

## Trace

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Threshold trace of a raster image | both | Choose an area or the whole image; turn dark pixels into cut outlines | have: `trace-image` CLI and the Trace Image tool; PNG/JPEG; adjustable threshold | replicate | 2026-11-02, SS 5.0.539: Trace panel: select the trace area, then threshold % and despeckle threshold %; PNG files are auto-traced on import by default (a preference). DS: docs only (sign-in required) |
| High/low-pass filter, scale and colour-range trace | SS | Image filters and colour selection that guide the trace | missing | replicate | 2026-11-02, SS 5.0.539: an Advanced section in the Trace panel holds the filters (not expanded yet) |
| Image effects | SS | Brightness, colour and other adjustments on placed images | missing | reimagine later, low priority | 2026-11-02, SS 5.0.539: the panel needs GPU support and showed only an unsupported-graphics notice in the VM |
| Trace outer edge only / trace & detach | SS | Keep only the outline, or split the image along the trace | missing | replicate | 2026-11-02, SS 5.0.539: Trace Outer Edge and Trace and Detach are actions next to Trace |
| Background remover / "complexity" upload flow | DS | A guided upload that removes the background, then cleans up with erase tools | missing | reimagine (local, deterministic; no cloud ML) | docs only (sign-in required) |
| Live preview of the trace on screen | both | Highlights the area that will be traced | missing | replicate | SS: pending. DS: docs only (sign-in required) |
| Full-colour/multi-layer trace | both | Trace each colour into its own layer | missing | replicate | SS: pending. DS: docs only (sign-in required) |

Sources: [Cricut, *How to upload images*](https://help.cricut.com/hc/en-us/articles/360009556313-How-to-upload-images-into-Design-Space).

## Weld and pathfinder

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Weld / Unite | both | Merge overlapping shapes into one outline | have: Pathfinder `union`/`merge` | replicate | 2026-11-02, SS 5.0.539: Weld in the Modify panel and the selection options bar. DS: docs only (sign-in required) |
| Subtract / Subtract All / Minus Back | both | Remove the top shape(s) from another | have: `difference`, `minus-back` | replicate | 2026-11-02, SS 5.0.539: Subtract and Subtract All in the Modify panel. DS: docs only (sign-in required) |
| Intersect | both | Keep only the overlap | have: `intersect` | replicate | 2026-11-02, SS 5.0.539: Intersect in the Modify panel. DS: docs only (sign-in required) |
| Exclude / XOR | both | Keep everything except the overlap | have: `xor` | replicate | 2026-11-02, SS 5.0.539: no exclude/XOR action in Basic's Modify panel. DS: docs only (sign-in required) |
| Divide / Slice | both | Split overlapping shapes into separate pieces | have: `divide`, `trim`, `crop`, `outline` | replicate | 2026-11-02, SS 5.0.539: Divide and Crop in the Modify panel. DS: docs only (sign-in required) |
| Detach Lines | SS | Separate a shape's lines from it | missing | replicate once its behaviour is confirmed | 2026-11-02, SS 5.0.539: a Modify panel action; its behaviour not tested yet |
| Attach | DS | Keep pieces in their relative positions on the mat without merging them | missing | reimagine (groups that keep positions when sent to the mat) | docs only (sign-in required) |
| Flatten (print layers into one printable image) | DS | Merge layers into one Print Then Cut image | missing | reimagine as part of Print & Cut | docs only (sign-in required) |
| Contour (hide parts of a shape) | DS | Hide individual sub-paths so they are not cut | missing | replicate (per-subpath enable) | docs only (sign-in required) |

## Replicate and array

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Duplicate in a direction (left/right/above/below) | SS | One-click copies next to the original | missing (only duplicate in place, `shapes.duplicate`) | replicate | 2026-11-02, SS 5.0.539: Replicate panel: duplicate left, right, above, below |
| Row/column/grid array | SS | Preset rows or columns of copies | missing | replicate | 2026-11-02, SS 5.0.539: presets only: row of 3, row of 4, column of 3, column of 4; no free count seen |
| Rotated copies (radial array) | SS | N copies rotated around a centre | missing | replicate | 2026-11-02, SS 5.0.539: Rotate Copies presets (a small set of radial counts) on a Radial tab |
| Fill page | SS | Repeat a shape until the media is full | missing | replicate (pairs with nesting) | 2026-11-02, SS 5.0.539: Fill Page in the Replicate panel |
| Mirror copy | SS | A mirrored duplicate | missing | replicate | 2026-11-02, SS 5.0.539: mirrored copy below, above, left or right |
| Nesting (auto pack shapes to save material) | SS (Business) | Pack shapes tightly onto the media | missing | replicate | 2026-11-02, SS 5.0.539: the Replicate panel may have a Nesting tab; not confirmed usable in Basic |

## Alignment

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Align left/centre/right/top/middle/bottom | both | Align the selection | have: `shapes.align` | replicate | 2026-11-02, SS 5.0.539: Transform panel, Align tab: horizontal left/centre/right, vertical top/middle/bottom. DS: docs only (sign-in required) |
| Align to page/mat | SS | Align against the media instead of the selection | have: `to_board` | replicate | 2026-11-02, SS 5.0.539: centre to page in the Align tab and the options bar |
| Distribute horizontally/vertically | both | Even spacing | have: `shapes.distribute` | replicate | 2026-11-02, SS 5.0.539: Distribute horizontally/vertically in the Align tab. DS: docs only (sign-in required) |
| Centre to page | SS | Put the selection in the middle of the media | have (align to board, both axes) | replicate | 2026-11-02, SS 5.0.539: centre to page and centre on each other in the Align tab |
| Spacing by a set value | SS | Exact gap between objects | missing | replicate | 2026-11-02, SS 5.0.539: Basic has only even distribution (Space Horizontally / Space Vertically); no spacing-by-value control |

## Layers and objects

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Layers panel: add/remove/show/hide | both | List of layers or objects with visibility toggles | have: `layer.add/remove/set-visible`, Layers & Objects panel | replicate | 2026-11-02, SS 5.0.539: no layers panel in Basic's panel column (vendor docs list layers as a Designer feature). DS: docs only (sign-in required) |
| Reorder (bring forward/send back, drag in list) | both | Change the stacking order | missing | replicate | 2026-11-02, SS 5.0.539: Object menu has an Arrange submenu; the options bar has arrange too; Arrange: bring to front, send to back, bring forward, send backward, each with a shortcut. DS: docs only (sign-in required) |
| Arrange shortcuts | SS | Keyboard shortcuts for bring to front/forward and send to back/backward | missing (no reorder command yet) | replicate with the reorder command | 2026-11-02, SS 5.0.539: Ctrl+Shift+] / Ctrl+Shift+[ for front/back, Ctrl+] / Ctrl+[ for one step |
| Lock layer | SS | Stop a layer from being edited | missing | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Rename layer/object | both | Give a layer or object a label | partial: layers have names; there is no rename command | replicate | SS: pending. DS: docs only (sign-in required) |
| Move object between layers | SS | Drag an object to another layer | missing | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Colour Sync | DS | Merge layers of similar colours so fewer mats are used | missing | reimagine (merge by operation/material) | docs only (sign-in required) |

## Fill and line styles

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Line colour and width | both | Stroke colour, used to group cut operations | partial: a shape is `cut` or `engrave` (`CutStyle`), shown in fixed production colours | reimagine: operation-first, colour as a label | 2026-11-02, SS 5.0.539: Line Style panel: a line-colour tab and thickness in pt; new shapes default to outline only, with a default line colour (preferences). DS: docs only (sign-in required) |
| Line style (dashed, perforated) | SS | Solid or dashed lines, with thickness, corner joins, end caps and stroke position | missing | replicate (perforation as a cut setting) | 2026-11-02, SS 5.0.539: solid/dash style, thickness (pt), corners miter/round/bevel, end caps butt/round/square, position centre or inside |
| Fill colour, gradient, pattern fill | both | Fill shapes for Print & Cut | missing | replicate colour/gradient fill for printing; user-supplied patterns only | 2026-11-02, SS 5.0.539: Fill panel tabs Solid/Gradient/Pattern; swatch grid with a no-fill swatch, recent colours, eyedropper, named custom palettes, transparency %, colour spectrum and a hex field. DS: docs only (sign-in required) |
| Print cut lines | SS | Print the outlines of chosen shapes too (zero-width lines print as hairlines) | missing | replicate | 2026-11-02, SS 5.0.539: a Line Style option to print lines of the selected shapes; 0 pt lines print as 0.1 pt, also in SVG/PDF exports |
| Sketch and Stipple fills | SS | Fill a shape with hatch lines or dots for pens | have (engine/MCP, no UI yet): `generate.fill`, `generate.release` | replicate (hatch fills for pen/engrave) | 2026-11-02, SS 5.0.539: Sketch fill patterns: cross-hatch, grid, lines, concentric, spiral; spacing and angle; Release Effects; Stipple on its own tab |
| Specialty tools (emboss/deboss, foil) | SS | Patterns and settings for emboss, deboss and foil tools | partial (engine/MCP): `generate.fill`/`generate.tool-lines` tool tags; machine output not wired | replicate the hatch patterns; tool support follows the machine work | 2026-11-02, SS 5.0.539: a panel with tabs per tool type; deboss/emboss toggle; the same five hatch patterns; spacing, angle, Release Effects |
| Pop-up card generator | SS | Generate the struts for a pop-up card from a design | partial (engine/MCP): `generate.popup`, a standalone card with struts (not fitted to a design) | reimagine later, low priority | 2026-11-02, SS 5.0.539: on/off with base width %, strut width and dash pitch |
| Vendor pattern libraries | both | Store pattern artwork | out of scope | won't do: licensed assets | 2026-11-02, SS 5.0.539: a Pattern tab exists in the Fill panel; where its patterns come from was not checked. DS: docs only (sign-in required) |
| Operation type (cut, draw/pen, score, deboss, engrave, wave, perforate, foil, print then cut) | DS | Each layer says what the machine does with it | partial: cut and engrave only | replicate the common ones (cut, pen, score, perforate) | docs only (sign-in required) |

## Print & Cut and registration marks

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Registration marks on the printed page | both | Printed marks the cutter's sensor uses to find the artwork | partial: `print-cut` writes an SVG page with three marks at a fixed 12 mm margin | replicate | 2026-11-02, SS 5.0.539: Print & Cut panel (View › Registration Marks) turns marks on; marks are off by default (a preference). DS: docs only (sign-in required) |
| Show print border | SS | Draw the printer's printable area on the page | missing | replicate | 2026-11-02, SS 5.0.539: View › Show Print Border (Ctrl+Shift+P), off by default |
| Mark style, length, inset choices | SS | Change the size and position of the marks | missing | replicate | 2026-11-02, SS 5.0.539: length 0.787 in, thickness 0.020 in, inset 0.625 in by default; More Settings; Restore Defaults |
| Send to printer from the app | both | System print dialog, with bleed and actual-size checks | missing: StencilCraft does not drive a printer | replicate | 2026-11-02, SS 5.0.539: File › Print and Print Page Setup; a Print button in the Print & Cut panel; print resolution and print preview are preferences. DS: docs only (sign-in required) |
| Bleed | both | Extend the print colour past the cut line so a slightly-off cut shows no white edge | missing | replicate | 2026-11-02, SS 5.0.539: Enable Print Bleed with a bleed radius (0.050 in default) in the Print & Cut panel. DS: docs only (sign-in required) |
| Optical sensor registration (automatic alignment) | both | The machine scans the marks before it cuts | missing (`TB5x` commands not driven) | replicate from protocol facts | SS: pending. DS: docs only (sign-in required) |
| Print & Cut calibration | both | Adjust for sensor/printer offset with a test sheet | missing | replicate | SS: pending. DS: docs only (sign-in required) |
| Print size limits | DS | Maximum printable area per mat/page | missing | replicate (validation) | docs only (sign-in required) |
| PixScan / scan-to-cut | SS | Photograph the material on a special mat and cut to it | missing | reimagine later, low priority | 2026-11-02, SS 5.0.539: the PixScan panel opens a camera/scan calibration dialog; File › Scan is also present |

Sources: [Cricut, *How to Print Then Cut*](https://help.cricut.com/hc/en-us/articles/360009387274-How-to-Print-Then-Cut-in-Design-Space), [Cricut, *How large can I Print Then Cut?*](https://help.cricut.com/hc/en-us/articles/360009429814-Design-Space-says-printable-image-is-too-large), [Silhouette, *Print and Cut troubleshooting*](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000276219-print-and-cut-troubleshooting). See also [`print-cut.md`](print-cut.md).

## Cut settings, materials and tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Material presets | both | Choose the material; force, speed and passes fill in from the preset | have: `MEDIA_PRESETS` (27 entries) with a Send-panel chooser | replicate with our own numbers; never import their preset data files | 2026-11-02, SS 5.0.539: Standard sub-tab: material, action and tool dropdowns per tool holder; a long built-in material list; a notice when the material has no action for that holder. DS: docs only (sign-in required) |
| Force, speed, passes override | both | Change the preset values for this job | partial: force and speed are overridable; passes are not exposed for the cutter (G-code has `--passes`) | replicate | 2026-11-02, SS 5.0.539: Material Settings: force, speed, passes, acceleration and a blade-depth dial, each a slider plus a number. DS: docs only (sign-in required) |
| Advanced material settings | SS | Per-material settings beyond force and speed: tracking, kraft-blade loop corners, smart cut | missing | replicate the documented behaviours from our own testing | 2026-11-02, SS 5.0.539: Track Enhancing checkbox; the kraft blade adds loop force, loop blade height, loop overcut, loop radius and a smart-cut toggle |
| Blade depth / tool type | SS | Pick the tool (ratchet, auto blade, kraft, rotary, pen) and blade depth | partial: `ToolKind` parsed from the device; no chooser in the UI | replicate | 2026-11-02, SS 5.0.539: the tool is detected from the holder (it shows awaiting connection without a machine); blade-depth dial for ratchet/auto blades; the kraft blade has its own sub-tab; blade type defaults to automatic |
| Custom user materials | both | Save your own material presets | missing | replicate | 2026-11-02, SS 5.0.539: +/− buttons in the material list, Create New…, and Revert / Save As… in Material Settings. DS: docs only (sign-in required) |
| Per-layer/per-colour settings | SS | Different settings per line colour or layer | missing | replicate | 2026-11-02, SS 5.0.539: Cut by Line Color / Cut by Fill Color tables, one row per colour (own row: Send sub-modes) |
| Test cut | both | Cut a small test shape first | missing | replicate | SS: pending. DS: docs only (sign-in required) |
| Overcut / line segment overcut | SS | Cut a little past corners or ends so pieces lift cleanly | missing | replicate | 2026-11-02, SS 5.0.539: a Line Segment Overcut checkbox per material; kraft blade adds loop overcut; Preferences › Advanced has Software Overcut (on) |
| Cut order / path sorting | SS | Cut inner shapes first and keep travel short | missing | replicate | 2026-11-02, SS 5.0.539: an Intelligent Path checkbox per material; its behaviour not tested yet |
| Mat-free (carrier sheet free) cutting strategy | SS | Path ordering that lets material be cut without a mat | missing | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Weed lines and weed borders | SS (Business) | Extra cut lines that make weeding easier | have (engine/MCP, no UI yet): `generate.weed` | replicate | 2026-11-02, SS 5.0.539: not present in Basic |
| Laser/G-code profiles | — | Not in these apps; StencilCraft's own | partial | — | — |

## Send panel and machine connection

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Design → Send/Make split | both | A separate production screen before cutting | have: Design/Send split with a dry-run program summary | replicate | 2026-11-02, SS 5.0.539: the SEND tab turns the canvas into a cut preview with settings on the right, in Guide / Standard / By Line Color / By Fill Color sub-tabs plus a gear for send preferences. DS: docs only (sign-in required) |
| Send sub-modes: Standard / by line colour / by fill colour | SS | Settings for the whole job, or one row of settings per line or fill colour | missing | reimagine: per-layer/operation settings | 2026-11-02, SS 5.0.539: By Line Color table: tool number, on/off, outline-only, colour, material, action, tool; By Fill Color is the same keyed by fill |
| Assign lines to tool holders on the cut preview | SS | Send the selected lines (or only their outline) to tool 1 or tool 2, or skip them | missing | replicate (dual-holder machines) | 2026-11-02, SS 5.0.539: overlay buttons: tool 1 all lines, tool 1 outline, tool 2 all, tool 2 outline, ignore selected lines |
| Mat preview on the Make screen | DS | Shows how pieces sit on each mat; you can move them there | missing | reimagine | docs only (sign-in required) |
| Machine selection | both | Choose the cutter model | have: model chooser (`Model`, Cameo 3–5, Portrait, Pro) | replicate | 2026-11-02, SS 5.0.539: machine card with a model dropdown (CAMEO 5 by default) and an illustration (own row: machine card). DS: docs only (sign-in required) |
| Machine card (model, firmware, status) | SS | A card showing the chosen machine, its firmware and its state | partial: model chooser; firmware version query in the CLI send path only | replicate | 2026-11-02, SS 5.0.539: model dropdown, firmware (unknown without a machine), status (none), an illustration |
| USB connection | both | Send the job over USB | partial: serial (CDC/ACM) transport via CLI `--device`; no USB-bulk transport; the app does not send | replicate (`nusb`) | 2026-11-02, SS 5.0.539: Guide sub-tab lists detected USB and Bluetooth devices with Connect; no machine was attached in the VM. DS: docs only (sign-in required) |
| Bluetooth connection | both | Wireless send | missing | replicate | 2026-11-02, SS 5.0.539: Bluetooth devices share the same list; a banner said Bluetooth was unavailable in the VM. DS: docs only (sign-in required) |
| Device discovery and status | both | Find connected cutters; show ready/loaded/tool status | partial: status query decoded; no discovery or live status in the UI | replicate | 2026-11-02, SS 5.0.539: Rescan for Devices button; with no machine the card shows unknown firmware and no status; a preference releases the device automatically when not in use (off by default). DS: docs only (sign-in required) |
| Packet size | SS | How much data is sent to the machine per write | missing (CLI streams the whole program) | replicate if real hardware needs it | 2026-11-02, SS 5.0.539: Preferences › Advanced: packet size Auto (1000) |
| Load/unload media, pause, cancel | both | Machine controls during a job | missing | replicate | SS: pending. DS: docs only (sign-in required) |
| Multi-cutter support | SS (Business) | Send jobs to several machines at once | missing | replicate later | 2026-11-02, SS 5.0.539: not present in Basic |
| Batch / queued jobs | SS (Business) | Queue several designs or repeats as one job | missing | replicate | 2026-11-02, SS 5.0.539: not present in Basic |
| Cricut machine protocol | DS | Driving Cricut machines | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | docs only (sign-in required) |
| Firmware updates | both | Update the machine firmware from the app | out of scope | won't do: vendor firmware | SS: pending. DS: docs only (sign-in required) |

## Library and store

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Local design library | SS | Organise designs on disk in folders | missing (a Recent menu only) | reimagine: plain folders of `.stencilcraft` files | 2026-11-02, SS 5.0.539: LIBRARY tab; a folder button on the tool rail opens a library sidebar; Save to Library and Update Library in the File menu; the default save location (disk or library) is a preference |
| Design store / Cricut Access images and projects | both | Buy or subscribe to designs | out of scope | won't do: proprietary cloud store and licensed assets | 2026-11-02, SS 5.0.539: STORE tab present; not explored. DS: docs only (sign-in required) |
| Ready-to-make projects | DS | Project templates from the vendor | out of scope | won't do: licensed assets; we may ship original example files | docs only (sign-in required) |
| Templates (mockups like shirts and mugs) | DS | Background images to size designs against | missing | reimagine with original, code-generated templates | docs only (sign-in required) |

## File import and export formats

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| SVG import | SS (Designer+), DS | Open vector artwork | have: a working subset (see [`gaps.md`](gaps.md) 1) | replicate | 2026-11-02, SS 5.0.539: Basic's Open dialog has no SVG. DS: docs only (sign-in required) |
| Merge (import into current document) | SS | Open a file's contents into the document already open | have: Import SVG/Image add to the open document | replicate | 2026-11-02, SS 5.0.539: File › Merge (Ctrl+Shift+O) |
| Open Recovered (autosave recovery) | SS | Reopen work saved automatically before a crash | missing: no autosave | replicate | 2026-11-02, SS 5.0.539: File › Open Recovered… |
| DXF import | both | Open CAD outlines | missing (DXF export only) | replicate | 2026-11-02, SS 5.0.539: DXF is in Basic's Open types; import options: fit to page, convert white lines to black, join lines into polylines. DS: docs only (sign-in required) |
| PDF import | SS (Designer+) | Open vector PDFs | missing | replicate | 2026-11-02, SS 5.0.539: Basic's Open dialog has no PDF |
| AI/EPS/CDR import | SS (Business) | Open Illustrator/CorelDRAW files | missing | replicate AI (its PDF-compatible content) and EPS (a PostScript subset); CDR only if its structure can be learned clean-room | 2026-11-02, SS 5.0.539: not present in Basic |
| Raster import (PNG/JPEG/BMP/GIF/TIFF) | both | Place a photo or clip art to trace or print | partial: PNG/JPEG for tracing only; no placed raster for printing | replicate | 2026-11-02, SS 5.0.539: PNG, JPEG, BMP, GIF and TIFF in Basic's Open types; PNG autotrace on import is on by default. DS: docs only (sign-in required) |
| Embroidery formats (PES, DST, …) | SS (Designer Plus) | Import embroidery outlines | missing | replicate (outline only) | 2026-11-02, SS 5.0.539: not present in Basic |
| Native `.studio`/`.studio3` | SS | The vendor's own formats: Basic saves `.studio3`, `.studio` and GSP and opens those plus GSD | missing | reimagine: import needs a public spec or a clean-room study of files users own (owner decision); until then users export DXF | 2026-11-02, SS 5.0.539: Basic's save types: Studio V3 (.studio3), V2 (.studio), GSP; GSD open options: as-is, convert white lines to black, join lines into polylines |
| SVG export | SS (Business) | Save as SVG | have | replicate | 2026-11-02, SS 5.0.539: Basic has no SVG export (and no DXF, PDF or PNG) |
| PDF/PNG/JPG export | SS (Business) | Save as PDF or image | missing | replicate (PDF first) | 2026-11-02, SS 5.0.539: not present in Basic |
| DXF export | — | StencilCraft's own | have (R14, flattened) | — | — |
| G-code export | — | StencilCraft's own | have | — | — |
| Plugins for external design tools | SS (Silhouette Connect) | Send straight from Illustrator/CorelDRAW to the cutter | missing | reimagine: CLI/MCP and SVG/PDF interchange; never tie the core to proprietary software | pending |
| Cloud-only save | DS | Projects saved to the vendor cloud | out of scope | won't do: account system; we save open, local files | docs only (sign-in required) |

Sources: [Silhouette, *File Types and Features*](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000282776-file-types-and-features), [Cricut, *Image uploads: unsupported items*](https://help.cricut.com/hc/en-us/articles/360009553213-Image-uploads-unsupported-items).

## Preferences

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Theme / appearance | both | Light or dark interface | have: suite light/dark themes, saved between sessions | replicate | 2026-11-02, SS 5.0.539: Display tab: theme (Light by default, with a choice), background colour, button size, animation speed, antialiasing and curve quality (the last two need a restart). DS: docs only (sign-in required) |
| Print resolution and preview | SS | The dpi used for printing and whether a preview is shown first | missing | replicate with printing | 2026-11-02, SS 5.0.539: General tab: print resolution 300/600 (default)/1200/custom dpi; print preview on |
| Reset and maintenance options | SS | Restore defaults, show hidden dialogs, graphics settings | partial: Reset Workspace | replicate | 2026-11-02, SS 5.0.539: Advanced tab: restore factory defaults, restore pre-loaded designs, unhide dialogs, library permissions, OpenGL settings |
| Usage data collection | SS | Opt-in performance data sent to the vendor | out of scope: StencilCraft sends nothing | won't do (owner decision if this ever changes) | 2026-11-02, SS 5.0.539: an opt-in setting, off by default |
| Units and grid spacing | both | Default units; grid size | partial: grid toggle only; mm fixed | replicate | 2026-11-02, SS 5.0.539: General tab: unit of measurement, Auto (Inches); Show Dimensions is on; grid spacing in the Grid Options dialog. DS: docs only (sign-in required) |
| Default material/machine | both | Settings that start each document and job: orientation, blade, page colour, fill style, text style | partial: the Send panel defaults are fixed in code | replicate | 2026-11-02, SS 5.0.539: Defaults tab: page orientation and blade type are automatic; default page colour; default fill style outline-only with default line/fill colours; default text style; always show the mat; cut to page edge; registration marks off. DS: docs only (sign-in required) |
| Import defaults (DPI, scale) | SS | How imported files are sized | partial: trace uses 96 dpi by default | replicate | 2026-11-02, SS 5.0.539: Import tab: options for GSD and DXF (white lines to black, join into polylines, DXF fit to page) and PNG autotrace on import |
| Workspace layout | SS | Panel arrangement | partial: dock layout and Reset Workspace | replicate | 2026-11-02, SS 5.0.539: panels open from a column of icons on the right as floating, collapsible, closable docked panels; scrollbar side is a preference; Starter mode hides most of them (own row) |
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
| `.studio3` (Studio V3) | SS Basic+ | open, save (observed 5.0.539) | missing | reimagine: needs a public spec or a clean-room study of files users own (owner decision); until then users export DXF/SVG |
| `.studio` (Studio V2) | SS Basic+ | open, save (observed 5.0.539) | missing | reimagine: needs a public spec or a clean-room study of files users own (owner decision); until then users export DXF/SVG |
| GSP | SS Basic+ | save (observed 5.0.539) | missing | reimagine: needs a public spec or a clean-room study of files users own (owner decision); until then users export DXF/SVG |
| GSD (older Graphtec Studio) | SS Basic+ | open (observed 5.0.539) | missing | reimagine: needs a public spec or a clean-room study of files users own (owner decision); until then users export DXF/SVG |
| DXF | SS Basic+ | open (observed 5.0.539) | import missing; export have (R14, flattened) | replicate |
| DXF | DS desktop | upload (not on iOS/Android) | import missing | replicate |
| SVG | SS Designer+ | open | import partial (subset) | replicate |
| SVG | SS Business | export | have | replicate |
| SVG | DS | upload | import partial (subset) | replicate |
| PDF | SS Designer+ | open | missing | replicate |
| PDF | SS Business | export | missing | replicate |
| PDF | DS | not accepted for upload (help centre) | missing | replicate (for SS parity) |
| AI (Illustrator) | SS Business | open | missing | replicate via its PDF-compatible content |
| EPS | SS Business | open | missing | replicate (PostScript subset) |
| CDR (CorelDRAW) | SS Business | open | missing | replicate only if its structure can be learned clean-room |
| PNG | SS Basic+ | open (observed 5.0.539; autotrace on import) | import partial (trace only) | replicate (also placed raster for printing) |
| PNG | SS Business | export | missing | replicate |
| PNG | DS | upload | import partial (trace only) | replicate |
| JPEG | SS Basic+ | open (observed 5.0.539) | import partial (trace only) | replicate |
| JPEG | SS Business | export | missing | replicate |
| JPEG | DS | upload | import partial (trace only) | replicate |
| BMP | SS Basic+, DS | open / upload | missing | replicate |
| GIF | SS Basic+, DS | open / upload | missing | replicate |
| TIFF | SS Basic+ | open (observed 5.0.539) | missing | replicate |
| TIFF | DS | not accepted for upload (help centre) | missing | replicate (for SS parity) |
| HEIC | DS | upload (listed in one help article) | missing | replicate |
| WebP | DS | not documented in the help articles we found | missing | verify, then replicate if supported |
| Embroidery: PES, DST, EXP, JEF, XXX | SS Designer Plus | open | missing | replicate (outline only) |
| Fonts: TTF/OTF (system fonts) | SS, DS | used for text | missing | replicate through system/user fonts; none bundled |
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
| PDF import | Designer | Open the vector content of PDF files | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Layers panel | Designer | Create, rename, reorder, hide and lock layers; move objects between layers | partial: `layer.add/remove/set-visible` only | replicate | not present in Basic; from vendor docs (not observed) |
| Cut by layer | Designer (to confirm) | Send settings and cut order chosen per layer | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Rhinestone tool | Designer | Place stones along a line, around an outline or as a fill; stone size, spacing and shape | have (engine/MCP, no UI yet): `generate.rhinestone` | replicate (stone templates are just circles we generate) | not present in Basic; from vendor docs (not observed) |
| Sketch effect | Designer (a Sketch panel was also visible in Basic 5.0.539) | Turn fills into hatch lines a pen can draw | have (engine/MCP, no UI yet): `generate.fill` | replicate | vendor docs list it as Designer; a Sketch panel was visible in Basic 5.0.539 (2026-11-02) |
| Custom gradient and pattern fills | Designer | Edit gradients, scale and rotate patterns, add your own pattern images (Basic has solid fill and transparency) | missing | replicate with user-supplied patterns only | not present in Basic; from vendor docs (not observed) |
| Eraser and knife options | Designer (to confirm) | Eraser size and shape, knife in straight, curved or polyline mode | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Eyedropper tool | Designer (to confirm) | Copy fill and line style from one shape to another | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Rulers and measure tool | Designer | Measure distances and angles on the canvas | partial: rulers only | replicate | not present in Basic; from vendor docs (not observed) |
| Shear and distort | Designer | Slant a shape or drag its corners freely | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Lattice | Designer (to confirm) | Turn a shape into a lattice of cut-out holes | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Font creator | Designer (to confirm) | Draw your own font, one glyph at a time, including single-line fonts | missing | replicate (writes a user font file; nothing bundled) | not present in Basic; from vendor docs (not observed) |
| Glyphs panel | Designer | Insert special characters of the current font | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Weld on cut | paid (edition to confirm) | Merge overlapping shapes automatically when sending, so overlaps are not cut twice | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Roll feed / mat-free cutting | paid (edition to confirm) | Cut long jobs straight from a roll without a mat, with path ordering that keeps the material steady | partial: model tables carry roll lengths; no roll-feed choice or mat-free ordering | replicate | not present in Basic; from vendor docs (not observed) |
| Cross-stitch patterns | Designer (to confirm) | Turn a design into a cross-stitch pattern for pens | missing | replicate, low priority | not present in Basic; from vendor docs (not observed) |
| Embroidery import (PES, DST, EXP, JEF, XXX) | Designer Plus | Open embroidery files as outlines | missing | replicate (outline only) | not present in Basic; from vendor docs (not observed) |
| Advanced warp (conical, perspective) | Designer Plus | Warp onto a cone (cups, tumblers) or into perspective | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Flexishapes | Designer Plus | Parametric shapes with handles you can drag | have (engine/MCP, no UI yet): `generate.shape` with editable params (`replace`) | replicate | not present in Basic; from vendor docs (not observed) |
| Puzzle generator | Designer Plus | Cut a design or photo into jigsaw pieces | have (engine/MCP, no UI yet): `generate.puzzle` (over a shape, not a photo) | replicate, low priority | not present in Basic; from vendor docs (not observed) |
| SVG export | Business | Save as SVG | have | replicate | not present in Basic; from vendor docs (not observed) |
| PDF export | Business | Save as PDF | missing | replicate | not present in Basic; from vendor docs (not observed) |
| PNG/JPG export | Business | Save as an image | missing | replicate | not present in Basic; from vendor docs (not observed) |
| AI, EPS and CDR import | Business | Open Illustrator, EPS and CorelDRAW files | missing | replicate AI (its PDF-compatible content) and EPS (a PostScript subset); CDR only if its structure can be learned clean-room | not present in Basic; from vendor docs (not observed) |
| Multi-cutter support | Business | Drive several machines from one app, each with its own job | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Nesting | Business | Pack shapes tightly onto the media to save material | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Tiling | Business | Split a design larger than the media into tiles | have: `tiling` crate | replicate | not present in Basic; from vendor docs (not observed) |
| Weed lines and borders | Business | Add cut lines around and between shapes so waste lifts in pieces | have (engine/MCP, no UI yet): `generate.weed` | replicate | not present in Basic; from vendor docs (not observed) |
| Job queue / batch | Business (to confirm) | Queue several designs or copies as one production run | missing | replicate | not present in Basic; from vendor docs (not observed) |

## Cricut Design Space, full inventory

Every Design Space feature found in Cricut's public documentation. The details, options,
platform differences and sources for each are in the
[Cricut Design Space feature library](cricut-design-space/README.md); each row links to
its entry. Design Space needs sign-in, so every row is "docs only". Status is checked
against the engine command set on main (2026-11-02).

| Feature | Area | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Home screen | Home | Browse and search projects, images, fonts ([library](cricut-design-space/home-and-projects.md#home-screen)) | missing | reimagine: new/recent local files; vendor content won't do | docs only (sign-in required) |
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
| Monogram Maker | Design panel | Monogram from up to 4 initials, style and frame ([library](cricut-design-space/design-panel.md#monogram-maker)) | missing | replicate with user fonts and our own frames | docs only (sign-in required) |
| Editable images | Design panel | Library images with editable text ([library](cricut-design-space/design-panel.md#editable-images)) | out of scope (vendor images) | reimagine: templates with editable text placeholders | docs only (sign-in required) |
| Phrases | Design panel | Unconfirmed in public docs ([library](cricut-design-space/design-panel.md#phrases)) | n/a | verify | docs only (sign-in required) |
| AI Create / AI Edit / AI Styles | Design panel | Generate or restyle images with credits ([library](cricut-design-space/design-panel.md#ai-create-and-ai-edit)) | out of scope | won't do: account-bound cloud service with paid credits | docs only (sign-in required) |
| AI Generated image filter | Design panel | Filter for AI-made images ([library](cricut-design-space/design-panel.md#ai-generated-image-filter)) | out of scope | won't do: account-bound cloud service | docs only (sign-in required) |
| Add text | Text | Text box on the Canvas ([library](cricut-design-space/text.md#add-text)) | partial: built-in single-line lettering | replicate | docs only (sign-in required) |
| Font picker with filters | Text | System/Cricut tabs, search, keywords, kerned filter ([library](cricut-design-space/text.md#font-picker)) | missing | replicate for system/user fonts; vendor fonts won't do (store) | docs only (sign-in required) |
| Font style incl. writing | Text | Regular, bold, italic, single-line writing ([library](cricut-design-space/text.md#font-style)) | partial: single-line lettering | replicate | docs only (sign-in required) |
| Font size | Text | Point size ([library](cricut-design-space/text.md#font-size)) | missing | replicate | docs only (sign-in required) |
| Letter space | Text | Spacing between letters ([library](cricut-design-space/text.md#letter-space)) | missing | replicate | docs only (sign-in required) |
| Line space | Text | Spacing between lines ([library](cricut-design-space/text.md#line-space)) | missing | replicate | docs only (sign-in required) |
| Text alignment | Text | Left, centre, right ([library](cricut-design-space/text.md#text-alignment)) | missing | replicate | docs only (sign-in required) |
| Curve | Text | Bend one line into an arc by diameter ([library](cricut-design-space/text.md#curve)) | missing | replicate | docs only (sign-in required) |
| Ungroup to lines / letters | Text | Split text into lines, then letters ([library](cricut-design-space/text.md#ungroup-to-lines--ungroup-to-letters)) | missing | replicate | docs only (sign-in required) |
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
| Cut, copy, paste | Edit bar | Clipboard ([library](cricut-design-space/edit-bar.md#edit-menu-cut-copy-paste)) | partial: duplicate only | replicate | docs only (sign-in required) |
| Align and distribute | Edit bar | Align 2+ objects; distribute evenly ([library](cricut-design-space/edit-bar.md#align)) | have: `shapes.align`, `shapes.distribute` | replicate | docs only (sign-in required) |
| Arrange | Edit bar | To back, backward, forward, to front ([library](cricut-design-space/edit-bar.md#arrange)) | missing | replicate | docs only (sign-in required) |
| Flip | Edit bar | Flip horizontal/vertical ([library](cricut-design-space/edit-bar.md#flip--mirror)) | partial: `shapes.transform` negative scale; `shapes.mirror-copy` makes a copy | replicate | docs only (sign-in required) |
| Size | Edit bar | Exact W/H, 0.1 steps, proportion lock ([library](cricut-design-space/edit-bar.md#size)) | have: size fields with aspect link | replicate | docs only (sign-in required) |
| Rotate | Edit bar | Exact angle, 1° steps ([library](cricut-design-space/edit-bar.md#rotate)) | have: `shape.rotate`, `shapes.transform` | replicate | docs only (sign-in required) |
| Position | Edit bar | Exact X/Y ([library](cricut-design-space/edit-bar.md#position)) | have: `shape.set-bounds` | replicate | docs only (sign-in required) |
| Offset | Edit bar | Outline at a distance; round/sharp corners; weld offsets ([library](cricut-design-space/edit-bar.md#offset)) | partial: engine `shapes.offset` (join style, miter limit); no UI; no weld-offsets option | replicate | docs only (sign-in required) |
| Warp (30 styles) | Edit bar | Preset warps with a strength slider (Access) ([library](cricut-design-space/edit-bar.md#warp)) | partial (engine/MCP): `generate.warp`, 15 styles with bend and distortion | replicate | docs only (sign-in required) |
| Create Sticker (die/kiss cut, border) | Edit bar | One-step sticker: flatten, border, die or kiss cut (Access) ([library](cricut-design-space/edit-bar.md#create-sticker)) | missing | replicate (offset + flatten + cut depth) | docs only (sign-in required) |
| Group / ungroup | Layers panel | Move and size together ([library](cricut-design-space/layers-panel.md#group--ungroup)) | missing | replicate | docs only (sign-in required) |
| Duplicate | Layers panel | Copy the selection ([library](cricut-design-space/layers-panel.md#duplicate)) | have: `shapes.duplicate` | replicate | docs only (sign-in required) |
| Delete | Layers panel | Remove the selection ([library](cricut-design-space/layers-panel.md#delete)) | have: `shapes.remove` | replicate | docs only (sign-in required) |
| Hide layer / object | Layers panel | Hidden items are not cut or printed ([library](cricut-design-space/layers-panel.md#hide--show-layer)) | have: `layer.set-visible`, `shapes.set-hidden` | replicate | docs only (sign-in required) |
| Rename and reorder layers | Layers panel | Rename; drag to reorder ([library](cricut-design-space/layers-panel.md#rename-and-reorder-layers)) | missing | replicate | docs only (sign-in required) |
| Slice | Layers panel | Split two overlapping layers ([library](cricut-design-space/layers-panel.md#slice)) | have: Pathfinder `divide` | replicate | docs only (sign-in required) |
| Weld | Layers panel | Merge permanently into one outline ([library](cricut-design-space/layers-panel.md#weld)) | have: Pathfinder `union` | replicate | docs only (sign-in required) |
| Combine (non-destructive Unite/Subtract/Intersect/Exclude) | Layers panel | Shape operations that stay editable ([library](cricut-design-space/layers-panel.md#combine-unite-subtract-intersect-exclude)) | partial: same operations exist but are destructive (undo only) | replicate (live boolean groups) | docs only (sign-in required) |
| Attach / detach | Layers panel | Keep positions on the mat; fasten draw/score to cut ([library](cricut-design-space/layers-panel.md#attach--detach)) | missing | replicate | docs only (sign-in required) |
| Flatten / unflatten | Layers panel | Merge into one printable layer ([library](cricut-design-space/layers-panel.md#flatten--unflatten)) | missing | replicate | docs only (sign-in required) |
| Contour | Layers panel | Hide individual cut lines ([library](cricut-design-space/layers-panel.md#contour)) | missing | replicate | docs only (sign-in required) |
| Color Sync | Layers panel | Merge colours to use fewer mats ([library](cricut-design-space/layers-panel.md#color-sync-panel)) | missing | replicate | docs only (sign-in required) |
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

## Missing items, by area

These are the rows marked **missing** above. Rows marked partial are not listed. The ranked order is in [`gaps.md`](gaps.md).

- **Canvas**: Home screen, Starter mode, document tabs, auto mat layout, snap to grid,
  smart snapping, live dimension callouts, measure.
- **Drawing**: freehand, arc, knife, eraser, parametric shapes, rhinestone/cross-stitch/puzzle.
- **Text**: typography controls, text on path / curve text, glyphs panel, spelling check.
- **Editing**: group/ungroup, break/join/close path, simplify, warp, lock, rotation knob,
  Paste in Front, Select by Color, action-after-tool-use preference, node count readout.
- **Offset**: weld offsets / per-letter offsets.
- **Trace**: image filters, outer-edge-only trace and trace & detach, background removal,
  live preview, multi-colour trace, image effects.
- **Pathfinder**: Attach, Flatten, Contour, Detach Lines.
- **Replicate**: directional duplicate, row/column presets, radial copies, fill page,
  mirror copy, nesting.
- **Alignment**: spacing by a set value.
- **Layers**: reorder and arrange shortcuts, lock, move between layers, Colour Sync.
- **Styles**: dashed lines and joins/caps, fill/gradient/pattern, print cut lines,
  sketch and stipple fills, specialty (emboss/deboss/foil) tools, pop-up card generator.
- **Print & Cut**: mark options, show print border, printing from the app, bleed, sensor
  registration, calibration, print size limits, scan-to-cut.
- **Cut settings**: custom materials, per-layer settings, advanced material settings,
  test cut, overcut, cut order, mat-free strategy, weed lines.
- **Machine**: Send sub-modes by colour, tool-holder line assignment, mat preview,
  Bluetooth transport, load/unload/pause/cancel controls, packet size, multi-cutter, batch
  jobs.
- **Formats**: DXF import, PDF import, PDF/PNG/JPG export, external design-tool plugins,
  autosave recovery.
- **Preferences**: language, print resolution and preview.
- **Platforms**: browser/mobile client.
- **SS paid editions**: PDF import/export, PNG/JPG export, layers panel, cut by layer,
  rhinestone, sketch effect, custom gradient/pattern fills, eraser/knife options,
  eyedropper, measure, shear/distort, lattice, font creator, glyphs, weld on cut, roll
  feed/mat-free, cross-stitch, embroidery import, advanced warp, Flexishapes, puzzle
  generator, AI/EPS/CDR import, multi-cutter, nesting, weed lines, job queue.
- **DS full inventory**: Home, templates, Smart Guides, history panel, deboss/wave/
  perforate/foil operations, arrange, font picker, size, letter/line space, alignment,
  curve, ungroup to letters, print fill, patterns, warp, group, attach, flatten, contour,
  Colour Sync, layer reorder, mat preview and arrangement, mirror per mat, project copies,
  tool prompts, go/pause/unload, Print Then Cut size limit and flow, calibration, remove
  background, erase/restore cleanup, uploaded-images library, project collections.
- **Formats**: `.studio3`/`.studio`/GSP/GSD (owner decision), DXF, PDF, AI, EPS, CDR, BMP,
  GIF, TIFF, HEIC and WebP import, PDF/PNG/JPEG export, embroidery import, system fonts,
  pattern images.
- **Cutters**: Silhouette SD, CAMEO 1/2, CAMEO 5 Alpha, Portrait 1/2, Curio 1/2, Craft
  ROBO CC200/CC300, Graphtec CE/CB; USB-bulk and Bluetooth transports for every model.
  All Cricut machines are missing and pending the owner's decision.
