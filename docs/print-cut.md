# Printable registration sheet (Print & Cut)

Create a printable page and its vector artwork with:

```sh
cargo run --release -p stencilcraft-cli -- print-cut \
  --in examples/starter.stencilcraft --out print-cut.svg
```

The SVG page is the artboard plus a 12 mm margin on each side. It has a small
black square in the top-left and asymmetric corner registration marks at the
top-right and bottom-left, plus the original cut/engrave paths in a nested,
physical-size SVG viewport. The asymmetry is intentional: keep the printed page
in the same orientation when aligning it on the mat. Print at **100% / actual
size** and disable “fit to page”; confirm the printer paper size matches the
page dimensions recorded in the root SVG.

This is a printable registration template and visual/manual alignment aid only.
It does not yet use a cutter's optical sensor, emit documented model-specific
registration commands, compensate for printer scaling, or calibrate machine
alignment. The `cut` command must not be treated as an automatic Print & Cut
workflow. Check mark visibility and test the workflow with pen/paper on the
specific model before cutting material.

Registration marks should not overlap artwork: the current fixed margin keeps
them outside the artwork viewport. Printer margins and supported page sizes
vary, so the generated SVG still needs to be sent to a printer that supports its
physical page size. StencilCraft does not launch or configure a printer.

For common setup failures—paper/mat orientation mismatch, clipped/faint marks,
scanner obstruction, or printer page-size mismatch—see the
[manufacturer's Print & Cut troubleshooting guidance](https://silhouetteamerica.freshdesk.com/support/solutions/articles/35000276219-print-and-cut-troubleshooting).
