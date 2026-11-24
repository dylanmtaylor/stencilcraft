# Phase 1: Operation-First Document Model - Context

**Gathered:** 2026-11-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 1 delivers the persisted document model and migration only. It stores geometry once, makes layers containers with an optional operation marker, supports N:M shape membership in operation layers, preserves intra-layer shape order, and migrates v1 `.stencilcraft` documents without data loss.

It does not build the Layer/Operations UI, per-operation G-code emission, job planning, streaming, recovery, or raster parity. Those are later phases.

</domain>

<decisions>
## Implementation Decisions

### Core Model

- **D-01:** A layer is a container. `Layer.operation` is optional, like Illustrator's appearance marker: `Some(operation)` means the layer is a production pass; `None` means it does not produce and does not enter the job sequence. Any number of layers may have no operation. — **Reversibility:** one-way — undoing this changes the v2 schema and every consumer of `Layer`.
- **D-02:** Shape geometry is stored once at document level. Layers reference shapes; a shape can belong to zero, one, or many operation layers (N:M). — **Reversibility:** one-way — undoing this needs a document migration and changes every shape/layer API.
- **D-03:** A layer with `Some(operation)` includes all of its shapes in that pass. Assigning an operation to an existing operation-less layer includes all shapes already in it. — **Reversibility:** costly — undo requires changing assignment semantics and possibly splitting layers.
- **D-04:** "Unassigned" is derived, never persisted. It is exactly the set of shapes with no membership in any layer that has `Some(operation)`. A shape in an operation-less container is unassigned unless it also belongs to an operation layer. — **Reversibility:** costly — undo touches queries, UI grouping, and output filters.
- **D-05:** Direct object operation is still a one-shape operation layer; it does not add a per-shape production authority. — **Reversibility:** costly — undo reintroduces per-shape operation ownership.

### Operation Semantics

- **D-06:** Internally, an operation has two axes: process/tool (laser, mill, knife, pen, creaser, punch, print) and strategy (line, fill, pocket, drill, score, kiss-cut, die-cut, perforate, guide), plus parameters. The user does not edit raw axes. — **Reversibility:** costly — undo changes the operation schema and preset catalogue.
- **D-07:** The user-facing unit is a cooked, parametric "New operation" preset (Laser cut, Laser engrave fill, Score, Punch, Mill profile, Mill pocket, Kiss-cut, Die-cut, Draw, Guide, etc.). Presets live in the profiles system and can be duplicated or customised. — **Reversibility:** costly — undo changes the profile and operation model.
- **D-08:** `ToolOp` is not a second production authority. It migrates into the cooked operation model. `Shape.tool` and `ShapeProduction.operation` become legacy read-only fields. — **Reversibility:** costly — undo would restore two overlapping operation sources.

### Settings and Presets

- **D-09:** Phase 1 includes the complete per-operation settings snapshot: power, speed, passes, focus, kerf, tabs, and the process/strategy-specific parameters. Emission of those settings remains Phase 3. — **Reversibility:** costly — undo requires a schema change and a second migration.
- **D-10:** Every layer embeds its operation snapshot. `preset_id` is optional provenance, not a dependency. A document must remain usable without the originating profile library. — **Reversibility:** one-way — undo changes the v2 schema and portability contract.
- **D-11:** Profiles follow LightBurn's Assign/Link model. **Assign** copies settings and leaves the layer editable. **Link** keeps the layer synced to the preset, makes layer settings read-only, and propagates preset changes. The embedded snapshot is the fallback when the preset is absent. — **Reversibility:** costly — undo changes profile semantics, UI, and persistence.
- **D-12:** "Extract preset" creates a named profile from the active layer's operation snapshot. It is the path for promoting embedded settings into the profile library. — **Reversibility:** reversible.

### Migration and Compatibility

- **D-13:** The wire format becomes v2. New documents are written as v2; old builds reject them with the existing format-version error. v1 documents are migrated in memory on load. The file is not rewritten until the user saves; saving writes v2. — **Reversibility:** one-way — v2 is a published file-format contract and undo needs a migration.
- **D-14:** Migration resolves the v1 conflict by effective shape operation, not by `Layer.kind` alone. For each v1 layer, group shapes by effective operation plus `Shape.tool`, create ordered operation layers for each distinct pass, and preserve original layer order and intra-layer shape order. — **Reversibility:** costly — undo needs a reverse migration and loses the richer operation split.
- **D-15:** v1 global `GcodeSettings` seed the default operation settings during migration because v1 has no per-layer settings. Exact field mapping is a planning/research detail. — **Reversibility:** costly — undo requires reworking migration defaults.
- **D-16:** `Layer.kind`, `Shape.style`, and `ShapeProduction.operation` are legacy read-only compatibility fields. They are deserialized for v1 migration, are not production authority, and are not written into v2. — **Reversibility:** costly — undo reintroduces duplicate operation sources.
- **D-17:** Template/guide layers migrate to `Operation::Guide` to preserve non-output semantics. In v2 a template/guide layer may be `None` or `Guide`; both are non-output. The old `template` flag is legacy read-only. — **Reversibility:** costly — undo changes migration and layer semantics.
- **D-18:** A new document starts with one base layer with `operation: None`. It is a workspace container, not an implicit Cut layer. Preferences/templates may assign it an operation. — **Reversibility:** costly — undo changes default-document behaviour and migration expectations.

### Ordering

- **D-19:** The order of layers with `Some(operation)` is the production sequence. Operation-less layers are not in the sequence. — **Reversibility:** costly — undo changes job ordering and UI semantics.
- **D-20:** Intra-layer shape order is preserved as an ordered `Vec<ShapeId>` (or equivalent). Phase 4 may optimise it; manual reordering comes later. — **Reversibility:** reversible.

### the agent's Discretion

- Exact Rust representation of an operation: data-carrying enum vs `OperationKind` + settings struct. The conceptual model is locked; representation is for research/planning.
- Exact v2 wire field names, shape storage layout, and membership serialization.
- Exact parameter schema per process/strategy and the validation matrix.
- Exact migration mapping from v1 `Operation`/`ToolOp`/`GcodeSettings` to cooked operations.
- Whether `preset_id` is a UUID, slug, or both; and how preset versions are tracked.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project and Phase

- `.planning/PROJECT.md` — project constraints, decisions, and upstream rules.
- `.planning/REQUIREMENTS.md` — v1 requirements MODEL-01..07; this phase covers all seven.
- `.planning/ROADMAP.md` — phase 1 goal, success criteria, and milestone boundary.

### Document Model

- `/home/r2d2/Sandbox/lasercraft/refs/craft-libs/crates/stencilcraft-document/src/lib.rs` — `Layer`, `Shape`, `Document`, `CutStyle`, `Document::default`, shape lookup.
- `/home/r2d2/Sandbox/lasercraft/refs/craft-libs/crates/stencilcraft-document/src/production.rs` — `Operation`, `ToolOp`, `ShapeProduction`, `Shape::operation()`.
- `/home/r2d2/Sandbox/lasercraft/refs/craft-libs/crates/stencilcraft-format/src/lib.rs` — `FORMAT_VERSION = 1`, `DocumentFields`, versioned JSON envelope.

### Existing Consumers

- `/home/r2d2/Sandbox/lasercraft/worktrees/stencilcraft-planning/crates/gcode/src/lib.rs` — current export uses `Layer.kind` for layer comments and power.
- `/home/r2d2/Sandbox/lasercraft/worktrees/stencilcraft-planning/crates/layout/src/job.rs` — layout uses `Shape::operation()` for grouping.
- `/home/r2d2/Sandbox/lasercraft/worktrees/stencilcraft-planning/crates/layout/src/mats.rs` — mat grouping uses per-shape operation.
- `/home/r2d2/Sandbox/lasercraft/worktrees/stencilcraft-planning/crates/engine/src/lib.rs` — `ShapesSetStyle` moves shapes to a layer and sets `Shape.style`.
- `/home/r2d2/Sandbox/lasercraft/worktrees/stencilcraft-planning/crates/engine/src/document.rs` — layer move/rename/color/lock/template commands and shape reordering.
- `/home/r2d2/Sandbox/lasercraft/worktrees/stencilcraft-planning/apps/stencilcraft/src/menus.rs` — Object menu and style actions.
- `/home/r2d2/Sandbox/lasercraft/worktrees/stencilcraft-planning/apps/stencilcraft/src/main.rs` — layer panel and command dispatch.

### External Behaviour Reference

- `https://docs.lightburnsoftware.com/2.2/Reference/MaterialLibrary/` — Assign vs Link semantics for operation presets.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `craft-stencilcraft-format`: already owns the versioned envelope, header check, and typed `DocumentFields`; v2 work belongs here.
- `craft-stencilcraft-document`: already owns `Layer`, `Shape`, `Document`, `Operation`, `ShapeProduction`, `ToolOp`, and `DocError`.
- `GcodeSettings`: existing global power/passes settings are the seed for migrated per-operation defaults.
- `Shape.id: u64` plus `next_shape_id`: stable IDs already exist for membership references.

### Established Patterns

- Serde defaults and `skip_serializing_if` keep old files loading and default fields out of output.
- Layer order is already meaningful; `Document.layers: Vec<Layer>` preserves it.
- Shape order is already meaningful; `Layer.shapes: Vec<Shape>` preserves it.
- The document crate treats malformed input as untrusted and returns calm `Result` errors.

### Integration Points

- `gcode::export` currently reads `Layer.kind`; Phase 3 will read the layer operation and its settings.
- `layout::job` and `layout::mats` currently read `Shape::operation()`; they must read the layer operation or a derived view.
- `engine::ShapesSetStyle` currently moves shapes and writes `Shape.style`; it must become layer-operation assignment.
- `Document::default()` currently creates a Cut layer; Phase 1 changes it to one base layer with no operation.
- `Document::shapes()` currently iterates embedded layer shapes; it must iterate document-level shapes and memberships.

</code_context>

<specifics>
## Specific Ideas

- "Operation is like Illustrator's appearance marker": a layer can exist without one and get it later.
- Opening an SVG with unassigned layers and assigning operations progressively is a primary workflow.
- "New operation" presets are cooked combinations of process + strategy; users should not assemble raw axes.
- LightBurn's **Assign** vs **Link** is the reference for profile behaviour.
- "Extract preset" from the active layer is the promotion path from embedded settings to the profile library.
- Cut is normally last because it releases the part; tabs keep the sheet together. Score/engrave run before cut.
- The same geometry can be scored and then cut, or engraved and then cut; this is the N:M case.

</specifics>

<deferred>
## Deferred Ideas

- Layer/Operations UI and "New operation" catalogue — Phase 2.
- Assign/Link UI and profile library management — Phase 2, with model support in Phase 1.
- Per-operation G-code emission and byte parity — Phase 3.
- 2-opt/start-point ordering and manual intra-layer reordering — Phase 4.
- Streaming, recovery, and raster parity — next milestone.
- Exact operation parameter matrix and profile versioning — planning/research.

</deferred>

---

*Phase: 1-Operation-First Document Model*
*Context gathered: 2026-11-24*
