# Development

How to build, test and run StencilCraft on each supported platform.

## Build and test

```sh
cargo test                      # workspace tests
cargo run --release -p stencilcraft            # desktop app
cargo run --release -p stencilcraft-cli -- run --in examples/starter.stencilcraft --export out.svg
packaging/linux/install-flatpak.sh             # build and install as a user Flatpak (Linux)
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

A shape may carry an optional `text` object (format version 1; no bump: older files have
none, and older readers ignore it and cut `path`). Its `path` is always the laid-out
outline; `text` holds what it was laid out from, so the text stays editable:

```json
"text": {
  "text": "Hello", "font": {"family": "Source Sans 3", "style": "Bold", "file": "/fonts/x.ttf"},
  "size": 72, "x": 10, "y": 20, "alignment": "centre",
  "letter_spacing": 0, "line_spacing": 100, "kerning": true,
  "vertical": false, "weld": false,
  "flow": {"kind": "curve", "diameter_mm": 80},
  "letters": [{"index": 1, "dx_mm": 0.5, "dy_mm": 0, "rotate_deg": 10}],
  "transform": [1, 0, 0, 1, 0, 0]
}
```

Only `text`, `font.family`, `size`, `x` and `y` are required; the rest default as shown
(`style` `Regular`, `flow` straight, `transform` identity) and are left out when default.
`transform` holds turns and scales made to the shape since its last layout (plain moves fold
into `x, y`). `flow` may also be `{"kind": "on-path", "path": <a path, as a shape's `path`>, "start",
"offset_mm", "flip", "shape"}`. Field meanings are in `docs/mcp.md` › Text.

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

- **macOS/Windows**: plain `cargo`, no system dependencies.
- **Linux**: default desktop builds include Bluetooth and need `pkg-config` and
  the D-Bus development headers (`libdbus-1-dev` on Debian/Ubuntu). To build the
  desktop without Bluetooth, use `cargo build -p stencilcraft --no-default-features
  --features usb`.
- **wasm**: planned; until the wasm build exists, `cargo xtask ci` only checks that every
  dependency stays wasm-buildable on paper (no platform-conditional non-wasm deps added
  to crates the web app will share). Web shell lands in M5.

## Fonts

No fonts are bundled here, ever. If a UI feature later needs custom text shaping, fonts
go into a separately maintained craft-fonts repository read through an optional
`CRAFT_FONTS_DIR` build input. Build, test and runtime must all work with that variable
unset. For now the UI renders no user text via shaped fonts beyond the toolkit's own
interface font.

### Flatpak USB access

Both Flatpak manifests and the fallback bundle builder grant `--device=usb` for
the desktop's raw USB cutter transport. This requires a Flatpak version supporting
that permission; older versions may ignore it. Host device permissions still apply.
Rebuild and reinstall the package, then restart the app to apply manifest changes
to an existing installation. This does not itself enable USB support in CLI builds.
See [Flatpak sandbox permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html).

For Linux host cutter permissions, see [Linux cutter permissions](machine.md#linux-cutter-permissions).

### Flatpak Bluetooth access

The Flatpak manifests grant Bluetooth sockets, the host network namespace and
access to `org.bluez` on the system bus for planned Linux Bluetooth connections.
This grants broader network access; it does not implement Bluetooth transport.
Rebuild and reinstall the Flatpak to apply these permissions. See [Flatpak sandbox
permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html).
