# gaps

Every known shortfall, ranked; the standing work list. Pick the first item you can finish.

1. **SVG import**: common shapes (`path`, basic shapes), groups, standard affine
   transforms, inherited presentation/inline display and production styles, physical
   units/viewBox/`preserveAspectRatio`, local `<use>` of supported shapes/groups, and
   `<symbol>` templates with viewBox sizing are supported. A deterministic malformed-input
   corpus is in place. Embedded CSS basic selectors, cascade, and inherited supported
   properties now work. Remaining: advanced CSS selectors and at-rules, color model,
   external `<use>`, text, clip paths, masks, and filters; unsupported drawable elements
   fail explicitly rather than disappear. The parser has bounded XML/CSS bytes,
   nodes/depth/coordinates, rejects duplicate IDs/reference cycles, and caps `<use>`
   instancing at 10,000 instances. Coverage-guided fuzzing remains to be done.
2. **Kerf robustness**: offsetting assumes simple contours; a self-intersecting subpath
   or a miter spike on a hairpin can produce a bad polygon. No sweep-based offset; no
   nested-counter detection beyond bbox-in-bbox.
3. **Tiling**: tile edges are not clipped; overlapping output exports whole shapes per
   tile, relying on overlap to keep continuity. No per-tile SVG merging view, no
   alignment marks.
4. **G-code**: fixed modal constants, no feed/speed profiles by machine or material,
   no first/last pass, no power ramp, no Z/laser mode switch.
5. **DXF**: no layer colour assignment, no arc entities (export flattens curves into
   line segments), no reader.
6. **Engine**: commands cover add/remove shape, add layer, kerf; no move/rotate/scale, no
   selection model in the engine, undo stack snapshots whole documents rather than
   sharing structure.
7. **UI**: no node editing, no ruler, no zoom-to-fit keyboard shortcuts, kerf preview
   recomputes on every paint when dirty (no cache), no recent-files list.
8. **Agent surface**: control methods do not cover the whole engine command set;
   no MCP tool schema hydration yet; no fuzz corpus for the JSON channel. (The
   document reader does have a malformed-input no-panic corpus now; the JSON
   control channel does not.)
9. **wasm**: no web build.
10. **ci**: `xtask layers` checks dependencies by reading manifests (cheap and honest),
   but there is no `cleanroom`/`assets` check yet; `ASSETS.md` is enforced by review only.
11. **Packaging**: no window/app icon wiring yet (the SVG icon exists in
   `assets/app-icon/`); no desktop integration (`.desktop`, file associations,
   `xtask bundle`); PNG previews in the README only (`docs/images/`).
