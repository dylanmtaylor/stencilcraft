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
7. **UI**: craft-suite integration complete with CSD `TitleBar` (window frame and embedded icon), `MenuBar` (File, Edit, Object, View, Window, Help), contextual `OptionsBar`, and vertical `ToolRail` with flyouts. Full node-level editing via `BezierOverlay`, interactive path drawing via `PenDraft`, live drag-to-create shape primitives, millimetre `Rulers` with draggable zero-origin reset, and dockable `Layers & Objects` and inspector panels using `craft-ui` components.
8. **Agent surface**: `command.execute` exposes the serde-tagged engine commands through
   the CLI JSON-lines process and shared session; the wider command set is not wrapped in
   dedicated control methods, MCP tool schema hydration is not implemented, and the JSON
   channel has no fuzz corpus. (The document reader does have a malformed-input no-panic
   corpus now.)
9. **wasm**: no web build.
10. **ci**: `xtask layers` checks dependencies by reading manifests (cheap and honest),
   but there is no `cleanroom`/`assets` check yet; `ASSETS.md` is enforced by review only.
11. **Raster trace**: CLI imports bounded PNG/JPEG and traces a configurable dark threshold silhouette into closed cut contours (96 dpi default). UI has dedicated Trace Image tool trigger in the tool rail.
12. **Packaging**: window icon wired from embedded PNG asset (`APP_ICON_PNG`); Linux packaging tarball and Flatpak scripts (`packaging/linux/package.sh`, `flatpak-bundle.sh`) with `.desktop` and metainfo established.
13. **Craft-libs and desktop UI**: `craft-ui`, `craft-theme`, `craft-window`, `craft-settings`, and `craft-atomic` are wired across the desktop application at the design-system branch head (`053c0043`), with `craft-draw` supplying the Bezier/pen path editing. The app already uses the shared chrome (`TitleBar`, `MenuBar`, `OptionsBar`, `ToolRail`, `StatusBar`, `DockArea`) and installs the suite default theme, the same base VectorCraft relies on. Remaining work is layout and visual parity with VectorCraft, which needs a display to check (not yet available in this environment).
15. **Visual parity with VectorCraft (checked on Xvfb, 1600x1000)**: both apps share the craft-libs `suite_default` theme and chrome. Differences seen in screenshots:
    - StencilCraft has a plain text title and menu row; VectorCraft has an app-icon home button and a workspace switcher (`Essentials`) in the top bar.
    - StencilCraft's tool rail is a flat column of unlabelled icons with no visible grouping; VectorCraft groups tools with flyout markers and a selected-state fill.
    - StencilCraft's right side is one `Design Inspector` tab plus a bottom `Layers & Objects` pane; VectorCraft's right dock has an icon strip and `Properties / Layers / Libraries` tabs.
    - StencilCraft's top toolbar mixes controls (`Kerf`, `Preview Kerf`, `Rulers`, `Grid`, `Make`); VectorCraft keeps a single row of zoom/rotation fields at the bottom.
    - StencilCraft has a text hint row above the canvas; VectorCraft's hint is in the bottom status bar with render and fps stats.
14. **Typing and dialog behaviour on the new head**: `TextField` buffers typing until Enter, focus loss or commit, and a plain field passes Escape through instead of clearing. The text-dialog test encodes this; any UI that assumed per-keystroke updates should commit explicitly.
