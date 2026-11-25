//! Silhouette's Send modes: Standard sends the whole design with one set of settings; By Line
//! Color, By Fill Color and By Layer list each colour (or layer) as a row with its own action,
//! pressure, speed and passes, and the cut runs the rows one after another, each with only its
//! shapes.

use eframe::egui;
use stencilcraft_doc::Document;

use crate::StencilApp;

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
        let _ = craft_ui::controls::Segmented::new("send-mode", "Send mode", &mut mode)
            .item(SendMode::Standard, "Standard")
            .item(SendMode::LineColor, "Line Color")
            .item(SendMode::FillColor, "Fill Color")
            .item(SendMode::Layer, "Layer")
            .size(craft_ui::controls::ControlSize::Sm)
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
                ui.add(
                    egui::DragValue::new(&mut self.send.overcut_mm)
                        .range(0.0..=10.0)
                        .speed(0.05)
                        .suffix(" mm"),
                );
            });
            ui.checkbox(&mut self.send.weld, "Weld overlapping shapes when cutting");
            ui.checkbox(&mut self.send.track_enhancing, "Track enhancing");
        });
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
