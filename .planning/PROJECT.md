# LaserCraft — StencilCraft Laser CAM & GRBL

## What This Is

A contribution programme that adds laser CAM and reliable GRBL execution to StencilCraft and its shared libraries. The current milestone delivers a complete document-organization product: operation-first geometry, layer/operation assignment, per-operation output, and job planning. Reliable GRBL streaming, recovery, and raster parity are the next milestone. The work lands upstream in `craft-libs` (document model, G-code, streaming) and StencilCraft (layout, machine, engine, UI); `rustfactor` stays a local lab for prototypes and conformance evidence. It is not a standalone product, editor, or fork.

## Core Value

A persisted laser document can express ordered operations per object/layer and produce per-operation output identically from desktop, CLI, and MCP.

## Requirements

### Validated

(None yet — this contribution programme starts at P1a.)

### Active

- [ ] **MODEL-01**: `craft-stencilcraft-document` stores shape geometry independently of operation layers, with N:M memberships between shapes and operation layers.
- [ ] **MODEL-02**: An operation layer carries an `Operation`, settings, colour, visibility, lock, and order; a shape can participate in zero, one, or many operation layers.
- [ ] **MODEL-03**: Existing v1 documents migrate to the new model without data loss; shapes with no membership remain valid and are excluded from output.
- [ ] **UI-01**: StencilCraft has a real Layer/Operations menu and a layer dialog that requires an operation; no implicit `Cut` layer.
- [ ] **UI-02**: Users can assign one or many objects to layers, remove assignments, reorder layers, and edit layer colour/settings.
- [ ] **UI-03**: The Layers panel shows operation, colour, visibility, lock, and order; objects show their operation memberships; an "Unassigned" group is derived, not persisted.
- [ ] **OUT-01**: Layout and G-code emit per-operation power/feed/passes and support the same shape in multiple ordered passes (for example engrave then cut).
- [ ] **OUT-02**: Desktop, CLI, and MCP produce identical bytes for the same persisted job; reopening a job reproduces the same output.
- [ ] **PLAN-01**: The job orderer uses 2-opt and start-point selection with a measured travel-distance regression test.
- [ ] **PLAN-02**: Tabs, overcut, and focus/Z are represented and emitted for laser jobs.

### Deferred to Next Milestone

- **STREAM-01**: `craft-gcode` provides a byte-accurate in-flight window with a session state machine; `GcodeProtocol` uses it instead of one-line-per-`ok`.
- **STREAM-02**: Mock-transport tests cover acknowledgement, errors, pause, resume, and abort with beam off.
- **RECOVERY-01**: A job checkpoint can resume after reconnect without re-cutting completed geometry.
- **RASTER-01**: Raster behaviour matches the reference on golden images: threshold/dither/DPI/overscan/joinPixel/bidirectional and per-pixel power.

### Out of Scope

- New editor, document format, UI fork, or app shell — the work must land in StencilCraft/craft-libs.
- Duplicate G-code writer, IR, machine stack, or transport — reuse the shared crates.
- AGPL/GPL translation — licensing boundary.
- Writes into VectorCraft documents — StencilCraft consumes VectorCraft, not the reverse.
- General multi-firmware server — GRBL-class lasers first.
- Polygon nesting (M2), tangential knife (M3), and milling (M4) — parked until the document-organization milestone lands; they must not distort the laser operation model.
- Persisting an "Unassigned" layer — it is a derived UI group, not a production layer.

## Context

StencilCraft already has a machine layer, protocols/transports, a layout job planner, kerf/tiling, CLI and MCP. Its document model lives in `craft-libs` as `craft-stencilcraft-document`; StencilCraft re-exports it.

The current model conflates layer, operation, and shape style: `Layer` owns `Vec<Shape>`, `Layer.kind` is only `CutStyle::{Cut,Engrave}`, and `ShapeProduction.operation` is a per-shape override with fallback to `Shape.style`. `ShapesSetStyle` changes a shape's style and moves it to a layer of that style, which makes the source of truth ambiguous and prevents one object from taking part in two operations.

The target orientation is operation-first, like LaserWeb: layers are ordered production passes; geometry is stored once; memberships are N:M. A direct object operation is a one-shape layer, not a separate operation field on the shape. `ShapeProduction` remains useful for colours, material, mat, send assignment, hidden contours, and combine sources, but not as the owner of N:M operation membership.

The canonical document upstream is `storytold/craft-libs`; the StencilCraft application upstream is `dylanmtaylor/stencilcraft`. During development, `[patch]` points the document dependency at a local craft-libs branch; the final change must be upstreamable.

## Constraints

- **Tech stack**: Rust 1.95, edition 2024; reuse StencilCraft/craft-libs crates and existing dependencies before adding new ones.
- **Upstream**: Changes must be PR-ready for `craft-libs` and StencilCraft; no shipping code in `rustfactor`.
- **Licensing**: MIT/Apache-2.0 compatible only; no AGPL/GPL translation.
- **Compatibility**: Existing `.stencilcraft` v1 documents must keep loading; new fields are optional and migration is tested.
- **UI**: Modify upstream StencilCraft; no UI fork or parallel editor.
- **Cross-repo development**: `[patch]` to a local/fork craft-libs branch is allowed for development, but the PR must reference a real upstreamable revision.
- **Ordering**: Document organization is the only active milestone. Streaming, recovery, and raster are the next milestone. M2–M4 stay design notes until those land.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Upstream contribution, not a standalone product | StencilCraft already owns the machine, layout, CLI, and MCP surfaces; a fourth app would duplicate them. | — Pending |
| Operation-first layers with N:M membership | LaserWeb-style ordered passes allow engrave-then-cut on the same geometry; one style per shape cannot express it. | — Pending |
| Geometry stored once, layers reference shapes | Current `Layer.shapes: Vec<Shape>` ownership prevents a shape from belonging to multiple layers. | — Pending |
| Direct object operation = one-shape layer | Avoids duplicating operation settings/order on shapes and keeps one production model. | — Pending |
| `ShapeProduction` is not the N:M owner | It is per-shape metadata; the membership relation belongs to the document, not the shape record. | — Pending |
| Document changes in `storytold/craft-libs` | `craft-stencilcraft-document` is the shared document model and is already pinned by StencilCraft. | — Pending |
| `[patch]` to a local craft-libs branch during P1a | Lets StencilCraft compile against the new document model without publishing a crate or forking the app. | — Pending |
| Unassigned is a derived UI group | Persisting it would require an operation/settings/order and risks exporting geometry by accident. | — Pending |
| GSD root is a StencilCraft fork worktree on branch `planning` | Keeps planning and code on the fork; `gsd-pr-branch` later produces the clean PR branch without `.planning`. | — Pending |
| `rustfactor` stays a lab | It holds the contribution charter, prototypes, and evidence; shipping code belongs upstream. | — Pending |
| Milestone split: document organization first | Phases 1–4 already deliver a complete document-organization product; streaming, recovery, and raster form the next milestone. | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-11-24 after narrowing the milestone to document organization*
