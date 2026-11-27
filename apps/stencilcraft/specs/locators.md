# Locators

Human phrase → accessible locator → source. Use this when translating a spoken
scenario into a step. Line numbers are from the worktree at the time of writing;
confirm with CodeGraph before relying on them.

| Human phrase (ES) | Locator (role + label) | Source |
| --- | --- | --- |
| "panel de geometría" | `Tree` labelled `Geometry` | `main.rs` `geometry_panel` |
| "fila del grupo / de la forma" | `TreeItem` whose label starts with the name (`outer, 3 shapes`, `#2`) | `main.rs` `push_shape_row` |
| "las etiquetas de operación de una forma" | `Label` `Operation {name} on #{id}` (contextual: opens that operation's actions) | `main.rs` `geometry_panel` trailing cluster |
| "el desbordamiento de etiquetas" | `Button` `More operations for #{id}`; one `MenuItem` submenu per operation after the first three pills | `main.rs` `tag_overflow_popup` |
| "quitar una forma de una operación" | menu item `Remove #{shape} from {operation}` | `main.rs` `tag_overflow_popup` |
| "ocultar/mostrar una operación" | menu item `Hide|Show {operation}` (document visibility) | `main.rs` `tag_overflow_popup` |
| "seleccionar los miembros de una operación" | menu item `Select members of {operation}` | `main.rs` `tag_overflow_popup` |
| "panel de operaciones" | `Tree` labelled `Operations` | `main.rs` `operations_panel` |
| "fila de operación" | `TreeItem` `{name}, Pass {n} · {kind} · … · Profile {label|none}`; child `Button` `Unlink Profile {name}` (only when linked) and trailing `Button` `Colour {name}` | `main.rs` `operations_panel` |
| "visibilidad de una operación" | toggle `Button` `Visibility {label}` (column 0) | `main.rs` `operations_panel` |
| "bloquear una operación" | toggle `Button` `Lock {name}` (column 1) → `OperationSetLocked` | `main.rs` `operations_panel`, `craft_ui::list::ToggleColumn::lock` |
| "fila miembro" | `TreeItem` `#{shape} · {operation}`; click/double-click selects the shape; drag onto an operation row moves it, Shift-drag copies it | `main.rs` `operations_panel` |
| "quitar un miembro" | `Button` `Remove #{shape} from {name}` (member row trailing) | `main.rs` `operations_panel` |
| "color de una operación" | `Button` `Colour {name}`; popup palette `Palette colour 1`…`40`, `Hex`, `Clear colour`, `Close` | `main.rs` `operation_color_popup`, `OPERATION_PALETTE` |
| "ajustes de una operación" | `Button` `Operation settings {name}`; popover `Kind`, `Power`, `Speed`, `Passes`, `Focus`, `Kerf` | `main.rs` `operation_settings_popup` |
| "agrupar / desagrupar la selección" | `Button` `Group selection` / `Ungroup selected` | `main.rs` `geometry_panel` footer |
| "confirmación de operación masiva" | dialog `Create {n} operations?`, `Button` `Continue` | `main.rs` `bulk_confirm_dialog` |
| "crear una operación por forma / para todas / por apariencia" | `Button` `New operation from all selected`; menu `More new operation actions` → `New operation: each selected` / `all selected` / `by appearance` | `main.rs` `bulk_menu_popup` |
| "ventana por apariencia" | window `New operation by appearance` with `Stroke`, `Fill`, `Both`; value buttons `{mode} {value}` | `main.rs` appearance dialog |
| "añadir la geometría seleccionada a la operación actual" | footer button `Add selected to current operation`; menu item `All selected to current operation`; drag a Geometry row onto an operation row | `main.rs` `operations_panel`, `geometry_panel` |
| "perfiles" | panel `Profiles`: swatch `Profile colour {name}`, button `Profile {name}` (links), separate description, counter `{n} linked to Profile {name}` / `No operations linked to Profile {name}`, cog `Profile settings {name}`; footer `Extract profiles`, `Import profiles`, `Export profiles`. A differing selection opens `{n} operation(s) differ. Link anyway?` with `Cancel` / `Link anyway`. No `Profile Guide`, no `New profile` | `main.rs` `profiles_panel` |
| "inspector" | `Header` `Shape #1 - Path` / `Document` | `main.rs` `inspector_panel` |
| "operación efectiva de la selección" | section `Operation`, values exposed as accesskit `value` (`Cut`, source name) | `main.rs` inspector |
| "capa heredada" | `Button` `Add Layer`, section `Layers` | `main.rs` inspector |
| "menú Paneles" | `MenuItem` `Panels` → `Geometry`, `Operations`, `Profiles` | `menus.rs` |

## Notes

- `PropertyRow::word` draws a visible word but the accessible content is the
  row's `value`, so inspector assertions go through the header subtree and
  `value` (see `inspector_shows` in `specs.rs`).
- Rows and toggle cells need real pointer press/release frames; use
  `raw_click_label` / `raw_click_row_containing`, not `Node::click`. Double-click
  a member row with two press/release pairs in one frame batch.
- Overflow submenus are `MenuItem` rows (submenu rows keep `Role::MenuItem`); the
  `the menu item "…" is on screen` step checks that role.
- The menus live in `top_panel`; mount it with `boot_menu` to test reachability.
