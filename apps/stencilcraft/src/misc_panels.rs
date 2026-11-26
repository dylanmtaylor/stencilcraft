//! Edit › History, Edit › Preferences and Select › Save/Edit Selection panels.

use craft_ui::choice::{Checkbox, Segmented};
use craft_ui::contract::{Bind, Entry, Props as _, Show as _, ShowContainer as _};
use craft_ui::sections::Panel;
use craft_ui::text::TextField;
use eframe::egui;
use stencilcraft_ui_egui::ColorTheme;

use crate::panels::{button, hint};
use crate::{StencilApp, TopMenuAction};

/// A command's JSON name as a History line: `shape.add-rect` → "Add rect".
pub fn history_title(label: &str) -> String {
    let (group, action) = label.rsplit_once('.').unwrap_or(("", label));
    let name = match action {
        "affine" | "transform" => "transform".to_string(),
        "set-bounds" => "resize".to_string(),
        // A one-word action reads better with what it acts on: "Set kerf", "New document".
        _ if !action.contains('-') && !group.is_empty() => format!("{action} {group}"),
        _ => action.replace('-', " "),
    };
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "Edit".to_string(),
    }
}

/// A named selection (Select › Save Selection).
#[derive(Clone, Debug, PartialEq)]
pub struct SavedSelection {
    pub name: String,
    pub ids: Vec<u64>,
}

/// A row of the colour panels' swatches bound to `value`; with `none`, a first "Default"
/// choice for no colour.
fn swatch_row(ui: &mut egui::Ui, id: &str, value: &mut Option<String>, none: bool) {
    use craft_ui::choice::Swatch;
    ui.horizontal_wrapped(|ui| {
        if none && crate::panels::button(ui, "Default", value.is_some()) {
            *value = None;
        }
        for (i, colour) in crate::colour_panels::PALETTE.iter().enumerate() {
            let Some(c) = stencilcraft_ui_egui::parse_hex_colour(colour) else {
                continue;
            };
            if Swatch::new(*colour, craft_theme::Srgba::from(c))
                .id_salt(format!("{id}-{i}"))
                .size(craft_theme::ControlSize::Sm)
                .selected(value.as_deref() == Some(*colour))
                .show(ui)
                .clicked
            {
                *value = Some((*colour).to_string());
            }
        }
    });
}

impl StencilApp {
    /// Step undo/redo until `done` commands are applied.
    pub fn history_go_to(&mut self, done: usize) {
        let mut guard = 0;
        while self.engine.undo_labels().len() > done && guard < 1000 {
            if !self.engine.undo() {
                break;
            }
            guard += 1;
        }
        while self.engine.undo_labels().len() < done && guard < 1000 {
            if !self.engine.redo() {
                break;
            }
            guard += 1;
        }
        self.retain_valid_selection();
    }

    pub fn history_panel(&mut self, ui: &mut egui::Ui) {
        let undo = self.engine.undo_labels();
        let redo = self.engine.redo_labels();
        let mut go_to = None;
        let _ = Panel::new("History")
            .id_salt("stencilcraft-history-panel")
            .show(ui, |ui, _| {
                if ui.selectable_label(undo.is_empty(), "Start").clicked() {
                    go_to = Some(0);
                }
                for (i, label) in undo.iter().enumerate() {
                    let current = i + 1 == undo.len();
                    if ui.selectable_label(current, history_title(label)).clicked() {
                        go_to = Some(i + 1);
                    }
                }
                for (i, label) in redo.iter().enumerate() {
                    let text = egui::RichText::new(history_title(label)).weak();
                    if ui.selectable_label(false, text).clicked() {
                        go_to = Some(undo.len() + i + 1);
                    }
                }
                hint(ui, "Click a step to go back (or forward) to it.");
            });
        if let Some(done) = go_to {
            self.history_go_to(done);
        }
    }

    pub fn preferences_panel(&mut self, ui: &mut egui::Ui) {
        let numbers = self.locale_numbers.clone();
        let mut restore = false;
        let mut prefs = self.prefs.clone();
        let mut theme = self.theme;
        let theme_entries = [
            Entry::new(ColorTheme::Dark, "Dark"),
            Entry::new(ColorTheme::Light, "Light"),
        ];
        let unit_entries: Vec<Entry<stencilcraft_ui_egui::LengthUnit>> =
            stencilcraft_ui_egui::LengthUnit::ALL
                .into_iter()
                .map(|u| Entry::new(u, u.symbol()))
                .collect();
        let _ = Panel::new("Preferences")
            .id_salt("stencilcraft-preferences-panel")
            .show(ui, |ui, _| {
                ui.label("Appearance");
                let _ = Segmented::new("Theme", Bind::bound(&mut theme), &theme_entries)
                    .id_salt("pref-theme")
                    .show(ui);
                ui.separator();
                ui.label("Canvas");
                for (label, value) in [
                    ("Show rulers", &mut prefs.show_rulers),
                    ("Show grid", &mut prefs.show_grid),
                    ("Show guides", &mut prefs.show_guides),
                    ("Smart guides", &mut prefs.smart_guides),
                    ("Show dimensions", &mut prefs.show_dimensions),
                    ("Show cut border", &mut prefs.show_cut_border),
                    ("Show print border", &mut prefs.show_print_border),
                    ("Mat preview", &mut prefs.mat_preview),
                    ("Outline view", &mut prefs.outline_view),
                ] {
                    let _ = Checkbox::new(Bind::bound(value)).label(label).show(ui);
                }
                ui.separator();
                ui.label("Units");
                let _ = Segmented::new("Units", Bind::bound(&mut prefs.length_unit), &unit_entries)
                    .id_salt("pref-units")
                    .show(ui);
                ui.separator();
                ui.label("Defaults: new shapes");
                let mut solid = prefs.default_fill.is_some();
                let _ = Segmented::new(
                    "Default fill style",
                    Bind::bound(&mut solid),
                    &[
                        Entry::new(false, "Outline only"),
                        Entry::new(true, "Solid fill"),
                    ],
                )
                .id_salt("pref-default-fill")
                .show(ui);
                if solid != prefs.default_fill.is_some() {
                    prefs.default_fill = solid.then(|| "#000000".to_string());
                }
                if solid {
                    hint(ui, "Fill");
                    swatch_row(ui, "pref-fill", &mut prefs.default_fill, false);
                }
                hint(ui, "Line");
                swatch_row(ui, "pref-line", &mut prefs.default_line, true);
                let _ = Checkbox::new(Bind::bound(&mut prefs.cut_to_page_edge))
                    .label("Cut to edge of page")
                    .show(ui);
                ui.separator();
                ui.label("Print");
                let _ = Checkbox::new(Bind::bound(&mut prefs.show_print_preview))
                    .label("Show print preview")
                    .show(ui);
                hint(ui, "Print resolution");
                let mut preset = [300.0, 600.0, 1200.0]
                    .into_iter()
                    .find(|d| (prefs.print_dpi - d).abs() < 1e-9)
                    .map_or(0, |d| d as u32);
                let _ = Segmented::new(
                    "Print resolution",
                    Bind::bound(&mut preset),
                    &[
                        Entry::new(300, "300 dpi"),
                        Entry::new(600, "600 dpi"),
                        Entry::new(1200, "1200 dpi"),
                        Entry::new(0, "Custom"),
                    ],
                )
                .id_salt("pref-print-dpi")
                .show(ui);
                if preset != 0 {
                    prefs.print_dpi = f64::from(preset);
                } else {
                    crate::panels::number(
                        ui,
                        "pref-print-dpi-custom",
                        "Custom print resolution",
                        &mut prefs.print_dpi,
                        72.0..=2400.0,
                        "{n} dpi",
                        &numbers,
                    );
                }
                ui.separator();
                ui.label("Import");
                hint(ui, "DXF files open:");
                let _ = Segmented::new(
                    "DXF: Open",
                    Bind::bound(&mut prefs.dxf_fit_to_page),
                    &[Entry::new(true, "Fit to Page"), Entry::new(false, "As-is")],
                )
                .id_salt("pref-dxf-open")
                .show(ui);
                let _ = Checkbox::new(Bind::bound(&mut prefs.import_join_lines))
                    .label("Join lines into polylines")
                    .show(ui);
                let _ = Checkbox::new(Bind::bound(&mut prefs.png_autotrace))
                    .label("PNG: Autotrace (cut round transparency)")
                    .show(ui);
                ui.separator();
                ui.label("Tools: action after tool use");
                for (label, id, value, keep) in [
                    (
                        "After creating a shape",
                        "pref-after-shape",
                        &mut prefs.select_after_drawing,
                        "Continue drawing",
                    ),
                    (
                        "After drawing freehand",
                        "pref-after-freehand",
                        &mut prefs.select_after_freehand,
                        "Continue drawing",
                    ),
                    (
                        "After using the Eraser",
                        "pref-after-eraser",
                        &mut prefs.select_after_eraser,
                        "Continue erasing",
                    ),
                    (
                        "After using the Knife",
                        "pref-after-knife",
                        &mut prefs.select_after_knife,
                        "Continue cutting",
                    ),
                    (
                        "After using Zoom",
                        "pref-after-zoom",
                        &mut prefs.select_after_zoom,
                        "Continue zooming",
                    ),
                ] {
                    hint(ui, label);
                    let _ = Segmented::new(
                        label,
                        Bind::bound(value),
                        &[Entry::new(false, keep), Entry::new(true, "Choose Select")],
                    )
                    .id_salt(id)
                    .show(ui);
                }
                hint(ui, "When drag selecting");
                let _ = Segmented::new(
                    "When drag selecting",
                    Bind::bound(&mut prefs.drag_select_inside),
                    &[
                        Entry::new(false, "Shapes touching the box"),
                        Entry::new(true, "Shapes fully inside"),
                    ],
                )
                .id_salt("pref-drag-select")
                .show(ui);
                ui.separator();
                ui.label("Maintenance");
                if button(ui, "Restore Factory Defaults…", true) {
                    restore = true;
                }
                hint(
                    ui,
                    "Designs are always stored in millimetres; Units sets what you read and type. \
                 Designs are snapshotted every 30 s while unsaved.",
                );
            });
        if theme != self.theme {
            self.handle_menu_action(TopMenuAction::SetTheme(theme));
        }
        if prefs != self.prefs {
            self.prefs = prefs;
            self.save_prefs();
        }
        if restore {
            self.confirm_restore_defaults = true;
        }
    }

    /// The question before Restore Factory Defaults.
    pub fn restore_defaults_dialog(&mut self, ui: &mut egui::Ui) {
        use craft_ui::dialog::Dialog;
        use craft_ui::sections::{DialogRole, FooterButton};
        if !self.confirm_restore_defaults {
            return;
        }
        let mut open = true;
        let shown = Dialog::new("Restore factory defaults?")
            .id_salt("restore-factory-defaults")
            .opened(Bind::bound(&mut open))
            .buttons([
                FooterButton::new("Restore", DialogRole::Accept),
                FooterButton::new("Cancel", DialogRole::Cancel),
            ])
            .show(ui, |ui, _| {
                let _ = craft_ui::display::Text::new(
                    "Every preference returns to its default and the panels are reset. Your \
                     designs, recent files, presets, styles and actions are kept.",
                )
                .show(ui);
            });
        if shown.output.picked == Some(0) {
            self.confirm_restore_defaults = false;
            self.restore_factory_defaults();
        } else if !open || shown.output.closed.is_some() {
            self.confirm_restore_defaults = false;
        }
    }

    pub fn selections_panel(&mut self, ui: &mut egui::Ui) {
        let any = !self.selected_shapes.is_empty();
        let mut name = self.selection_name_draft.clone();
        let mut save = false;
        let mut pick = None;
        let mut remove = None;
        let saved = self.saved_selections.clone();
        let _ = Panel::new("Selections")
            .id_salt("stencilcraft-selections-panel")
            .show(ui, |ui, _| {
                let _ = TextField::new("Selection name", Bind::bound(&mut name))
                    .id_salt("selection-name")
                    .show(ui);
                // A saved selection's name is at most 80 characters.
                if name.chars().count() > 80 {
                    name = name.chars().take(80).collect();
                }
                save = button(ui, "Save Current Selection", any && !name.trim().is_empty());
                for (i, s) in saved.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} ({})", s.name, s.ids.len()));
                        if button(ui, "Select", true) {
                            pick = Some(i);
                        }
                        if button(ui, "Delete", true) {
                            remove = Some(i);
                        }
                    });
                }
                if saved.is_empty() {
                    hint(ui, "Save a selection to pick the same shapes again later.");
                }
            });
        self.selection_name_draft = name.clone();
        if save {
            let name = name.trim().to_string();
            self.saved_selections.retain(|s| s.name != name);
            self.saved_selections.push(SavedSelection {
                name,
                ids: self.selected_shapes.clone(),
            });
        }
        if let Some(i) = pick
            && let Some(s) = self.saved_selections.get(i)
        {
            let ids = s.ids.clone();
            self.set_selection(ids);
        }
        if let Some(i) = remove
            && i < self.saved_selections.len()
        {
            self.saved_selections.remove(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_engine::Command;

    #[test]
    fn history_titles_and_going_back() {
        assert_eq!(history_title("shape.add-rect"), "Add rect");
        assert_eq!(history_title("shapes.affine"), "Transform");
        assert_eq!(history_title(""), "Edit");
        assert_eq!(history_title("kerf.set"), "Set kerf");
        assert_eq!(history_title("document.new"), "New document");
        let mut app = StencilApp::new(None);
        for x in [0.0, 10.0, 20.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            });
        }
        app.history_go_to(1);
        assert_eq!(app.engine.doc.shapes().count(), 1);
        assert_eq!(app.engine.redo_labels().len(), 2);
        app.history_go_to(3);
        assert_eq!(app.engine.doc.shapes().count(), 3);
        app.history_go_to(0);
        assert_eq!(app.engine.doc.shapes().count(), 0);
    }
}

#[cfg(test)]
mod starter_tests {
    use crate::{StencilApp, TopMenuAction, WorkspacePanel};

    #[test]
    fn starter_mode_shows_the_everyday_panels() {
        let mut app = StencilApp::new(None);
        app.prefs.starter_mode = false;
        app.handle_menu_action(TopMenuAction::ToggleStarterMode);
        assert!(app.prefs.starter_mode);
        assert!(app.workspace.layout().contains(&WorkspacePanel::TextStyle));
        assert!(!app.workspace.layout().contains(&WorkspacePanel::Glyphs));
        app.handle_menu_action(TopMenuAction::ToggleStarterMode);
        assert!(app.workspace.layout().contains(&WorkspacePanel::Glyphs));
    }
}
