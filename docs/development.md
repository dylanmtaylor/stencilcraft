# Development

How to build, test and run StencilCraft on each supported platform.

## Build and test

```sh
cargo test                      # workspace tests
cargo run --release -p stencilcraft            # desktop app
cargo run --release -p stencilcraft-cli -- run --in examples/starter.stencilcraft --export out.svg
```

Quality gates before every commit:

```sh
cargo xtask ci    # fmt --check, clippy -D warnings, tests, layering
```

The no-panic clippy lints are workspace-level (`unwrap_used`, `expect_used`, `panic`,
`unreachable`, `todo`, `unimplemented` are all `deny`); `clippy.toml` re-allows them in
tests. Shipped code returns `Result`/`Option` — see AGENTS.md › "Never crash".

## Toolchain

Rust 1.95 or later (edition 2024). No nightly, no external tools: everything the CI
checks is a plain `cargo` invocation that `xtask` orchestrates.

## Documents

`.stencilcraft` files are JSON with a `"format": "stencilcraft"` header. Write them with
the desktop app's Save, `stencilcraft-cli`, or by hand — a hand-written file is a fine test
input. Malformed files must yield a calm error message ("line 7: unknown layer kind"),
never a crash; `crates/doc/tests/` feeds junk into the loader and checks it never panics.

## Exports

- **SVG**: one `<g>` per layer, `data-kind` records cut/engrave, viewBox in millimetres.
- **DXF**: ASCII R14 subset, `LWPOLYLINE` entities in modelspace, `$INSUNITS` = mm.
- **G-code**: `G21`/`G90` modality, one subpath at a time (`G0` travel, `G1` cut with a
  configured feed rate and power). Machine profiles with the same output shape come later.

## Kerf width

`kerf` offsets every closed contour in a shape by half the kerf width (outward for the
outer contour, inward for counters). Open contours (score/engrave) are left alone — their
tool compensation is a machine-side setting. Minimum supported kerf is 0 (identity, still
recomputes and round-trips so a kerf build is always present in tests).

## Platforms

- **macOS/Windows/Linux**: plain `cargo`, no system dependencies.
- **wasm**: planned; until the wasm build exists, `cargo xtask ci` only checks that every
  dependency stays wasm-buildable on paper (no platform-conditional non-wasm deps added
  to crates the web app will share). Web shell lands in M5.

## Fonts

No fonts are bundled here, ever. If a UI feature later needs custom text shaping, fonts
go into a separately maintained craft-fonts repository read through an optional
`CRAFT_FONTS_DIR` build input. Build, test and runtime must all work with that variable
unset. For now the UI renders no user text via shaped fonts beyond the toolkit's own
interface font.
