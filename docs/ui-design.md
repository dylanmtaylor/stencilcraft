# UI design: what we borrow from Silhouette Studio & Cricut Design Space

StencilCraft uses a dark canvas, restrained two-accent palette, and hairline
production strokes, while adopting the
*structure* that made the commercial cutter apps usable. We learn that structure
by driving the apps through their normal UI and from their public documentation.
We then build it from our own design. Their icons, artwork, UI text and branding are
never copied (see [`reference-apps.md`](reference-apps.md#clean-room-posture) for the
clean-room posture and the full feature inventory). The structure is borrowed; the skin is ours. Everything colours-wise comes from
`stencilcraft_ui_egui::Tokens`; nothing hard-codes hex inline.

## What these apps get right (and we adopt)

| Borrowed structure | From | StencilCraft realization |
| --- | --- | --- |
| The mat is the home screen | both | Canvas always shows the loaded cutting mat behind the design sheet; the mat is selectable (12×12, 12×24, 24×24 Pro…) and comes from the machine crate's `Mat` table |
| Design → Make workflow | Silhouette Studio's Design/Send flow and Cricut's Make It | Persistent canvas workspace with a clear Make action; Make changes the inspector into the machine/material setup and dry-run review |
| Material drives everything | both | Send tab opens on the media preset chooser (from `MEDIA_PRESETS`); pressure/speed defaults follow the preset, overridable per job |
| "Make It"/Send as distinct mode | Cricut's Make-It screen | Send tab shows the encoded program summary (command count, byte size, cut area) before anything touches hardware |
| Left tool rail | Cricut's vertical tool column | Select, rectangle, delete, undo and redo stay beside the canvas; file operations remain in the top bar |

## Workspace

- Dark, uncluttered chrome; the canvas and physical mat are the main focus.
- A narrow left tool rail, central canvas, and right inspector for mat, layers,
  objects, and cut setup. The inspector becomes a production review pane in Make.
- The initial viewport fits the complete physical mat and board; wheel zoom and
  drag pan remain available.

## StencilCraft visual conventions

- Chrome comes from craft-libs' design system (aiongg/craft-libs `design-system`), resolved by
  `craft_ui::theme` and installed every frame. Dark (the default for new installs; a saved
  `dark_theme` preference wins) uses the suite built-in `craft.pro` dark: charcoal `323232`
  panels on `1E1E1E`, blue accent `378EF0`. Light uses the `stencilcraft.modern` theme file
  (`crates/ui-egui/src/stencilcraft-theme.json`, VectorCraft's Modern Light with the coral
  accent). Panels use `Inspector`, `SectionHeader`, `PropertyRow`, `AlignBar`, `ListRow`;
  dialogs use `craft_ui::dialog::Dialog`; menus use `MenuBar`/`PressMenu` (`docs/menu-map.md`).

- Canvas after the reference cutting-software dark look: mid-grey pasteboard `646464`, the
  mat a light-grey `E0E0E0` rounded rectangle (5 mm rim, 10 mm top rim), a paper-white page
  inside it with a thin `378EF0` blue border (light: canvas `DFE3E9`, mat `F3F5F8`, same
  white page and border). An up arrow in the top rim marks the mat's feed edge. Sober panel styling, no gradients, no comic helpers, no mascot
  buttons.
- Hairline `1.5px`-min production strokes: cut = coral `E8573F` (light:
  `D9472F`), kerf preview = blue `3F88E8` (light: `1473E6`), engrave =
  `58606E` in both (the page is white in both). Selection is suite blue `4F80FF`; coral
  `F06A52` marks what is on/active.
- A single status line at the bottom, terse, machine-readable-ish.
- All actions flow through engine `Command`s / transports — the UI stays thin.
- Selection supports click, left-to-right box/lasso containment, and right-to-left intersecting selection. Current candidates use shape bounds, so curved or concave paths may be conservatively selected; multi-select transforms and modifier-key additive selection remain future work.

## Right-hand panels (Silhouette's panel strip)

Below Properties/Layers the dock holds Silhouette's editing panels as tabs, each a thin
front end over engine commands (`apps/stencilcraft/src/panels.rs`):

- **Transform**: `AlignBar` + Center to Page, rotate by angle, scale by percent (linked),
  move by X/Y.
- **Replicate**: duplicate left/right/above/below, mirror copies, row/column of N copies,
  fill page, rotate copies (count), one shared spacing field. "Above" is screen-above:
  the canvas draws document y downwards, so it maps to the engine's `below`.
- **Offset**: Offset / Internal Offset, distance, corner sharp/round/bevel, corner limit.
  Touching a control shows a blue preview outline on the canvas (computed with
  `stencilcraft_kerf::offset_path`, the same code `shapes.offset` runs); Cancel drops it,
  Apply runs `shapes.offset` and selects the result.
- **Modify**: weld, subtract, subtract all, intersect, crop, divide, exclude, the
  Illustrator Pathfinder extras, make/release compound path, convert to path, outline
  stroke.
- **Point Editing**: double-click a shape to edit its points (double-click empty canvas
  to leave); delete, break, corner, smooth, merge, average, make flat/curve, close, join,
  simplify, convert to path, and a node-count readout.

The options bar shows X/Y/W/H of the selection with a reference-point picker and an
aspect-lock link; the canvas shows W/H while drawing a shape and X/Y/W/H while dragging
one. Hidden shapes are not drawn; hidden and locked shapes are skipped by clicks, boxes
and lassos, and the Layers panel lists every object with eye and lock toggles.

## New Document dialog

File › New… (Ctrl/Cmd+N), File › New from Template… (Ctrl/Cmd+Shift+N, opens on Vinyl) and
starting the app without a file show New Document (`apps/stencilcraft/src/new_doc.rs`). It
follows Illustrator's New Document and the suite's craft-ui `NewDocumentDialog` (same width
and card tokens, Create/Close footer), not Silhouette's starter carousel:

- Tabs: Recent (sizes created before, kept in settings, then a few defaults), Cut Mats (one
  card per machine-crate `Mat`), Vinyl, Paper & Card, Print & Cut, Laser. The kinds follow
  Silhouette's project starters (docs/observations/silhouette-studio.md) with our own names.
- Cards: the page outline at its aspect plus a code-drawn mark (mat grid, fold line, tag
  hole, registration marks, crosshair…), name and size line. Click picks, double-click
  creates, arrows move between cards.
- Preset Details: Name (title bar and Save's default file name), Width, Height, Units
  (mm/in; the document stays mm), Orientation, Cutting Mat, Machine, Kerf. Create is
  disabled while a value is out of range.
- Create runs `document.new` and `kerf.set` through the engine and sets the canvas mat and
  Send machine. If the current document has changed since it was opened, saved, or created,
  a confirmation lets the user cancel and save it before replacing it.

## Mat backdrop behaviour

- The mat paints behind the design sheet, mats at pixel-corner aligned to the
  sheet's top-left origin (media loads left-aligned on Cameo 4 Pro).
- Mat surface: `Tokens::MAT`, coarser inch grid on the mat, and the sheet
  (board) keeps `Tokens::BOARD` on top of it.
- When the sheet exceeds the mat, the mat paints only where it exists; the
  overflow strips outside are canvas background — a real-world situation
  (`12×24` art on a `12×12` mat) must be *visible*, not masked.

## Make workflow contract (v1)

- Model chooser (default `Cameo4Pro`), media preset chooser, mat chooser
  (shared with the canvas backdrop), force/speed override sliders.
- Encode is always dry-run: `machine::encode_cut` on the visible flattened
  shapes; the panel reports commands, bytes, and cut area, plus a preview of
  the first program lines in a scroll area.
- No hardware I/O in the app yet — real sends stay with the CLI
  (`stencilcraft-cli cut --device …`) until the transport story matures.

## Menu bar

The menu structure is in [`menu-map.md`](menu-map.md).

## Later passes (not now)

- Icons for the tool rail (Cricut-style), node editing palette, multi-tab
  page management, in-app BLE/USB device pickers, live machine status chip.
