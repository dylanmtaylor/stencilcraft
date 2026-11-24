# Requirements: Document Organization — LaserCraft/StencilCraft

**Defined:** 2026-11-24
**Core Value:** A persisted laser document can express ordered operations per object/layer and produce per-operation output identically from desktop, CLI, and MCP.

## v1 Requirements

### Document Model

- [ ] **MODEL-01**: Document stores shape geometry once, independently of operation layers.
- [ ] **MODEL-02**: An operation layer carries an `Operation`, production settings, colour, visibility, lock, and order.
- [ ] **MODEL-03**: A shape can belong to zero, one, or many operation layers; the same shape can take part in multiple ordered passes.
- [ ] **MODEL-04**: Existing `.stencilcraft` v1 documents migrate without geometry or production-data loss.
- [ ] **MODEL-05**: Shapes with no operation membership remain valid, are shown as "Unassigned", and are excluded from jobs by default.
- [ ] **MODEL-06**: `Operation` covers at least Cut, Engrave, and Score end-to-end; the existing richer operation variants remain representable.
- [ ] **MODEL-07**: Deleting an operation layer removes memberships, never shape geometry.

### Layer and Assignment UI

- [ ] **UI-01**: StencilCraft exposes a Layer/Operations menu and an add-layer flow that requires an operation.
- [ ] **UI-02**: Creating a custom layer never silently defaults to `Cut`; the chosen operation is visible and editable.
- [ ] **UI-03**: Users can add or remove one or many selected objects from a layer without moving their geometry.
- [ ] **UI-04**: Users can reorder operation layers and see their production order.
- [ ] **UI-05**: Users can edit layer colour, settings, visibility, and lock from the Layers panel.
- [ ] **UI-06**: Object rows show their operation memberships; "Unassigned" is a derived group, not a persisted layer.
- [ ] **UI-07**: Selecting a layer or object shows the effective operation and its source in the inspector.

### Laser Output

- [ ] **OUT-01**: Layout and G-code emit per-operation power, feed, and passes.
- [ ] **OUT-02**: A shape assigned to two operation layers emits two ordered passes (for example Engrave then Cut).
- [ ] **OUT-03**: Desktop, CLI, and MCP produce byte-identical output for the same persisted job.
- [ ] **OUT-04**: Reopening a job reproduces the same operation order and output bytes.
- [ ] **OUT-05**: Unassigned shapes are absent from emitted jobs unless the user explicitly assigns them.

### Planning

- [ ] **PLAN-01**: The job orderer supports 2-opt and start-point selection.
- [ ] **PLAN-02**: A travel-distance regression test measures the ordering improvement.
- [ ] **PLAN-03**: Tabs, overcut, and focus/Z are represented and emitted for laser jobs.

## v2 Requirements

Deferred to a separate GSD milestone after the document-organization product ships.

### Streaming

- **STREAM-01**: `craft-gcode` provides a byte-accurate in-flight window.
- **STREAM-02**: `GcodeProtocol` uses the window instead of one-line-per-`ok`.
- **STREAM-03**: A session state machine supports pause, resume, and abort with beam off.
- **STREAM-04**: Mock-transport tests cover acknowledgement, errors, pause, resume, and abort.

### Recovery

- **RECOVERY-01**: A job checkpoint records the completed line index.
- **RECOVERY-02**: Reconnect resumes without re-cutting completed geometry and keeps the beam off until safe.

### Raster

- **RASTER-01**: Threshold, dither, DPI, overscan, and per-pixel power match the reference behaviour.
- **RASTER-02**: `joinPixel` and bidirectional raster parity are covered by golden images.
- **RASTER-03**: Raster jobs use the same persisted operation model as vector jobs.

### Nesting (M2)

- **NEST-01**: Opt-in polygon packer with spacing, rotations, part-in-part, seeded GA, and bounded time.
- **NEST-02**: Skyline nesting remains the default and unchanged.

### Tangential Knife (M3)

- **KNIFE-01**: Tangent-orientation planner with lift/plunge and corner rules.
- **KNIFE-02**: Drag-knife output remains unchanged; golden vectors cover corners, arcs, and closed contours.

### Milling (M4)

- **MILL-01**: 2.5D profile milling with Z depth passes, work origin, spindle control, ramp entry, tabs, and tool changes.
- **MILL-02**: Pocketing only after profiles are proven.

## Out of Scope

| Feature | Reason |
|---------|--------|
| New editor, document format, UI fork, or app shell | Work must land in StencilCraft/craft-libs, not a fourth app. |
| Duplicate G-code writer, IR, machine stack, or transport | Reuse shared upstream crates. |
| AGPL/GPL translation | Licensing boundary. |
| Writes into VectorCraft documents | StencilCraft consumes VectorCraft; it does not write back. |
| General multi-firmware server | GRBL-class lasers first. |
| Persisted "Unassigned" layer | It is a derived UI group, not a production layer; persisting it risks accidental export. |
| Nesting, knife, and milling in this milestone | Parked as M2–M4; they must not distort the laser operation model. |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| MODEL-01 | Phase 1 | Pending |
| MODEL-02 | Phase 1 | Pending |
| MODEL-03 | Phase 1 | Pending |
| MODEL-04 | Phase 1 | Pending |
| MODEL-05 | Phase 1 | Pending |
| MODEL-06 | Phase 1 | Pending |
| MODEL-07 | Phase 1 | Pending |
| UI-01 | Phase 2 | Pending |
| UI-02 | Phase 2 | Pending |
| UI-03 | Phase 2 | Pending |
| UI-04 | Phase 2 | Pending |
| UI-05 | Phase 2 | Pending |
| UI-06 | Phase 2 | Pending |
| UI-07 | Phase 2 | Pending |
| OUT-01 | Phase 3 | Pending |
| OUT-02 | Phase 3 | Pending |
| OUT-03 | Phase 3 | Pending |
| OUT-04 | Phase 3 | Pending |
| OUT-05 | Phase 3 | Pending |
| PLAN-01 | Phase 4 | Pending |
| PLAN-02 | Phase 4 | Pending |
| PLAN-03 | Phase 4 | Pending |

**Coverage:**
- v1 requirements: 22 total
- Mapped to phases: 22
- Unmapped: 0 ✓

---
*Requirements defined: 2026-11-24*
*Last updated: 2026-11-24 after narrowing the milestone to document organization*
