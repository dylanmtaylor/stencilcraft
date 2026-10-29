# Control protocol (CLI JSON lines / MCP)

Two agent-facing surfaces share one core (`crates/mcp`):

- a **JSON-lines control protocol** on stdio (`stencilcraft-cli mcp`),
- the same session object used by the desktop app's `--control` socket.

One request per line, on stdin; one response per line, on stdout. Logging, if any, goes to
stderr only, never stdout.

## Envelope

```json
{"id": 1, "method": "doc.open", "params": {"path": "examples/starter.stencilcraft"}}
```

Response:

```json
{"id": 1, "ok": true, "result": {"path": "examples/starter.stencilcraft"}}
```

On failure:

```json
{"id": 1, "ok": false, "error": "open: examples/x.stencilcraft: no such file"}
```

Every request with a known `method` must produce exactly one response with the same `id`.
Unknown methods produce `{"ok": false, "error": "unknown method: ..."}`. Malformed JSON
produces a response with `id: null`. No request may abort the session on a bad parameter.
(Vectors: `crates/mcp/tests/`.)

## Methods

| Method | Params | Result |
| --- | --- | --- |
| `doc.open` | `{path}` | `{layers: n, shapes: n}` |
| `doc.save` | `{path}` | `{path}` |
| `doc.summary` | `{}` | `{layers, shapes, kerf_mm, board: [w, h]}` |
| `kerf.apply` | `{width_mm}` | `{width_mm}` |
| `export.svg` | `{path}` | `{path}` |
| `export.dxf` | `{path}` | `{path}` |
| `export.gcode` | `{path}` | `{path}` |
| `tile.run` | `{path, sheet_w, sheet_h, overlap}` | `{tiles: n}` |
| `command.execute` | `{command: <engine command as JSON>}` | engine result |

## MCP equivalence

The stdio loop is a subset of the Model Context Protocol tool surface: a later milestone
exposes each method above as an MCP tool with a JSON-schema parameter description, backed
by the same session type, so the CLI (`stencilcraft-cli mcp`) is the server process —
the shape VectorCraft uses for `vectorcraft-cli mcp`.
