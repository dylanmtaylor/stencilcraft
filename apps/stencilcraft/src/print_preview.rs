//! File › Print with Silhouette's General preferences: Show Print Preview (on) opens a preview
//! of the page (with the registration marks when Print & Cut is on) before printing, and
//! Print Resolution (300 / 600 / 1200 dpi or custom; 600 by default) is the resolution the
//! printed file renders placed images at.

use craft_ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use eframe::egui;

use crate::StencilApp;

/// The longest side of the preview picture, in pixels.
const PREVIEW_PX: f64 = 520.0;

/// The preview dialog's picture of the page.
pub struct PrintPreview {
    texture: egui::TextureHandle,
    /// The page size it shows, millimetres.
    page_mm: (f64, f64),
}

/// The dpi that fits a `width` × `height` mm page into the preview.
pub fn preview_dpi(width: f64, height: f64) -> f64 {
    // `max` would pass over a NaN side, so both are checked.
    let long = width.max(height);
    if !(width.is_finite() && height.is_finite() && long > 0.0) {
        return 72.0;
    }
    (PREVIEW_PX * 25.4 / long).clamp(1.0, 300.0)
}

impl StencilApp {
    /// File › Print: the preview first when Show Print Preview is on, else print at once.
    pub fn request_print(&mut self, ctx: &egui::Context) {
        if !self.prefs.show_print_preview {
            self.print_via_pdf();
            return;
        }
        let doc = &self.engine.doc;
        let (w, h) = (doc.board.width_mm, doc.board.height_mm);
        let rendered = stencilcraft_import::render(doc, preview_dpi(w, h));
        match rendered {
            Ok(pixels) => {
                let (pw, ph) = pixels.dimensions();
                let (Ok(pw), Ok(ph)) = (usize::try_from(pw), usize::try_from(ph)) else {
                    return;
                };
                let image = egui::ColorImage::from_rgba_unmultiplied([pw, ph], &pixels.into_raw());
                let texture =
                    ctx.load_texture("print-preview", image, egui::TextureOptions::LINEAR);
                self.print_preview = Some(PrintPreview {
                    texture,
                    page_mm: (w, h),
                });
            }
            Err(e) => self.status = format!("print preview: {e}"),
        }
    }

    pub fn print_preview_dialog(&mut self, ui: &mut egui::Ui) {
        use craft_ui::dialog::Dialog;
        use craft_ui::sections::{DialogRole, FooterButton};
        let Some(preview) = self.print_preview.as_ref() else {
            return;
        };
        let marks = self.prefs.print_cut_marks.then(|| {
            let m = self.prefs.marks;
            stencilcraft_ui_egui::styled_mark_rects(
                preview.page_mm.0,
                preview.page_mm.1,
                m.geometry(),
                m.style(),
            )
        });
        let texture = preview.texture.clone();
        let page_mm = preview.page_mm;
        let mut open = true;
        let shown = Dialog::new("Print Preview")
            .id_salt("print-preview")
            .opened(Bind::bound(&mut open))
            .buttons([
                FooterButton::new("Print", DialogRole::Accept),
                FooterButton::new("Cancel", DialogRole::Cancel),
            ])
            .show(ui, |ui, _| {
                let size = texture.size_vec2();
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
                painter.image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                // The marks, in page millimetres scaled to the picture.
                if let Some(rects) = &marks {
                    let (sx, sy) = (
                        rect.width() / page_mm.0 as f32,
                        rect.height() / page_mm.1 as f32,
                    );
                    for [x, y, w, h] in rects {
                        let mark = egui::Rect::from_min_size(
                            rect.min + egui::vec2(*x as f32 * sx, *y as f32 * sy),
                            egui::vec2(*w as f32 * sx, *h as f32 * sy),
                        );
                        painter.rect_filled(mark, 0.0, egui::Color32::BLACK);
                    }
                }
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, egui::Color32::GRAY),
                    egui::StrokeKind::Outside,
                );
                let _ = craft_ui::display::Text::new(format!(
                    "{} dpi · print at 100 % (no fit to page)",
                    self.prefs.print_dpi
                ))
                .show(ui);
            });
        if shown.output.picked == Some(0) {
            self.print_preview = None;
            self.print_via_pdf();
        } else if !open || shown.output.closed.is_some() {
            self.print_preview = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preview_fits_its_picture_and_survives_bad_sizes() {
        let dpi = preview_dpi(304.8, 304.8);
        assert!((304.8 / 25.4 * dpi - PREVIEW_PX).abs() < 1e-6);
        assert_eq!(preview_dpi(0.0, 0.0), 72.0);
        assert_eq!(preview_dpi(f64::NAN, 10.0), 72.0);
        assert!(preview_dpi(1.0, 1.0) <= 300.0);
    }
}
