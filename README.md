<p align="center">
  <img src="assets/app-icon/stencilcraft.svg" width="128" height="128" alt="StencilCraft icon: a paper stencil with a bridged ring cut-out and a craft knife">
</p>

<h1 align="center">StencilCraft</h1>

<p align="center">
  <b>Cut preparation; an open-source, modern stencil and cutting-prep application, built in pure Rust.</b>
</p>

<p align="center">
  A fast, open-source stencil workflow. It runs natively on macOS, Windows and Linux, and in the
  browser via WebAssembly (planned). It is one Rust codebase, written from scratch.
</p>

<p align="center">
  <img alt="Status: in active development" src="https://img.shields.io/badge/status-in%20active%20development-e8573f?style=flat-square">
  <img alt="Written in pure Rust" src="https://img.shields.io/badge/pure-Rust-b83a24?style=flat-square&logo=rust&logoColor=white">
  <img alt="Runs on macOS, Windows and Linux" src="https://img.shields.io/badge/runs%20on-macOS%20%C2%B7%20Windows%20%C2%B7%20Linux-555555?style=flat-square">
  <img alt="MCP server for agents" src="https://img.shields.io/badge/agents-MCP%20server-555555?style=flat-square">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-555555?style=flat-square">
</p>

<p align="center">
  <a href="#a-look-around">A look around</a> ·
  <a href="#why-stencilcraft">Why StencilCraft</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#use-it-from-claude-code-and-other-agents">Agents</a> ·
  <a href="#status">Status</a> ·
  <a href="#downloads">Downloads</a> ·
    <a href="#license-and-credits">License</a>
</p>

## A look around

Every image below is rendered by StencilCraft's own SVG exporter from the
[`examples/`](examples) documents it refers to.

<table>
<tr>
<td width="33%" valign="top">
  <img src="docs/images/starter-preview.png" alt="The starter document: two cut rectangles and an engraving line on the 100 × 150 mm board, dark theme" width="100%">
  <p align="center"><sub><b>Starter</b>: two cut shapes and an engraving line on the default board · <code>examples/starter.stencilcraft</code></sub></p>
</td>
<td width="33%" valign="top">
  <img src="docs/images/stencil-frame-preview.png" alt="The stencil-frame document with 0.2 mm kerf applied: the outer frame grown outward and the inner counter grown inward" width="100%">
  <p align="center"><sub><b>Kerf compensation</b>: the outer frame widens while the counter shrinks · <code>examples/stencil-frame.stencilcraft</code></sub></p>
</td>
<td width="33%" valign="top">
  <img src="docs/images/poster-tile-preview.png" alt="The left tile of the poster document after tiling to 400 × 600 mm sheets with a 10 mm overlap" width="100%">
  <p align="center"><sub><b>Tiling</b>: a 500 × 400 mm board split onto printable sheets · <code>examples/poster.stencilcraft</code></sub></p>
</td>
</tr>
</table>

> [!NOTE]
> `stencilcraft-cli cut` without `--device` is a **dry run**: it prints the exact
> Graphtec command transcript and never touches hardware. A real send is a
> deliberate, explicit step — and every model, media preset and cutting mat is
> chosen in the Send panel the same way.

## Why StencilCraft

- **Familiar.** Laser/vinyl workflow conventions from public user documentation only,
  clean-room: never reading or copying another cutting-prep program's code or assets.
  Graphtec protocol facts come from community-reverse-engineered GPL drivers as
  *specifications only* ([inkscape-silhouette](https://github.com/fablabnbg/inkscape-silhouette),
  [inkscape-cutcutgo](https://github.com/virtualabs/inkscape-cutcutgo),
  [pklaus/Silhouette-Cutting-Plotters](https://github.com/pklaus/Silhouette-Cutting-Plotters)) —
  no GPL code in this MIT/Apache codebase.
- **Robust.** No panics in shipped code: an untrusted file can never lose unsaved work.
  Fallible operations return `Result`; errors become calm messages, never aborts. The
  document reader is corpus-tested against thousands of malformed inputs.
- **Documented format.** The native document is plain JSON with the extension
  `.stencilcraft`; the header names `"format": "stencilcraft"` with a version, so files
  read years later.
- **Agent-native.** The CLI doubles as a JSON-lines control channel and MCP server, so
  agents can load, kerf-apply, tile, export and *dry-run cut* files the way a person does.
- **Modular Rust workspace.** The reusable domain crates separate geometry, color,
  documents, kerf, tiling, exports and machine communication; nothing below `ui-egui`
  knows about egui (enforced by `cargo xtask layers`). Reusable movable/dockable panel
  infrastructure is not yet implemented.

## Quick start

Import a raster silhouette as closed cut contours, then export a printable artwork
sheet with registration marks for a manual alignment workflow:

```sh
cargo run --release -p stencilcraft-cli -- trace-image --in logo.png --out logo.stencilcraft --threshold 128
cargo run --release -p stencilcraft-cli -- print-cut --in logo.stencilcraft --out logo-print-cut.svg
```

The trace path supports bounded PNG/JPEG inputs; its default physical scale is 96 dpi.
Print & Cut export requires printing at actual size. Optical-sensor reading and automatic
machine alignment are not implemented yet; see [the alignment limitations](docs/print-cut.md).

```sh
git clone https://github.com/dylanmtaylor/stencilcraft
cd stencilcraft
cargo run --release -p stencilcraft                      # desktop app (Design / Send)
cargo run --release -p stencilcraft-cli -- import-svg --in artwork.svg --out artwork.stencilcraft
cargo run --release -p stencilcraft-cli -- run --in examples/starter.stencilcraft --export out.svg
cargo run --release -p stencilcraft-cli -- run --in examples/stencil-frame.stencilcraft --export out.svg   # kerf applied (0.2 mm)
cargo run --release -p stencilcraft-cli -- run --in examples/poster.stencilcraft \
                                        --tile-sheet 400x600 --overlap 10 --export out     # tiling
cargo run --release -p stencilcraft-cli -- cut --in examples/starter.stencilcraft          # machine dry-run transcript
cargo run --release -p stencilcraft-cli -- cut --in examples/starter.stencilcraft --device /dev/ttyUSB0  # real cut
cargo xtask ci                                            # fmt, clippy, tests, layering
```

Use the desktop app with a control channel in a second terminal. Requests sent to
`127.0.0.1:7979` act on the document that is open in the window, and can be undone there:

```sh
cargo run -p stencilcraft -- --control 7979 examples/starter.stencilcraft
printf '%s\n' '{"id":1,"method":"doc.summary","params":{}}' | nc 127.0.0.1 7979
```

`examples/` ships starter documents: `starter.stencilcraft` (the CLI round trip),
`stencil-frame.stencilcraft` (a true counter for kerf) and `poster.stencilcraft`
(an oversized board for tiling). Protocol and design notes live in [`docs/`](docs):
`machine.md` (the Graphtec command stream), `ui-design.md` (Silhouette/Cricut structure
on our skin), `architecture.md`, `mcp.md`, `gaps.md`, `development.md`.

## Use it from Claude Code and other agents

Register the MCP server with Claude Code:

```sh
claude mcp add stencilcraft -- /path/to/stencilcraft-cli mcp
```

The full method list and the equivalent MCP tools are in [`docs/mcp.md`](docs/mcp.md).

## Status

StencilCraft is under active development. [**ROADMAP.md**](ROADMAP.md) has the stage,
the numbers by dimension and what's next; [`docs/gaps.md`](docs/gaps.md) the ranked
list of what's missing.

**Where we are (2026-10-29): scaffold+.** The document model and `.stencilcraft` round
trip, kerf compensation, sheet tiling, and SVG/DXF/G-code exports work and are tested;
the desktop shell edits rectangle shapes, previews kerf, saves/opens files, provides basic
click/box/lasso selection, and shows the **Design/Send** split with a live machine dry-run;
`cut` encodes Silhouette/Graphtec Cameo streams end to end (the protocol is in place;
real-hardware transport is the next step).

**What's missing:**
- SVG features beyond common paths/shapes/groups/transforms/styles (advanced CSS,
  external `<use>`, text conversion, clipping/masks/filters);
- drawing tools (pen, node editing) and selection refinements (box/lasso now exist, but are basic);
- bridge generation so counters stay attached, plus overlap merge and alignment marks for tiled sheets; print-and-cut has printable marks but no automatic sensor alignment;
- machine transports beyond serial documented dry-run + opt-in serial (USB bulk, Bluetooth).

**Where we're going:** next is broader SVG import (CSS, `<use>`, text conversion,
clipping/masks/filters), then the kerf offset hardening pass, then bridges and
machine transports. The ranked list is
in [`docs/gaps.md`](docs/gaps.md).

**Workspace:** `crates/{geom, color, doc, kerf, tiling, svg, dxf, gcode, engine, machine, mcp, ui-egui, testkit}`
and `apps/{stencilcraft, stencilcraft-cli}`. The egui frontend is its own crate, so the
UI can be swapped without touching the engine.

## Downloads

There are no packaged releases yet — StencilCraft builds from source with
`cargo run --release -p stencilcraft` (desktop) or `cargo build --release -p stencilcraft-cli`
(headless). Release packaging, signed installers and the web build follow the Crafting
release packaging is on the roadmap (M5).

Agent and contributor rules (clean-room, the asset policy, no panics in shipped code,
quality gates) are in [`AGENTS.md`](AGENTS.md). Every bundled asset is listed with its
author, source and licence in [`ASSETS.md`](ASSETS.md).

## License and credits

StencilCraft is dual-licensed under MIT or Apache-2.0, at your option.
Copyright (c) 2026 StencilCraft contributors. Required notices are in [NOTICE](NOTICE).
