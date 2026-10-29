# UI design: what we borrow from Silhouette Studio & Cricut Design Space

StencilCraft's apps keep the VectorCraft language — dark canvas, restrained
two-accent palette, hairline production strokes — while adopting the
*structure* that made the commercial cutter apps usable. Structure is
copied; skin is ours. Everything colours-wise comes from
`stencilcraft_ui_egui::Tokens`; nothing hard-codes hex inline.

## What these apps get right (and we adopt)

| Borrowed structure | From | StencilCraft realization |
| --- | --- | --- |
| The mat is the home screen | both | Canvas always shows the loaded cutting mat behind the design sheet; the mat is selectable (12×12, 12×24, 24×24 Pro…) and comes from the machine crate's `Mat` table |
| Design ↔ Send tabs | Silhouette Studio's top-right **Send** tab | A right panel with **Design** and **Send** tabs. Design = layers/kerf/shape list as today; Send = production step (material, tool, force/speed, dry-run summary) |
| Material drives everything | both | Send tab opens on the media preset chooser (from `MEDIA_PRESETS`); pressure/speed defaults follow the preset, overridable per job |
| "Make It"/Send as distinct mode | Cricut's Make-It screen | Send tab shows the encoded program summary (command count, byte size, cut area) before anything touches hardware |
| Left tool rail | Cricut's vertical tool column | New/shape/edit actions consolidated: file ops stay in the top bar ( ours), shape tools get a rail section (later pass) |

## What we keep (VectorCraft identity)

- Dark gray canvas (`CANVAS_BG 28`), sober panel styling, no gradients, no
  comic helpers, no mascot buttons.
- Hairline `1.5px`-min production strokes: cut = red `E8573F`, kerf preview =
  blue `3F88E8`, engrave = gray 140.
- A single status line at the bottom, terse, machine-readable-ish.
- All actions flow through engine `Command`s / transports — the UI stays thin.

## Mat backdrop behaviour

- The mat paints behind the design sheet, mats at pixel-corner aligned to the
  sheet's top-left origin (media loads left-aligned on Cameo 4 Pro).
- Mat surface: `Tokens::MAT`, coarser inch grid on the mat, and the sheet
  (board) keeps `Tokens::BOARD` on top of it.
- When the sheet exceeds the mat, the mat paints only where it exists; the
  overflow strips outside are canvas background — a real-world situation
  (`12×24` art on a `12×12` mat) must be *visible*, not masked.

## Send tab contract (v1)

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
