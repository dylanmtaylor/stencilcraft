//! Upload cleanup's Select, as an eraser: Effect › Erase Colour… arms the canvas, and each click
//! on a placed image makes the pixels like the clicked one transparent (`image.erase-colour`,
//! one undo step each), the connected region or every match. Done (or another tool) ends it.

use craft_ui::choice::Checkbox;
use craft_ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
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
}

impl Default for EraseColourState {
    fn default() -> Self {
        Self {
            picking: false,
            tolerance: 12.0,
            contiguous: true,
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

    /// While picking: a crosshair, and a click on an image erases its colour there. Returns
    /// true while picking (it owns the clicks).
    pub fn erase_colour_canvas(&mut self, ui: &egui::Ui, response: &egui::Response) -> bool {
        if !self.erase_colour.picking {
            return false;
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
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

    pub fn erase_colour_panel(&mut self, ui: &mut egui::Ui) {
        let numbers = self.locale_numbers.clone();
        let mut state = self.erase_colour.clone();
        let has_image = !self.engine.doc.images.is_empty();
        let (mut start, mut done) = (false, false);
        let _ = Panel::new("Erase Colour")
            .id_salt("stencilcraft-erase-colour-panel")
            .show(ui, |ui, _| {
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
                ui.horizontal(|ui| {
                    start = button(ui, "Pick Colour", has_image && !state.picking);
                    done = button(ui, "Done", state.picking);
                });
                hint(
                    ui,
                    if state.picking {
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
        if state.tolerance.is_finite() {
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
    }
}
