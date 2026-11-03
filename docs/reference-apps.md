# Reference apps: Silhouette Studio and Cricut Design Space

This is the feature inventory of the two consumer cutting apps StencilCraft is measured
against. For each feature it gives our own description, StencilCraft's status and how we
plan to handle it. It replaces the old "Silhouette Studio feature TODO" list in
[`ROADMAP.md`](../ROADMAP.md). The ranked work list stays in [`gaps.md`](gaps.md).

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
design screen opens. We do not bypass sign-in, so every DS observation reads
`blocked: sign-in required` until the owner decides whether to sign in with an account.
Cricut rows are still based on Cricut's public help centre.

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
| Cutting mat selection and backdrop | both | Choose the mat (12×12, 12×24, …); the canvas shows it behind the design | have: `Mat` table in `crates/machine`, mat backdrop in the app | replicate | 2026-11-02, SS 5.0.539: a grey mat is drawn around the white page, with a feed-direction arrow at the top; the mat is picked in Page Setup; a preference keeps the mat always on screen. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Document tabs | SS | Several open documents as tabs, with a button for a new tab | missing: one document per window, one board | replicate | 2026-11-02, SS 5.0.539: tabs above the canvas (one per document) with a + button; Close Tab is in the File menu; only one page per tab was seen |
| Infinite canvas with auto mat layout | DS | Design on a free canvas; the Make screen lays shapes out onto mats per colour/material | missing | reimagine: bridge to nesting and tiling | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Orientation and rotated view | SS | Portrait/landscape feed and a rotated view of the mat | partial: the encoder can emit the feed direction (`TB50`), but the UI does not expose it | replicate | 2026-11-02, SS 5.0.539: Page Setup has portrait/landscape and a rotated view (0/90/180/270); View menu has Rotate Page; page orientation defaults to automatic in Preferences |
| Cut border / safe area | SS | A border around the media that the blade will not enter | partial: plot margins exist in `PlotOptions`; nothing is drawn on the canvas | replicate | 2026-11-02, SS 5.0.539: Show Cut Border (on by default) outlines the area the blade can reach; a preference allows cutting to the edge of the page |
| Rulers, grid, guides | both | Rulers on the canvas edges, a background grid, draggable guides | partial: rulers (with zero reset) and grid toggle exist; no guides | replicate | 2026-11-02, SS 5.0.539: View menu has Show Grid and a Grid Options dialog; Page Setup has a Grid tab; rulers and guides not checked yet. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Snap to grid | both | Objects snap to grid lines | missing | replicate | 2026-11-02, SS 5.0.539: Snap to Grid in the View menu; object snapping is a separate row (Smart Snapping). DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Smart snapping (to objects) | SS | Snap to other objects' edges and centres while moving | missing | replicate | 2026-11-02, SS 5.0.539: View › Enable Smart Snapping, on by default |
| Zoom, pan, fit | both | Zoom tools, hand pan, fit to page/selection | have: Hand, Zoom, Fit (F), Center (C), status-bar zoom | replicate | 2026-11-02, SS 5.0.539: full toolbar has zoom in/out, zoom to fit, zoom to selection and a pan hand; View menu has a Zoom submenu. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Live dimension callouts while drawing | SS | Width and height labels follow the shape as you drag, and stay on the selection | missing | replicate | 2026-11-02, SS 5.0.539: width shown below and height on the left, in inches; they update during an offset preview; Show Dimensions is a preference (on) |
| Measure | SS | Measure a distance on the canvas | missing | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Units | both | Choose inches or millimetres; the default is inches | partial: everything is millimetres; no inch display option | replicate (display-only conversion at the UI boundary) | 2026-11-02, SS 5.0.539: default units are inches (preference set to Auto (Inches)); page size, dimension callouts and offset distances all show inches. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Kerf/compensation preview | — | Not a reference-app feature; StencilCraft's own | have | — | — |
| Oversized design tiling | SS (Business) | Split a design larger than the media into tiles that are cut separately | have: `tiling` crate, `tile-preview`, overlap and marks | replicate | 2026-11-02, SS 5.0.539: not present in Basic; needs a paid edition to observe |

## Drawing tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Rectangle, rounded rectangle | both | Drag to draw; the corner radius can be changed | have: `shape.add-rect`, `shape.add-rounded-rect` | replicate | 2026-11-02, SS 5.0.539: Shape flyout has rectangle and rounded rectangle; width and height callouts follow the drag. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Ellipse/circle | both | Drag to draw; hold a modifier for a circle | have: `shape.add-ellipse` | replicate | 2026-11-02, SS 5.0.539: ellipse in the Shape flyout. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Polygon, star | both | Regular polygon or star; the number of sides or points can be changed | have: `shape.add-polygon`, `shape.add-star` | replicate | 2026-11-02, SS 5.0.539: polygon in the Shape flyout; no star tool was seen. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Basic shape palette (heart, triangle, score line, …) | DS | A panel of ready-made shapes | partial: geometric primitives only | reimagine with shapes generated in code; never copy their shape artwork | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Line | SS | Straight line tool | partial: the pen draws open paths | replicate | 2026-11-02, SS 5.0.539: Line Tools flyout: line, polyline, curve, arc |
| Polyline/Bezier pen | both | Click to place points, drag for curves | have: `PenDraft` → `shape.add-path` | replicate | 2026-11-02, SS 5.0.539: polyline and curve (Bezier) tools in the Line Tools flyout. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Freehand draw and smooth freehand | SS | Draw by hand, with optional smoothing | missing | replicate | 2026-11-02, SS 5.0.539: Freehand Tools flyout: freehand and smooth freehand; the tool stays active after a stroke |
| Arc | SS | Draw an arc segment | missing | replicate | 2026-11-02, SS 5.0.539: arc in the Line Tools flyout |
| Knife | both | Cut a shape apart along a drawn line | missing | replicate | 2026-11-02, SS 5.0.539: knife tool at the bottom of the tool rail; the tool stays active after use. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Eraser | SS | Erase part of a closed shape with a brush | missing | replicate | 2026-11-02, SS 5.0.539: eraser tool in the tool rail; the tool stays active after use |
| Flexishapes | SS (Designer Plus) | Parametric shape presets you can edit | missing | reimagine (parametric shapes) | 2026-11-02, SS 5.0.539: not present in Basic |
| Rhinestone, cross-stitch, puzzle generator | SS (Designer+) | Special pattern generators | missing | reimagine later, low priority | 2026-11-02, SS 5.0.539: not present in Basic (Sketch and Stipple are in Basic: see the Fill and line styles rows) |

## Text

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Text object with system fonts | both | Type text in an installed font; edit it later | partial: the Text tool places single-line engraved lettering from a built-in bitmap glyph set (80 characters max); no font support, the text cannot be edited later | replicate with user fonts read through `CRAFT_FONTS_DIR`/system fonts; no fonts bundled | 2026-11-02, SS 5.0.539: text tool with a font picker (frequently used, recently used, all fonts), size in pt (72 by default), bold/italic/underline; a text box/notes tool is also in the rail. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Vendor/licensed font libraries | both | Fonts from the vendor store or subscription | out of scope | won't do: licensed fonts and assets | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Size, letter spacing, line spacing, alignment | both | Typography controls | missing | replicate | 2026-11-02, SS 5.0.539: Text Style panel: line spacing %, character spacing %, a kerning toggle, left/centre/right/justify, horizontal or vertical text, a preview-text field. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Text on path / curve text | both | Lay text along a path (SS) or bend it into an arc (DS Curve) | missing | replicate | 2026-11-02, SS 5.0.539: a text-on-path control in the text options bar. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Spelling check | SS | Check the spelling of text objects | missing | reimagine later, low priority | 2026-11-02, SS 5.0.539: a Spelling tab in the Text Style panel and Panels › Spelling (F7) |
| Convert text to path / ungroup to letters | both | Turn letters into editable outlines | partial: placed text is already a path | replicate once real text exists | 2026-11-02, SS 5.0.539: Object menu has Convert to Path. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Writing fonts (single-line) | both | Single-stroke fonts for pens | partial: the built-in lettering is single-line engrave | replicate (user-supplied single-line fonts) | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Glyphs panel | SS (Designer) | Insert special characters from a font | missing | replicate | 2026-11-02, SS 5.0.539: not in Basic's Panels menu |

## Editing and node tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Select, multi-select, box/lasso | both | Pick objects; add to the selection with a modifier | have: click/box/lasso, additive multi-select | replicate | 2026-11-02, SS 5.0.539: 8-handle bounding box; Select All and Deselect All in the Edit menu; a drag box selects shapes it touches by default (a preference); many selected shapes share one bounding box. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Move, rotate, scale, exact size/position fields | both | Transform panel with numeric fields | have: `shape.move/rotate/scale/set-bounds`, `shapes.transform` | replicate | 2026-11-02, SS 5.0.539: Transform panel with Align, Scale, Rotate and Move tabs; the options bar also has W/H/X/Y fields (own row). DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Flip/mirror | both | Mirror horizontally/vertically | partial: possible through `shapes.transform` with negative scale; no menu item or button | replicate | 2026-11-02, SS 5.0.539: Object menu has a Mirror submenu. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Duplicate, cut/copy/paste | both | Copy objects; paste in place | partial: `shapes.duplicate`; no clipboard | replicate | 2026-11-02, SS 5.0.539: Cut/Copy/Paste and Duplicate (Ctrl+D) in the Edit menu; Paste in Front is its own row. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Group/ungroup | both | Treat objects as one unit | missing: the document has layers, but no groups | replicate | 2026-11-02, SS 5.0.539: Group and Ungroup in the Object menu, the toolbar and the selection options bar. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Compound path make/release | SS | Join paths into one shape with holes | partial: Pathfinder Merge/Union makes compound shapes; no release | replicate | 2026-11-02, SS 5.0.539: Make/Release Compound Path in the Object menu and the Modify panel |
| Point editing | both | Move, add and delete points; change smooth/corner points | partial: `BezierOverlay` direct select and convert-point | replicate | 2026-11-02, SS 5.0.539: point edit tool in the rail; Bezier control handles show for selected points only by default (a preference); Point Editing panel: delete point, break path, corner, smooth, make a segment flat or curved, simplify, convert to path. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Node count readout | SS | Counts for the selection: shapes, contours, nodes, straight and curved nodes | missing | replicate (cheap, helps judge cut complexity) | 2026-11-02, SS 5.0.539: shown in the Point Editing panel |
| Break/join path, close path | SS | Split a path at a point; join two endpoints | missing | replicate | 2026-11-02, SS 5.0.539: the Point Editing panel has Break Path; no join-path command was found in Basic |
| Simplify / reduce points | SS | Remove extra points within a tolerance | missing | replicate | 2026-11-02, SS 5.0.539: Object menu has Simplify (Ctrl+J) |
| Warp / distort | SS (Designer), DS | Bend a shape with an envelope or perspective | missing | reimagine, low priority | 2026-11-02, SS 5.0.539: not present in Basic. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Undo/redo history | both | Multi-step undo | have: journal-based undo/redo | replicate | 2026-11-02, SS 5.0.539: Undo/Redo in the Edit menu and the toolbar. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
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
| Outward/inward offset of a shape | both | Make a new outline a set distance outside or inside the shape | partial: the `kerf` crate offsets contours for kerf compensation, but there is no user offset command that adds a new shape | replicate as an engine command on top of `kerf` | 2026-11-02, SS 5.0.539: Offset panel shows a live preview on the selection before Apply; the dimension callouts update to the offset size; Cancel discards it. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Corner style (round/sharp) | both | Choose rounded or sharp corners, with a limit on how far sharp corners reach | partial: kerf uses a miter join only | replicate | 2026-11-02, SS 5.0.539: sharp or round corners plus a corner limit value (1.0 default); the live test defaulted to round at 0.125 in, the panel showed 0.079 in. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Weld offsets / per-letter offsets | DS | Merge the offset outlines into one, or keep one per piece | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Internal offset | SS | Offset inward, used for frames and borders | partial (kerf counters) | replicate | 2026-11-02, SS 5.0.539: Internal Offset is a mode beside Offset in the same panel |

Sources: [Cricut, *How to use the Offset feature*](https://help.cricut.com/hc/en-us/articles/360061650414-How-to-use-the-Offset-feature-in-Design-Space).

## Trace

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Threshold trace of a raster image | both | Choose an area or the whole image; turn dark pixels into cut outlines | have: `trace-image` CLI and the Trace Image tool; PNG/JPEG; adjustable threshold | replicate | 2026-11-02, SS 5.0.539: Trace panel: select the trace area, then threshold % and despeckle threshold %; PNG files are auto-traced on import by default (a preference). DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| High/low-pass filter, scale and colour-range trace | SS | Image filters and colour selection that guide the trace | missing | replicate | 2026-11-02, SS 5.0.539: an Advanced section in the Trace panel holds the filters (not expanded yet) |
| Image effects | SS | Brightness, colour and other adjustments on placed images | missing | reimagine later, low priority | 2026-11-02, SS 5.0.539: the panel needs GPU support and showed only an unsupported-graphics notice in the VM |
| Trace outer edge only / trace & detach | SS | Keep only the outline, or split the image along the trace | missing | replicate | 2026-11-02, SS 5.0.539: Trace Outer Edge and Trace and Detach are actions next to Trace |
| Background remover / "complexity" upload flow | DS | A guided upload that removes the background, then cleans up with erase tools | missing | reimagine (local, deterministic; no cloud ML) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Live preview of the trace on screen | both | Highlights the area that will be traced | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Full-colour/multi-layer trace | both | Trace each colour into its own layer | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |

Sources: [Cricut, *How to upload images*](https://help.cricut.com/hc/en-us/articles/360009556313-How-to-upload-images-into-Design-Space).

## Weld and pathfinder

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Weld / Unite | both | Merge overlapping shapes into one outline | have: Pathfinder `union`/`merge` | replicate | 2026-11-02, SS 5.0.539: Weld in the Modify panel and the selection options bar. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Subtract / Subtract All / Minus Back | both | Remove the top shape(s) from another | have: `difference`, `minus-back` | replicate | 2026-11-02, SS 5.0.539: Subtract and Subtract All in the Modify panel. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Intersect | both | Keep only the overlap | have: `intersect` | replicate | 2026-11-02, SS 5.0.539: Intersect in the Modify panel. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Exclude / XOR | both | Keep everything except the overlap | have: `xor` | replicate | 2026-11-02, SS 5.0.539: no exclude/XOR action in Basic's Modify panel. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Divide / Slice | both | Split overlapping shapes into separate pieces | have: `divide`, `trim`, `crop`, `outline` | replicate | 2026-11-02, SS 5.0.539: Divide and Crop in the Modify panel. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Detach Lines | SS | Separate a shape's lines from it | missing | replicate once its behaviour is confirmed | 2026-11-02, SS 5.0.539: a Modify panel action; its behaviour not tested yet |
| Attach | DS | Keep pieces in their relative positions on the mat without merging them | missing | reimagine (groups that keep positions when sent to the mat) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Flatten (print layers into one printable image) | DS | Merge layers into one Print Then Cut image | missing | reimagine as part of Print & Cut | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Contour (hide parts of a shape) | DS | Hide individual sub-paths so they are not cut | missing | replicate (per-subpath enable) | blocked: sign-in required (2026-11-02, DS 9.83.76) |

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
| Align left/centre/right/top/middle/bottom | both | Align the selection | have: `shapes.align` | replicate | 2026-11-02, SS 5.0.539: Transform panel, Align tab: horizontal left/centre/right, vertical top/middle/bottom. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Align to page/mat | SS | Align against the media instead of the selection | have: `to_board` | replicate | 2026-11-02, SS 5.0.539: centre to page in the Align tab and the options bar |
| Distribute horizontally/vertically | both | Even spacing | have: `shapes.distribute` | replicate | 2026-11-02, SS 5.0.539: Distribute horizontally/vertically in the Align tab. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Centre to page | SS | Put the selection in the middle of the media | have (align to board, both axes) | replicate | 2026-11-02, SS 5.0.539: centre to page and centre on each other in the Align tab |
| Spacing by a set value | SS | Exact gap between objects | missing | replicate | 2026-11-02, SS 5.0.539: Basic has only even distribution (Space Horizontally / Space Vertically); no spacing-by-value control |

## Layers and objects

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Layers panel: add/remove/show/hide | both | List of layers or objects with visibility toggles | have: `layer.add/remove/set-visible`, Layers & Objects panel | replicate | 2026-11-02, SS 5.0.539: no layers panel in Basic's panel column (vendor docs list layers as a Designer feature). DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Reorder (bring forward/send back, drag in list) | both | Change the stacking order | missing | replicate | 2026-11-02, SS 5.0.539: Object menu has an Arrange submenu; the options bar has arrange too; Arrange: bring to front, send to back, bring forward, send backward, each with a shortcut. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Arrange shortcuts | SS | Keyboard shortcuts for bring to front/forward and send to back/backward | missing (no reorder command yet) | replicate with the reorder command | 2026-11-02, SS 5.0.539: Ctrl+Shift+] / Ctrl+Shift+[ for front/back, Ctrl+] / Ctrl+[ for one step |
| Lock layer | SS | Stop a layer from being edited | missing | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Rename layer/object | both | Give a layer or object a label | partial: layers have names; there is no rename command | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Move object between layers | SS | Drag an object to another layer | missing | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Colour Sync | DS | Merge layers of similar colours so fewer mats are used | missing | reimagine (merge by operation/material) | blocked: sign-in required (2026-11-02, DS 9.83.76) |

## Fill and line styles

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Line colour and width | both | Stroke colour, used to group cut operations | partial: a shape is `cut` or `engrave` (`CutStyle`), shown in fixed production colours | reimagine: operation-first, colour as a label | 2026-11-02, SS 5.0.539: Line Style panel: a line-colour tab and thickness in pt; new shapes default to outline only, with a default line colour (preferences). DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Line style (dashed, perforated) | SS | Solid or dashed lines, with thickness, corner joins, end caps and stroke position | missing | replicate (perforation as a cut setting) | 2026-11-02, SS 5.0.539: solid/dash style, thickness (pt), corners miter/round/bevel, end caps butt/round/square, position centre or inside |
| Fill colour, gradient, pattern fill | both | Fill shapes for Print & Cut | missing | replicate colour/gradient fill for printing; user-supplied patterns only | 2026-11-02, SS 5.0.539: Fill panel tabs Solid/Gradient/Pattern; swatch grid with a no-fill swatch, recent colours, eyedropper, named custom palettes, transparency %, colour spectrum and a hex field. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Print cut lines | SS | Print the outlines of chosen shapes too (zero-width lines print as hairlines) | missing | replicate | 2026-11-02, SS 5.0.539: a Line Style option to print lines of the selected shapes; 0 pt lines print as 0.1 pt, also in SVG/PDF exports |
| Sketch and Stipple fills | SS | Fill a shape with hatch lines or dots for pens | missing | replicate (hatch fills for pen/engrave) | 2026-11-02, SS 5.0.539: Sketch fill patterns: cross-hatch, grid, lines, concentric, spiral; spacing and angle; Release Effects; Stipple on its own tab |
| Specialty tools (emboss/deboss, foil) | SS | Patterns and settings for emboss, deboss and foil tools | missing | replicate the hatch patterns; tool support follows the machine work | 2026-11-02, SS 5.0.539: a panel with tabs per tool type; deboss/emboss toggle; the same five hatch patterns; spacing, angle, Release Effects |
| Pop-up card generator | SS | Generate the struts for a pop-up card from a design | missing | reimagine later, low priority | 2026-11-02, SS 5.0.539: on/off with base width %, strut width and dash pitch |
| Vendor pattern libraries | both | Store pattern artwork | out of scope | won't do: licensed assets | 2026-11-02, SS 5.0.539: a Pattern tab exists in the Fill panel; where its patterns come from was not checked. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Operation type (cut, draw/pen, score, deboss, engrave, wave, perforate, foil, print then cut) | DS | Each layer says what the machine does with it | partial: cut and engrave only | replicate the common ones (cut, pen, score, perforate) | blocked: sign-in required (2026-11-02, DS 9.83.76) |

## Print & Cut and registration marks

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Registration marks on the printed page | both | Printed marks the cutter's sensor uses to find the artwork | partial: `print-cut` writes an SVG page with three marks at a fixed 12 mm margin | replicate | 2026-11-02, SS 5.0.539: Print & Cut panel (View › Registration Marks) turns marks on; marks are off by default (a preference). DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Show print border | SS | Draw the printer's printable area on the page | missing | replicate | 2026-11-02, SS 5.0.539: View › Show Print Border (Ctrl+Shift+P), off by default |
| Mark style, length, inset choices | SS | Change the size and position of the marks | missing | replicate | 2026-11-02, SS 5.0.539: length 0.787 in, thickness 0.020 in, inset 0.625 in by default; More Settings; Restore Defaults |
| Send to printer from the app | both | System print dialog, with bleed and actual-size checks | missing: StencilCraft does not drive a printer | replicate | 2026-11-02, SS 5.0.539: File › Print and Print Page Setup; a Print button in the Print & Cut panel; print resolution and print preview are preferences. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Bleed | both | Extend the print colour past the cut line so a slightly-off cut shows no white edge | missing | replicate | 2026-11-02, SS 5.0.539: Enable Print Bleed with a bleed radius (0.050 in default) in the Print & Cut panel. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Optical sensor registration (automatic alignment) | both | The machine scans the marks before it cuts | missing (`TB5x` commands not driven) | replicate from protocol facts | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Print & Cut calibration | both | Adjust for sensor/printer offset with a test sheet | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Print size limits | DS | Maximum printable area per mat/page | missing | replicate (validation) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| PixScan / scan-to-cut | SS | Photograph the material on a special mat and cut to it | missing | reimagine later, low priority | 2026-11-02, SS 5.0.539: the PixScan panel opens a camera/scan calibration dialog; File › Scan is also present |

Sources: [Cricut, *How to Print Then Cut*](https://help.cricut.com/hc/en-us/articles/360009387274-How-to-Print-Then-Cut-in-Design-Space), [Cricut, *How large can I Print Then Cut?*](https://help.cricut.com/hc/en-us/articles/360009429814-Design-Space-says-printable-image-is-too-large), [Silhouette, *Print and Cut troubleshooting*](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000276219-print-and-cut-troubleshooting). See also [`print-cut.md`](print-cut.md).

## Cut settings, materials and tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Material presets | both | Choose the material; force, speed and passes fill in from the preset | have: `MEDIA_PRESETS` (27 entries) with a Send-panel chooser | replicate with our own numbers; never import their preset data files | 2026-11-02, SS 5.0.539: Standard sub-tab: material, action and tool dropdowns per tool holder; a long built-in material list; a notice when the material has no action for that holder. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Force, speed, passes override | both | Change the preset values for this job | partial: force and speed are overridable; passes are not exposed for the cutter (G-code has `--passes`) | replicate | 2026-11-02, SS 5.0.539: Material Settings: force, speed, passes, acceleration and a blade-depth dial, each a slider plus a number. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Advanced material settings | SS | Per-material settings beyond force and speed: tracking, kraft-blade loop corners, smart cut | missing | replicate the documented behaviours from our own testing | 2026-11-02, SS 5.0.539: Track Enhancing checkbox; the kraft blade adds loop force, loop blade height, loop overcut, loop radius and a smart-cut toggle |
| Blade depth / tool type | SS | Pick the tool (ratchet, auto blade, kraft, rotary, pen) and blade depth | partial: `ToolKind` parsed from the device; no chooser in the UI | replicate | 2026-11-02, SS 5.0.539: the tool is detected from the holder (it shows awaiting connection without a machine); blade-depth dial for ratchet/auto blades; the kraft blade has its own sub-tab; blade type defaults to automatic |
| Custom user materials | both | Save your own material presets | missing | replicate | 2026-11-02, SS 5.0.539: +/− buttons in the material list, Create New…, and Revert / Save As… in Material Settings. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Per-layer/per-colour settings | SS | Different settings per line colour or layer | missing | replicate | 2026-11-02, SS 5.0.539: Cut by Line Color / Cut by Fill Color tables, one row per colour (own row: Send sub-modes) |
| Test cut | both | Cut a small test shape first | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Overcut / line segment overcut | SS | Cut a little past corners or ends so pieces lift cleanly | missing | replicate | 2026-11-02, SS 5.0.539: a Line Segment Overcut checkbox per material; kraft blade adds loop overcut; Preferences › Advanced has Software Overcut (on) |
| Cut order / path sorting | SS | Cut inner shapes first and keep travel short | missing | replicate | 2026-11-02, SS 5.0.539: an Intelligent Path checkbox per material; its behaviour not tested yet |
| Mat-free (carrier sheet free) cutting strategy | SS | Path ordering that lets material be cut without a mat | missing | replicate | not present in Basic (2026-11-02, SS 5.0.539) |
| Weed lines and weed borders | SS (Business) | Extra cut lines that make weeding easier | missing | replicate | 2026-11-02, SS 5.0.539: not present in Basic |
| Laser/G-code profiles | — | Not in these apps; StencilCraft's own | partial | — | — |

## Send panel and machine connection

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Design → Send/Make split | both | A separate production screen before cutting | have: Design/Send split with a dry-run program summary | replicate | 2026-11-02, SS 5.0.539: the SEND tab turns the canvas into a cut preview with settings on the right, in Guide / Standard / By Line Color / By Fill Color sub-tabs plus a gear for send preferences. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Send sub-modes: Standard / by line colour / by fill colour | SS | Settings for the whole job, or one row of settings per line or fill colour | missing | reimagine: per-layer/operation settings | 2026-11-02, SS 5.0.539: By Line Color table: tool number, on/off, outline-only, colour, material, action, tool; By Fill Color is the same keyed by fill |
| Assign lines to tool holders on the cut preview | SS | Send the selected lines (or only their outline) to tool 1 or tool 2, or skip them | missing | replicate (dual-holder machines) | 2026-11-02, SS 5.0.539: overlay buttons: tool 1 all lines, tool 1 outline, tool 2 all, tool 2 outline, ignore selected lines |
| Mat preview on the Make screen | DS | Shows how pieces sit on each mat; you can move them there | missing | reimagine | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Machine selection | both | Choose the cutter model | have: model chooser (`Model`, Cameo 3–5, Portrait, Pro) | replicate | 2026-11-02, SS 5.0.539: machine card with a model dropdown (CAMEO 5 by default) and an illustration (own row: machine card). DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Machine card (model, firmware, status) | SS | A card showing the chosen machine, its firmware and its state | partial: model chooser; firmware version query in the CLI send path only | replicate | 2026-11-02, SS 5.0.539: model dropdown, firmware (unknown without a machine), status (none), an illustration |
| USB connection | both | Send the job over USB | partial: serial (CDC/ACM) transport via CLI `--device`; no USB-bulk transport; the app does not send | replicate (`nusb`) | 2026-11-02, SS 5.0.539: Guide sub-tab lists detected USB and Bluetooth devices with Connect; no machine was attached in the VM. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Bluetooth connection | both | Wireless send | missing | replicate | 2026-11-02, SS 5.0.539: Bluetooth devices share the same list; a banner said Bluetooth was unavailable in the VM. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Device discovery and status | both | Find connected cutters; show ready/loaded/tool status | partial: status query decoded; no discovery or live status in the UI | replicate | 2026-11-02, SS 5.0.539: Rescan for Devices button; with no machine the card shows unknown firmware and no status; a preference releases the device automatically when not in use (off by default). DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Packet size | SS | How much data is sent to the machine per write | missing (CLI streams the whole program) | replicate if real hardware needs it | 2026-11-02, SS 5.0.539: Preferences › Advanced: packet size Auto (1000) |
| Load/unload media, pause, cancel | both | Machine controls during a job | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Multi-cutter support | SS (Business) | Send jobs to several machines at once | missing | replicate later | 2026-11-02, SS 5.0.539: not present in Basic |
| Batch / queued jobs | SS (Business) | Queue several designs or repeats as one job | missing | replicate | 2026-11-02, SS 5.0.539: not present in Basic |
| Cricut machine protocol | DS | Driving Cricut machines | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Firmware updates | both | Update the machine firmware from the app | out of scope | won't do: vendor firmware | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |

## Library and store

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Local design library | SS | Organise designs on disk in folders | missing (a Recent menu only) | reimagine: plain folders of `.stencilcraft` files | 2026-11-02, SS 5.0.539: LIBRARY tab; a folder button on the tool rail opens a library sidebar; Save to Library and Update Library in the File menu; the default save location (disk or library) is a preference |
| Design store / Cricut Access images and projects | both | Buy or subscribe to designs | out of scope | won't do: proprietary cloud store and licensed assets | 2026-11-02, SS 5.0.539: STORE tab present; not explored. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Ready-to-make projects | DS | Project templates from the vendor | out of scope | won't do: licensed assets; we may ship original example files | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Templates (mockups like shirts and mugs) | DS | Background images to size designs against | missing | reimagine with original, code-generated templates | blocked: sign-in required (2026-11-02, DS 9.83.76) |

## File import and export formats

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| SVG import | SS (Designer+), DS | Open vector artwork | have: a working subset (see [`gaps.md`](gaps.md) 1) | replicate | 2026-11-02, SS 5.0.539: Basic's Open dialog has no SVG. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Merge (import into current document) | SS | Open a file's contents into the document already open | have: Import SVG/Image add to the open document | replicate | 2026-11-02, SS 5.0.539: File › Merge (Ctrl+Shift+O) |
| Open Recovered (autosave recovery) | SS | Reopen work saved automatically before a crash | missing: no autosave | replicate | 2026-11-02, SS 5.0.539: File › Open Recovered… |
| DXF import | both | Open CAD outlines | missing (DXF export only) | replicate | 2026-11-02, SS 5.0.539: DXF is in Basic's Open types; import options: fit to page, convert white lines to black, join lines into polylines. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| PDF import | SS (Designer+) | Open vector PDFs | missing | replicate | 2026-11-02, SS 5.0.539: Basic's Open dialog has no PDF |
| AI/EPS/CDR import | SS (Business) | Open Illustrator/CorelDRAW files | missing | replicate AI (its PDF-compatible content) and EPS (a PostScript subset); CDR only if its structure can be learned clean-room | 2026-11-02, SS 5.0.539: not present in Basic |
| Raster import (PNG/JPEG/BMP/GIF/TIFF) | both | Place a photo or clip art to trace or print | partial: PNG/JPEG for tracing only; no placed raster for printing | replicate | 2026-11-02, SS 5.0.539: PNG, JPEG, BMP, GIF and TIFF in Basic's Open types; PNG autotrace on import is on by default. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Embroidery formats (PES, DST, …) | SS (Designer Plus) | Import embroidery outlines | missing | replicate (outline only) | 2026-11-02, SS 5.0.539: not present in Basic |
| Native `.studio`/`.studio3` | SS | The vendor's own formats: Basic saves `.studio3`, `.studio` and GSP and opens those plus GSD | missing | reimagine: import needs a public spec or a clean-room study of files users own (owner decision); until then users export DXF | 2026-11-02, SS 5.0.539: Basic's save types: Studio V3 (.studio3), V2 (.studio), GSP; GSD open options: as-is, convert white lines to black, join lines into polylines |
| SVG export | SS (Business) | Save as SVG | have | replicate | 2026-11-02, SS 5.0.539: Basic has no SVG export (and no DXF, PDF or PNG) |
| PDF/PNG/JPG export | SS (Business) | Save as PDF or image | missing | replicate (PDF first) | 2026-11-02, SS 5.0.539: not present in Basic |
| DXF export | — | StencilCraft's own | have (R14, flattened) | — | — |
| G-code export | — | StencilCraft's own | have | — | — |
| Plugins for external design tools | SS (Silhouette Connect) | Send straight from Illustrator/CorelDRAW to the cutter | missing | reimagine: CLI/MCP and SVG/PDF interchange; never tie the core to proprietary software | pending |
| Cloud-only save | DS | Projects saved to the vendor cloud | out of scope | won't do: account system; we save open, local files | blocked: sign-in required (2026-11-02, DS 9.83.76) |

Sources: [Silhouette, *File Types and Features*](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000282776-file-types-and-features), [Cricut, *Image uploads: unsupported items*](https://help.cricut.com/hc/en-us/articles/360009553213-Image-uploads-unsupported-items).

## Preferences

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Theme / appearance | both | Light or dark interface | have: suite light/dark themes, saved between sessions | replicate | 2026-11-02, SS 5.0.539: Display tab: theme (Light by default, with a choice), background colour, button size, animation speed, antialiasing and curve quality (the last two need a restart). DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Print resolution and preview | SS | The dpi used for printing and whether a preview is shown first | missing | replicate with printing | 2026-11-02, SS 5.0.539: General tab: print resolution 300/600 (default)/1200/custom dpi; print preview on |
| Reset and maintenance options | SS | Restore defaults, show hidden dialogs, graphics settings | partial: Reset Workspace | replicate | 2026-11-02, SS 5.0.539: Advanced tab: restore factory defaults, restore pre-loaded designs, unhide dialogs, library permissions, OpenGL settings |
| Usage data collection | SS | Opt-in performance data sent to the vendor | out of scope: StencilCraft sends nothing | won't do (owner decision if this ever changes) | 2026-11-02, SS 5.0.539: an opt-in setting, off by default |
| Units and grid spacing | both | Default units; grid size | partial: grid toggle only; mm fixed | replicate | 2026-11-02, SS 5.0.539: General tab: unit of measurement, Auto (Inches); Show Dimensions is on; grid spacing in the Grid Options dialog. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Default material/machine | both | Settings that start each document and job: orientation, blade, page colour, fill style, text style | partial: the Send panel defaults are fixed in code | replicate | 2026-11-02, SS 5.0.539: Defaults tab: page orientation and blade type are automatic; default page colour; default fill style outline-only with default line/fill colours; default text style; always show the mat; cut to page edge; registration marks off. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Import defaults (DPI, scale) | SS | How imported files are sized | partial: trace uses 96 dpi by default | replicate | 2026-11-02, SS 5.0.539: Import tab: options for GSD and DXF (white lines to black, join into polylines, DXF fit to page) and PNG autotrace on import |
| Workspace layout | SS | Panel arrangement | partial: dock layout and Reset Workspace | replicate | 2026-11-02, SS 5.0.539: panels open from a column of icons on the right as floating, collapsible, closable docked panels; scrollbar side is a preference; Starter mode hides most of them (own row) |
| Recent files | both | Reopen recent documents | have: eight-entry Recent menu | replicate | 2026-11-02, SS 5.0.539: File › Open Recent; the Home screen lists recent library files. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Language | both | UI language | missing | replicate later | 2026-11-02, SS 5.0.539: General tab: language, default English. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |

## Account and cloud

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Sign-in / account | both | A vendor account: DS needs one before any design screen opens; in SS it is optional (store and cloud) | out of scope | won't do: account systems; StencilCraft works offline and needs no account | 2026-11-02, SS 5.0.539: not asked to sign in during this session. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Cloud library sync | both | Sync designs across devices | out of scope | won't do: no hosted service; users sync files with their own tools | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Purchased-design management | both | Manage licences for bought designs | out of scope | won't do: proprietary store | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Offline mode | DS | Work without a connection (limited) | have: StencilCraft is offline by design | — | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Mobile apps | both | Phone and tablet clients | missing | reimagine as the WASM/browser build (M5) | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Machine registration | both | Register a machine to an account | out of scope | won't do: account system | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |

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
| Rhinestone tool | Designer | Place stones along a line, around an outline or as a fill; stone size, spacing and shape | missing | replicate (stone templates are just circles we generate) | not present in Basic; from vendor docs (not observed) |
| Sketch effect | Designer (a Sketch panel was also visible in Basic 5.0.539) | Turn fills into hatch lines a pen can draw | missing | replicate | vendor docs list it as Designer; a Sketch panel was visible in Basic 5.0.539 (2026-11-02) |
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
| Flexishapes | Designer Plus | Parametric shapes with handles you can drag | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Puzzle generator | Designer Plus | Cut a design or photo into jigsaw pieces | missing | replicate, low priority | not present in Basic; from vendor docs (not observed) |
| SVG export | Business | Save as SVG | have | replicate | not present in Basic; from vendor docs (not observed) |
| PDF export | Business | Save as PDF | missing | replicate | not present in Basic; from vendor docs (not observed) |
| PNG/JPG export | Business | Save as an image | missing | replicate | not present in Basic; from vendor docs (not observed) |
| AI, EPS and CDR import | Business | Open Illustrator, EPS and CorelDRAW files | missing | replicate AI (its PDF-compatible content) and EPS (a PostScript subset); CDR only if its structure can be learned clean-room | not present in Basic; from vendor docs (not observed) |
| Multi-cutter support | Business | Drive several machines from one app, each with its own job | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Nesting | Business | Pack shapes tightly onto the media to save material | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Tiling | Business | Split a design larger than the media into tiles | have: `tiling` crate | replicate | not present in Basic; from vendor docs (not observed) |
| Weed lines and borders | Business | Add cut lines around and between shapes so waste lifts in pieces | missing | replicate | not present in Basic; from vendor docs (not observed) |
| Job queue / batch | Business (to confirm) | Queue several designs or copies as one production run | missing | replicate | not present in Basic; from vendor docs (not observed) |

## Cricut Design Space, full inventory

Design Space features by screen, from the [Cricut help centre](https://help.cricut.com/hc/en-us)
(notably the [Canvas screen guide](https://help.cricut.com/hc/en-us/articles/26751604105879-Canvas-screen-Design-Space-Guide),
[Offset](https://help.cricut.com/hc/en-us/articles/360061650414-How-to-use-the-Offset-feature-in-Design-Space),
[uploads](https://help.cricut.com/hc/en-us/articles/360009556313-How-to-upload-images-into-Design-Space) and
[Print Then Cut](https://help.cricut.com/hc/en-us/articles/360009387274-How-to-Print-Then-Cut-in-Design-Space)).
Some rows repeat a DS row in the area tables above, so this table can be read on its own.

| Feature | Area | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Home: new project, my projects, ready-to-make | Home | Start screen with recent projects and vendor projects | missing | reimagine: new/recent local files; vendor projects are store content (won't do) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Images panel (image library search) | Design panels | Search and insert images from the vendor library | out of scope | won't do: store content; reimagine as browsing the user's own folders | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Projects panel | Design panels | Open vendor ready-to-make projects | out of scope | won't do: store content | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Text panel (add text) | Design panels | Add a text object, then pick font and style in the edit bar | partial: built-in single-line lettering only | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Shapes panel | Design panels | Square, triangle, circle, heart, star, pentagon, hexagon, octagon and a score line | partial: rectangle, ellipse, polygon, star; no heart/triangle presets or score line | replicate (shapes generated in code) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Upload panel | Design panels | Upload raster or vector files to the canvas | partial: SVG import and PNG/JPEG trace | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Templates panel | Design panels | Mock-up backgrounds (shirts, mugs, cards) to size a design against | missing | reimagine with original, code-generated templates | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Smart Guides | Canvas | Alignment lines that appear while dragging near other objects | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Canvas grid and units | Canvas | Grid on/off, grid size, inches or cm | partial: grid toggle; mm only | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| History panel | Canvas | A list of past edits; jump back to any step | missing (undo/redo only) | replicate on the edit journal | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Undo/redo | Edit bar | Step back and forward | have | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Operation: Basic Cut | Edit bar | Cut the outline | have: `cut` style | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Operation: Pen / Draw | Edit bar | Draw the outline with a pen, with a pen colour | partial: `engrave` style traces without cutting; no pen colour | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Operation: Score | Edit bar | Fold line with a scoring stylus or wheel | partial: `engrave` style; no score tool | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Operation: Engrave | Edit bar | Engrave with an engraving tip | partial: `engrave` style; no engraving-tool settings | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Operation: Deboss | Edit bar | Press a line into the material | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Operation: Wave | Edit bar | Cut a wavy decorative line | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Operation: Perforate | Edit bar | Cut a dashed tear line | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Operation: Foil | Edit bar | Press foil with a foil tool (fine, medium, bold) | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Operation: Print Then Cut | Edit bar | Print the layer, then cut around it | partial: `print-cut` printable sheet; no sensor alignment | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Select all / deselect | Edit bar | Select every object or none | have | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Edit dropdown: cut, copy, paste | Edit bar | Clipboard actions | partial: duplicate only | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Align and distribute | Edit bar | Align left/centre/right/top/middle/bottom, distribute | have: `shapes.align`, `shapes.distribute` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Arrange: to back, backward, forward, to front | Edit bar | Change the stacking order | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Flip horizontal/vertical | Edit bar | Mirror the selection | partial: negative scale through `shapes.transform`; no button | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Size W/H with lock | Edit bar | Exact size with a proportion lock | partial: size fields and an aspect-link setting | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Rotate field | Edit bar | Exact rotation angle | have: `shape.rotate`, `shapes.transform` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Position X/Y | Edit bar | Exact position | have: `shape.set-bounds` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Font picker (system and vendor fonts, filters) | Edit bar (text) | Choose a font; filter by system, vendor, writing | missing | replicate for system/user fonts; vendor fonts are store content (won't do) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Font style (regular, bold, italic, writing) | Edit bar (text) | Style of the font, including single-line writing styles | partial: built-in single-line lettering | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Font size | Edit bar (text) | Text size | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Letter space | Edit bar (text) | Spacing between letters | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Line space | Edit bar (text) | Spacing between lines | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Text alignment | Edit bar (text) | Left, centre, right | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Curve text | Edit bar (text) | Bend text into an arc by a diameter value | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Advanced: ungroup to letters / lines | Edit bar (text) | Split text into one object per letter or line | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Linetype swatch / colour | Edit bar | Colour of a cut or draw layer, used to group mats | partial: cut/engrave only, fixed colours | reimagine: operation first, colour as a label | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Fill: no fill / print | Edit bar | Make a layer printable with a colour or pattern | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Patterns | Edit bar | Fill printable layers with a pattern; upload your own patterns | missing | replicate with user patterns; vendor patterns are store content (won't do) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Warp | Edit bar | Bend a shape or text with preset warps | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Offset | Edit bar | Outline at a distance, round or sharp corners, weld offsets | partial: kerf offset only | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Group / ungroup | Layers panel | Treat layers as one unit | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Duplicate | Layers panel | Copy the selection | have: `shapes.duplicate` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Delete | Layers panel | Remove the selection | have: `shapes.remove` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Hide layer | Layers panel | Hide a layer from canvas and mats | have: `layer.set-visible` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Slice | Layers panel | Split two overlapping shapes into pieces | have: Pathfinder `divide` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Combine: Weld | Layers panel | Merge into one outline | have: Pathfinder `union` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Combine: Unite | Layers panel | Join into one layer that keeps the overlapping outlines | partial: Pathfinder `merge` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Combine: Subtract | Layers panel | Cut the top shape out of the bottom one | have: Pathfinder `difference` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Combine: Intersect | Layers panel | Keep only the overlap | have: Pathfinder `intersect` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Combine: Exclude | Layers panel | Remove the overlap | have: Pathfinder `xor` | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Attach / detach | Layers panel | Keep positions together on the mat without merging | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Flatten / unflatten | Layers panel | Merge layers into one Print Then Cut image | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Contour | Layers panel | Hide single sub-paths so they are not cut | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Colour Sync | Layers panel | Merge layers of similar colours so fewer mats are needed | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Reorder layers by drag | Layers panel | Change stacking by dragging in the list | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Make It (go to Make screen) | Make screen | Leave the design and set up the cut | have: Design/Send split | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Mat preview | Make screen | One mat per colour/material with the pieces laid out | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Move, rotate, hide items on the mat; move to another mat | Make screen | Arrange pieces on each mat by hand | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Mirror per mat | Make screen | Mirror a mat's contents (for iron-on vinyl cut face down) | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Material size | Make screen | Pick the mat or sheet size (12×12, 12×24, small Joy sizes, large Venture sizes) | partial: `Mat` table for Silhouette mats | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Load type: on mat / without mat | Make screen | Cut on a mat or straight from Smart Materials without one | partial: `Mat::None` exists; no load-type choice | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Project copies | Make screen | Cut several copies of the project | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Material list, favourites and custom materials | Make screen | Browse and favourite materials, add your own | partial: 27 presets; no favourites or custom ones | replicate with our own numbers | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Pressure: default, more, less | Make screen | Quick pressure adjustment | partial: force override | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Tool and accessory prompts | Make screen | Tells you which blade/tool to load in which clamp | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Go, pause, unload | Make screen | Machine controls during the job | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Print Then Cut size limit | Make screen | Maximum printable area for Print Then Cut | missing | replicate (validation) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Print Then Cut flow: bleed, system dialog, printer | Make screen | Send to a printer with bleed on or off, optionally through the system dialog | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Machine calibration | Make screen | Print Then Cut, rotary blade and knife blade calibration | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Cricut Joy / Joy Xtra | Machines | Small machines: Smart Materials without a mat, Card Mat, insert cards; Print Then Cut on Joy Xtra | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Cricut Explore family | Machines | Mid-size machines with two clamps (blade plus pen/scoring stylus), Smart Materials on Explore 3 | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Cricut Maker family | Machines | Adaptive tools: rotary blade, knife blade (multi-pass), scoring wheel, debossing, engraving, perforation, wave, foil | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Cricut Venture | Machines | Large-format machine with a 24-inch mat and long rolls | missing | pending owner decision: Cricut machines speak a proprietary, reportedly encrypted protocol tied to Design Space; supporting them would mean reverse-engineering that protocol and possibly circumventing a technical protection measure. We do not do that without the owner's explicit decision and legal review | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Remove background | Upload | Automatic background removal on uploaded photos | missing | reimagine (local, deterministic) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Erase / restore cleanup | Upload | Brush to erase or restore parts of an uploaded image before cutting | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Save as cut image vs Print Then Cut image | Upload | Choose whether an upload becomes an outline or a printable image | partial: uploads become cut outlines (trace) | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Vector upload (SVG, DXF) with ungroup | Upload | Upload vector files; their layers become separate layers | partial: SVG import; no DXF import | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Upload file types (JPG, GIF, PNG, BMP, SVG, DXF, HEIC) | Upload | Formats the upload accepts (no TIFF or PDF) | partial: PNG, JPEG, SVG | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Uploaded images library | Upload | Reuse earlier uploads | missing | reimagine: local folder of the user's images | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Project Collections | Projects | Organise saved projects into collections | missing | reimagine with local folders | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Cloud save / sharing | Projects | Save and share projects through the vendor cloud | out of scope | won't do: account system | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Cricut Access subscription | Account | Subscription for vendor images, fonts and projects | out of scope | won't do: proprietary store | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Offline mode (download fonts and images) | Account | Use downloaded content without a connection | have: StencilCraft is offline by design | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Settings: machine, canvas, load last project | Settings | Default machine, grid/units, reopen the last project at start | partial: theme and Recent; no default machine or reopen-last | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |

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
