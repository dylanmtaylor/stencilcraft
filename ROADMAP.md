# ROADMAP

Stage: **M0 — scaffold** (2026-10-29).
Headline: document + kerf + tiling + SVG/DXF/G-code established and tested; desktop shell
edits shapes, previews kerf, saves/opens `.stencilcraft`.

## Dimensions

| Dimension | Stage | Notes |
| --- | --- | --- |
| Document & format | working | Document/Layer/Shape model, `.stencilcraft` JSON v1, roundtrip-tested |
| Import (SVG) | preliminary | `<path>`-only, no transforms/groups/styles |
| Kerf compensation | working | polygon offsetting with miter join; convex shapes exact |
| Tiling | working | sheet grid with overlap; shapes not yet clipped at tile edges |
| Exports | preliminary | SVG, DXF R14, G-code: straight-line structure works |
| Editing tools | none | rectangle add/remove only |
| UI | preliminary | canvas pan/zoom, layers panel, kerf preview, save/open |
| Agent surface | preliminary | CLI `run`/`mcp`, JSON-lines control hooks ✅ command set small |
| Machine control | preliminary | Graphtec/Silhouette encoder + dry-run `cut` CLI (docs/machine.md); no real-hardware transport yet |
| Platforms | preliminary | desktop native OK; wasm planned |

## Milestones

- **M0 (this): scaffold.** Workspace, conventions, quality gates, format v1, working CLI
  round trip: load → kerf → tile → export.
- **M1: real import.** SVG import through transforms/groups/styles; first property tests
  of round trips; `.stencilcraft` v1 frozen.
- **M2: kerf 2.0.** Self-intersection-robust offsetting (sweep-based), per-layer and
  per-object kerf overrides, inlet/pressure presets by material, kerf preview overlay.
- **M3: cutting output.** Digital-bridge generation so counters stay attached, tile-edge
  clipping with overlap merge, multiply-aligned SVG/PDF sheets, machine-specific G-code
  profiles (Grbl, Marlin/Creality, diode-laser).
- **M4: editing tools.** Pen, direct selection, box/lasso select, node editing; SVG path
  tools on par with the import set; undo structural sharing.
- **M5: web.** Same UI in the browser via WASM; file open/export parity.

## What's next (ranked)

1. SVG import completeness (M1): groups, transforms, `style`/attributes, `rect`/`circle`/
   `ellipse`/`polyline` elements; fuzz the parser (untrusted input).
2. Property tests: kerf invariants (area monotonic in kerf width on convex shapes),
   tiling coverage (every point of the board interior lies within `overlap/2` of a tile
   cut path), format fuzz.
3. Kerf: counter detection hardened to proper nesting (every subpath tested against every
   other, not bbox equal); miter-limit fallback to bevel on spikes.
4. Bridge generation prototype (M3 early): algorithm notes in `docs/bridges.md`.
5. G-code profiles: material presets, first/last pass, power ramp.
6. Machine control (docs/machine.md): USB-bulk `nusb` transport for the `0b4d` device
   family, Bluetooth RFCOMM/BLE, registration marks (print-and-cut), mat-free path
   sorting for knife-pressure relief.

## Progress log

- 2026-10-29 — M0 scaffold: workspace (11 crates, 2 apps, xtask), document model v1,
  kerf offsetting, sheet tiling, SVG/DXF/G-code exporters, desktop shell with kerf
  preview, CLI `run`/`mcp`, `cargo xtask` quality gates.
- 2026-10-29 — machine control (12th crate): Graphtec/Silhouette ASCII encoder with
  Cameo-4 preamble, media/mat/model tables,`RecordingTransport` dry-run + optional
  serial transport, CLI `cut` (transcript by default, `--device` real send), protocol
  reference in `docs/machine.md` (clean-room from GPL community drivers; facts only).
