# Locators

Human phrase → accessible locator → source. Use this when translating a spoken
scenario into a step. Line numbers are from the worktree at the time of writing;
confirm with CodeGraph before relying on them.

| Human phrase (ES) | Locator (role + label) | Source |
| --- | --- | --- |
| "panel de geometría" | `Tree` labelled `Geometry` | `main.rs:2933` (`geometry_panel`) |
| "fila del grupo / de la forma" | `TreeItem` whose label starts with the name (`outer, 3 shapes`, `#2, #1`) | `main.rs:729` (`push_shape_row`) |
| "el martillo de una fila de geometría" | `Button` `New operation from {label}` | `main.rs:3087`, `main.rs:3342` |
| "las etiquetas de operación de una forma" | `Label` `Operation {name} on #{id}`; overflow `More operations for #{id}` | `main.rs:3148` |
| "quitar una forma de una operación" | `Button` `Remove #{shape} from {operation}` (fila miembro) | `main.rs:3450` |
| "ocultar/mostrar una operación de la vista" | menu item `Hide|Show {operation} from the geometry view` | `main.rs:3519` |
| "seleccionar los miembros de una operación" | menu item `Select members of {operation}` | `main.rs:3513` |
| "panel de operaciones" | `Tree` labelled `Operations` | `main.rs:3218` (`operations_panel`) |
| "fila de operación" | `TreeItem` `{name}, Pass {n} · {kind} · {members} · …` | `main.rs:3237` (`operations_panel`) |
| "visibilidad de una operación" | `Button` `Visibility {label}` | `main.rs:3300` |
| "filtrar una operación" | checkbox `Show {name} in geometry view` (settings popover) | `main.rs:3890` |
| "color de una operación" | `Button` `Colour {name}`; popup `Hex`, `Clear colour`, `Close` | `main.rs:3480`, `main.rs:3660` |
| "agrupar / desagrupar la selección" | `Button` `Group selection` / `Ungroup selected` | `main.rs:3047` |
| "confirmación de operación masiva" | dialog `Create {n} operations?`, `Button` `Continue` | `main.rs:3455` |
| "crear una operación por forma / para todas" | `Button` `New operation from all selected`; menú `More new operation actions` → `New operation: each/all/by colour` | `main.rs:3390` |
| "acciones de una operación" | `Button` `Operation settings {name}`, `Delete operation {name}`, `More` | `main.rs:3500` |
| "perfiles" | panel `Profiles` + entrada en `Paneles` | `main.rs:4410`, `menus.rs` |
| "inspector" | `Header` `Shape #1 - Path` / `Document` | `main.rs:3644` (`inspector_panel`) |
| "operación efectiva de la selección" | section `Operation`, values exposed as accesskit `value` (`Cut`, source name) | `main.rs:3684` (`inspector`) |
| "capa heredada" | `Button` `Add Layer`, section `Layers` | `main.rs:3815`, `main.rs:3813` |
| "menú Paneles" | `MenuItem` `Panels` → `Geometry`, `Operations` | `menus.rs:89`, `menus.rs:648` |

## Notes

- `PropertyRow::word` draws a visible word but the accessible content is the
  row's `value`, so inspector assertions go through the header subtree and
  `value` (see `inspector_shows` in `specs.rs`).
- Rows and toggle cells need real pointer press/release frames; use
  `raw_click_label` / `raw_click_row_containing`, not `Node::click`.
- The menus live in `top_panel`; mount it with `boot_menu` to test reachability.
