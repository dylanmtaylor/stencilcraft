# Driving cutting machines

`stencilcraft-machine` encodes cut jobs for vinyl/craft cutters and, when asked
explicitly, sends them. It covers the Silhouette/Graphtec family that speaks the
ASCII GP-GL dialect over USB (vendor `0b4d`, plus `3844` for the CAMEO 5 Alpha
line), professional Graphtec CE cutters, and any HP-GL or DMPL sign cutter (Roland,
USCutter, Vinyl Express, …), and Grbl G-code lasers/knife CNCs. Cricut machines
have registry slots but are export-only — see [Cricut](#cricut). The driver
architecture (CutJob, Capabilities, Protocol, Transport, DeviceRegistry) is in
[`machine-architecture.md`](machine-architecture.md).

## Safety contract

- **Dry run by default.** `cut` prints a transcript; `--out FILE` writes the byte
  stream to a file. Neither touches hardware. Only `--device …` sends, or the explicit desktop USB Send action moves hardware. The engine also exposes
  `hardware.cut` with `send: true`; control clients must treat this as a physical action.
- **Reject, never clamp.** `encode_job` checks every value against the model's
  catalog row and returns `MachineError::Limit` naming the knob and the allowed
  range: force, speed, tool holder, tool, AutoBlade depth (0–10, holder 1), passes
  (1–10), line-segment overcut (0–2 mm), blade offset (0–2.3 mm), corner
  sharpening (0–2 mm), mat, registration geometry, non-finite or negative
  coordinates, more than 2 000 000 points, and the cut area (design + margins must
  fit the model's cut area, the mat, the loaded media and the registration area).
- **Undocumented = refused.** Where the catalog has no published force/speed range
  the encoder refuses an explicit value ("set it on the panel"); where no cut width
  is known it requires `--media-size`; where no protocol is published it refuses to
  encode at all (`export-only`).
- **The session checks who it talks to.** A real send initializes the device, reads
  the firmware string (`FG`) and aborts if it names a different model than the job
  was validated for, polls status until ready (aborts on "unloaded"), and for print
  & cut waits for the device to report the marks found before any cutting command.
- The desktop preview (`encode_cut`) still clamps force/speed into range for an
  on-screen transcript; it never sends.

## Machine catalog

`stencilcraft-cli machines` (or `--json`, `show ID`, or MCP `machines_list` /
`machine_capabilities`) prints this from `crates/machine/machines.toml`. `?` = not publicly documented. Force/speed are
device steps; "panel" means the encoder never sends them.

| id | Model | Dialect | USB | Cut W × L (mm) | Holders | Force | Speed | BT |
|---|---|---|---|---|---|---|---|---|
| `portrait` | Portrait | legacy | 0b4d:1123 | 206 × 3000 | 1 | 1–33 | 1–10 | no |
| `portrait-2` | Portrait 2 | legacy | 0b4d:1132 | 203 × 3000 | 1 | 1–33 | 1–10 | ? |
| `portrait-3` | Portrait 3 | cameo4 | 0b4d:113a | 203 × 18290 | 1 | 1–33 | 1–30 | yes |
| `portrait-4` | Portrait 4 | cameo4 | 0b4d:113f | 216 × 18290 | 1 | 1–33 | 1–30 | yes |
| `cameo` | CAMEO | legacy | 0b4d:1121 | 304 × 3000 | 1 | 1–33 | 1–10 | no |
| `cameo-2` | CAMEO 2 | legacy | 0b4d:112b | 304 × 3000 | 1 | 1–33 | 1–10 | no |
| `cameo-3` | CAMEO 3 | cameo3 | 0b4d:112f | 304.8 × 3000 | 2 | 1–33 | 1–10 | yes |
| `cameo-4` | CAMEO 4 | cameo4 | 0b4d:1137 | 304.8 × 3000 | 2 | 1–33 | 1–30 | yes |
| `cameo-4-plus` | CAMEO 4 Plus | cameo4 | 0b4d:1138 | 372* × 3000 | 2 | 1–33 | 1–30 | yes |
| `cameo-4-pro` | CAMEO 4 Pro | cameo4 | 0b4d:1139 | 600 × 3000 | 2 | 1–33 | 1–30 | yes |
| `cameo-pro-mk2` | CAMEO Pro MK II | cameo4 | 0b4d:1146 | 609 × 18290 | 2 | 1–33 | 1–30 | ? |
| `cameo-5` | CAMEO 5 | cameo4 | 0b4d:1140 | 330.2 × 3000 | 2 | 1–33 | 1–30 | yes |
| `cameo-5-plus` | CAMEO 5 Plus | cameo4 | 0b4d:1141 | 372 × 3000 | 2 | 1–33 | 1–30 | yes |
| `cameo-5-alpha` | CAMEO 5 Alpha | cameo4 | 3844:0001 | 330.2 × 3000 | 2 | 1–40 | 1–30 | yes |
| `cameo-5-alpha-plus` | CAMEO 5 Alpha Plus | cameo4 | 3844:0002 | 372 × 3000 | 2 | 1–40 | 1–30 | yes |
| `curio` / `curio-2` | Curio, Curio 2 | export-only | ? | ? | ? | ? | ? | ? |
| `craft-robo-cc200-20` | Craft Robo CC200-20 | legacy | 0b4d:110a | 200 × 1000 | 1 | 1–33 | 1–10 | ? |
| `craft-robo-cc300-20` | Craft Robo CC300-20 | legacy | 0b4d:111a | ? | ? | 1–33 | 1–10 | ? |
| `silhouette-sd-1` / `-2` | Silhouette SD | legacy | 0b4d:111c / 111d | ? | ? | 1–33 | 1–10 | ? |
| `graphtec-ce6000-40/60/120` | Graphtec CE6000 | gpgl | ? | media 484 / 712 / 1346 wide | ? | panel | panel | ? |
| `graphtec-ce7000-40/60/130/160` | Graphtec CE7000 | gpgl | ? | media 484 / 712 / 1372 / 1626 wide | ? | panel (max 450 gf) | panel | ? |
| `graphtec-ce-lite-50` | Graphtec CE LITE-50 | gpgl | ? | 498 × 3000 | ? | panel | panel | ? |
| `generic-gpgl` | any GP-GL plotter | gpgl | — | `--media-size` | 1 | panel | 1–10 | — |
| `generic-hpgl` | Roland, USCutter, Vinyl Express, … in HP-GL mode | hpgl | — | `--media-size` | 1 | panel | panel | — |
| `generic-dmpl` | DMPL cutters (experimental) | dmpl | — | `--media-size` | 1 | panel | panel | — |
| `grbl-laser` | Grbl laser / knife CNC | gcode | — | `--media-size` | 1 | S 0–1000 | 1–60000 mm/min | — |
| `cricut-*` (7 rows) | Joy, Joy Xtra, Explore Air 2, Explore 3, Maker, Maker 3, Venture | unsupported | ? | public widths, unverified | | — | — | yes |

\* community estimate. Published maximum force: CAMEO 3 210 gf, CAMEO 4 line
5 kg (vendor marketing), CE7000 450 gf; every other figure is unknown.

Mats (`TG` codes 0 none, 1 12×12, 2 12×24, 3 8×12, 8 15×15, 9 24×24, 11 8.5×12):
CAMEO/CAMEO 2–5: 12×12, 12×24; Plus models add 15×15; Pro models add 24×24;
Portrait 1–3 8×12; Portrait 4 8×12, 8.5×12; Alpha: undocumented, so only "no mat".
A mat bounds the cut area (its extent is checked) but the `\`/`Z` cutting region
the device receives stays the model's full area.

Tools per model (validated by `--tool`): legacy CAMEO/Portrait: ratchet blade,
pen; Portrait 2: AutoBlade, pen; CAMEO 3: AutoBlade, ratchet, deep-cut, pen;
CAMEO 4/5 and Pro models: AutoBlade, ratchet, deep-cut, kraft, rotary, punch, pen;
Portrait 3/4: AutoBlade, ratchet, pen; Alpha: AutoBlade, pen (conservative, full
list undocumented); Curio: emboss, etch, pen; Curio 2: emboss, deboss, etch, pen;
plotters: drag blade, pen. Per-model availability of each vendor tool is taken
from the community tool-setup codes and vendor product listings and is **not
verified on hardware**.

### What is verified vs unknown

- **From the community driver table (spec, not hardware-verified by us):** every
  USB id above, cut widths/lengths for the Silhouette rows, force caps (33, Alpha
  40), speed ranges (10 pre-CAMEO-4, 30 CAMEO-4 line), single vs dual holders,
  which models do registration marks and four-mark (`quad`) registration.
- **From published Graphtec/reseller spec sheets:** CE6000 media widths
  ([datasheet](https://wiki.acemakerspace.org/wp-content/uploads/2025/10/graphtec-ce6000-datasheet.pdf)),
  CE7000 media widths and 450 gf
  ([Large Format Review](https://www.largeformatreview.com/?p=44761),
  [MyBinding](https://www.mybinding.com/products/graphtec-ce7000-60-plus-24-vinyl-cutter-and-plotter-with-stand)),
  CE LITE-50 498 mm × 3 m
  ([AA Print Supply](https://aaprintsupplyco.com/products/graphtec-ce-lite-50-vinyl-cutter)).
- **Unknown (left `None`):** Curio 1/2 USB ids and protocol, Graphtec CE USB
  ids and GP-GL force/speed ranges, CC300/SD cut areas, most Bluetooth flags of
  older models, most published force figures.
- **Unverified framing:** DMPL (`;:`, `EC1` = 0.001 in, `U`/`D`, `@`) — no
  published manual was found; test on scrap. Plain GP-GL assumes the panel's
  GP-GL step size is 0.1 mm and the Silhouette axis order (feed first).

## Protocol facts (Silhouette / Graphtec GP-GL)

| What | Value |
|---|---|
| Native resolution | 1 SU = 0.05 mm (20 SU per mm, 508 per inch) |
| Command framing | ASCII text, each command terminated with ETX (`0x03`) |
| Init | escape `\x1b\x04` (EOT) |
| Status query | escape `\x1b\x05` (ENQ); reply like `"    0\x03"` (`0` ready, `1` moving, `2` unloaded) |
| Tool-setup query (CAMEO 4 line) | escape `\x1b\x15` (NAK); reply `a,b` tool codes per holder (0 empty, 1 ratchet, 2 AutoBlade, 3 deep-cut, 4 kraft, 5 rotary, 7 pen) |
| Version query | `FG`, replies e.g. `CAMEO 4 PRO V1.02` |
| Coordinates | origin top-left; `M<down>,<right>` move, `D<down>,<right>` draw |
| Cutting region | `\<top>,<left>` then `Z<bottom>,<right>`, in SU |

### Setup preamble per dialect (golden: `crates/machine/tests/golden/*.txt`)

CAMEO 4 line (CAMEO 4/5, Pro, Alpha, Portrait 3/4), observed order:

```
TG9 FN0 TB50,0 \0,0 Z60000,12000    mat, reset, portrait feed, cutting region
J1                                  tool holder (two-holder models only)
FX10,1 TJ0 !5,1                     force, acceleration 0, speed
FC0,1,1 FE0,1 FF1,0,1 FF1,1,1       zero offset, lift, corner sharpening (tenths of mm)
FX10,1 TJ3 FC18,1,1                 force again, acceleration 3, blade offset 0.9 mm
TF<d>,1                             AutoBlade depth (only with --tool autoblade, holder 1)
FY0                                 track enhancing (only when asked)
```

CAMEO 3: the same mat block, then `J t`, `!s,t`, `FXp,t`, `FE`, `FF` pair,
`FC0,1,t FC18,1,t`, `TF`, `FY0`. Legacy (CAMEO 1/2, Portrait 1/2, Craft Robo, SD):
`FW<media>`, `!s`, `FXp`, `FC18` (`FC0` for a pen), `FY1`/`FY0`, `FN0`, `TB50,0`,
`\0,0`, `Z…`, `L0`, `FE0,0`, `FF0,0,0`. Every dialect ends the plot with
`M<below>,0` + `SO0` to park below the cut.

### Job options

- **Multiple passes** (`--passes N`): each path is cut N times back to back.
- **Line-segment overcut** (`--overcut MM`): at every corner sharper than 30° the
  blade runs MM past the corner and back (blade down); flattened curves are left
  alone. StencilCraft's own geometric rule, not a captured vendor stream.
- **Cut order** (`--order none|speed|roller`, default `roller`; MCP `order`): the
  reference's Send preference "Cut Order Sorting" (No Sort | Maximize Speed |
  Minimize Roller Movement, default the last), observed in
  [`observations/silhouette-studio.md`](observations/silhouette-studio.md). `roller`
  sorts each operation's paths by their top (feed-axis) edge so the media moves one
  way; `speed` is greedy nearest-neighbour from the head's last position. Above
  5000 paths per operation both fall back to the plain feed-axis sort.
- **Interior contours first** (on by default; `--no-interior-first`, MCP
  `interior_first: false`): a path enclosed by a closed path (strict bbox
  containment plus point-in-polygon) is cut before it, so holes are cut before the
  part is freed. Combines with any order: a path becomes eligible once everything
  inside it is cut.
- **Closed-contour overcut** (`--closed-overcut MM`, 0–5 mm, default 0; MCP
  `closed_overcut`): each closed contour continues MM past its start along its
  first segments so the start/end join is cut through (the reference's "Software
  Overcut" preference). Not applied to pen operations; works on every protocol.
- **Mirror on mat** (`--mirror`, MCP `mirror`): flips the job horizontally about the
  media centre for heat-transfer vinyl (cut face down). The width is the loaded
  media size, else the mat, else the machine's cut width; with origin margins the
  flip is `x' = W − 2·margin − x`. Applied when the job is built, after ordering and
  overcut; the document is never touched. Refused with registration marks.
- **Weld on cut** (`--weld`, MCP `weld`; off by default): before ordering, each
  operation's overlapping closed contours are united with the same boolean engine as
  Pathfinder/Weld, so a shared region is cut once (two 20 mm squares overlapping by
  10 mm: 160 mm of cut becomes 100 mm). Contours are grouped into shapes by nesting
  (inside an odd number of others = a hole of the innermost one), so counters stay
  holes; open paths pass through. A small shape lying wholly inside another (not in a
  hole) counts as a hole and is still cut. Skipped above 5000 paths per operation.
  The document is untouched. Silhouette's Weld on Cut is a paid-edition vendor feature
  (docs only, not observed).
- **Cut by layer / by line colour** (`--layer NAME:SET`, repeatable; MCP `layers:
  [{layer, skip, holder, passes, force, speed}]`): the reference's Send › Cut by Line
  Color table (Tool No., On/Off, per-row settings), observed in
  [`observations/silhouette-studio.md`](observations/silhouette-studio.md). `NAME` is a
  layer name or a `#rrggbb` line colour (the Silhouette importer makes one `Line
  #rrggbb` layer per colour); `SET` is `skip`, or comma-separated `tool=1|2` (alias
  `holder=`), `passes=N`, `force=N`, `speed=N`. The job gets one operation per layer
  (`CutJob::split_layers`); neighbouring layers whose settings end up equal merge, so
  a job without overrides is one operation as before. Overrides are validated against
  the machine like any job value (the error names the layer); an unknown layer is
  refused. The dry run shows each operation's holder, passes, force and speed.
- These job settings are applied by `apply_job_settings` in the job layer between
  two `validate` passes (the second checks the mirrored design still fits), so every
  protocol gets them; the desktop preview (`encode_cut`) applies them too.
- **Track enhancing** (`--track-enhancing`): `FY0`; the device only acts at force ≥ 19.
- **Print & cut** (`--regmarks W,L,X,Y [--regmark-search] [--regmark-quad]`): after
  setup, `TB50,0 TB99 TB52,2 TB51,400 TB53,10 TB55,1` then `TB123,h,w,top,left`
  (automatic search, 10 mm search margin), `TB124,…` (four marks, Pro MK II and
  Alpha only) or `TB23,h,w` (manual positioning). The design is shifted so the
  first mark is the origin and must fit inside the mark area. The sender waits for
  `0` before streaming the cut; no reply aborts with nothing cut. Pair it with the
  `print-cut` sheet ([`print-cut.md`](print-cut.md)), whose mark geometry must match.

## Generic plotter outputs

| Dialect | Stream |
|---|---|
| HP-GL | `IN;` `SP1;` then `PUx,y;` / `PDx,y,…;` per path, `PU;` `SP0;` — 40 units per mm, origin bottom-left, y flipped about the design's bottom edge. One command per line. |
| GP-GL | `!s` (if given), `M<down>,<right>` / `D…` at 0.1 mm steps, ETX-terminated, park `M<below>,0`. |
| DMPL | `;:` `EC1` then `Ux,y` / `Dx,y,…`, `U0,0`, `@` — 0.001 in units. Experimental. |

Orientation of the X axis relative to the media feed differs between cutters;
run `--bbox` first to see where the job lands.

## Transports

| Transport | Feature | Status |
|---|---|---|
| `RecordingTransport` | — | dry run: captures the exact bytes |
| `MockTransport` | — | scripted replies, for tests/simulation |
| `SerialTransport` | `serial` | tty/COM devices, OS-paired Bluetooth rfcomm ports |
| `UsbTransport` | `usb` | `nusb` 0.2 (MIT OR Apache-2.0, pure Rust): bulk OUT/IN on interface 0, kernel driver detached, 5 s timeouts. Implemented, not yet exercised on real hardware by us. |
| Bluetooth RFCOMM/BLE | — | **remaining.** No permissive, cross-platform Rust crate makes RFCOMM straightforward; BLE GATT (`btleplug`, MIT/Apache) would also need the undocumented GATT service layout. Workaround: pair in the OS and use the resulting serial port with `--device /dev/rfcomm0`. |

CLI:

- `stencilcraft-cli machines [--json]` — the catalog.
- `stencilcraft-cli devices [--status]` (`--features usb`) — enumerate cutter-vendor
  USB devices as `usb:BUS-ADDR VID:PID model`; `--status` sends only the read-only
  status and version queries.
- `stencilcraft-cli cut --in FILE --model ID [options]` — dry run; `--out FILE`
  writes the stream; `--device usb[:BUS-ADDR]` or `--device /dev/ttyUSB0` performs
  the real send (the deliberate step; build with the matching feature).

## Cricut

**Not implemented, deliberately.** Cricut machines (Joy, Explore, Maker, Venture…)
are driven only by the vendor's Design Space app. The machine protocol is
proprietary and undocumented, is reportedly encrypted, and the app ties cutting to
the vendor's cloud account. StencilCraft does not implement or reverse-engineer it;
doing so would need an owner decision and a legal review (terms of service,
anti-circumvention law) first.

Supported workaround: export the design and upload it into Design Space —
`stencilcraft-cli run --in FILE --export out.svg` (SVG is Design Space's
preferred upload format; DXF via `--export out.dxf`; for print-then-cut images,
export SVG and rasterize to PNG at actual size). Set the material and cut in
Design Space.

## Media presets

`MEDIA_PRESETS` in the crate (id → force/speed/depth defaults), e.g. `102` Vinyl
Sticker = 10 / 5, `122` Cardstock = 30 / 10. They apply to the Silhouette dialects
only; plotters get force/speed from the panel.

## Not yet implemented

- Bluetooth transport (see above); USB tested only against `MockTransport`.
- Curio and Graphtec CE USB ids; Graphtec CE force/speed over GP-GL.
- Mat-free cutting strategy (pressure relief beyond the cut-order sort).
- Choosing the start point of closed contours (the speed order only reorders paths).
- Binary-encoded relative draw (`BE`) that newer firmware accepts.

## Provenance & licensing

Facts, not code, from:

- `fablabnbg/inkscape-silhouette` — `silhouette/Graphtec.py` device table and
  `Commands.md` (**GPL-2.0**), read as a specification:
  <https://github.com/fablabnbg/inkscape-silhouette>.
- `virtualabs/inkscape-cutcutgo` (**GPL-3.0**), independent confirmation of the
  command set: <https://github.com/virtualabs/inkscape-cutcutgo>.
- `pklaus/Silhouette-Cutting-Plotters` — protocol notes and media table.
- The `robocut` lineage both of the above credit.

These GPL drivers are read as *specifications only*. Observing what a vendor app sends to a
machine during a normal job is also allowed. Disassembling, decompiling or debugging
vendor binaries is not. The full clean-room posture is in
[`reference-apps.md`](reference-apps.md#clean-room-posture).
- Graphtec's GP-GL command list (CraftRobo Pro S / CE5000):
  <https://web.archive.org/web/20160801023226/http://ohthehugemanatee.net/uploads/2011/07/GP-GL.pdf>.

No GPL code was ported; the Rust implementation is written from these facts and
StencilCraft stays MIT OR Apache-2.0.

## Desktop USB cutting (hardware validation pending)

Desktop builds enable USB by default. In Make, use **Find USB cutters**, select
your connected cutter, choose its supported mat and material settings, and use
**Validate USB cut** before **Send cut to selected USB cutter**. Discovery does not
move hardware. Selection uses catalog USB identifiers; submission checks the model
again before opening the transport. USB work runs outside the UI thread.

Load material first and set blade depth on the machine. This initial workflow cuts
visible, non-hidden paths with holder 1; it does not apply kerf compensation or
per-layer operations. Bounding-box preview disables real submission. Submission
reports bytes, not physical completion. Use the machine controls to stop motion.
On Linux the user must have permission to open the USB device; permission failures
appear in Make. No broad USB permission rules are installed by the application.

Cricut remains SVG export to Design Space, not direct USB control. Public connection
instructions describe Design Space but do not specify cut-command bytes:
https://help.cricut.com/hc/en-us/articles/6581830148759-Bluetooth-and-USB-Connection-Help

### Linux cutter permissions

From a source checkout, run `packaging/linux/grant-cutter-access.sh` to install
a udev rule for the USB IDs in the machine catalog. It uses sudo for the system
rule, reloads udev and triggers matching connected devices. The `uaccess` tag
grants the active local desktop session access; it does not make devices writable
by everyone. Preview the rule with `packaging/linux/grant-cutter-access.sh --print`.
If necessary reconnect the cutter and click **Find USB cutters** again. Check
`lsusb` and `getfacl /dev/bus/usb/BBB/DDD` for the current bus and device.
Flatpak additionally requires `--device=usb` and a rebuilt application containing
the USB workflow; see [development](development.md#flatpak-usb-access).
The catalog and generated rule cover known USB IDs only, not all manufacturers.
