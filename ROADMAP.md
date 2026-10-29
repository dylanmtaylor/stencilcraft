# ROADMAP

Stage: **M0 — scaffold** (2026-10-29).
Headline: document + kerf + tiling + SVG/DXF/G-code established and tested; desktop shell
edits shapes, previews kerf, saves/opens `.stencilcraft`.

## Dimensions

| Dimension | Stage | Notes |
| --- | --- | --- |
| Document & format | working | Document/Layer/Shape model, `.stencilcraft` JSON v1, roundtrip-tested |
| Import (SVG) | working subset | paths + common basic shapes; groups/transforms/styles; physical units/viewBox; unsupported drawable elements fail closed |
| Kerf compensation | working | polygon offsetting with miter join; convex shapes exact |
| Tiling | working | sheet grid with overlap; shapes not yet clipped at tile edges |
| Exports | working | SVG, DXF R14, G-code: structure renders in browsers/plotter tools (SVG `<svg>` tag bug fixed) |
| Editing tools | none | rectangle add/remove only |
| UI | preliminary | canvas pan/zoom, layers panel, kerf preview, save/open |
| Agent surface | preliminary | CLI `run`/`mcp`, JSON-lines control hooks; command set small |
| Machine control | preliminary | Graphtec/Silhouette encoder + dry-run `cut` CLI (docs/machine.md); no real-hardware transport yet |
| Craft-libs & parity | preliminary | `color` crate added; reusable domain-crate layering like VectorCraft's family; icon/README/examples parity pass done |
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

1. SVG import completeness (M1): advanced CSS selectors/at-rules, external `<use>`,
   text conversion, clip paths/masks and filters; run coverage-guided fuzzing on the
   bounded XML/CSS parser.
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

## Silhouette Studio feature TODO

The scope is to track the full feature comparison, including ecosystem-scale work; this
list is an inventory, not a promise to reproduce proprietary services or assets.
Completed items should be marked here only when behavior is usable end-to-end.

- [x] Engine commands for moving, rotating and scaling a shape around its center, with
  finite-geometry validation and undo/redo. UI selection/editing remains.
- [ ] Add direct selection, box/lasso selection, pen and node editing; continue with
  welding and path boolean operations.
- [ ] Add image import and trace-to-path workflow, including clear trace controls and
  geometry limits.
- [ ] Add text objects and text-to-path conversion without bundling fonts; define a user
  font/shaping integration point.
- [ ] Expand SVG fidelity: advanced CSS/at-rules, external references, clipping, masks,
  filters, and additional vector formats.
- [ ] Implement Print & Cut: registration mark layout, printer output, sensor-aware cut
  alignment/calibration and end-to-end fixtures.
- [ ] Add production tools: weed lines, nesting, multi-cutter job assignment, batch and
  repeated-pass workflows.
- [ ] Expand design formats and project interoperability, including DXF import and a
  documented strategy for proprietary Studio3 files (do not reverse-engineer protected
  formats without an appropriate public specification).
- [ ] Grow machine support: USB bulk, Bluetooth, more model generations, device discovery,
  live status, registration-mark protocol, and mat-free knife path ordering.
- [ ] Add cloud/account features: sign-in, library sync, purchased-design management and
  offline cache; decide on a compatible service and data/rights model before implementation.
- [ ] Add a design marketplace or catalog only with original or permissively licensed
  assets, clear ownership/licensing, and a sustainable distribution model.
- [ ] Add mobile and browser clients with shared editing/sending workflows and cloud sync.
- [ ] Add optional integrations for external design tools (e.g. Illustrator/CorelDRAW-like
  interchange or plugins) without coupling the core to proprietary software.
- [ ] Improve export/production parity: PDF/print sheets, design previews, automatic
  nesting, registration marks and multi-machine output.

## Progress log

- 2026-10-29 — M0 scaffold: workspace (11 crates, 2 apps, xtask), document model v1,
  kerf offsetting, sheet tiling, SVG/DXF/G-code exporters, desktop shell with kerf
  preview, CLI `run`/`mcp`, `cargo xtask` quality gates.
- 2026-10-29 — machine control (12th crate): Graphtec/Silhouette ASCII encoder with
  Cameo-4 preamble, media/mat/model tables,`RecordingTransport` dry-run + optional
  serial transport, CLI `cut` (transcript by default, `--device` real send), protocol
  reference in `docs/machine.md` (clean-room from GPL community drivers; facts only).
- 2026-10-29 — M1 SVG import: roxmltree-based XML reader for path + rect (including
  rounded corners) + circle/ellipse/line/polyline/polygon, nested groups, standard
  affine transforms, inherited data-kind/class and inline display/visibility, physical
  SVG lengths (`px/mm/cm/in/pt/pc`), viewBox and preserveAspectRatio. Unsupported drawable
  elements fail closed; 16 MiB/100k node/128 depth/±1e9 coordinate limits. Added CLI
  `import-svg --in ... --out ...`, native SVG export→import→export e2e, 12 SVG tests;
  `cargo xtask ci` green.
- 2026-10-29 — local SVG `<use>` import: fragment-only references to supported
  shapes/groups, including use-site x/y and transforms, inherited production styles,
  duplicate-ID and cycle rejection; malformed/external/unsupported references fail calmly.
- 2026-10-29 — engine editing baseline: serde-tagged move/rotate/scale commands for
  individual shapes, center-pivot transforms, finite-coordinate validation and undo/redo.
- 2026-10-29 — SVG CSS subset: embedded stylesheet rules for type/class/ID/attribute
  selectors, descendant and child combinators, selector lists, source order and
  `!important`; limited to supported production/presentation properties with 1 MiB and
  10,000-rule limits.
- 2026-10-29 — SVG `<symbol>` templates: local `<use>` instancing with `viewBox`,
  `preserveAspectRatio`, use-site x/y/width/height and style inheritance; bounded
  instance expansion and clear errors for invalid viewports and cycles.
- 2026-10-29 — parity pass with VectorCraft conventions: original app icon
  (`assets/app-icon/stencilcraft.svg`, Crafting Apps tile template) with ASSETS/NOTICE
  attribution; README restyled to the ArtCraft README structure with PNG previews
  rendered from our own exporter; committed `examples/` (starter, kerf counter,
  tiling); new `stencilcraft-color` craft-lib (CSS parse/format, luminance, blend);
  CLI end-to-end integration tests (9) and a malformed-input no-panic corpus (5) —
  110 workspace tests; fixed the SVG export `<svg>` unterminated-tag bug found by
  the preview renders.
