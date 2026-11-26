//! PixScan, by hand (Silhouette's scan-to-cut, without the vendor mat): place a photo of the
//! page or mat, click its four corners on the canvas, and Straighten to Page warps the photo
//! (perspective) to the page at true size, so a design drawn or traced over it lines up with
//! the real material.

use craft_theme::{ControlSize, Variant};
use craft_ui::buttons::Button;
use craft_ui::contract::{Props as _, Show as _, ShowContainer as _};
use craft_ui::sections::Panel;
use eframe::egui;
use stencilcraft_engine::Command;
use stencilcraft_geom::Point;

use crate::StencilApp;
use crate::panels::{button, hint};

const CORNER_NAMES: [&str; 4] = ["top left", "top right", "bottom right", "bottom left"];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PixScanState {
    pub picking: bool,
    /// Corners clicked so far (millimetres), clockwise from the top left.
    pub corners: Vec<Point>,
}

impl StencilApp {
    /// The picked corners in the picked image's pixels, when all four are set.
    fn pixscan_corners_px(&self) -> Option<[[f64; 2]; 4]> {
        let image = self
            .selected_image
            .and_then(|id| self.engine.doc.get_image(id))?;
        if self.pixscan.corners.len() != 4 || !(image.width_mm > 0.0 && image.height_mm > 0.0) {
            return None;
        }
        let (sx, sy) = (
            f64::from(image.width_px) / image.width_mm,
            f64::from(image.height_px) / image.height_mm,
        );
        let mut out = [[0.0; 2]; 4];
        for (slot, p) in out.iter_mut().zip(&self.pixscan.corners) {
            *slot = [(p.x - image.x_mm) * sx, (p.y - image.y_mm) * sy];
        }
        Some(out)
    }

    /// Detect Corners: find the page in the picked photo and set its four corners, which can
    /// still be picked again by hand.
    pub fn detect_pixscan_corners(&mut self) {
        let Some(image) = self
            .selected_image
            .and_then(|id| self.engine.doc.get_image(id))
            .cloned()
        else {
            self.status = "pick the photo first".to_string();
            return;
        };
        let found = stencilcraft_raster::image_pixels(&image)
            .ok()
            .and_then(|pixels| stencilcraft_raster::detect_page_corners(&pixels));
        let Some(px) = found else {
            self.status = "no page found; click its corners instead".to_string();
            return;
        };
        if !(image.width_px > 0 && image.height_px > 0) {
            return;
        }
        let (sx, sy) = (
            image.width_mm / f64::from(image.width_px),
            image.height_mm / f64::from(image.height_px),
        );
        self.pixscan = PixScanState {
            picking: false,
            corners: px
                .iter()
                .map(|[x, y]| Point::new(image.x_mm + x * sx, image.y_mm + y * sy))
                .collect(),
        };
        self.status =
            "found the page's corners; Straighten to Page, or pick them again".to_string();
    }

    /// Straighten to Page: warp the picked image's page to the document page.
    pub fn straighten_to_page(&mut self) {
        let (Some(id), Some(corners)) = (self.selected_image, self.pixscan_corners_px()) else {
            self.status = "pick the image and its four corners first".to_string();
            return;
        };
        let (width_mm, height_mm) = (
            self.engine.doc.board.width_mm,
            self.engine.doc.board.height_mm,
        );
        self.run_command(Command::ImageRectify {
            id,
            corners,
            width_mm,
            height_mm,
        });
        self.pixscan = PixScanState::default();
    }

    /// Corner clicks while picking, and the corners and their outline on the canvas.
    /// Returns true while picking (it owns the clicks).
    pub fn pixscan_canvas(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
    ) -> bool {
        let view = self.view;
        if self.pixscan.picking {
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
            if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
            {
                let p = view.screen_to_world(pos.to_vec2());
                if p.x.is_finite() && p.y.is_finite() && self.pixscan.corners.len() < 4 {
                    self.pixscan.corners.push(p);
                }
                if self.pixscan.corners.len() == 4 {
                    self.pixscan.picking = false;
                }
            }
        }
        let colour = stencilcraft_ui_egui::Palette::for_theme(self.theme).selection;
        let screen: Vec<egui::Pos2> = self
            .pixscan
            .corners
            .iter()
            .map(|p| view.world_to_screen(*p))
            .collect();
        for pair in screen.windows(2) {
            if let [a, b] = pair {
                painter.line_segment([*a, *b], egui::Stroke::new(1.5, colour));
            }
        }
        if let (4, Some(first), Some(last)) = (screen.len(), screen.first(), screen.last()) {
            painter.line_segment([*last, *first], egui::Stroke::new(1.5, colour));
        }
        for at in &screen {
            painter.circle_stroke(*at, 6.0, egui::Stroke::new(2.0, colour));
        }
        self.pixscan.picking
    }

    pub fn pixscan_panel(&mut self, ui: &mut egui::Ui) {
        let has_image = self.selected_image.is_some();
        let picked = self.pixscan.corners.len();
        let picking = self.pixscan.picking;
        let (mut pick, mut reset, mut straighten, mut detect) = (false, false, false, false);
        let _ = Panel::new("PixScan")
            .id_salt("stencilcraft-pixscan-panel")
            .show(ui, |ui, _| {
            hint(
                ui,
                "Import a photo of the page or mat (File › Import › Image), pick it, then click \
                 its four corners: top left, top right, bottom right, bottom left.",
            );
            ui.horizontal(|ui| {
                detect = button(ui, "Detect Corners", has_image && !picking);
                pick = button(ui, "Pick Corners", has_image && !picking);
                reset = button(ui, "Reset", picked > 0 || picking);
            });
            if picking {
                let next = CORNER_NAMES.get(picked).copied().unwrap_or("corner");
                ui.label(format!("Click the page's {next} corner ({picked} of 4)."));
            }
            straighten = Button::new("Straighten to Page")
                .variant(Variant::PRIMARY)
                .size(ControlSize::Sm)
                .enabled(has_image && picked == 4)
                .show(ui)
                .clicked();
        });
        if pick {
            self.pixscan = PixScanState {
                picking: true,
                corners: Vec::new(),
            };
            self.active_tool = crate::ActiveTool::Select;
        }
        if reset {
            self.pixscan = PixScanState::default();
        }
        if detect {
            self.detect_pixscan_corners();
        }
        if straighten {
            self.straighten_to_page();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_corners_finds_the_page_in_a_photo() {
        use craft_raster_codecs::common::Pixel;
        let mut photo =
            stencilcraft_raster::RgbaImage::from_pixel(120, 90, Pixel([30, 30, 30, 255]));
        for y in 15..75 {
            for x in 20..100 {
                photo.put_pixel(x, y, Pixel([240, 240, 235, 255]));
            }
        }
        let mut app = StencilApp::new(None);
        app.detect_pixscan_corners();
        assert!(app.status.contains("pick the photo"));
        let image = stencilcraft_raster::place(&photo, "photo", 96.0).expect("placed");
        app.place_image(image);
        app.detect_pixscan_corners();
        assert_eq!(app.pixscan.corners.len(), 4, "{}", app.status);
        assert!(app.pixscan_corners_px().is_some(), "ready to straighten");
    }

    #[test]
    fn corners_map_to_image_pixels_and_straighten() {
        let mut app = StencilApp::new(None);
        let pixels = craft_raster_codecs::common::Pixels::from_pixel(
            100,
            100,
            craft_raster_codecs::common::Pixel([128, 128, 128, 255]),
        );
        // 100 px over 50 mm: 2 px per mm.
        let image = stencilcraft_raster::place(&pixels, "photo", 50.8).expect("placed");
        app.place_image(image);
        app.straighten_to_page();
        assert!(app.status.contains("four corners"));
        app.pixscan.corners = vec![
            Point::new(5.0, 5.0),
            Point::new(45.0, 5.0),
            Point::new(45.0, 45.0),
            Point::new(5.0, 45.0),
        ];
        let px = app.pixscan_corners_px().expect("corners");
        assert!(
            (px[1][0] - 90.0).abs() < 1e-9 && (px[1][1] - 10.0).abs() < 1e-9,
            "{px:?}"
        );
        app.straighten_to_page();
        let id = app.selected_image.expect("image");
        let image = app.engine.doc.get_image(id).expect("image");
        assert_eq!(
            (image.width_mm, image.height_mm),
            (100.0, 150.0),
            "the page size"
        );
        assert!(app.pixscan.corners.is_empty());
    }
}
