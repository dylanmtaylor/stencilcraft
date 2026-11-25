//! Silhouette Studio's Fill Color, Line Style and Select by Color panels. Colours are stored
//! on each shape (`production.fill_color` / `line_color`, `#rrggbb`) and drawn on the canvas;
//! they label the design (cut by colour, print) without changing what is cut.

use craft_ui::controls::{ControlSize, Swatch, TextField};
use craft_ui::edit::EditEvent;
use craft_ui::patterns::Panel;
use eframe::egui;
use stencilcraft_doc::CutStyle;
use stencilcraft_engine::{Command, ProductionCommand, ProductionField};

use crate::StencilApp;
use crate::panels::{button, hint};

/// The swatch palette, as in Silhouette's colour panels.
pub const PALETTE: &[&str] = &[
    "#000000", "#4d4d4d", "#808080", "#b3b3b3", "#ffffff", "#e6194b", "#ff0000", "#f58231",
    "#ffa500", "#ffe119", "#ffff00", "#bfef45", "#3cb44b", "#00a000", "#42d4f4", "#00ffff",
    "#4363d8", "#0000ff", "#911eb4", "#800080", "#f032e6", "#ff69b4", "#9a6324", "#800000",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourTarget {
    Fill,
    Line,
}

impl ColourTarget {
    fn field(self) -> ProductionField {
        match self {
            Self::Fill => ProductionField::FillColor,
            Self::Line => ProductionField::LineColor,
        }
    }
}

/// `#rrggbb` lower-cased if well formed.
pub fn normalise_hex(text: &str) -> Option<String> {
    let text = text.trim();
    let text = text.strip_prefix('#').unwrap_or(text);
    (text.len() == 6 && text.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| format!("#{}", text.to_ascii_lowercase()))
}

impl StencilApp {
    /// Set (or clear with `None`) the fill or line colour of the selection: one undo step.
    pub fn set_selection_colour(&mut self, target: ColourTarget, colour: Option<String>) {
        if self.selected_shapes.is_empty() {
            self.status = "select shapes first".to_string();
            return;
        }
        let ids = self.selected_shapes.clone();
        let command = match colour {
            Some(hex) => ProductionCommand::SetProduction {
                ids,
                operation: None,
                line_color: (target == ColourTarget::Line).then(|| hex.clone()),
                fill_color: (target == ColourTarget::Fill).then_some(hex),
                material: None,
                send: None,
                clear: Vec::new(),
            },
            None => ProductionCommand::SetProduction {
                ids,
                operation: None,
                line_color: None,
                fill_color: None,
                material: None,
                send: None,
                clear: vec![target.field()],
            },
        };
        self.run_command(Command::Production(command));
    }

    /// The first selected shape's colour for `target`.
    fn selection_colour(&self, target: ColourTarget) -> Option<String> {
        let shape = self
            .selected_shapes
            .first()
            .and_then(|id| self.engine.doc.get_shape(*id))?;
        match target {
            ColourTarget::Fill => shape.production.fill_color.clone(),
            ColourTarget::Line => shape.production.line_color.clone(),
        }
    }

    fn colour_panel(&mut self, ui: &mut egui::Ui, target: ColourTarget) {
        let any = !self.selected_shapes.is_empty();
        let current = self.selection_colour(target);
        let mut picked: Option<Option<String>> = None;
        let mut hex = self.colour_hex_draft.clone();
        let (id, clear_label) = match target {
            ColourTarget::Fill => ("fill", "No Fill"),
            ColourTarget::Line => ("line", "Default Line"),
        };
        let _ = Panel::new(format!("stencilcraft-{id}-colour-panel")).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, colour) in PALETTE.iter().enumerate() {
                    let Some(c) = stencilcraft_ui_egui::parse_hex_colour(colour) else {
                        continue;
                    };
                    let response = ui.add(
                        Swatch::new(format!("{id}-swatch-{i}"), *colour, c)
                            .size(ControlSize::Sm)
                            .selected(current.as_deref() == Some(*colour))
                            .enabled(any),
                    );
                    if response.clicked() {
                        picked = Some(Some((*colour).to_string()));
                    }
                }
            });
            let edit = TextField::new(format!("{id}-hex"), "Colour (#rrggbb)", hex.as_str())
                .char_limit(9)
                .show(ui);
            if let Some(EditEvent::Commit { value, .. } | EditEvent::Set { value, .. }) = edit.event
            {
                hex = value.clone();
                if let Some(h) = normalise_hex(&value) {
                    picked = Some(Some(h));
                }
            } else if let Some(EditEvent::Update { value, .. }) = edit.event {
                hex = value;
            }
            if button(ui, clear_label, any) {
                picked = Some(None);
            }
            if target == ColourTarget::Line {
                ui.horizontal(|ui| {
                    if button(ui, "Cut", any) {
                        self.pending_style = Some(CutStyle::Cut);
                    }
                    if button(ui, "Engrave / Score", any) {
                        self.pending_style = Some(CutStyle::Engrave);
                    }
                });
            }
            hint(
                ui,
                if any {
                    "Colours label the design; what is cut depends on the line style."
                } else {
                    "Select shapes to colour."
                },
            );
        });
        self.colour_hex_draft = hex;
        if let Some(colour) = picked {
            self.set_selection_colour(target, colour);
        }
        if let Some(style) = self.pending_style.take() {
            self.handle_menu_action(crate::TopMenuAction::SetStyle(style));
        }
    }

    pub fn fill_colour_panel(&mut self, ui: &mut egui::Ui) {
        self.colour_panel(ui, ColourTarget::Fill);
    }

    pub fn line_style_panel(&mut self, ui: &mut egui::Ui) {
        self.colour_panel(ui, ColourTarget::Line);
    }

    /// The colours in use, with how many shapes have each, for Select by Color.
    pub fn colours_in_use(&self) -> Vec<(ColourTarget, String, Vec<u64>)> {
        let mut out: Vec<(ColourTarget, String, Vec<u64>)> = Vec::new();
        for (_, shape) in self.engine.doc.shapes() {
            if !stencilcraft_ui_egui::is_selectable(shape) {
                continue;
            }
            for (target, colour) in [
                (ColourTarget::Line, &shape.production.line_color),
                (ColourTarget::Fill, &shape.production.fill_color),
            ] {
                let Some(colour) = colour else { continue };
                match out.iter_mut().find(|(t, c, _)| *t == target && c == colour) {
                    Some((_, _, ids)) => ids.push(shape.id),
                    None => out.push((target, colour.clone(), vec![shape.id])),
                }
            }
        }
        out
    }

    /// The Select by Color panel: each line and fill colour in use; Select picks its shapes.
    pub fn select_by_colour_panel(&mut self, ui: &mut egui::Ui) {
        let colours = self.colours_in_use();
        let mut select = None;
        let _ = Panel::new("stencilcraft-select-by-colour-panel").show(ui, |ui| {
            if colours.is_empty() {
                hint(ui, "No shape has a line or fill colour yet.");
            }
            for (i, (target, colour, ids)) in colours.iter().enumerate() {
                ui.horizontal(|ui| {
                    if let Some(c) = stencilcraft_ui_egui::parse_hex_colour(colour) {
                        ui.add(
                            Swatch::new(format!("select-colour-{i}"), colour, c)
                                .size(ControlSize::Sm),
                        );
                    }
                    let what = match target {
                        ColourTarget::Line => "line",
                        ColourTarget::Fill => "fill",
                    };
                    ui.label(format!("{colour} {what} ({})", ids.len()));
                    if button(ui, "Select", true) {
                        select = Some(ids.clone());
                    }
                });
            }
        });
        if let Some(ids) = select {
            self.set_selection(ids);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_set_clear_and_select() {
        assert_eq!(normalise_hex(" FF8000 "), Some("#ff8000".into()));
        assert_eq!(normalise_hex("#12345"), None);
        let mut app = StencilApp::new(None);
        for x in [0.0, 30.0, 60.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            });
        }
        let ids: Vec<u64> = app.engine.doc.shapes().map(|(_, s)| s.id).collect();
        app.set_selection([ids[0], ids[1]]);
        app.set_selection_colour(ColourTarget::Fill, Some("#ff0000".into()));
        app.set_selection([ids[2]]);
        app.set_selection_colour(ColourTarget::Line, Some("#0000ff".into()));
        let used = app.colours_in_use();
        assert!(used.contains(&(ColourTarget::Fill, "#ff0000".into(), vec![ids[0], ids[1]])));
        assert!(used.contains(&(ColourTarget::Line, "#0000ff".into(), vec![ids[2]])));
        app.set_selection([ids[0]]);
        app.set_selection_colour(ColourTarget::Fill, None);
        assert_eq!(
            app.engine
                .doc
                .get_shape(ids[0])
                .and_then(|s| s.production.fill_color.clone()),
            None
        );
    }
}
