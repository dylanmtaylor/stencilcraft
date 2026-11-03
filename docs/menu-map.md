# Menu map

This is the design for StencilCraft's menu bar. The aim is that a Silhouette Studio user
finds things where they expect them, with the depth of VectorCraft or Adobe Illustrator
for anything useful in cutting and stencil work.

- **Order and names**: File, Edit, View, Panels and Object keep Silhouette Studio's names
  and item order where the item exists there (observed in SS 5.0.539; see
  [`reference-apps.md`](reference-apps.md)). Illustrator/VectorCraft-grade items are added
  after the Silhouette items in the same menu, or in the extra menus (Type, Select, Effect,
  Machine).
- **Shortcuts**: Silhouette's shortcut wins where it has one. Otherwise we use
  Illustrator's, unless it clashes with a Silhouette one; then we pick a free key and mark
  it *ours*. Ctrl means Cmd on macOS.
- **Source**:
  - **SS**: Silhouette Studio.
  - **DS**: Cricut Design Space.
  - **AI**: Adobe Illustrator conventions.
  - **VC**: VectorCraft.
  - **ours**: StencilCraft-specific.
- **Status**: **have** means the item is in today's desktop menu or tool rail and works;
  **missing** means it is not built yet; **partial** says what part exists.
- **Implementation**: the engine command (`crates/engine/src/lib.rs`) behind the item.
  *proposed `name`* is a suggested name for a command that does not exist yet. "app" means
  file or view handling in the desktop shell (`apps/stencilcraft/src/main.rs`) that is not
  a document edit.

Top level: **File · Edit · Object · Type · Select · Effect · View · Panels · Machine · Help**.
Panels is shown as "Window" on macOS, as the platform expects.

## File

Silhouette users look here for New, Open, Merge, Recent, Save/Save As, Library, Print,
Send and Close Tab. Saving to the vendor library and cloud is replaced by local files.

| Item | Shortcut | Source | Status | Implementation |
| --- | --- | --- | --- | --- |
| New… (New Document dialog) | Ctrl+N | SS, AI | have | app dialog → `document.new` + `kerf.set` |
| New from Template… (board presets, mats) | Ctrl+Shift+N | SS (New Project Wizard) | partial (opens New Document on its first template tab; no starter geometry) | app dialog → `document.new` + `kerf.set` |
| Open… | Ctrl+O | SS | have | app: `stencilcraft-doc` read |
| Open Recent › | — | SS | have | app (persisted Recent list) |
| Open Recovered… | — | SS | missing | app: autosave store (new) |
| Merge… (import into current document) | Ctrl+Shift+O | SS | have (as Import SVG/Image) | `shape.add-paths` |
| Import › SVG, Image (trace), DXF, PDF, AI/EPS, embroidery | — | SS/DS/AI | partial: SVG and image have; others missing | `shape.add-paths` after a format reader |
| Place Image (printable raster) | Ctrl+Shift+Alt+P (*ours*; AI's Ctrl+Shift+P is Show Print Border here) | AI (Place) | missing | *proposed `image.place`* |
| Save | Ctrl+S | SS | have | app: `stencilcraft-doc` write |
| Save As… | Ctrl+Shift+S | SS | have | app |
| Save Selection As… | — | SS | missing | app: write selected shapes |
| Export › SVG, Print & Cut SVG, DXF, G-code, Tiled SVG | — | ours/SS Business | have | app: `stencilcraft-svg/dxf/gcode/tiling` |
| Export › PDF, PNG, JPEG | — | SS Business | missing | app: new exporters |
| Library › (open a local design folder) | — | SS | missing | app |
| Scan… (scan-to-cut) | — | SS (PixScan) | missing | app |
| Print… | Ctrl+P | SS | missing | app: system print dialog |
| Print Page Setup… | — | SS | missing | app |
| Send to Machine… | Ctrl+L | SS | have (Send tab) | app: Send panel → `stencilcraft-machine` |
| Document Setup / Boards… | — (AI's Ctrl+Alt+P is PixScan here) | AI | missing | `document.set-board` (have) + *proposed `board.add`* |
| Close Tab | Ctrl+W | SS | missing | app (tabs) |
| Quit | Ctrl+Q | SS | have (window close) | app |

## Edit

Silhouette users look here for Undo/Redo, clipboard with Paste in Front, Duplicate,
Delete, Select All / Deselect All, Select by Color and Preferences.

| Item | Shortcut | Source | Status | Implementation |
| --- | --- | --- | --- | --- |
| Undo | Ctrl+Z | SS | have | `edit.undo` |
| Redo | Ctrl+Shift+Z | SS | have | `edit.redo` |
| Cut | Ctrl+X | SS | missing | app clipboard + `shapes.remove` |
| Copy | Ctrl+C | SS | missing | app clipboard |
| Paste | Ctrl+V | SS | missing | `shape.add-paths` |
| Paste in Front | Ctrl+F | SS/AI | missing | *proposed `shapes.paste` {in_place, front}* |
| Paste in Back | Ctrl+B | AI | missing | *proposed `shapes.paste` {in_place, back}* |
| Paste in Place | Ctrl+Shift+V | AI | missing | *proposed `shapes.paste` {in_place}* |
| Delete | Delete / Backspace | SS | have | `shapes.remove` |
| Duplicate | Ctrl+D | SS | have | `shapes.duplicate` |
| Select All | Ctrl+A | SS | have | app selection |
| Deselect All | Ctrl+Shift+A (also Esc) | SS | have (Esc today) | app selection |
| Select by Color… | — | SS | missing | app selection (see Select) |
| History… | — | DS | missing | app view of the edit journal |
| Preferences… | Ctrl+K | SS | partial (theme only, in Window) | app: `craft-settings` |

## Object

Silhouette users look here for Mirror, Arrange, Align, Rotate, Replicate, Modify (weld and
the rest), Group/Ungroup, Convert to Path, compound paths and Simplify. We add the full
Illustrator Pathfinder, Transform Each, Transform Again, Lock/Hide, clipping masks,
Expand and Offset Path.

| Item | Shortcut | Source | Status | Implementation |
| --- | --- | --- | --- | --- |
| Mirror › Horizontal, Vertical | — | SS | missing | `shapes.transform` (scale −1) |
| Arrange › Bring to Front | Ctrl+Shift+] | SS | missing | *proposed `shapes.arrange`* |
| Arrange › Bring Forward | Ctrl+] | SS | missing | *proposed `shapes.arrange`* |
| Arrange › Send Backward | Ctrl+[ | SS | missing | *proposed `shapes.arrange`* |
| Arrange › Send to Back | Ctrl+Shift+[ | SS | missing | *proposed `shapes.arrange`* |
| Arrange › Send to Layer… | — | AI | missing | *proposed `shapes.move-to-layer`* |
| Align › Left, Center, Right, Top, Middle, Bottom | — | SS | have | `shapes.align` |
| Align › Center to Page | — | SS | have | `shapes.align` {to_board} |
| Align › To Key Object | — | AI | missing | `shapes.align` + *proposed key-object field* |
| Align › Space Horizontally / Vertically | — | SS | have | `shapes.distribute` |
| Align › Distribute Spacing by Value… | — | AI | missing | *proposed `shapes.distribute` {gap_mm}* |
| Rotate › 15° steps, 90°, 180°, custom… | — | SS | have (custom angle in the Transform panel) | `shapes.transform` {degrees} |
| Replicate › Duplicate left/right/above/below, rows, columns, fill page, mirror, rotate copies | — | SS | have (menu + Replicate panel) | `shapes.replicate-side/-array/-fill`, `shapes.mirror-copy`, `shapes.rotate-copies` |
| Transform › Move…, Scale…, Rotate…, Shear… | — | AI | partial: move/scale/rotate in the Transform panel; shear missing | `shapes.transform`; shear *proposed* |
| Transform Again | Ctrl+Shift+D (*ours*; AI's Ctrl+D is Duplicate here) | AI | missing | app: replay the last `shapes.transform` |
| Transform Each… | Ctrl+Alt+Shift+D | AI | missing | *proposed `shapes.transform-each`* |
| Modify › Weld | Ctrl+Shift+W | SS | have | `shape.pathfinder` union |
| Modify › Subtract | — | SS | have | `shape.pathfinder` difference |
| Modify › Subtract All | — | SS | have | `shape.pathfinder` difference over all |
| Modify › Intersect | — | SS | have | `shape.pathfinder` intersect |
| Modify › Exclude | — | AI/DS | have | `shape.pathfinder` xor |
| Modify › Divide | — | SS | have | `shape.pathfinder` divide |
| Modify › Trim | — | AI | have | `shape.pathfinder` trim |
| Modify › Merge | — | AI | have | `shape.pathfinder` merge |
| Modify › Crop | — | SS | have | `shape.pathfinder` crop |
| Modify › Outline | — | AI | have | `shape.pathfinder` outline |
| Modify › Minus Back | — | AI | have | `shape.pathfinder` minus-back |
| Modify › Detach Lines | — | SS | missing | *proposed `shapes.detach-lines`* |
| Group | Ctrl+G | SS | missing | *proposed `shapes.group`* |
| Ungroup | Ctrl+Shift+G | SS | missing | *proposed `shapes.ungroup`* |
| Attach / Detach (keep positions on the mat) | — | DS | missing | *proposed `shapes.attach`* |
| Lock | Ctrl+2 | AI | have | `shapes.set-locked` |
| Unlock All | Ctrl+Alt+2 | AI | have | `shapes.set-locked` |
| Hide | Ctrl+3 | AI | have | `shapes.set-hidden` |
| Show All | Ctrl+Alt+3 | AI | have | `shapes.set-hidden` |
| Convert to Path | — | SS | have | `shapes.to-path` |
| Make Compound Path | Ctrl+E | SS | have | `shapes.make-compound` |
| Release Compound Path | Ctrl+Shift+E | SS | have | `shapes.release-compound` |
| Clipping Mask › Make | Ctrl+7 | AI | missing | *proposed `shapes.clip`* |
| Clipping Mask › Release | Ctrl+Alt+7 | AI | missing | *proposed `shapes.unclip`* |
| Simplify | Ctrl+J | SS | have (tolerance in Point Editing) | `path.simplify` |
| Path › Join | Ctrl+Shift+J (*ours*; AI's Ctrl+J is Simplify here) | AI | have | `path.join` |
| Path › Average | Ctrl+Alt+J | AI | have | `path.average-nodes` |
| Path › Break at Point | — | SS (Point Editing) | have | `path.break` |
| Path › Close Path | — | AI | have | `path.close` |
| Path › Offset Path… | — | AI / SS Offset | have (Offset panel) | `shapes.offset` |
| Path › Outline Stroke | — | AI | have (width in the Modify panel) | `path.outline-stroke` |
| Expand | — | AI | missing | *proposed `shapes.expand`* |
| Contour (hide sub-paths) | — | DS | missing | *proposed `shape.set-subpath-enabled`* |
| Flatten (for Print Then Cut) | — | DS | missing | *proposed `shapes.flatten`* |
| Isolation Mode (enter / exit group) | double-click / Esc | AI | missing | app |
| Boards › New Board, Fit Board to Artwork | — | AI (Artboards) | partial: one board, resize | `document.set-board` + *proposed `board.add`* |

## Type

Silhouette keeps text controls in the Text Style panel and the text options bar. This menu
brings them together as Illustrator does.

| Item | Shortcut | Source | Status | Implementation |
| --- | --- | --- | --- | --- |
| Font › (system and user fonts) | — | SS/DS/AI | missing | *proposed `text.set-font`* |
| Size › | — | SS/DS | missing | *proposed `text.set-style`* |
| Character panel (letter spacing, kerning, line spacing) | Ctrl+T | AI / SS Text Style | missing | *proposed `text.set-style`* |
| Paragraph panel (alignment, justify, direction) | Ctrl+Alt+T | AI / SS Text Style | missing | *proposed `text.set-style`* |
| Glyphs | Shift+Alt+F11 | AI / SS Designer | missing | app panel → text insert |
| Type on a Path | — | SS/AI | missing | *proposed `text.on-path`* |
| Curve Text… | — | DS | missing | *proposed `text.on-path` {arc}* |
| Create Outlines | Ctrl+Alt+Shift+O (*ours*; AI's Ctrl+Shift+O is Merge here) | AI / SS Convert to Path | partial (placed text is outlines already) | *proposed `text.outline`* |
| Ungroup to Letters / Lines | — | DS | missing | *proposed `text.split`* |
| Find Font… | — | AI | missing | app |
| Spelling… | F7 | SS | missing | app |
| Place lettering | T (tool) | ours | have: editable text with installed-family and size picker | `text.add` |
| Edit Selected Text… | — | ours | have: text, family and size | `text.edit` |

## Select

Silhouette users get Select All, Deselect and Select by Color (Edit menu and its panel).
We add the Illustrator Select menu.

| Item | Shortcut | Source | Status | Implementation |
| --- | --- | --- | --- | --- |
| All | Ctrl+A | SS | have | app selection |
| Deselect | Ctrl+Shift+A | SS | have (Esc) | app selection |
| Inverse | — | AI | have | `select.inverse` |
| Select by Color… | — | SS | missing | app selection |
| Same › Fill Color, Stroke Color, Stroke Weight | — | AI | partial: fill and stroke | `select.same-fill`, `select.same-stroke` |
| Same › Operation (cut, engrave, score…) | — | ours | missing | app selection |
| Same › Layer | — | AI | missing | app selection |
| Next Object Above / Below | Ctrl+Alt+] / Ctrl+Alt+[ | AI | missing | app selection |
| Save Selection… / Edit Selection… | — | AI | missing | app |

## Effect

Silhouette users look for the Offset, Trace, Sketch, Stipple, Pop up, Rhinestone and
Image Effects panels. Here they are actions that open the matching panel.

| Item | Shortcut | Source | Status | Implementation |
| --- | --- | --- | --- | --- |
| Offset… / Internal Offset… | — | SS | have (Offset panel with live preview) | `shapes.offset` |
| Kerf Compensation… | — | ours | have | `kerf.set` |
| Bridges… (keep counters attached) | — | ours | missing | *proposed `shapes.bridge`* |
| Weed Lines and Border… | — | SS Business | missing | *proposed `shapes.weed-lines`* |
| Trace Image… | — | SS/DS | have | app: trace → `shape.add-paths` |
| Remove Background… | — | DS | missing | app: image pipeline |
| Image Effects… | — | SS | missing | app: image pipeline |
| Sketch Fill… | — | SS | missing | *proposed `shapes.hatch-fill`* |
| Stipple… | — | SS | missing | *proposed `shapes.stipple`* |
| Emboss / Deboss Patterns… | — | SS Specialty | missing | *proposed `shapes.hatch-fill`* |
| Rhinestone… | — | SS Designer | missing | *proposed `shapes.rhinestone`* |
| Warp › Arc, Flag, Bulge, Conical, Perspective… | — | SS/DS/AI | missing | *proposed `shapes.warp`* |
| Distort › Shear, Free Distort | — | SS/AI | missing | *proposed `shapes.warp`* |
| Lattice… | — | SS Designer | missing | *proposed `shapes.lattice`* |
| Pop-up Card… | — | SS | missing | *proposed `shapes.popup`* |
| Puzzle… | — | SS Designer Plus | missing | *proposed `shapes.puzzle`* |
| Apply Last Effect | Ctrl+Shift+Alt+E (*ours*; AI's Ctrl+Shift+E is Release Compound here) | AI | missing | app: replay the last effect command |

## View

Silhouette users look here for zoom, Rotate Page, grid options and snapping, Smart
Snapping, Print Border, Cut Border, Registration Marks, Design Page Settings and switching
between Design and Send. We add guides, smart guides, outline view and point snapping.

| Item | Shortcut | Source | Status | Implementation |
| --- | --- | --- | --- | --- |
| Zoom › In, Out | Ctrl++ / Ctrl+− | SS | have | app view |
| Zoom › Fit to Page | F | SS/ours | have | app view |
| Zoom › Center Page | C | ours | have | app view |
| Zoom › Actual Size | Ctrl+1 | AI | have | app view |
| Zoom › Selection | Ctrl+Alt+0 (*ours*) | SS | missing | app view |
| Rotate Page › 0/90/180/270 | — | SS | missing | app view |
| Grid Options… | Ctrl+F4 | SS | missing | app view (prefs) |
| Show Grid | Ctrl+' | SS/AI | have | app view |
| Snap to Grid | Ctrl+Shift+' | SS/AI | missing | app snapping |
| Enable Smart Snapping (objects) | — | SS | missing | app snapping |
| Smart Guides | Ctrl+U | AI/DS | missing | app snapping |
| Snap to Point | Ctrl+Alt+' | AI | missing | app snapping |
| Rulers | Ctrl+Shift+R | ours | have | app view |
| Guides › Show | Ctrl+; | AI | missing | app view |
| Guides › Lock | Ctrl+Alt+; | AI | missing | app view |
| Guides › Make | Ctrl+5 | AI | missing | *proposed `guides.make`* |
| Guides › Release | Ctrl+Alt+5 | AI | missing | *proposed `guides.release`* |
| Outline View | Ctrl+Y | AI | missing | app view |
| Show Print Border | Ctrl+Shift+P | SS | missing | app view |
| Show Cut Border | Ctrl+Shift+L | SS | missing | app view |
| Show Dimensions | — | SS | missing | app view |
| Preview Kerf Compensation | — | ours | have | app view (cached kerf) |
| Print & Cut Marks | Ctrl+F3 | SS (Registration Marks) | have | app view |
| Design Page Settings… | Ctrl+F1 | SS | missing | app |
| Show Design / Send | Ctrl+Alt+D / Ctrl+F2 | SS | have (tabs; no shortcut yet) | app |

## Panels

Silhouette's Panels menu in its own order, then the Illustrator/VectorCraft panels.
Silhouette users expect every right-column panel to be listed here.

| Item | Shortcut | Source | Status | Implementation |
| --- | --- | --- | --- | --- |
| Page Setup | — | SS | partial (board size in Properties) | `document.set-board` |
| PixScan | Ctrl+Alt+P | SS | missing | app |
| Fill Color | — | SS | missing | *proposed `shapes.set-fill`* |
| Line Style | — | SS | partial (cut/engrave) | `shapes.set-style` |
| Select by Color | — | SS | missing | app selection |
| Trace | — | SS | partial (Trace Image tool) | app trace |
| Image Effects | — | SS | missing | app |
| Text Style | — | SS | missing | *proposed `text.set-style`* |
| Spelling | F7 | SS | missing | app |
| Transform | — | SS | have (align, rotate, scale, move by value) | `shapes.transform`, `shapes.align`, `shapes.distribute` |
| Replicate | — | SS | have | `shapes.replicate-*`, `shapes.mirror-copy`, `shapes.rotate-copies` |
| Modify | — | SS | have (weld family, Pathfinder, compound paths, outline stroke) | `shape.pathfinder`, `shapes.make-compound`, `path.outline-stroke` |
| Offset | — | SS | have (distance, internal, corner, limit, preview) | `shapes.offset` |
| Pop up | — | SS | missing | *proposed `shapes.popup`* |
| Stipple / Sketch | — | SS | missing | *proposed `shapes.hatch-fill`* |
| Print & Cut (Registration Marks) | Ctrl+F3 | SS | partial (marks toggle, export) | app: `print-cut` |
| Layers | — | SS Designer/DS/AI | partial (per-object eye and lock) | `layer.add/remove/set-visible`, `shapes.set-hidden/-locked` |
| Point Editing | — | SS | have (double-click to edit; delete, break, corner/smooth, flat/curve, simplify, node count) | `path.*`, `shape.set-path`, `query.node-count` |
| Pathfinder | — | AI/VC | have (Object › Pathfinder) | `shape.pathfinder` |
| Align | — | AI/VC | have (Object › Align) | `shapes.align` |
| Swatches | — | AI | missing | app + *proposed `shapes.set-fill`* |
| Recolor Artwork | — | AI | missing | *proposed `shapes.recolor`* |
| Graphic Styles (saved line/fill/operation sets) | — | AI | missing | *proposed `shapes.apply-style`* |
| Character / Paragraph / Glyphs | Ctrl+T / Ctrl+Alt+T / Shift+Alt+F11 | AI | missing | see Type |
| Actions / Batch | — | AI | missing | app: recorded command lists replayed through `engine.run` (MCP covers scripting today) |
| Libraries (local design folders) | — | SS/VC | missing | app |
| Reset Panels to Default | — | ours | have | app |
| Starter Mode (simplified workspace) | — | SS | missing | app: workspace preset |
| Theme › Dark, Light | — | ours | have | app |

## Machine

Silhouette puts this in the Send tab and File › Send to Silhouette; Cricut puts it on the
Make screen. Users look here for connecting, materials, test cuts and the cut itself.

| Item | Shortcut | Source | Status | Implementation |
| --- | --- | --- | --- | --- |
| Send… (open the Send tab) | Ctrl+L | SS | have | app Send panel |
| Connect / Rescan Devices | — | SS | missing | `stencilcraft-machine` transports (USB bulk, Bluetooth) |
| Machine › model, firmware, status | — | SS | partial (model chooser) | `stencilcraft-machine` `Model`, status query |
| Send Mode › Standard / By Line Color / By Fill Color / By Layer | — | SS | missing | app Send panel |
| Tool Holders › assign lines to Tool 1 / Tool 2 / ignore | — | SS | missing | app Send panel |
| Material Settings… (custom materials, force, speed, passes, acceleration, blade depth, overcut) | — | SS/DS | partial (preset, force, speed) | `stencilcraft-machine` `MEDIA_PRESETS`, `CutSettings` |
| Mat Preview / Arrange on Mat | — | DS | missing | app |
| Mirror on Mat | — | DS | missing | app (transform at send time) |
| Load Type › On Mat / Without Mat / Roll | — | DS/SS | partial (`Mat::None`) | `stencilcraft-machine` |
| Test Cut | — | SS/DS | missing | app → encoder |
| Dry Run (command transcript) | — | ours | have | `stencilcraft-machine` `RecordingTransport` |
| Cut / Pause / Cancel | — | SS/DS | missing in app (CLI `cut --device`) | `stencilcraft-machine` `SerialTransport` |
| Load / Unload Media | — | SS/DS | missing | `stencilcraft-machine` |
| Register Print & Cut Marks (sensor scan) | — | SS/DS | missing | `stencilcraft-machine` (`TB5x`) |
| Calibration… | — | SS/DS | missing | app + `stencilcraft-machine` |
| Export G-code for laser… | — | ours | have | app: `stencilcraft-gcode` |
| Multi-Cutter Jobs… | — | SS Business | missing | app |

## Help

| Item | Shortcut | Source | Status | Implementation |
| --- | --- | --- | --- | --- |
| Documentation | F1 | SS (User's Manual) | have (no shortcut yet) | app |
| Keyboard Shortcuts | — | ours | have | app |
| Tutorials / first-run guide | — | SS | missing | app |
| About StencilCraft | — | ours | have | app |
