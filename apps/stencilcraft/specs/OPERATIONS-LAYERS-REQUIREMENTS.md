# Operations and Layers UX requirements

Status: agreed 2026-11-25. Source: user review of the running app
(`/tmp/stencilcraft-running.png` and the Operations/Geometry panels).

This file is the contract for the next UI waves. Specs live beside it under
`specs/features/`; a requirement is done when its baseline scenario passes and
`GAPS.md` no longer lists it.

## Glossary

- **Operation / pass**: one entry in `Document.operations`; the list order is
  the pass order. `members` holds shape ids only.
- **Member row**: the shape row shown under an expanded operation.
- **Geometry row**: a row of the Geometry panel (group, shape or Unassigned).
- **Pill**: the coloured operation tag drawn on a geometry row.
- **Layers**: the Geometry panel rows. The legacy document-level `doc.layers`
  section is out of scope.

## Decisions

- Tags are a trailing pill cluster, not inline inside the row label:
  `craft_ui::ListRow` cannot host widgets in its label, and extending craft-ui
  is a separate change.
- **Geometry never creates operations.** The per-row cog/hammer and the
  `New operation from {label}` control are removed; operations are created only
  from the Operations footer `+` (or the Inspector's "Add to operation").
- **Pills are contextual.** Clicking a pill opens only that operation's actions
  (remove membership, hide/show, select members). The `…` overflow opens one
  submenu per operation after the first three pills; there is no flat
  all-operations list.
- **No view-only filter.** `hidden_ops`, the `Filter`/`Unfilter` tag actions, the
  `Show {name} in geometry view` checkbox and `shape_filtered_out` are removed.
  Geometry rows carry no operation ids in their detail.
- **Lock is a visible list toggle.** The Operations list has an eye (visibility)
  and a lock column; the lock checkbox leaves the settings popover.
- **Colour popup is palette-backed.** It offers 40 palette swatches plus the
  inline RGB sliders, `Hex`, `Clear colour` and `Close`.
- Profiles list the base Cut, Score and Engrave profiles (no Guide, no "New
  profile"). A new operation links to Cut; editing its numeric settings unlinks
  it (`Profile none`). Profile values are provisional.
- `+` main click creates one operation over every selected shape. Shift+click
  (or the chevron) opens the menu: each selected, all selected, by appearance.
  The appearance chooser lists existing stroke/fill/both values from Geometry and
  creates one operation over every matching shape.
- Deleting an operation needs no confirmation: `operation.delete` is undoable.

## Requirements

### OP-01 — Collapsible operation rows with members

An operation row is a tree group. Expanding it lists its member shapes as child
rows labelled `#{shape}` with the same `Spline` icon as the Geometry rows.
Clicking a member row selects that shape. Each member row has a trash
button `Remove #{shape} from {name}` that calls `operation.remove-members`.

Acceptance: expanding an operation shows one row per existing member; removing
one updates the document and the row disappears.

### OP-02 — Working colour swatch

The row swatch reflects `OperationSetColor`. Clicking it opens a popover that
stays open while the inline picker is used (no `Popup::menu`, which closes on
the first inside click). The popover offers an inline picker, a `Hex` field
(`#rrggbb`) and `Clear colour`; changes commit when the popover closes.

Acceptance: setting `#ff0000` stores that colour; the swatch shows it.

### OP-03 — Operation profiles stub

A `Profiles` dock panel, listed under Panels, listing the base Cut, Score and
Engrave profiles (no Guide). Profile values are provisional: a profile's
defaults are its kind with default settings. Clicking a profile row applies
those defaults to every selected operation, linking it back after an edit; a
new operation links to Cut, and editing its numeric settings unlinks it
(`Profile none`). No profile data model and no "New profile" action in this
wave.

### OP-04 — Row actions and `+`

Operation row trailing actions: visibility (eye), colour swatch, `…`
(settings) and trash. The filter eye and the per-row gear disappear. The
`…` button opens the settings popover (OP-05), not a contextual enum menu.

The Operations footer has one `+` control:

- LMB: one operation over all selected shapes (`BulkKind::AllSelected`).
- Shift+LMB or chevron: menu with each selected, all selected, by colour
  (existing `BulkKind`s and the bulk confirm threshold).
- Disabled with no selection.

"Duplicate pass" moves into the settings popover.

Acceptance: each path creates the same operations as the old footer buttons;
the footer no longer shows three separate bulk buttons.

### OP-05 — Settings popover

`…` opens a popover anchored to the row. It contains: kind (`Select`), lock,
"Show {name} in geometry view", power, speed, passes, focus, kerf,
"Duplicate pass {name}", "Move up {name}" and "Move down {name}". Kind, lock,
visibility, move and duplicate apply immediately; numeric settings commit on
change. The popover uses `Popup::from_response` with
`PopupCloseBehavior::CloseOnClickOutside`.

Acceptance: kind, lock, filter and power can be changed from the popover; the
old modal settings dialog is gone.

### OP-06 — Reorder

`List::reorder(Reorder::new(Axis::Vertical))` with `can_drag` limited to
operation rows. A drop maps `Move { target, position }` to `OperationMove`:
`to = target + (After ? 1 : 0)`, then `to -= 1` when `from < to`. `Ctrl+[` and
`Ctrl+]` use the same path. Member rows are not draggable. Selection is cleared
after a move because operation keys are indices.

Acceptance: dragging (or the keyboard) changes pass order; members follow their
operation.

### OP-07 — Assign the selection to an operation (UI-03)

The Inspector shows `Add to operation` when shapes are selected. It opens a
dialog titled `Choose an operation` listing the unlocked operations; choosing
one calls `OperationAddMembers`. This is how an existing (including empty)
operation receives objects.

Faster paths mirror that command:

- With geometry selected and an operation (or one of its member rows) selected,
  the Operations footer's `Add selected to current operation` button adds the
  whole geometry selection to that operation.
- The Operations chevron menu offers `All selected to current operation` for
  the same action.
- Dragging one or more Geometry shape rows onto an operation row adds the
  dragged shapes. Group rows do not drag.
- A locked operation refuses with a status message.

Acceptance: each path adds exactly the selected ids to the current operation;
the member-row trash lines up with the operation rows' trash column.

### LY-01 — Operation pills on geometry rows

Geometry shape rows show their operation tags as coloured pills in a trailing
cluster, with the operation name as text and `operation_swatch_color` as fill.
When the cluster does not fit, it shows a `…` pill with the remaining count.
Clicking a pill or `…` opens the existing tag overflow menu (remove membership,
hide/show, select members, view filter). The per-row gear buttons disappear.

Acceptance: a shape with an operation shows a pill named after the operation;
the old `Operations of #{id}` button is gone; the overflow menu still removes
memberships.

## Out of scope

- The legacy `doc.layers` model and Send to Layer.
- Profile storage, editing or machine mapping.
- UI-01 "Add shape" (still a gap).
- Changing operation identity or the N:M membership model.

## Spec map

| Requirement | Feature |
| --- | --- |
| OP-01 | `operations_panel.feature` (member rows) |
| OP-02 | `operations_panel.feature` (colour) |
| OP-03 | `profiles_panel.feature` |
| OP-04 | `operations_panel.feature`, `bulk_operations.feature` |
| OP-05 | `operations_panel.feature` (settings popover) |
| OP-06 | `operations_panel.feature` (reorder) |
| OP-07 | `operation_assignment.feature` |
| LY-01 | `geometry_panel.feature` (pills) |

## Implementation ledger

| ID | Spec | UI | Status |
| --- | --- | --- | --- |
| OP-01 | wip | expandable member tree + remove + member double-click | wip |
| OP-02 | wip | colour picker popup + 40-swatch palette | done |
| OP-03 | wip | Profiles dock panel: Cut/Score/Engrave + provisional link | done |
| OP-04 | wip | row actions, trash, `+` / chevron menu + appearance popup | done |
| OP-05 | wip | settings popover (no lock, no view filter) | wip |
| OP-06 | wip | drag + Ctrl+[ / ] reorder | wip |
| OP-07 | wip | assignment dialog | wip |
| LY-01 | wip | contextual pills + per-operation overflow submenus | wip |

Update the Status column as each wave lands: `todo` → `wip` → `done`.
