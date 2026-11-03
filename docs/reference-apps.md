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
| Page/media size | SS | Set the media width and height, or choose a preset size; the design area matches the material | have | replicate | pending |
| Cutting mat selection and backdrop | both | Choose the mat (12×12, 12×24, …); the canvas shows it behind the design | have: `Mat` table in `crates/machine`, mat backdrop in the app | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Multiple pages/tabs per document | SS | Several open designs as tabs, and several pages in one file | missing: one document per window, one board | reimagine (tabs of documents) | pending |
| Infinite canvas with auto mat layout | DS | Design on a free canvas; the Make screen lays shapes out onto mats per colour/material | missing | reimagine: bridge to nesting and tiling | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Orientation and rotated view | SS | Portrait/landscape feed and a rotated view of the mat | partial: the encoder can emit the feed direction (`TB50`), but the UI does not expose it | replicate | pending |
| Cut margin / safe area | SS | A border around the media that the blade will not enter | partial: plot margins exist in `PlotOptions`; nothing is drawn on the canvas | replicate | pending |
| Rulers, grid, guides | both | Rulers on the canvas edges, a background grid, draggable guides | partial: rulers (with zero reset) and grid toggle exist; no guides | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Snap to grid/objects | both | Objects snap to grid lines, guides or each other's edges | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Zoom, pan, fit | both | Zoom tools, hand pan, fit to page/selection | have: Hand, Zoom, Fit (F), Center (C), status-bar zoom | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Measure | SS | Measure a distance on the canvas | missing | replicate | pending |
| Units | both | Choose inches or millimetres | partial: everything is millimetres; no inch display option | replicate (display-only conversion at the UI boundary) | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Kerf/compensation preview | — | Not a reference-app feature; StencilCraft's own | have | — | — |
| Oversized design tiling | SS (Business) | Split a design larger than the media into tiles that are cut separately | have: `tiling` crate, `tile-preview`, overlap and marks | replicate | pending |

## Drawing tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Rectangle, rounded rectangle | both | Drag to draw; the corner radius can be changed | have: `shape.add-rect`, `shape.add-rounded-rect` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Ellipse/circle | both | Drag to draw; hold a modifier for a circle | have: `shape.add-ellipse` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Polygon, star | both | Regular polygon or star; the number of sides or points can be changed | have: `shape.add-polygon`, `shape.add-star` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Basic shape palette (heart, triangle, score line, …) | DS | A panel of ready-made shapes | partial: geometric primitives only | reimagine with shapes generated in code; never copy their shape artwork | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Line | SS | Straight line tool | partial: the pen draws open paths | replicate | pending |
| Polyline/Bezier pen | both | Click to place points, drag for curves | have: `PenDraft` → `shape.add-path` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Freehand draw and smooth freehand | SS | Draw by hand, with optional smoothing | missing | replicate | pending |
| Arc | SS | Draw an arc segment | missing | replicate | pending |
| Knife | both | Cut a shape apart along a drawn line | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Eraser | SS | Erase part of a closed shape with a brush | missing | replicate | pending |
| Flexishapes | SS (Designer Plus) | Parametric shape presets you can edit | missing | reimagine (parametric shapes) | pending |
| Rhinestone, sketch effect, cross-stitch, puzzle generator | SS (Designer+) | Special pattern generators | missing | reimagine later, low priority | pending |

## Text

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Text object with system fonts | both | Type text in an installed font; edit it later | partial: the Text tool places single-line engraved lettering from a built-in bitmap glyph set (80 characters max); no font support, the text cannot be edited later | replicate with user fonts read through `CRAFT_FONTS_DIR`/system fonts; no fonts bundled | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Vendor/licensed font libraries | both | Fonts from the vendor store or subscription | out of scope | won't do: licensed fonts and assets | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Size, letter spacing, line spacing, alignment | both | Typography controls | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Text on path / curve text | both | Lay text along a path (SS) or bend it into an arc (DS Curve) | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Convert text to path / ungroup to letters | both | Turn letters into editable outlines | partial: placed text is already a path | replicate once real text exists | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Writing fonts (single-line) | both | Single-stroke fonts for pens | partial: the built-in lettering is single-line engrave | replicate (user-supplied single-line fonts) | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Glyphs panel | SS (Designer) | Insert special characters from a font | missing | replicate | pending |

## Editing and node tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Select, multi-select, box/lasso | both | Pick objects; add to the selection with a modifier | have: click/box/lasso, additive multi-select | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Move, rotate, scale, exact size/position fields | both | Transform panel with numeric fields | have: `shape.move/rotate/scale/set-bounds`, `shapes.transform` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Flip/mirror | both | Mirror horizontally/vertically | partial: possible through `shapes.transform` with negative scale; no menu item or button | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Duplicate, cut/copy/paste | both | Copy objects; paste in place | partial: `shapes.duplicate`; no clipboard | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Group/ungroup | both | Treat objects as one unit | missing: the document has layers, but no groups | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Compound path make/release | SS | Join paths into one shape with holes | partial: Pathfinder Merge/Union makes compound shapes; no release | replicate | pending |
| Point editing | both | Move, add and delete points; change smooth/corner points | partial: `BezierOverlay` direct select and convert-point | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Break/join path, close path | SS | Split a path at a point; join two endpoints | missing | replicate | pending |
| Simplify / reduce points | SS | Remove extra points within a tolerance | missing | replicate | pending |
| Warp / distort | SS (Designer), DS | Bend a shape with an envelope or perspective | missing | reimagine, low priority | pending |
| Undo/redo history | both | Multi-step undo | have: journal-based undo/redo | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Lock objects | SS | Stop an object from being selected or moved | missing | replicate | pending |

## Offset

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Outward/inward offset of a shape | both | Make a new outline a set distance outside or inside the shape | partial: the `kerf` crate offsets contours for kerf compensation, but there is no user offset command that adds a new shape | replicate as an engine command on top of `kerf` | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Corner style (round/sharp) | both | Choose rounded or mitred corners for the offset | partial: kerf uses a miter join only | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Weld offsets / per-letter offsets | DS | Merge the offset outlines into one, or keep one per piece | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Internal offset | SS | Offset inward, used for frames and borders | partial (kerf counters) | replicate | pending |

Sources: [Cricut, *How to use the Offset feature*](https://help.cricut.com/hc/en-us/articles/360061650414-How-to-use-the-Offset-feature-in-Design-Space).

## Trace

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Threshold trace of a raster image | both | Choose an area or the whole image; turn dark pixels into cut outlines | have: `trace-image` CLI and the Trace Image tool; PNG/JPEG; adjustable threshold | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| High/low-pass filter, scale and colour-range trace | SS | Image filters and colour selection that guide the trace | missing | replicate | pending |
| Trace outer edge only / trace & detach | SS | Keep only the outline, or split the image along the trace | missing | replicate | pending |
| Background remover / "complexity" upload flow | DS | A guided upload that removes the background, then cleans up with erase tools | missing | reimagine (local, deterministic; no cloud ML) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Live preview of the trace on screen | both | Highlights the area that will be traced | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Full-colour/multi-layer trace | both | Trace each colour into its own layer | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |

Sources: [Cricut, *How to upload images*](https://help.cricut.com/hc/en-us/articles/360009556313-How-to-upload-images-into-Design-Space).

## Weld and pathfinder

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Weld / Unite | both | Merge overlapping shapes into one outline | have: Pathfinder `union`/`merge` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Subtract / Subtract All / Minus Back | both | Remove the top shape(s) from another | have: `difference`, `minus-back` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Intersect | both | Keep only the overlap | have: `intersect` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Exclude / XOR | both | Keep everything except the overlap | have: `xor` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Divide / Slice | both | Split overlapping shapes into separate pieces | have: `divide`, `trim`, `crop`, `outline` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Attach | DS | Keep pieces in their relative positions on the mat without merging them | missing | reimagine (groups that keep positions when sent to the mat) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Flatten (print layers into one printable image) | DS | Merge layers into one Print Then Cut image | missing | reimagine as part of Print & Cut | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Contour (hide parts of a shape) | DS | Hide individual sub-paths so they are not cut | missing | replicate (per-subpath enable) | blocked: sign-in required (2026-11-02, DS 9.83.76) |

## Replicate and array

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Duplicate in a direction (left/right/above/below) | SS | One-click copies next to the original | missing (only duplicate in place, `shapes.duplicate`) | replicate | pending |
| Row/column/grid array | SS | Copies in rows and columns with spacing | missing | replicate | pending |
| Rotated copies (radial array) | SS | N copies rotated around a centre | missing | replicate | pending |
| Fill page | SS | Repeat a shape until the media is full | missing | replicate (pairs with nesting) | pending |
| Mirror copy | SS | A mirrored duplicate | missing | replicate | pending |
| Nesting (auto pack shapes to save material) | SS (Business) | Pack shapes tightly onto the media | missing | replicate | pending |

## Alignment

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Align left/centre/right/top/middle/bottom | both | Align the selection | have: `shapes.align` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Align to page/mat | SS | Align against the media instead of the selection | have: `to_board` | replicate | pending |
| Distribute horizontally/vertically | both | Even spacing | have: `shapes.distribute` | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Centre to page | SS | Put the selection in the middle of the media | have (align to board, both axes) | replicate | pending |
| Spacing by a set value | SS | Exact gap between objects | missing | replicate | pending |

## Layers and objects

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Layers panel: add/remove/show/hide | both | List of layers or objects with visibility toggles | have: `layer.add/remove/set-visible`, Layers & Objects panel | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Reorder (bring forward/send back, drag in list) | both | Change the stacking order | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Lock layer | SS | Stop a layer from being edited | missing | replicate | pending |
| Rename layer/object | both | Give a layer or object a label | partial: layers have names; there is no rename command | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Move object between layers | SS | Drag an object to another layer | missing | replicate | pending |
| Colour Sync | DS | Merge layers of similar colours so fewer mats are used | missing | reimagine (merge by operation/material) | blocked: sign-in required (2026-11-02, DS 9.83.76) |

## Fill and line styles

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Line colour and width | both | Stroke colour, used to group cut operations | partial: a shape is `cut` or `engrave` (`CutStyle`), shown in fixed production colours | reimagine: operation-first, colour as a label | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Line style (dashed, perforated) | SS | Dash or perforation patterns on cut lines | missing | replicate (perforation as a cut setting) | pending |
| Fill colour, gradient, pattern fill | both | Fill shapes for Print & Cut | missing | replicate colour/gradient fill for printing; user-supplied patterns only | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Vendor pattern libraries | both | Store pattern artwork | out of scope | won't do: licensed assets | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Operation type (cut, draw/pen, score, deboss, engrave, wave, perforate, foil, print then cut) | DS | Each layer says what the machine does with it | partial: cut and engrave only | replicate the common ones (cut, pen, score, perforate) | blocked: sign-in required (2026-11-02, DS 9.83.76) |

## Print & Cut and registration marks

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Registration marks on the printed page | both | Printed marks the cutter's sensor uses to find the artwork | partial: `print-cut` writes an SVG page with three marks at a fixed 12 mm margin | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Mark style, length, inset choices | SS | Change the size and position of the marks | missing | replicate | pending |
| Send to printer from the app | both | System print dialog, with bleed and actual-size checks | missing: StencilCraft does not drive a printer | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Bleed | DS | Extend the print colour past the cut line | missing | replicate | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Optical sensor registration (automatic alignment) | both | The machine scans the marks before it cuts | missing (`TB5x` commands not driven) | replicate from protocol facts | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Print & Cut calibration | both | Adjust for sensor/printer offset with a test sheet | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Print size limits | DS | Maximum printable area per mat/page | missing | replicate (validation) | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| PixScan / scan-to-cut | SS | Photograph the material on a special mat and cut to it | missing | reimagine later, low priority | pending |

Sources: [Cricut, *How to Print Then Cut*](https://help.cricut.com/hc/en-us/articles/360009387274-How-to-Print-Then-Cut-in-Design-Space), [Cricut, *How large can I Print Then Cut?*](https://help.cricut.com/hc/en-us/articles/360009429814-Design-Space-says-printable-image-is-too-large), [Silhouette, *Print and Cut troubleshooting*](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000276219-print-and-cut-troubleshooting). See also [`print-cut.md`](print-cut.md).

## Cut settings, materials and tools

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Material presets | both | Choose the material; force, speed and passes fill in from the preset | have: `MEDIA_PRESETS` (27 entries) with a Send-panel chooser | replicate with our own numbers; never import their preset data files | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Force, speed, passes override | both | Change the preset values for this job | partial: force and speed are overridable; passes are not exposed for the cutter (G-code has `--passes`) | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Blade depth / tool type | SS | Pick the tool (ratchet, auto blade, kraft, rotary, pen) and blade depth | partial: `ToolKind` parsed from the device; no chooser in the UI | replicate | pending |
| Custom user materials | both | Save your own material presets | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Per-layer/per-colour settings | SS | Different settings per line colour or layer | missing | replicate | pending |
| Test cut | both | Cut a small test shape first | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Overcut / line segment overcut | SS | Cut a little past corners or ends so pieces lift cleanly | missing | replicate | pending |
| Cut order / path sorting | SS | Cut inner shapes first and keep travel short | missing | replicate | pending |
| Mat-free (carrier sheet free) cutting strategy | SS | Path ordering that lets material be cut without a mat | missing | replicate | pending |
| Weed lines and weed borders | SS (Business) | Extra cut lines that make weeding easier | missing | replicate | pending |
| Laser/G-code profiles | — | Not in these apps; StencilCraft's own | partial | — | — |

## Send panel and machine connection

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Design → Send/Make split | both | A separate production screen before cutting | have: Design/Send split with a dry-run program summary | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Mat preview on the Make screen | DS | Shows how pieces sit on each mat; you can move them there | missing | reimagine | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Machine selection | both | Choose the cutter model | have: model chooser (`Model`, Cameo 3–5, Portrait, Pro) | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| USB connection | both | Send the job over USB | partial: serial (CDC/ACM) transport via CLI `--device`; no USB-bulk transport; the app does not send | replicate (`nusb`) | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Bluetooth connection | both | Wireless send | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Device discovery and status | both | Find connected cutters; show ready/loaded/tool status | partial: status query decoded; no discovery or live status in the UI | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Load/unload media, pause, cancel | both | Machine controls during a job | missing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Multi-cutter support | SS (Business) | Send jobs to several machines at once | missing | replicate later | pending |
| Batch / queued jobs | SS (Business) | Queue several designs or repeats as one job | missing | replicate | pending |
| Cricut machine protocol | DS | Driving Cricut machines | out of scope | won't do: no public specification; would need protocol work we don't do | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Firmware updates | both | Update the machine firmware from the app | out of scope | won't do: vendor firmware | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |

## Library and store

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Local design library | SS | Organise designs on disk in folders | missing (a Recent menu only) | reimagine: plain folders of `.stencilcraft` files | pending |
| Design store / Cricut Access images and projects | both | Buy or subscribe to designs | out of scope | won't do: proprietary cloud store and licensed assets | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Ready-to-make projects | DS | Project templates from the vendor | out of scope | won't do: licensed assets; we may ship original example files | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Templates (mockups like shirts and mugs) | DS | Background images to size designs against | missing | reimagine with original, code-generated templates | blocked: sign-in required (2026-11-02, DS 9.83.76) |

## File import and export formats

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| SVG import | SS (Designer+), DS | Open vector artwork | have: a working subset (see [`gaps.md`](gaps.md) 1) | replicate | pending |
| DXF import | both | Open CAD outlines | missing (DXF export only) | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| PDF import | SS (Designer+) | Open vector PDFs | missing | replicate | pending |
| AI/EPS/CDR import | SS (Business) | Open Illustrator/CorelDRAW files | missing | won't do directly: proprietary formats; users export SVG/PDF | pending |
| Raster import (PNG/JPEG/BMP/GIF/TIFF) | both | Place a photo or clip art to trace or print | partial: PNG/JPEG for tracing only; no placed raster for printing | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Embroidery formats (PES, DST, …) | SS (Designer Plus) | Import embroidery outlines | out of scope | won't do: not a cutting-prep need | pending |
| Native `.studio`/`.studio3` | SS | The vendor's own design format | out of scope | won't do: proprietary format with no public specification; we do not reverse-engineer it | pending |
| SVG export | SS (Business) | Save as SVG | have | replicate | pending |
| PDF/PNG/JPG export | SS (Business) | Save as PDF or image | missing | replicate (PDF first) | pending |
| DXF export | — | StencilCraft's own | have (R14, flattened) | — | — |
| G-code export | — | StencilCraft's own | have | — | — |
| Plugins for external design tools | SS (Silhouette Connect) | Send straight from Illustrator/CorelDRAW to the cutter | missing | reimagine: CLI/MCP and SVG/PDF interchange; never tie the core to proprietary software | pending |
| Cloud-only save | DS | Projects saved to the vendor cloud | out of scope | won't do: account system; we save open, local files | blocked: sign-in required (2026-11-02, DS 9.83.76) |

Sources: [Silhouette, *File Types and Features*](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000282776-file-types-and-features), [Cricut, *Image uploads: unsupported items*](https://help.cricut.com/hc/en-us/articles/360009553213-Image-uploads-unsupported-items).

## Preferences

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Theme / appearance | both | Light or dark interface | have: suite light/dark themes, saved between sessions | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Units and grid spacing | both | Default units; grid size | partial: grid toggle only; mm fixed | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Default material/machine | both | Settings that start each job | partial: the Send panel defaults are fixed in code | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Import defaults (DPI, scale) | SS | How imported files are sized | partial: trace uses 96 dpi by default | replicate | pending |
| Workspace layout | SS | Panel arrangement | partial: dock layout and Reset Workspace | replicate | pending |
| Recent files | both | Reopen recent documents | have: eight-entry Recent menu | replicate | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Language | both | UI language | missing | replicate later | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |

## Account and cloud

| Feature | App | Description | Status | Posture | Observed |
| --- | --- | --- | --- | --- | --- |
| Sign-in / account | DS (required), SS (optional) | A vendor account to use the app or the store | out of scope | won't do: account systems; StencilCraft works offline and needs no account | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Cloud library sync | both | Sync designs across devices | out of scope | won't do: no hosted service; users sync files with their own tools | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Purchased-design management | both | Manage licences for bought designs | out of scope | won't do: proprietary store | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Offline mode | DS | Work without a connection (limited) | have: StencilCraft is offline by design | — | blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Mobile apps | both | Phone and tablet clients | missing | reimagine as the WASM/browser build (M5) | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |
| Machine registration | both | Register a machine to an account | out of scope | won't do: account system | SS: pending. DS: blocked: sign-in required (2026-11-02, DS 9.83.76) |

## Missing items, by area

These are the rows marked **missing** above. Rows marked partial are not listed. The ranked order is in [`gaps.md`](gaps.md).

- **Canvas**: multiple pages/tabs, auto mat layout, guides, snap, measure.
- **Drawing**: freehand, arc, knife, eraser, parametric shapes, pattern generators.
- **Text**: typography controls, text on path / curve text, glyphs panel.
- **Editing**: group/ungroup, break/join/close path, simplify, warp, lock.
- **Offset**: weld offsets / per-letter offsets.
- **Trace**: image filters, outer-edge-only trace, background removal, live preview,
  multi-colour trace.
- **Pathfinder**: attach, flatten, contour.
- **Replicate**: directional duplicate, arrays, radial copies, fill page, mirror copy,
  nesting.
- **Alignment**: spacing by a set value.
- **Layers**: reorder, lock, rename, move between layers, colour sync.
- **Styles**: dashed/perforated lines, fill/gradient for printing, more operation types.
- **Print & Cut**: mark options, printing from the app, bleed, sensor registration,
  calibration, print size limits, scan-to-cut.
- **Cut settings**: custom materials, per-layer settings, test cut, overcut, cut order,
  mat-free strategy, weed lines.
- **Machine**: mat preview, Bluetooth transport, load/unload/pause/cancel controls,
  multi-cutter, batch jobs.
- **Formats**: DXF import, PDF import, PDF/PNG/JPG export, external design-tool plugins.
- **Preferences**: language.
- **Platforms**: browser/mobile client.
