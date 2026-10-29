# StencilCraft app icon

**Motif:** two stacked stencil sheets crossed by a dashed cut path, with a weeding
hole — the three objects a cut-prep workflow is about. Deliberately geometric and
abstract, drawn from the app's own drawing vocabulary (sheets, cut strokes, dashes).

**Style:** the Crafting Apps tile template — a full-bleed rounded square
(`rx = 112` on a `512 × 512` viewBox) in the app's accent color.

**Palette** (exactly three colours, the Crafting Apps icon trio):

| Colour | Hex | Used for |
|---|---|---|
| Ink | `#0b0b0c` | the back sheet |
| Paper | `#efe9dc` | the front sheet (the stencil) |
| StencilCraft colour | `#e8573f` | the full-bleed field, cut path, weeding hole |

**Provenance:** original work by StencilCraft contributors, licensed
MIT OR Apache-2.0 as part of the project. Listed in [`../../ASSETS.md`](../../ASSETS.md).
This file is the only form that exists; raster renders are derived from it when
packaging needs them:

```sh
rsvg-convert -w 256 -h 256 stencilcraft.svg > stencilcraft-256.png
rsvg-convert -w 1024 -h 1024 stencilcraft.svg > stencilcraft-1024.png
# macOS .icns / Windows .ico are built from the PNGs at release time
```
