# ADR 0001: Geometry-preserving import

- **Status:** Accepted
- **Date:** 2026-11-25
- **Phase:** 2.1 (Geometry/Operations Separation)
- **Deciders:** StencilCraft maintainers

## Context

The Phase-2 importer maps SVG `<g>` elements to **operation layers** and inherited
`data-kind`/class styling to **operations** (`crates/svg/src/lib.rs:392`; test
`imports_nested_group_transforms_and_inherited_production_style` asserts two layers).
The document already models untouched nesting as `Document.groups` with
`GroupChild::{Shape, Group}` (`stencilcraft-document/src/canvas.rs:239`), but the importer
never populates it. The result is the conflation users report: importing a file silently
bakes operations from source styling and flattens the source tree into the layer list.

## Decision

1. **Import preserves geometry.** `<g>` nesting becomes `Document.groups`; shapes keep
   their ids and transforms. A plain import creates **no** operation layers; all shapes
   start Unassigned.
2. **Styling is a hint, not a decision.** `data-kind`, classes, and CSS are collected as
   `operation_hints` on `SvgImport` and surfaced in the UI for explicit user confirmation.
   The importer never writes operations.
3. **Legacy behaviour is opt-in.** `UserPrefs.import_legacy_style_operations` (default
   `false`) restores the Phase-2 semantics (groups→layers, style→operation) for one
   release, so existing workspaces can reproduce old results.

## Consequences

- Geometry from any vector source is reproducible and independent of operation choice.
- Grouping/ungrouping becomes a first-class UI operation backed by the existing
  `GroupCreate`/`GroupUngroup` commands.
- Operation assignment becomes an explicit user action (hammer/bulk), matching the
  LaserWeb-style workflow.
- Existing importer tests change meaning: `groups_are_skipped_not_rejected` becomes
  `groups_are_preserved`; the nested-style test asserts groups + zero operation layers,
  with a separate legacy-mode assertion for the old two-layer result.
- Round-trip is covered: SVG export writes `data-group` (`crates/svg/src/lib.rs:61`) and
  import must reconstruct the same group tree.

## Alternatives considered

- **Keep mapping `<g>` to layers.** Rejected: it makes source nesting unrecoverable and
  keeps operations implicit.
- **Map `<g>` to groups and also auto-create operations.** Rejected: it still bakes
  operations without consent; hints cover the useful case.
- **Hard switch with no legacy flag.** Rejected: breaks reproducibility for one release.

## Notes

- Config surface: `UserPrefs` (`apps/stencilcraft/src/main.rs:70`, persisted via
  `craft-settings`).
- Migration to first-class `Document.operations` is a separate step (Phase 2.1 plan 02);
  this ADR only governs import semantics. No v2 was ever published, so that model change
  is revised in place (no v3).
