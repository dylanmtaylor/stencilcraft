# Roadmap: Document Organization — LaserCraft/StencilCraft

## Overview

This milestone turns StencilCraft's conflated layer/operation/shape model into an
operation-first document: geometry stored once, ordered operation layers as the
production passes, N:M membership, and a derived "Unassigned" group. From that
foundation it builds the Layer/Operations UI, per-operation laser output with
byte parity across desktop/CLI/MCP, and measurable job planning. Work lands in
`craft-libs` (document) and StencilCraft (layout, machine, engine, UI); the
`rustfactor` lab supplies prototypes and conformance vectors only.

Reliable GRBL streaming, recovery, and raster parity are a separate milestone and
are intentionally not phases of this roadmap.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Operation-First Document Model** - Geometry stored once, ordered operation layers, N:M membership, v1 migration
- [ ] **Phase 2: Layer/Operations UI and Assignment** - Real Layers menu, operation-required layers, assignment and ordering UX
- [ ] **Phase 3: Per-Operation Laser Output** - Power/feed/passes per operation, ordered multi-pass shapes, desktop/CLI/MCP byte parity
- [ ] **Phase 4: Job Planning and Laser Features** - 2-opt and start-point ordering with measured travel, tabs/overcut/focus

## Phase Details

### Phase 1: Operation-First Document Model
**Goal**: A persisted document stores each shape's geometry once and expresses ordered production passes through N:M membership in operation layers, with existing v1 documents migrating intact.
**Mode:** mvp
**Depends on**: Nothing (first phase)
**Requirements**: MODEL-01, MODEL-02, MODEL-03, MODEL-04, MODEL-05, MODEL-06, MODEL-07
**Success Criteria** (what must be TRUE):
  1. A document saves and reloads with each shape's geometry stored once while that same shape appears in two ordered operation layers.
  2. Loading an existing v1 `.stencilcraft` file preserves every shape's geometry and production data and maps its former layers to operation layers.
  3. A shape with zero memberships saves, reloads, and is reported as unassigned while contributing nothing to any job.
  4. Deleting an operation layer removes its memberships and leaves all shape geometry intact and still editable.
  5. Cut, Engrave, and Score each select an operation on a layer end-to-end, and the richer `Operation` variants remain representable and round-trip.
**Plans**: TBD

### Phase 2: Layer/Operations UI and Assignment
**Goal**: Users can build an ordered operation stack in StencilCraft and assign any selection of objects to it without changing geometry.
**Mode:** mvp
**Depends on**: Phase 1
**Requirements**: UI-01, UI-02, UI-03, UI-04, UI-05, UI-06, UI-07
**Success Criteria** (what must be TRUE):
  1. User can open a Layers/Operations menu and add a layer only by choosing an operation; no layer is ever created implicitly as Cut.
  2. User can add one or many selected objects to a layer and remove them again, and object geometry stays exactly where it was.
  3. User can reorder operation layers and see the resulting production order in the panel.
  4. User can edit a layer's colour, settings, visibility, and lock from the Layers panel and see the change reflected in the canvas.
  5. The Layers panel shows operation, colour, visibility, lock, and order per layer, object rows show their memberships, and an "Unassigned" group lists exactly the zero-membership shapes without being persisted.
**Plans**: TBD
**UI hint**: yes

### Phase 3: Per-Operation Laser Output
**Goal**: A persisted job emits laser G-code per operation — power, feed, and passes — with the same shape cut in multiple ordered passes and identical bytes from every surface.
**Mode:** mvp
**Depends on**: Phase 1
**Requirements**: OUT-01, OUT-02, OUT-03, OUT-04, OUT-05
**Success Criteria** (what must be TRUE):
  1. A job containing Cut, Engrave, and Score layers emits each operation's own power, feed, and pass count in the G-code.
  2. A shape assigned to both an Engrave and a Cut layer emits two ordered passes over the same geometry.
  3. The same persisted job exported from desktop, CLI, and MCP produces byte-identical G-code.
  4. Reopening a saved job and re-exporting reproduces the same operation order and identical bytes.
  5. A job containing unassigned shapes emits none of their geometry.
**Plans**: TBD

### Phase 4: Job Planning and Laser Features
**Goal**: Job ordering measurably reduces travel while laser-specific features — tabs, overcut, and focus/Z — are represented and emitted.
**Mode:** mvp
**Depends on**: Phase 3
**Requirements**: PLAN-01, PLAN-02, PLAN-03
**Success Criteria** (what must be TRUE):
  1. The orderer applies 2-opt with start-point selection, and a regression test on a fixed job reports lower travel distance than the greedy baseline.
  2. A laser job configured with tabs and overcut emits them at the configured positions in the G-code.
  3. Focus/Z height is configurable per operation and emitted per pass in the job output.
**Plans**: TBD

## Next Milestone (Not in This Roadmap)

The following requirements are deferred to a separate GSD milestone after this
document-organization product ships:

- **STREAM-01..04**: byte-accurate GRBL in-flight window, `GcodeProtocol` integration, pause/resume/abort, mock transport.
- **RECOVERY-01..02**: durable checkpoint, beam-safe resume after reconnect.
- **RASTER-01..03**: threshold/dither/DPI/overscan/joinPixel/bidirectional parity on golden images.

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4
(Phase 3 depends only on Phase 1; Phase 2 and Phase 3 may overlap in planning, but execution stays sequential.)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Operation-First Document Model | 0/TBD | Not started | - |
| 2. Layer/Operations UI and Assignment | 0/TBD | Not started | - |
| 3. Per-Operation Laser Output | 0/TBD | Not started | - |
| 4. Job Planning and Laser Features | 0/TBD | Not started | - |
