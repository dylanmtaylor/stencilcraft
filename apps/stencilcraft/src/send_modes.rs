//! Silhouette's Send modes: Standard sends the whole design with one set of settings; By Line
//! Color, By Fill Color and By Layer list each colour (or layer) as a row with its own action,
//! pressure, speed and passes, and the cut runs the rows one after another, each with only its
//! shapes.

use craft_ui::contract::{Bind, Entry, Props as _, Show as _};
use eframe::egui;
use stencilcraft_doc::Document;

use crate::StencilApp;

/// A saved material (Send › My materials): its cut settings by name.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CustomMaterial {
    pub name: String,
    pub pressure: u8,
    pub speed: u8,
    pub passes: u8,
    #[serde(default)]
    pub depth: Option<u8>,
}

/// Most saved materials kept.
pub const MAX_CUSTOM_MATERIALS: usize = 100;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SendMode {
    #[default]
    Standard,
    LineColor,
    FillColor,
    Layer,
}

/// One row of a by-colour or by-layer send.
#[derive(Clone, Debug, PartialEq)]
pub struct SendRow {
    pub key: String,
    pub label: String,
    pub shapes: usize,
    pub enabled: bool,
    pub draw: bool,
    pub pressure: u8,
    pub speed: u8,
    pub passes: u8,
}

/// The group a shape belongs to in `mode`.
fn group_of(
    mode: SendMode,
    layer: usize,
    doc: &Document,
    shape: &stencilcraft_doc::Shape,
) -> (String, String) {
    match mode {
        SendMode::Standard => ("all".into(), "All".into()),
        SendMode::LineColor => match &shape.production.line_color {
            Some(c) => (c.clone(), format!("Line {c}")),
            None => ("".into(), "Default line".into()),
        },
        SendMode::FillColor => match &shape.production.fill_color {
            Some(c) => (c.clone(), format!("Fill {c}")),
            None => ("".into(), "No fill".into()),
        },
        SendMode::Layer => {
            let name = doc
                .layers
                .get(layer)
                .map_or_else(|| format!("Layer {}", layer + 1), |l| l.name.clone());
            (layer.to_string(), name)
        }
    }
}

/// The rows for `mode`, keeping the settings of rows that already exist.
pub fn rows_for(
    doc: &Document,
    mode: SendMode,
    existing: &[SendRow],
    defaults: (bool, u8, u8, u8),
) -> Vec<SendRow> {
    let mut rows: Vec<SendRow> = Vec::new();
    for (index, layer) in doc.layers.iter().enumerate() {
        if !layer.visible {
            continue;
        }
        for shape in layer.shapes.iter().filter(|s| !s.hidden) {
            let (key, label) = group_of(mode, index, doc, shape);
            if let Some(row) = rows.iter_mut().find(|r| r.key == key) {
                row.shapes += 1;
                continue;
            }
            let (draw, pressure, speed, passes) = defaults;
            let mut row = SendRow {
                key: key.clone(),
                label,
                shapes: 1,
                enabled: true,
                draw,
                pressure,
                speed,
                passes,
            };
            if let Some(old) = existing.iter().find(|r| r.key == key) {
                row.enabled = old.enabled;
                row.draw = old.draw;
                row.pressure = old.pressure;
                row.speed = old.speed;
                row.passes = old.passes;
            }
            rows.push(row);
        }
    }
    rows
}

/// `doc` with only the shapes of group `key`.
pub fn row_document(doc: &Document, mode: SendMode, key: &str) -> Document {
    let mut out = doc.clone();
    let source = doc.clone();
    for (index, layer) in out.layers.iter_mut().enumerate() {
        layer
            .shapes
            .retain(|shape| group_of(mode, index, &source, shape).0 == key);
    }
    out
}

impl StencilApp {
    /// The Send mode chooser and, outside Standard, the rows with their settings.
    pub fn send_mode_ui(&mut self, ui: &mut egui::Ui) {
        let mut mode = self.send.mode;
        ui.label("Send mode");
        let entries = [
            Entry::new(SendMode::Standard, "Standard"),
            Entry::new(SendMode::LineColor, "Line Color"),
            Entry::new(SendMode::FillColor, "Fill Color"),
            Entry::new(SendMode::Layer, "Layer"),
        ];
        let _ = craft_ui::choice::Segmented::new("Send mode", Bind::bound(&mut mode), &entries)
            .id_salt("send-mode")
            .show(ui);
        self.send.mode = mode;
        if mode == SendMode::Standard {
            return;
        }
        let defaults = (
            self.send.draw,
            self.send.pressure_override,
            self.send.speed_override,
            self.send.passes,
        );
        self.send.rows = rows_for(&self.engine.doc, mode, &self.send.rows, defaults);
        for row in &mut self.send.rows {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut row.enabled, format!("{} ({})", row.label, row.shapes));
                egui::ComboBox::from_id_salt(("send-row-action", row.key.as_str()))
                    .selected_text(if row.draw { "Draw" } else { "Cut" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut row.draw, false, "Cut");
                        ui.selectable_value(&mut row.draw, true, "Draw");
                    });
                ui.label("Pressure");
                ui.add(egui::DragValue::new(&mut row.pressure).range(1..=33));
                ui.label("Speed");
                ui.add(egui::DragValue::new(&mut row.speed).range(1..=30));
                ui.label("Passes");
                ui.add(egui::DragValue::new(&mut row.passes).range(1..=10));
            });
        }
    }

    /// Send › Advanced: cut order, interior first, overcut, weld on cut, track enhancing.
    pub fn send_advanced_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Advanced");
        ui.scope(|ui| {
            ui.horizontal(|ui| {
                ui.label("Cut order");
                egui::ComboBox::from_id_salt("send-order")
                    .selected_text(match self.send.order.as_str() {
                        "speed" => "Fastest path",
                        "none" => "Document order",
                        _ => "Least roller movement",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.send.order,
                            "roller".to_string(),
                            "Least roller movement",
                        );
                        ui.selectable_value(
                            &mut self.send.order,
                            "speed".to_string(),
                            "Fastest path",
                        );
                        ui.selectable_value(
                            &mut self.send.order,
                            "none".to_string(),
                            "Document order",
                        );
                    });
            });
            ui.checkbox(&mut self.send.interior_first, "Cut inner contours first");
            ui.horizontal(|ui| {
                ui.label("Overcut");
                crate::panels::length_drag(ui, &mut self.send.overcut_mm, 0.0..=10.0, 0.05);
            });
            ui.checkbox(&mut self.send.weld, "Weld overlapping shapes when cutting");
            ui.checkbox(&mut self.send.track_enhancing, "Track enhancing");
        });
    }

    /// Save the current Send settings as a material named `name` (replacing one of that name).
    pub fn save_custom_material(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.status = "name the material first".to_string();
            return;
        }
        let material = CustomMaterial {
            name: name.to_string(),
            pressure: self.send.pressure_override,
            speed: self.send.speed_override,
            passes: self.send.passes,
            depth: self.send.set_blade_depth.then_some(self.send.blade_depth),
        };
        let list = &mut self.prefs.custom_materials;
        list.retain(|m| m.name != material.name);
        list.push(material);
        if list.len() > MAX_CUSTOM_MATERIALS {
            list.remove(0);
        }
        self.send.custom_material = list.len().checked_sub(1);
        self.save_prefs();
        self.status = format!("saved material {name}");
    }

    /// Use saved material `index`'s settings.
    pub fn apply_custom_material(&mut self, index: usize) {
        let Some(m) = self.prefs.custom_materials.get(index).cloned() else {
            return;
        };
        self.send.pressure_override = m.pressure.clamp(1, 33);
        self.send.speed_override = m.speed.clamp(1, 30);
        self.send.passes = m.passes.clamp(1, 10);
        self.send.set_blade_depth = m.depth.is_some();
        if let Some(depth) = m.depth {
            self.send.blade_depth = depth.min(10);
        }
        self.send.custom_material = Some(index);
        self.status = format!("using material {}", m.name);
    }

    /// Send › My materials: pick, save or delete saved cut settings.
    pub fn custom_materials_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("My materials");
        let names: Vec<String> = self
            .prefs
            .custom_materials
            .iter()
            .map(|m| {
                format!(
                    "{} (pressure {}, speed {}, {}×)",
                    m.name, m.pressure, m.speed, m.passes
                )
            })
            .collect();
        let mut picked = None;
        let mut delete = false;
        ui.horizontal(|ui| {
            let current = self
                .send
                .custom_material
                .and_then(|i| names.get(i))
                .cloned()
                .unwrap_or_else(|| "Choose…".to_string());
            egui::ComboBox::from_id_salt("send-my-material")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (i, name) in names.iter().enumerate() {
                        if ui
                            .selectable_label(self.send.custom_material == Some(i), name)
                            .clicked()
                        {
                            picked = Some(i);
                        }
                    }
                });
            delete = ui
                .add_enabled(
                    self.send.custom_material.is_some(),
                    egui::Button::new("Delete"),
                )
                .clicked();
        });
        let mut save = false;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.send.custom_name)
                    .hint_text("Material name")
                    .desired_width(140.0),
            );
            save = ui.button("Save current settings").clicked();
        });
        if let Some(i) = picked {
            self.apply_custom_material(i);
        }
        if save {
            let name = self.send.custom_name.clone();
            self.save_custom_material(&name);
        }
        if delete
            && let Some(i) = self.send.custom_material.take()
            && i < self.prefs.custom_materials.len()
        {
            self.prefs.custom_materials.remove(i);
            self.save_prefs();
        }
    }

    /// The jobs a cut sends: the whole design in Standard mode, else one per enabled row.
    pub fn send_jobs(&self, send: bool) -> Vec<(Document, stencilcraft_engine::HardwareCommand)> {
        if self.send.mode == SendMode::Standard {
            return vec![(self.engine.doc.clone(), self.cut_command(send))];
        }
        self.send
            .rows
            .iter()
            .filter(|row| row.enabled && row.shapes > 0)
            .map(|row| {
                let mut command = self.cut_command(send);
                if let stencilcraft_engine::HardwareCommand::Cut {
                    force,
                    speed,
                    passes,
                    draw,
                    ..
                } = &mut command
                {
                    *force = u32::from(row.pressure);
                    *speed = u32::from(row.speed);
                    *passes = row.passes;
                    *draw = row.draw;
                }
                (
                    row_document(&self.engine.doc, self.send.mode, &row.key),
                    command,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_engine::Command;

    #[test]
    fn custom_materials_save_apply_and_replace() {
        let mut app = StencilApp::new(None);
        app.send.pressure_override = 22;
        app.send.speed_override = 3;
        app.send.passes = 2;
        app.save_custom_material("  ");
        assert!(app.prefs.custom_materials.is_empty());
        app.save_custom_material("Kraft board");
        app.send.pressure_override = 5;
        app.save_custom_material("Kraft board");
        assert_eq!(app.prefs.custom_materials.len(), 1, "same name replaces");
        app.send.pressure_override = 30;
        app.apply_custom_material(0);
        assert_eq!(app.send.pressure_override, 5);
        assert_eq!((app.send.speed_override, app.send.passes), (3, 2));
        app.apply_custom_material(99);
        assert_eq!(app.send.custom_material, Some(0));
    }

    #[test]
    fn rows_group_by_colour_and_layer_and_keep_settings() {
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
            crate::colour_panels::ColourTarget::Line,
            Some("#ff0000".into()),
        );
        let defaults = (false, 10, 5, 1);
        let mut rows = rows_for(&app.engine.doc, SendMode::LineColor, &[], defaults);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.iter().map(|r| r.shapes).sum::<usize>(), 3);
        rows[0].pressure = 25;
        let again = rows_for(&app.engine.doc, SendMode::LineColor, &rows, defaults);
        assert_eq!(again[0].pressure, 25, "settings survive a refresh");
        let red = row_document(&app.engine.doc, SendMode::LineColor, "#ff0000");
        assert_eq!(red.shapes().count(), 1);
        assert_eq!(
            rows_for(&app.engine.doc, SendMode::Layer, &[], defaults).len(),
            1
        );

        app.send.mode = SendMode::LineColor;
        app.send.rows = again;
        let jobs = app.send_jobs(false);
        assert_eq!(jobs.len(), 2);
        assert!(matches!(
            jobs[0].1,
            stencilcraft_engine::HardwareCommand::Cut { force: 25, .. }
        ));
        app.send.rows[1].enabled = false;
        assert_eq!(app.send_jobs(false).len(), 1);
        app.send.mode = SendMode::Standard;
        assert_eq!(app.send_jobs(false).len(), 1);
    }
}

/// The Send view's buttons for the selected lines (Silhouette's cut-preview buttons).
pub const SEND_ASSIGN_BUTTONS: &[(&str, Option<stencilcraft_doc::SendAssign>)] = &[
    (
        "Tool 1: Use all selected lines",
        Some(stencilcraft_doc::SendAssign::Tool1),
    ),
    (
        "Tool 1: Use outline of selection",
        Some(stencilcraft_doc::SendAssign::Tool1Outline),
    ),
    (
        "Tool 2: Use all selected lines",
        Some(stencilcraft_doc::SendAssign::Tool2),
    ),
    (
        "Tool 2: Use outline of selection",
        Some(stencilcraft_doc::SendAssign::Tool2Outline),
    ),
    (
        "Ignore selected lines",
        Some(stencilcraft_doc::SendAssign::Ignore),
    ),
    ("Clear tool choice", None),
];

impl StencilApp {
    /// Assign the selected shapes to a tool holder, their outline only, or nothing (`None`
    /// clears the choice back to the layer's settings).
    pub fn assign_send(&mut self, assign: Option<stencilcraft_doc::SendAssign>) {
        if self.selected_shapes.is_empty() {
            self.status = "select the lines to assign first".to_string();
            return;
        }
        let ids = self.selected_shapes.clone();
        self.run_command(stencilcraft_engine::Command::Production(
            stencilcraft_engine::ProductionCommand::SetProduction {
                ids,
                operation: None,
                line_color: None,
                fill_color: None,
                material: None,
                send: assign,
                clear: if assign.is_none() {
                    vec![stencilcraft_engine::ProductionField::Send]
                } else {
                    Vec::new()
                },
            },
        ));
        if let Some((label, _)) = SEND_ASSIGN_BUTTONS.iter().find(|(_, a)| *a == assign) {
            self.status = format!("{} ({} shapes)", label, self.selected_shapes.len());
        }
    }

    /// The Send view's "selected lines" buttons.
    pub fn send_assign_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Selected lines");
        let any = !self.selected_shapes.is_empty();
        let mut chosen = None;
        ui.horizontal_wrapped(|ui| {
            for (label, assign) in SEND_ASSIGN_BUTTONS {
                if crate::panels::button(ui, label, any) {
                    chosen = Some(*assign);
                }
            }
        });
        if let Some(assign) = chosen {
            self.assign_send(assign);
        }
    }
}

#[cfg(test)]
mod send_assign_tests {
    use super::*;

    #[test]
    fn selected_lines_go_to_a_tool_or_are_ignored() {
        let mut app = StencilApp::new(None);
        app.run_command(stencilcraft_engine::Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });
        app.assign_send(Some(stencilcraft_doc::SendAssign::Tool2));
        assert!(app.status.contains("select"), "nothing selected");
        app.select_latest_shape();
        let id = app.selected_shapes[0];
        let send = |app: &StencilApp| app.engine.doc.get_shape(id).and_then(|s| s.production.send);
        app.assign_send(Some(stencilcraft_doc::SendAssign::Tool2Outline));
        assert_eq!(send(&app), Some(stencilcraft_doc::SendAssign::Tool2Outline));
        app.assign_send(Some(stencilcraft_doc::SendAssign::Ignore));
        assert_eq!(send(&app), Some(stencilcraft_doc::SendAssign::Ignore));
        app.assign_send(None);
        assert_eq!(send(&app), None, "cleared");
        assert!(app.engine.undo());
        assert_eq!(
            send(&app),
            Some(stencilcraft_doc::SendAssign::Ignore),
            "one undo step each"
        );
    }
}
