//! Edit › History, Edit › Preferences and Select › Save/Edit Selection panels.

use craft_ui::controls::{Checkbox, Segmented, TextField};
use craft_ui::edit::EditEvent;
use craft_ui::patterns::Panel;
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
        let _ = Panel::new("stencilcraft-history-panel").show(ui, |ui| {
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
        let mut prefs = self.prefs.clone();
        let mut theme = self.theme;
        let _ = Panel::new("stencilcraft-preferences-panel").show(ui, |ui| {
            ui.label("Appearance");
            let _ = Segmented::new("pref-theme", "Theme", &mut theme)
                .item(ColorTheme::Dark, "Dark")
                .item(ColorTheme::Light, "Light")
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
                let _ = Checkbox::bool(label, value).show(ui);
            }
            ui.separator();
            ui.label("Units");
            let mut unit = prefs.length_unit;
            let mut units = Segmented::new("pref-units", "Units", &mut unit);
            for u in stencilcraft_ui_egui::LengthUnit::ALL {
                units = units.item(u, u.symbol());
            }
            let _ = units.show(ui);
            prefs.length_unit = unit;
            ui.separator();
            ui.label("Tools");
            let _ = Checkbox::bool(
                "Return to the Select tool after drawing a shape",
                &mut prefs.select_after_drawing,
            )
            .show(ui);
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
    }

    pub fn selections_panel(&mut self, ui: &mut egui::Ui) {
        let any = !self.selected_shapes.is_empty();
        let mut name = self.selection_name_draft.clone();
        let mut save = false;
        let mut pick = None;
        let mut remove = None;
        let saved = self.saved_selections.clone();
        let _ = Panel::new("stencilcraft-selections-panel").show(ui, |ui| {
            let edit = TextField::new("selection-name", "Selection name", name.as_str())
                .char_limit(80)
                .show(ui);
            if let Some(
                EditEvent::Update { value, .. }
                | EditEvent::Commit { value, .. }
                | EditEvent::Set { value, .. },
            ) = edit.event
            {
                name = value;
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
        assert!(app.workspace.contains(&WorkspacePanel::TextStyle));
        assert!(!app.workspace.contains(&WorkspacePanel::Glyphs));
        app.handle_menu_action(TopMenuAction::ToggleStarterMode);
        assert!(app.workspace.contains(&WorkspacePanel::Glyphs));
    }
}
