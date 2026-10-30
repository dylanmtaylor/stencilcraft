# StencilCraft — instructions for agents

StencilCraft is a clean-room, open-source, Rust-native cutting-prep app: it takes vector
art (SVG) and prepares it for laser, vinyl and blade cutting — kerf compensation, sheet
tiling, SVG/DXF/G-code export. It runs natively on macOS, Windows and Linux, and on the
web via WASM (planned). The workspace uses strict quality gates and a no-panic discipline. Target-app reference: public user
documentation of consumer laser/vinyl ecosystem software, cited by URL — never its code.

## Start every session here

1. Read [`ROADMAP.md`](ROADMAP.md) (stage, dimensions, what's next) and `.gaps` checklist
   in `docs/gaps.md`. Unless the user gives you a task, pick work from that list.
2. Follow the operation protocol: orient → plan → implement + test → verify
   (`cargo xtask ci`) → record (update the docs the change touches) → commit. Do not stop
   to ask unless the user claimed the decision.
3. Keep commits at one task: a short message naming the task and the milestone
   (`M1.2: kerf for nested counters`).

`plan/` is gitignored (local session notes).

- **Use craft-libs where they exist.** Before rebuilding common UI or domain facilities,
  inspect this workspace's available craft-libs and reuse their public APIs. Do not claim
  parity with sibling applications unless the relevant components are actually in this
  checkout and wired into the app.

## Non-negotiables

- **Clean-room.** Never read, disassemble, run, script or compare against proprietary
  cutting-software binaries. Behaviour comes from public documentation (machine/vendor
  user manuals, published IETF-like file specs: SVG 1.1, DXF reference, Grbl) cited by URL,
  and from files users send that they own (used locally, never committed). Never copy
  GPL/AGPL code (Inkscape, lib2geom, LaserGRBL, VisiCut) — behaviour mirrors are fine,
  code is not.
- **Assets: original or permissively licensed only.** Never add, trace or embed artwork,
  icons, cursors or sample files whose licence doesn't allow redistribution. Every asset
  file must have a row in [`ASSETS.md`](ASSETS.md) (path, author, source URL, licence,
  notes); prefer art generated in code over files. `cargo xtask ci` fails on an unattributed
  asset.
- **Fonts live elsewhere, never here.** If the UI ever ships custom text shaping, fonts go
  into a sibling craft-fonts repository read through an optional `CRAFT_FONTS_DIR` build
  option. Everything must build, test and run with fonts absent.
- **Never crash: no panics in shipped code.** A crash loses the user's unsaved work.
  Files, cut parameters, MCP and control messages, and UI gestures are all untrusted input.
  - Fallible work returns `Result`/`Option`; callers handle or propagate with `?`.
  - Workspace clippy bans, outside tests: `unwrap()`, `expect()`, `panic!`,
    `unreachable!`, `todo!`, `unimplemented!`. Use `?`, `.ok_or(…)?`,
    `let … else { return … }`, `if let`, `.get(i)` over `v[i]`, and a real error for an
    impossible-looking match arm. Cap sizes and counts from input; check floats are finite
    before casting or looping.
  - Tests may unwrap and panic (`clippy.toml` allows it in `#[test]` and `tests/`):
    prefer `Result`-returning tests; allow the attribute-only escape.
- **Everything is a command.** User-visible behaviour = a command variant in
  `crates/engine/src/lib.rs` (documented params, serde-tagged so MCP and the control
  channel can construct it) + tests. Tools and UI act only through `engine.run(command)`.
- **Layering** is enforced by `cargo xtask layers`. Nothing below `crates/ui-egui` depends
  on egui/eframe/winit/rfd; exports don't depend on the engine; the engine doesn't depend
  on a specific export format.
- **Units are millimetres.** The document, kerf, tiling and every export speak mm. Never
  hand-roll unit conversion at call sites: convert once at a boundary, through
  `stencilcraft_geom` (and later, when pixels enter the UI, one scaling point in   `ui-egui`).

- **Rust only** (no handwritten JS/TS). **Never break wasm** (keep every dependency
  wasm-buildable; `cargo xtask ci` checks the workspace config compiles for wasm32).
- **Shared corpora.** Real-file test oracles (a .dxf from a real user's machine, machine
  G-code dumps) live locally under the gitignored `corpora/` directory, pinned by sha256 —
  never commit large binary fixtures.
- **Quality gates before every commit:** `cargo xtask ci` (fmt, clippy -D warnings including
  the no-panic lints, tests, layers). Performance-sensitive work (kerf sweeps, tiling,
  flatteners) adds a bench run before/after, recorded in the PR description.

## Layout

```
Cargo.toml          # workspace, lints (no-panic), profiles
crates/
  geom/             # kurbo self-built, mm helpers, flattening
  doc/              # Document/Layer/Shape model, .stencilcraft JSON read/write
  kerf/             # contour offsetting (kerf compensation)
  tiling/           # oversized-artboard → sheet tiling with overlap
  svg/              # SVG import/export
  dxf/              # DXF (R14 ASCII, LWPOLYLINE) export
  gcode/            # Grbl-style G-code export
  engine/           # command model + undo/redo (serde-tagged commands)
  ui-egui/          # egui frontend: canvas painter, panels, theme tokens
  mcp/              # JSON-lines control protocol / MCP server core (stdio)
  testkit/          # shared sample documents for tests
apps/
  stencilcraft/     # desktop app (eframe)
  stencilcraft-cli/ # headless: run/kerf/tiling/export, mcp mode
xtask/              # cargo xtask {ci, layers, fmt, clippy, test}
docs/               # development.md, mcp.md, gaps.md
examples/           # .stencilcraft samples
```

## Running and looking at the app

- `cargo run --release -p stencilcraft [-- examples/starter.stencilcraft]` (desktop app)
- `cargo run --release -p stencilcraft -- --control 7979` — JSON lines on
  `127.0.0.1:7979`; methods live in `apps/stencilcraft/src/control.rs`.
- Drive the headless path instead of the window on a machine without a display:
  `stencilcraft-cli run --in … --export out.svg` and cover panels with unit tests of the
  paint helpers (`crates/ui-egui`), which never need a screen.
- MCP: `stencilcraft-cli mcp` (see `docs/mcp.md`).

## Docs that move with code

- [`ROADMAP.md`](ROADMAP.md): stage, headline numbers, dimensions, progress log. Update in
  the same commit as the change it describes.
- `docs/gaps.md`: every known shortfall, ranked; the standing work list.
- `docs/development.md`: how to build, test and release; platform notes.
- `docs/mcp.md`: the control protocol and the equivalent MCP tools.

Grade progress by behaviour against public documentation, not by whether a command exists.
Scores are self-assessed: err low, mark every number measured or estimated.
