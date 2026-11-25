//! Silhouette Studio's Fill Color, Line Style and Select by Color panels. Colours are stored
//! on each shape (`production.fill_color` / `line_color`, `#rrggbb`) and drawn on the canvas;
//! they label the design (cut by colour, print) without changing what is cut.

use craft_theme::{ControlSize, Srgba};
use craft_ui::choice::Swatch;
use craft_ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft_ui::sections::Panel;
use craft_ui::text::TextField;
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
        let title = match target {
            ColourTarget::Fill => "Fill Color",
            ColourTarget::Line => "Line Style",
        };
        let _ = Panel::new(title)
            .id_salt(format!("stencilcraft-{id}-colour-panel"))
            .show(ui, |ui, _| {
                ui.horizontal_wrapped(|ui| {
                    for (i, colour) in PALETTE.iter().enumerate() {
                        let Some(c) = stencilcraft_ui_egui::parse_hex_colour(colour) else {
                            continue;
                        };
                        let clicked = Swatch::new(*colour, Srgba::from(c))
                            .id_salt(format!("{id}-swatch-{i}"))
                            .size(ControlSize::Sm)
                            .selected(current.as_deref() == Some(*colour))
                            .enabled(any)
                            .show(ui)
                            .clicked;
                        if clicked {
                            picked = Some(Some((*colour).to_string()));
                        }
                    }
                });
                // The field writes `hex` as it is typed; a committed value applies the colour.
                let edit = TextField::new("Colour (#rrggbb)", Bind::bound(&mut hex))
                    .id_salt(format!("{id}-hex"))
                    .limit(9)
                    .show(ui);
                if let Some(value) = edit.events.committed()
                    && let Some(h) = normalise_hex(value)
                {
                    picked = Some(Some(h));
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
                    // Silhouette's perforated line style: the lines become dashes that cut.
                    let (mut cut, mut gap) = self.perforate;
                    ui.horizontal(|ui| {
                        ui.label("Perforate: cut");
                        crate::panels::length_drag(ui, &mut cut, 0.2..=100.0, 0.1);
                        ui.label("gap");
                        crate::panels::length_drag(ui, &mut gap, 0.2..=100.0, 0.1);
                    });
                    self.perforate = (cut, gap);
                    if button(ui, "Perforate Lines", any) {
                        self.pending_perforate = true;
                    }
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
        if std::mem::take(&mut self.pending_perforate) {
            self.perforate_selection();
        }
    }

    /// Line Style › Perforate Lines: the selected lines become cut dashes (`generate.perforate`).
    pub fn perforate_selection(&mut self) {
        if self.selected_shapes.is_empty() {
            return;
        }
        let (cut, gap) = self.perforate;
        let ids = self.selected_shapes.clone();
        self.run_command(Command::GeneratePerforate(
            stencilcraft_engine::generate::PerforateArgs {
                ids: ids.clone(),
                cut_mm: cut,
                gap_mm: gap,
            },
        ));
        self.set_selection(ids);
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
        let mut recolor_row = self.recolor_row;
        let mut recolor_to: Option<(usize, String)> = None;
        let _ = Panel::new("Select by Color")
            .id_salt("stencilcraft-select-by-colour-panel")
            .show(ui, |ui, _| {
                if colours.is_empty() {
                    hint(ui, "No shape has a line or fill colour yet.");
                }
                for (i, (target, colour, ids)) in colours.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if let Some(c) = stencilcraft_ui_egui::parse_hex_colour(colour) {
                            let _ = Swatch::new(colour.clone(), Srgba::from(c))
                                .id_salt(format!("select-colour-{i}"))
                                .size(ControlSize::Sm)
                                .show(ui);
                        }
                        let what = match target {
                            ColourTarget::Line => "line",
                            ColourTarget::Fill => "fill",
                        };
                        ui.label(format!("{colour} {what} ({})", ids.len()));
                        if button(ui, "Select", true) {
                            select = Some(ids.clone());
                        }
                        if button(ui, "Recolor", true) {
                            recolor_row = (recolor_row != Some(i)).then_some(i);
                        }
                    });
                    if recolor_row == Some(i) {
                        ui.horizontal_wrapped(|ui| {
                            for (k, swatch) in PALETTE.iter().enumerate() {
                                let Some(c) = stencilcraft_ui_egui::parse_hex_colour(swatch) else {
                                    continue;
                                };
                                if Swatch::new(*swatch, Srgba::from(c))
                                    .id_salt(format!("recolor-{i}-{k}"))
                                    .size(ControlSize::Sm)
                                    .show(ui)
                                    .clicked
                                {
                                    recolor_to = Some((i, (*swatch).to_string()));
                                }
                            }
                        });
                    }
                }
            });
        self.recolor_row = recolor_row;
        if let Some(ids) = select {
            self.set_selection(ids);
        }
        if let Some((row, colour)) = recolor_to
            && let Some((target, _, ids)) = colours.get(row)
        {
            // Recolor Artwork: every shape with that colour takes the new one (one step).
            let keep = std::mem::replace(&mut self.selected_shapes, ids.clone());
            self.set_selection_colour(*target, Some(colour));
            self.set_selection(keep);
            self.recolor_row = None;
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

#[cfg(test)]
mod perforate_tests {
    use super::*;

    #[test]
    fn perforate_turns_lines_into_dashes() {
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 20.0,
        });
        let id = app
            .engine
            .doc
            .shapes()
            .map(|(_, s)| s.id)
            .next()
            .expect("rect");
        app.set_selection([id]);
        app.perforate = (3.0, 1.0);
        app.perforate_selection();
        let path = &app.engine.doc.get_shape(id).expect("shape").path;
        let pieces = path
            .elements()
            .iter()
            .filter(|el| matches!(el, craft_behavior::kurbo::PathEl::MoveTo(_)))
            .count();
        assert!(pieces > 20, "{pieces} dashes along 120 mm at 4 mm pitch");
        assert!(app.engine.undo());
    }

    #[test]
    fn drawing_can_return_to_select() {
        let mut app = StencilApp::new(None);
        app.active_tool = crate::ActiveTool::Rect;
        let (a, b) = (
            stencilcraft_geom::Point::new(0.0, 0.0),
            stencilcraft_geom::Point::new(20.0, 20.0),
        );
        app.commit_dragged_shape(a, b, crate::DragMods::default());
        assert_eq!(
            app.active_tool,
            crate::ActiveTool::Rect,
            "the tool stays by default"
        );
        app.prefs.select_after_drawing = true;
        app.commit_dragged_shape(a, b, crate::DragMods::default());
        assert_eq!(app.active_tool, crate::ActiveTool::Select);
    }
}
