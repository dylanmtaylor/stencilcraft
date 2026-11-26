//! Upload cleanup's Select, as an eraser: Effect › Erase Colour… arms the canvas, and each click
//! on a placed image makes the pixels like the clicked one transparent (`image.erase-colour`,
//! one undo step each), the connected region or every match. Done (or another tool) ends it.

use craft_ui::choice::{Checkbox, Segmented};
use craft_ui::contract::{Bind, Entry, Props as _, Show as _, ShowContainer as _};
use craft_ui::sections::Panel;
use eframe::egui;
use stencilcraft_engine::Command;

use crate::StencilApp;
use crate::panels::{button, hint, labelled, number};

#[derive(Clone, Debug, PartialEq)]
pub struct EraseColourState {
    /// Clicks on the canvas erase.
    pub picking: bool,
    /// Percent: how far a pixel's channels may differ from the clicked one.
    pub tolerance: f64,
    /// Only the region connected to the click, else every matching pixel.
    pub contiguous: bool,
    /// Erase with a brush (drag over the image) instead of by colour.
    pub brush: bool,
    /// The brush's diameter, millimetres.
    pub brush_mm: f64,
    /// The stroke being dragged: the image and its points (millimetres).
    pub stroke: Option<(u64, Vec<stencilcraft_geom::Point>)>,
}

impl Default for EraseColourState {
    fn default() -> Self {
        Self {
            picking: false,
            tolerance: 12.0,
            contiguous: true,
            brush: false,
            brush_mm: 3.0,
            stroke: None,
        }
    }
}

impl StencilApp {
    /// Erase the colour under `at` (millimetres) on the topmost image there.
    pub fn erase_colour_at(&mut self, image: u64, at: stencilcraft_geom::Point) {
        let state = self.erase_colour.clone();
        self.run_command(Command::ImageEraseColour {
            id: image,
            x_mm: at.x,
            y_mm: at.y,
            tolerance: state.tolerance,
            contiguous: state.contiguous,
        });
    }

    /// Erase the stroke `points` (millimetres) from `image` with the brush.
    pub fn brush_erase_image(&mut self, image: u64, points: &[stencilcraft_geom::Point]) {
        let radius_mm = self.erase_colour.brush_mm / 2.0;
        self.run_command(Command::ImageEraseBrush {
            id: image,
            points_mm: points.iter().map(|p| [p.x, p.y]).collect(),
            radius_mm,
        });
    }

    /// While picking: a crosshair; a click on an image erases its colour there, or with the
    /// brush a drag (or a click) over it erases the stroke. Returns true while picking (it owns
    /// the pointer).
    pub fn erase_colour_canvas(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
    ) -> bool {
        if !self.erase_colour.picking {
            return false;
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        if self.erase_colour.brush {
            self.brush_canvas(ui, response, painter);
            return true;
        }
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            match stencilcraft_ui_egui::hit_test_image(&self.engine.doc, &self.view, pos) {
                Some(image) => {
                    let at = self.view.screen_to_world(pos.to_vec2());
                    self.erase_colour_at(image, at);
                }
                None => self.status = "click a placed image to erase a colour".to_string(),
            }
        }
        true
    }

    fn brush_canvas(&mut self, ui: &egui::Ui, response: &egui::Response, painter: &egui::Painter) {
        let view = self.view;
        let world = |pos: egui::Pos2| view.screen_to_world(pos.to_vec2());
        if response.drag_started()
            && let Some(press) = ui.input(|input| input.pointer.press_origin())
            && let Some(image) =
                stencilcraft_ui_egui::hit_test_image(&self.engine.doc, &view, press)
        {
            self.erase_colour.stroke = Some((image, vec![world(press)]));
        }
        if response.dragged()
            && let Some(pos) = response.interact_pointer_pos()
            && let Some((_, points)) = self.erase_colour.stroke.as_mut()
            && points.len() < stencilcraft_raster::MAX_BRUSH_POINTS
        {
            let p = world(pos);
            // A new point every quarter of the brush.
            let step = (self.erase_colour.brush_mm / 4.0).max(0.05);
            if points.last().is_none_or(|last| (p - *last).hypot() >= step) {
                points.push(p);
            }
        }
        if response.drag_stopped()
            && let Some((image, points)) = self.erase_colour.stroke.take()
        {
            self.brush_erase_image(image, &points);
        } else if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            match stencilcraft_ui_egui::hit_test_image(&self.engine.doc, &view, pos) {
                Some(image) => self.brush_erase_image(image, &[world(pos)]),
                None => self.status = "brush over a placed image to erase it".to_string(),
            }
        }
        // The stroke so far, and the brush under the pointer.
        let radius = (self.erase_colour.brush_mm / 2.0 * f64::from(view.zoom)) as f32;
        let colour = stencilcraft_ui_egui::Palette::for_theme(self.theme).selection;
        if let Some((_, points)) = &self.erase_colour.stroke {
            let screen: Vec<egui::Pos2> = points.iter().map(|p| view.world_to_screen(*p)).collect();
            painter.add(egui::Shape::line(
                screen,
                egui::Stroke::new(radius * 2.0, colour.gamma_multiply(0.35)),
            ));
        }
        if let Some(at) = response.hover_pos() {
            painter.circle_stroke(at, radius, egui::Stroke::new(1.0, colour));
        }
    }

    pub fn erase_colour_panel(&mut self, ui: &mut egui::Ui) {
        let numbers = self.locale_numbers.clone();
        let mut state = self.erase_colour.clone();
        let has_image = !self.engine.doc.images.is_empty();
        let (mut start, mut done) = (false, false);
        let _ = Panel::new("Erase Colour")
            .id_salt("stencilcraft-erase-colour-panel")
            .show(ui, |ui, _| {
                let modes = [Entry::new(false, "Colour"), Entry::new(true, "Brush")];
                let _ = Segmented::new("Erase by", Bind::bound(&mut state.brush), &modes)
                    .id_salt("erase-mode")
                    .show(ui);
                if state.brush {
                    labelled(ui, "Brush size", |ui| {
                        number(
                            ui,
                            "erase-brush-size",
                            "Brush size",
                            &mut state.brush_mm,
                            0.1..=100.0,
                            "{n} mm",
                            &numbers,
                        );
                    });
                } else {
                    labelled(ui, "Tolerance", |ui| {
                        number(
                            ui,
                            "erase-tolerance",
                            "Erase tolerance",
                            &mut state.tolerance,
                            0.0..=100.0,
                            "{n} %",
                            &numbers,
                        );
                    });
                    let _ = Checkbox::new(Bind::bound(&mut state.contiguous))
                        .label("Contiguous")
                        .show(ui);
                }
                ui.horizontal(|ui| {
                    let label = if state.brush {
                        "Start Erasing"
                    } else {
                        "Pick Colour"
                    };
                    start = button(ui, label, has_image && !state.picking);
                    done = button(ui, "Done", state.picking);
                });
                hint(
                    ui,
                    if state.picking && state.brush {
                        "Drag over an image to erase under the brush; each stroke is one undo \
                         step."
                    } else if state.picking {
                        "Click a colour on an image to make it transparent; each click is one \
                         undo step."
                    } else {
                        "Clears a colour from a placed image (a background, a speck), so tracing \
                         and printing leave it out."
                    },
                );
            });
        if start {
            state.picking = true;
            self.active_tool = crate::ActiveTool::Select;
        }
        if done {
            state.picking = false;
        }
        if state.tolerance.is_finite() && state.brush_mm.is_finite() {
            self.erase_colour = state;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erasing_a_colour_makes_it_transparent_on_the_placed_image() {
        use craft_raster_codecs::common::{Pixel, Pixels};
        let mut app = StencilApp::new(None);
        let pixels = Pixels::from_fn(20, 10, |x, _| {
            if x < 10 {
                Pixel([30, 30, 200, 255])
            } else {
                Pixel([255, 255, 255, 255])
            }
        });
        let image = stencilcraft_raster::place(&pixels, "two", 100.0).expect("placed");
        app.place_image(image);
        let id = app.engine.doc.images.first().map(|i| i.id).expect("image");
        app.erase_colour_at(id, stencilcraft_geom::Point::new(4.0, 1.0));
        let after = stencilcraft_raster::image_pixels(app.engine.doc.get_image(id).expect("image"))
            .expect("pixels");
        assert_eq!(after.get_pixel(18, 5).0[3], 0, "the white half is gone");
        assert_eq!(after.get_pixel(2, 5).0[3], 255, "the blue half stays");
        // The brush: a stroke across the blue half's top clears under it only.
        app.erase_colour.brush_mm = 0.5;
        app.brush_erase_image(
            id,
            &[
                stencilcraft_geom::Point::new(0.2, 0.3),
                stencilcraft_geom::Point::new(2.3, 0.3),
            ],
        );
        let brushed =
            stencilcraft_raster::image_pixels(app.engine.doc.get_image(id).expect("image"))
                .expect("pixels");
        assert_eq!(brushed.get_pixel(4, 1).0[3], 0, "under the stroke");
        assert_eq!(brushed.get_pixel(4, 8).0[3], 255, "below it");
    }
}
