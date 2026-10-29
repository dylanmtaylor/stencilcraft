# StencilCraft

Cut preparation; an open-source, modern stencil and cutting-prep application, built in pure Rust.
A fast, open-source stencil workflow. It runs natively on macOS, Windows and Linux, and in the
browser via WebAssembly (planned). Following the direction of
[VectorCraft](https://github.com/storytold/vectorcraft) and the Crafting Apps: one codebase,
written from scratch.

## What it does

StencilCraft prepares artwork for cutting:

- **Import SVG** art on an artboard measured in millimetres.
- **Kerf compensation** widens outer cut contours and shrinks inner counters by half the
  kerf width, so a cut part lands at its drawn size.
- **Tiling** splits an oversized artboard into physical sheets with a chosen overlap, so
  large stencils can be cut on small machines or cut by hand page by page.
- **Export** SVG, DXF (R14 ASCII, LWPOLYLINE) and Grbl-style G-code.

No drawing tools yet: StencilCraft starts behind a vector application, reading standard
files and preparing them for cutting. A native drawing set follows in later milestones
(see [ROADMAP.md](ROADMAP.md)).

## Why StencilCraft

- **Familiar.** Lazercut/vinyl-workflow conventions from public user documentation only,
  clean-room: never reading or copying another cutting-prep program's code or assets.
- **Robust.** No panics in shipped code: an untrusted file can never lose unsaved work.
  Fallible operations return `Result`; errors become calm messages, never aborts.
- **Documented format.** The native document is plain JSON with the extension `.stencilcraft`;
  the header names `"format": "stencilcraft"` and a version, so files read years later.
- **Agent-native.** The CLI doubles as a JSON-lines control channel and MCP server, so
  agents can load, kerf-apply, tile and export files the way a person does.
- **Everywhere.** One codebase for the desktop apps; the same UI planned for the browser.

## Quick start

```sh
git clone <this repository> stencilcraft
cd stencilcraft
cargo run --release -p stencilcraft                      # desktop app
cargo run --release -p stencilcraft-cli -- run --in examples/starter.stencilcraft --export out.svg
cargo run --release -p stencilcraft-cli -- run --in samples/dusk.stencilcraft --apply-kerf 0.2 --export out.svg
cargo run --release -p stencilcraft-cli -- run --in samples/poster.stencilcraft \
                                        --tile-sheet 400x600 --overlap 10 --export out
cargo run --release -p stencilcraft-cli -- mcp            # JSON-lines control (stdio)
cargo xtask ci                                            # fmt, clippy, tests, layering
```

Use the desktop app with a control channel in a second terminal:

```sh
cargo run --release -p stencilcraft -- --control 7979
```

`examples/` ships a small starter document to try the CLI round trip.

## Structure

Workspace: `crates/{geom, doc, kerf, tiling, svg, dxf, gcode, engine, ui-egui, mcp, testkit}`
and `apps/{stencilcraft, stencilcraft-cli}`. The egui frontend is its own crate, so the UI
can be swapped without touching the engine; nothing below `ui-egui` depends on egui
(enforced by `cargo xtask layers`).

## Status

StencilCraft is under active development. [ROADMAP.md](ROADMAP.md) has the stage, the
numbers by dimension and what's next.

Where we are (2026-10-29): scaffold. Document model and `.stencilcraft` round trip, kerf
compensation, sheet tiling, and SVG/DXF/G-code exports work and are tested; the desktop
shell edits rectangle shapes, previews kerf and saves/opens files. Next: SVG import on the
real-world scale (transforms, groups, styles), then bridge generation and pen/selection
tools.

## The Crafting direction

StencilCraft follows the design decisions of
[VectorCraft](https://github.com/storytold/vectorcraft) and the Crafting Apps community.
It stands on its own: pure Rust, no dependency on Adobe software or code, dual-licensed
MIT OR Apache-2.0.

## License and credits

StencilCraft is dual-licensed under MIT or Apache-2.0, at your option.
Copyright (c) 2026 StencilCraft contributors. Required notices are in [NOTICE](NOTICE).
Every bundled asset, if any ever exist, must have a row in [ASSETS.md](ASSETS.md).
