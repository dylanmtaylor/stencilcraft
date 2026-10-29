//! The egui frontend: canvas painting, panels and theme tokens.
//!
//! The UI is thin: it reads document state and produces paint commands; all
//! acting goes through the engine's commands (owned by the app). Nothing here
//! depends on the app crate; the app depends on this crate and calls
//! [`paint_document`] with an [`egui::Painter`].

/// Theme tokens: every colour the frontend draws comes from here.
pub struct Tokens;

impl Tokens {
    pub const CANVAS_BG: egui::Color32 = egui::Color32::from_gray(28);
    pub const GRID: egui::Color32 = egui::Color32::from_gray(46);
    pub const BOARD: egui::Color32 = egui::Color32::from_gray(50);
    pub const CUT_LINE: egui::Color32 = egui::Color32::from_rgb(232, 87, 63);
    pub const KERF_LINE: egui::Color32 = egui::Color32::from_rgb(63, 136, 232);
    pub const ENGRAVE_LINE: egui::Color32 = egui::Color32::from_gray(140);
    pub const TEXT: egui::Color32 = egui::Color32::from_gray(200);
    pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(232, 87, 63);
}

/// The canvas view: a zoom and a pan, in screen pixels versus world millimetres.
/// Screen = (world − pan) × zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub zoom: f32,
    pub pan_mm: egui::Vec2,
}

impl Default for View {
    fn default() -> Self {
        // 4 px per mm fits the default 100 × 150 board inside a mid-size window.
        Self {
            zoom: 4.0,
            pan_mm: egui::Vec2::ZERO,
        }
    }
}

impl View {
    pub fn world_to_screen(&self, world: stencilcraft_geom::Point) -> egui::Pos2 {
        egui::pos2(
            ((world.x - f64::from(self.pan_mm.x)) * f64::from(self.zoom)) as f32,
            ((world.y - f64::from(self.pan_mm.y)) * f64::from(self.zoom)) as f32,
        )
    }

    /// Screen → world, used for pointer-driven pan/zoom.
    pub fn screen_to_world(&self, screen: egui::Vec2) -> stencilcraft_geom::Point {
        stencilcraft_geom::Point::new(
            f64::from(screen.x) / f64::from(self.zoom) + f64::from(self.pan_mm.x),
            f64::from(screen.y) / f64::from(self.zoom) + f64::from(self.pan_mm.y),
        )
    }

    /// Zoom by a factor while keeping `anchor_screen` stationary on the same
    /// world point.
    pub fn zoom_at(&mut self, factor: f32, anchor_screen: egui::Pos2) {
        let anchor = egui::vec2(anchor_screen.x, anchor_screen.y);
        let world_before = self.screen_to_world(anchor);
        self.zoom = (self.zoom * factor).clamp(0.05, 400.0);
        // Re-anchor: solve `anchor/new_zoom + pan = world_before` for pan.
        self.pan_mm.x += world_before.x as f32 - anchor.x / self.zoom;
        self.pan_mm.y += world_before.y as f32 - anchor.y / self.zoom;
    }

    /// The document's artboard corners on screen.
    pub fn board_rect_screen(&self, doc: &stencilcraft_doc::Document) -> egui::Rect {
        let min = self.world_to_screen(stencilcraft_geom::Point::ZERO);
        let max = self.world_to_screen(stencilcraft_geom::Point::new(
            doc.board.width_mm,
            doc.board.height_mm,
        ));
        egui::Rect::from_min_max(min, max)
    }
}

/// Colour and stroke weight for one shape, given its production style and the view.
fn shape_visual(
    shape: &stencilcraft_doc::Shape,
    view: &View,
    kerf_preview: bool,
) -> (egui::Color32, f32) {
    let line_color = if kerf_preview {
        // The compensated-contours preview: a different line colour so a user can
        // tell what the machine will actually trace.
        Tokens::KERF_LINE
    } else {
        match shape.style {
            stencilcraft_doc::CutStyle::Cut => Tokens::CUT_LINE,
            stencilcraft_doc::CutStyle::Engrave => Tokens::ENGRAVE_LINE,
        }
    };
    (line_color, 1.5_f32.max(0.4 * view.zoom))
}

/// Paint a whole document onto a painter: board rectangle, millimetre grid at a
/// sensible pitch, then each visible shape's flattened outline.
///
/// Headless-testable: the only egui requirement is a `Painter`, which tests can
/// obtain via `egui::Context::begin_pass`-style setup or a scratch painter.
pub fn paint_document(
    painter: &egui::Painter,
    clip: egui::Rect,
    doc: &stencilcraft_doc::Document,
    view: &View,
    kerf_preview: bool,
) {
    // Board backing. (The painter is already clipped by its caller.)
    painter.rect_filled(clip, 0.0, Tokens::CANVAS_BG);
    painter.rect_filled(view.board_rect_screen(doc), 2.0, Tokens::BOARD);

    // Grid: 10 mm major pitch, drawn only over the board as hairlines.
    let pitch_mm = 10.0_f64;
    let step_px = view.zoom * pitch_mm as f32;
    if step_px > 6.0 {
        let origin = view.world_to_screen(stencilcraft_geom::Point::ZERO);
        let mut x_mm = 0.0_f64;
        while x_mm <= doc.board.width_mm + 1e-9 {
            let x_px = origin.x + (x_mm * view.zoom as f64) as f32;
            painter.line_segment(
                [
                    egui::pos2(x_px, origin.y),
                    egui::pos2(x_px, origin.y + doc.board.height_mm as f32 * view.zoom),
                ],
                (1.0, Tokens::GRID),
            );
            x_mm += pitch_mm;
        }
        let mut y_mm = 0.0_f64;
        while y_mm <= doc.board.height_mm + 1e-9 {
            let y_px = origin.y + (y_mm * view.zoom as f64) as f32;
            painter.line_segment(
                [
                    egui::pos2(origin.x, y_px),
                    egui::pos2(origin.x + doc.board.width_mm as f32 * view.zoom, y_px),
                ],
                (1.0, Tokens::GRID),
            );
            y_mm += pitch_mm;
        }
    }

    // Shapes: flatten and convert each polyline to screen space.
    for layer in &doc.layers {
        if !layer.visible {
            continue;
        }
        for shape in &layer.shapes {
            let (color, width) = shape_visual(shape, view, kerf_preview);
            for pl in stencilcraft_geom::path_to_polylines(
                &shape.path,
                stencilcraft_geom::DEFAULT_FLATTEN_STEPS,
            ) {
                if pl.points.is_empty() {
                    continue;
                }
                let mut screen: Vec<egui::Pos2> =
                    pl.points.iter().map(|p| view.world_to_screen(*p)).collect();
                if pl.closed
                    && let Some(first) = pl.points.first()
                {
                    screen.push(view.world_to_screen(*first));
                }
                painter.add(egui::Shape::line(screen, (width, color)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_testkit::simple_square;

    #[test]
    fn world_screen_roundtrip_at_identity_pan() {
        let view = View::default();
        let world = stencilcraft_geom::Point::new(12.5, 30.25);
        let screen = view.world_to_screen(world);
        let back = view.screen_to_world(egui::vec2(screen.x, screen.y));
        assert!((back.x - world.x).abs() < 1e-9);
        assert!((back.y - world.y).abs() < 1e-9);
    }

    #[test]
    fn zoom_at_keeps_anchor_stationary() {
        let mut view = View::default();
        let anchor = egui::pos2(133.0, 77.0);
        let before = view.screen_to_world(egui::vec2(anchor.x, anchor.y));
        view.zoom_at(2.0, anchor);
        let after = view.screen_to_world(egui::vec2(anchor.x, anchor.y));
        assert!((after.x - before.x).abs() < 1e-6, "{before:?} → {after:?}");
        assert!((after.y - before.y).abs() < 1e-6);
    }

    #[test]
    fn board_rect_tracks_zoom() {
        let doc = simple_square();
        let view = View {
            zoom: 1.0,
            pan_mm: egui::Vec2::ZERO,
        };
        let r = view.board_rect_screen(&doc);
        assert_eq!(r.width(), doc.board.width_mm as f32);
    }

    #[test]
    fn kerf_preview_changes_visuals() {
        let doc = simple_square();
        let shape = &doc.layers[0].shapes[0];
        let view = View::default();
        let (plain_col, _) = shape_visual(shape, &view, false);
        let (kerf_col, _) = shape_visual(shape, &view, true);
        assert_ne!(plain_col, kerf_col);
    }
}
