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
`stencilcraft.svg` is the canonical artwork; every other file here is rendered from it.

## Files

| File | What |
|---|---|
| `stencilcraft.svg` | master vector; also the hicolor scalable icon |
| `stencilcraft-1024.png` | full tile, 1024 px (docs, store listings) |
| `stencilcraft-macos-512.png` | tile with Apple's transparent margin (tile = 824/1024 of the canvas), 512 px |
| `stencilcraft.icns` | macOS icon set, on Apple's grid |
| `stencilcraft.ico` | Windows icon, 16 to 256 px |
| `hicolor/<size>/apps/ai.storyteller.stencilcraft.png` | Linux theme icons, 16 to 512 px |
| `hicolor/scalable/apps/ai.storyteller.stencilcraft.svg` | Linux scalable icon |

## Regenerate

Edit `stencilcraft.svg`, then run `packaging/icons.sh` (needs `rsvg-convert` and python3 with
Pillow).
