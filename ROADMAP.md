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
| Editing tools | working foundation | VectorCraft-backed ellipse, polygon, star, transforms, Pathfinder, alignment/distribution and journal-based undo/redo; StencilCraft retains its millimetre document metadata |
| UI | working | Craft-suite desktop shell (`TitleBar`, `MenuBar`, `OptionsBar`, `ToolRail`, `Rulers`, `BezierOverlay`, `PenDraft`), canvas pan/zoom with Fit (F) and Center (C), layers & objects panel, click/box/lasso multi-select, Pathfinder, cached compensated kerf preview, persisted settings |
| Agent surface | preliminary | CLI `run`/MCP tools and legacy JSON-lines control; desktop session is separate |
| Machine control | preliminary | Graphtec/Silhouette encoder + dry-run `cut` CLI (docs/machine.md); no real-hardware transport yet |
| Craft-libs & parity | integrated | `craft-ui`, `craft-theme`, `craft-window`, `craft-settings`, `craft-atomic` integrated across desktop app; design aligns with VectorCraft and laser/vinyl prep suites |
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

- [x] VectorCraft-backed engine commands for ellipses, polygons, stars, moving, rotating,
  scaling, exact-position duplication, Pathfinder, alignment and distribution, with
  finite-geometry validation and journal-backed undo/redo that preserves StencilCraft
  metadata snapshots. The desktop exposes duplication for selected objects.
- [ ] Add pen and node editing; continue expanding path construction and editing. The desktop supports click, box and lasso selection, additive multi-select, drag-to-move, Pathfinder operations, alignment/distribution, and primary-shape rotate/scale controls.
- [x] Add image import and threshold silhouette trace-to-path workflow for PNG/JPEG, with a configurable threshold and geometry/resource limits; full-color tracing and embedded raster editing remain open.
- [ ] Add text objects and text-to-path conversion without bundling fonts; define a user
  font/shaping integration point.
- [ ] Expand SVG fidelity: advanced CSS/at-rules, external references, clipping, masks,
  filters, and additional vector formats.
- [ ] Implement Print & Cut: printable SVG sheet export with three alignment marks is available; printer dialog support, machine sensor protocol, calibration and hardware end-to-end tests remain open.
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
  nesting, printer controls and multi-machine output. A manual printable SVG page with
  three registration marks is available through `print-cut`; automatic sensor alignment
  is not implemented.

## Progress log

- 2026-11-01 — MCP stdio JSON-RPC initialization, tool discovery and calls for the existing
  headless session, preserving legacy JSON-lines control; desktop integration remains separate.

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
- 2026-10-30 — desktop canvas parity: F fits and C centers the board while text
  fields retain their keys; compensated kerf geometry is cached until shape geometry
  or kerf width changes; a persisted, bounded Recent menu reopens the eight most
  recently saved/opened documents. App regressions and full `cargo xtask ci` pass.
- 2026-11-01 — shared desktop controls moved to `aiongg/craft-libs` design-system
  revision `7ca77f6a`; the existing VectorCraft-style shell builds against that revision.
  Control of the visible desktop document remains an open gap.
- 2026-10-31 — CI/CD and multi-platform packaging parity: wired shared release, gate CI, packaging-lint, SonarCloud, FreeBSD, and Windows ARM64 workflows via craft-common-actions@main; added macOS, Windows (WiX MSI), FreeBSD, Web, and Linux (deb/rpm via nfpm, AppImage, Flatpak OCI) packaging pipelines; added cargo xtask version subcommand.
- 2026-10-31 — Flatpak OCI distribution: added `--oci` export support to
  `packaging/linux/flatpak-bundle.sh` and `packaging/linux/flatpak-oci.sh` publishing helper
  for GHCR distribution alongside standard bundle builds.
- 2026-10-30 — desktop craft-suite UI overhaul: integrated `craft-ui`, `craft-theme`, `craft-window`, `craft-settings`, and `craft-atomic`. Implemented CSD `TitleBar` with embedded icon, `MenuBar`, contextual `OptionsBar`, vertical `ToolRail` with flyouts, millimetre `Rulers` with zero reset, interactive `BezierOverlay` node editing, `PenDraft` path tool, live drag-shape creation, and persistent settings. Quality gates and 32 desktop tests pass.
- 2026-10-29 — app identity pass: original app icon
  (`assets/app-icon/stencilcraft.svg`, original artwork) with ASSETS/NOTICE
  attribution; README with PNG previews
  rendered from our own exporter; committed `examples/` (starter, kerf counter,
  tiling); new `stencilcraft-color` craft-lib (CSS parse/format, luminance, blend);
  CLI end-to-end integration tests (9) and a malformed-input no-panic corpus (5) —
  110 workspace tests; fixed the SVG export `<svg>` unterminated-tag bug found by
  the preview renders.
