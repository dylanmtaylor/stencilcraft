# UI design: what we borrow from Silhouette Studio & Cricut Design Space

StencilCraft uses a dark canvas, restrained two-accent palette, and hairline
production strokes, while adopting the
*structure* that made the commercial cutter apps usable. Structure is
copied; skin is ours. Everything colours-wise comes from
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

- Dark gray canvas (`CANVAS_BG 28`), sober panel styling, no gradients, no
  comic helpers, no mascot buttons.
- Hairline `1.5px`-min production strokes: cut = red `E8573F`, kerf preview =
  blue `3F88E8`, engrave = gray 140.
- A single status line at the bottom, terse, machine-readable-ish.
- All actions flow through engine `Command`s / transports — the UI stays thin.
- Selection supports click, left-to-right box/lasso containment, and right-to-left intersecting selection. Current candidates use shape bounds, so curved or concave paths may be conservatively selected; multi-select transforms and modifier-key additive selection remain future work.

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

## Later passes (not now)

- Icons for the tool rail (Cricut-style), node editing palette, multi-tab
  page management, in-app BLE/USB device pickers, live machine status chip.
