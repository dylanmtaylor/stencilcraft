//! The egui frontend: canvas painting, panels and theme tokens.
//!
//! The UI is thin: it reads document state and produces paint commands; all
//! acting goes through the engine's commands (owned by the app). Nothing here
//! depends on the app crate; the app depends on this crate and calls
//! [`paint_document`] with an [`egui::Painter`].

/// Theme selection shared by the workspace chrome and canvas renderer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ColorTheme {
    #[default]
    Dark,
    Light,
}

/// Theme tokens: every colour the frontend draws comes from here.
pub struct Tokens;

/// Semantic canvas colours and overlay flags for a specific workspace theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasOptions {
    pub kerf_preview: bool,
    pub show_grid: bool,
    pub print_cut_marks: bool,
    pub show_selection_bounds: bool,
    pub theme: ColorTheme,
}

impl Default for CanvasOptions {
    fn default() -> Self {
        Self {
            kerf_preview: false,
            show_grid: true,
            print_cut_marks: false,
            show_selection_bounds: true,
            theme: ColorTheme::Dark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub canvas_bg: egui::Color32,
    pub mat: egui::Color32,
    pub mat_grid: egui::Color32,
    pub ruler: egui::Color32,
    pub grid: egui::Color32,
    pub board: egui::Color32,
    pub cut_line: egui::Color32,
    pub kerf_line: egui::Color32,
    pub engrave_line: egui::Color32,
    pub text: egui::Color32,
    pub accent: egui::Color32,
    pub selection: egui::Color32,
}

impl Palette {
    pub fn for_theme(theme: ColorTheme) -> Self {
        match theme {
            ColorTheme::Dark => Self {
                canvas_bg: Tokens::CANVAS_BG,
                mat: Tokens::MAT,
                mat_grid: Tokens::MAT_GRID,
                ruler: Tokens::RULER,
                grid: Tokens::GRID,
                board: Tokens::BOARD,
                cut_line: Tokens::CUT_LINE,
                kerf_line: Tokens::KERF_LINE,
                engrave_line: Tokens::ENGRAVE_LINE,
                text: Tokens::TEXT,
                accent: Tokens::ACCENT,
                selection: Tokens::SELECTION,
            },
            ColorTheme::Light => Self {
                canvas_bg: egui::Color32::from_rgb(0xdf, 0xe3, 0xe9),
                mat: egui::Color32::from_rgb(0xf3, 0xf5, 0xf8),
                mat_grid: egui::Color32::from_rgb(0xd0, 0xd5, 0xdd),
                ruler: egui::Color32::from_rgb(0x58, 0x60, 0x6e),
                grid: egui::Color32::from_rgb(0xdd, 0xe1, 0xe8),
                board: egui::Color32::WHITE,
                cut_line: Tokens::ACCENT_LIGHT,
                kerf_line: egui::Color32::from_rgb(0x14, 0x73, 0xe6),
                engrave_line: egui::Color32::from_rgb(0x58, 0x60, 0x6e),
                text: egui::Color32::from_rgb(0x1b, 0x1f, 0x27),
                accent: Tokens::ACCENT_LIGHT,
                selection: Tokens::SELECTION,
            },
        }
    }
}

impl Tokens {
    /// Suite-shared dark canvas hues: VectorCraft Modern Dark pasteboard/panel
    /// with the Craft coral accent and blue selection. Chrome (craft-ui
    /// `suite_default`) and canvas now share one source.
    pub const CANVAS_BG: egui::Color32 = egui::Color32::from_rgb(0x10, 0x12, 0x17);
    pub const MAT: egui::Color32 = egui::Color32::from_rgb(0x18, 0x1b, 0x22);
    pub const MAT_GRID: egui::Color32 = egui::Color32::from_rgb(0x26, 0x2b, 0x35);
    pub const RULER: egui::Color32 = egui::Color32::from_rgb(0x6f, 0x78, 0x8a);
    pub const GRID: egui::Color32 = egui::Color32::from_rgb(0x23, 0x28, 0x32);
    pub const BOARD: egui::Color32 = egui::Color32::from_rgb(0x27, 0x2c, 0x36);
    pub const CUT_LINE: egui::Color32 = egui::Color32::from_rgb(232, 87, 63);
    pub const KERF_LINE: egui::Color32 = egui::Color32::from_rgb(63, 136, 232);
    pub const ENGRAVE_LINE: egui::Color32 = egui::Color32::from_rgb(0x96, 0x9f, 0xaf);
    pub const TEXT: egui::Color32 = egui::Color32::from_rgb(0xdf, 0xe4, 0xec);
    pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(0xf0, 0x6a, 0x52);
    /// Dark-on-light coral for the light theme (VectorCraft Modern Light).
    pub const ACCENT_LIGHT: egui::Color32 = egui::Color32::from_rgb(0xd9, 0x47, 0x2f);
    /// Selection blue, shared by both themes (VectorCraft `selection`).
    pub const SELECTION: egui::Color32 = egui::Color32::from_rgb(0x4f, 0x80, 0xff);
}

/// The cutting mat under the design sheet: physical extent in millimetres
/// (from the machine crate's `Mat` table) plus a short label painted on it.
#[derive(Debug, Clone, PartialEq)]
pub struct MatBackdrop {
    pub width_mm: f64,
    pub height_mm: f64,
    pub label: String,
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
        if !factor.is_finite()
            || factor <= 0.0
            || !self.zoom.is_finite()
            || self.zoom <= 0.0
            || !anchor_screen.is_finite()
        {
            return;
        }
        let anchor = egui::vec2(anchor_screen.x, anchor_screen.y);
        let world_before = self.screen_to_world(anchor);
        self.zoom = (self.zoom * factor).clamp(0.05, 400.0);
        // Re-anchor: solve `anchor/new_zoom + pan = world_before` for pan.
        self.pan_mm.x += world_before.x as f32 - anchor.x / self.zoom;
        self.pan_mm.y += world_before.y as f32 - anchor.y / self.zoom;
    }

    /// Center a world-space extent in `viewport` while preserving the current zoom.
    pub fn center_extent(&mut self, width_mm: f64, height_mm: f64, viewport: egui::Rect) -> bool {
        if !valid_view_extent(width_mm, height_mm, viewport)
            || !self.zoom.is_finite()
            || self.zoom <= 0.0
        {
            return false;
        }
        self.pan_mm = egui::vec2(
            (width_mm * 0.5 - f64::from(viewport.center().x / self.zoom)) as f32,
            (height_mm * 0.5 - f64::from(viewport.center().y / self.zoom)) as f32,
        );
        self.pan_mm.is_finite()
    }

    /// Fit a world-space extent inside `viewport`, leaving `margin` points on each side.
    pub fn fit_extent(
        &mut self,
        width_mm: f64,
        height_mm: f64,
        viewport: egui::Rect,
        margin: f32,
    ) -> bool {
        if !valid_view_extent(width_mm, height_mm, viewport)
            || !margin.is_finite()
            || margin < 0.0
            || viewport.width() <= margin * 2.0
            || viewport.height() <= margin * 2.0
        {
            return false;
        }
        let available_width = f64::from(viewport.width() - margin * 2.0);
        let available_height = f64::from(viewport.height() - margin * 2.0);
        self.zoom = (available_width / width_mm)
            .min(available_height / height_mm)
            .clamp(0.05, 400.0) as f32;
        self.center_extent(width_mm, height_mm, viewport)
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

    /// The cutting mat's corners on screen (mat origin = sheet origin: media
    /// loads left-aligned on Cameo 4-series machines).
    pub fn mat_rect_screen(&self, backdrop: &MatBackdrop) -> egui::Rect {
        let min = self.world_to_screen(stencilcraft_geom::Point::ZERO);
        let max = self.world_to_screen(stencilcraft_geom::Point::new(
            backdrop.width_mm,
            backdrop.height_mm,
        ));
        egui::Rect::from_min_max(min, max)
    }

    /// Convert to [`craft_ui::canvas::CanvasView`] for `Rulers`, `BezierOverlay`, `PenDraft`,
    /// `TransformBox`, `CanvasScrollbars`, and `MarqueeDrag`.
    pub fn to_canvas_view(&self, viewport: egui::Rect) -> craft_ui::canvas::CanvasView {
        let center_screen = viewport.center();
        let center_world = self.screen_to_world(egui::vec2(center_screen.x, center_screen.y));
        craft_ui::canvas::CanvasView::new(viewport, center_world, f64::from(self.zoom.max(0.05)))
    }

    /// Update `zoom` and `pan_mm` from a [`craft_ui::canvas::CanvasView`].
    pub fn from_canvas_view(&mut self, cv: &craft_ui::canvas::CanvasView) {
        if !cv.zoom.is_finite()
            || cv.zoom <= 0.0
            || !cv.center.x.is_finite()
            || !cv.center.y.is_finite()
        {
            return;
        }
        let zoom = (cv.zoom as f32).clamp(0.05, 400.0);
        let center_screen = cv.rect.center();
        self.zoom = zoom;
        self.pan_mm = egui::vec2(
            cv.center.x as f32 - center_screen.x / zoom,
            cv.center.y as f32 - center_screen.y / zoom,
        );
    }
}

fn valid_view_extent(width_mm: f64, height_mm: f64, viewport: egui::Rect) -> bool {
    width_mm.is_finite()
        && height_mm.is_finite()
        && width_mm > 0.0
        && height_mm > 0.0
        && viewport.is_finite()
        && viewport.width() > 0.0
        && viewport.height() > 0.0
}

/// Paint the cutting mat: surface, inch grid, one-inch ruler ticks on the
/// top and left edges, and the mat label. The sheet paints on top later.
fn paint_mat(painter: &egui::Painter, view: &View, backdrop: &MatBackdrop, palette: Palette) {
    let rect = view.mat_rect_screen(backdrop);
    painter.rect_filled(rect, 2.0, palette.mat);

    let inch_px = (25.4 * view.zoom as f64) as f32;
    if inch_px > 10.0 {
        // Inch grid over the mat surface.
        let mut i = 1.0_f64;
        while i * 25.4 < backdrop.width_mm {
            let x = rect.left() + (i * 25.4 * view.zoom as f64) as f32;
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                (1.0, palette.mat_grid),
            );
            i += 1.0;
        }
        let mut j = 1.0_f64;
        while j * 25.4 < backdrop.height_mm {
            let y = rect.top() + (j * 25.4 * view.zoom as f64) as f32;
            painter.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                (1.0, palette.mat_grid),
            );
            j += 1.0;
        }
    }
    if inch_px > 4.0 {
        // Half-inch ruler ticks on the top and left mat edges, full-inch
        // ticks darker and longer.
        let mut k = 1.0_f64;
        while k * 12.7 < backdrop.width_mm {
            let x = rect.left() + (k * 12.7 * view.zoom as f64) as f32;
            let (len, color) = tick_style(k, palette);
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.top() + len)],
                (1.0, color),
            );
            k += 1.0;
        }
        let mut m = 1.0_f64;
        while m * 12.7 < backdrop.height_mm {
            let y = rect.top() + (m * 12.7 * view.zoom as f64) as f32;
            let (len, color) = tick_style(m, palette);
            painter.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.left() + len, y)],
                (1.0, color),
            );
            m += 1.0;
        }
    }
    painter.text(
        egui::pos2(rect.left() + 4.0, rect.bottom() - 4.0),
        egui::Align2::LEFT_BOTTOM,
        &backdrop.label,
        egui::FontId::proportional(10.0),
        palette.text,
    );
}

/// Ruler tick length and colour: inch marks are bold, half-inch marks faint.
fn tick_style(index: f64, palette: Palette) -> (f32, egui::Color32) {
    if index % 2.0 == 0.0 {
        (8.0, palette.ruler)
    } else {
        (4.0, palette.mat_grid)
    }
}

/// Colour and stroke weight for one shape, given its production style and the view.
fn shape_visual(
    shape: &stencilcraft_doc::Shape,
    view: &View,
    kerf_preview: bool,
    palette: Palette,
) -> (egui::Color32, f32) {
    let line_color = if kerf_preview {
        // The compensated-contours preview: a different line colour so a user can
        // tell what the machine will actually trace.
        palette.kerf_line
    } else {
        match shape.style {
            stencilcraft_doc::CutStyle::Cut => palette.cut_line,
            stencilcraft_doc::CutStyle::Engrave => palette.engrave_line,
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
    backdrop: Option<&MatBackdrop>,
    options: CanvasOptions,
    selection: &SelectionState,
) {
    let palette = Palette::for_theme(options.theme);
    // Canvas backing, then (optionally) the cutting mat, then the design
    // sheet. Overflow beyond the mat simply stays canvas — a real-world
    // situation (12×24 art on a 12×12 mat) is visible, not masked.
    painter.rect_filled(clip, 0.0, palette.canvas_bg);
    if let Some(backdrop) = backdrop {
        paint_mat(painter, view, backdrop, palette);
    }
    painter.rect_filled(view.board_rect_screen(doc), 2.0, palette.board);

    // Grid: 10 mm major pitch, drawn only over the board as hairlines.
    if options.show_grid {
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
                    (1.0, palette.grid),
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
                    (1.0, palette.grid),
                );
                y_mm += pitch_mm;
            }
        }
    }

    if options.print_cut_marks {
        paint_registration_marks(painter, view, doc, palette);
    }

    // Shapes: flatten and convert each polyline to screen space.
    for layer in &doc.layers {
        if !layer.visible {
            continue;
        }
        for shape in &layer.shapes {
            let (mut color, mut width) = shape_visual(shape, view, options.kerf_preview, palette);
            if selection.selected_shapes.contains(&shape.id) {
                color = palette.selection;
                width = width.max(2.5);
            }
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

    if let Some(overlay) = selection.overlay.as_ref() {
        match overlay {
            SelectionOverlay::Box(rect) => {
                painter.rect_stroke(
                    *rect,
                    0.0,
                    egui::Stroke::new(1.0, palette.selection),
                    egui::StrokeKind::Outside,
                );
                painter.rect_filled(*rect, 0.0, palette.selection.linear_multiply(0.08));
            }
            SelectionOverlay::Lasso(points) if points.len() >= 2 => {
                painter.add(egui::Shape::line(
                    points.clone(),
                    egui::Stroke::new(1.0, palette.selection),
                ));
            }
            SelectionOverlay::Lasso(_) => {}
        }
    }

    if options.show_selection_bounds
        && let Some(union) = selected_world_bounds(doc, &selection.selected_shapes)
    {
        let bounds = egui::Rect::from_two_pos(
            view.world_to_screen(stencilcraft_geom::Point::new(union.x0, union.y0)),
            view.world_to_screen(stencilcraft_geom::Point::new(union.x1, union.y1)),
        );
        painter.rect_stroke(
            bounds.expand(3.0),
            0.0,
            egui::Stroke::new(1.0, palette.selection),
            egui::StrokeKind::Outside,
        );
    }
}

/// Compute the union axis-aligned bounding box in millimetres of `ids` inside `doc`.
pub fn selected_world_bounds(doc: &stencilcraft_doc::Document, ids: &[u64]) -> Option<kurbo::Rect> {
    let mut iter = ids
        .iter()
        .filter_map(|id| doc.get_shape(*id).and_then(stencilcraft_doc::Shape::bounds));
    let first = iter.next()?;
    Some(iter.fold(first, |acc, r| acc.union(r)))
}

fn paint_registration_marks(
    painter: &egui::Painter,
    view: &View,
    doc: &stencilcraft_doc::Document,
    palette: Palette,
) {
    let mark_mm = 6.0_f64;
    let inset_mm = 4.0_f64;
    let w = doc.board.width_mm;
    let h = doc.board.height_mm;
    if w < 20.0 || h < 20.0 {
        return;
    }
    let mark_color = match palette.board {
        c if c.r() > 140 => egui::Color32::from_gray(24),
        _ => egui::Color32::from_gray(230),
    };
    let tl_min = view.world_to_screen(stencilcraft_geom::Point::new(inset_mm, inset_mm));
    let tl_max = view.world_to_screen(stencilcraft_geom::Point::new(
        inset_mm + mark_mm,
        inset_mm + mark_mm,
    ));
    painter.rect_filled(egui::Rect::from_min_max(tl_min, tl_max), 0.5, mark_color);

    let tr_h = egui::Rect::from_min_max(
        view.world_to_screen(stencilcraft_geom::Point::new(
            w - inset_mm - mark_mm,
            inset_mm,
        )),
        view.world_to_screen(stencilcraft_geom::Point::new(w - inset_mm, inset_mm + 1.5)),
    );
    let tr_v = egui::Rect::from_min_max(
        view.world_to_screen(stencilcraft_geom::Point::new(w - inset_mm - 1.5, inset_mm)),
        view.world_to_screen(stencilcraft_geom::Point::new(
            w - inset_mm,
            inset_mm + mark_mm,
        )),
    );
    painter.rect_filled(tr_h, 0.0, mark_color);
    painter.rect_filled(tr_v, 0.0, mark_color);

    let bl_h = egui::Rect::from_min_max(
        view.world_to_screen(stencilcraft_geom::Point::new(inset_mm, h - inset_mm - 1.5)),
        view.world_to_screen(stencilcraft_geom::Point::new(
            inset_mm + mark_mm,
            h - inset_mm,
        )),
    );
    let bl_v = egui::Rect::from_min_max(
        view.world_to_screen(stencilcraft_geom::Point::new(
            inset_mm,
            h - inset_mm - mark_mm,
        )),
        view.world_to_screen(stencilcraft_geom::Point::new(inset_mm + 1.5, h - inset_mm)),
    );
    painter.rect_filled(bl_h, 0.0, mark_color);
    painter.rect_filled(bl_v, 0.0, mark_color);
}

/// Convert a [`kurbo::BezPath`] into editable [`craft_draw::ui::pen::EditSubpath`]s for
/// `BezierOverlay` (Direct Selection & Convert Anchor Point tools).
pub fn bezpath_to_edit_subpaths(path: &kurbo::BezPath) -> Vec<craft_draw::ui::pen::EditSubpath> {
    use craft_draw::ui::pen::{EditSubpath, Knot};
    let mut subpaths: Vec<EditSubpath> = Vec::new();
    let mut current: Option<EditSubpath> = None;

    for el in path.elements() {
        match *el {
            kurbo::PathEl::MoveTo(p) => {
                if let Some(mut sub) = current.take()
                    && !sub.knots.is_empty()
                {
                    finalize_subpath_smoothness(&mut sub);
                    subpaths.push(sub);
                }
                current = Some(EditSubpath {
                    knots: vec![Knot::corner(p)],
                    closed: false,
                });
            }
            kurbo::PathEl::LineTo(p) => {
                let sub = current.get_or_insert_with(|| EditSubpath {
                    knots: vec![Knot::corner(p)],
                    closed: false,
                });
                sub.knots.push(Knot::corner(p));
            }
            kurbo::PathEl::QuadTo(c, p) => {
                let sub = current.get_or_insert_with(|| EditSubpath {
                    knots: vec![Knot::corner(p)],
                    closed: false,
                });
                let p0 = sub.knots.last().map_or(p, |k| k.anchor);
                let c1 = p0 + (2.0 / 3.0) * (c - p0);
                let c2 = p + (2.0 / 3.0) * (c - p);
                if let Some(prev) = sub.knots.last_mut() {
                    prev.out_ctrl = c1;
                }
                sub.knots.push(Knot {
                    anchor: p,
                    in_ctrl: c2,
                    out_ctrl: p,
                    smooth: false,
                });
            }
            kurbo::PathEl::CurveTo(c1, c2, p) => {
                let sub = current.get_or_insert_with(|| EditSubpath {
                    knots: vec![Knot::corner(p)],
                    closed: false,
                });
                if let Some(prev) = sub.knots.last_mut() {
                    prev.out_ctrl = c1;
                }
                sub.knots.push(Knot {
                    anchor: p,
                    in_ctrl: c2,
                    out_ctrl: p,
                    smooth: false,
                });
            }
            kurbo::PathEl::ClosePath => {
                if let Some(mut sub) = current.take() {
                    sub.closed = true;
                    if sub.knots.len() > 1 {
                        let first_anchor = sub.knots[0].anchor;
                        let last_matches = sub
                            .knots
                            .last()
                            .is_some_and(|last| last.anchor.distance(first_anchor) < 1e-6);
                        if last_matches
                            && let Some(last) = sub.knots.pop()
                            && let Some(first) = sub.knots.first_mut()
                        {
                            first.in_ctrl = last.in_ctrl;
                        }
                    }
                    finalize_subpath_smoothness(&mut sub);
                    subpaths.push(sub);
                }
            }
        }
    }
    if let Some(mut sub) = current.take()
        && !sub.knots.is_empty()
    {
        finalize_subpath_smoothness(&mut sub);
        subpaths.push(sub);
    }
    subpaths
}

fn finalize_subpath_smoothness(sub: &mut craft_draw::ui::pen::EditSubpath) {
    for knot in &mut sub.knots {
        let v_in = knot.anchor - knot.in_ctrl;
        let v_out = knot.out_ctrl - knot.anchor;
        let len_in = v_in.hypot();
        let len_out = v_out.hypot();
        if len_in > 1e-6 && len_out > 1e-6 {
            let cos = v_in.dot(v_out) / (len_in * len_out);
            knot.smooth = cos > 0.998;
        }
    }
}

/// Convert [`craft_draw::ui::pen::EditSubpath`]s back into a [`kurbo::BezPath`], preserving
/// straight segments as `LineTo` and curves as `CurveTo`.
pub fn edit_subpaths_to_bezpath(subpaths: &[craft_draw::ui::pen::EditSubpath]) -> kurbo::BezPath {
    let mut out = kurbo::BezPath::new();
    for sub in subpaths {
        let Some(first) = sub.knots.first() else {
            continue;
        };
        out.move_to(first.anchor);
        let seg_count = sub.segment_count();
        for i in 0..seg_count {
            let Some((a, b)) = sub.segment_ends(i) else {
                continue;
            };
            if sub.is_straight(i) {
                if !(sub.closed && i + 1 == seg_count) {
                    out.line_to(sub.knots[b].anchor);
                }
            } else {
                out.curve_to(
                    sub.knots[a].out_ctrl,
                    sub.knots[b].in_ctrl,
                    sub.knots[b].anchor,
                );
            }
        }
        if sub.closed {
            out.close_path();
        }
    }
    out
}

/// Apply a [`craft_draw::ui::pen::PathEdit`] gesture to a subpath slice.
pub fn apply_path_edit(
    base: &[craft_draw::ui::pen::EditSubpath],
    edit: &craft_draw::ui::pen::PathEdit,
) -> Vec<craft_draw::ui::pen::EditSubpath> {
    use craft_draw::ui::pen::{HandleSide, Knot, PathEdit};
    use kurbo::ParamCurve as _;
    let mut subpaths = base.to_vec();
    match edit {
        PathEdit::MoveAnchors { anchors, by, .. } => {
            for &r in anchors {
                if let Some(k) = subpaths
                    .get_mut(r.subpath)
                    .and_then(|s| s.knots.get_mut(r.knot))
                {
                    k.anchor += *by;
                    k.in_ctrl += *by;
                    k.out_ctrl += *by;
                }
            }
        }
        &PathEdit::MoveHandle {
            knot,
            side,
            to,
            alone,
            ..
        } => {
            if let Some(k) = subpaths
                .get_mut(knot.subpath)
                .and_then(|s| s.knots.get_mut(knot.knot))
            {
                match side {
                    HandleSide::In => k.in_ctrl = to,
                    HandleSide::Out => k.out_ctrl = to,
                }
                if alone {
                    k.smooth = false;
                } else if k.smooth {
                    let v = to - k.anchor;
                    let len = v.hypot();
                    if len > 1e-9 {
                        let other = match side {
                            HandleSide::In => &mut k.out_ctrl,
                            HandleSide::Out => &mut k.in_ctrl,
                        };
                        let other_len = (*other - k.anchor).hypot();
                        *other = k.anchor - v * (other_len / len);
                    }
                }
            }
        }
        &PathEdit::BendSegment { segment, t, by, .. } => {
            if let Some(sub) = subpaths.get_mut(segment.subpath)
                && let Some((a, b)) = sub.segment_ends(segment.knot)
                && let Some(c) = sub.segment(segment.knot)
            {
                if sub.is_straight(segment.knot) {
                    sub.knots[a].out_ctrl = c.eval(1.0 / 3.0);
                    sub.knots[b].in_ctrl = c.eval(2.0 / 3.0);
                    sub.knots[a].smooth = false;
                    sub.knots[b].smooth = false;
                }
                let u = 1.0 - t;
                let w1 = 3.0 * u * u * t;
                let w2 = 3.0 * u * t * t;
                let denom = w1 * w1 + w2 * w2;
                if denom > 1e-9 {
                    sub.knots[a].out_ctrl += by * (w1 / denom);
                    sub.knots[b].in_ctrl += by * (w2 / denom);
                }
            }
        }
        &PathEdit::Convert { knot, out, .. } => {
            if let Some(k) = subpaths
                .get_mut(knot.subpath)
                .and_then(|s| s.knots.get_mut(knot.knot))
            {
                *k = match out {
                    Some(out) => Knot::smooth(k.anchor, out),
                    None => Knot::corner(k.anchor),
                };
            }
        }
    }
    subpaths
}

/// Split segment `seg` at parameter `t` (`0.0..=1.0`) by inserting a smooth cubic knot.
pub fn insert_path_knot(
    subpaths: &mut [craft_draw::ui::pen::EditSubpath],
    seg: craft_draw::ui::pen::KnotRef,
    t: f64,
) -> Option<craft_draw::ui::pen::KnotRef> {
    use craft_draw::ui::pen::{Knot, KnotRef};
    use kurbo::ParamCurve as _;
    let sub = subpaths.get_mut(seg.subpath)?;
    let (a, b) = sub.segment_ends(seg.knot)?;
    let c = sub.segment(seg.knot)?;
    let left = c.subsegment(0.0..t);
    let right = c.subsegment(t..1.0);
    sub.knots[a].out_ctrl = left.p1;
    sub.knots[b].in_ctrl = right.p2;
    let at = a + 1;
    sub.knots.insert(
        at,
        Knot {
            anchor: left.p3,
            in_ctrl: left.p2,
            out_ctrl: right.p1,
            smooth: true,
        },
    );
    Some(KnotRef::new(seg.subpath, at))
}

/// Remove the specified knots from `subpaths`, dropping any subpath that becomes empty.
pub fn delete_path_knots(
    subpaths: &mut Vec<craft_draw::ui::pen::EditSubpath>,
    knots: &[craft_draw::ui::pen::KnotRef],
) {
    for (si, sub) in subpaths.iter_mut().enumerate() {
        let mut indices: Vec<usize> = knots
            .iter()
            .filter(|r| r.subpath == si)
            .map(|r| r.knot)
            .collect();
        indices.sort_unstable();
        indices.dedup();
        for i in indices.into_iter().rev() {
            if i < sub.knots.len() {
                sub.knots.remove(i);
            }
        }
        if sub.knots.len() < 2 {
            sub.closed = false;
        }
    }
    subpaths.retain(|s| !s.knots.is_empty());
}

/// Selection state painted over the canvas.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectionState {
    pub selected_shapes: Vec<u64>,
    pub overlay: Option<SelectionOverlay>,
}

/// A visible selection gesture painted over the canvas.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionOverlay {
    Box(egui::Rect),
    Lasso(Vec<egui::Pos2>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Contain,
    Intersect,
}

/// Select visible shapes whose geometry intersects the screen-space rectangle or
/// lies completely inside it. Box/lasso gestures use these same testable primitives.
pub fn select_shapes_in_rect(
    doc: &stencilcraft_doc::Document,
    view: &View,
    rect: egui::Rect,
    mode: SelectionMode,
) -> Vec<u64> {
    if !rect.is_finite() || !view.zoom.is_finite() || view.zoom <= 0.0 {
        return Vec::new();
    }
    doc.layers
        .iter()
        .filter(|layer| layer.visible)
        .flat_map(|layer| layer.shapes.iter())
        .filter(|shape| {
            let Some(bounds) = shape.bounds() else {
                return false;
            };
            let bounds = egui::Rect::from_two_pos(
                view.world_to_screen(stencilcraft_geom::Point::new(bounds.x0, bounds.y0)),
                view.world_to_screen(stencilcraft_geom::Point::new(bounds.x1, bounds.y1)),
            );
            match mode {
                SelectionMode::Contain => rect.contains_rect(bounds),
                SelectionMode::Intersect => rect.intersects(bounds),
            }
        })
        .map(|shape| shape.id)
        .collect()
}

/// The even-odd point-in-lasso test in screen coordinates.
pub fn point_in_lasso(point: egui::Pos2, polygon: &[egui::Pos2]) -> bool {
    if polygon.len() < 3 || !point.x.is_finite() || !point.y.is_finite() {
        return false;
    }
    let mut inside = false;
    let mut previous = polygon.len() - 1;

    for current in 0..polygon.len() {
        let a = polygon[current];
        let b = polygon[previous];
        if !a.x.is_finite() || !a.y.is_finite() || !b.x.is_finite() || !b.y.is_finite() {
            return false;
        }
        if (a.y > point.y) != (b.y > point.y)
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

/// Select shape bounds by lasso containment or intersection with the polygon's
/// bounding rectangle (conservative intersection avoids missing any crossed shape).
pub fn select_shapes_in_lasso(
    doc: &stencilcraft_doc::Document,
    view: &View,
    polygon: &[egui::Pos2],
    mode: SelectionMode,
) -> Vec<u64> {
    if polygon.len() < 3 || polygon.len() > 10_000 {
        return Vec::new();
    }
    let Some(first) = polygon.first() else {
        return Vec::new();
    };
    if polygon
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Vec::new();
    }
    let bounds =
        polygon
            .iter()
            .skip(1)
            .fold(egui::Rect::from_min_max(*first, *first), |rect, point| {
                egui::Rect::from_min_max(
                    egui::pos2(rect.min.x.min(point.x), rect.min.y.min(point.y)),
                    egui::pos2(rect.max.x.max(point.x), rect.max.y.max(point.y)),
                )
            });
    let candidates = select_shapes_in_rect(doc, view, bounds, SelectionMode::Intersect);
    candidates
        .into_iter()
        .filter(|id| {
            let Some(shape) = doc.get_shape(*id) else {
                return false;
            };
            let Some(shape_bounds) = shape.bounds() else {
                return false;
            };
            let corners = [
                stencilcraft_geom::Point::new(shape_bounds.x0, shape_bounds.y0),
                stencilcraft_geom::Point::new(shape_bounds.x1, shape_bounds.y0),
                stencilcraft_geom::Point::new(shape_bounds.x1, shape_bounds.y1),
                stencilcraft_geom::Point::new(shape_bounds.x0, shape_bounds.y1),
            ];
            let screen_corners = corners.map(|point| view.world_to_screen(point));
            match mode {
                SelectionMode::Contain => screen_corners
                    .iter()
                    .all(|point| point_in_lasso(*point, polygon)),
                SelectionMode::Intersect => {
                    screen_corners
                        .iter()
                        .any(|point| point_in_lasso(*point, polygon))
                        || polygon.iter().any(|point| {
                            egui::Rect::from_min_max(screen_corners[0], screen_corners[2])
                                .contains(*point)
                        })
                        || polygon.windows(2).any(|segment| {
                            rects_intersect_segment(
                                egui::Rect::from_min_max(screen_corners[0], screen_corners[2]),
                                segment[0],
                                segment[1],
                            )
                        })
                }
            }
        })
        .collect()
}

fn rects_intersect_segment(rect: egui::Rect, start: egui::Pos2, end: egui::Pos2) -> bool {
    if rect.contains(start) || rect.contains(end) {
        return true;
    }
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    (0..4).any(|index| segments_intersect(start, end, corners[index], corners[(index + 1) % 4]))
}

fn segments_intersect(a: egui::Pos2, b: egui::Pos2, c: egui::Pos2, d: egui::Pos2) -> bool {
    let orient = |p: egui::Pos2, q: egui::Pos2, r: egui::Pos2| {
        (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)
    };
    let (o1, o2, o3, o4) = (
        orient(a, b, c),
        orient(a, b, d),
        orient(c, d, a),
        orient(c, d, b),
    );
    o1 * o2 <= 0.0 && o3 * o4 <= 0.0
}

/// Find the topmost visible shape whose flattened outline is within the screen-space
/// tolerance of `pointer`. Returns its stable document id.
pub fn hit_test_shape(
    doc: &stencilcraft_doc::Document,
    view: &View,
    pointer: egui::Pos2,
    tolerance_px: f32,
) -> Option<u64> {
    if !pointer.x.is_finite() || !pointer.y.is_finite() || !tolerance_px.is_finite() {
        return None;
    }
    let tolerance_sq = tolerance_px.max(0.0).powi(2);
    for layer in doc.layers.iter().rev().filter(|layer| layer.visible) {
        for shape in layer.shapes.iter().rev() {
            let polylines = stencilcraft_geom::path_to_polylines(
                &shape.path,
                stencilcraft_geom::DEFAULT_FLATTEN_STEPS,
            );
            if polylines.iter().any(|polyline| {
                let mut segments = polyline.points.windows(2).map(|pair| (pair[0], pair[1]));
                let hit_open = segments.any(|(start, end)| {
                    let start = view.world_to_screen(start);
                    let end = view.world_to_screen(end);
                    distance_to_segment_squared(pointer, start, end) <= tolerance_sq
                });
                let hit_close = polyline.closed
                    && polyline
                        .points
                        .first()
                        .zip(polyline.points.last())
                        .is_some_and(|(first, last)| {
                            let first = view.world_to_screen(*first);
                            let last = view.world_to_screen(*last);
                            distance_to_segment_squared(pointer, last, first) <= tolerance_sq
                        });
                hit_open || hit_close
            }) {
                return Some(shape.id);
            }
        }
    }
    None
}

fn distance_to_segment_squared(point: egui::Pos2, start: egui::Pos2, end: egui::Pos2) -> f32 {
    let segment = end - start;
    let length_sq = segment.length_sq();
    let fraction = if length_sq <= f32::EPSILON {
        0.0
    } else {
        ((point - start).dot(segment) / length_sq).clamp(0.0, 1.0)
    };
    point.distance_sq(start + fraction * segment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_testkit::simple_square;

    #[test]
    fn palettes_keep_the_design_legible_in_both_themes() {
        let dark = Palette::for_theme(ColorTheme::Dark);
        let light = Palette::for_theme(ColorTheme::Light);
        assert_eq!(dark.canvas_bg, Tokens::CANVAS_BG);
        assert_eq!(dark.board, Tokens::BOARD);
        assert_ne!(light.canvas_bg, dark.canvas_bg);
        assert_ne!(light.board, dark.board);
        // Cut/kerf darken for paper-white boards; selection blue is shared.
        assert_eq!(dark.cut_line, Tokens::CUT_LINE);
        assert_eq!(light.cut_line, Tokens::ACCENT_LIGHT);
        assert_eq!(dark.accent, Tokens::ACCENT);
        assert_eq!(light.accent, Tokens::ACCENT_LIGHT);
        assert_eq!(dark.selection, Tokens::SELECTION);
        assert_eq!(light.selection, Tokens::SELECTION);
    }

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
    fn center_extent_places_its_center_at_the_viewport_center_without_changing_zoom() {
        let mut view = View {
            zoom: 2.5,
            pan_mm: egui::Vec2::ZERO,
        };
        let viewport = egui::Rect::from_min_size(egui::pos2(100.0, 60.0), egui::vec2(800.0, 500.0));
        assert!(view.center_extent(600.0, 300.0, viewport));
        let center = view.world_to_screen(stencilcraft_geom::Point::new(300.0, 150.0));
        assert_eq!(center, viewport.center());
        assert_eq!(view.zoom, 2.5);
    }

    #[test]
    fn fit_extent_contains_the_mat_with_a_margin_and_centers_it() {
        let mut view = View::default();
        let viewport = egui::Rect::from_min_size(egui::pos2(30.0, 20.0), egui::vec2(1000.0, 700.0));
        assert!(view.fit_extent(600.0, 300.0, viewport, 40.0));
        let rect = view.mat_rect_screen(&MatBackdrop {
            width_mm: 600.0,
            height_mm: 300.0,
            label: "mat".into(),
        });
        assert!(rect.left() >= viewport.left() + 39.9);
        assert!(rect.right() <= viewport.right() - 39.9);
        assert!(rect.top() >= viewport.top() + 39.9);
        assert!(rect.bottom() <= viewport.bottom() - 39.9);
        assert_eq!(rect.center(), viewport.center());
    }

    #[test]
    fn view_operations_ignore_invalid_geometry_and_zoom_inputs() {
        let mut view = View::default();
        let original = view;
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
        assert!(!view.center_extent(f64::NAN, 20.0, viewport));
        assert!(!view.fit_extent(20.0, 20.0, viewport, f32::NAN));
        view.zoom_at(f32::NAN, viewport.center());
        view.zoom_at(2.0, egui::pos2(f32::NAN, 0.0));
        assert_eq!(view, original);
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
    fn mat_rect_and_ticks_track_dimensions() {
        let view = View {
            zoom: 4.0,
            pan_mm: egui::Vec2::ZERO,
        };
        // 600 mm × 600 mm Pro mat in mm, labelled for display.
        let backdrop = MatBackdrop {
            width_mm: 600.0,
            height_mm: 600.0,
            label: "24×24 mat".into(),
        };
        let rect = view.mat_rect_screen(&backdrop);
        assert_eq!(rect.width(), 2400.0, "600 mm at 4 px/mm");
        assert_eq!(rect.height(), 2400.0);
        // Full-inch ticks are bold, half-inch ticks faint.
        let palette = Palette::for_theme(ColorTheme::Dark);
        let (inch_len, inch_col) = tick_style(2.0, palette);
        let (half_len, half_col) = tick_style(1.0, palette);
        assert_eq!(inch_len, 8.0);
        assert_eq!(inch_col, Tokens::RULER);
        assert_eq!(half_len, 4.0);
        assert_ne!(inch_col, half_col);
    }

    #[test]
    fn box_selection_supports_containment_and_intersection_modes() {
        let doc = simple_square();
        let view = View {
            zoom: 2.0,
            pan_mm: egui::Vec2::ZERO,
        };
        let contained = egui::Rect::from_min_max(egui::pos2(15.0, 15.0), egui::pos2(125.0, 125.0));
        let crossing = egui::Rect::from_min_max(egui::pos2(15.0, 15.0), egui::pos2(50.0, 50.0));
        assert_eq!(
            select_shapes_in_rect(&doc, &view, contained, SelectionMode::Contain),
            vec![1]
        );
        assert_eq!(
            select_shapes_in_rect(&doc, &view, crossing, SelectionMode::Contain),
            Vec::<u64>::new()
        );
        assert_eq!(
            select_shapes_in_rect(&doc, &view, crossing, SelectionMode::Intersect),
            vec![1]
        );
    }

    #[test]
    fn lasso_selection_uses_polygon_not_only_bounds_for_containment() {
        let doc = simple_square();
        let view = View {
            zoom: 2.0,
            pan_mm: egui::Vec2::ZERO,
        };
        let lasso = [
            egui::pos2(0.0, 0.0),
            egui::pos2(140.0, 0.0),
            egui::pos2(140.0, 140.0),
            egui::pos2(0.0, 140.0),
        ];
        assert!(point_in_lasso(egui::pos2(80.0, 80.0), &lasso));
        assert_eq!(
            select_shapes_in_lasso(&doc, &view, &lasso, SelectionMode::Contain),
            vec![1]
        );
        assert!(
            select_shapes_in_lasso(&doc, &view, &lasso[..2], SelectionMode::Contain).is_empty()
        );
    }

    #[test]
    fn hit_test_selects_topmost_visible_outline_in_screen_space() {
        let mut doc = simple_square();
        let first = doc.layers[0].shapes[0].clone();
        doc.push_shape(0, first.path, first.style).unwrap();
        let view = View {
            zoom: 2.0,
            pan_mm: egui::Vec2::ZERO,
        };
        let edge = view.world_to_screen(stencilcraft_geom::Point::new(10.0, 35.0));
        let interior = view.world_to_screen(stencilcraft_geom::Point::new(35.0, 35.0));
        assert_eq!(hit_test_shape(&doc, &view, edge, 3.0), Some(2));
        assert_eq!(hit_test_shape(&doc, &view, interior, 3.0), None);
        assert_eq!(hit_test_shape(&doc, &view, edge, f32::NAN), None);
        doc.layers[0].visible = false;
        assert_eq!(hit_test_shape(&doc, &view, edge, 3.0), None);
    }

    #[test]
    fn kerf_preview_changes_visuals() {
        let doc = simple_square();
        let shape = &doc.layers[0].shapes[0];
        let view = View::default();
        let palette = Palette::for_theme(ColorTheme::Dark);
        let (plain_col, _) = shape_visual(shape, &view, false, palette);
        let (kerf_col, _) = shape_visual(shape, &view, true, palette);
        assert_ne!(plain_col, kerf_col);
    }

    #[test]
    fn canvas_view_and_bezpath_edit_subpath_roundtrip() {
        let mut view = View {
            zoom: 3.5,
            pan_mm: egui::vec2(12.0, -8.0),
        };
        let rect = egui::Rect::from_min_size(egui::pos2(20.0, 40.0), egui::vec2(600.0, 400.0));
        let cv = view.to_canvas_view(rect);
        let p = stencilcraft_geom::Point::new(42.0, 58.0);
        let s1 = view.world_to_screen(p);
        let s2 = cv.to_screen(p);
        assert!((s1.x - s2.x).abs() < 1e-4);
        assert!((s1.y - s2.y).abs() < 1e-4);
        view.from_canvas_view(&cv);
        assert!((view.zoom - 3.5).abs() < 1e-5);

        let doc = simple_square();
        let orig = &doc.layers[0].shapes[0].path;
        let mut subs = bezpath_to_edit_subpaths(orig);
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].knots.len(), 4);
        assert!(subs[0].closed);
        let inserted = insert_path_knot(&mut subs, craft_draw::ui::pen::KnotRef::new(0, 0), 0.5);
        assert!(inserted.is_some());
        assert_eq!(subs[0].knots.len(), 5);
        let rebuilt = edit_subpaths_to_bezpath(&subs);
        assert!(!rebuilt.elements().is_empty());
    }
}
