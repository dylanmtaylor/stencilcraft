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
   or a miter spike on a hairpin can produce a bad polygon. No sweep-based offset.
   Counter detection counts nesting depth (outline, hole, island, …), but containment is
   tested vertex-by-vertex against flattened contours, so a deeply nested or nearly
   touching pair can still be misjudged. Convex invariants are covered by tests: area is
   strictly increasing in kerf width from triangles to 64-gons, the area gain for a
   hairline kerf matches perimeter × kerf/2 within 5%, and mitered vertices land at the
   geometric miter distance.
3. **Tiling**: geometry is clipped to each sheet (closed contours that cross the edge come
   back as open runs, so no cut is added along the material edge) and sheets with nothing
   on them are not exported. Sheets can be stamped with corner registration brackets and
   edge overlap guides (`--tile-marks`, `tile.run {marks}`), and `tile-preview`
   (`tile.preview`) renders the whole tiling on one labelled page with cut lines in red and
   engraving in blue. Still missing: overlap merge of the clipped halves into one
   registered sheet — the marks make it possible to align two sheets by hand, but the
   artwork is not re-joined for the machine. A separate `print-cut` command outputs
   artwork with three printable registration/alignment marks, but it does not send sensor
   commands or calibrate a cutter. Tilings are capped at 4096 sheets.
4. **G-code**: feed, cut and engrave power, repeated passes with an optional power
   ramp, and the `M4`/`M3` laser-mode switch are settable (`--feed`, `--power`,
   `--engrave-power`, `--passes`, `--ramp`, `--laser-mode`, and the same knobs on
   `export.gcode`); rapids are always emitted with the beam off, and closed contours
   return to their start point. Missing: a material/machine preset table (the right
   numbers for a diode on 3 mm ply are the operator's, not a guess), separate
   first/last-pass settings, Z-based knife/plotter mode with per-pass depth, lead-in
   and lead-out, and corner power compensation on acceleration-limited machines.
5. **DXF**: layers carry operation colours (red cut, blue engrave; hidden layers are
   written with a negative index, the DXF "layer off" convention) so CAM tools that
   sort by colour do the right thing. Import reads LINE, LWPOLYLINE (bulges), POLYLINE,
   CIRCLE, ARC, ELLIPSE, SPLINE and more through VectorCraft's DXF reader, with "fit to
   page" / as-is and endpoint joining, and a mutation corpus. Missing: arc entities
   (export flattens curves into line segments), block/INSERT reuse on export, and the
   exporter writes the document's y-down coordinates straight into DXF's y-up space, so
   the art is mirrored in CAD tools (a round trip through our importer flips it).
6. **Engine**: VectorCraft backs StencilCraft shape creation, transforms, Pathfinder,
   alignment/distribution and edit-journal undo/redo; StencilCraft retains its millimetre
   document and metadata snapshots. The model still has no explicit persistent selection,
   and undo snapshots clone whole documents rather than sharing structure.
7. **UI**: craft-suite integration complete with CSD `TitleBar` (window frame and embedded icon), `MenuBar` (File, Edit, Object, View, Window, Help), contextual `OptionsBar`, and vertical `ToolRail` with flyouts. Full node-level editing via `BezierOverlay`, interactive path drawing via `PenDraft`, live drag-to-create shape primitives, millimetre `Rulers` with draggable zero-origin reset, and dockable `Layers & Objects` and inspector panels using `craft-ui` components.
8. **Agent surface**: the CLI offers MCP stdio tools and legacy JSON-lines control, including
   serde-tagged engine commands; the CLI session stays headless. The desktop `--control`
   socket queues each request onto the UI thread and runs it against the engine behind the
   visible canvas, so agents edit the open document (undoable, with the window
   repainting). Requests are handled between frames, one at a time. `stencilcraft-cli mcp
   --control PORT` bridges an MCP client to that socket, so the same tool names drive the
   open window. The wider command set is not wrapped in dedicated tools, the JSON channel
   has no fuzz corpus, and a request can only be answered while the window is alive. (The
   document reader does have a malformed-input no-panic corpus now.)
9. **wasm**: no web build.
10. **ci**: `xtask layers` checks dependencies by reading manifests (cheap and honest),
   but there is no `cleanroom`/`assets` check yet; `ASSETS.md` is enforced by review only.
11. **Raster trace**: PNG, JPEG, BMP, GIF, TIFF and WebP import as placed image objects
   (bounded decoding) and `image.trace` traces them with threshold, despeckle, low/high-pass
   filters, colour-range mode, smoothing and corner threshold, in all-edges, outer-edge or
   detach mode, plus background removal (`stencilcraft-raster`). Missing: HEIC/HEIF (no
   permissively licensed pure-Rust decoder), centreline tracing, multi-colour layered
   tracing, and curve fitting beyond midpoint quadratics. The desktop UI still uses the
   older `trace-image` path (`apps/stencilcraft-cli/src/trace.rs`) and does not draw
   placed images yet.
12. **PDF/EPS/AI import and PDF/PNG/JPEG export**: vector paths from PDF, EPS/PS and
   `.ai` come in through VectorCraft's readers; fills, text, images and symbol instances
   are dropped and every path becomes a cut shape. PDF export writes strokes only (no
   placed images); PNG/JPEG export renders at a chosen dpi.
13. **Packaging**: window icon wired from embedded PNG asset (`APP_ICON_PNG`); Linux packaging tarball and Flatpak scripts (`packaging/linux/package.sh`, `flatpak-bundle.sh`) with `.desktop` and metainfo established.
14. **Craft-libs and desktop UI**: `craft-ui`, `craft-theme`, `craft-window`, `craft-settings`, and `craft-atomic` are wired across the desktop application at the `storytold/craft-libs` design-system revision `053c0043`, with `craft-draw` supplying the Bezier/pen path editing. The app uses shared chrome (`TitleBar`, `MenuBar`, `OptionsBar`, `ToolRail`, `StatusBar`, `DockArea`) and installs the suite default theme, the same base VectorCraft relies on. Layout and visual parity with VectorCraft remain incomplete.
15. **Typing and dialog behaviour on the new head**: `TextField` buffers typing until Enter, focus loss or commit, and a plain field passes Escape through instead of clearing. The text-dialog test encodes this; any UI that assumed per-keystroke updates should commit explicitly.
16. **Visual parity with VectorCraft**: both apps share the craft-libs `suite_default` theme and chrome.
    - Top bar: StencilCraft has a Home button and an `Essentials` workspace switcher.
    - Right dock: `Properties` and `Layers` tabs share a dock group; VectorCraft also has an icon strip and Libraries tab.
    - Canvas navigation uses the shared bottom status bar; VectorCraft also shows render and fps stats.
    - Remaining differences include tool rail grouping and flyout markers, and the production controls in StencilCraft's top toolbar.
