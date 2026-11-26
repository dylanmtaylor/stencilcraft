//! The Measure tool: drag a line on the design to read its length, its horizontal and
//! vertical extent and its angle. The measurement stays on the canvas until the next drag or
//! another tool; it is never part of the document.

use eframe::egui;
use stencilcraft_engine::document::{DocumentCommand, DocumentEffect, Measurement};
use stencilcraft_engine::{Command, Effect};
use stencilcraft_geom::Point;

use crate::{ActiveTool, StencilApp, panels};

impl StencilApp {
    /// The engine's measurement of `from` → `to` (millimetres).
    pub fn measure(&mut self, from: Point, to: Point) -> Option<Measurement> {
        match self
            .engine
            .run(Command::Document(DocumentCommand::MeasureDistance {
                from: [from.x, from.y],
                to: [to.x, to.y],
            })) {
            Ok(Effect::Document(DocumentEffect::Measure(m))) => Some(m),
            _ => None,
        }
    }

    /// One line of text for a measurement, in the person's units.
    pub fn measure_text(m: &Measurement) -> String {
        format!(
            "{}  (W {}, H {})  {:.1}°",
            panels::len(m.distance_mm),
            panels::len(m.dx_mm.abs()),
            panels::len(m.dy_mm.abs()),
            // The page is y-down; read the angle the way the screen shows it.
            -m.angle_degrees
        )
    }

    pub fn measure_canvas(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
    ) {
        if response.drag_started()
            && let Some(press) = ui.input(|input| input.pointer.press_origin())
        {
            let p = self.snap_world_point(self.view.screen_to_world(press.to_vec2()));
            self.measure_line = Some((p, p));
        }
        if response.dragged()
            && let (Some(pointer), Some((from, _))) =
                (response.interact_pointer_pos(), self.measure_line)
        {
            let to = self.snap_world_point(self.view.screen_to_world(pointer.to_vec2()));
            let to = crate::line_end(from, to, crate::DragMods::from(ui.input(|i| i.modifiers)));
            self.measure_line = Some((from, to));
        }
        let Some((from, to)) = self.measure_line else {
            return;
        };
        if self.active_tool != ActiveTool::Measure {
            return;
        }
        let colour = stencilcraft_ui_egui::Palette::for_theme(self.theme).selection;
        let (a, b) = (
            self.view.world_to_screen(from),
            self.view.world_to_screen(to),
        );
        painter.line_segment([a, b], egui::Stroke::new(1.5, colour));
        for end in [a, b] {
            painter.circle_stroke(end, 3.0, egui::Stroke::new(1.5, colour));
        }
        if let Some(m) = self.measure(from, to) {
            let text = Self::measure_text(&m);
            if response.drag_stopped() {
                self.status = format!("measured {text}");
            }
            let at = a.lerp(b, 0.5) + egui::vec2(8.0, -18.0);
            let font = egui::FontId::proportional(12.0);
            let galley = painter.layout_no_wrap(text, font, colour);
            let rect = egui::Rect::from_min_size(at, galley.size()).expand(3.0);
            painter.rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
            painter.galley(at, galley, colour);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measuring_reads_length_extent_and_screen_angle() {
        let mut app = StencilApp::new(None);
        let m = app
            .measure(Point::new(0.0, 0.0), Point::new(30.0, -40.0))
            .expect("measured");
        assert!((m.distance_mm - 50.0).abs() < 1e-9);
        let text = StencilApp::measure_text(&m);
        assert!(
            text.contains("53.1°"),
            "up and right reads positive: {text}"
        );
        // Nothing is added to the document.
        assert!(app.engine.doc.shapes().next().is_none());
        assert!(!app.engine.can_undo());
    }
}
