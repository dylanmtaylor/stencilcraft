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
