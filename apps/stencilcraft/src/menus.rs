//! The menu bar: Silhouette Studio's menus and item order where the item exists there, with
//! Illustrator/VectorCraft items added after them and in the Type, Select, Effect and Machine
//! menus (`docs/menu-map.md`). Items with no engine command yet are listed disabled with a
//! "Not yet implemented" tooltip, so the structure is complete and users find where things
//! will be.

use std::cell::Cell;

use craft_ui::contract::{Props as _, Show as _, TooltipSpec};
use craft_ui::display::Divider;
use craft_ui::menu::{Menu, MenuBar, MenuTitle};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use stencilcraft_doc::CutStyle;
use stencilcraft_engine::document::{ArrangeOrder, PasteMode};
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

fn kbd(modifiers: Modifiers, key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(modifiers, key)
}

/// The mark an item shows in the menu's mark column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Check(bool),
    Radio(bool),
}

/// One item of the app's menu table: a command row, or a submenu of entries.
#[derive(Clone, Debug)]
pub struct MenuItem<T> {
    label: String,
    command: Option<T>,
    children: Option<Vec<MenuEntry<T>>>,
    enabled: bool,
    tooltip: Option<String>,
    shortcut: Option<KeyboardShortcut>,
    mark: Option<Mark>,
}

/// An entry of a menu: an item or a separator.
#[derive(Clone, Debug)]
pub enum MenuEntry<T> {
    Item(MenuItem<T>),
    Separator,
}

impl<T> From<MenuItem<T>> for MenuEntry<T> {
    fn from(item: MenuItem<T>) -> Self {
        MenuEntry::Item(item)
    }
}

impl<T> MenuEntry<T> {
    /// The item, unless this is a separator.
    #[cfg(test)]
    pub fn item(&self) -> Option<&MenuItem<T>> {
        match self {
            MenuEntry::Item(item) => Some(item),
            MenuEntry::Separator => None,
        }
    }
}

impl<T> MenuItem<T> {
    /// A row that runs `command`.
    pub fn new(label: &str, command: T) -> Self {
        MenuItem {
            label: label.to_owned(),
            command: Some(command),
            children: None,
            enabled: true,
            tooltip: None,
            shortcut: None,
            mark: None,
        }
    }

    /// A submenu of `children`.
    pub fn submenu(label: &str, children: Vec<MenuEntry<T>>) -> Self {
        MenuItem {
            label: label.to_owned(),
            command: None,
            children: Some(children),
            enabled: true,
            tooltip: None,
            shortcut: None,
            mark: None,
        }
    }

    #[must_use]
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    #[must_use]
    pub fn tooltip(mut self, text: &str) -> Self {
        self.tooltip = Some(text.to_owned());
        self
    }

    #[must_use]
    pub fn shortcut(mut self, shortcut: KeyboardShortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// A check mark, shown while `on`.
    #[must_use]
    pub fn check(mut self, on: bool) -> Self {
        self.mark = Some(Mark::Check(on));
        self
    }

    /// A radio mark, shown while `on`.
    #[must_use]
    pub fn radio(mut self, on: bool) -> Self {
        self.mark = Some(Mark::Radio(on));
        self
    }

    #[cfg(test)]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The label as the pin's menu takes it: a `&` there marks the mnemonic and `&&` is a
    /// literal ampersand (`craft_keymap::mnemonic::marked`), so every `&` of the app's words is
    /// doubled ("Print & Cut" would show as "Print  Cut").
    pub fn shown_label(&self) -> String {
        self.label.replace('&', "&&")
    }

    #[cfg(test)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[cfg(test)]
    pub fn command(&self) -> Option<&T> {
        self.command.as_ref()
    }

    pub fn children(&self) -> Option<&[MenuEntry<T>]> {
        self.children.as_deref()
    }
}

/// Shows the menu bar of `menus` (one title per top-level item) and returns the command picked this pass.
pub fn show_bar<T: Copy>(ui: &mut egui::Ui, menus: &[MenuItem<T>]) -> Option<T> {
    let chosen = Cell::new(None);
    let titles = menus
        .iter()
        .map(|menu| {
            let chosen = &chosen;
            MenuTitle::new(menu.shown_label(), move |ui: &mut egui::Ui| {
                if let Some(command) = show_rows(ui, menu.children().unwrap_or_default()) {
                    chosen.set(Some(command));
                }
            })
        })
        .collect();
    let _ = MenuBar::new("StencilCraft", titles).show(ui);
    chosen.get()
}

/// Shows `entries` as menu rows, a submenu after its row, and returns the command of the row that ran.
fn show_rows<T: Copy>(ui: &mut egui::Ui, entries: &[MenuEntry<T>]) -> Option<T> {
    let mut picked = None;
    for entry in entries {
        let MenuEntry::Item(item) = entry else {
            Divider::horizontal().show(ui);
            continue;
        };
        if let Some(children) = &item.children {
            let mut inner = None;
            Menu::submenu_row_enabled(ui, item.shown_label(), item.enabled, |ui, _| {
                inner = show_rows(ui, children);
            });
            picked = picked.or(inner);
            continue;
        }
        let mut row = craft_ui::menu::MenuItem::new(item.shown_label()).enabled(item.enabled);
        if let Some(shortcut) = &item.shortcut {
            row = row.shortcut(ui.ctx().format_shortcut(shortcut));
        }
        match item.mark {
            Some(Mark::Check(on)) => row = row.check(on),
            Some(Mark::Radio(on)) => row = row.radio_mark(on),
            None => {}
        }
        if let Some(tip) = &item.tooltip {
            row = row.tooltip(TooltipSpec::new(tip.clone()).disabled_text(tip.clone()));
        }
        if row.show(ui).ran {
            picked = item.command;
        }
    }
    picked
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
        MenuItem::submenu("Edit", edit_menu(app)),
        MenuItem::submenu("Object", object(app)),
        MenuItem::submenu("Type", type_menu(app)),
        MenuItem::submenu("Select", select(app)),
        MenuItem::submenu("Effect", effect(app)),
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
        item("New…", A::NewDoc).shortcut(kbd(CMD, Key::N)).into(),
        item("New from Template…", A::NewFromTemplate)
            .shortcut(kbd(cmd_shift(), Key::N))
            .into(),
        item("Open…", A::OpenDoc).shortcut(kbd(CMD, Key::O)).into(),
        sub("Open Recent", recent),
        item("Open Recovered…", A::ShowPanel(P::Library))
            .enabled(!app.stores.recovered.is_empty())
            .into(),
        item("Merge…", A::ImportSvg)
            .shortcut(kbd(cmd_shift(), Key::O))
            .into(),
        sub(
            "Import",
            vec![
                item("SVG…", A::ImportSvg).into(),
                item("Image…", A::ImportImage).into(),
                item("DXF…", A::ImportVector(crate::file_menu::ImportKind::Dxf)).into(),
                item("PDF…", A::ImportVector(crate::file_menu::ImportKind::Pdf)).into(),
                item(
                    "AI / EPS…",
                    A::ImportVector(crate::file_menu::ImportKind::AiEps),
                )
                .into(),
            ],
        ),
        SEP,
        item("Save", A::SaveDoc).shortcut(kbd(CMD, Key::S)).into(),
        item("Save As…", A::SaveAsDoc)
            .shortcut(kbd(cmd_shift(), Key::S))
            .into(),
        item("Save Selection As…", A::SaveSelectionAs)
            .enabled(!app.selected_shapes.is_empty())
            .into(),
        sub(
            "Export",
            vec![
                item("SVG…", A::ExportSvg).into(),
                item("Print & Cut SVG…", A::ExportPrintCutSvg).into(),
                item("DXF (R14 ASCII)…", A::ExportDxf).into(),
                item("G-code (Grbl)…", A::ExportGcode).into(),
                item("Tiled SVG Sheets…", A::ExportTiledSvg).into(),
                SEP,
                item("PDF…", A::Export(crate::file_menu::ExportKind::Pdf)).into(),
                item("PNG…", A::Export(crate::file_menu::ExportKind::Png)).into(),
                item("JPEG…", A::Export(crate::file_menu::ExportKind::Jpeg)).into(),
            ],
        ),
        SEP,
        item("Library…", A::ShowPanel(P::Library)).into(),
        item("Scan…", A::ShowPanel(P::PixScan)).into(),
        SEP,
        item("Print…", A::Print).shortcut(kbd(CMD, Key::P)).into(),
        item("Print Page Setup…", A::ShowPanel(P::PageSetup)).into(),
        item("Send to Machine…", A::ShowSend)
            .shortcut(kbd(CMD, Key::L))
            .into(),
        item("Document Setup…", A::ShowPanel(P::PageSetup)).into(),
        SEP,
        item("Close Tab", A::CloseDoc)
            .shortcut(kbd(CMD, Key::W))
            .into(),
        item("Quit", A::Quit).shortcut(kbd(CMD, Key::Q)).into(),
    ]
}

fn edit_menu(app: &StencilApp) -> Vec<MenuEntry<A>> {
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
        item("Cut", A::Cut)
            .shortcut(kbd(CMD, Key::X))
            .enabled(any)
            .into(),
        item("Copy", A::Copy)
            .shortcut(kbd(CMD, Key::C))
            .enabled(any)
            .into(),
        item("Paste", A::Paste(PasteMode::Offset))
            .shortcut(kbd(CMD, Key::V))
            .into(),
        item("Paste in Front", A::Paste(PasteMode::InFront))
            .shortcut(kbd(CMD, Key::F))
            .into(),
        item("Paste in Back", A::Paste(PasteMode::InBack))
            .shortcut(kbd(CMD, Key::B))
            .into(),
        item("Paste in Place", A::Paste(PasteMode::InPlace))
            .shortcut(kbd(cmd_shift(), Key::V))
            .into(),
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
        item("Select by Color…", A::ShowPanel(P::SelectByColor)).into(),
        SEP,
        item("History…", A::ShowPanel(P::History)).into(),
        item("Preferences…", A::ShowPanel(P::Preferences))
            .shortcut(kbd(CMD, Key::K))
            .into(),
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
            vec![
                when(
                    "Flip Horizontal",
                    A::Flip(DistributionAxis::Horizontal),
                    any,
                ),
                when("Flip Vertical", A::Flip(DistributionAxis::Vertical), any),
            ],
        ),
        sub(
            "Arrange",
            vec![
                item("Bring to Front", A::Arrange(ArrangeOrder::Front))
                    .shortcut(kbd(cmd_shift(), Key::CloseBracket))
                    .enabled(any)
                    .into(),
                item("Bring Forward", A::Arrange(ArrangeOrder::Forward))
                    .shortcut(kbd(CMD, Key::CloseBracket))
                    .enabled(any)
                    .into(),
                item("Send Backward", A::Arrange(ArrangeOrder::Backward))
                    .shortcut(kbd(CMD, Key::OpenBracket))
                    .enabled(any)
                    .into(),
                item("Send to Back", A::Arrange(ArrangeOrder::Back))
                    .shortcut(kbd(cmd_shift(), Key::OpenBracket))
                    .enabled(any)
                    .into(),
                SEP,
                sub(
                    "Send to Layer",
                    app.engine
                        .doc
                        .layers
                        .iter()
                        .enumerate()
                        .map(|(i, layer)| when(&layer.name, A::SendToLayer(i), any))
                        .collect(),
                ),
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
                // The key is the last shape selected (shift-click it last).
                sub(
                    "To Key Object",
                    [
                        ("Left", Alignment::Left),
                        ("Center", Alignment::HorizontalCenter),
                        ("Right", Alignment::Right),
                        ("Top", Alignment::Top),
                        ("Middle", Alignment::VerticalCenter),
                        ("Bottom", Alignment::Bottom),
                    ]
                    .into_iter()
                    .map(|(label, a)| when(label, A::AlignToKey(a), many))
                    .collect(),
                ),
                SEP,
                // Even spacing needs three or more shapes (the engine refuses fewer).
                when(
                    "Space Horizontally",
                    A::Distribute(DistributionAxis::Horizontal),
                    app.selected_shapes.len() >= 3,
                ),
                when(
                    "Space Vertically",
                    A::Distribute(DistributionAxis::Vertical),
                    app.selected_shapes.len() >= 3,
                ),
                when(
                    "Distribute Spacing by Value…",
                    A::ShowPanel(P::Transform),
                    many,
                ),
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
                item("Custom…", A::ShowPanel(P::Transform)).into(),
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
                edit(
                    "Column of Copies",
                    E::Array(DistributionAxis::Vertical),
                    any,
                ),
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
                item("Shear…", A::ShowPanel(P::Transform)).into(),
                SEP,
                item("Transform Again", A::TransformAgain)
                    .shortcut(kbd(cmd_shift(), Key::D))
                    .enabled(any)
                    .into(),
                item("Transform Each…", A::ShowPanel(P::Transform)).into(),
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
        item("Group", A::Group)
            .shortcut(kbd(CMD, Key::G))
            .enabled(many)
            .into(),
        item("Ungroup", A::Ungroup)
            .shortcut(kbd(cmd_shift(), Key::G))
            .enabled(any)
            .into(),
        when(
            if app.selection_attached() {
                "Detach"
            } else {
                "Attach"
            },
            A::ToggleAttach,
            if app.selection_attached() { any } else { many },
        ),
        edit_key("Lock", E::Lock, any, CMD, Key::Num2),
        edit_key("Unlock All", E::UnlockAll, true, cmd_alt(), Key::Num2),
        edit_key("Hide", E::Hide, any, CMD, Key::Num3),
        edit_key("Show All", E::ShowAll, true, cmd_alt(), Key::Num3),
        SEP,
        edit("Convert to Path", E::ToPath, any),
        edit_key("Make Compound Path", E::MakeCompound, many, CMD, Key::E),
        edit_key(
            "Release Compound Path",
            E::ReleaseCompound,
            any,
            cmd_shift(),
            Key::E,
        ),
        sub(
            "Clipping Mask",
            vec![
                item("Make", A::ClipMake)
                    .shortcut(kbd(CMD, Key::Num7))
                    .enabled(app.selected_shapes.len() == 2)
                    .into(),
                item("Release", A::ClipRelease)
                    .shortcut(kbd(cmd_alt(), Key::Num7))
                    .enabled(any)
                    .into(),
            ],
        ),
        edit_key("Simplify", E::Simplify, any, CMD, Key::J),
        sub(
            "Path",
            vec![
                edit_key("Join", E::Join, any, cmd_shift(), Key::J),
                edit_key(
                    "Average",
                    E::Average(NodeAxis::Both),
                    nodes,
                    cmd_alt(),
                    Key::J,
                ),
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
        edit("Expand", E::ToPath, any),
        item("Contour…", A::ShowPanel(P::Contour)).into(),
        when("Flatten", A::Flatten, any),
    ]
}

fn type_menu(app: &StencilApp) -> Vec<MenuEntry<A>> {
    let text = app.selected_text_ids().next().is_some();
    vec![
        sub(
            "Font",
            vec![item("System Fonts…", A::ShowPanel(P::TextStyle)).into()],
        ),
        sub(
            "Size",
            vec![item("Custom…", A::ShowPanel(P::TextStyle)).into()],
        ),
        item("Character", A::ShowPanel(P::TextStyle))
            .shortcut(kbd(CMD, Key::T))
            .into(),
        item("Paragraph", A::ShowPanel(P::TextStyle))
            .shortcut(kbd(cmd_alt(), Key::T))
            .into(),
        item("Glyphs", A::ShowPanel(P::Glyphs)).into(),
        SEP,
        item("Place Lettering…", A::PlaceText)
            .shortcut(kbd(NONE, Key::T))
            .into(),
        item("Edit Selected Text…", A::EditText).into(),
        when(
            "Type on a Path",
            A::TypeOnPath,
            text && app.selected_shapes.len() >= 2,
        ),
        when("Curve Text…", A::CurveText, text),
        when("Create Outlines", A::TextToOutlines, text),
        when("Ungroup to Letters", A::TextUngroupLetters, text),
        SEP,
        item("Find Font…", A::ShowPanel(P::TextStyle)).into(),
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
        item("Select by Color…", A::ShowPanel(P::SelectByColor)).into(),
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
        when(
            "Next Object Above",
            A::SelectNextObject { above: true },
            any,
        ),
        when(
            "Next Object Below",
            A::SelectNextObject { above: false },
            any,
        ),
        SEP,
        item("Save Selection…", A::ShowPanel(P::Selections)).into(),
        item("Edit Selection…", A::ShowPanel(P::Selections)).into(),
    ]
}

/// An Effect menu item opening the Effects panel.
fn eff(label: &str, menu: crate::effects::EffectMenu) -> MenuEntry<A> {
    item(label, A::OpenEffect(menu)).into()
}

fn effect(app: &StencilApp) -> Vec<MenuEntry<A>> {
    use crate::effects::EffectMenu as M;
    let any = !app.selected_shapes.is_empty();
    vec![
        item("Offset…", A::Edit(E::Offset { internal: false })).into(),
        item("Internal Offset…", A::Edit(E::Offset { internal: true })).into(),
        item("Kerf Compensation…", A::ToggleKerfPreview).into(),
        eff("Bridges…", M::Bridges),
        eff("Weed Lines and Border…", M::WeedLines),
        SEP,
        item("Trace Image…", A::ShowPanel(P::Trace)).into(),
        eff("Remove Background…", M::RemoveBackground),
        item("Erase…", A::ShowPanel(P::EraseColour)).into(),
        eff("Image Effects…", M::ImageEffects),
        eff("Reduce Colours…", M::ReduceColours),
        eff("Despeckle…", M::Despeckle),
        eff("Smooth…", M::Smooth),
        SEP,
        eff("Sketch Fill…", M::SketchFill),
        eff("Stipple…", M::Stipple),
        eff("Emboss / Deboss Patterns…", M::EmbossPattern),
        eff("Rhinestone…", M::Rhinestone),
        sub(
            "Warp",
            vec![
                eff("Arc", M::WarpArc),
                eff("Flag", M::WarpFlag),
                eff("Bulge", M::WarpBulge),
                eff("Perspective…", M::WarpPerspective),
            ],
        ),
        sub(
            "Distort",
            vec![
                item("Shear…", A::ShowPanel(P::Transform)).into(),
                when(
                    "Free Distort",
                    A::BeginDistort {
                        rows: 1,
                        columns: 1,
                    },
                    any,
                ),
                when(
                    "Lattice Distort…",
                    A::BeginDistort {
                        rows: 3,
                        columns: 3,
                    },
                    any,
                ),
            ],
        ),
        eff("Lattice…", M::Lattice),
        eff("Pop-up Card…", M::PopupCard),
        eff("Puzzle…", M::Puzzle),
        SEP,
        item("Apply Last Effect", A::ApplyLastEffect).into(),
    ]
}

/// View › Zoom presets (Silhouette Studio's list).
const ZOOM_PRESETS: [u16; 8] = [25, 33, 50, 75, 100, 150, 200, 400];

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
                item("Zoom to Selection", A::ZoomSelection)
                    .shortcut(kbd(cmd_alt(), Key::Num0))
                    .enabled(!app.selected_shapes.is_empty())
                    .into(),
                SEP,
            ]
            .into_iter()
            .chain(ZOOM_PRESETS.iter().map(|&percent| {
                let current = (app.view.zoom * 100.0).round() as i64 == i64::from(percent);
                item(&format!("{percent}%"), A::ZoomPercent(percent))
                    .radio(current)
                    .into()
            }))
            .collect(),
        ),
        sub(
            "Rotate Page",
            (0..4u8)
                .map(|turns| {
                    item(&format!("{}°", u16::from(turns) * 90), A::RotateView(turns))
                        .radio(app.view.quarter_turns % 4 == turns)
                        .into()
                })
                .collect(),
        ),
        SEP,
        item("Grid Options…", A::ShowPanel(P::PageSetup)).into(),
        item("Show Grid", A::ToggleGrid)
            .shortcut(kbd(CMD, Key::Quote))
            .check(app.prefs.show_grid)
            .into(),
        item("Snap to Grid", A::ToggleSnapGrid)
            .shortcut(kbd(cmd_shift(), Key::Quote))
            .check(app.engine.doc.snap.grid)
            .into(),
        item("Smart Snapping", A::ToggleSmartSnapping)
            .check(app.engine.doc.snap.objects)
            .into(),
        item("Smart Guides", A::ToggleSmartGuides)
            .shortcut(kbd(CMD, Key::U))
            .check(app.prefs.smart_guides)
            .into(),
        item("Snap to Point", A::ToggleSnapPoint)
            .shortcut(kbd(cmd_alt(), Key::Quote))
            .check(app.engine.doc.snap.points)
            .into(),
        item("Rulers", A::ToggleRulers)
            .shortcut(kbd(cmd_shift(), Key::R))
            .check(app.prefs.show_rulers)
            .into(),
        sub(
            "Guides",
            vec![
                item("Show Guides", A::ToggleShowGuides)
                    .shortcut(kbd(CMD, Key::Semicolon))
                    .check(app.prefs.show_guides)
                    .into(),
                item("Lock Guides", A::ToggleLockGuides)
                    .shortcut(kbd(cmd_alt(), Key::Semicolon))
                    .check(app.guides_locked())
                    .enabled(!app.engine.doc.guides.is_empty())
                    .into(),
                item("Make Guides", A::MakeGuides)
                    .shortcut(kbd(CMD, Key::Num5))
                    .enabled(!app.selected_shapes.is_empty())
                    .into(),
                item("Release Guides", A::ReleaseGuides)
                    .shortcut(kbd(cmd_alt(), Key::Num5))
                    .enabled(app.engine.doc.guides.iter().any(|g| !g.locked))
                    .into(),
                item("Clear Guides", A::ClearGuides)
                    .enabled(app.engine.doc.guides.iter().any(|g| !g.locked))
                    .into(),
            ],
        ),
        SEP,
        item("Outline View", A::ToggleOutlineView)
            .check(app.prefs.outline_view)
            .into(),
        item("Show Print Border", A::TogglePrintBorder)
            .shortcut(kbd(cmd_shift(), Key::P))
            .check(app.prefs.show_print_border)
            .into(),
        item("Show Cut Border", A::ToggleCutBorder)
            .shortcut(kbd(cmd_shift(), Key::L))
            .check(app.prefs.show_cut_border)
            .into(),
        item("Show Dimensions", A::ToggleDimensions)
            .check(app.prefs.show_dimensions)
            .into(),
        item("Preview Kerf Compensation", A::ToggleKerfPreview)
            .check(app.kerf_preview)
            .into(),
        item("Registration Marks", A::TogglePrintCutMarks)
            .check(app.prefs.print_cut_marks)
            .into(),
        item("Design Page Settings…", A::ShowPanel(P::PageSetup)).into(),
        SEP,
        item("Show Design", A::ShowDesign)
            .radio(!app.send_mode)
            .into(),
        item("Show Send", A::ShowSend).radio(app.send_mode).into(),
    ]
}

fn panels(app: &StencilApp) -> Vec<MenuEntry<A>> {
    use crate::effects::EffectMenu as M;
    use stencilcraft_ui_egui::ColorTheme;
    vec![
        item("Page Setup", A::ShowPanel(P::PageSetup)).into(),
        item("PixScan", A::ShowPanel(P::PixScan)).into(),
        item("Fill Color", A::ShowPanel(P::FillColor)).into(),
        item("Line Style", A::ShowPanel(P::LineStyle)).into(),
        item("Select by Color", A::ShowPanel(P::SelectByColor)).into(),
        item("Trace", A::ShowPanel(P::Trace)).into(),
        eff("Image Effects", M::ImageEffects),
        item("Text Style", A::ShowPanel(P::TextStyle)).into(),
        todo("Spelling"),
        item("Transform", A::ShowPanel(P::Transform)).into(),
        item("Replicate", A::ShowPanel(P::Replicate)).into(),
        item("Modify", A::ShowPanel(P::Modify)).into(),
        item("Offset", A::ShowPanel(P::Offset)).into(),
        eff("Pop up", M::PopupCard),
        eff("Sketch", M::SketchFill),
        eff("Specialty", M::Rhinestone),
        item("Print & Cut", A::ShowPanel(P::PrintCut)).into(),
        item("Point Editing", A::ShowPanel(P::Points)).into(),
        item("Layers", A::ShowPanel(crate::WorkspacePanel::Layers)).into(),
        SEP,
        item("Properties", A::ShowPanel(crate::WorkspacePanel::Inspector)).into(),
        item("Tools", A::ShowPanel(crate::WorkspacePanel::Tools)).into(),
        item("Swatches", A::ShowPanel(P::FillColor)).into(),
        item("Recolor Artwork", A::ShowPanel(P::SelectByColor)).into(),
        item("Graphic Styles", A::ShowPanel(P::GraphicStyles)).into(),
        item("Actions", A::ShowPanel(P::Actions)).into(),
        item("Libraries", A::ShowPanel(P::Library)).into(),
        SEP,
        item("Reset Panels to Default", A::ResetWorkspace).into(),
        item("Starter Mode", A::ToggleStarterMode)
            .check(app.prefs.starter_mode)
            .into(),
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

/// A Machine › Send Mode radio item.
fn send_mode(app: &StencilApp, label: &str, mode: crate::send_modes::SendMode) -> MenuEntry<A> {
    item(label, A::SetSendMode(mode))
        .radio(app.send.mode == mode)
        .into()
}

fn machine(app: &StencilApp) -> Vec<MenuEntry<A>> {
    vec![
        item("Send…", A::ShowSend).shortcut(kbd(CMD, Key::L)).into(),
        item(
            "Connect / Rescan Devices",
            A::Machine(crate::machine_menu::MachineAction::Rescan),
        )
        .into(),
        sub(
            "Send Mode",
            vec![
                send_mode(app, "Standard", crate::send_modes::SendMode::Standard),
                send_mode(app, "By Line Color", crate::send_modes::SendMode::LineColor),
                send_mode(app, "By Fill Color", crate::send_modes::SendMode::FillColor),
                send_mode(app, "By Layer", crate::send_modes::SendMode::Layer),
            ],
        ),
        item("Tool Holders…", A::ShowSend).into(),
        item("Material Settings…", A::ShowSend).into(),
        item("Mat Preview", A::ToggleMatPreview)
            .check(app.prefs.mat_preview)
            .into(),
        item("Mirror on Mat", A::ToggleMirror)
            .check(app.send.mirror)
            .into(),
        sub(
            "Load Type",
            vec![
                item("On Mat", A::LoadOnMat(true))
                    .radio(app.mat != stencilcraft_machine::Mat::None)
                    .into(),
                item("Without Mat / Roll", A::LoadOnMat(false))
                    .radio(app.mat == stencilcraft_machine::Mat::None)
                    .into(),
            ],
        ),
        SEP,
        item(
            "Test Cut",
            A::Machine(crate::machine_menu::MachineAction::TestCut),
        )
        .into(),
        when("Dry Run", A::DryRun, app.engine.doc.shape_count() > 0),
        item("Cut", A::Machine(crate::machine_menu::MachineAction::Cut))
            .enabled(app.engine.doc.shape_count() > 0)
            .into(),
        todo("Pause"),
        when("Cancel", A::CancelSend, app.send.busy),
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
        item("Tutorials", A::HelpManual).into(),
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

    /// Every label of `entries`, submenus and their rows, depth first.
    fn all_items<'a>(entries: &'a [MenuEntry<A>], out: &mut Vec<&'a MenuItem<A>>) {
        for entry in entries {
            if let Some(item) = entry.item() {
                out.push(item);
                if let Some(children) = item.children() {
                    all_items(children, out);
                }
            }
        }
    }

    #[test]
    fn menu_labels_show_their_ampersands() {
        // Regression: the pin's menu read the `&` of "Print & Cut" as a mnemonic marker and
        // showed "Print  Cut" (Panels, File › Export).
        let app = StencilApp::new(None);
        let menus = build(&app);
        let mut items: Vec<&MenuItem<A>> = menus.iter().collect();
        for menu in &menus {
            all_items(menu.children().unwrap_or_default(), &mut items);
        }
        let folder = craft_i18n::Folder::new("en");
        let mut ampersands = 0;
        for item in items {
            let (shown, _) = craft_keymap::mnemonic::marked(&item.shown_label(), &folder);
            assert_eq!(shown, item.label(), "the menu shows {:?}", item.label());
            ampersands += usize::from(item.label().contains('&'));
        }
        assert!(
            ampersands >= 2,
            "Print & Cut is in Panels and File › Export"
        );
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
            "Flip Horizontal",
            "Flip Vertical",
            "Shear…",
            "Transform Again",
            "Transform Each…",
            "Distribute Spacing by Value…",
            "Fill Color",
            "Stroke Color",
            "Replicate",
            "Offset",
            "Zoom to Selection",
            "200%",
            "90°",
            "Snap to Grid",
            "Smart Snapping",
            "Smart Guides",
            "Snap to Point",
            "Show Guides",
            "Lock Guides",
            "Make Guides",
            "Release Guides",
            "Clear Guides",
            "Paste in Back",
            "Create Outlines",
            "Ungroup to Letters",
            "Next Object Above",
            "Next Object Below",
        ] {
            let found: Vec<_> = items.iter().filter(|(l, ..)| l == wired).collect();
            assert!(!found.is_empty(), "missing {wired}");
            assert!(
                found.iter().any(|(_, _, command)| *command != A::NotYet),
                "{wired} is still a placeholder"
            );
        }
        // Items still waiting for an engine command stay planned.
        for planned in ["Detach Lines", "Spelling…"] {
            assert!(
                items
                    .iter()
                    .any(|(l, enabled, command)| l == planned && !enabled && *command == A::NotYet),
                "{planned} should stay planned"
            );
        }
    }
}
