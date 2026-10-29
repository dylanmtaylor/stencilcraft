# architecture

How the crates fit together and why. The layering (`cargo xtask layers`) has six levels;
higher levels may depend on lower ones, never the reverse.

```
L6  apps/stencilcraft            # desktop shell (eframe, rfd)
L5  crates/ui-egui               # frontend: canvas painter, panels, theme tokens
L4  apps/stencilcraft-cli        # headless driver, control/MCP stdio loop
L3  crates/mcp · crates/engine   # session/protocol · commands + undo
L2  crates/{svg, dxf, gcode, tiling, kerf} · stencilcraft-doc
L1  crates/geom                  # geometry primitives (kurbo re-exports), mm units
L0  external: kurbo, serde, egui (via ui-egui only), eframe (via apps only)
```

- **geom** at the bottom: points, rects, paths (kurbo), mm helpers, path flattening.
  Everything else depends on it; it depends on nothing but kurbo.
- **doc** holds the document model and the `.stencilcraft` format. It knows nothing about
  kerf, tiling or exporting; those live above it and take `&Document` (or `Serde copy` of it).
- **kerf** and **tiling** are pure transforms of documents; **svg/dxf/gcode** are pure
  serializers. None of them touch undo, the frame loop or the console.
- **engine** is the one place user-visible behaviour is defined. Commands are
  serde-tagged; every actor (mouse, menubar, CLI, MCP, control channel) calls
  `engine.run(command)` and receives a result. Undo/redo lives here.
- **mcp** wraps a session (a document + an engine + tiling/export helpers) with a JSON
  lines interpreter; both stdio and sockets use it.
- **ui-egui** reads document state and sends commands; the desktop app is a thin wrapper
  (window bootstrap, file dialogs, persistence of the window frame).

The UI is thin: panels read engine state and act through `app.run(command)`. Colours come
from a single theme token table in `ui-egui`.
