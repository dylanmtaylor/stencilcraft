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
rows labelled `#{shape} · {name}` (the operation's name disambiguates them from
the Geometry rows) with the same `Spline` icon as the Geometry rows. Clicking a
member row selects that shape; double-clicking selects it too. Each member row
has a trash button `Remove #{shape} from {name}` that calls
`operation.remove-members`. Dragging a member row onto another operation row
moves it (`operation.move-members`); Shift-drag copies it while keeping the
source.

Acceptance: expanding an operation shows one row per existing member; removing
one updates the document and the row disappears; dragging moves the member and
Shift-dragging duplicates it.

### OP-02 — Working colour swatch

The operation row's trailing colour well reflects `OperationSetColor`. It is a
`craft_ui::controls::Swatch`, not a hand-painted rectangle, and carries no frame
of its own. Clicking it opens a popover that stays open while the picker is
used (no `Popup::menu`, which closes on the first inside click). The popover
reuses `craft_color::ui::SwatchPalette` for the 40 palette entries (8 per row,
the selection ring drawn inside the cell), with RGB controls, a `Hex` field
(`#rrggbb`) and `Clear colour`; changes commit when the popover closes. The same
popover serves the profile wells (one `ColorTarget` enum, no second picker).

Acceptance: setting `#ff0000` stores that colour; the trailing well shows it;
the palette cells show their selection ring, while the row and kind icon do not.

### OP-03 — Operation profiles

A `Profiles` dock panel lists the profile library. A profile is `{id, name,
kind, settings, color}` stored in the app preferences (`UserPrefs.profiles`), so
profiles are shared by every document and survive restarts. The default library
is Cut, Score and Engrave with default settings and a colour (no Guide, no `New
profile`); imported or extracted profiles get a fresh id and a fallback colour.

Each row is `[link + count] [colour well] [Profile {name}] [description] [cog]`.
The colour well is a `craft_ui::controls::Swatch` labelled `Profile colour
{name}`: clicking it opens the same popover as the operation well (shared
`ColorTarget`) and writes the new `Profile.color` as `#rrggbb` to the library
(persisted with the prefs, parsed back as hex — never decimal). A profile with
no stored colour takes its kind's fallback. Closing the popover recolours every
operation still linked to the profile, so canvas, geometry pills and the row
well all follow. The counter reads `{n} linked to
Profile {name}` or `No operations linked to Profile {name}`; the description is
its own column; the cog (`Profile settings {name}`) edits
power/speed/passes/focus/kerf in a popup. Footer icons run `Extract profiles`,
`Import profiles` and `Export profiles` from the panel footer, right-aligned
like Operations (`Panel::footer` + `PanelFooter`), not as a body row.

Clicking the name applies the kind and settings to the selected operations. If
any selected operation's kind+settings differ, a dialog reports `{n}
operation(s) differ. Link anyway?` with `Cancel` and `Link anyway`.

The Operations row is `[eye][lock][link][name] … [colour well]`. The kind icon is
plain, with no thumbnail frame. The link is its own toggle column beside the eye
and lock; its label describes the action (`Unlink Profile {name}` when linked,
`Link Profile {name}` when detached) and toggling adds or removes the session
detach marker without changing the operation's values. The trailing well shows
the profile colour, or the operation's own colour at half strength when
unlinked; a new link inherits the profile colour. Links are derived from
kind+settings because `Operation` has no profile field; the detach marker is
session-only (see `GAPS.md`). `Extract profiles` adds a profile for each unique
operation kind+settings, naming a colliding one `Perfil (N)`; `Import profiles`
/ `Export profiles` merge or write the library as JSON (desktop only).

Acceptance: the base rows are present with no Guide; a row links, asks before
overwriting differing settings and unlinks a selected operation; the counter,
colour well and separate description are shown; changing the well changes the
profile colour and recolours its linked operations; the link is its own column
and the colour well is trailing; the
cog opens the parameter editor; an operation with unique settings appears as a
profile after extraction.

### OP-04 — Row actions and `+`

Operation row columns: visibility (eye), lock, profile link, label, then the
trailing colour well, `…` (settings) and trash. The filter eye disappears. The
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
- Dragging an operation's member row onto another operation moves that member;
  Shift-drag copies it to the target and keeps it in the source.
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

## Implementation notes

- The Profiles footer's Import action uses `Icon::FolderOpen`: `arrow-square-in`
  is not in the pinned craft-libs icon set and the pin is not bumped.
- Unlink is a session-only marker (`detached_ops`); the document has no
  `profile_id`, so links derive from kind+settings and reset on restart.
- A member drag groups ids by source operation and issues one
  `OperationMoveMembers` per source, so a single undo reverts the whole drag.
- Colour wells and the 40-swatch popup use `craft_color` (`Swatch`,
  `SwatchTable`, `ColorPopover`, `ColorPicker`); the crate is already in the
  graph through `craft-draw` at the same pin, so it is a direct dependency, not
  a new version. The selection ring belongs to the picker's cells only.
- UI changes pass the AGENTS.md UI review gate: before/after screenshots via
  `--control` + `app.screenshot`, inspected for columns, alignment and footer
  placement.

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
| OP-01 | wip | expandable member tree + remove + member double-click + move/Shift-copy drag | done |
| OP-02 | wip | trailing `Swatch` + craft-color picker/SwatchTable popup (no row frame) | done |
| OP-03 | wip | Profiles dock: clickable colour well + counter/description + link column + differ dialog + right footer | done |
| OP-04 | wip | row actions, trash, `+` / chevron menu + appearance popup | done |
| OP-05 | wip | settings popover (no lock, no view filter) | wip |
| OP-06 | wip | drag + Ctrl+[ / ] reorder | wip |
| OP-07 | wip | assignment dialog + geometry/member drag (Shift copies) | wip |
| LY-01 | wip | contextual pills + per-operation overflow submenus | wip |

Update the Status column as each wave lands: `todo` → `wip` → `done`.
