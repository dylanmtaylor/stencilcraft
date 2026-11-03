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
   sort by colour do the right thing. Import reads LINE, LWPOLYLINE (bulges), POLYLINE,
   CIRCLE, ARC, ELLIPSE, SPLINE and more through VectorCraft's DXF reader, with "fit to
   page" / as-is and endpoint joining, and a mutation corpus. Missing: arc entities
   (export flattens curves into line segments) and block/INSERT reuse on export. DXF
   export preserves canvas orientation using a board-height Y reflection. The DXF
   importer normalizes the artwork to its extents, so absolute page offsets and blank
   margins do not round-trip. Axis conventions elsewhere, including tile previews, still
   need a separate audit.
6. **Engine**: VectorCraft backs StencilCraft shape creation, transforms, Pathfinder,
   alignment/distribution and edit-journal undo/redo; StencilCraft retains its millimetre
   document and metadata snapshots. Node-level path commands (break, join, close,
   average/merge, delete, corner/smooth, line/curve segment), simplify, outline stroke,
   compound make/release, Subtract All, lock/hide flags and selection/node-count queries
   exist as engine commands and MCP tools (`docs/mcp.md`); desktop menus and panels
   expose a subset, while complete node-editing workflows remain unfinished. Shapes carry one operation colour, so "same fill" and "same stroke" differ
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
16a. **User offset**: `shapes.offset` creates outward or inward outlines with miter,
    round or bevel joins; the desktop Offset panel provides a preview and Apply. Welding
    multiple offset results into one remains missing.
17. **Groups, stacking and layer control**: engine commands support groups,
    reordering, object and layer locks, renaming and moving objects between layers.
    Desktop controls and shortcuts remain incomplete. Attach and Contour commands exist
    in the production engine; their desktop workflows still need integration.
18. **Snapping and guides**: the document and command layer supports guides, grid
    and snap settings, snapping queries and measurements. Desktop gesture integration,
    smart guides and live width/height callouts remain incomplete; display-unit support
    also needs integration in the desktop UI.
19. **Text**: the desktop Text dialog creates editable text using the built-in single-line
    font or an installed font family, with a point-size control; Type › Edit Selected
    Text updates it. Missing: font file and style selection, direct canvas insertion,
    spacing and alignment controls, text on a path or arc, and a glyphs panel.
20. **Replicate and nesting**: engine commands and desktop panels cover directional,
    mirrored, array, radial and fill-page copies. Production mat planning has nesting;
    an integrated desktop mat preview and hardware validation remain open.
21. **Cut job settings**: cut order (none/speed/roller), interior contours first,
    closed-contour overcut and line-segment overcut exist as job settings (CLI `cut`, MCP
    `machine.dry_run`); the desktop Send panel doesn't expose them yet, and the speed order
    doesn't choose contour start points. Missing: test cut, custom saved materials, per-layer settings, a tool/blade-depth chooser and passes for the cutter.
    Also missing: weed lines and borders, and more operation types (pen, score,
    perforate). Also missing: Send modes that set one row of settings per line or fill
    colour, assigning lines to tool holder 1 or 2 (or skipping them) on the cut preview,
    and advanced material settings (acceleration, track enhancing, kraft-blade loop
    corners).
22. **Machine connection**: CLI supports serial/CDC and feature-gated USB bulk, device
    discovery and status queries; USB and registration-mark sessions need real hardware
    validation. Bluetooth, a desktop machine status card, interactive mat preview,
    multi-cutter output and batch jobs remain open. Cricut hardware control is out of
    scope without a public protocol.
23. **Path editing tools**: missing freehand and smoothed freehand, arc, knife, eraser,
    break/join/close path, release compound path, simplify, flip/mirror buttons, a
    clipboard, Paste in Front, Select by Color, Detach Lines, a rotation handle on the
    selection, a reference-point picker for the X/Y fields, a preference for what a tool
    does after use, a node-count readout, and warp.
24. **Trace**: engine/MCP support filtering, outer-edge and detach modes and background
    removal; the desktop trace workflow still needs these controls and a live preview.
    Multi-colour tracing to layers remains missing.
25. **Print & Cut, beyond item 3**: missing printing from the app (resolution, preview),
    a printable-area border on the canvas, mark size and position options, bleed,
    printing cut lines, fill colour/gradient for printed artwork, Flatten, print size checks
    and calibration. Scan-to-cut is low priority.
26. **Formats**: DXF, PDF and EPS/AI vector import, PDF/PNG/JPEG export and placed
    raster images are available with the fidelity limits in items 5, 11 and 12. The
    `.studio`/`.studio3` reader imports a limited, lossy subset; writing, GSD and CDR
    remain unsupported. Recovery helpers exist but automatic desktop recovery is not wired.
27. **Documents and preferences**: one board per document and one document per window,
    with no tabs. File › New opens a New Document dialog with size/mat/laser presets and
    recent sizes, but presets add no starter geometry yet (card fold score, tag hole,
    print-and-cut marks) and there are no saved user presets. No Home screen and no
    simplified starter workspace. No
    default machine/material preferences, no UI language choice and no local design
    library beyond Recent. The browser/mobile client is item 9.
28. **Line and fill styles for pens and specialty tools**: shapes are only cut or
    engrave. Missing: dashed lines with joins and caps, solid/gradient/pattern fills,
    sketch (hatch) and stipple fills for pens, and emboss/deboss/foil patterns. Low
    priority: a pop-up card strut generator, a spelling check for text, and image
    effects on placed rasters.
29. **Outline Stroke and Expand**: the engine now has `path.outline-stroke`, which turns
    a stroked or open path into a closed cuttable outline. Missing: its menu/UI wiring and
    a general Expand. This is the usual way to make stencil lettering and line art
    cuttable.
30. **Mirror on mat and load type**: a per-job mirror (`cut --mirror`, MCP `mirror`)
    flips about the media centre at job build time; the desktop Send panel doesn't offer it
    yet and no material preset turns it on. No choice between on-mat, without-mat (Smart Materials) and roll feed,
    no project copies, and no arranging pieces on a mat preview.
31. **Weld on cut and cut by layer**: overlapping shapes are cut twice unless welded by
    hand, and there is no per-layer cut order or per-layer settings at send time.
32. **Path construction**: the engine now has `path.join`, `path.average-nodes`,
    `path.break`, `path.close`, `shapes.make-compound` and `shapes.release-compound`.
    Missing: clipping masks (make/release) and menu/UI wiring for the path commands.
33. **Transform power tools**: Transform Again, Transform Each, shear, flip, align to a
    key object and distribute spacing by value now run from the menus and the Transform
    panel, which also has Silhouette's 33/50/100/200/300 % Scale presets. Missing: a
    key-object picker (the key is the last shape selected), Rotate To (an absolute angle
    needs a stored per-shape rotation), and the Ctrl+Alt+Shift+D shortcut. Move To with a
    reference point is the options bar's X/Y fields and pivot.
34. **More cutters and formats** (see the Cutters and File formats tables in
    [`reference-apps.md`](reference-apps.md)): the model catalog and USB transport have
    expanded but need hardware validation, and not every model is supported. The readers
    and exporters in items 5, 11, 12 and 26 cover subsets; HEIC, GSD and CDR remain
    unsupported. Cricut hardware control lacks a public protocol.
35. **Multiple boards**: the document model supports extra pages and the command
    layer can create a document from templates. Desktop multi-page editing, a fully
    integrated template wizard and Save Selection As remain incomplete.
36. **View aids**: missing smart guides, snap to point, guides (make, release, lock),
    outline view, Show Print/Cut Border, rotated page view and zoom to selection.
37. **Select menu**: Select Inverse and Select Same (fill, stroke, layer) exist now.
    Missing: Select Same operation, next object above/below, and saved selections.
38. **Rhinestone, lattice and other generators**: engine/MCP include rhinestone, puzzle
    and other geometric generators. Complete desktop controls, lattice, Flexishapes,
    cross-stitch and a font creator remain open.
39. **Colour tools**: missing swatches, recolour, graphic styles (saved
    line/fill/operation sets) and an eyedropper.
40. **Automation**: missing recorded actions and batch replay inside the app. MCP and the
    CLI cover scripting from outside.
41. **Design Space conveniences**: missing a history panel, tool and accessory prompts on
    the Make screen, a favourite-materials list, an uploaded-images library and project
    collections (as local folders).

42. **Sticker workflow** (Design Space Create Sticker): engine/MCP provide a sticker
    command with a welded border and kiss-cut or die-cut operation metadata. Desktop
    creation, print rendering and hardware depth validation remain open. See the
    [Cricut library](cricut-design-space/edit-bar.md#create-sticker).
43. **Live (non-destructive) shape operations**: engine/MCP Combine retains source
    shapes for Unite, Subtract, Intersect and Exclude and can uncombine them; the
    desktop Pathfinder remains destructive and needs a live-combine workflow.
44. **Offset UI**: the desktop Offset panel previews and applies `shapes.offset` with
    join style and miter limit. A weld-offsets option remains missing.
45. **Templates and Guide layers**: missing sizing backgrounds that are shown but never
    cut, a Guide operation, and document presets ("base type").
46. **Make-screen validation**: missing checks for each machine's limits: Smart Materials
    minimum and maximum lengths, card mats (2×2, 1×2, Joy), Print Then Cut usable area
    and page size, and roller margins. Also missing: per-machine default load type and
    page size, and remembering the material between mats.
47. **Upload cleanup**: missing Select (same-colour areas), Erase/Restore brushes,
    despeckle, smooth, reduce colours, preview as a single layer, and saving an upload as a
    printable image rather than an outline. User pattern uploads are also missing.
48. **Monogram and text conveniences**: engine/MCP provide a monogram command, ungroup
    to lines or letters, and curved text by diameter. Desktop builders and editing controls
    are not yet wired.
49. **Silhouette fills, groups and live text**: the `.studio` reader maps line colours to
    layers but drops fill colours, groups and live-text parameters (text comes in as
    outlines); no writer.
50. **Control channel while the window gets no frames**: requests now time out after 10 s
    and `app.screenshot` falls back to a 96 dpi document render, but that render has no
    window chrome, selection or kerf preview, and edits still cannot be applied until the
    window draws again (the headless session could run them off the UI thread).

The menu-bar placement and proposed engine command for each of these is in
[`menu-map.md`](menu-map.md).
