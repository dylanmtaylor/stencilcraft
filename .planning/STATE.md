---
gsd_state_version: '1.0'
status: planning
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-11-24)

**Core value:** A persisted laser document can express ordered operations per object/layer and produce per-operation output identically from desktop, CLI, and MCP.
**Current focus:** Phase 1 — Operation-First Document Model

Note: the repo-root `ROADMAP.md` is StencilCraft's own product roadmap. This
file's roadmap is `.planning/ROADMAP.md` (the current GSD milestone plan:
document organization). Streaming, recovery, and raster are the next milestone.

## Current Position

Phase: 1 of 4 (Operation-First Document Model)
Plan: 0 of TBD in current phase
Status: Ready to plan
Last activity: 2026-11-24 — roadmap narrowed to document organization (phases 1–4)

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: —
- Total execution time: 0.0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**
- Last 5 plans: —
- Trend: —

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Geometry stored once; operation layers reference shapes (N:M), replacing `Layer.shapes: Vec<Shape>`.
- Direct object operation is represented as a one-shape layer — no new per-shape operation ownership.
- "Unassigned" is a derived UI group, never persisted and never emitted.
- Document changes go to `craft-libs`; development pins a local branch via `[patch]`.
- Current milestone is document organization (phases 1–4); streaming/recovery/raster are the next milestone.

### Pending Todos

None yet.

### Blockers/Concerns

- Cross-repo `[patch]` must be replaced by an upstreamable craft-libs revision before PR.
- `ShapeProduction.operation` users (layout grouping, `ShapesSetStyle`) need a migration path that does not silently drop per-shape overrides.
- Root `ROADMAP.md` and `docs/gaps.md` must move with any shipped StencilCraft behaviour.

## Deferred Items

Items acknowledged and deferred at milestone close, most recent first:

| Category | Item | Status | Deferred At | Milestone |
|----------|------|--------|-------------|-----------|
| *(none)* | | | | |

## Session Continuity

Last session: 2026-11-24 10:11
Stopped at: Roadmap narrowed to document organization; awaiting approval.
Resume file: None
