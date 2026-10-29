# Driving cutting machines (Graphtec / Silhouette)

`stencilcraft-machine` encodes cut jobs for the Graphtec/Silhouette cutting
plotters that speak the classic ASCII command language over USB (the "0b4d"
vendor family) — including the full Cameo 4 line and its 24" Pro sibling.

## Protocol facts (reference)

Learned from community reverse engineering; see *Provenance* below.

| What | Value |
|---|---|
| Native resolution | 1 SU = 0.05 mm (20 SU per mm, 508 per inch) |
| Command framing | ASCII text, each command terminated with ETX (`0x03`) |
| Init | escape `\x1b\x04` (EOT) |
| Status query | escape `\x1b\x05` (ENQ); reply like `"    0\x03"` |
| Status codes | `0` ready, `1` moving, `2` unloaded |
| Tool-setup query ( Cameo 4) | escape `\x1b\x15` (NAK) |
| Version query | `FG`, replies e.g. `CAMEO 4 PRO V1.02` |
| Coordinates | SVG-like: origin top-left, +x right, +y down; device commands `M<y>,<x>` (move) and `D<y>,<x>` (draw) swap the axes |
| Cutting region | `\<top>,<left>` then `Z<bottom>,<right>`, in SU |

### Cameo-4-family setup preamble (observed order)

```
J1            select tool holder
FX10,1        pressure, holder 1
TJ0           acceleration (first connect sets 0)
!5,1          speed, holder 1
FC0,1,1       blade swivel circle (always 0 mm x 0.05 mm first)
FE0,1         lift head between paths (0 here; 1 = sharpen corners)
FF1,1,1       corner sharpening range, tenths of a mm (0,0 for pen)
FX10,1        pressure again (yes, twice)
TJ3           acceleration
FC18,1,1      swivel circle for the real blade: 0.9 mm = 18 SU
TG9           cutting mat select (9 = 24x24 Pro mat)
FN0           device soft reset marker
TB50,0        portrait feed (0 = portrait, 1 = landscape)
\0,0          region upper-left
Z60000,12000  region lower-right: 3000 mm x 600 mm (Pro hardware)
```

Plot body: `M`/`D` pairs per path, then `M<lly>,0` + `SO0` to park below the
cut (ready for the next job or the media cross-cutter).

### Hardware table (Cameo 4 line)

| Model | USB id | Cuttable width | Length |
|---|---|---|---|
| Cameo 4 | `0b4d:1137` | 304.8 mm | 3000 mm |
| Cameo 4 Plus | `0b4d:1138` | 372 mm | 3000 mm |
| Cameo 4 Pro | `0b4d:1139` | **600 mm** (of 609.6 roll width) | 3000 mm |
| Portrait 4 | `0b4d:113f` | 216 mm | 18290 mm |

Media presets (id → pressure/speed defaults) ship in `MEDIA_PRESETS` in the
crate; e.g. `102` Vinyl Sticker = 10 / 5, `122` Cardstock = 30 / 10.

## What StencilCraft implements today

- Typed command encoder (`crates/machine`): full Cameo-4 preamble, plot with
  margins, bbox-preview mode, park trailer; delimitation to the exact byte
  stream via `CutProgram::to_stream`.
- Transports: `RecordingTransport` (dry-run, captures the byte stream) and
  `SerialTransport` behind the `serial` feature. A `Transport` trait keeps a
  future USB-bulk (`nusb`) or BLE transport pluggable without encoder changes.
- CLI: `stencilcraft-cli cut --in FILE [--media vinyl] [--bbox] …` — dry run
  prints a one-command-per-line transcript; `--device PATH` performs a real
  serial send (initialize, version check, ready-poll, stream).

## Not yet implemented (deliberate gaps)

- USB bulk transport for the `0b4d` devices proper (needs `nusb`; serial path
  only covers CDC/ACM-style tty exposure).
- Bluetooth (RFCOMM/BLE) transport.
- Registration marks (`TB51/52/53/55/99…`, print-and-cut).
- Mat-free cutting strategy (path sorting for knife pressure relief);
  StencilCraft's kerf/tiling crates are the natural home for that logic.

## Provenance & licensing

The command vocabulary, tables, and sequences above are protocol *facts*
learned by the community, chiefly from:

- `fablabnbg/inkscape-silhouette` — Python driver, **GPL-2.0** ( Cameo 4 Pro
  confirmed working; USB + Bluetooth; mat-free strategies). We did not port
  its code; our Rust implementation is written from the facts, keeping
  StencilCraft MIT OR Apache-2.0 clean.
- `pklaus/Silhouette-Cutting-Plotters` — protocol notes and media table.
- The `robocut` lineage both of the above credit.
