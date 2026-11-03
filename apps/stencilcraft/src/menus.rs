//! The menu bar: Silhouette Studio's menus and item order where the item exists there, with
//! Illustrator/VectorCraft items added after them and in the Type, Select, Effect and Machine
//! menus (`docs/menu-map.md`). Items with no engine command yet are listed disabled with a
//! "Not yet implemented" tooltip, so the structure is complete and users find where things
//! will be.

use craft_ui::controls::Kbd;
use craft_ui::menu::{MenuEntry, MenuItem};
use eframe::egui::{Key, KeyboardShortcut, Modifiers};
use stencilcraft_doc::CutStyle;
use stencilcraft_engine::{Alignment, BooleanOperation, DistributionAxis, NodeAxis};

use crate::panels::{EditAction as E, ScreenSide};
use crate::{StencilApp, TopMenuAction as A, WorkspacePanel as P};

/// The tooltip of an item whose command does not exist yet.
pub const NOT_YET: &str = "Not yet implemented";

const CMD: Modifiers = Modifiers::COMMAND;
const NONE: Modifiers = Modifiers::NONE;

fn cmd_shift() -> Modifiers {
    Modifiers::COMMAND | Modifiers::SHIFT
}

fn cmd_alt() -> Modifiers {
    Modifiers::COMMAND | Modifiers::ALT
}

fn kbd(modifiers: Modifiers, key: Key) -> Kbd {
    Kbd::new(&KeyboardShortcut::new(modifiers, key))
}

/// A working item.
fn item(label: &str, action: A) -> MenuItem<A> {
    MenuItem::new(label, action)
}

/// A working item that is enabled only when `enabled`.
fn when(label: &str, action: A, enabled: bool) -> MenuEntry<A> {
    item(label, action).enabled(enabled).into()
}

/// A planned item: shown, disabled, with the "Not yet implemented" tooltip.
fn todo(label: &str) -> MenuEntry<A> {
    item(label, A::NotYet)
        .enabled(false)
        .tooltip(NOT_YET)
        .into()
}

/// A planned item with its future shortcut shown.
fn todo_key(label: &str, modifiers: Modifiers, key: Key) -> MenuEntry<A> {
    item(label, A::NotYet)
        .shortcut(kbd(modifiers, key))
        .enabled(false)
        .tooltip(NOT_YET)
        .into()
}

/// An editing action enabled only when `enabled`.
fn edit(label: &str, action: E, enabled: bool) -> MenuEntry<A> {
    when(label, A::Edit(action), enabled)
}

/// An editing action with its shortcut.
fn edit_key(label: &str, action: E, enabled: bool, modifiers: Modifiers, key: Key) -> MenuEntry<A> {
    item(label, A::Edit(action))
        .shortcut(kbd(modifiers, key))
        .enabled(enabled)
        .into()
}

fn sub(label: &str, children: Vec<MenuEntry<A>>) -> MenuEntry<A> {
    MenuItem::submenu(label, children).into()
}

const SEP: MenuEntry<A> = MenuEntry::Separator;

/// The whole menu bar for the app's current state. Help stays last (the bar's help search).
pub fn build(app: &StencilApp) -> Vec<MenuItem<A>> {
    vec![
        MenuItem::submenu("File", file(app)),
        MenuItem::submenu("Edit", edit(app)),
        MenuItem::submenu("Object", object(app)),
        MenuItem::submenu("Type", type_menu()),
        MenuItem::submenu("Select", select(app)),
        MenuItem::submenu("Effect", effect()),
        MenuItem::submenu("View", view(app)),
        MenuItem::submenu("Panels", panels(app)),
        MenuItem::submenu("Machine", machine(app)),
        MenuItem::submenu("Help", help()),
    ]
}

fn file(app: &StencilApp) -> Vec<MenuEntry<A>> {
    let recent = if app.recent_files.is_empty() {
        vec![when("No Recent Documents", A::NotYet, false)]
    } else {
        app.recent_files
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .unwrap_or(path);
                item(name, A::OpenRecent(i)).into()
            })
            .collect()
    };
    vec![
        item("New", A::NewDoc).shortcut(kbd(CMD, Key::N)).into(),
        todo_key("New from Template…", cmd_shift(), Key::N),
        item("Open…", A::OpenDoc).shortcut(kbd(CMD, Key::O)).into(),
        sub("Open Recent", recent),
        todo("Open Recovered…"),
        item("Merge…", A::ImportSvg)
            .shortcut(kbd(cmd_shift(), Key::O))
            .into(),
        sub(
            "Import",
            vec![
                item("SVG…", A::ImportSvg).into(),
                item("Image (Trace)…", A::ImportImage).into(),
                todo("DXF…"),
                todo("PDF…"),
                todo("AI / EPS…"),
            ],
        ),
        SEP,
        item("Save", A::SaveDoc).shortcut(kbd(CMD, Key::S)).into(),
        item("Save As…", A::SaveAsDoc)
            .shortcut(kbd(cmd_shift(), Key::S))
            .into(),
        todo("Save Selection As…"),
        sub(
            "Export",
            vec![
                item("SVG…", A::ExportSvg).into(),
                item("Print & Cut SVG…", A::ExportPrintCutSvg).into(),
                item("DXF (R14 ASCII)…", A::ExportDxf).into(),
                item("G-code (Grbl)…", A::ExportGcode).into(),
                item("Tiled SVG Sheets…", A::ExportTiledSvg).into(),
                SEP,
                todo("PDF…"),
                todo("PNG…"),
                todo("JPEG…"),
            ],
        ),
        SEP,
        todo("Library…"),
        todo("Scan…"),
        SEP,
        todo_key("Print…", CMD, Key::P),
        todo("Print Page Setup…"),
        item("Send to Machine…", A::ShowSend)
            .shortcut(kbd(CMD, Key::L))
            .into(),
        todo("Document Setup…"),
        SEP,
        todo_key("Close Tab", CMD, Key::W),
        item("Quit", A::Quit).shortcut(kbd(CMD, Key::Q)).into(),
    ]
}

fn edit(app: &StencilApp) -> Vec<MenuEntry<A>> {
    let any = !app.selected_shapes.is_empty();
    vec![
        item("Undo", A::Undo)
            .shortcut(kbd(CMD, Key::Z))
            .enabled(app.engine.can_undo())
            .into(),
        item("Redo", A::Redo)
            .shortcut(kbd(cmd_shift(), Key::Z))
            .enabled(app.engine.can_redo())
            .into(),
        SEP,
        todo_key("Cut", CMD, Key::X),
        todo_key("Copy", CMD, Key::C),
        todo_key("Paste", CMD, Key::V),
        todo_key("Paste in Front", CMD, Key::F),
        todo_key("Paste in Back", CMD, Key::B),
        todo_key("Paste in Place", cmd_shift(), Key::V),
        item("Delete", A::Delete)
            .shortcut(kbd(NONE, Key::Delete))
            .enabled(any)
            .into(),
        SEP,
        item("Duplicate", A::Duplicate)
            .shortcut(kbd(CMD, Key::D))
            .enabled(any)
            .into(),
        item("Select All", A::SelectAll)
            .shortcut(kbd(CMD, Key::A))
            .into(),
        item("Deselect All", A::Deselect)
            .shortcut(kbd(cmd_shift(), Key::A))
            .enabled(any)
            .into(),
        todo("Select by Color…"),
        SEP,
        todo("History…"),
        todo_key("Preferences…", CMD, Key::K),
    ]
}

fn object(app: &StencilApp) -> Vec<MenuEntry<A>> {
    let any = !app.selected_shapes.is_empty();
    let many = app.selected_shapes.len() >= 2;
    let nodes = !app.path_selection.anchors.is_empty();
    let pathfinder = |label: &str, op: BooleanOperation| when(label, A::Pathfinder(op), many);
    let align = |label: &str, a: Alignment| when(label, A::Align(a), many);
    vec![
        sub(
            "Mirror",
            vec![todo("Flip Horizontal"), todo("Flip Vertical")],
        ),
        sub(
            "Arrange",
            vec![
                todo_key("Bring to Front", cmd_shift(), Key::CloseBracket),
                todo_key("Bring Forward", CMD, Key::CloseBracket),
                todo_key("Send Backward", CMD, Key::OpenBracket),
                todo_key("Send to Back", cmd_shift(), Key::OpenBracket),
                SEP,
                todo("Send to Layer…"),
            ],
        ),
        sub(
            "Align",
            vec![
                align("Left", Alignment::Left),
                align("Center", Alignment::HorizontalCenter),
                align("Right", Alignment::Right),
                align("Top", Alignment::Top),
                align("Middle", Alignment::VerticalCenter),
                align("Bottom", Alignment::Bottom),
                SEP,
                when("Center to Page", A::CenterToPage, any),
                todo("To Key Object"),
                SEP,
                when(
                    "Space Horizontally",
                    A::Distribute(DistributionAxis::Horizontal),
                    many,
                ),
                when(
                    "Space Vertically",
                    A::Distribute(DistributionAxis::Vertical),
                    many,
                ),
                todo("Distribute Spacing by Value…"),
            ],
        ),
        sub(
            "Rotate",
            vec![
                when("Rotate 15° Clockwise", A::RotateBy(15), any),
                when("Rotate 15° Counterclockwise", A::RotateBy(-15), any),
                when("Rotate 90° Clockwise", A::RotateBy(90), any),
                when("Rotate 90° Counterclockwise", A::RotateBy(-90), any),
                when("Rotate 180°", A::RotateBy(180), any),
                todo("Custom…"),
            ],
        ),
        sub(
            "Replicate",
            vec![
                edit("Duplicate Left", E::Duplicate(ScreenSide::Left), any),
                edit("Duplicate Right", E::Duplicate(ScreenSide::Right), any),
                edit("Duplicate Above", E::Duplicate(ScreenSide::Above), any),
                edit("Duplicate Below", E::Duplicate(ScreenSide::Below), any),
                edit("Row of Copies", E::Array(DistributionAxis::Horizontal), any),
                edit("Column of Copies", E::Array(DistributionAxis::Vertical), any),
                edit("Fill Page", E::FillPage, any),
                edit("Mirror Copies", E::Mirror(ScreenSide::Right), any),
                edit("Rotate Copies", E::RotateCopies, any),
                SEP,
                item("Replicate Options…", A::ShowPanel(P::Replicate)).into(),
            ],
        ),
        sub(
            "Transform",
            vec![
                item("Move…", A::ShowPanel(P::Transform)).into(),
                item("Scale…", A::ShowPanel(P::Transform)).into(),
                item("Rotate…", A::ShowPanel(P::Transform)).into(),
                todo("Shear…"),
                SEP,
                todo_key("Transform Again", cmd_shift(), Key::D),
                todo("Transform Each…"),
            ],
        ),
        sub(
            "Modify",
            vec![
                item("Weld", A::Pathfinder(BooleanOperation::Union))
                    .shortcut(kbd(cmd_shift(), Key::W))
                    .enabled(many)
                    .into(),
                pathfinder("Subtract", BooleanOperation::Difference),
                pathfinder("Subtract All", BooleanOperation::SubtractAll),
                pathfinder("Intersect", BooleanOperation::Intersect),
                pathfinder("Exclude", BooleanOperation::Xor),
                pathfinder("Divide", BooleanOperation::Divide),
                pathfinder("Crop", BooleanOperation::Crop),
                SEP,
                pathfinder("Trim", BooleanOperation::Trim),
                pathfinder("Merge", BooleanOperation::Merge),
                pathfinder("Outline", BooleanOperation::Outline),
                pathfinder("Minus Back", BooleanOperation::MinusBack),
                SEP,
                todo("Detach Lines"),
                SEP,
                item("Modify Panel…", A::ShowPanel(P::Modify)).into(),
            ],
        ),
        sub(
            "Line Style",
            vec![
                item("Cut", A::SetStyle(CutStyle::Cut)).enabled(any).into(),
                item("Engrave / Score", A::SetStyle(CutStyle::Engrave))
                    .enabled(any)
                    .into(),
            ],
        ),
        SEP,
        todo_key("Group", CMD, Key::G),
        todo_key("Ungroup", cmd_shift(), Key::G),
        todo("Attach / Detach"),
        edit_key("Lock", E::Lock, any, CMD, Key::Num2),
        edit_key("Unlock All", E::UnlockAll, true, cmd_alt(), Key::Num2),
        edit_key("Hide", E::Hide, any, CMD, Key::Num3),
        edit_key("Show All", E::ShowAll, true, cmd_alt(), Key::Num3),
        SEP,
        edit("Convert to Path", E::ToPath, any),
        edit_key("Make Compound Path", E::MakeCompound, many, CMD, Key::E),
        edit_key("Release Compound Path", E::ReleaseCompound, any, cmd_shift(), Key::E),
        sub(
            "Clipping Mask",
            vec![
                todo_key("Make", CMD, Key::Num7),
                todo_key("Release", cmd_alt(), Key::Num7),
            ],
        ),
        edit_key("Simplify", E::Simplify, any, CMD, Key::J),
        sub(
            "Path",
            vec![
                edit_key("Join", E::Join, any, cmd_shift(), Key::J),
                edit_key("Average", E::Average(NodeAxis::Both), nodes, cmd_alt(), Key::J),
                edit("Break at Point", E::Break, nodes),
                edit("Close Path", E::Close, any),
                edit("Merge Points", E::MergeNodes, nodes),
                edit("Delete Points", E::DeleteNodes, nodes),
                SEP,
                edit("Offset Path…", E::Offset { internal: false }, true),
                edit("Outline Stroke", E::OutlineStroke, any),
                SEP,
                item("Point Editing Panel…", A::ShowPanel(P::Points)).into(),
            ],
        ),
        todo("Expand"),
        todo("Contour"),
        todo("Flatten"),
    ]
}

fn type_menu() -> Vec<MenuEntry<A>> {
    vec![
        sub("Font", vec![todo("System Fonts")]),
        sub("Size", vec![todo("Custom…")]),
        todo_key("Character", CMD, Key::T),
        todo_key("Paragraph", cmd_alt(), Key::T),
        todo("Glyphs"),
        SEP,
        item("Place Lettering…", A::PlaceText)
            .shortcut(kbd(NONE, Key::T))
            .into(),
        todo("Type on a Path"),
        todo("Curve Text…"),
        todo("Create Outlines"),
        todo("Ungroup to Letters"),
        SEP,
        todo("Find Font…"),
        todo_key("Spelling…", NONE, Key::F7),
    ]
}

fn select(app: &StencilApp) -> Vec<MenuEntry<A>> {
    let any = !app.selected_shapes.is_empty();
    vec![
        item("All", A::SelectAll).shortcut(kbd(CMD, Key::A)).into(),
        item("Deselect", A::Deselect)
            .shortcut(kbd(cmd_shift(), Key::A))
            .enabled(any)
            .into(),
        item("Inverse", A::SelectInverse).into(),
        todo("Select by Color…"),
        SEP,
        sub(
            "Same",
            vec![
                when("Operation (Cut / Engrave)", A::SelectSameStyle, any),
                when("Layer", A::SelectSameLayer, any),
                edit("Fill Color", E::SelectSameFill, any),
                edit("Stroke Color", E::SelectSameStroke, any),
                todo("Stroke Weight"),
            ],
        ),
        todo("Next Object Above"),
        todo("Next Object Below"),
        SEP,
        todo("Save Selection…"),
        todo("Edit Selection…"),
    ]
}

fn effect() -> Vec<MenuEntry<A>> {
    vec![
        item("Offset…", A::Edit(E::Offset { internal: false })).into(),
        item("Internal Offset…", A::Edit(E::Offset { internal: true })).into(),
        item("Kerf Compensation…", A::ToggleKerfPreview).into(),
        todo("Bridges…"),
        todo("Weed Lines and Border…"),
        SEP,
        item("Trace Image…", A::ImportImage).into(),
        todo("Remove Background…"),
        todo("Image Effects…"),
        SEP,
        todo("Sketch Fill…"),
        todo("Stipple…"),
        todo("Emboss / Deboss Patterns…"),
        todo("Rhinestone…"),
        sub(
            "Warp",
            vec![
                todo("Arc"),
                todo("Flag"),
                todo("Bulge"),
                todo("Perspective…"),
            ],
        ),
        sub("Distort", vec![todo("Shear"), todo("Free Distort")]),
        todo("Lattice…"),
        todo("Pop-up Card…"),
        todo("Puzzle…"),
        SEP,
        todo("Apply Last Effect"),
    ]
}

fn view(app: &StencilApp) -> Vec<MenuEntry<A>> {
    vec![
        sub(
            "Zoom",
            vec![
                item("Zoom In", A::ZoomIn)
                    .shortcut(kbd(CMD, Key::Plus))
                    .into(),
                item("Zoom Out", A::ZoomOut)
                    .shortcut(kbd(CMD, Key::Minus))
                    .into(),
                item("Fit to Page", A::FitView)
                    .shortcut(kbd(NONE, Key::F))
                    .into(),
                item("Center Page", A::CenterView)
                    .shortcut(kbd(NONE, Key::C))
                    .into(),
                item("Actual Size", A::ActualSize)
                    .shortcut(kbd(CMD, Key::Num1))
                    .into(),
                todo("Zoom to Selection"),
            ],
        ),
        sub(
            "Rotate Page",
            vec![todo("0°"), todo("90°"), todo("180°"), todo("270°")],
        ),
        SEP,
        todo("Grid Options…"),
        item("Show Grid", A::ToggleGrid)
            .shortcut(kbd(CMD, Key::Quote))
            .check(app.prefs.show_grid)
            .into(),
        todo("Snap to Grid"),
        todo("Smart Snapping"),
        todo_key("Smart Guides", CMD, Key::U),
        todo("Snap to Point"),
        item("Rulers", A::ToggleRulers)
            .shortcut(kbd(cmd_shift(), Key::R))
            .check(app.prefs.show_rulers)
            .into(),
        sub(
            "Guides",
            vec![
                todo("Show Guides"),
                todo("Lock Guides"),
                todo_key("Make Guides", CMD, Key::Num5),
                todo_key("Release Guides", cmd_alt(), Key::Num5),
            ],
        ),
        SEP,
        todo_key("Outline View", CMD, Key::Y),
        todo_key("Show Print Border", cmd_shift(), Key::P),
        todo_key("Show Cut Border", cmd_shift(), Key::L),
        todo("Show Dimensions"),
        item("Preview Kerf Compensation", A::ToggleKerfPreview)
            .check(app.kerf_preview)
            .into(),
        item("Registration Marks", A::TogglePrintCutMarks)
            .check(app.prefs.print_cut_marks)
            .into(),
        todo("Design Page Settings…"),
        SEP,
        item("Show Design", A::ShowDesign)
            .radio(!app.send_mode)
            .into(),
        item("Show Send", A::ShowSend).radio(app.send_mode).into(),
    ]
}

fn panels(app: &StencilApp) -> Vec<MenuEntry<A>> {
    use stencilcraft_ui_egui::ColorTheme;
    vec![
        todo("Page Setup"),
        todo("PixScan"),
        todo("Fill Color"),
        todo("Line Style"),
        todo("Select by Color"),
        item("Trace", A::ImportImage).into(),
        todo("Image Effects"),
        todo("Text Style"),
        todo("Spelling"),
        item("Transform", A::ShowPanel(P::Transform)).into(),
        item("Replicate", A::ShowPanel(P::Replicate)).into(),
        item("Modify", A::ShowPanel(P::Modify)).into(),
        item("Offset", A::ShowPanel(P::Offset)).into(),
        todo("Pop up"),
        todo("Sketch"),
        todo("Specialty"),
        item("Print & Cut", A::TogglePrintCutMarks).into(),
        item("Point Editing", A::ShowPanel(P::Points)).into(),
        item("Layers", A::ShowPanel(crate::WorkspacePanel::Layers)).into(),
        SEP,
        item("Properties", A::ShowPanel(crate::WorkspacePanel::Inspector)).into(),
        item("Tools", A::ShowPanel(crate::WorkspacePanel::Tools)).into(),
        todo("Swatches"),
        todo("Recolor Artwork"),
        todo("Graphic Styles"),
        todo("Actions"),
        todo("Libraries"),
        SEP,
        item("Reset Panels to Default", A::ResetWorkspace).into(),
        todo("Starter Mode"),
        sub(
            "Theme",
            vec![
                item("Dark", A::SetTheme(ColorTheme::Dark))
                    .radio(app.theme == ColorTheme::Dark)
                    .into(),
                item("Light", A::SetTheme(ColorTheme::Light))
                    .radio(app.theme == ColorTheme::Light)
                    .into(),
            ],
        ),
    ]
}

fn machine(app: &StencilApp) -> Vec<MenuEntry<A>> {
    vec![
        item("Send…", A::ShowSend).shortcut(kbd(CMD, Key::L)).into(),
        todo("Connect / Rescan Devices"),
        sub(
            "Send Mode",
            vec![
                todo("Standard"),
                todo("By Line Color"),
                todo("By Fill Color"),
                todo("By Layer"),
            ],
        ),
        todo("Tool Holders…"),
        item("Material Settings…", A::ShowSend).into(),
        todo("Mat Preview"),
        todo("Mirror on Mat"),
        sub(
            "Load Type",
            vec![todo("On Mat"), todo("Without Mat"), todo("Roll")],
        ),
        SEP,
        todo("Test Cut"),
        when("Dry Run", A::DryRun, app.engine.doc.shape_count() > 0),
        todo("Cut"),
        todo("Pause"),
        todo("Cancel"),
        SEP,
        todo("Load / Unload Media"),
        todo("Register Print & Cut Marks"),
        todo("Calibration…"),
        SEP,
        item("Export G-code for Laser…", A::ExportGcode).into(),
        todo("Multi-Cutter Jobs…"),
    ]
}

fn help() -> Vec<MenuEntry<A>> {
    vec![
        item("Documentation", A::HelpManual)
            .shortcut(kbd(NONE, Key::F1))
            .into(),
        item("Keyboard Shortcuts", A::HelpCheatsheet).into(),
        todo("Tutorials"),
        SEP,
        item("About StencilCraft", A::HelpAbout).into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk(entries: &[MenuEntry<A>], out: &mut Vec<(String, bool, A)>) {
        for entry in entries {
            if let Some(item) = entry.item() {
                if let Some(children) = item.children() {
                    walk(children, out);
                } else if let Some(command) = item.command() {
                    out.push((item.label().to_string(), item.is_enabled(), *command));
                }
            }
        }
    }

    #[test]
    fn top_level_order_follows_the_menu_map() {
        let app = StencilApp::new(None);
        let names: Vec<String> = build(&app).iter().map(|m| m.label().to_string()).collect();
        assert_eq!(
            names,
            [
                "File", "Edit", "Object", "Type", "Select", "Effect", "View", "Panels", "Machine",
                "Help"
            ]
        );
    }

    #[test]
    fn planned_items_are_disabled_and_never_dispatch() {
        let app = StencilApp::new(None);
        let mut items = Vec::new();
        for menu in build(&app) {
            walk(menu.children().unwrap_or_default(), &mut items);
        }
        assert!(items.len() > 150, "menu has {} leaf items", items.len());
        for (label, enabled, command) in &items {
            if *command == A::NotYet {
                assert!(!enabled, "{label} is planned but enabled");
            }
        }
        for wanted in [
            "Weld",
            "Paste in Front",
            "Make Compound Path",
            "Send…",
            "Inverse",
        ] {
            assert!(items.iter().any(|(l, ..)| l == wanted), "missing {wanted}");
        }
        // Items whose engine commands exist now dispatch them instead of the placeholder.
        for wired in [
            "Duplicate Left",
            "Fill Page",
            "Rotate Copies",
            "Lock",
            "Unlock All",
            "Hide",
            "Show All",
            "Convert to Path",
            "Make Compound Path",
            "Release Compound Path",
            "Simplify",
            "Join",
            "Break at Point",
            "Offset Path…",
            "Outline Stroke",
            "Internal Offset…",
            "Subtract All",
            "Fill Color",
            "Stroke Color",
            "Replicate",
            "Offset",
        ] {
            let found: Vec<_> = items.iter().filter(|(l, ..)| l == wired).collect();
            assert!(!found.is_empty(), "missing {wired}");
            assert!(
                found.iter().all(|(_, _, command)| *command != A::NotYet),
                "{wired} is still a placeholder"
            );
        }
        // Items still waiting for an engine command stay planned.
        for planned in ["Flip Horizontal", "Bring to Front", "Shear…", "Detach Lines"] {
            assert!(
                items
                    .iter()
                    .any(|(l, enabled, command)| l == planned && !enabled && *command == A::NotYet),
                "{planned} should stay planned"
            );
        }
    }
}
