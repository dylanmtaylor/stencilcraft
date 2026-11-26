# Silhouette Studio observations

Notes from driving Silhouette Studio (Basic edition, Windows 11 VM) through its normal UI, in our own words. See the clean-room posture in [`../reference-apps.md`](../reference-apps.md). The app version each session was made against is noted in the section headings.

## Interface tour (2026-11-02, SS 5.0.539 Basic)

- Launch: splash "Initializing", then Home screen: top menu (File Edit View Panels Object Help); dark top bar with Home icon, "Starter mode" toggle, DESIGN / STORE / LIBRARY / SEND tabs. Home shows "New Document" carousel, Recent Library Files, News & Updates feed.
- First run: "How well do you know Silhouette Studio?" Beginner / Experienced dialog.
- Starter mode toggle (top bar): ON hides the quick-access toolbar and most panels; OFF shows full toolbar (New, Open, Save, Print, Cut/Copy/Paste, Undo/Redo, Group/Ungroup, compound, zoom in/out/fit/select, pan hand, fit-to-page), the context options bar and a right-hand column of ~15 panel icons.
- Design view: document tabs ("Untitled-1" + "+" new tab), grey cutting mat with feed-direction arrow at top, white page inside with a cut border; left tool rail (select, point edit, shapes w/ flyout, line/curve w/ flyout, drawing w/ flyout, text, text box?, eraser, knife), folder button bottom-left (library sidebar).
- Context options bar changes per tool. Text tool: font picker, style "Default", size 72.00 pt, bold/italic/underline, alignment, character spacing, text-on-path, etc.
- Single-letter keyboard shortcuts switch tools (typing into the canvas changed the active tool).
- File menu: New (Ctrl+N), New Project Wizard (Ctrl+Shift+N), Open (Ctrl+O), Open Recent >, Open Recovered…, Merge… (Ctrl+Shift+O, import into current doc), Save (Ctrl+S), Save As >, Save Selection >, Library >, Update Library…, Scan…, Print… (Ctrl+P), Print Page Setup…, Send to Silhouette… (Ctrl+L), Close Tab (Ctrl+W), Quit (Ctrl+Q).
- Edit menu: Undo/Redo (Ctrl+Z / Ctrl+Shift+Z), Cut/Copy/Paste, Paste in Front (Ctrl+F), Delete, Duplicate (Ctrl+D), Select All / Deselect All (Ctrl+Shift+A), Select by Color…, Preferences… (Ctrl+K).
- View menu: Zoom >, Rotate Page >, Grid Options… (Ctrl+F4), Show Grid, Snap to Grid, Enable Smart Snapping (on by default), Show Print Border (Ctrl+Shift+P), Show Cut Border (Ctrl+Shift+L, on), Registration Marks… (Ctrl+F3), Show Design Page Settings… (Ctrl+F1), Show Design Page / Store / Library / Send Page (Ctrl+Alt+D/S/L, Ctrl+F2).
- Panels menu: Page Setup, PixScan… (Ctrl+Alt+P), Fill Color, Line Style, Select by Color, Trace, Image Effects, Text Style, Spelling… (F7), Transform, Replicate, Modify, Offset, Pop up…, Stipple, Sketch.
- Object menu: Mirror >, Arrange >, Align >, Rotate >, Replicate >, Modify >, Group (Ctrl+G), Ungroup (Ctrl+Shift+G), Convert to Path, Make Compound Path (Ctrl+E), Release Compound Path (Ctrl+Shift+E), Simplify (Ctrl+J).
- Save As > Save to Hard Drive (Ctrl+Shift+S) / Save to Library (Ctrl+Alt+R). Basic edition save types: Silhouette Studio V3 (.studio3), V2 (.studio), GSP. No SVG/DXF/PDF/PNG export in Basic.
- Open (Ctrl+O) types in Basic: Silhouette Studio V3/V2, "Graphics Files (Standard)", GSD, DXF, PNG, JPEG, BMP, GIF, TIFF, Picture Files. No SVG/PDF import in Basic (paid editions add them per vendor docs).
- File dialogs are slow to appear (~15 s first time).
- SEND tab: canvas becomes a cut preview on the left; right panel has sub-tabs Guide / Standard / By Line Color (icon) / By Fill Color (icon) and a gear (send preferences).
  - Machine card: model ("CAMEO 5 (Default)"), firmware ("Firmware Unknown"), status ("No Status"), dropdown to pick machine, machine illustration.
  - Guide: "Connect To Machine" list of detected USB/Bluetooth devices, "Rescan for Devices", Connect button. Footer: "No Silhouette Devices Connected", trouble-connecting link, "Bluetooth is unavailable" banner.
  - Standard: Tool Holder 1 / Tool Holder 2 tabs; "Material, Action and Tool" dropdowns (e.g. Cardstock, Plain / Auto (No Action) / tool auto-detected: "Awaiting Connection…"); "Modify Settings" with MORE and "Create New…"; message when no compatible action exists for the material/holder.
  - On-canvas overlay buttons assign selected lines: Tool 1 all selected lines / Tool 1 outline of selection / Tool 2 all / Tool 2 outline / Ignore selected lines.
  - Cut by Line Color: table Tool No. | On/Off | (outline-only icon) | Color | Material | Action | Tool — one row per line colour in the document. Fill-colour mode is the same keyed by fill.
  - MORE → Material Settings: long material list (Crepe Paper, Double-Sided Adhesive, Duct Tape Printable, Fabric Thick/Thin/Linen/Wool, Faux Leather Paper, Felt Acrylic, Cardstock Plain, …) with +/− to add/remove custom materials; selected "<Material> : <Action> - Tool Holder N"; per-holder tabs; per-tool sub-tab (e.g. Kraft Cut). Parameters: Force (slider+number, e.g. 20), Speed (4), Passes (1), Accel. (3), Blade Depth (dial, for ratchet/auto blade), checkboxes Line Segment Overcut, Track Enhancing, Intelligent Path Technology. Kraft blade adds Loop Force, Loop blade height, Loop Overcut (in), Loop Radius (in), Disable Smart Cut. Revert / Save As… buttons.
## Right panel column (Design), top to bottom — each opens a floating, collapsible, closable panel docked at the right
- Page Setup: tabs Media / Grid. "Page Setup Guide" button. Machine (Auto (CAMEO 5)), Feed Type (Auto (Manual)), Cutting Mat (Auto (CAMEO)), Media Size (Auto (12x12)). Media Customization: Custom Width/Height sliders+fields (12.000 in), "Constrain Media to Cutting Mat", Portrait/Landscape, page Transparency %, page colour swatches + spectrum, Rotate View (0/90/180/270).
- Fill: tabs Solid / Gradient / Pattern. Recently Used + eyedropper; Fill Color swatch grid (greys + hue × tint) with "no fill" swatch; named custom palettes ("New Palette Name" +); Transparency %; Custom Color spectrum + brightness slider; HEX field (modes dropdown).
- Line Style: tabs Style / (line colour). Style (solid/dash dropdown), Thickness pt, Corner (miter/round/bevel), End Caps (butt/round/square), Position (centre/inside), "Print Lines of Selected Shapes" (0 pt cut lines print as 0.1 pt, also in SVG/PDF exports).
- Trace: Select Trace Area; Threshold %, Despeckle Threshold %; Advanced (filters); actions Trace / Trace Outer Edge / Trace and Detach.
- Transform: tabs Align / Scale / Rotate / Move. Align: Center (center to page, center on each other), Horizontal L/C/R, Vertical T/M/B, Distribute H/V.
- Replicate: tabs Replicate / Nesting(?) / Radial. Duplicate left/right/below/above; Rows and Columns (row of 3, row of 4, column of 3, column of 4); Fill Page; Mirror (below/above/left/right); Rotate Copies (2/3/4/6? radial copies).
- Text Style: tabs Text / Spelling. Font name + size (pt); Frequently Used / Recently Used / All fonts lists; preview-text field; B / I / U; style dropdown; alignment L/C/R/justify; horizontal vs vertical text direction; Character Spacing section: Line Spacing %, Character Spacing %, Kerning toggle.
- Print & Cut: Enable Registration Marks (explanation text); Advanced Settings Length 0.787 in, Thickness 0.020 in, Inset 0.625 in; More Settings; Restore Defaults; Print; Enable Print Bleed + Bleed Radius 0.050 in.
- Modify: Weld, Subtract, Subtract All, Intersect, Divide, Crop; Compound Paths Make / Release; Detach Lines.
- Offset: Offset / Internal Offset modes; Distance (0.079 in), Corner (sharp / round), Corner Limit (1.000); Cancel / Apply (live preview).
- Sketch & Stipple: tabs Sketch / Stipple / (pattern). Sketch Fill patterns (cross-hatch, grid, lines, concentric, spiral); Spacing (0.079 in), Angle °; Release Effects.
- Specialty Tools: tabs for tool types (emboss/deboss, foil, sketch pen?, etc. + "3D"); Deboss / Emboss toggle; Embossing Patterns (same 5 hatch patterns); Spacing, Angle; Release Effects.
- PixScan: opens camera/scan calibration flow (dialog).
- Image Effects: "The graphics card installed in this computer does not provide support for Effects to be displayed." (needs GPU).
- Pop up: On / Off; Pop up Options Base Width %, Strut Width (0.079 in), Dash Pitch (0.020 in) — generates pop-up card struts.
- Default units: inches.
## Tool rail flyouts
- Line Tools: line, polyline, curve (bezier), arc. Shape flyout: rectangle, rounded rectangle, ellipse, polygon. Freehand Tools: freehand, smooth freehand. Also: point edit, text, text box/notes, eraser, knife.
## Behaviour checks
- Drawing a rectangle shows live dimension callouts (width below, height left, in inches) while dragging; they stay attached to the selected shape.
- Selection: bounding box with 8 handles + green rotation knob above; options bar (Select tool) shows fill/line swatches, line width pt, then W / H (in) with aspect lock, X / Y position with reference-point picker, align, center-to-page, group/ungroup, arrange, weld, offset, replicate, delete.
- Offset panel acts on the selection with a live preview (default distance 0.125 in, round corners); the dimension callouts update to the offset result (5.469→5.719 in) before Apply. Cancel discards.
## Preferences (Ctrl+K) — tabs General / Defaults / Display / Import / Tools / Updates / Advanced; Apply / Cancel / OK
- General: Language (Default (English)); Default Save Location (Hard Disk / Library); Unit of Measurement (Auto (Inches)); Show Dimensions (on); Show Print Preview (on); Print Resolution 300/600 (default)/1200/custom dpi.
- Defaults: Page Orientation (Choose Automatically), Blade Type (Choose Automatically), Default Page Color, Default Fill Style (Solid Fill / Outline Only (default) + default line/fill colours), Default Text Style (font, size, B/I/U), Always Display Mat, Cut to Edge of Page, Registration Marks (default off), Scrollbar Position (Left/Right).
- Display: Antialiasing (8 Sample, restart required), Current Theme (Light; theme choice exists), Background Color (Current Theme), Button Size slider Small–Large, Animation speed Slow–Fast, Curve Quality 0–12 (restart required).
- Import: GSD open As-is / convert white lines to black / join lines into polylines; DXF open Fit to Page / convert white lines to black (on) / join lines into polylines (on); PNG Autotrace on import (on).
- Tools ("Action After Tool Use"): After Creating a Shape → Continue Drawing Shapes; after Freehand/Eraser/Knife → continue same tool; after Zoom / Eyedropper → Choose Select. Selection Tools: When Drag Selecting → select shapes touching drag box; When Many Shapes Selected → Single Bounding Box; Right-click subsequent shapes → New Shape Becomes Selected. Editing Tools: Show Bezier Control Handles for → selected points only.
- Advanced: Restore Factory Defaults, Restore Pre-loaded Designs, Unhide All Dialogs, Set Library Permissions, OpenGL Settings; Packet Size Auto (1000); Use IME; Include Cut Data; Software Overcut (on); opt-in performance data collection (off); Silhouette Device Connections: do not auto-release / auto-release when not in use.
## Remaining checks (2026-11-02, SS 5.0.539 Basic)
- Object > Arrange: Bring to Front (Ctrl+Shift+]), Send to Back (Ctrl+Shift+[), Bring Forward (Ctrl+]), Send Backward (Ctrl+[).
- Object > Align: Align Center-Middle; Align Left/Center/Right; Align Top/Middle/Bottom; Space Horizontally / Space Vertically (even distribution only — no spacing-by-value control); Center to Page.
- Object > Modify submenu: Weld (Ctrl+Shift+W), Intersect, Subtract, Crop, Subtract All, Divide, Detach Lines.
- Point editing (double-click a shape, or point-edit tool): context bar + "Point Editing" panel. Selected Point: Delete Point, Break Path, Corner, Smooth. Selected Line: Make Flat, Make Curve. Simplify. Convert to Path. Node Count readout: shapes selected, contours, node count, linear nodes, bezier nodes. No "join path" command was found in Basic.
- Not found anywhere in Basic's menus/panels: a measure tool, object lock, a layers panel (lock layer / move to layer), mat-free cutting option in Send.
## Transform panel tabs (2026-11-02, SS 5.0.539 Basic)
- Scale: preset buttons 33% / 50% / 100% / 200% / 300%; Scale % slider+field with Apply; Specify Dimensions Width / Height sliders+fields with aspect lock and Apply.
- Rotate: Rotate To (absolute) 0 / 90 / 180 / 270 buttons + custom angle slider/field; Rotate By (relative) +45 / +90 / 180 / −90 / −45 buttons + custom slider/field; Apply.
- Move: Move By arrow buttons left/right/up/down with a Distance (default 0.125 in); Move To X / Y sliders+fields with a Center Point (reference corner/centre) picker; Apply.
- Application is slow under host load (~6 s per action); selection state lags the click.
## Replicate panel tabs
- Tab 1 Replicate (presets; see above).
- Tab 2 Advanced Replicate: Number of Copies (slider+field); Position each Copy: on top / left / right / above / below / custom angle (dial + degree field); Apply.
- Tab 3 Object on Path: drag the selected object onto another shape with a grab handle ("Show Grab Handle"); Angle: Perpendicular (checkbox, on), Start Angle °, Increment Angle °; Position: Start Position (in), Section Length (in); spacing mode radio: Number of Repeats (default 50) OR Step length (default 1.969 in); Release Copies (turn into independent objects). NEW — not in our inventory.
## Fill panel tabs
- Fill Gradient: preset gradient swatch grid + named custom palettes ("New Palette Name" +); Transparency %; gradient editor bar with draggable colour stops (each removable via ×; add by clicking the bar), stop colour from swatches / spectrum / HEX; Rotate Gradient angle °.
- Fill Pattern: built-in raster pattern library (vendor artwork — not copied); Transparency %; More Options: Aspect Ratio keep vs stretch, Mirror Pattern horizontal/vertical, Scale %, Pan Pattern (drag offset), Rotate 0/90/180/270.
## Line Style tab 2 — Line Color
- Recently Used + eyedropper; Fill Color grid (greys + hue×tint); named palettes; Transparency %; Custom Color spectrum + brightness strip; HEX field (mode dropdown). Line colour drives Send "Cut by Line Color".
## Specialty Tools tabs (tool-specific operations)
- Tab 1 (blade/sketch icon): Sketch/emboss fill (seen earlier).
- Score & Emboss: Fine vs Wide tip; Passes (checkbox + count); Fill (checkbox: emboss-fill the shape interior); "Apply Score & Emboss Lines".
- Power Engraver Fills: etch patterns into solid materials (wood). Pattern buttons: cross-hatch, grid, lines, concentric, spiral; Spacing (default 0.016 in), Angle °; "Release Engraving Fill".
- Heat Pen Fills: heat-activated foil; same 5 pattern fills, Spacing 0.016 in, Angle; "Release Heat Pen Fill".
- Leather Punch (Curio 2 + punch tool): Punch Size presets 0.039 / 0.118 / 0.197 in; "Draw Punch Points" (manual tool); "Add Punch Points" (convert a selected line into evenly spaced punch points); Spacing (default 0.138 in); "Release Punch Points". NEW.
- Send to S3D: export design to Silhouette Alta 3D printer app; "Open Page Setup". Out of scope (3D printing) — note only.
## Send tab — preferences (gear)
- Feed Options: after cut → Return to Origin (default) | Feed (advance media out).
- Show Cutter Divisions: Known Cutters (default) | All Cutters (machine picker list scope).
- Show Weed settings: Auto (default) | Always | Never.
- Cut Order Sorting: No Sort | Maximize Speed | Minimize Roller Movement (default) — plus checkbox "Sort Interior Contours First" (default on).
- Stipple Sorting: Default | Edge to Center | Center to Edge.
- Defaults: "Restore Default Materials".
- Send preview: the design is redrawn as bold cut lines in the tool colour while "generating" the cut job; Tool 1 lines blue, Tool 2 red.
## Library tab (requires Silhouette account; "Sign in later" returns to Design — not bypassed)
- Onboarding screens describe: account-based library; switch accounts or work offline via the profile icon; folder tree All Designs (count) / Favorites / Fonts / Library / Patterns / Recycle Bin; File Types filter, Sort by, A→Z / Z→A; search box; thumbnail size slider; grid vs list view; settings gear.
- Cloud sync: designs synced to account and stored locally for offline use; per-item sync badges (synced own design / not synced / store purchase that can't be un-synced); star = Favorite.
- Posture: local design library (folders, favourites, search, thumbnails, trash) = replicate; cloud sync/account = out of scope.
## New Project Wizard (File > New Project Wizard, Ctrl+Shift+N)
- Project types: Blank Document, Card, Emboss, Gift Tag, PixScan, Print & Cut, Sketch, Stamping, Stipple. Pick one + Start → new tab pre-set for that workflow.
- Card: creates a 10 × 7 in card outline (W 10.000 H 7.000, centre X 5.5 Y 4.0 — X/Y here reported at centre? reference point shown) with a dashed vertical centre fold (score) line; group of outline + fold; opens Page Setup with "Constrain Media to Cutting Mat" unchecked.
- Posture: replicate as project templates (card with fold score, gift tag with hole, emboss/sketch/stamp/stipple/print&cut starters) built from our own geometry.

## Built-in project starters (2026-11-02, SS 5.0.539 Basic)

Two entry points create a new document pre-set for a kind of project.

**New Project Wizard** (File > New Project Wizard, Ctrl+Shift+N): a grid of project types, pick one and Start.
Blank Document, Card, Emboss, Gift Tag, PixScan, Print & Cut, Sketch, Stamping, Stipple.
Card creates a 10 × 7 in outline with a dashed centre fold (score) line, grouped.

**Home screen "What will you create today?" carousel**: illustrated starter projects, paged with arrows (each click moves it a small step and the app is slow, so page slowly). In order:
New Document, Vinyl Sticker, Heat Transfer T-Shirt, Sticker Sheet with Print & Cut, Greetings Card, Gift Tag, Stencil, Sketched Message, Foil Stamp Card, Foil Stamp Notebook, Acrylic Light, Metal Engraved Framed Decor, Embossed Card, Embossed Sticker, Coin Case, Faux Leather Tray, Leather Earrings, Leather Keychain (18 in all; the carousel stops at Leather Keychain). The mouse wheel over the carousel scrolls it much faster than the arrows.

Posture: replicate the set of starters, not the carousel UI (an arrow-paged strip that shows five at a time is slow to browse). StencilCraft uses a searchable, category-grouped grid in a new-project wizard of our own templates (our own geometry, names and thumbnails; none of the vendor's artwork or photos), each setting page/mat, operation (cut, score, sketch/pen, foil, emboss, engrave), mirror for heat transfer, and print-and-cut marks as the project needs.

Stability: Silhouette Studio exited without an error dialog twice under rapid input (opening a View submenu; fast carousel clicks) on a loaded host. The VM had 5.6 GB disk and 2.1 GB RAM free, and no application-log entry was written.

## Menus, dialogs and behaviour (2026-11-03, SS 5.0.539 Basic, Tiny11 VM)

This build shows no STORE tab: the top bar has DESIGN / LIBRARY / SEND only.

- Help menu: User's Manual…, Introduction…, Page Setup Guide, Tutorials…, Online Tutorials, Software License Agreement…, SilhouetteAmerica.com, Enable Starter Mode | Apply Upgrade Edition, Manage Account | Debug Log… | About Silhouette Studio…
- View > Zoom: 25/33/50/75 | 100% | 150/200/400 | Fit To Window (Ctrl+1); current zoom checked (both 100% and Fit checked at once). (Tiny11 build has no STORE tab/Show Store item.)
- View > Rotate Page: 0° (checked) / 90° / 180° / 270° — rotates the on-screen view of the page, same as Page Setup's Rotate View.
- View > Grid Options… (Ctrl+F4) opens a docked "Grid" panel: Show Print Border, Show Cut Border (on), Show Grid, Snap to Grid, Enable Smart Snapping (on), Smart Snap only within media; Tolerance slider+field (0.059 in); collapsible Grid Settings: Square | Isometric grid type, Spacing (0.394 in = 10 mm), Divisions (1), grid colour tabs Primary Lines / Secondary Lines (swatches + spectrum + HEX).
- View > Registration Marks… (Ctrl+F3) = the Print & Cut panel. Enabling it draws on the page: a filled black square at top-left, L-shaped corner brackets at top-right and bottom-left (Type 1 style, 3 marks), a hatched no-design zone along the top band and round each mark, and a blue inner rectangle = printable/cuttable area inside the marks. More Settings (expands): mark style Default (corners pointing out) | Inverted; per-edge Left / Top / Right / Bottom Inset (0.625 in each, sliders + fields). Restore Defaults; Print; Enable Print Bleed + Bleed Radius 0.050 in (disabled until bleed on).
- Panels > Select by Color: mode toggle By Line | By Fill; a list of the document's colours (empty on an empty page); Options: Include Groups (on), Only Shapes on Media (on).
- Right-click on a selected shape (context menu): Undo <action name> ("Undo Create 3 Shapes") / Redo (greyed "Nothing to Redo") | Cut, Copy, Paste, Paste in Front, Duplicate, Delete | Flip Horizontally, Flip Vertically | Edit Points, Weld, Offset | Group, Ungroup (greyed when not applicable) | Send to Back, Bring to Front, Bring Forward, Send Backward. Undo/redo items name the action.
- Select by Color, By Line, after drawing a default rectangle: one row, a blue swatch labelled "RGB (255,0,0)" (label channel order appears swapped vs the swatch; display quirk). Default new-shape line colour is blue.
- Select-tool options bar with a shape selected: fill swatch, line swatch, line weight pt; W/H (in) with lock; X/Y (in) with reference-point picker; then align, centre, group/ungroup, arrange (4), weld, replicate/star, 3D-ish (offset?) and delete (X) icons.
- Object > Mirror: Flip Horizontally, Flip Vertically.
- Object > Rotate: Rotate by 45° Clockwise, by 90° Clockwise, by 180°, by 45° Counter Clockwise, by 90° Counter Clockwise | Rotate to 0°, 90°, 180°, 270°.
- Object menu, a plain rectangle selected: Group, Ungroup, Make Compound Path greyed; Release Compound Path enabled (a rectangle counts as a compound path of one).
- Object > Replicate (with shortcuts): Duplicate Left/Right/Above/Below = Ctrl+Left/Right/Up/Down Arrow | Mirror Left/Right/Above/Below = Shift+Alt+Left/Right/Up/Down Arrow | Rotate One Copy (Ctrl+Shift+F1), Rotate Two Copies (Ctrl+Shift+F2), Rotate Three Copies (Ctrl+Shift+F3), Rotate Five Copies (Ctrl+Shift+F5) — i.e. radial totals 2/3/4/6 | Row of Three (Ctrl+Shift+Right), Row of Four (Ctrl+Alt+Right), Column of Three (Ctrl+Shift+Down), Column of Four (Ctrl+Alt+Down), Fill Page (Ctrl+Shift+F).
- Object > Modify with one shape: all booleans greyed (need 2+), Detach Lines enabled.
- Object > Align: Align Center-Middle | Align Left/Center/Right | Align Top/Middle/Bottom | Space Horizontally/Vertically (greyed with <3? shapes) | Center to Page. Align items stay enabled with a single shape selected.
- Selection hit-test: clicking inside an unfilled (outline-only) shape does NOT select it; you must click its outline.
- Rotated objects: W/H in the options bar keep the unrotated (object-local) size; X/Y report the axis-aligned bounding box's top-left (1in square at 1,1 rotated 45° -> X=Y=0.793). Selection handles and dimension callouts rotate with the object.
- Select by Color labels colours in B,G,R order: an ochre swatch (184,124,27) reads 'RGB (27,124,184)', a green (84,169,35) reads 'RGB (35,169,84)', the default blue line reads 'RGB (255,0,0)'. Swatches themselves are correct. (Display bug; our UI should label R,G,B.)
- Page Setup: typing Custom Width 10 / Height 7 switches Media Size to 'Custom' and auto-selects Landscape (orientation follows W>H); the canvas re-zooms and the mat shows a 1-inch grid with numbered rulers along its edges beyond the page.
- File > New (Ctrl+N) inherits the page setup of the last document (a new tab after a 10x7 custom page opened at 10x7).
- Multi-selection: callouts show the combined bounds (1.924 in) but the options-bar W/H show a single object's size (0.700) — X/Y show the combined top-left. With 2+ selected the bar adds an align-centres icon.
- Text tool: click on canvas places a text box and types immediately (Arial, style Default, 72.00 pt). Edit mode shows a green frame around the text, a red/yellow vertical bar at the right edge (width handle), and a crosshair anchor at the baseline start. Options bar for text: font, style, size, B/I/U, alignment, then the general align/arrange/weld icons.

## Onboarding, Page Setup Guide, trace and import (2026-11-23, SS 5.0.556 Basic, Windows 11 Pro on hardware)

Driven over RDP on a physical Windows 11 Pro machine (Intel HD P530; OpenGL is required, the app exits at startup with a driver error when only a non-working OpenGL driver is available). Test images were original PNGs drawn for this session (red circle, blue rectangle, green triangle, black bar on white, 600 × 400 px).

- First launch asks, in order: automatic update checks (Check / Do not check), an opt-in "Performance Improvement Program" checkbox that is **checked by default**, then the EULA. We chose no update checks (to pin the version) and opted out of data collection.
- After "How well do you know Silhouette Studio?" → Experienced, a 7-page "Welcome" tour shows: New Home Screen (recent files, project tutorials, discover ideas), Project Tutorials ("15 common project types"), Starter Mode (toggle between a starter-friendly and an advanced UI), New Tool Support (Specialty Tools panel for new tools), Send Guide (follow-along connect/load/send instructions), New Look ("cleaner, clearer, more accessible"), then an end page offering Page Setup Guide or Skip to Design.
- **Page Setup Guide** (a wizard, also reachable from the Page Setup panel): "What machine will you be using?" Portrait Series / CAMEO Series / CURIO Series, plus "I don't have a machine yet" → model grid (CAMEO series, as labelled: CAMEO 5a, CAMEO 5a Plus, Cameo Pro MK-II, CAMEO 5, CAMEO 5 Plus, CAMEO 4, CAMEO 4 Plus, CAMEO 4 Pro, CAMEO 3 or older) → "What Feed Type will you be using?" Manual Feed / Electrostatic Mat / Auto Sheet Feeder (picture cards) → "What mat will you be using?" list with a live mat preview: Auto (CAMEO), None, Electrostatic Mat 12 inch (12 × 12 in), CAMEO (12 × 12 in), CAMEO (12 × 24 in), Portrait 4 (8.5 × 12 in), Stamp (6 × 8 in), PixScan CAMEO (11.7 × 8.5 in), Emboss Mat (12 × 12 in); hint: use Auto if unsure → "What media will you be using?" Auto (12x12), Custom, 12x12, Letter, A4, A5, … with a page preview; hint: the size is on the packaging or can be measured on the mat → "Thank you…" / Get Started. Every step has Back; each says "You can change this later using Page Setup."
- **Starter mode differs from 5.0.539.** On: the full quick-access toolbar stays, with large icons; the right column shows 8 labelled panel buttons (Page Setup, Fill Style, Line Style, Trace, Transform, Replicate, Fonts, Print & Cut) and a "+ Show more…" button. Off: compact unlabelled icons, about 15 panels (adds Modify, Offset, Sketch/Stipple, Specialty Tools/Pop up, PixScan, Image Effects, …) and extra toolbar buttons (zoom to selection, pan, fit). Starter mode no longer hides the toolbar.
- **Raster import size.** Open (Ctrl+O) and Merge (Ctrl+Shift+O) of a 600 × 400 px PNG both placed it at **2.000 × 1.333 in (300 dpi)** at X 0, Y 0 (page top-left), whether the PNG had no DPI metadata or a `pHYs` chunk saying 150 dpi. The embedded DPI is ignored. Open makes a new document tab named after the file; Merge adds to the current one.
- No auto-trace happened on Open or Merge of a PNG (the 5.0.539 notes list "PNG Autotrace on import (on)" as a preference); the image stayed a single raster object.
- **Trace panel, live preview.** In Basic the panel has one tab. Threshold/Despeckle and the actions stay disabled until Select Trace Area; you then drag a box on the canvas. The box is resizable, has an × button to remove it, and everything that will be traced is painted **solid yellow** over the image. Threshold defaults to 45.0 %, Despeckle 0.0 %. Moving the Threshold slider updates the preview live: at 8 % only the black bar is yellow, at 95 % every coloured shape is — threshold is a darkness cutoff.
- Trace › Advanced: High Pass (Off) 0.00, Low Pass (Off) 0.00, Scale 10, Trace Quality a 5-stop slider Low…Max (middle default, hint "a healthy balance between speed and memory consumption"), and **Trace Preview Display: Solid Fill | Outline**. Outline dims the image and draws the would-be cut lines in yellow.
- Trace result: one **compound path** of red, unfilled, 0.00 pt lines; the source image is kept unchanged underneath. Four shapes became one selection W 1.740 × H 0.968 in at X 0.130, Y 0.267 — within about 0.003 in of the source pixel edges at 300 dpi (slightly outside). After tracing, the trace area is cleared and the panel greys out again. No colour/multi-layer trace option exists in Basic.
- Send tab, no machine connected: unchanged from 5.0.539. The machine picker lists only the default ("CAMEO 5 (Default)"); no Send, Test, Load/Unload or calibration controls are shown until a machine is connected, so those stay unobserved. The Send gear menu is the same as 5.0.539 (no calibration entry).
- Send preview line colours: Tool 1 lines are drawn **red** and Tool 2 **blue** (the overlay buttons' icons use the same colours). This is the reverse of the 5.0.539 note above; unverified whether the version changed it or the earlier note was swapped.

## Drawing and point-editing tools (2026-11-24, SS 5.0.556 Basic)

Driven through the UI; cross-checked against the in-app User's Manual (Help › User's Manual, manual no. SS-UM-151, ch. 04 "Drawing/Editing Images").

- New lines and shapes get a red line (0.00 pt) by default in this install. After a shape is drawn the same tool stays active and the new shape is selected (W/H/X/Y in the options bar).
- Line tool: drag start → end. Shift constrains to horizontal, vertical or 45° steps from the start point (manual).
- Polyline ("Polygon" in the manual): each click drops a corner point; endpoints show as red dots, interior points as grey squares. Clicking the start point closes the shape; a double-click ends an open polyline at the double-click position. Shift constrains each segment to 0/45/90° from the last point (manual).
- Curve ("Curved Shape") tool: each click drops a point and the curve passes smoothly through every clicked point (interpolating, no visible handles while drawing); the last segment previews to the cursor; double-click ends it, clicking the start closes it.
- Arc tool: press at the centre and drag to the start point — this sets the radius (a guide line labelled with the radius, e.g. "4.375 in", and the start angle "0°"). Then move the pointer: an arc of that fixed radius sweeps after it with a live angle readout and two radius guide lines; a click commits an open arc.
- Rectangle / rounded rectangle / ellipse: Shift = square/circle, Alt = the press point is the centre (verified: Shift-drag gave 2.450 × 2.450 in). Rounded rectangle: two red control points at the top-left corner adjust the horizontal and vertical corner radius; Shift-drag one to move both (manual).
- Regular polygon: 5 sides by default with a slider to change it (manual).
- Freehand / smooth freehand: a continuous stroke until release; closes when the end meets the start; the smooth variant has no sharp angles (manual).
- Edit Points: entered by double-clicking a selected shape or with the Edit Points tool; exited by double-clicking again or choosing Select. Only a single ungrouped shape can be point-edited (manual). The options bar becomes a point toolbar (Delete, Break, Corner, Smooth, Make Flat, Make Curve, Simplify, Convert to Path, "Show Panel").
- Point Editing panel: Selected Point (Delete Point, Break Path, Corner, Smooth — disabled until a point is selected; the current point type is highlighted), Selected Line (Make Flat, Make Curve), Simplify (tooltip: turns shapes made of many straight lines, such as GSD designs, into fewer smooth Bézier curves), Convert to Path, and a Node Count readout (shapes selected, contours, node count, linear nodes, Bézier nodes).
- A selected point is a white square with blue Bézier handles; the segment after it is drawn bold red; other points are grey squares, open-path ends red dots. Hovering a segment where no point exists lets a click add a point; right-clicking a point offers Delete Point / Break Path; dragging one open end onto the other rejoins the path (manual).
- Knife tool: options Solid | Outline (Outline default), cut line type ("Straight"), a gap-width slider (0.000), Auto Apply (on) and Apply. It acts on the selected shape(s) — with nothing selected the drag does nothing. While dragging it shows the cut line with its length/height callouts. Outline mode leaves separate **open** paths split where the cut crosses; Solid mode leaves separate **closed** pieces (two triangles from a diagonal cut of a rectangle).
- Eraser tool: options Solid (default) | Outline, brush shape square | round (round default), Size slider (0.866). The cursor is a translucent blue disc of the brush size. A Solid stroke straight across a rectangle removed a band and left two closed rectangles.
- Clicking inside an unfilled shape does not select it (only its outline does), so tools that act on the selection silently do nothing after such a click.

## Text tool (2026-11-25, SS 5.0.556 Basic, Windows 11 Pro on hardware)

- Text tool, click the page and type: the click point is the first line's baseline start (the
  ⊕ text-to-path handle sits there). Letters appear as red cut outlines while typing, inside a
  green text box with a blue wrap bar on its right. Default font Arial, style Default, 72 pt;
  the options bar switches to font, style, size, B/I/U and alignment.
- Esc ends editing, keeps the text, selects nothing and returns to the Select tool. Each typed
  letter is its own undo step (Ctrl+Z after Esc first re-enters editing, then removes letters
  one at a time).
- Text Style panel: font search field and size, font lists (Frequently Used, Recently Used,
  All fonts), a preview-text field, B/I/U and a style list, left/centre/right/justify,
  horizontal/vertical, Line Spacing 100 %, Character Spacing 100 %, Kerning on.
- Character Spacing is linear: with Arial at 72 pt the gap added between letter cells was
  37 px at 200 % and 74 px at 300 % for an em of about 101 px, i.e. 0.366 em per 100 % above
  normal.

## Detach Lines (2026-11-25, SS 5.0.556)

- Object › Modify › Detach Lines is enabled with one closed rectangle selected. Running it on
  the rectangle (with or without a stroke weight) leaves one object: dragging its top edge
  afterwards moves the whole rectangle.
- With a fill and a line (rectangle 4.813 × 4.375 in, yellow fill, blue line 20 pt): Detach
  Lines leaves two objects, selected together. The line becomes its own shape: the stroke's
  outline as a closed ring (5.091 × 4.653 in, i.e. half the 20 pt = 0.278 in line width on each
  side), filled with the old line colour, line width 0, no line colour. The original keeps its
  fill and its size and no longer shows a line. Dragging the ring afterwards moves it alone.

## Preferences › Import and General (2026-11-25, SS 5.0.556)

- Import tab, "Import Options": GSD: Open (As-is), Convert white lines to black (off), Join
  lines into polylines (on). DXF: Open (Fit to Page), Convert white lines to black (on), Join
  lines into polylines (on). PNG: Autotrace (on). There is no import DPI or scale setting.
- General tab: Language, Default Save Location (Hard Disk / Library), Unit of Measurement
  (Auto (inches)), Show Dimensions (on), Show Print Preview (on), Print Resolution (300, 600
  (chosen), 1200 dpi, or Custom DPI with a number, 300.00).

## PNG Autotrace on open (2026-11-26, SS 5.0.556)

- With Preferences › Import › PNG Autotrace on (the default), File › Open of an opaque PNG
  (coloured shapes on a white background) places the image in a new tab at the page's top
  left with no cut lines: the Send view shows no lines.
- The same picture with the white made transparent comes in with a red cut line round every
  opaque shape (circle, rectangle, triangle, bar), hugging the edge of the opaque pixels.
  Autotrace traces the outline of a PNG's opaque area; it does nothing on a PNG without
  transparency.

## Object › Replicate presets (2026-11-26, SS 5.0.556)

- Menu: Duplicate Left/Right/Above/Below (Ctrl+Arrow); Mirror Left/Right/Above/Below
  (Shift+Alt+Arrow); Rotate One/Two/Three/Five Copies (Ctrl+Shift+F1/F2/F3/F5); Row of Three
  (Ctrl+Shift+Right), Row of Four (Ctrl+Alt+Right), Column of Three (Ctrl+Shift+Down), Column of
  Four (Ctrl+Alt+Down), Fill Page (Ctrl+Shift+F).
- Row of Three on a 2.188 in square: three objects in all, 6.563 in wide (3 × 2.188 = 6.564):
  the copies touch, no gap.
- Rotate Three Copies on a 2 × 0.16 in bar: four bars about the bar's own centre at 0°, 45°,
  90°, 135°; Rotate Five Copies: six bars 30° apart. n copies are 180°/(n+1) apart (they fan
  over half a turn).

## Preferences: Defaults, Display, Tools, Advanced (2026-11-26, SS 5.0.556)

- Defaults: Page Orientation (Choose Automatically), Blade Type (Choose Automatically),
  Default Page Color (swatch), Default Fill Style (Solid Fill | Outline Only (chosen), with fill
  and line colour swatches, black and red), Default Text Style (Arial, 72.00, B/I/U), Always
  Display Mat (off), Cut to Edge of Page (off), Registration Marks (off), Scrollbar Position
  (Left | Right (chosen)), Panel Mode (Flexible Panel Mode).
- Display: Antialiasing (8 Sample, restart required), Current Theme (Light), Background Color
  (Current Theme), Button Size (Small–Large slider), Animation (Slow–Fast slider), Curve Quality
  (0–12 slider, restart required).
- Tools › Action After Tool Use: After Creating a Shape (Continue Drawing Shapes), After Drawing
  Freehand (Continue Drawing Freehand), After Using Eraser (Continue Using Eraser), After Using
  Knife (Continue Using Knife), After Using Zoom (Choose Select), After Using Eyedropper (Choose
  Select). Selection Tools: When Drag Selecting (Select shapes touching drag box), When Many
  Shapes Selected (Single Bounding Box), When Right-clicking Subsequent Shapes (New Shape Becomes
  Selected). Editing Tools: Show Bezier Control Handles for (Selected points only), When Using
  Subtract and Subtract All (Convert thick lines to polygons), When Using Eraser and Knife
  (Maintain thick lines as lines), When Pasting an Object (New Shape Becomes Selected), Offset
  Lines (Set to 0pt red lines).
- Advanced: Restore Factory Defaults…, Restore Pre-loaded Designs…, Unhide All Dialogs, Set
  Library Permissions…, OpenGL Settings…; Packet Size (Auto (1000)); Use IME (on), Software
  Overcut (on), Include Cut Data (on); Enable Performance Improvement data collection (off);
  Silhouette Device Connections: Do not auto-release connections (chosen) | Auto-release when
  connections are not in use; OpenGL Configure (WGL_SWAP_COPY / EXCHANGE / UNDEFINED / None).

## Panels › Pop up (2026-11-26, SS 5.0.556)

- The panel: Pop up On | Off (Off chosen), and Pop up Options greyed while off: Base Width
  (33.3 %), Strut Width (0.079 in), Dash Pitch (0.000 in). On's tooltip: "Select shapes and
  click here to convert them to a Pop up."
- On with a 2.625 in square selected converts the square itself: the options become Base
  Width 10.0 %, Strut Width 0.079 in, Dash Pitch 0.039 in, and the selection grows to
  2.673 × 3.039 in. A strip is added above the square, joined to it by a blue dashed fold line
  along the square's top edge, with another dashed fold along the strip's top; the red cut
  line runs round square and strip; short dashed folds appear at the square's bottom
  corners.
- A second sample, 5.250 × 1.750 in, became 5.306 × 2.170 in: the strip above adds about
  0.42 in whatever the shape's size (0.414 in on the square). Raising Base Width to 50 % left
  the bounding box unchanged and lengthened the dashed fold segments along the bottom edge
  (one at each end). The converted shape shows red handles at the middle of its left and
  right sides, likely for dragging the fold.

## Cut to Edge of Page, Pop up's Strut Width (2026-11-26, SS 5.0.556)

- Preferences › Defaults › Cut to Edge of Page on: the red cut border moves from just inside
  the page (1.5 mm) to the page's own edge.
- Pop up's Strut Width raised to 0.5 in on a converted rectangle changed neither its bounding
  box nor any line visible at that zoom.

## Panel Mode (2026-11-26, SS 5.0.556)

- Preferences › Defaults › Panel Mode offers Flexible Panel Mode (the default), Single Panel
  Mode and Multiple Panel Mode.
- Single: opening Panels › Modify after Panels › Transform leaves only Modify open.
- Multiple: the same two open stacked in one column, Modify above Transform, each with its own
  header and close box.

## GSP files (2026-11-26, SS 5.0.556)

Saved from File › Save As with "Save as type: GSP" (the list offers Silhouette Studio V3,
Silhouette Studio V2 and GSP). A 1 in square saves as 1220 bytes. Layout, from saving one line
or rectangle at a time and comparing:

- 0x00: the ASCII magic `GRAPHTEC PRT&CUT`, then little-endian u32 fields. At 0x80, eight
  u32s: the length of the cut program below, the design's bounding box (x min, y min, x max,
  y max, in 1/20 mm: a 2.188 in line reads 1111), the mat size (6096 × 6100, 12 in), then 1.
- From 0x100 everything is XOR 0xA9.
- The decoded body starts with a Graphtec GP-GL cut program, commands ended by 0x03: `TG1`,
  `FN0`, `TB50,0`, `TR0,1`, `\0,20`, `Z6096,6100`, `J1`, `FC0,1,1`, `!0,1`, `APS30`, `FE0,1`,
  `FF0,0,1`, then per path `M y,x` (absolute, 1/20 mm) and a binary relative draw: `BE1`
  (2 bytes), `BE2` (3 bytes) or `BD3` (5 bytes) per segment, by length. Lines start at
  their top or left end. Then `L0`, `\0,0`, `M0,0`, `TR0,0`, `J0`, `FN0`, `TB50,0`.
- In a `BE2` code (the 3 bytes as a little-endian u24) the high 12 bits are 0x900 plus the
  horizontal step × 1.07 (signed); the low 12 bits follow the vertical step but are not yet
  decoded. The design itself has no other coordinates: after the program come tagged records
  (page 297 × 210 mm, "Graphtec CC330L", "ISO A4", blade settings as f64), a `TD` record and
  a 40 × 40 one-bit thumbnail.

## Cut border on send (2026-11-26, SS 5.0.556)

- A rectangle drawn half off the page's left edge shows in the Send view as two parts. The part
  inside the cut border is bold red (cut): its top, right and bottom edges. The part outside is
  pale (not cut). No line is added along the border, so the clipped shape is cut open.

## What a `.studio3` writer must produce (2026-11-26, SS 5.0.556)

Tested by opening edited copies of documents saved here (not shipped anywhere):

- The u32 at 0x3E8 is the same constant (0x1DCD672C) in every file, not a checksum.
- Of the 13 sections listed at 0x10 (u64 lengths), only section 0, the objects, is needed. A
  copy with every other length set to 0 and those sections removed opens with all its shapes.
- Section 0 is read in a fixed order. Keeping only the page block and the objects block (or
  the objects block alone) gives "The file you are attempting to access could not be loaded.
  It may be corrupt." So a writer must emit every root block in order: page, document
  settings, the objects block, and the run of cut-layer and material records (`layer_@cnon`,
  `@ccut`, `@cced`, `@cper`, `@cped`, `@weed`…, about 8 KB), plus the 2.5 KB that follows
  the root block.

## `.studio3` section 0 map, empty document (2026-11-26, SS 5.0.556)

Groundwork for a writer that generates every byte itself (no bytes copied from Silhouette's
output). An empty Letter document's section 0 is `silhouette05;` then one root record
(`01`, u32 length counting itself) of about 5.8 KB. Records within it are read in this order.
Offsets are into section 0.

| At | Length | What it appears to be (from contents; to confirm by saving variations) |
| --- | --- | --- |
| 18 | 56 | Default line style: f32 25.4 (`33 33 cb 41`), counts, the join name "square", RGBA f32 colours (0.5 grey, alpha 1) |
| 79 | 518 | Page and mat: four f32 15.875 (`00 00 7e 41`, page margins?), sub-records of 67, 5, 5, 160, 24, 17, 29 and 6 bytes, the mat name `constant:12x12`, "auto", RGBA white |
| 602 | 69 | View or transform state: f32 1.0 values, a 32-byte sub-record |
| 676 | 283 | Document info: "default", the tab name "Untitled-1", f32 3.175 (`33 33 4b 40`), machine "Cameo5" (`…meo5`), "auto" twice |
| 964 | 4 | The objects record. Empty here (`ff ff ff ff`); with shapes, a count, then one record per object (665–766 bytes for a rectangle) |
| 973 | 35 | A setting "off", f32 10.0 ×4 |
| 1013 | 3154 | Cut layers: count 9, then one record per layer named `layer_@cnon`, `@ccut`, `@cced`, `@cper`, `@cped`, `@ccu2`, `@cce2`, `@cpf2`, `@cpe2`, `@weed`, each with a run of `2` (0x32) bytes and a 96- or 252-byte sub-record |
| 4172 | 98 | The `@all` group: ids and `ff ff ff ff` placeholders |
| 4275 | 873 | Identity transforms (f32 1, 0, 0, 1) and a 703-byte sub-record |
| 5153 | 19 | Small flags record |
| 5182–5826 | about 640 | The trailer: a 571-byte record with "score" and a hex id string, then small records (f32 1.27, 1.5, 20.0) |

Next experiments, one change per saved file: page size and margins (record 79), the
document's name and machine (676), one layer's settings (1013), and an object with each
property changed alone (964). Then generate each record and confirm by opening the result in
Silhouette, replacing Silhouette's bytes one record at a time.

### Decoded so far (2026-11-26)

- **Record 18 (line style), confirmed byte for byte by generating it from these fields:**
  f32 25.4, u32 4, u32 1, two flag bytes (0, 0), the length-prefixed join name `square`,
  then two RGBA f32 colours (0.5, 0.5, 0.5, 1).
- **Record 79 (page and mat), first reading only, not yet confirmed by variations:**
  - four f32 15.875 mm (the page margins);
  - the page size, f32 304.8 × 304.8 mm (12 × 12 in);
  - f32 20, 0.5 and 2;
  - the mat name `12x12` (length-prefixed), its outer size f32 324.8 × 344.8 mm, f32 10 and
    20, and its colour RGBA (0.62, 0.63, 0.64, 1);
  - `constant:12x12`;
  - a `media` sub-record with f32 25.4 sizes, 271.6 × 184.6, 15.875 ×4, 246.2 × 159.2;
  - "auto" and RGBA white.

- **Record 79, confirmed (2026-11-26) on three saved pages (12 × 12, 10 × 12 and 10 × 11 in):**
  it is regenerated byte for byte from its fields.
  - The page width and height are f32 mm. Silhouette stores 10 in as 254.00006, converting
    in f32.
  - The page's position on the mat is f32 x, y: the centring offsets, 25.4 and 12.7 mm here.
  - The media preset name is `constant:12x12`, and empty for a custom size.
  - The `media` sub-record holds the cut area, computed in f32 as the page size minus the
    stored 10 mm (f32 bits 0x411FFFEB, 9.99998).
  - The mat colour is 159/255, 161/255 and 163/255.
  - The mat fields are still the 12x12 mat's; a 12x24 mat save is next.
- **Also confirmed byte for byte (2026-11-26), regenerated from their fields:**
  - **602 (view):** pan, zoom, a sub-record, then u32 2.
  - **676 (document):** the default material, given as name, label, key, description, two
    u32 10 values, "adjustable" and step 0.1; then the document name "default", the tab
    title, f32 3.175, guide settings, "edge", the machine id (`cameo5`) twice and "auto"
    twice.
  - **964 (objects):** −1 when there are none.
  - **973:** a setting "off" with a self-sized margins struct (u32 length counting itself,
    u8, four f32).
  - **1013 (cut layers):** u32 4, u8 0, the count minus one (as path contour counts are
    stored), then ten layers `layer_@<key>:` with keys cnon, ccut, cced, cper, cped, ccu2,
    cce2, cpf2, cpe2 and weed. Each is a self-sized 298-byte body:
    - the key and a 31-character pattern string (`2` × 30, `A`);
    - flags: in use (every layer but cnon), edge (cced, cped, cce2, cpe2), second tool
      (ccu2, cce2, cpf2, cpe2);
    - two tool blocks: `no_tool`, u32 100, a 32-hex id, the pattern, `auto`, `no_action`;
    - the raw key `studio4.3extension1_dghyujnbvf` between the two blocks.
  - **4172 (`@all` group):** a count, a sized header and six entries.
  - **5153:** small integer and flag fields.
- That is 4,281 of the empty document's 5,812 root bytes.
- **Left:**
  - 4275, the "Default Layer" object (873 bytes). It shares the shape-object layout, so it
    is best decoded together with shape records changed one property at a time.
  - The trailer after 5153.
  - Whether Silhouette needs the 32-hex id and the extension key exactly, to test by
    opening a generated file.
- **Whole root generated (2026-11-26).** The empty document's root record now regenerates byte
  for byte, 4,935 of its 5,813 bytes from decoded fields; only the "Default Layer" object
  (record 4275) is still taken from the sample.
  - **The end of the root record:** two empty records, then a `score` and a `template` group.
    Each group is self-sized and holds two presets: name, index, a fixed 32-hex preset id,
    two RGBA colours (red 0.963, 0, 0 and blue 0.04, 0.424, 1) and an
    `offsetter.<group>.layer_name.<n>` key, then f32 0.3, three counts, 1 and 0. After the
    groups come small records holding f32 1.27, 1.5 and 20.
  - **The ids:** the four preset ids are the same in every file saved here. One document id
    in record 4275 changes from save to save.
  - **Only the root record is needed.** The 2.5 KB after it in section 0 (copies of the page,
    more material entries) can be dropped, and Silhouette still opens the file with its shapes.
  - **Opened in Silhouette:** a file with a generated header (magic, section length, the
    0x3E8 constant, two u64 300 000 000 values) and the generated root opens. With the page
    set to 100 × 150 mm it shows as Custom, 3.937 × 5.906 in.
- **Line style in an object record, confirmed (2026-11-26).** Five rectangles were saved, each
  with one Line Style panel setting changed. All five style records regenerate byte for byte
  from these fields:
  - u8 0, u8 1, a 2×2 matrix (f32 1, 0, 0, 1), u32 0;
  - the line width in millimetres as an f32 (2 pt is stored as 0.7056);
  - the cut-style record: key `cut`, label `Cut`, index `0`, then the line colour as RGBA f32;
  - u8 0, then the corner (join) name: `rounded`, or `corner` for the sharp corner;
  - the position flag: 1 for the line centred on the outline, 0 for behind the shape;
  - the end-cap name: `rounded`, or `flat`.

  The three enclosing record lengths follow from what they hold. A fill adds a flag and an
  RGBA colour after the cap name (fillA → fillC).
