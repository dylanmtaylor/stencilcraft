# gaps

Every known shortfall, ranked; the standing work list. Pick the first item you can finish.

Items 16 onward come from the reference-app inventory in
[`reference-apps.md`](reference-apps.md). It holds the feature-by-feature detail and the
clean-room posture for studying Silhouette Studio and Cricut Design Space.

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
   geometric miter distance. Round and bevel joins and a corner (miter) limit are
   selectable (`--kerf-join`, `--kerf-limit`, `shapes.offset`); round-join area is tested
   to grow with distance and stay within the miter area on convex shapes. A large internal
   offset can still invert a thin feature instead of removing it.
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
   sort by colour do the right thing. Missing: arc entities (export flattens curves
   into line segments), block/INSERT reuse, and a reader.
6. **Engine**: VectorCraft backs StencilCraft shape creation, transforms, Pathfinder,
   alignment/distribution and edit-journal undo/redo; StencilCraft retains its millimetre
   document and metadata snapshots. Node-level path commands (break, join, close,
   average/merge, delete, corner/smooth, line/curve segment), simplify, outline stroke,
   compound make/release, Subtract All, lock/hide flags and selection/node-count queries
   exist as engine commands and MCP tools (`docs/mcp.md`); the desktop menus do not call
   them yet. Shapes carry one operation colour, so "same fill" and "same stroke" differ
   only by open vs closed paths. The model still has no explicit persistent selection,
   and undo snapshots clone whole documents rather than sharing structure.
7. **UI**: craft-suite integration complete with CSD `TitleBar` (window frame and embedded icon), `MenuBar` (File, Edit, Object, View, Window, Help), contextual `OptionsBar`, and vertical `ToolRail` with flyouts. Node editing (move points, convert smooth/corner) via `BezierOverlay`, interactive path drawing via `PenDraft`, live drag-to-create shape primitives, millimetre `Rulers` with draggable zero-origin reset, and dockable `Layers & Objects` and inspector panels using `craft-ui` components.
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
11. **Raster trace**: CLI imports bounded PNG/JPEG and traces a configurable dark threshold silhouette into closed cut contours (96 dpi default). UI has dedicated Trace Image tool trigger in the tool rail.
12. **Packaging**: window icon wired from embedded PNG asset (`APP_ICON_PNG`); Linux packaging tarball and Flatpak scripts (`packaging/linux/package.sh`, `flatpak-bundle.sh`) with `.desktop` and metainfo established.
13. **Craft-libs and desktop UI**: `craft-ui`, `craft-theme`, `craft-window`, `craft-settings`, and `craft-atomic` are wired across the desktop application at the `storytold/craft-libs` design-system revision `053c0043`, with `craft-draw` supplying the Bezier/pen path editing. The app uses shared chrome (`TitleBar`, `MenuBar`, `OptionsBar`, `ToolRail`, `StatusBar`, `DockArea`) and installs the suite default theme, the same base VectorCraft relies on. Layout and visual parity with VectorCraft remain incomplete.
14. **Typing and dialog behaviour on the new head**: `TextField` buffers typing until Enter, focus loss or commit, and a plain field passes Escape through instead of clearing. The text-dialog test encodes this; any UI that assumed per-keystroke updates should commit explicitly.
15. **Visual parity with VectorCraft**: both apps share the craft-libs `suite_default` theme and chrome.
    - Top bar: StencilCraft has a Home button and an `Essentials` workspace switcher.
    - Right dock: `Properties` and `Layers` tabs share a dock group; VectorCraft also has an icon strip and Libraries tab.
    - Canvas navigation uses the shared bottom status bar; VectorCraft also shows render and fps stats.
    - Remaining differences include tool rail grouping and flyout markers, and the production controls in StencilCraft's top toolbar.
16. **User offset**: kerf offsetting exists, but there is no command that adds an
    outward/inward offset as a new shape. Also missing: round corners (kerf only mitres)
    and the option to weld the offset outlines into one.
17. **Groups, stacking and layer control**: no group/ungroup, no reordering
    (forward/back, with shortcuts), no locking of objects or layers, no rename, no moving objects
    between layers. Attach (keep positions on the mat) and Contour (turn off single
    sub-paths) build on these.
18. **Snapping and guides**: there is a grid display but no snapping to the grid,
    objects or guides (Silhouette Studio has smart snapping on by default). Also missing:
    guides, a measure tool, and width/height callouts that follow a shape while it is
    drawn. Units are mm only, with no inch display (Silhouette Studio defaults to
    inches).
19. **Text**: the Text tool places built-in single-line bitmap lettering only. Missing:
    user/system fonts (through `CRAFT_FONTS_DIR`, never bundled), size and spacing
    controls, text that can be edited after placing, text on a path or arc, and a
    glyphs panel.
20. **Replicate and nesting**: only duplicate in place exists. Missing: copies in a
    direction, row/column and radial arrays, mirrored copies, fill page and automatic
    nesting.
21. **Cut job settings**: missing test cut, overcut, cut order/path sorting, custom saved
    materials, per-layer settings, a tool/blade-depth chooser and passes for the cutter.
    Also missing: weed lines and borders, and more operation types (pen, score,
    perforate). Also missing: Send modes that set one row of settings per line or fill
    colour, assigning lines to tool holder 1 or 2 (or skipping them) on the cut preview,
    and advanced material settings (acceleration, track enhancing, kraft-blade loop
    corners).
22. **Machine connection**: the only transport is serial/CDC, and only from the CLI.
    Missing: USB bulk (`nusb`), Bluetooth, device discovery, live status, load/unload,
    pause and cancel, a Make-screen mat preview, a machine card with firmware and status
    in the app, a packet-size setting, multi-cutter output and batch jobs.
    Driving Cricut machines is out of scope (no public specification).
23. **Path editing tools**: missing freehand and smoothed freehand, arc, knife, eraser,
    break/join/close path, release compound path, simplify, flip/mirror buttons, a
    clipboard, Paste in Front, Select by Color, Detach Lines, a rotation handle on the
    selection, a reference-point picker for the X/Y fields, a preference for what a tool
    does after use, a node-count readout, and warp.
24. **Trace**: threshold silhouette only. Missing: image filters, outer-edge-only trace,
    trace & detach, background removal, a live preview of the trace area, and
    multi-colour trace to layers.
25. **Print & Cut, beyond item 3**: missing printing from the app (resolution, preview),
    a printable-area border on the canvas, mark size and position options, bleed,
    printing cut lines, fill colour/gradient for printed artwork, Flatten, print size checks
    and calibration. Scan-to-cut is low priority.
26. **Formats**: missing DXF import (see item 5), PDF import, PDF/PNG/JPG export and
    placed raster images for printing. There is no autosave or crash recovery. Proprietary `.studio`/`.studio3` and AI/CDR
    formats are out of scope.
27. **Documents and preferences**: one board per document and one document per window,
    with no tabs. No Home screen (new/recent) and no simplified starter workspace. No
    default machine/material preferences, no UI language choice and no local design
    library beyond Recent. The browser/mobile client is item 9.
28. **Line and fill styles for pens and specialty tools**: shapes are only cut or
    engrave. Missing: dashed lines with joins and caps, solid/gradient/pattern fills,
    sketch (hatch) and stipple fills for pens, and emboss/deboss/foil patterns. Low
    priority: a pop-up card strut generator, a spelling check for text, and image
    effects on placed rasters.
29. **Outline Stroke and Expand**: no way to turn a stroked line or open path into a
    closed outline that can be cut. This is the usual way to make stencil lettering and
    line art cuttable.
30. **Mirror on mat and load type**: no per-job mirror for heat-transfer vinyl, which is
    cut face down. No choice between on-mat, without-mat (Smart Materials) and roll feed,
    no project copies, and no arranging pieces on a mat preview.
31. **Weld on cut and cut by layer**: overlapping shapes are cut twice unless welded by
    hand, and there is no per-layer cut order or per-layer settings at send time.
32. **Path construction**: missing Join, Average, Break at Point, Close Path, clipping
    masks (make/release), and Release Compound Path.
33. **Transform power tools**: missing Transform Again, Transform Each, shear, align to a
    key object and distribute spacing by a set value.
34. **More cutters and formats** (see the Cutters and File formats tables in
    [`reference-apps.md`](reference-apps.md)):
    - Cutters: Silhouette SD, CAMEO 1/2, CAMEO 5 Alpha, Portrait 1/2, Curio 1/2, Craft
      ROBO and Graphtec CE/CB are not in the `Model` table.
    - Formats: PDF/AI/EPS/BMP/GIF/TIFF/HEIC import and PDF/PNG/JPEG export are missing.
      `.studio3`/GSD import waits on an owner decision.
    - Cricut machines are pending the owner's decision and a legal review.
35. **Multiple boards**: one board per document. Missing: Illustrator-style artboards,
    a template-based new document, and Save Selection As.
36. **View aids**: missing smart guides, snap to point, guides (make, release, lock),
    outline view, Show Print/Cut Border, rotated page view and zoom to selection.
37. **Select menu**: missing Select Inverse, Select Same (fill, stroke, operation, layer),
    next object above/below, and saved selections.
38. **Rhinestone, lattice and other generators**: rhinestone layouts, lattice cut-outs,
    Flexishapes, puzzle and cross-stitch generators, and a font creator.
39. **Colour tools**: missing swatches, recolour, graphic styles (saved
    line/fill/operation sets) and an eyedropper.
40. **Automation**: missing recorded actions and batch replay inside the app. MCP and the
    CLI cover scripting from outside.
41. **Design Space conveniences**: missing a history panel, tool and accessory prompts on
    the Make screen, a favourite-materials list, an uploaded-images library and project
    collections (as local folders).

The menu-bar placement and proposed engine command for each of these is in
[`menu-map.md`](menu-map.md).
