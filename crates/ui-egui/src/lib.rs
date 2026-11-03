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

/// StencilCraft's suite theme: VectorCraft's Modern slate greys with the coral accent, as a
/// `craft-design-theme` file the design system resolves like any built-in.
const THEME_JSON: &[u8] = include_bytes!("stencilcraft-theme.json");

/// The design-system theme base StencilCraft installs each frame (`craft_ui::theme::install`),
/// in `theme`'s mode at the Compact size. Falls back to the suite default if the bundled theme
/// file ever fails to parse.
pub fn theme_base(theme: ColorTheme) -> craft_ui::theme::Base {
    use craft_theme::{
        AppDefaults, ModeChoice, Size, SuiteChoice, SystemAppearance, ThemeId, ThemePair,
    };
    let own = craft_theme::format::parse(THEME_JSON)
        .ok()
        .map(|file| file.theme);
    let id = own
        .as_ref()
        .map_or_else(ThemeId::suite_default, |t| t.id.clone());
    // Dark chrome is the suite's built-in Pro dark (charcoal #323232 panels, blue accent), the
    // reference cutting-software look; light keeps StencilCraft Modern.
    let dark = ThemeId::parse("craft.pro").unwrap_or_else(|_| id.clone());
    let defaults = AppDefaults {
        theme: ThemePair { light: id, dark },
        mode: match theme {
            ColorTheme::Dark => ModeChoice::Dark,
            ColorTheme::Light => ModeChoice::Light,
        },
        size: Size::Compact,
        sizes: Default::default(),
    };
    craft_ui::theme::Base::resolve(
        &SuiteChoice::default().with_defaults(&defaults),
        SystemAppearance::default(),
        &defaults,
        None,
        &mut |wanted| {
            own.clone().filter(|t| t.id == *wanted).or_else(|| {
                craft_theme::builtins()
                    .iter()
                    .find(|t| t.id == *wanted)
                    .cloned()
            })
        },
    )
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
    /// Thin outline around the design page.
    pub page_border: egui::Color32,
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
                engrave_line: egui::Color32::from_rgb(0x58, 0x60, 0x6e),
                text: egui::Color32::from_rgb(0x3a, 0x3a, 0x3a),
                accent: Tokens::ACCENT,
                selection: Tokens::SELECTION,
                page_border: Tokens::PAGE_BORDER,
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
                page_border: Tokens::PAGE_BORDER,
            },
        }
    }
}

impl Tokens {
    /// Dark-theme canvas hues, after the reference cutting-software dark look: mid-grey
    /// pasteboard around charcoal craft `pro` chrome, a light-grey mat and a white page, so the
    /// design reads as it will on real media.
    pub const CANVAS_BG: egui::Color32 = egui::Color32::from_rgb(0x64, 0x64, 0x64);
    pub const MAT: egui::Color32 = egui::Color32::from_rgb(0xe0, 0xe0, 0xe0);
    pub const MAT_GRID: egui::Color32 = egui::Color32::from_rgb(0xc8, 0xc8, 0xc8);
    pub const RULER: egui::Color32 = egui::Color32::from_rgb(0x80, 0x80, 0x80);
    pub const GRID: egui::Color32 = egui::Color32::from_rgb(0xe8, 0xe8, 0xe8);
    pub const BOARD: egui::Color32 = egui::Color32::WHITE;
    /// Page outline: craft `pro` accent blue.
    pub const PAGE_BORDER: egui::Color32 = egui::Color32::from_rgb(0x37, 0x8e, 0xf0);
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

/// The canvas view: a zoom, a pan and a quarter-turn rotation, in screen pixels versus
/// world millimetres. Screen = (turn(world) − pan) × zoom, where `turn` rotates the page
/// clockwise on screen by `quarter_turns` × 90° (Silhouette's View › Rotate Page). The
/// rotation is view state only; the document never turns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub zoom: f32,
    pub pan_mm: egui::Vec2,
    /// 0–3 clockwise quarter turns; other values are taken modulo 4.
    pub quarter_turns: u8,
}

impl Default for View {
    fn default() -> Self {
        // 4 px per mm fits the default 100 × 150 board inside a mid-size window.
        Self {
            zoom: 4.0,
            pan_mm: egui::Vec2::ZERO,
            quarter_turns: 0,
        }
    }
}

impl View {
    /// Rotate a world vector into view space (clockwise on a y-down screen).
    fn turn(&self, x: f64, y: f64) -> (f64, f64) {
        match self.quarter_turns % 4 {
            0 => (x, y),
            1 => (-y, x),
            2 => (-x, -y),
            _ => (y, -x),
        }
    }

    /// The inverse of [`turn`](Self::turn).
    fn unturn(&self, x: f64, y: f64) -> (f64, f64) {
        match self.quarter_turns % 4 {
            0 => (x, y),
            1 => (y, -x),
            2 => (-x, -y),
            _ => (-y, x),
        }
    }

    pub fn world_to_screen(&self, world: stencilcraft_geom::Point) -> egui::Pos2 {
        let (x, y) = self.turn(world.x, world.y);
        egui::pos2(
            ((x - f64::from(self.pan_mm.x)) * f64::from(self.zoom)) as f32,
            ((y - f64::from(self.pan_mm.y)) * f64::from(self.zoom)) as f32,
        )
    }

    /// Screen → world, used for pointer-driven pan/zoom.
    pub fn screen_to_world(&self, screen: egui::Vec2) -> stencilcraft_geom::Point {
        let (x, y) = self.unturn(
            f64::from(screen.x) / f64::from(self.zoom) + f64::from(self.pan_mm.x),
            f64::from(screen.y) / f64::from(self.zoom) + f64::from(self.pan_mm.y),
        );
        stencilcraft_geom::Point::new(x, y)
    }

    /// A screen-space drag delta as a world-space move in millimetres.
    pub fn screen_delta_to_world(&self, delta: egui::Vec2) -> stencilcraft_geom::Vec2 {
        let (x, y) = self.unturn(
            f64::from(delta.x) / f64::from(self.zoom),
            f64::from(delta.y) / f64::from(self.zoom),
        );
        stencilcraft_geom::Vec2::new(x, y)
    }

    /// Put world point `world` at screen position `screen` at the current zoom.
    fn place(&mut self, world: stencilcraft_geom::Point, screen: egui::Pos2) -> bool {
        let (x, y) = self.turn(world.x, world.y);
        let pan = egui::vec2(
            (x - f64::from(screen.x / self.zoom)) as f32,
            (y - f64::from(screen.y / self.zoom)) as f32,
        );
        if !pan.is_finite() {
            return false;
        }
        self.pan_mm = pan;
        true
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
        let world_before = self.screen_to_world(anchor_screen.to_vec2());
        self.zoom = (self.zoom * factor).clamp(0.05, 400.0);
        self.place(world_before, anchor_screen);
    }

    /// Turn the view to `quarter_turns` × 90° keeping the world point at the viewport centre.
    pub fn set_rotation(&mut self, quarter_turns: u8, viewport: egui::Rect) {
        let center = self.screen_to_world(viewport.center().to_vec2());
        self.quarter_turns = quarter_turns % 4;
        if viewport.is_finite() {
            self.place(center, viewport.center());
        }
    }

    /// Center a world-space extent in `viewport` while preserving the current zoom.
    pub fn center_extent(&mut self, width_mm: f64, height_mm: f64, viewport: egui::Rect) -> bool {
        self.center_rect(kurbo::Rect::new(0.0, 0.0, width_mm, height_mm), viewport)
    }

    /// Center world rectangle `rect` in `viewport` while preserving the current zoom.
    pub fn center_rect(&mut self, rect: kurbo::Rect, viewport: egui::Rect) -> bool {
        if !valid_view_extent(rect.width(), rect.height(), viewport)
            || !rect.x0.is_finite()
            || !rect.y0.is_finite()
            || !self.zoom.is_finite()
            || self.zoom <= 0.0
        {
            return false;
        }
        self.place(rect.center(), viewport.center())
    }

    /// Fit a world-space extent inside `viewport`, leaving `margin` points on each side.
    pub fn fit_extent(
        &mut self,
        width_mm: f64,
        height_mm: f64,
        viewport: egui::Rect,
        margin: f32,
    ) -> bool {
        self.fit_rect(
            kurbo::Rect::new(0.0, 0.0, width_mm, height_mm),
            viewport,
            margin,
        )
    }

    /// Fit world rectangle `rect` inside `viewport` (Zoom to Selection), leaving `margin`
    /// points on each side. A horizontal or vertical line fits along its length.
    pub fn fit_rect(&mut self, rect: kurbo::Rect, viewport: egui::Rect, margin: f32) -> bool {
        if !rect.x0.is_finite()
            || !rect.y0.is_finite()
            || rect.width() <= 0.0 && rect.height() <= 0.0
        {
            return false;
        }
        // Give a line a hair of thickness so it still frames along its length.
        let hair = rect.width().max(rect.height()) * 1e-3;
        let rect = kurbo::Rect::new(
            rect.x0,
            rect.y0,
            rect.x1.max(rect.x0 + hair),
            rect.y1.max(rect.y0 + hair),
        );
        if !valid_view_extent(rect.width(), rect.height(), viewport)
            || !margin.is_finite()
            || margin < 0.0
            || viewport.width() <= margin * 2.0
            || viewport.height() <= margin * 2.0
        {
            return false;
        }
        let (w, h) = if self.quarter_turns % 2 == 1 {
            (rect.height(), rect.width())
        } else {
            (rect.width(), rect.height())
        };
        let available_width = f64::from(viewport.width() - margin * 2.0);
        let available_height = f64::from(viewport.height() - margin * 2.0);
        self.zoom = (available_width / w)
            .min(available_height / h)
            .clamp(0.05, 400.0) as f32;
        self.center_rect(rect, viewport)
    }

    /// World rectangle (x0, y0)–(x1, y1) on screen, whatever the rotation.
    fn rect_screen(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> egui::Rect {
        egui::Rect::from_two_pos(
            self.world_to_screen(stencilcraft_geom::Point::new(x0, y0)),
            self.world_to_screen(stencilcraft_geom::Point::new(x1, y1)),
        )
    }

    /// The document's artboard corners on screen.
    pub fn board_rect_screen(&self, doc: &stencilcraft_doc::Document) -> egui::Rect {
        self.rect_screen(0.0, 0.0, doc.board.width_mm, doc.board.height_mm)
    }

    /// The cutting mat's corners on screen (mat origin = sheet origin: media
    /// loads left-aligned on Cameo 4-series machines).
    pub fn mat_rect_screen(&self, backdrop: &MatBackdrop) -> egui::Rect {
        self.rect_screen(0.0, 0.0, backdrop.width_mm, backdrop.height_mm)
    }

    /// The whole mat on screen: the cutting area plus its rim.
    pub fn mat_frame_rect_screen(&self, backdrop: &MatBackdrop) -> egui::Rect {
        self.rect_screen(
            -MAT_RIM_SIDE_MM,
            -MAT_RIM_TOP_MM,
            backdrop.width_mm + MAT_RIM_SIDE_MM,
            backdrop.height_mm + MAT_RIM_SIDE_MM,
        )
    }

    /// Convert to [`craft_ui::canvas::CanvasView`] for `Rulers`, `BezierOverlay`, `PenDraft`,
    /// `TransformBox`, `CanvasScrollbars`, and `MarqueeDrag`.
    pub fn to_canvas_view(&self, viewport: egui::Rect) -> craft_ui::canvas::CanvasView {
        let center_screen = viewport.center();
        let center_world = self.screen_to_world(egui::vec2(center_screen.x, center_screen.y));
        craft_ui::canvas::CanvasView {
            rotation: 90.0 * f64::from(self.quarter_turns % 4),
            ..craft_ui::canvas::CanvasView::new(
                viewport,
                center_world,
                f64::from(self.zoom.max(0.05)),
            )
        }
    }

    /// Update `zoom` and `pan_mm` from a [`craft_ui::canvas::CanvasView`] (its rotation is
    /// ignored: the view keeps its own quarter turns).
    pub fn from_canvas_view(&mut self, cv: &craft_ui::canvas::CanvasView) {
        if !cv.zoom.is_finite()
            || cv.zoom <= 0.0
            || !cv.center.x.is_finite()
            || !cv.center.y.is_finite()
        {
            return;
        }
        self.zoom = (cv.zoom as f32).clamp(0.05, 400.0);
        self.place(cv.center, cv.rect.center());
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
    let frame = view.mat_frame_rect_screen(backdrop);
    painter.rect_filled(
        frame,
        (MAT_RIM_SIDE_MM as f32 * view.zoom).min(12.0),
        palette.mat,
    );
    // The arrow marks the edge fed into the machine. It lives in the top rim,
    // outside the document, and scales with the mat as the canvas is zoomed.
    let (head, stem) = mat_feed_arrow(view, backdrop);
    painter.add(egui::Shape::convex_polygon(
        head.to_vec(),
        palette.ruler,
        egui::Stroke::NONE,
    ));
    painter.rect_filled(stem, 0.0, palette.ruler);

    let seg = |x0: f64, y0: f64, x1: f64, y1: f64, color| {
        painter.line_segment(
            [
                view.world_to_screen(stencilcraft_geom::Point::new(x0, y0)),
                view.world_to_screen(stencilcraft_geom::Point::new(x1, y1)),
            ],
            (1.0, color),
        );
    };
    let (w, h) = (backdrop.width_mm, backdrop.height_mm);
    let inch_px = (25.4 * view.zoom as f64) as f32;
    if inch_px > 10.0 {
        // Inch grid over the mat surface.
        let mut i = 1.0_f64;
        while i * 25.4 < w {
            seg(i * 25.4, 0.0, i * 25.4, h, palette.mat_grid);
            i += 1.0;
        }
        let mut j = 1.0_f64;
        while j * 25.4 < h {
            seg(0.0, j * 25.4, w, j * 25.4, palette.mat_grid);
            j += 1.0;
        }
    }
    if inch_px > 4.0 {
        // Half-inch ruler ticks on the top and left mat edges, full-inch
        // ticks darker and longer (lengths in screen points).
        let px_mm = 1.0 / f64::from(view.zoom.max(0.05));
        let mut k = 1.0_f64;
        while k * 12.7 < w {
            let (len, color) = tick_style(k, palette);
            seg(k * 12.7, 0.0, k * 12.7, f64::from(len) * px_mm, color);
            k += 1.0;
        }
        let mut m = 1.0_f64;
        while m * 12.7 < h {
            let (len, color) = tick_style(m, palette);
            seg(0.0, m * 12.7, f64::from(len) * px_mm, m * 12.7, color);
            m += 1.0;
        }
    }
    let label_at = view.world_to_screen(stencilcraft_geom::Point::new(0.0, h));
    let toward = rect.center() - label_at;
    let inset = egui::vec2(4.0_f32.copysign(toward.x), 4.0_f32.copysign(toward.y));
    painter.text(
        label_at + inset,
        egui::Align2([
            if toward.x >= 0.0 {
                egui::Align::Min
            } else {
                egui::Align::Max
            },
            if toward.y >= 0.0 {
                egui::Align::Min
            } else {
                egui::Align::Max
            },
        ]),
        &backdrop.label,
        egui::FontId::proportional(10.0),
        palette.text,
    );
}

/// The mat's rim around its cutting area; the top rim is deeper.
const MAT_RIM_SIDE_MM: f64 = 5.0;
const MAT_RIM_TOP_MM: f64 = 10.0;

fn mat_feed_arrow(view: &View, backdrop: &MatBackdrop) -> ([egui::Pos2; 3], egui::Rect) {
    let center = backdrop.width_mm / 2.0;
    let point = |x, y| view.world_to_screen(stencilcraft_geom::Point::new(x, y));
    (
        [
            point(center, -8.0),
            point(center + 3.0, -4.5),
            point(center - 3.0, -4.5),
        ],
        egui::Rect::from_two_pos(point(center - 1.8, -4.5), point(center + 1.8, -2.5)),
    )
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
    let page = view.board_rect_screen(doc);
    painter.rect_filled(page, 0.0, palette.board);
    painter.rect_stroke(
        page,
        0.0,
        egui::Stroke::new(1.0, palette.page_border),
        egui::StrokeKind::Outside,
    );

    // Grid: the document's grid (10 mm by default) with its subdivisions once they are
    // far enough apart, drawn only over the board as hairlines.
    if options.show_grid {
        let spacing = doc.grid.spacing_mm;
        let pitch_mm = if f64::from(view.zoom) * doc.grid.step_mm() > 6.0 {
            doc.grid.step_mm()
        } else {
            spacing
        };
        if pitch_mm.is_finite() && pitch_mm > 0.0 && f64::from(view.zoom) * pitch_mm > 6.0 {
            let (w, h) = (doc.board.width_mm, doc.board.height_mm);
            let seg = |x0, y0, x1, y1| {
                painter.line_segment(
                    [
                        view.world_to_screen(stencilcraft_geom::Point::new(x0, y0)),
                        view.world_to_screen(stencilcraft_geom::Point::new(x1, y1)),
                    ],
                    (1.0, palette.grid),
                );
            };
            // At most 10 000 lines a side: a tiny pitch on a huge board stays bounded.
            let mut i = 0.0_f64;
            while i * pitch_mm <= w + 1e-9 && i < 10_000.0 {
                seg(i * pitch_mm, 0.0, i * pitch_mm, h);
                i += 1.0;
            }
            let mut j = 0.0_f64;
            while j * pitch_mm <= h + 1e-9 && j < 10_000.0 {
                seg(0.0, j * pitch_mm, w, j * pitch_mm);
                j += 1.0;
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
        for shape in layer.shapes.iter().filter(|shape| !shape.hidden) {
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
    painter.rect_filled(egui::Rect::from_two_pos(tl_min, tl_max), 0.5, mark_color);

    let tr_h = egui::Rect::from_two_pos(
        view.world_to_screen(stencilcraft_geom::Point::new(
            w - inset_mm - mark_mm,
            inset_mm,
        )),
        view.world_to_screen(stencilcraft_geom::Point::new(w - inset_mm, inset_mm + 1.5)),
    );
    let tr_v = egui::Rect::from_two_pos(
        view.world_to_screen(stencilcraft_geom::Point::new(w - inset_mm - 1.5, inset_mm)),
        view.world_to_screen(stencilcraft_geom::Point::new(
            w - inset_mm,
            inset_mm + mark_mm,
        )),
    );
    painter.rect_filled(tr_h, 0.0, mark_color);
    painter.rect_filled(tr_v, 0.0, mark_color);

    let bl_h = egui::Rect::from_two_pos(
        view.world_to_screen(stencilcraft_geom::Point::new(inset_mm, h - inset_mm - 1.5)),
        view.world_to_screen(stencilcraft_geom::Point::new(
            inset_mm + mark_mm,
            h - inset_mm,
        )),
    );
    let bl_v = egui::Rect::from_two_pos(
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

/// Apply a [`craft_draw::ui::pen::PathEdit`] gesture to a subpath slice, by craft-draw's own
/// edit rules; an edit naming a knot that does not exist leaves the subpaths unchanged.
pub fn apply_path_edit(
    base: &[craft_draw::ui::pen::EditSubpath],
    edit: &craft_draw::ui::pen::PathEdit,
) -> Vec<craft_draw::ui::pen::EditSubpath> {
    let mut path = craft_draw::ui::pen::EditPath {
        key: edit.path(),
        subpaths: base.to_vec(),
        color: None,
    };
    edit.apply(&mut path);
    path.subpaths
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
        .filter(|shape| is_selectable(shape))
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
                            egui::Rect::from_two_pos(screen_corners[0], screen_corners[2])
                                .contains(*point)
                        })
                        || polygon.windows(2).any(|segment| {
                            rects_intersect_segment(
                                egui::Rect::from_two_pos(screen_corners[0], screen_corners[2]),
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

/// Whether the canvas may pick `shape`: hidden and locked shapes are left out of clicks, boxes
/// and lassos (the layers panel still lists them, with eye and lock toggles).
pub fn is_selectable(shape: &stencilcraft_doc::Shape) -> bool {
    !shape.hidden && !shape.locked
}

/// Paint `paths` (millimetres) as solid hairlines in `color`: the live preview of an effect
/// (Offset) before it is applied.
pub fn paint_preview_paths(
    painter: &egui::Painter,
    view: &View,
    paths: &[kurbo::BezPath],
    color: egui::Color32,
) {
    let width = 1.5_f32.max(0.4 * view.zoom);
    for path in paths {
        for pl in
            stencilcraft_geom::path_to_polylines(path, stencilcraft_geom::DEFAULT_FLATTEN_STEPS)
        {
            let mut screen: Vec<egui::Pos2> =
                pl.points.iter().map(|p| view.world_to_screen(*p)).collect();
            if pl.closed
                && let Some(first) = screen.first().copied()
            {
                screen.push(first);
            }
            if screen.len() >= 2 {
                painter.add(egui::Shape::line(screen, (width, color)));
            }
        }
    }
}

/// The range the canvas scroll bars cover: the mat/board extent (`width_mm` × `height_mm`
/// from the origin) plus a margin of a tenth of its longer side, at least 20 mm. The bars
/// widen it to take in the current viewport themselves.
pub fn scroll_extent(width_mm: f64, height_mm: f64) -> kurbo::Rect {
    let (w, h) = (width_mm.max(0.0), height_mm.max(0.0));
    let margin = (w.max(h) * 0.1).max(20.0);
    kurbo::Rect::new(-margin, -margin, w + margin, h + margin)
}

/// The document's ruler guides as `craft_ui` canvas guides: key = index + 1, so the
/// engine's `guide.*` index is `key - 1`. `preview` (a guide being dragged: key and
/// place) moves or hides that guide until the drag commits.
pub fn canvas_guides(
    guides: &[stencilcraft_doc::Guide],
    preview: Option<(u64, craft_ui::canvas::GuidePlace)>,
) -> Vec<craft_ui::canvas::Guide> {
    use craft_ui::canvas::Orientation;
    guides
        .iter()
        .zip(1u64..)
        .filter_map(|(g, key)| {
            let orientation = match g.axis {
                stencilcraft_doc::GuideAxis::Horizontal => Orientation::Horizontal,
                stencilcraft_doc::GuideAxis::Vertical => Orientation::Vertical,
            };
            let (orientation, position) = match preview {
                Some((k, place)) if k == key => (place.orientation, place.position?),
                _ => (orientation, g.position_mm),
            };
            Some(craft_ui::canvas::Guide {
                locked: g.locked,
                ..craft_ui::canvas::Guide::new(key, orientation, position)
            })
        })
        .collect()
}

/// Paint smart guides: the alignment and spacing lines snapping found while dragging,
/// in world millimetres. A zero-length line is a snapped anchor point and gets a small
/// square marker. Headless-testable like [`paint_document`].
pub fn paint_snap_lines(
    painter: &egui::Painter,
    view: &View,
    lines: &[(stencilcraft_geom::Point, stencilcraft_geom::Point)],
    color: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.0, color);
    for (a, b) in lines {
        let (sa, sb) = (view.world_to_screen(*a), view.world_to_screen(*b));
        if !sa.is_finite() || !sb.is_finite() {
            continue;
        }
        if sa.distance(sb) < 0.5 {
            painter.rect_stroke(
                egui::Rect::from_center_size(sa, egui::vec2(7.0, 7.0)),
                0.0,
                stroke,
                egui::StrokeKind::Middle,
            );
        } else {
            painter.line_segment([sa, sb], stroke);
        }
    }
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
        for shape in layer
            .shapes
            .iter()
            .rev()
            .filter(|shape| is_selectable(shape))
        {
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
    fn bundled_theme_parses_and_resolves_in_both_modes() {
        assert!(craft_theme::format::parse(THEME_JSON).is_ok());
        for theme in [ColorTheme::Dark, ColorTheme::Light] {
            let base = theme_base(theme);
            assert_eq!(base.inputs.dark.id.as_str(), "craft.pro");
            assert_eq!(base.inputs.light.id.as_str(), "stencilcraft.modern");
        }
    }

    #[test]
    fn palettes_keep_the_design_legible_in_both_themes() {
        let dark = Palette::for_theme(ColorTheme::Dark);
        let light = Palette::for_theme(ColorTheme::Light);
        assert_eq!(dark.canvas_bg, Tokens::CANVAS_BG);
        assert_eq!(dark.board, Tokens::BOARD);
        assert_ne!(light.canvas_bg, dark.canvas_bg);
        assert_ne!(light.mat, dark.mat);
        // The page is paper-white in both themes, outlined in blue.
        assert_eq!(light.board, dark.board);
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
        let mut view = View {
            zoom: 3.0,
            pan_mm: egui::vec2(-40.0, 25.0),
            ..View::default()
        };
        let anchor = egui::pos2(133.0, 77.0);
        let before = view.screen_to_world(egui::vec2(anchor.x, anchor.y));
        view.zoom_at(2.0, anchor);
        let after = view.screen_to_world(egui::vec2(anchor.x, anchor.y));
        assert!((after.x - before.x).abs() < 1e-4, "{before:?} → {after:?}");
        assert!((after.y - before.y).abs() < 1e-4);
        // Repeated wheel steps in and back out return to the same view.
        for _ in 0..10 {
            view.zoom_at(1.1, anchor);
        }
        for _ in 0..10 {
            view.zoom_at(1.0 / 1.1, anchor);
        }
        let back = view.screen_to_world(egui::vec2(anchor.x, anchor.y));
        assert!((back.x - before.x).abs() < 1e-3 && (back.y - before.y).abs() < 1e-3);
        assert!((view.zoom - 6.0).abs() < 1e-3);
    }

    #[test]
    fn center_extent_places_its_center_at_the_viewport_center_without_changing_zoom() {
        let mut view = View {
            zoom: 2.5,
            pan_mm: egui::Vec2::ZERO,
            ..View::default()
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
            ..View::default()
        };
        let r = view.board_rect_screen(&doc);
        assert_eq!(r.width(), doc.board.width_mm as f32);
    }

    #[test]
    fn feed_arrow_points_up_and_stays_in_mat_top_rim() {
        let view = View {
            zoom: 2.0,
            pan_mm: egui::vec2(10.0, 20.0),
            ..View::default()
        };
        let backdrop = MatBackdrop {
            width_mm: 300.0,
            height_mm: 200.0,
            label: String::new(),
        };
        let frame = view.mat_frame_rect_screen(&backdrop);
        let mat = view.mat_rect_screen(&backdrop);
        let (head, stem) = mat_feed_arrow(&view, &backdrop);
        assert!(head.iter().all(|point| frame.contains(*point)));
        assert!(frame.contains_rect(stem));
        assert!(head[0].y < head[1].y);
        assert!(stem.bottom() < mat.top());
        assert!((head[0].x - mat.center().x).abs() < 0.01);
    }

    #[test]
    fn mat_rect_and_ticks_track_dimensions() {
        let view = View {
            zoom: 4.0,
            pan_mm: egui::Vec2::ZERO,
            ..View::default()
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
            ..View::default()
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
            ..View::default()
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
            ..View::default()
        };
        let edge = view.world_to_screen(stencilcraft_geom::Point::new(10.0, 35.0));
        let interior = view.world_to_screen(stencilcraft_geom::Point::new(35.0, 35.0));
        assert_eq!(hit_test_shape(&doc, &view, edge, 3.0), Some(2));
        assert_eq!(hit_test_shape(&doc, &view, interior, 3.0), None);
        assert_eq!(hit_test_shape(&doc, &view, edge, f32::NAN), None);
        // Locked and hidden shapes fall through to the one below, and box selection skips them.
        doc.layers[0].shapes[1].locked = true;
        assert_eq!(hit_test_shape(&doc, &view, edge, 3.0), Some(1));
        doc.layers[0].shapes[0].hidden = true;
        assert_eq!(hit_test_shape(&doc, &view, edge, 3.0), None);
        let all = egui::Rect::from_min_max(egui::pos2(-1e4, -1e4), egui::pos2(1e4, 1e4));
        assert!(select_shapes_in_rect(&doc, &view, all, SelectionMode::Intersect).is_empty());
        doc.layers[0].shapes[0].hidden = false;
        doc.layers[0].shapes[1].locked = false;
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
            ..View::default()
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

    /// Paint headlessly into a context pass and return the shapes it produced.
    fn painted(paint: impl Fn(&egui::Painter)) -> Vec<egui::Shape> {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput::default());
        paint(&ctx.layer_painter(egui::LayerId::background()));
        let mut output = ctx.end_pass();
        output.textures_delta.clear();
        output.shapes.into_iter().map(|c| c.shape).collect()
    }

    #[test]
    fn quarter_turns_round_trip_and_turn_clockwise() {
        for turns in 0..4u8 {
            let view = View {
                zoom: 2.0,
                pan_mm: egui::vec2(-30.0, 12.0),
                quarter_turns: turns,
            };
            let world = stencilcraft_geom::Point::new(17.0, -4.0);
            let back = view.screen_to_world(view.world_to_screen(world).to_vec2());
            assert!((back - world).hypot() < 1e-4, "{turns}: {back:?}");
        }
        // A quarter turn clockwise: the page's +x runs down the screen, +y runs left.
        let view = View {
            quarter_turns: 1,
            ..View::default()
        };
        let o = view.world_to_screen(stencilcraft_geom::Point::ZERO);
        let x = view.world_to_screen(stencilcraft_geom::Point::new(1.0, 0.0));
        let y = view.world_to_screen(stencilcraft_geom::Point::new(0.0, 1.0));
        assert!(x.y > o.y && (x.x - o.x).abs() < 1e-6);
        assert!(y.x < o.x && (y.y - o.y).abs() < 1e-6);
        // It matches craft-ui's view, which the rulers and overlays use.
        let viewport = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(400.0, 300.0));
        let cv = view.to_canvas_view(viewport);
        let p = stencilcraft_geom::Point::new(12.0, 34.0);
        assert!((cv.to_screen(p) - view.world_to_screen(p)).length() < 1e-3);
        // A turned drag maps back to the page's axes.
        let d = view.screen_delta_to_world(egui::vec2(0.0, 8.0));
        assert!((d.x - 2.0).abs() < 1e-9 && d.y.abs() < 1e-9);
    }

    #[test]
    fn rotating_keeps_the_centre_and_fit_swaps_the_sides() {
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 400.0));
        let mut view = View::default();
        assert!(view.fit_extent(200.0, 100.0, viewport, 0.0));
        let upright = view.zoom;
        let center = view.screen_to_world(viewport.center().to_vec2());
        view.set_rotation(1, viewport);
        let after = view.screen_to_world(viewport.center().to_vec2());
        assert!((after - center).hypot() < 1e-3);
        // The 200 × 100 sheet stands 200 tall on screen now: it fits at half the zoom.
        assert!(view.fit_extent(200.0, 100.0, viewport, 0.0));
        assert!((view.zoom - upright / 2.0).abs() < 1e-4);
        let board = view.mat_rect_screen(&MatBackdrop {
            width_mm: 200.0,
            height_mm: 100.0,
            label: String::new(),
        });
        assert!((board.height() - 400.0).abs() < 1e-2 && board.width() > 0.0);
        view.set_rotation(6, viewport);
        assert_eq!(view.quarter_turns, 2);
    }

    #[test]
    fn zoom_to_selection_frames_a_rect_and_a_line() {
        let viewport = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(500.0, 500.0));
        let mut view = View {
            pan_mm: egui::vec2(40.0, -10.0),
            ..View::default()
        };
        assert!(view.fit_rect(kurbo::Rect::new(50.0, 60.0, 70.0, 70.0), viewport, 50.0));
        assert!((view.zoom - 20.0).abs() < 1e-4);
        let c = view.world_to_screen(stencilcraft_geom::Point::new(60.0, 65.0));
        assert!((c - viewport.center()).length() < 1e-2);
        // A horizontal line fits along its length.
        assert!(view.fit_rect(kurbo::Rect::new(0.0, 5.0, 100.0, 5.0), viewport, 50.0));
        assert!((view.zoom - 4.0).abs() < 1e-4);
        // Nothing to frame, or NaN: the view stays.
        let before = view;
        assert!(!view.fit_rect(kurbo::Rect::new(3.0, 3.0, 3.0, 3.0), viewport, 0.0));
        assert!(!view.fit_rect(kurbo::Rect::new(f64::NAN, 0.0, 1.0, 1.0), viewport, 0.0));
        assert_eq!(view, before);
    }

    #[test]
    fn zoom_at_keeps_the_anchor_with_a_pan() {
        let mut view = View {
            pan_mm: egui::vec2(25.0, -40.0),
            ..View::default()
        };
        let anchor = egui::pos2(210.0, 90.0);
        let before = view.screen_to_world(anchor.to_vec2());
        view.zoom_at(1.7, anchor);
        let after = view.screen_to_world(anchor.to_vec2());
        assert!((after - before).hypot() < 1e-4, "{before:?} → {after:?}");
    }

    #[test]
    fn scroll_extent_pads_the_mat() {
        assert_eq!(
            scroll_extent(300.0, 600.0),
            kurbo::Rect::new(-60.0, -60.0, 360.0, 660.0)
        );
        assert_eq!(
            scroll_extent(50.0, 20.0),
            kurbo::Rect::new(-20.0, -20.0, 70.0, 40.0)
        );
        // Non-finite or negative sizes still give a usable range.
        let r = scroll_extent(-5.0, 0.0);
        assert!(r.width() > 0.0 && r.height() > 0.0);
    }

    #[test]
    fn canvas_guides_map_keys_locks_and_a_drag_preview() {
        use craft_ui::canvas::{GuidePlace, Orientation};
        let guides = [
            stencilcraft_doc::Guide {
                axis: stencilcraft_doc::GuideAxis::Vertical,
                position_mm: 10.0,
                locked: false,
            },
            stencilcraft_doc::Guide {
                axis: stencilcraft_doc::GuideAxis::Horizontal,
                position_mm: 20.0,
                locked: true,
            },
        ];
        let shown = canvas_guides(&guides, None);
        assert_eq!(shown.len(), 2);
        assert_eq!(
            (
                shown[0].key,
                shown[0].orientation,
                shown[0].position,
                shown[0].locked
            ),
            (1, Orientation::Vertical, 10.0, false)
        );
        assert_eq!((shown[1].key, shown[1].locked), (2, true));
        let moved = canvas_guides(
            &guides,
            Some((
                1,
                GuidePlace {
                    orientation: Orientation::Vertical,
                    position: Some(15.0),
                },
            )),
        );
        assert_eq!(moved[0].position, 15.0);
        let removed = canvas_guides(
            &guides,
            Some((
                1,
                GuidePlace {
                    orientation: Orientation::Vertical,
                    position: None,
                },
            )),
        );
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].key, 2);
    }

    #[test]
    fn snap_lines_paint_segments_and_point_markers() {
        let view = View::default();
        let lines = [
            (
                stencilcraft_geom::Point::new(0.0, 0.0),
                stencilcraft_geom::Point::new(0.0, 50.0),
            ),
            (
                stencilcraft_geom::Point::new(5.0, 5.0),
                stencilcraft_geom::Point::new(5.0, 5.0),
            ),
            (
                stencilcraft_geom::Point::new(f64::NAN, 0.0),
                stencilcraft_geom::Point::new(1.0, 0.0),
            ),
        ];
        let shapes = painted(|p| paint_snap_lines(p, &view, &lines, egui::Color32::RED));
        assert_eq!(shapes.len(), 2, "{shapes:?}");
        assert!(matches!(shapes[0], egui::Shape::LineSegment { .. }));
        let egui::Shape::Rect(marker) = &shapes[1] else {
            panic!("a point gets a square marker: {:?}", shapes[1]);
        };
        assert_eq!(marker.rect.center(), egui::pos2(20.0, 20.0));
    }

    #[test]
    fn a_turned_document_paints_its_board_turned() {
        let doc = stencilcraft_doc::Document::default();
        let view = View {
            zoom: 1.0,
            quarter_turns: 1,
            ..View::default()
        };
        let board = view.board_rect_screen(&doc);
        assert!((board.width() - doc.board.height_mm as f32).abs() < 1e-3);
        assert!((board.height() - doc.board.width_mm as f32).abs() < 1e-3);
        let clip =
            egui::Rect::from_min_size(egui::pos2(-500.0, -500.0), egui::vec2(1000.0, 1000.0));
        let shapes = painted(|p| {
            paint_document(
                p,
                clip,
                &doc,
                &view,
                None,
                CanvasOptions::default(),
                &SelectionState::default(),
            )
        });
        assert!(
            shapes
                .iter()
                .any(|s| matches!(s, egui::Shape::Rect(r) if r.rect == board))
        );
    }
}
