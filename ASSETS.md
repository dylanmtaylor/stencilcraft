# Assets

Every image, icon, font, cursor or other creative asset bundled into this repository must
have a row here: path, author, source URL, licence, notes. Origins allowed: original work
by a contributor (licensing it MIT OR Apache-2.0), OSI open source, public domain / CC0,
or Creative Commons with redistribution allowed. `cargo xtask ci` fails on an
unattributed asset.

No asset files exist yet. Prefer art generated in code for defaults (cursors, panel icons,
sample documents' content); if the provenance of an asset is unclear, don't add it.

| Path | Author | Source | Licence | Notes |
| --- | --- | --- | --- | --- |
| `assets/app-icon/stencilcraft.svg` | StencilCraft contributors (project owner) | original, drawn for this project (no external source) | MIT OR Apache-2.0 | App icon: stacked stencil sheets crossed by a dashed cut path; single source of truth, rasters derived at release time |
| `docs/images/starter-preview.png` | StencilCraft contributors (project owner) | rendered by `stencilcraft-svg` export of `examples/starter.stencilcraft` | MIT OR Apache-2.0 | README gallery; regenerable (`cargo run -p stencilcraft-cli -- run … --export` + `rsvg-convert`) |
| `docs/images/stencil-frame-preview.png` | StencilCraft contributors (project owner) | rendered by `stencilcraft-svg` export of `examples/stencil-frame.stencilcraft` | MIT OR Apache-2.0 | README gallery; regenerable same way |
| `docs/images/poster-tile-preview.png` | StencilCraft contributors (project owner) | rendered by `stencilcraft-tiling` + `stencilcraft-svg` export of `examples/poster.stencilcraft` tile 0 | MIT OR Apache-2.0 | README gallery; regenerable same way |
| `docs/images/poster-tile-01-preview.png` | StencilCraft contributors (project owner) | rendered by `stencilcraft-tiling` + `stencilcraft-svg` export of `examples/poster.stencilcraft` tile 1 | MIT OR Apache-2.0 | README gallery (spare tile render); regenerable same way |
