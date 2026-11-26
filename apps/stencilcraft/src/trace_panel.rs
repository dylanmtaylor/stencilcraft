//! Silhouette Studio's Trace panel: choose Select Trace Area and drag a box over a placed
//! image; what will be traced is painted live in yellow (solid, or as outlines over the dimmed
//! image) while Threshold, Despeckle and the High/Low Pass filters change; Trace, Trace Outer
//! Edge and Trace and Detach each make one compound cut path and clear the area.

use craft_theme::{ControlSize, Variant};
use craft_ui::buttons::Button;
use craft_ui::choice::{Checkbox, Segmented};
use craft_ui::contract::{Bind, Entry, Props as _, Show as _, ShowContainer as _};
use craft_ui::sections::{Panel, Section};
use craft_ui::value_drag::Slider;
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
    /// Silhouette's Trace Quality, Low (0) to Max (4): how closely the outlines follow the
    /// pixels.
    pub quality: u8,
    /// Trace by Colour: how many colours Find Colours looks for.
    pub colour_count: f64,
    /// The colours found under the area, and whether each is traced (else it is background).
    pub palette: Vec<([u8; 3], bool)>,
}

/// Smoothing tolerance (pixels) for each Trace Quality stop, Low to Max.
const QUALITY_SMOOTHING: [f64; 5] = [2.0, 1.5, 1.0, 0.5, 0.25];

/// What a drag on the trace box changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaGrab {
    Move,
    /// A corner, clockwise from the top left.
    Corner(u8),
    /// An edge midpoint: top, right, bottom, left.
    Edge(u8),
}

/// The trace box after dragging `grab` by `d` (millimetres), at least the minimum size.
pub fn edit_area(area: [f64; 4], grab: AreaGrab, d: stencilcraft_geom::Vec2) -> [f64; 4] {
    let [x, y, w, h] = area;
    let (mut x0, mut y0, mut x1, mut y1) = (x, y, x + w, y + h);
    match grab {
        AreaGrab::Move => {
            x0 += d.x;
            x1 += d.x;
            y0 += d.y;
            y1 += d.y;
        }
        AreaGrab::Corner(c) => {
            if c == 0 || c == 3 {
                x0 += d.x;
            } else {
                x1 += d.x;
            }
            if c < 2 {
                y0 += d.y;
            } else {
                y1 += d.y;
            }
        }
        AreaGrab::Edge(0) => y0 += d.y,
        AreaGrab::Edge(1) => x1 += d.x,
        AreaGrab::Edge(2) => y1 += d.y,
        AreaGrab::Edge(_) => x0 += d.x,
    }
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return area;
    }
    area_between(Point::new(x0, y0), Point::new(x1, y1)).unwrap_or(area)
}

/// The eight handles of a trace box (screen space), with what each grabs.
fn area_handles(area: [f64; 4], view: &stencilcraft_ui_egui::View) -> Vec<(AreaGrab, egui::Pos2)> {
    let [x, y, w, h] = area;
    let at = |px: f64, py: f64| view.world_to_screen(Point::new(px, py));
    vec![
        (AreaGrab::Corner(0), at(x, y)),
        (AreaGrab::Corner(1), at(x + w, y)),
        (AreaGrab::Corner(2), at(x + w, y + h)),
        (AreaGrab::Corner(3), at(x, y + h)),
        (AreaGrab::Edge(0), at(x + w / 2.0, y)),
        (AreaGrab::Edge(1), at(x + w, y + h / 2.0)),
        (AreaGrab::Edge(2), at(x + w / 2.0, y + h)),
        (AreaGrab::Edge(3), at(x, y + h / 2.0)),
    ]
}

/// Where the trace box's × button sits: just outside its top-right corner, on screen.
pub fn area_close_button(area: [f64; 4], view: &stencilcraft_ui_egui::View) -> egui::Pos2 {
    let [x, y, w, _] = area;
    view.world_to_screen(Point::new(x + w, y)) + egui::vec2(CLOSE_OFFSET_PX, -CLOSE_OFFSET_PX)
}

/// The × button's distance from the box corner and its radius, in screen pixels.
const CLOSE_OFFSET_PX: f32 = 14.0;
const CLOSE_RADIUS_PX: f32 = 8.0;

impl Default for TraceDraft {
    fn default() -> Self {
        Self {
            colour_count: 4.0,
            palette: Vec::new(),
            area: None,
            picking: false,
            threshold: 45.0,
            despeckle: 0.0,
            high_pass_on: false,
            high_pass: 2.0,
            low_pass_on: false,
            low_pass: 2.0,
            outline_preview: false,
            quality: 2,
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
            smoothing: QUALITY_SMOOTHING
                .get(usize::from(self.quality))
                .copied()
                .unwrap_or(1.0),
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
        let Some(mut area) = self.panels.trace.area.filter(|_| shown) else {
            return claimed;
        };
        // The × button clears the area, as in Silhouette.
        if !self.panels.trace.picking
            && response.clicked()
            && let Some(at) = response.interact_pointer_pos()
            && at.distance(area_close_button(area, &view)) <= CLOSE_RADIUS_PX
        {
            self.panels.trace.area = None;
            self.trace_area_grab = None;
            self.trace_preview = None;
            self.status = "trace area cleared".to_string();
            return true;
        }
        // Silhouette's trace box resizes from its handles and moves when dragged inside.
        if !self.panels.trace.picking {
            if response.drag_started()
                && let Some(press) = ui
                    .input(|input| input.pointer.press_origin())
                    .or(response.interact_pointer_pos())
            {
                let handle = area_handles(area, &view)
                    .into_iter()
                    .map(|(grab, at)| (grab, at.distance(press)))
                    .filter(|(_, d)| *d <= 8.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(grab, _)| grab);
                let p = view.screen_to_world(press.to_vec2());
                let [x, y, w, h] = area;
                let inside = p.x >= x && p.x <= x + w && p.y >= y && p.y <= y + h;
                if let Some(grab) = handle.or(inside.then_some(AreaGrab::Move)) {
                    self.trace_area_grab = Some((grab, area, p));
                }
            }
            if let Some((grab, original, from)) = self.trace_area_grab {
                claimed = true;
                if let Some(now) = response.interact_pointer_pos() {
                    let now = view.screen_to_world(now.to_vec2());
                    area = edit_area(original, grab, now - from);
                    self.panels.trace.area = Some(area);
                }
                if response.drag_stopped() {
                    self.trace_area_grab = None;
                }
            }
        }
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
        let colour = stencilcraft_ui_egui::Palette::for_theme(self.theme).selection;
        stencilcraft_ui_egui::paint_preview_paths(
            painter,
            &view,
            &[outline_of(area_corners(area))],
            colour,
        );
        for (_, at) in area_handles(area, &view) {
            painter.rect_filled(
                egui::Rect::from_center_size(at, egui::vec2(7.0, 7.0)),
                1.0,
                colour,
            );
        }
        if !self.panels.trace.picking {
            let at = area_close_button(area, &view);
            painter.circle_filled(at, CLOSE_RADIUS_PX, colour);
            let arm = CLOSE_RADIUS_PX * 0.45;
            let stroke = egui::Stroke::new(1.6, egui::Color32::WHITE);
            painter.line_segment(
                [at + egui::vec2(-arm, -arm), at + egui::vec2(arm, arm)],
                stroke,
            );
            painter.line_segment(
                [at + egui::vec2(-arm, arm), at + egui::vec2(arm, -arm)],
                stroke,
            );
        }
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

    /// Find Colours: the main colours of the first image under the area. Near-white colours
    /// start left out (they are usually the background).
    pub fn find_trace_colours(&mut self) {
        let Some(area) = self.panels.trace.area else {
            return;
        };
        let Some(image) = images_under(&self.engine.doc, area)
            .first()
            .and_then(|id| self.engine.doc.get_image(*id))
            .cloned()
        else {
            return;
        };
        // In 1..=16 after the clamp, so the cast is exact.
        let count = self.panels.trace.colour_count.round().clamp(2.0, 16.0) as usize;
        let found = stencilcraft_raster::image_pixels(&image)
            .ok()
            .and_then(|pixels| stencilcraft_raster::crop_to_area(&image, &pixels, area))
            .map(|(crop, _, _)| stencilcraft_raster::palette(&crop, count));
        match found {
            Some(Ok(colours)) => {
                self.panels.trace.palette = colours
                    .into_iter()
                    .map(|c| (c, c.iter().map(|v| u32::from(*v)).sum::<u32>() < 690))
                    .collect();
            }
            Some(Err(e)) => self.status = e.to_string(),
            None => self.status = "the trace area misses the image".to_string(),
        }
    }

    /// Trace by Colour: each ticked colour traces into its own filled layer.
    pub fn trace_by_colour(&mut self) {
        let Some(area) = self.panels.trace.area else {
            return;
        };
        let hex = |list: &[([u8; 3], bool)], on: bool| -> Vec<String> {
            list.iter()
                .filter(|(_, traced)| *traced == on)
                .map(|(c, _)| stencilcraft_raster::hex(*c))
                .collect()
        };
        let colours = hex(&self.panels.trace.palette, true);
        let ignore = hex(&self.panels.trace.palette, false);
        if colours.is_empty() {
            self.status = "tick at least one colour to trace".to_string();
            return;
        }
        let options = self.panels.trace.options(TraceMode::AllEdges);
        for (i, id) in images_under(&self.engine.doc, area).into_iter().enumerate() {
            self.run_adding(
                Command::ImageTraceColours {
                    id,
                    colours: colours.clone(),
                    ignore: ignore.clone(),
                    options,
                    area: Some(area),
                },
                i > 0,
                "traced by colour",
            );
        }
        self.panels.trace.area = None;
        self.panels.trace.palette.clear();
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
        let (mut find_colours, mut by_colour) = (false, false);
        let draft = &mut self.panels.trace;
        let display_entries = [Entry::new(false, "Solid Fill"), Entry::new(true, "Outline")];
        let quality_entries = [
            Entry::new(0, "Low"),
            Entry::new(1, "2"),
            Entry::new(2, "3"),
            Entry::new(3, "4"),
            Entry::new(4, "Max"),
        ];
        let _ = Panel::new("Trace")
            .id_salt("stencilcraft-trace-panel")
            .show(ui, |ui, _| {
                ui.horizontal(|ui| {
                    select_area = Button::new("Select Trace Area")
                        .variant(if has_area {
                            Variant::SECONDARY
                        } else {
                            Variant::PRIMARY
                        })
                        .size(ControlSize::Sm)
                        .show(ui)
                        .clicked();
                    clear_area = button(ui, "Clear Area", has_area || draft.picking);
                });
                if draft.picking {
                    hint(ui, "Drag a box over the image to trace.");
                }
                ui.add_enabled_ui(has_area, |ui| {
                    let _ =
                        Section::new("Trace Preview")
                            .id_salt("trace-preview")
                            .show(ui, |ui, _| {
                                let _ = Segmented::new(
                                    "Preview",
                                    Bind::bound(&mut draft.outline_preview),
                                    &display_entries,
                                )
                                .id_salt("trace-display")
                                .show(ui);
                                labelled(ui, "Quality", |ui| {
                                    let _ = Segmented::new(
                                        "Trace quality",
                                        Bind::bound(&mut draft.quality),
                                        &quality_entries,
                                    )
                                    .id_salt("trace-quality")
                                    .show(ui);
                                });
                            });
                    let _ = Section::new("Filters")
                        .id_salt("trace-filters")
                        .show(ui, |ui, _| {
                            labelled(ui, "Threshold", |ui| {
                                let text = format!("{:.1} %", draft.threshold);
                                // The slider writes the threshold as it moves; the reset is its default.
                                let _ = Slider::new(
                                    Bind::bound(&mut draft.threshold).reset(45.0),
                                    0.0..=100.0,
                                )
                                .label("Threshold")
                                .id_salt("trace-threshold")
                                .value_text(text)
                                .show(ui);
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
                                let _ = Checkbox::new(Bind::bound(&mut draft.high_pass_on))
                                    .label("High Pass")
                                    .show(ui);
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
                                let _ = Checkbox::new(Bind::bound(&mut draft.low_pass_on))
                                    .label("Low Pass")
                                    .show(ui);
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
                ui.add_enabled_ui(has_image, |ui| {
                let _ = Section::new("Trace by Colour").id_salt("trace-colours").show(ui, |ui, _| {
                    ui.horizontal(|ui| {
                        labelled(ui, "Colours", |ui| {
                            number(
                                ui,
                                "trace-colour-count",
                                "Colours to find",
                                &mut draft.colour_count,
                                2.0..=16.0,
                                "{n}",
                                &numbers,
                            );
                        });
                        find_colours = button(ui, "Find Colours", has_image);
                    });
                    for (i, (colour, traced)) in draft.palette.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            let [r, g, b] = *colour;
                            ui.painter()
                                .rect_filled(rect, 2.0, egui::Color32::from_rgb(r, g, b));
                            let label = format!("{} {}", i + 1, stencilcraft_raster::hex(*colour));
                            let _ = Checkbox::new(Bind::bound(traced))
                                .label(label)
                                .id_salt(("trace-colour", i))
                                .show(ui);
                        });
                    }
                    by_colour = button(
                        ui,
                        "Trace by Colour",
                        has_image && draft.palette.iter().any(|(_, on)| *on),
                    );
                    hint(
                        ui,
                        "Each ticked colour becomes a filled layer; unticked colours (like the \
                         background) are left out.",
                    );
                });
            });
                if !has_area && !draft.picking {
                    hint(
                        ui,
                        "Import an image, choose Select Trace Area and drag a box over it.",
                    );
                }
            });
        if find_colours {
            self.find_trace_colours();
        }
        if by_colour {
            self.trace_by_colour();
        }
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
    fn trace_by_colour_from_the_panel() {
        use craft_raster_codecs::common::{Pixel, Pixels};
        let mut app = StencilApp::new(None);
        let pixels = Pixels::from_fn(60, 30, |x, y| {
            if !(5..55).contains(&x) || !(5..25).contains(&y) {
                Pixel([255, 255, 255, 255])
            } else if x < 30 {
                Pixel([220, 30, 30, 255])
            } else {
                Pixel([30, 30, 220, 255])
            }
        });
        let image = stencilcraft_raster::place(&pixels, "two", 100.0).expect("placed");
        let (w, h) = (image.width_mm, image.height_mm);
        app.place_image(image);
        app.panels.trace.area = Some([0.0, 0.0, w, h]);
        app.panels.trace.colour_count = 3.0;
        app.find_trace_colours();
        assert_eq!(app.panels.trace.palette.len(), 3, "red, blue, white");
        assert_eq!(
            app.panels
                .trace
                .palette
                .iter()
                .filter(|(_, on)| *on)
                .count(),
            2,
            "white starts left out"
        );
        let shapes = app.engine.doc.shapes().count();
        app.trace_by_colour();
        assert_eq!(
            app.engine.doc.shapes().count(),
            shapes + 2,
            "{}",
            app.status
        );
        assert!(app.panels.trace.area.is_none() && app.panels.trace.palette.is_empty());
    }

    #[test]
    fn close_button_sits_off_the_top_right_corner() {
        let view = stencilcraft_ui_egui::View::default();
        let area = [10.0, 20.0, 30.0, 40.0];
        let corner = view.world_to_screen(Point::new(40.0, 20.0));
        let at = area_close_button(area, &view);
        assert!(at.x > corner.x && at.y < corner.y, "{at:?} vs {corner:?}");
    }

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
    fn the_trace_box_moves_and_resizes_and_quality_sets_smoothing() {
        use stencilcraft_geom::Vec2;
        let area = [10.0, 10.0, 20.0, 10.0];
        assert_eq!(
            edit_area(area, AreaGrab::Move, Vec2::new(5.0, -2.0)),
            [15.0, 8.0, 20.0, 10.0]
        );
        assert_eq!(
            edit_area(area, AreaGrab::Corner(2), Vec2::new(5.0, 5.0)),
            [10.0, 10.0, 25.0, 15.0]
        );
        assert_eq!(
            edit_area(area, AreaGrab::Edge(3), Vec2::new(-4.0, 9.0)),
            [6.0, 10.0, 24.0, 10.0]
        );
        // Dragging an edge past the other flips the box rather than breaking it.
        assert_eq!(
            edit_area(area, AreaGrab::Edge(1), Vec2::new(-30.0, 0.0)),
            [0.0, 10.0, 10.0, 10.0]
        );
        assert_eq!(
            edit_area(area, AreaGrab::Move, Vec2::new(f64::NAN, 0.0)),
            area
        );
        let mut draft = TraceDraft::default();
        assert_eq!(draft.options(TraceMode::AllEdges).smoothing, 1.0);
        draft.quality = 4;
        assert_eq!(draft.options(TraceMode::AllEdges).smoothing, 0.25);
        draft.quality = 200;
        assert_eq!(draft.options(TraceMode::AllEdges).smoothing, 1.0);
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
