//! Graphic Styles: named sets of line colour, fill colour and operation (cut or engrave),
//! saved from a shape and applied to the selection; Match Style is the eyedropper: the other
//! selected shapes take the first one's style.

use craft_ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft_ui::sections::Panel;
use craft_ui::text::TextField;
use eframe::egui;
use stencilcraft_doc::CutStyle;
use stencilcraft_engine::{Command, ProductionCommand, ProductionField};

use crate::StencilApp;
use crate::panels::{button, hint};

/// A saved graphic style.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GraphicStyle {
    pub name: String,
    #[serde(default)]
    pub line_color: Option<String>,
    #[serde(default)]
    pub fill_color: Option<String>,
    /// `true` engraves (draws/scores), else cuts.
    #[serde(default)]
    pub engrave: bool,
}

/// Most graphic styles kept.
const MAX_STYLES: usize = 100;

impl StencilApp {
    /// The style of shape `id`.
    fn style_of(&self, id: u64, name: &str) -> Option<GraphicStyle> {
        let shape = self.engine.doc.get_shape(id)?;
        Some(GraphicStyle {
            name: name.to_string(),
            line_color: shape.production.line_color.clone(),
            fill_color: shape.production.fill_color.clone(),
            engrave: shape.style == CutStyle::Engrave,
        })
    }

    /// Give shapes `ids` the style: colours (cleared when the style has none) and operation.
    fn apply_style(&mut self, ids: Vec<u64>, style: &GraphicStyle) {
        if ids.is_empty() {
            return;
        }
        let mut clear = Vec::new();
        if style.line_color.is_none() {
            clear.push(ProductionField::LineColor);
        }
        if style.fill_color.is_none() {
            clear.push(ProductionField::FillColor);
        }
        self.run_command(Command::Production(ProductionCommand::SetProduction {
            ids: ids.clone(),
            operation: None,
            line_color: style.line_color.clone(),
            fill_color: style.fill_color.clone(),
            material: None,
            send: None,
            fill_pattern: None,
            fill_gradient: None,
            clear,
        }));
        self.run_command(Command::ShapesSetStyle {
            ids: ids.clone(),
            style: if style.engrave {
                CutStyle::Engrave
            } else {
                CutStyle::Cut
            },
        });
        self.set_selection(ids);
    }

    /// Save the first selected shape's style as `name` (replacing a style of that name).
    pub fn save_graphic_style(&mut self, name: &str) {
        let name = name.trim();
        let Some(style) = self
            .selected_shapes
            .first()
            .and_then(|id| self.style_of(*id, name))
        else {
            self.status = "select a shape to take the style from".to_string();
            return;
        };
        if name.is_empty() {
            self.status = "name the style first".to_string();
            return;
        }
        let list = &mut self.prefs.graphic_styles;
        list.retain(|s| s.name != style.name);
        list.push(style);
        if list.len() > MAX_STYLES {
            list.remove(0);
        }
        self.save_prefs();
    }

    /// Match Style (eyedropper): the other selected shapes take the first one's style.
    pub fn match_style(&mut self) {
        let Some((first, rest)) = self.selected_shapes.split_first() else {
            return;
        };
        let rest = rest.to_vec();
        let all = self.selected_shapes.clone();
        if let Some(style) = self.style_of(*first, "") {
            self.apply_style(rest, &style);
        }
        self.set_selection(all);
    }

    pub fn graphic_styles_panel(&mut self, ui: &mut egui::Ui) {
        let styles = self.prefs.graphic_styles.clone();
        let any = !self.selected_shapes.is_empty();
        let mut name = self.style_name_draft.clone();
        let (mut save, mut matching) = (false, false);
        let mut apply = None;
        let mut remove = None;
        let _ = Panel::new("Graphic Styles")
            .id_salt("stencilcraft-graphic-styles-panel")
            .show(ui, |ui, _| {
                for (i, s) in styles.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if let Some(c) = s
                            .fill_color
                            .as_deref()
                            .or(s.line_color.as_deref())
                            .and_then(stencilcraft_ui_egui::parse_hex_colour)
                        {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, c);
                        }
                        ui.label(format!(
                            "{} ({})",
                            s.name,
                            if s.engrave { "engrave" } else { "cut" }
                        ));
                        if button(ui, "Apply", any) {
                            apply = Some(i);
                        }
                        if button(ui, "Delete", true) {
                            remove = Some(i);
                        }
                    });
                }
                if styles.is_empty() {
                    hint(
                        ui,
                        "Save a shape's colours and operation as a style to reuse them.",
                    );
                }
                let _ = TextField::new("Style name", Bind::bound(&mut name))
                    .id_salt("graphic-style-name")
                    .show(ui);
                // A style's name is at most 80 characters.
                if name.chars().count() > 80 {
                    name = name.chars().take(80).collect();
                }
                ui.horizontal(|ui| {
                    save = button(
                        ui,
                        "Save Style from Selection",
                        any && !name.trim().is_empty(),
                    );
                    matching = button(ui, "Match Style", self.selected_shapes.len() >= 2);
                });
                hint(
                    ui,
                    "Match Style gives the other selected shapes the first one's style.",
                );
            });
        self.style_name_draft = name.clone();
        if save {
            self.save_graphic_style(&name);
        }
        if matching {
            self.match_style();
        }
        if let Some(i) = apply
            && let Some(style) = self.prefs.graphic_styles.get(i).cloned()
        {
            let ids = self.selected_shapes.clone();
            self.apply_style(ids, &style);
        }
        if let Some(i) = remove
            && i < self.prefs.graphic_styles.len()
        {
            self.prefs.graphic_styles.remove(i);
            self.save_prefs();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_save_apply_and_match() {
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
        app.set_selection([ids[0]]);
        app.set_selection_colour(
            crate::colour_panels::ColourTarget::Fill,
            Some("#00ff00".into()),
        );
        app.handle_menu_action(crate::TopMenuAction::SetStyle(CutStyle::Engrave));
        app.save_graphic_style("Green engrave");
        assert_eq!(app.prefs.graphic_styles.len(), 1);
        app.set_selection([ids[1]]);
        let style = app.prefs.graphic_styles[0].clone();
        app.apply_style(vec![ids[1]], &style);
        let shape = app.engine.doc.get_shape(ids[1]).expect("shape");
        assert_eq!(shape.production.fill_color.as_deref(), Some("#00ff00"));
        assert_eq!(shape.style, CutStyle::Engrave);
        // Match Style: the third takes the first's style.
        app.set_selection([ids[0], ids[2]]);
        app.match_style();
        let third = app.engine.doc.get_shape(ids[2]).expect("shape");
        assert_eq!(third.production.fill_color.as_deref(), Some("#00ff00"));
    }
}
