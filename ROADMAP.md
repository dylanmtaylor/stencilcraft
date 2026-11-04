# ROADMAP

Stage: **M0 — scaffold** (2026-10-29).
Headline: document + kerf + tiling + SVG/DXF/G-code established and tested; desktop shell
edits shapes, previews kerf, saves/opens `.stencilcraft`.

## Dimensions

| Dimension | Stage | Notes |
| --- | --- | --- |
| Document & format | working | Document/Layer/Shape model, `.stencilcraft` JSON v1, roundtrip-tested |
| Import (SVG) | working subset | paths + common basic shapes; groups/transforms/styles; physical units/viewBox; CSS selectors incl. :not/siblings; stroke-colour layers; clip paths; text/masks/filters warned; unsupported drawable elements fail closed |
| Kerf compensation | working | polygon offsetting with miter join; convex shapes exact |
| Tiling | working | sheet grid with overlap; geometry clipped per sheet, empty sheets skipped, 4096-sheet cap |
| Exports | working | SVG, DXF R14, G-code: structure renders in browsers/plotter tools (SVG `<svg>` tag bug fixed) |
| Editing tools | working foundation | VectorCraft-backed ellipse, polygon, star, transforms, Pathfinder, alignment/distribution and journal-based undo/redo; StencilCraft retains its millimetre document metadata |
| UI | working | Craft-suite desktop shell (`TitleBar`, `MenuBar`, `OptionsBar`, `ToolRail`, `Rulers`, `BezierOverlay`, `PenDraft`), canvas pan/zoom with Fit (F) and Center (C), layers & objects panel, click/box/lasso multi-select, Pathfinder, cached compensated kerf preview, persisted settings |
| Agent surface | working | CLI `run`/MCP tools and legacy JSON-lines control; desktop `--control` drives the visible document |
| Machine control | preliminary | 32-model catalog (Silhouette/Graphtec, Graphtec CE, generic GP-GL/HP-GL/DMPL), limit-checked encoders, `nusb` USB transport behind `usb` (untested on hardware), no Bluetooth (docs/machine.md) |
| Craft-libs & parity | integrated | `craft-ui`, `craft-theme`, `craft-window`, `craft-settings`, `craft-atomic` integrated across desktop app; design aligns with VectorCraft and laser/vinyl prep suites |
| Platforms | preliminary | desktop native OK; wasm planned |

## Milestones

- **M0 (this): scaffold.** Workspace, conventions, quality gates, format v1, working CLI
  round trip: load → kerf → tile → export.
- **M1: real import.** SVG import through transforms/groups/styles; first property tests
  of round trips; `.stencilcraft` v1 frozen.
- **M2: kerf 2.0.** Self-intersection-robust offsetting (sweep-based), per-layer and
  per-object kerf overrides, inlet/pressure presets by material, kerf preview overlay.
- **M3: cutting output.** Digital-bridge generation so counters stay attached, overlap
  merge of clipped tiles plus alignment marks, multiply-aligned SVG/PDF sheets,
  machine-specific G-code profiles (Grbl, Marlin/Creality, diode-laser).
- **M4: editing tools.** Pen, direct selection, box/lasso select, node editing; SVG path
  tools on par with the import set; undo structural sharing.
- **M5: web.** Same UI in the browser via WASM; file open/export parity.

## What's next (ranked)

1. SVG import completeness (M1): text to outlines (needs a font seam below the
   engine), masks/filters, external `<use>`, `@media`; run coverage-guided fuzzing on
   the bounded XML/CSS parser.
2. Property tests: convex kerf invariants and tiling coverage/bounds are in place; still
   open are containment-heavy kerf cases, format fuzz, and coverage-guided parsing.
3. Kerf: sweep-based offset for self-intersecting contours; nesting depth is counted now,
   but containment still tests flattened vertices.
4. Bridge generation prototype (M3 early): algorithm notes in `docs/bridges.md`.
5. G-code profiles: material/machine presets, separate first/last pass, Z knife mode.
6. Machine control (docs/machine.md): hardware validation of the USB transport and the
   registration-mark session, Bluetooth RFCOMM/BLE, mat-free path sorting for
   knife-pressure relief.

## Reference-app feature inventory

The full feature comparison with Silhouette Studio 5.0.x and Cricut Design Space 9.8x
lives in [`docs/reference-apps.md`](docs/reference-apps.md). It lists each feature, our
status (have / partial / missing / out of scope) and our posture (replicate / reimagine /
won't do). It replaced the Silhouette Studio feature TODO list that used to be here. The
same doc states the clean-room posture: we drive the reference apps through their normal
UI and read their public documentation, then write our own implementation, never using
their code or assets. Missing items feed the ranked list in [`docs/gaps.md`](docs/gaps.md). The menu bar
design, with Silhouette Studio's names and order plus Illustrator-grade items, is in
[`docs/menu-map.md`](docs/menu-map.md). Owner goal: no Silhouette or Cricut feature is
left undocumented or unreproduced, and every format and cutter they support is covered.
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
- [ ] Expand SVG fidelity: external references, `@media`, masks, filters (clipping,
  colours and advanced selectors are in), and additional vector formats.
- [ ] Implement Print & Cut: printable SVG sheet export with three alignment marks is available; printer dialog support, machine sensor protocol, calibration and hardware end-to-end tests remain open.
- [ ] Add production tools: weed lines, nesting, multi-cutter job assignment, batch and
  repeated-pass workflows.
- [ ] Expand design formats and project interoperability, including DXF import and a
  documented strategy for proprietary Studio3 files (do not reverse-engineer protected
  formats without an appropriate public specification).
- [ ] Grow machine support: Bluetooth, hardware-verified USB and registration marks,
  Curio/Graphtec CE USB ids, and mat-free knife path ordering. (Catalog, USB transport,
  discovery, status query and registration-mark commands landed 2026-11-02.)
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

- 2026-11-03 — M3: Make exposes cut/draw, tool, holder, passes and mirror
  through validated hardware jobs. Per-layer actions, tool detection and Pop-out
  remain missing; new controls have not been hardware tested.

- 2026-11-03 — M3: Make offers matless sheet dimensions and optional AutoBlade
  depth; matless jobs validate against sheet bounds. Exact roller margins and
  Pop-out cutting remain unimplemented and hardware verification remains pending.

- 2026-11-03 — M3: documented Pop-out Cut behavior and unresolved channel
  positioning and protocol requirements; matless through-cut is not implemented.

- 2026-11-03 — M4: Delete and Backspace remove selected shapes in Make as
  well as the canvas, while text input keeps keyboard focus.

- 2026-11-03 — M3: USB outcomes and errors are shown at the top of Make
  and scrolled into view after communication; hardware cutting remains unverified.

- 2026-11-03 — M3: Make displays the detected USB cutter model without a
  conflicting model selector; offline preparation retains manual selection.

- 2026-11-03 — M3: Make describes validation and USB submission separately,
  and no longer directs desktop users to the CLI for real sends.

- 2026-11-03 — M3: Make limits mat choices to the selected cutter catalog and
  clears an incompatible mat when the cutter changes.

- 2026-11-03 — M3: Linux cutter access setup generates udev rules for catalogued
  USB IDs; active-session access and Flatpak permissions are documented.

- 2026-11-03 — M3: grant raw USB access in both Flatpak manifests and the
  fallback bundle builder so packaged desktop builds can reach cutters. Host
  device permissions and hardware validation remain necessary.

- 2026-11-03 — Shape drawing now paints a live outline while dragging stars,
  polygons, ellipses and rectangles, using the same command as the final shape.

- 2026-11-03 — Desktop USB submission: discover catalogued cutters, select a device,
  validate visible paths and submit through the existing machine session on a worker
  thread. USB enabled by default in desktop builds. Hardware testing pending (Cameo 4
  on Linux); Cricut remains export-only. Validation: `cargo xtask ci` passed.
  Additional WASM check blocked by local toolchain/dependency resolution.

- 2026-11-03 — View aids (M4, gaps 18/36): Zoom to Selection (Ctrl+Alt+0) and the 25–400 %
  presets, Rotate Page 0/90/180/270 (view state: rulers, overlays and drags follow the
  turn), canvas guides from the rulers (`guide.add/move/remove`) plus Show/Lock/Make/
  Release/Clear (`guide.lock-all`, `guide.make`, `guide.release`), Snap to Grid, Smart
  Snapping and Snap to Point (`snap.set` {points}) applied to move drags through
  `snap.point`, Smart Guides drawing the snap lines, and canvas scroll bars (craft-ui
  `CanvasScrollbars` over the mat/board extent plus a margin) to drag a lost mat back. Measured: unit tests only
  (engine snapping and guide commands, headless view/paint helpers); not checked on screen.
- 2026-11-03 — Canvas: wheel zoom no longer throws the canvas off screen once the view has
  been panned (zoom re-anchored by adding to the pan instead of replacing it) and now
  zooms around the pointer. The single-entry "Essentials" workspace switcher is removed
  from the title bar.
- 2026-11-03 — Weld on cut and cut by layer in the machine job layer: `cut --weld` /
  MCP `weld` unites overlapping closed contours per operation with the Pathfinder
  boolean engine (measured: two squares overlapping by 10 mm cut as one 100 mm outline,
  not 160 mm), and `cut --layer NAME:SET` / MCP `layers` give each layer or `#rrggbb`
  line colour skip, tool holder, passes, force and speed, validated against the machine
  and shown in the dry run (gap 31; desktop Send panel not wired).

- 2026-11-03 — SVG import remainder (M1): CSS Color 4 colours in `crates/color` (148
  names, `#rgba`/`#rrggbbaa`, `rgb()`/`hsl()` with percentages and alpha); stroke colours
  become `Line #rrggbb` layers; `clip-path` clips geometry via VectorCraft booleans;
  attribute operators, sibling combinators, `:not()`/`:first-child`/`:last-child`/`:root`;
  `import_with_warnings` names skipped text, masks, filters and ignored CSS, and the CLI
  `import-svg` and desktop import show them. Measured: 8 new SVG tests, 2-seed mutation
  corpus; text to outlines still open (docs/gaps.md 1).
- 2026-11-03 — Silhouette Studio colours: line colours (stored as B, G, R, A floats) put
  each imported shape on a `Line #rrggbb` cut layer, as Send by Line Color groups them.
  Measured: the default blue and the #b87c1b sample come back as their own layers.
- 2026-11-03 — Cut job settings in the machine job layer, for every protocol: cut order
  (No Sort / Maximize Speed / Minimize Roller Movement, default roller), interior contours
  first (default on), closed-contour overcut (0–5 mm) and mirror on mat for heat-transfer
  vinyl, as `stencilcraft-cli cut` flags and `machine.dry_run` params (gaps 21, 30;
  behaviour from our Send-preferences observation notes, measured by unit tests only, no
  hardware run).
- 2026-11-03 — Transform power tools reach the desktop: `shapes.shear` (also a
  `transform.each` field), Flip Horizontal/Vertical, Transform Again (Ctrl+Shift+D),
  Transform Each (panel checkbox), Align › To Key Object and Distribute Spacing by Value.
  The Transform panel and Rotate menu now use the remembered `transform.each`, so Transform
  Again repeats them. Measured: engine tests cover shear geometry, repeat, undo and refusal.
- 2026-11-03 — Silhouette Studio documents: the reader now reads each object's
  page-coordinate outline, which brings in open paths (lines), compound paths and text
  (as glyph outlines), and rotated and grouped objects at their on-page positions. This
  was learned from 9 new samples saved on screen in SS 5.0.539. The table of verified
  samples now checks page size (10 × 7 in confirms width before height). Measured: 8 of the 9
  samples present decode to the bounds the app showed; the text sample is checked by count only.
- 2026-11-03 — Reconciled the standing gaps list with merged command and desktop
  coverage. Engine-only features remain marked as incomplete desktop workflows.
- 2026-11-03 — DXF export reflects canvas Y coordinates at the board height, preserving
  asymmetric artwork orientation in Cartesian CAD viewers and the DXF import round trip.
  DXF import still normalizes artwork to its extents and does not preserve page margins.
- 2026-11-03 — Desktop text creates editable objects with a font-family picker and point
  size, and Type › Edit Selected Text updates them through the engine. Font-style and
  spacing controls remain open; the single-line font needs no bundled fonts.

- 2026-11-03 — Document model and commands add groups, layer ordering, page metadata,
  guides, grid and snap settings, local library and recovery helpers, and project
  templates. They are available through engine commands and MCP; desktop integration
  remains incomplete. Raster images and canvas metadata share the document round trip.
- 2026-11-03 — Production groundwork: shape operation and contour metadata, reversible
  combine/attach workflows, mat layout, and print-sheet planning are available through
  engine commands and MCP. Hardware cutting and complete UI wiring remain open.
- 2026-11-02 — New Document dialog (M1.3): File › New… (Ctrl/Cmd+N), New from Template…
  (Ctrl/Cmd+Shift+N) and app start without a file open an Illustrator-style dialog: tabs
  (Recent, Cut Mats, Vinyl, Paper & Card, Print & Cut, Laser) over a grid of preset cards
  with code-drawn thumbnails, and a Preset Details column (name, width, height, units,
  orientation, cutting mat, machine, kerf) with Create/Close. Create runs `document.new` and
  `kerf.set`; recent sizes persist in settings. Built from craft-ui's dialog, tokens and
  controls (`apps/stencilcraft/src/new_doc.rs`, 8 tests). Presets set board, mat and kerf
  only; starter geometry (card fold score, tag hole) is still missing (docs/gaps.md 27).
- 2026-11-02 — canvas restyled after the reference app's dark look (dark stays the default
  for new installs): charcoal craft `pro` dark chrome, mid-grey pasteboard, light-grey
  rounded mat with a rim and up arrow indicating its feed edge, white page with a thin blue border (`docs/ui-design.md`). Appearance estimated, not measured.

- 2026-11-02 — Silhouette Studio documents: the new `crates/studio` reads outlines and page
  size from `.studio` (v2) and `.studio3` (v3) files. Everything it knows was learned only
  from files we saved ourselves (`docs/formats/silhouette-studio.md`). It sits behind the new
  `FormatHandler` seam (`docs/formats/README.md`) and is wired into CLI `import`/`--in` and
  MCP `doc.open`, with a truncation and bit-flip corpus. Protected store designs are
  rejected. Curves and writing are still to come.
- 2026-11-02 — Import and trace: `import` (CLI) / `import_file` (MCP) read SVG, DXF,
  PDF, EPS/PS/AI vector paths (as is or fitted to the page, open paths joined within a
  tolerance) and PNG/JPEG/BMP/GIF/TIFF/WebP images placed as image objects; `trace` /
  `image.trace` turn an image into closed, oriented, simplified cut contours with
  threshold, despeckle, blur/edge filters, colour range, smoothing/corner threshold and
  all-edges/outer-edge/detach modes, plus background removal. New exports: vector PDF
  and PNG/JPEG at 300/600/1200/custom dpi. Every reader has a mutation corpus.
- 2026-11-02 — live text engine commands (`docs/mcp.md` › Text): `text.add`/`text.edit`
  with system fonts or a font file (VectorCraft's font database and shaping), size, x/y,
  left/centre/right/justify, letter and line spacing, kerning toggle, vertical; weld,
  ungroup to lines/letters, per-letter nudges, curve by diameter, text on a path (offset,
  start, flip), to-outlines, font listing, monogram with built-in frames, and an original
  single-stroke font drawn in code for pens. Text objects saved in `.stencilcraft` (no
  version bump). 13 engine tests (measured). Desktop UI not wired yet.

- 2026-11-02 — path editing engine commands (`docs/mcp.md`): break, join, close,
  average/merge, delete nodes, corner/smooth, line/curve segment, simplify with a
  tolerance (deviation tested ≤ 1.5× tolerance), outline stroke with join/cap, shape to
  path, make/release compound, Pathfinder Subtract All, lock/hide flags (hidden shapes
  skipped by every exporter), same-fill/same-stroke/inverse selection and node-count
  queries; dedicated MCP tools and `run --command`. 14 engine tests (measured). Desktop
  menus not wired yet.
- 2026-11-02 — Reference-app inventory: `docs/reference-apps.md` lists Silhouette
  Studio and Cricut Design Space features area by area, with status and posture; it
  replaces the feature TODO list here. The clean-room posture now allows driving the
  reference apps through their UI and recording what we observe; their code and assets
  stay off limits.

- 2026-11-02 — Canvas navigation uses the shared status bar: Fit, Center and zoom
  controls move below the canvas, leaving more room for the artwork. The zoom slider
  remains anchored at the canvas center and is covered by a desktop UI test.

- 2026-11-01 — Kerf invariants: convex contours must grow in area with every kerf width
  (triangles through 64-gons, 5–120 mm), a hairline kerf must add perimeter × kerf/2
  of area within 5%, and mitered vertices must land at the geometric miter distance.

- 2026-11-01 — Cut profiles and canvas auto-fit. G-code export takes feed, cut and
  engrave power, repeated passes with an optional power ramp, and the `M4`/`M3` mode
  switch; rapids are emitted with the beam off (they used to travel with the laser on),
  and closed contours return to their start. DXF layers now carry operation colours
  (red cut, blue engrave, negative index for a hidden layer). Changing the board size
  re-frames the canvas instead of leaving the old zoom/pan pointing at the previous
  sheet.

- 2026-11-01 — Tile alignment marks (corner brackets plus overlap guides) and
  `tile-preview`/`tile.preview`, which renders every sheet on one labelled page with
  cut lines red and engraving blue; `mcp --control PORT` bridges MCP clients to the
  open window.

- 2026-11-01 — Tiling clips geometry to each sheet instead of copying whole shapes (empty
  sheets are skipped, tilings capped at 4096 sheets); kerf decides hole vs solid by nesting
  depth; `--tile-prefix` is honoured; the desktop `--control` socket now runs requests against
  the visible document.

- 2026-11-02 — cutters: catalog of 32 machines (Portrait 1-4, CAMEO 1-5 incl. Plus/Pro/
  Alpha, Curio 1/2 export-only, Craft Robo, Graphtec CE6000/CE7000/CE LITE, generic
  GP-GL/HP-GL/DMPL) with unknowns left unknown; `encode_job` rejects out-of-range force,
  speed, tool, holder, depth, mat and area; passes, line-segment overcut, AutoBlade depth,
  track enhancing, registration marks; `nusb` USB transport (`usb` feature), `machines` /
  `devices` CLI, MCP `machines_list` / `machine_dry_run`; per-model golden transcripts.
  Cricut documented as export-only.
- 2026-11-01 — MCP stdio JSON-RPC initialization, tool discovery and calls for the existing
  headless session, preserving legacy JSON-lines control.

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
- 2026-11-03 — control socket without frames: requests the window does not pick up
  within 10 s answer a "not drawing frames" error (id kept, request discarded) and
  `app.screenshot` falls back to a headless 96 dpi document render. Unit-tested.
- 2026-11-01 — VectorCraft visual parity pass: added Home button and Essentials workspace switcher dropdown to title bar, unified right dock into Properties and Layers tab group, and relocated canvas navigation shortcuts hint to status bar. Full quality gates and desktop tests pass.
- 2026-11-01 — shared desktop controls pinned to `storytold/craft-libs` design-system
  revision `053c0043`; the existing VectorCraft-style shell builds cleanly against that revision.
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
