# StencilCraft app icon

**Motif:** a paper stencil with a ring cut out of it, its island held by two bridges,
and a craft knife finishing a dashed cut along the bottom: the stencil, the cut, and the
tool.

**Style:** the Crafting Apps flat-object template (CADCraft, DeckCraft, WordCraft): a
paper object with heavy ink outlines on a full-bleed rounded square (`rx = 112` on a
`512 × 512` viewBox) in the app's colour.

**Palette** (exactly three colours, the Crafting Apps icon trio):

| Colour | Hex | Used for |
|---|---|---|
| Ink | `#0b0b0c` | outlines, the cut line, the knife handle |
| Paper | `#efe9dc` | the stencil sheet, bridges, blade and collar |
| StencilCraft colour | `#e8573f` | the full-bleed field, showing through the cut-out |

**Provenance:** original work by StencilCraft contributors, licensed
MIT OR Apache-2.0 as part of the project. Listed in [`../../ASSETS.md`](../../ASSETS.md).
This file is the only form that exists; raster renders are derived from it when
packaging needs them:

```sh
rsvg-convert -w 256 -h 256 stencilcraft.svg > stencilcraft-256.png
rsvg-convert -w 1024 -h 1024 stencilcraft.svg > stencilcraft-1024.png
# macOS .icns / Windows .ico are built from the PNGs at release time
```
