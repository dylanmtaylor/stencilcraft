//! Silhouette's Print & Cut panel: registration marks on the page (length, thickness, inset),
//! bleed, a check that the design stays clear of the marks, and printing the page at 100 %.

use craft_ui::choice::Checkbox;
use craft_ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft_ui::sections::Panel;
use eframe::egui;
use stencilcraft_doc::Document;
use stencilcraft_geom::Shape as _;

use crate::StencilApp;
use crate::panels::{button, hint, labelled, number};

/// Clearance kept between the marks and the design, millimetres.
const MARK_GAP_MM: f64 = 2.0;

/// The marks and bleed (kept in the preferences, like Silhouette's last-used settings).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MarkSettings {
    pub length_mm: f64,
    pub thickness_mm: f64,
    pub inset_mm: f64,
    /// Print past each filled shape's edge so a slightly-off cut shows no white edge.
    pub bleed: bool,
    pub bleed_mm: f64,
}

impl Default for MarkSettings {
    fn default() -> Self {
        let [length_mm, thickness_mm, inset_mm] = stencilcraft_ui_egui::DEFAULT_MARK_GEOMETRY;
        Self {
            length_mm,
            thickness_mm,
            inset_mm,
            bleed: false,
            bleed_mm: 0.05 * 25.4,
        }
    }
}

impl MarkSettings {
    pub fn geometry(&self) -> [f64; 3] {
        [self.length_mm, self.thickness_mm, self.inset_mm]
    }

    /// The area the design must stay in: inside the marks with a small gap.
    pub fn safe_area(&self, width: f64, height: f64) -> stencilcraft_geom::Rect {
        let edge = self.inset_mm + self.thickness_mm + MARK_GAP_MM;
        stencilcraft_geom::Rect::new(edge, edge, width - edge, height - edge)
    }
}

/// `doc` with a bleed band (the shape grown by `bleed_mm`, in its fill colour) behind every
/// filled shape, for printing.
pub fn with_bleed(doc: &Document, bleed_mm: f64) -> Document {
    let mut out = doc.clone();
    if !(bleed_mm.is_finite() && bleed_mm > 0.0) {
        return out;
    }
    let offset = stencilcraft_kerf::OffsetOptions {
        join: stencilcraft_kerf::JoinStyle::Round,
        ..stencilcraft_kerf::OffsetOptions::default()
    };
    let mut next_id = out.next_shape_id;
    for layer in &mut out.layers {
        let mut shapes = Vec::with_capacity(layer.shapes.len() * 2);
        for shape in &layer.shapes {
            if let Some(fill) = shape.production.fill_color.clone()
                && let Ok(grown) = stencilcraft_kerf::offset_path(&shape.path, bleed_mm, &offset)
            {
                let mut band = shape.clone();
                band.id = next_id;
                next_id = next_id.saturating_add(1);
                band.path = grown;
                band.production.line_color = Some(fill);
                band.text = None;
                shapes.push(band);
            }
            shapes.push(shape.clone());
        }
        layer.shapes = shapes;
    }
    out.next_shape_id = next_id;
    out
}

impl StencilApp {
    /// The Print & Cut page as SVG: the page with its marks, fills and line colours (and bleed).
    pub fn print_sheet_svg(&self) -> Result<String, String> {
        let marks = self.prefs.marks;
        let doc = if marks.bleed {
            with_bleed(&self.engine.doc, marks.bleed_mm)
        } else {
            self.engine.doc.clone()
        };
        let rects = stencilcraft_ui_egui::mark_rects(
            doc.board.width_mm,
            doc.board.height_mm,
            marks.geometry(),
        );
        stencilcraft_svg::export_print_sheet(&doc, &rects).map_err(|e| e.to_string())
    }

    /// Shapes that reach into the marks (outside the safe area).
    pub fn shapes_over_marks(&self) -> usize {
        let doc = &self.engine.doc;
        let safe = self
            .prefs
            .marks
            .safe_area(doc.board.width_mm, doc.board.height_mm);
        doc.shapes()
            .filter(|(_, s)| !s.hidden)
            .filter(|(_, s)| {
                let b = s.path.bounding_box();
                b.x0 < safe.x0 || b.y0 < safe.y0 || b.x1 > safe.x1 || b.y1 > safe.y1
            })
            .count()
    }

    pub fn print_cut_panel(&mut self, ui: &mut egui::Ui) {
        let numbers = self.locale_numbers.clone();
        let mut on = self.prefs.print_cut_marks;
        let mut marks = self.prefs.marks;
        let over = if on { self.shapes_over_marks() } else { 0 };
        let (mut print, mut defaults) = (false, false);
        let _ = Panel::new("Print & Cut")
            .id_salt("stencilcraft-print-cut-panel")
            .show(ui, |ui, _| {
                let _ = Checkbox::new(Bind::bound(&mut on))
                    .label("Show Registration Marks")
                    .show(ui);
                ui.add_enabled_ui(on, |ui| {
                for (label, id, value, range) in [
                    ("Length", "marks-length", &mut marks.length_mm, 2.0..=100.0),
                    (
                        "Thickness",
                        "marks-thickness",
                        &mut marks.thickness_mm,
                        0.1..=5.0,
                    ),
                    ("Inset", "marks-inset", &mut marks.inset_mm, 0.0..=100.0),
                ] {
                    labelled(ui, label, |ui| {
                        number(ui, id, label, value, range, "{n} mm", &numbers);
                    });
                }
                ui.horizontal(|ui| {
                    let _ = Checkbox::new(Bind::bound(&mut marks.bleed)).label("Bleed").show(ui);
                    number(
                        ui,
                        "marks-bleed",
                        "Bleed",
                        &mut marks.bleed_mm,
                        0.0..=10.0,
                        "{n} mm",
                        &numbers,
                    );
                });
                if over > 0 {
                    hint(
                        ui,
                        &format!(
                            "{over} shape(s) reach into the registration marks' area; move them \
                             inside so the cutter can read the marks."
                        ),
                    );
                }
                ui.horizontal(|ui| {
                    print = button(ui, "Print…", true);
                    defaults = button(
                        ui,
                        "Defaults",
                        marks != crate::print_marks::MarkSettings::default(),
                    );
                });
            });
                hint(
                    ui,
                    "Print the page at 100 % (no fit to page), load it with the marks at the top \
                 left, and send: the cutter reads the marks before cutting.",
                );
            });
        if defaults {
            marks = MarkSettings::default();
        }
        let valid = [
            marks.length_mm,
            marks.thickness_mm,
            marks.inset_mm,
            marks.bleed_mm,
        ]
        .iter()
        .all(|v| v.is_finite());
        if (on != self.prefs.print_cut_marks || marks != self.prefs.marks) && valid {
            self.prefs.print_cut_marks = on;
            self.prefs.marks = marks;
            self.save_prefs();
        }
        if print {
            self.print_via_pdf();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_engine::Command;

    #[test]
    fn marks_bleed_and_the_safe_area() {
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 1.0,
            y: 1.0,
            width: 10.0,
            height: 10.0,
        });
        app.run_command(Command::ShapeAddRect {
            x: 40.0,
            y: 40.0,
            width: 10.0,
            height: 10.0,
        });
        app.prefs.print_cut_marks = true;
        assert_eq!(
            app.shapes_over_marks(),
            1,
            "the corner rectangle is over the marks"
        );
        let id = app
            .engine
            .doc
            .shapes()
            .map(|(_, s)| s.id)
            .last()
            .expect("rect");
        app.set_selection([id]);
        app.set_selection_colour(
            crate::colour_panels::ColourTarget::Fill,
            Some("#ff0000".into()),
        );
        let bled = with_bleed(&app.engine.doc, 1.0);
        assert_eq!(bled.shapes().count(), 3, "one band behind the filled shape");
        let band = bled
            .shapes()
            .map(|(_, s)| s)
            .find(|s| s.id >= app.engine.doc.next_shape_id)
            .expect("band");
        let b = band.path.bounding_box();
        assert!((b.width() - 12.0).abs() < 1e-6, "grown by the bleed: {b:?}");
        app.prefs.marks.bleed = true;
        let svg = app.print_sheet_svg().expect("sheet");
        assert!(svg.contains("registration-marks"));
        assert_eq!(
            svg.matches("<rect").count(),
            5,
            "square and two brackets: {svg}"
        );
    }
}
