# gaps

Every known shortfall, ranked; the standing work list. Pick the first item you can finish.

1. **SVG import**: common shapes (`path`, basic shapes), groups, standard affine
   transforms, inherited presentation/inline display and production styles, physical
   units/viewBox/`preserveAspectRatio`, local `<use>` of supported shapes/groups, and
   `<symbol>` templates with viewBox sizing are supported. A deterministic malformed-input
   corpus is in place. Embedded CSS basic selectors, cascade, and inherited supported
   properties now work. Remaining: advanced CSS selectors and at-rules, color model,
   external `<use>`, text, clip paths, masks, and filters; unsupported drawable elements
   fail explicitly rather than disappear. The parser has bounded XML/CSS bytes,
   nodes/depth/coordinates, rejects duplicate IDs/reference cycles, and caps `<use>`
   instancing at 10,000 instances. Coverage-guided fuzzing remains to be done.
2. **Kerf robustness**: offsetting assumes simple contours; a self-intersecting subpath
   or a miter spike on a hairpin can produce a bad polygon. No sweep-based offset; no
   nested-counter detection beyond bbox-in-bbox.
3. **Tiling**: tile edges are not clipped; overlapping output exports whole shapes per
   tile, relying on overlap to keep continuity. No per-tile SVG merging view or tile
   alignment marks. A separate `print-cut` command outputs artwork with three printable
   registration/alignment marks, but it does not send sensor commands or calibrate a cutter.
4. **G-code**: fixed modal constants, no feed/speed profiles by machine or material,
   no first/last pass, no power ramp, no Z/laser mode switch.
5. **DXF**: no layer colour assignment, no arc entities (export flattens curves into
   line segments), no reader.
6. **Engine**: VectorCraft backs StencilCraft shape creation, transforms, Pathfinder,
   alignment/distribution and edit-journal undo/redo; StencilCraft retains its millimetre
   document and metadata snapshots. The model still has no explicit persistent selection,
   and undo snapshots clone whole documents rather than sharing structure.
7. **UI**: visible-outline selection, canvas drag-to-move, multi-select with additive
   selection, exact-position duplication (button and Ctrl/Command+D shortcut), Delete/
   Backspace removal, box/lasso selection, Pathfinder operations, alignment/distribution
   and primary-shape rotate/scale controls work (selection uses
   conservative shape bounds). Remaining: node editing, a ruler, zoom-to-fit keyboard
   shortcuts, kerf preview
   recomputes on every paint when dirty (no cache), no recent-files list.
8. **Agent surface**: `command.execute` exposes the serde-tagged engine commands through
   the CLI JSON-lines process and shared session; the wider command set is not wrapped in
   dedicated control methods, MCP tool schema hydration is not implemented, and the JSON
   channel has no fuzz corpus. (The document reader does have a malformed-input no-panic
   corpus now.)
9. **wasm**: no web build.
10. **ci**: `xtask layers` checks dependencies by reading manifests (cheap and honest),
   but there is no `cleanroom`/`assets` check yet; `ASSETS.md` is enforced by review only.
11. **Raster trace**: CLI imports bounded PNG/JPEG and traces a configurable dark threshold silhouette into closed cut contours (96 dpi default). No UI import panel, multi-color tracing, or smoothing controls.
12. **Packaging**: no window/app icon wiring yet (the SVG icon exists in
   `assets/app-icon/`); no desktop integration (`.desktop`, file associations,
   `xtask bundle`); PNG previews in the README only (`docs/images/`).
13. **Craft-libs and desktop UI**: the workspace currently contains foundational domain crates (`geom`, `color`, `doc`, `kerf`, `tiling`, `svg`, `dxf`, `gcode`, `engine`, `machine`, `mcp`) and an egui canvas crate. Reusable dockable/movable panel infrastructure, a tool palette, broad design tools and reusable app-shell components have not been built in this workspace. Avoid implying the UI has parity with larger creative applications; select and adopt suitable craft-libs only after their source and APIs are available in this project.
