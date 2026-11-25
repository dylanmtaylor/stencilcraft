//! Silhouette Studio's Trace panel: choose Select Trace Area and drag a box over a placed
//! image; what will be traced is painted live in yellow (solid, or as outlines over the dimmed
//! image) while Threshold, Despeckle and the High/Low Pass filters change; Trace, Trace Outer
//! Edge and Trace and Detach each make one compound cut path and clear the area.

use craft_ui::composites::SectionHeader;
use craft_ui::controls::{Button, ButtonVariant, Checkbox, ControlSize, Segmented, Slider};
use craft_ui::edit::EditEvent;
use craft_ui::patterns::Panel;
use eframe::egui;
use stencilcraft_engine::Command;
use stencilcraft_geom::{BezPath, Point};
use stencilcraft_raster::{TraceMode, TraceOptions};

use crate::StencilApp;
use crate::panels::{button, hint, labelled, number};

/// Trace preview yellow, as in Silhouette Studio.
const PREVIEW_YELLOW: egui::Color32 = egui::Color32::from_rgb(255, 214, 0);

/// The smallest trace area side, millimetres; a smaller drag is treated as a click.
const MIN_AREA_MM: f64 = 0.5;

#[derive(Clone, Debug, PartialEq)]
pub struct TraceDraft {
    /// The trace area in millimetres: x, y, width, height.
    pub area: Option<[f64; 4]>,
    /// The next canvas drag sets the trace area.
    pub picking: bool,
    /// Percent: pixels darker than this are traced (Silhouette's default 45 %).
    pub threshold: f64,
    /// Islands and holes smaller than this many pixels are ignored.
    pub despeckle: f64,
    pub high_pass_on: bool,
    pub high_pass: f64,
    pub low_pass_on: bool,
    pub low_pass: f64,
    /// Preview as outlines over the dimmed image instead of a solid fill.
    pub outline_preview: bool,
}

impl Default for TraceDraft {
    fn default() -> Self {
        Self {
            area: None,
            picking: false,
            threshold: 45.0,
            despeckle: 0.0,
            high_pass_on: false,
            high_pass: 2.0,
            low_pass_on: false,
            low_pass: 2.0,
            outline_preview: false,
        }
    }
}

fn finite_in(value: f64, lo: f64, hi: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.clamp(lo, hi)
    } else {
        fallback
    }
}

impl TraceDraft {
    /// Engine trace options for `mode`, every value brought into range.
    pub fn options(&self, mode: TraceMode) -> TraceOptions {
        TraceOptions {
            threshold: finite_in(self.threshold, 0.0, 100.0, 45.0),
            // In range after the clamp, so the cast is exact.
            despeckle: finite_in(self.despeckle, 0.0, 10_000.0, 0.0).round() as u32,
            high_pass: if self.high_pass_on {
                finite_in(self.high_pass, 0.0, 50.0, 0.0)
            } else {
                0.0
            },
            low_pass: if self.low_pass_on {
                finite_in(self.low_pass, 0.0, 50.0, 0.0)
            } else {
                0.0
            },
            mode,
            ..TraceOptions::default()
        }
    }
}

/// The normalised rectangle (x, y, width, height) between two corners, if big enough.
pub fn area_between(a: Point, b: Point) -> Option<[f64; 4]> {
    let (x0, y0) = (a.x.min(b.x), a.y.min(b.y));
    let (w, h) = ((a.x - b.x).abs(), (a.y - b.y).abs());
    (w.is_finite() && h.is_finite() && w >= MIN_AREA_MM && h >= MIN_AREA_MM)
        .then_some([x0, y0, w, h])
}

fn area_corners([x, y, w, h]: [f64; 4]) -> [Point; 4] {
    [
        Point::new(x, y),
        Point::new(x + w, y),
        Point::new(x + w, y + h),
        Point::new(x, y + h),
    ]
}

fn outline_of(corners: [Point; 4]) -> BezPath {
    let mut path = BezPath::new();
    for (i, corner) in corners.into_iter().enumerate() {
        if i == 0 {
            path.move_to(corner);
        } else {
            path.line_to(corner);
        }
    }
    path.close_path();
    path
}

/// Ids of the placed images the area overlaps, bottom to top.
pub fn images_under(doc: &stencilcraft_doc::Document, [x, y, w, h]: [f64; 4]) -> Vec<u64> {
    doc.images
        .iter()
        .filter(|i| {
            i.x_mm < x + w && x < i.x_mm + i.width_mm && i.y_mm < y + h && y < i.y_mm + i.height_mm
        })
        .map(|i| i.id)
        .collect()
}

/// One image's part of the preview: the crop's corners and its yellow mask or outlines.
pub struct PreviewPiece {
    pub corners: [Point; 4],
    pub mask: Option<egui::TextureHandle>,
    pub outlines: Vec<BezPath>,
}

/// What the preview was built from, so it is rebuilt only when that changes.
#[derive(Clone, Debug, PartialEq)]
struct PreviewKey {
    images: Vec<(u64, usize, [f64; 4])>,
    area: [f64; 4],
    options: TraceOptions,
    outline: bool,
}

pub struct TracePreview {
    key: PreviewKey,
    pub pieces: Vec<PreviewPiece>,
    pub error: Option<String>,
}

fn preview_key(
    doc: &stencilcraft_doc::Document,
    area: [f64; 4],
    options: TraceOptions,
    outline: bool,
) -> PreviewKey {
    let under = images_under(doc, area);
    PreviewKey {
        images: doc
            .images
            .iter()
            .filter(|i| under.contains(&i.id))
            .map(|i| {
                (
                    i.id,
                    std::sync::Arc::as_ptr(&i.png_base64).cast::<u8>() as usize,
                    [i.x_mm, i.y_mm, i.width_mm, i.height_mm],
                )
            })
            .collect(),
        area,
        options,
        outline,
    }
}

fn build_piece(
    ctx: &egui::Context,
    image: &stencilcraft_doc::RasterImage,
    area: [f64; 4],
    options: &TraceOptions,
    outline: bool,
) -> Result<Option<PreviewPiece>, String> {
    let pixels = stencilcraft_raster::image_pixels(image).map_err(|e| e.to_string())?;
    let Some((crop, ox, oy)) = stencilcraft_raster::crop_to_area(image, &pixels, area) else {
        return Ok(None);
    };
    let (sx, sy) = (
        image.width_mm / f64::from(image.width_px.max(1)),
        image.height_mm / f64::from(image.height_px.max(1)),
    );
    let (fx, fy) = (f64::from(ox), f64::from(oy));
    let x0 = image.x_mm + fx * sx;
    let y0 = image.y_mm + fy * sy;
    let (cw, ch) = crop.dimensions();
    let corners = area_corners([x0, y0, f64::from(cw) * sx, f64::from(ch) * sy]);
    if outline {
        let traced = stencilcraft_raster::trace(&crop, options).map_err(|e| e.to_string())?;
        let map = |p: Point| Point::new(x0 + p.x * sx, y0 + p.y * sy);
        let outlines = traced
            .contours
            .iter()
            .map(|c| stencilcraft_raster::contour_path(c, options.corner_threshold, map))
            .collect();
        return Ok(Some(PreviewPiece {
            corners,
            mask: None,
            outlines,
        }));
    }
    let mask = stencilcraft_raster::trace_mask(&crop, options).map_err(|e| e.to_string())?;
    let (Ok(w), Ok(h)) = (usize::try_from(cw), usize::try_from(ch)) else {
        return Ok(None);
    };
    let pixels: Vec<egui::Color32> = mask
        .iter()
        .map(|&on| {
            if on {
                PREVIEW_YELLOW
            } else {
                egui::Color32::TRANSPARENT
            }
        })
        .collect();
    if pixels.len() != w.saturating_mul(h) {
        return Ok(None);
    }
    let colour = egui::ColorImage {
        size: [w, h],
        pixels,
        source_size: egui::vec2(w as f32, h as f32),
    };
    let texture = ctx.load_texture(
        format!("trace-preview-{}", image.id),
        colour,
        egui::TextureOptions::NEAREST,
    );
    Ok(Some(PreviewPiece {
        corners,
        mask: Some(texture),
        outlines: Vec::new(),
    }))
}

impl StencilApp {
    /// Rebuild the live preview when the area, the options or the images under it changed.
    fn refresh_trace_preview(&mut self, ctx: &egui::Context) {
        let Some(area) = self.panels.trace.area else {
            self.trace_preview = None;
            return;
        };
        let options = self.panels.trace.options(TraceMode::AllEdges);
        let outline = self.panels.trace.outline_preview;
        let key = preview_key(&self.engine.doc, area, options, outline);
        if self.trace_preview.as_ref().is_some_and(|p| p.key == key) {
            return;
        }
        let mut pieces = Vec::new();
        let mut error = None;
        for image in &self.engine.doc.images {
            if !key.images.iter().any(|(id, ..)| *id == image.id) {
                continue;
            }
            match build_piece(ctx, image, area, &options, outline) {
                Ok(Some(piece)) => pieces.push(piece),
                Ok(None) => {}
                Err(e) => error = Some(e),
            }
        }
        self.trace_preview = Some(TracePreview { key, pieces, error });
    }

    /// The canvas part of tracing: the area drag while picking, then the area and the preview.
    /// Returns true when it claimed the pointer.
    pub fn trace_canvas(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
    ) -> bool {
        let shown = std::mem::take(&mut self.panels.trace_shown);
        let view = self.view;
        let mut claimed = false;
        if self.panels.trace.picking {
            claimed = true;
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
            if response.drag_started()
                && let Some(press) = ui
                    .input(|input| input.pointer.press_origin())
                    .or(response.interact_pointer_pos())
            {
                self.trace_area_drag = Some(view.screen_to_world(press.to_vec2()));
            }
            if let (Some(start), Some(now)) =
                (self.trace_area_drag, ui.ctx().pointer_interact_pos())
            {
                let now = view.screen_to_world(now.to_vec2());
                let box_ = [
                    start.x.min(now.x),
                    start.y.min(now.y),
                    (start.x - now.x).abs(),
                    (start.y - now.y).abs(),
                ];
                stencilcraft_ui_egui::paint_preview_paths(
                    painter,
                    &view,
                    &[outline_of(area_corners(box_))],
                    stencilcraft_ui_egui::Palette::for_theme(self.theme).selection,
                );
            }
            if response.drag_stopped()
                && let (Some(start), Some(end)) =
                    (self.trace_area_drag.take(), response.interact_pointer_pos())
            {
                let end = view.screen_to_world(end.to_vec2());
                if let Some(area) = area_between(start, end) {
                    self.panels.trace.area = Some(area);
                    self.panels.trace.picking = false;
                    self.status = if images_under(&self.engine.doc, area).is_empty() {
                        "the trace area has no image under it".to_string()
                    } else {
                        "trace area set".to_string()
                    };
                }
            }
        }
        let Some(area) = self.panels.trace.area.filter(|_| shown) else {
            return claimed;
        };
        self.refresh_trace_preview(ui.ctx());
        if let Some(preview) = &self.trace_preview {
            for piece in &preview.pieces {
                if let Some(mask) = &piece.mask {
                    stencilcraft_ui_egui::paint_texture_quad(
                        painter,
                        &view,
                        piece.corners,
                        mask.id(),
                    );
                } else {
                    // Outline preview: dim the image under the would-be cut lines.
                    let quad: Vec<egui::Pos2> = piece
                        .corners
                        .iter()
                        .map(|p| view.world_to_screen(*p))
                        .collect();
                    painter.add(egui::Shape::convex_polygon(
                        quad,
                        egui::Color32::from_white_alpha(170),
                        egui::Stroke::NONE,
                    ));
                    stencilcraft_ui_egui::paint_preview_paths(
                        painter,
                        &view,
                        &piece.outlines,
                        PREVIEW_YELLOW,
                    );
                }
            }
        }
        stencilcraft_ui_egui::paint_preview_paths(
            painter,
            &view,
            &[outline_of(area_corners(area))],
            stencilcraft_ui_egui::Palette::for_theme(self.theme).selection,
        );
        claimed
    }

    /// Trace every image under the area with `mode`, as one compound path each, then clear the
    /// area as Silhouette does.
    pub fn trace_area(&mut self, mode: TraceMode) {
        let Some(area) = self.panels.trace.area else {
            return;
        };
        let images = images_under(&self.engine.doc, area);
        if images.is_empty() {
            self.status = "the trace area has no image under it".to_string();
            return;
        }
        let options = self.panels.trace.options(mode);
        for (i, id) in images.into_iter().enumerate() {
            self.run_adding(
                Command::ImageTrace {
                    id,
                    options,
                    style: stencilcraft_doc::CutStyle::Cut,
                    area: Some(area),
                    compound: true,
                },
                i > 0,
                "traced",
            );
        }
        self.panels.trace.area = None;
        self.trace_preview = None;
    }

    pub fn trace_panel(&mut self, ui: &mut egui::Ui) {
        self.panels.trace_shown = true;
        let numbers = self.locale_numbers.clone();
        let has_area = self.panels.trace.area.is_some();
        let has_image = self
            .panels
            .trace
            .area
            .is_some_and(|area| !images_under(&self.engine.doc, area).is_empty());
        let error = self.trace_preview.as_ref().and_then(|p| p.error.clone());
        let mut select_area = false;
        let mut clear_area = false;
        let mut action = None;
        let draft = &mut self.panels.trace;
        let _ = Panel::new("stencilcraft-trace-panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                select_area = ui
                    .add(
                        Button::new("Select Trace Area")
                            .variant(if has_area {
                                ButtonVariant::Secondary
                            } else {
                                ButtonVariant::Primary
                            })
                            .size(ControlSize::Sm),
                    )
                    .clicked();
                clear_area = button(ui, "Clear Area", has_area || draft.picking);
            });
            if draft.picking {
                hint(ui, "Drag a box over the image to trace.");
            }
            ui.add_enabled_ui(has_area, |ui| {
                let _ = SectionHeader::new("trace-preview", "Trace Preview").show(ui, |ui| {
                    let _ = Segmented::new("trace-display", "Preview", &mut draft.outline_preview)
                        .item(false, "Solid Fill")
                        .item(true, "Outline")
                        .size(ControlSize::Sm)
                        .show(ui);
                });
                let _ = SectionHeader::new("trace-filters", "Filters").show(ui, |ui| {
                    labelled(ui, "Threshold", |ui| {
                        let edit = Slider::new(
                            "trace-threshold",
                            "Threshold",
                            draft.threshold,
                            0.0..=100.0,
                        )
                        .default_value(45.0)
                        .value_text(format!("{:.1} %", draft.threshold))
                        .show(ui);
                        if let Some(
                            EditEvent::Update { value, .. }
                            | EditEvent::Commit { value, .. }
                            | EditEvent::Set { value, .. },
                        ) = edit.event
                        {
                            draft.threshold = value;
                        }
                    });
                    labelled(ui, "Despeckle", |ui| {
                        number(
                            ui,
                            "trace-despeckle",
                            "Despeckle",
                            &mut draft.despeckle,
                            0.0..=10_000.0,
                            "{n} px",
                            &numbers,
                        );
                    });
                    ui.horizontal(|ui| {
                        let _ = Checkbox::bool("High Pass", &mut draft.high_pass_on).show(ui);
                        number(
                            ui,
                            "trace-high-pass",
                            "High pass radius",
                            &mut draft.high_pass,
                            0.0..=50.0,
                            "{n} px",
                            &numbers,
                        );
                    });
                    ui.horizontal(|ui| {
                        let _ = Checkbox::bool("Low Pass", &mut draft.low_pass_on).show(ui);
                        number(
                            ui,
                            "trace-low-pass",
                            "Low pass radius",
                            &mut draft.low_pass,
                            0.0..=50.0,
                            "{n} px",
                            &numbers,
                        );
                    });
                });
            });
            if let Some(error) = &error {
                hint(ui, error);
            }
            ui.horizontal_wrapped(|ui| {
                for (label, mode) in [
                    ("Trace", TraceMode::AllEdges),
                    ("Trace Outer Edge", TraceMode::OuterEdge),
                    ("Trace and Detach", TraceMode::Detach),
                ] {
                    if button(ui, label, has_image && error.is_none()) {
                        action = Some(mode);
                    }
                }
            });
            if !has_area && !draft.picking {
                hint(
                    ui,
                    "Import an image, choose Select Trace Area and drag a box over it.",
                );
            }
        });
        if select_area {
            self.panels.trace.picking = true;
            self.panels.trace.area = None;
            self.active_tool = crate::ActiveTool::Select;
        }
        if clear_area {
            self.panels.trace.picking = false;
            self.panels.trace.area = None;
            self.trace_area_drag = None;
        }
        if let Some(mode) = action {
            self.trace_area(mode);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_follow_the_panel_and_stay_in_range() {
        let mut draft = TraceDraft::default();
        let o = draft.options(TraceMode::OuterEdge);
        assert_eq!(
            (o.threshold, o.despeckle, o.high_pass, o.low_pass),
            (45.0, 0, 0.0, 0.0)
        );
        assert_eq!(o.mode, TraceMode::OuterEdge);
        draft.high_pass_on = true;
        draft.high_pass = f64::NAN;
        draft.low_pass_on = true;
        draft.low_pass = 900.0;
        draft.threshold = -5.0;
        draft.despeckle = f64::INFINITY;
        let o = draft.options(TraceMode::AllEdges);
        assert_eq!(
            (o.threshold, o.despeckle, o.high_pass, o.low_pass),
            (0.0, 0, 0.0, 50.0)
        );
        assert!(o.validate().is_ok());
    }

    #[test]
    fn a_tiny_or_broken_drag_is_no_area() {
        assert_eq!(
            area_between(Point::new(5.0, 9.0), Point::new(1.0, 2.0)),
            Some([1.0, 2.0, 4.0, 7.0])
        );
        assert!(area_between(Point::new(0.0, 0.0), Point::new(0.1, 9.0)).is_none());
        assert!(area_between(Point::new(f64::NAN, 0.0), Point::new(3.0, 3.0)).is_none());
    }
}
