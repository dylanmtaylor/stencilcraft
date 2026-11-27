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

/// The runtime root StencilCraft hands the framework each frame (`FrameInput`): `theme`'s mode
/// at the Compact density. Dark chrome is the suite's built-in Pro dark (charcoal #323232
/// panels, blue accent), the reference cutting-software look; light is StencilCraft Modern, the
/// bundled library theme. Falls back to the suite default if the bundled theme file ever fails
/// to parse.
pub fn root_env(theme: ColorTheme) -> craft_ui::runtime::env::RootEnv {
    use craft_theme::{ModeChoice, Size, ThemeId};
    let root = craft_ui::runtime::env::RootEnv::new().density(Size::Compact);
    match theme {
        ColorTheme::Dark => {
            let pro = ThemeId::parse("craft.pro").unwrap_or_else(|_| ThemeId::suite_default());
            root.mode(ModeChoice::Dark).theme(pro)
        }
        ColorTheme::Light => {
            let root = root.mode(ModeChoice::Light);
            match craft_theme::format::parse(THEME_JSON) {
                Ok(file) => root.library_theme(std::sync::Arc::new(file.theme)),
                Err(_) => root,
            }
        }
    }
}

/// Theme tokens: every colour the frontend draws comes from here.
pub struct Tokens;

/// Registration mark layouts (Silhouette's three marks, or four corners).
pub use stencilcraft_layout::MarkStyle;

/// Silhouette's default registration marks: 0.787 in long, 0.020 in thick, 0.625 in in from
/// the page edge.
pub const DEFAULT_MARK_GEOMETRY: [f64; 3] = [0.787 * 25.4, 0.020 * 25.4, 0.625 * 25.4];

/// Registration mark rectangles `[x, y, width, height]` on a `width` × `height` page for
/// `geometry` (length, thickness, inset): Silhouette's square at the top left and brackets at
/// the top right and bottom left.
pub fn mark_rects(width: f64, height: f64, geometry: [f64; 3]) -> Vec<[f64; 4]> {
    styled_mark_rects(
        width,
        height,
        geometry,
        stencilcraft_layout::MarkStyle::Silhouette,
    )
}

/// [`mark_rects`] in `style`: Silhouette's three marks, or a bracket at every corner.
pub fn styled_mark_rects(
    width: f64,
    height: f64,
    geometry: [f64; 3],
    style: stencilcraft_layout::MarkStyle,
) -> Vec<[f64; 4]> {
    let [length, thickness, inset] = geometry;
    let page = stencilcraft_layout::PageOptions {
        width_mm: width,
        height_mm: height,
        marks: Some(style),
        mark_length_mm: length,
        mark_thickness_mm: thickness,
        mark_inset_mm: inset,
    };
    stencilcraft_layout::registration_marks(&page, style)
}

/// The unit lengths are shown in (Preferences › Units). The document is always millimetres;
/// only what the person reads and types changes.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum LengthUnit {
    #[default]
    Millimetres,
    Centimetres,
    Inches,
}

impl LengthUnit {
    pub const ALL: [LengthUnit; 3] = [
        LengthUnit::Millimetres,
        LengthUnit::Centimetres,
        LengthUnit::Inches,
    ];

    /// Units per millimetre.
    pub fn per_mm(self) -> f64 {
        match self {
            LengthUnit::Millimetres => 1.0,
            LengthUnit::Centimetres => 0.1,
            LengthUnit::Inches => 1.0 / 25.4,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            LengthUnit::Millimetres => "mm",
            LengthUnit::Centimetres => "cm",
            LengthUnit::Inches => "in",
        }
    }

    /// The unit around a number, for craft-ui fields (`{n} mm`).
    pub fn template(self) -> &'static str {
        match self {
            LengthUnit::Millimetres => "{n} mm",
            LengthUnit::Centimetres => "{n} cm",
            LengthUnit::Inches => "{n} in",
        }
    }

    /// Decimals shown for a length (Silhouette shows inches to three places).
    pub fn decimals(self) -> usize {
        match self {
            LengthUnit::Millimetres => 1,
            LengthUnit::Centimetres => 2,
            LengthUnit::Inches => 3,
        }
    }

    pub fn from_mm(self, mm: f64) -> f64 {
        mm * self.per_mm()
    }

    pub fn to_mm(self, value: f64) -> f64 {
        value / self.per_mm()
    }

    /// `mm` written in this unit with its symbol, `extra` decimals past the usual.
    pub fn format(self, mm: f64, extra: usize) -> String {
        format!(
            "{:.*} {}",
            self.decimals().saturating_add(extra),
            self.from_mm(mm),
            self.symbol()
        )
    }

    fn index(self) -> u8 {
        match self {
            LengthUnit::Millimetres => 0,
            LengthUnit::Centimetres => 1,
            LengthUnit::Inches => 2,
        }
    }
}

static LENGTH_UNIT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// The unit lengths are shown in now.
pub fn length_unit() -> LengthUnit {
    match LENGTH_UNIT.load(std::sync::atomic::Ordering::Relaxed) {
        1 => LengthUnit::Centimetres,
        2 => LengthUnit::Inches,
        _ => LengthUnit::Millimetres,
    }
}

/// Set the unit lengths are shown in (the app does this from its preferences each frame).
pub fn set_length_unit(unit: LengthUnit) {
    LENGTH_UNIT.store(unit.index(), std::sync::atomic::Ordering::Relaxed);
}

/// Semantic canvas colours and overlay flags for a specific workspace theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasOptions {
    pub kerf_preview: bool,
    pub show_grid: bool,
    pub print_cut_marks: bool,
    /// Registration mark length, thickness and inset from the page edge, millimetres.
    pub mark_geometry: [f64; 3],
    /// Four corner brackets instead of Silhouette's three marks.
    pub four_marks: bool,
    pub show_selection_bounds: bool,
    pub theme: ColorTheme,
    /// View › Outline View: every line a hairline, whatever its weight.
    pub outline_view: bool,
    /// View › Show Cut Border: the area the machine can cut, just inside the page.
    pub cut_border: bool,
    /// Preferences › Cut to Edge of Page: the cut border is the page's own edge.
    pub cut_to_page_edge: bool,
    /// View › Show Print Border: the area a printer can print, for Print & Cut.
    pub print_border: bool,
    /// View › Show Dimensions: the selection's width and height beside it.
    pub show_dimensions: bool,
    /// Page Setup's page colour (`#rrggbb`); the paper white when `None`.
    pub page_colour: Option<[u8; 3]>,
    /// Page Setup's page transparency, 0 (opaque) to 100 % (the mat shows through).
    pub page_transparency: f64,
    /// Grid line colours (Page Setup › Grid Settings): the spacing lines and the subdivision
    /// lines between them; `None` is the theme's grid colour.
    pub grid_primary: Option<[u8; 3]>,
    pub grid_secondary: Option<[u8; 3]>,
}

/// The page's fill: Page Setup's colour (else `board`) with its transparency.
pub fn page_fill(board: egui::Color32, options: &CanvasOptions) -> egui::Color32 {
    let [r, g, b] = options
        .page_colour
        .unwrap_or([board.r(), board.g(), board.b()]);
    let t = if options.page_transparency.is_finite() {
        options.page_transparency.clamp(0.0, 100.0)
    } else {
        0.0
    };
    // In 0..=255 after the clamp, so the cast is exact.
    let alpha = (255.0 * (1.0 - t / 100.0)).round() as u8;
    egui::Color32::from_rgba_unmultiplied(r, g, b, alpha)
}

impl Default for CanvasOptions {
    fn default() -> Self {
        Self {
            kerf_preview: false,
            show_grid: true,
            print_cut_marks: false,
            mark_geometry: DEFAULT_MARK_GEOMETRY,
            four_marks: false,
            show_selection_bounds: true,
            theme: ColorTheme::Dark,
            outline_view: false,
            cut_border: true,
            print_border: false,
            show_dimensions: true,
            page_colour: None,
            page_transparency: 0.0,
            grid_primary: None,
            grid_secondary: None,
            cut_to_page_edge: false,
        }
    }
}

/// How far inside the page Silhouette Studio draws its cut border (measured in SS 5.0.556:
/// 1.4–1.5 mm on a 12 × 12 in CAMEO page).
pub const CUT_BORDER_INSET_MM: f64 = 1.5;

/// How far inside the page the cut border runs: Silhouette's 1.5 mm, or none with Cut to Edge of
/// Page (as SS 5.0.556 draws it, 2026-11-26).
pub fn cut_border_inset(options: &CanvasOptions) -> f64 {
    if options.cut_to_page_edge {
        0.0
    } else {
        CUT_BORDER_INSET_MM
    }
}

/// The print border's inset: a typical printer's unprintable margin, a quarter inch.
pub const PRINT_BORDER_INSET_MM: f64 = 6.35;

/// The main board's rectangle inset by `inset` millimetres, as a closed screen polyline.
fn board_inset_screen(
    doc: &stencilcraft_doc::Document,
    view: &View,
    inset: f64,
) -> Option<Vec<egui::Pos2>> {
    let (w, h) = (doc.board.width_mm, doc.board.height_mm);
    if !(w > 2.0 * inset && h > 2.0 * inset) {
        return None;
    }
    let (x0, y0, x1, y1) = (inset, inset, w - inset, h - inset);
    Some(
        [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]
            .into_iter()
            .map(|(x, y)| view.world_to_screen(stencilcraft_geom::Point::new(x, y)))
            .collect(),
    )
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

    /// Convert to [`craft_behavior::CanvasView`] for `Rulers`, `BezierOverlay`, `PenDraft`,
    /// `TransformBox`, `CanvasScrollbars`, and `MarqueeDrag`.
    pub fn to_canvas_view(&self, viewport: egui::Rect) -> craft_behavior::CanvasView {
        let center_screen = viewport.center();
        let center_world = self.screen_to_world(egui::vec2(center_screen.x, center_screen.y));
        craft_behavior::CanvasView::new(viewport, center_world, f64::from(self.zoom.max(0.05)))
            .rotation(90.0 * f64::from(self.quarter_turns % 4))
    }

    /// Update `zoom` and `pan_mm` from a [`craft_behavior::CanvasView`] (its rotation is
    /// ignored: the view keeps its own quarter turns).
    pub fn from_canvas_view(&mut self, cv: &craft_behavior::CanvasView) {
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
/// The ink of a guide layer's lines.
pub const GUIDE_LAYER_INK: egui::Color32 = egui::Color32::from_rgb(140, 146, 158);

pub fn paint_document(
    painter: &egui::Painter,
    clip: egui::Rect,
    doc: &stencilcraft_doc::Document,
    view: &View,
    layers: CanvasLayers<'_>,
    options: CanvasOptions,
    selection: &SelectionState,
) {
    let CanvasLayers {
        backdrop,
        image_textures,
        pattern_textures,
    } = layers;
    let palette = Palette::for_theme(options.theme);
    // Canvas backing, then (optionally) the cutting mat, then the design
    // sheet. Overflow beyond the mat simply stays canvas — a real-world
    // situation (12×24 art on a 12×12 mat) is visible, not masked.
    painter.rect_filled(clip, 0.0, palette.canvas_bg);
    if let Some(backdrop) = backdrop {
        paint_mat(painter, view, backdrop, palette);
    }
    let page = view.board_rect_screen(doc);
    painter.rect_filled(page, 0.0, page_fill(palette.board, &options));
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
        let colour = |c: Option<[u8; 3]>| {
            c.map_or(palette.grid, |[r, g, b]| egui::Color32::from_rgb(r, g, b))
        };
        let (w, h) = (doc.board.width_mm, doc.board.height_mm);
        let lines = |pitch_mm: f64, colour: egui::Color32| {
            if !(pitch_mm.is_finite() && pitch_mm > 0.0 && f64::from(view.zoom) * pitch_mm > 6.0) {
                return;
            }
            let seg = |x0, y0, x1, y1| {
                painter.line_segment(
                    [
                        view.world_to_screen(stencilcraft_geom::Point::new(x0, y0)),
                        view.world_to_screen(stencilcraft_geom::Point::new(x1, y1)),
                    ],
                    (1.0, colour),
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
        };
        // Isometric: three families of lines (vertical, 30° and 150°) the same distance apart,
        // which meet as equilateral triangles.
        let iso = |pitch_mm: f64, colour: egui::Color32| {
            if !(pitch_mm.is_finite() && pitch_mm > 0.0 && f64::from(view.zoom) * pitch_mm > 6.0) {
                return;
            }
            for angle in [90.0_f64, 30.0, 150.0] {
                for (a, b) in isometric_lines(w, h, pitch_mm, angle) {
                    painter.line_segment(
                        [view.world_to_screen(a), view.world_to_screen(b)],
                        (1.0, colour),
                    );
                }
            }
        };
        // Subdivisions in the secondary colour once they are far enough apart, then the
        // spacing lines over them in the primary colour.
        let step = doc.grid.step_mm();
        if doc.grid.isometric {
            if step < spacing {
                iso(step, colour(options.grid_secondary));
            }
            iso(spacing, colour(options.grid_primary));
        } else {
            if step < spacing {
                lines(step, colour(options.grid_secondary));
            }
            lines(spacing, colour(options.grid_primary));
        }
    }

    if options.print_cut_marks {
        paint_registration_marks(
            painter,
            view,
            doc,
            palette,
            options.mark_geometry,
            options.four_marks,
        );
    }

    if options.cut_border
        && let Some(points) = board_inset_screen(doc, view, cut_border_inset(&options))
    {
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(1.0, palette.cut_line.linear_multiply(0.6)),
        ));
    }
    if options.print_border
        && let Some(points) = board_inset_screen(doc, view, PRINT_BORDER_INSET_MM)
    {
        painter.extend(egui::Shape::dashed_line(
            &points,
            egui::Stroke::new(1.0, palette.engrave_line),
            6.0,
            4.0,
        ));
    }

    // Placed images, under the cut lines, each mapped corner to corner so a turned view turns
    // them too.
    for image in &doc.images {
        let Some(texture) = image_textures.get(&image.id) else {
            continue;
        };
        paint_image_quad(painter, view, image, *texture);
    }

    // Shapes: flatten and convert each polyline to screen space.
    for layer in &doc.layers {
        if !layer.visible {
            continue;
        }
        for shape in layer.shapes.iter().filter(|shape| !shape.hidden) {
            let (mut color, mut width) = shape_visual(shape, view, options.kerf_preview, palette);
            if !options.kerf_preview
                && let Some(line) = shape
                    .production
                    .line_color
                    .as_deref()
                    .and_then(parse_hex_colour)
            {
                color = line;
            }
            let pattern = shape
                .production
                .fill_pattern
                .as_deref()
                .zip(pattern_textures.get(&shape.id).copied());
            let solid = shape
                .production
                .fill_color
                .as_deref()
                .and_then(parse_hex_colour);
            let gradient = shape
                .production
                .fill_gradient
                .as_deref()
                .and_then(|g| Some((g, parse_hex_colour(&g.from)?, parse_hex_colour(&g.to)?)));
            if !options.outline_view && (pattern.is_some() || gradient.is_some() || solid.is_some())
            {
                let polygons: Vec<Vec<egui::Pos2>> = stencilcraft_geom::path_to_polylines(
                    &shape.path,
                    stencilcraft_geom::DEFAULT_FLATTEN_STEPS,
                )
                .into_iter()
                .enumerate()
                .filter(|(i, pl)| pl.closed && !shape.production.hidden_contours.contains(i))
                .map(|(_, pl)| pl.points.iter().map(|p| view.world_to_screen(*p)).collect())
                .collect();
                if let Some((pattern, (texture, aspect))) = pattern {
                    let mut mesh = even_odd_fill(&polygons, clip, egui::Color32::WHITE);
                    map_pattern(&mut mesh, view, texture, pattern, aspect);
                    painter.add(egui::Shape::mesh(mesh));
                } else if let Some((g, from, to)) = gradient {
                    let mut mesh = even_odd_fill(&polygons, clip, egui::Color32::WHITE);
                    let bounds = kurbo::Shape::bounding_box(&shape.path);
                    shade_gradient(&mut mesh, view, bounds, g.angle_deg, from, to);
                    painter.add(egui::Shape::mesh(mesh));
                } else if let Some(fill) = solid {
                    painter.add(egui::Shape::mesh(even_odd_fill(&polygons, clip, fill)));
                }
            }
            if options.outline_view {
                width = 1.0;
            }
            // A guide layer (shown, never cut) draws in thin grey dashes, so a sizing
            // background never reads as a cut line.
            let guide = layer.template && !selection.selected_shapes.contains(&shape.id);
            if guide {
                color = GUIDE_LAYER_INK;
                width = 1.0;
            }
            if selection.selected_shapes.contains(&shape.id) {
                color = palette.selection;
                width = width.max(if options.outline_view { 1.5 } else { 2.5 });
            }
            for (index, pl) in stencilcraft_geom::path_to_polylines(
                &shape.path,
                stencilcraft_geom::DEFAULT_FLATTEN_STEPS,
            )
            .into_iter()
            .enumerate()
            {
                if pl.points.is_empty() {
                    continue;
                }
                // Contours hidden from cutting (Contour) are shown faintly.
                let (color, width) = if shape.production.hidden_contours.contains(&index) {
                    (color.linear_multiply(0.3), 1.0)
                } else {
                    (color, width)
                };
                let mut screen: Vec<egui::Pos2> =
                    pl.points.iter().map(|p| view.world_to_screen(*p)).collect();
                if pl.closed
                    && let Some(first) = pl.points.first()
                {
                    screen.push(view.world_to_screen(*first));
                }
                if guide {
                    painter.extend(egui::Shape::dashed_line(&screen, (width, color), 6.0, 4.0));
                } else {
                    painter.add(egui::Shape::line(screen, (width, color)));
                }
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
        if options.show_dimensions {
            paint_dimensions(painter, bounds.expand(3.0), union, palette);
        }
    }
}

/// A dimension callout's length: two decimals of a millimetre, else the unit's usual places
/// (Silhouette writes 1.250 in).
fn dimension(mm: f64) -> String {
    let unit = length_unit();
    let extra = usize::from(unit == LengthUnit::Millimetres);
    unit.format(mm, extra)
}

/// Silhouette's dimension callouts: the selection's width below it and its height to its left,
/// each with a measuring line.
fn paint_dimensions(
    painter: &egui::Painter,
    bounds: egui::Rect,
    world: kurbo::Rect,
    palette: Palette,
) {
    let stroke = egui::Stroke::new(1.0, palette.selection);
    let font = egui::FontId::proportional(11.0);
    let gap = 10.0;
    let y = bounds.max.y + gap;
    painter.line_segment(
        [egui::pos2(bounds.min.x, y), egui::pos2(bounds.max.x, y)],
        stroke,
    );
    for x in [bounds.min.x, bounds.max.x] {
        painter.line_segment([egui::pos2(x, y - 4.0), egui::pos2(x, y + 4.0)], stroke);
    }
    painter.text(
        egui::pos2(bounds.center().x, y + 3.0),
        egui::Align2::CENTER_TOP,
        dimension(world.width()),
        font.clone(),
        palette.selection,
    );
    let x = bounds.min.x - gap;
    painter.line_segment(
        [egui::pos2(x, bounds.min.y), egui::pos2(x, bounds.max.y)],
        stroke,
    );
    for y in [bounds.min.y, bounds.max.y] {
        painter.line_segment([egui::pos2(x - 4.0, y), egui::pos2(x + 4.0, y)], stroke);
    }
    painter.text(
        egui::pos2(x - 3.0, bounds.center().y),
        egui::Align2::RIGHT_CENTER,
        dimension(world.height()),
        font,
        palette.selection,
    );
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
    geometry: [f64; 3],
    four: bool,
) {
    let mark_color = match palette.board {
        c if c.r() > 140 => egui::Color32::from_gray(24),
        _ => egui::Color32::from_gray(230),
    };
    let style = if four {
        stencilcraft_layout::MarkStyle::FourCorner
    } else {
        stencilcraft_layout::MarkStyle::Silhouette
    };
    for [x, y, w, h] in styled_mark_rects(doc.board.width_mm, doc.board.height_mm, geometry, style)
    {
        if ![x, y, w, h].iter().all(|v| v.is_finite()) {
            continue;
        }
        let rect = egui::Rect::from_two_pos(
            view.world_to_screen(stencilcraft_geom::Point::new(x, y)),
            view.world_to_screen(stencilcraft_geom::Point::new(x + w, y + h)),
        );
        painter.rect_filled(rect, 0.0, mark_color);
    }
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
                current = Some(EditSubpath::new(vec![Knot::corner(p)], false));
            }
            kurbo::PathEl::LineTo(p) => {
                let sub =
                    current.get_or_insert_with(|| EditSubpath::new(vec![Knot::corner(p)], false));
                sub.knots.push(Knot::corner(p));
            }
            kurbo::PathEl::QuadTo(c, p) => {
                let sub =
                    current.get_or_insert_with(|| EditSubpath::new(vec![Knot::corner(p)], false));
                let p0 = sub.knots.last().map_or(p, |k| k.anchor);
                let c1 = p0 + (2.0 / 3.0) * (c - p0);
                let c2 = p + (2.0 / 3.0) * (c - p);
                if let Some(prev) = sub.knots.last_mut() {
                    prev.out_ctrl = c1;
                }
                sub.knots.push(Knot::new(p, c2, p, false));
            }
            kurbo::PathEl::CurveTo(c1, c2, p) => {
                let sub =
                    current.get_or_insert_with(|| EditSubpath::new(vec![Knot::corner(p)], false));
                if let Some(prev) = sub.knots.last_mut() {
                    prev.out_ctrl = c1;
                }
                sub.knots.push(Knot::new(p, c2, p, false));
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
    let mut path = craft_draw::ui::pen::EditPath::new(edit.path(), base.to_vec());
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
    if !t.is_finite() {
        return None;
    }
    let t = t.clamp(0.0, 1.0);
    let sub = subpaths.get_mut(seg.subpath)?;
    let (a, b) = sub.segment_ends(seg.knot)?;
    let c = sub.segment(seg.knot)?;
    let straight = c.p1 == c.p0 && c.p2 == c.p3;
    let knot = if straight {
        // A point added to a straight line keeps it straight: a corner, no handles anywhere.
        Knot::corner(c.eval(t))
    } else {
        let left = c.subsegment(0.0..t);
        let right = c.subsegment(t..1.0);
        sub.knots.get_mut(a)?.out_ctrl = left.p1;
        sub.knots.get_mut(b)?.in_ctrl = right.p2;
        Knot::new(left.p3, left.p2, right.p1, true)
    };
    let at = a + 1;
    if at > sub.knots.len() {
        return None;
    }
    sub.knots.insert(at, knot);
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

/// The ends of every open subpath with at least two knots (Silhouette Studio rejoins a path
/// when one of these is dragged onto another).
pub fn open_path_ends(
    subpaths: &[craft_draw::ui::pen::EditSubpath],
) -> Vec<craft_draw::ui::pen::KnotRef> {
    use craft_draw::ui::pen::KnotRef;
    let mut ends = Vec::new();
    for (si, sub) in subpaths.iter().enumerate() {
        if !sub.closed && sub.knots.len() >= 2 {
            ends.push(KnotRef::new(si, 0));
            ends.push(KnotRef::new(si, sub.knots.len() - 1));
        }
    }
    ends
}

/// The open end nearest `at` within `reach` (document units), other than `moved` itself.
pub fn open_end_near(
    subpaths: &[craft_draw::ui::pen::EditSubpath],
    moved: craft_draw::ui::pen::KnotRef,
    at: stencilcraft_geom::Point,
    reach: f64,
) -> Option<craft_draw::ui::pen::KnotRef> {
    open_path_ends(subpaths)
        .into_iter()
        .filter(|end| *end != moved)
        .filter_map(|end| {
            let knot = subpaths.get(end.subpath)?.knots.get(end.knot)?;
            let d = (knot.anchor - at).hypot();
            (d <= reach).then_some((end, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(end, _)| end)
}

fn reverse_subpath(sub: &mut craft_draw::ui::pen::EditSubpath) {
    sub.knots.reverse();
    for knot in &mut sub.knots {
        std::mem::swap(&mut knot.in_ctrl, &mut knot.out_ctrl);
    }
}

/// Join the open end `moved` onto the open end `target`, the two becoming one knot at
/// `target`'s anchor (keeping `moved`'s incoming handle, re-based). The ends of one subpath
/// close it (it needs three knots or more); ends of two subpaths join them into one. Returns
/// whether anything changed; anything else (not open ends, the same end) is left alone.
pub fn join_path_ends(
    subpaths: &mut Vec<craft_draw::ui::pen::EditSubpath>,
    moved: craft_draw::ui::pen::KnotRef,
    target: craft_draw::ui::pen::KnotRef,
) -> bool {
    let ends = open_path_ends(subpaths);
    if moved == target || !ends.contains(&moved) || !ends.contains(&target) {
        return false;
    }
    if moved.subpath == target.subpath {
        let Some(sub) = subpaths.get_mut(moved.subpath) else {
            return false;
        };
        if sub.knots.len() < 3 {
            return false;
        }
        // Make the moved end the last knot, then fold it into the first.
        if moved.knot == 0 {
            reverse_subpath(sub);
        }
        let (Some(last), Some(first)) = (sub.knots.pop(), sub.knots.first_mut()) else {
            return false;
        };
        first.in_ctrl = first.anchor + (last.in_ctrl - last.anchor);
        sub.closed = true;
        return true;
    }
    // Two subpaths: orient `moved`'s so it ends there and `target`'s so it starts there.
    let (Some(mut a), Some(mut b)) = (
        subpaths.get(moved.subpath).cloned(),
        subpaths.get(target.subpath).cloned(),
    ) else {
        return false;
    };
    if moved.knot == 0 {
        reverse_subpath(&mut a);
    }
    if target.knot != 0 {
        reverse_subpath(&mut b);
    }
    let Some(last) = a.knots.pop() else {
        return false;
    };
    if let Some(first) = b.knots.first_mut() {
        first.in_ctrl = first.anchor + (last.in_ctrl - last.anchor);
    }
    a.knots.extend(b.knots);
    let (keep, drop) = (moved.subpath, target.subpath);
    if let Some(slot) = subpaths.get_mut(keep) {
        *slot = a;
    }
    if drop < subpaths.len() {
        subpaths.remove(drop);
    }
    true
}

/// The corners of a placed image in millimetres, clockwise from the top left.
pub fn image_corners(image: &stencilcraft_doc::RasterImage) -> [stencilcraft_geom::Point; 4] {
    let (x0, y0) = (image.x_mm, image.y_mm);
    let (x1, y1) = (x0 + image.width_mm, y0 + image.height_mm);
    [
        stencilcraft_geom::Point::new(x0, y0),
        stencilcraft_geom::Point::new(x1, y0),
        stencilcraft_geom::Point::new(x1, y1),
        stencilcraft_geom::Point::new(x0, y1),
    ]
}

/// The topmost placed image under the screen point `at`, if any.
pub fn hit_test_image(
    doc: &stencilcraft_doc::Document,
    view: &View,
    at: egui::Pos2,
) -> Option<u64> {
    let p = view.screen_to_world(at.to_vec2());
    doc.images
        .iter()
        .rev()
        .find(|image| {
            p.x >= image.x_mm
                && p.y >= image.y_mm
                && p.x <= image.x_mm + image.width_mm
                && p.y <= image.y_mm + image.height_mm
        })
        .map(|image| image.id)
}

fn paint_image_quad(
    painter: &egui::Painter,
    view: &View,
    image: &stencilcraft_doc::RasterImage,
    texture: egui::TextureId,
) {
    paint_texture_quad(painter, view, image_corners(image), texture);
}

/// Paint `texture` stretched over the document quad `corners` (millimetres, clockwise from the
/// texture's top left), so it follows a turned view.
pub fn paint_texture_quad(
    painter: &egui::Painter,
    view: &View,
    corners: [stencilcraft_geom::Point; 4],
    texture: egui::TextureId,
) {
    let corners = corners.map(|p| view.world_to_screen(p));
    if corners.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return;
    }
    let uvs = [
        egui::pos2(0.0, 0.0),
        egui::pos2(1.0, 0.0),
        egui::pos2(1.0, 1.0),
        egui::pos2(0.0, 1.0),
    ];
    let mut mesh = egui::Mesh::with_texture(texture);
    for (pos, uv) in corners.into_iter().zip(uvs) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv,
            color: egui::Color32::WHITE,
        });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

/// A `#rrggbb` colour, if well formed.
pub fn parse_hex_colour(hex: &str) -> Option<egui::Color32> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| {
        hex.get(i..i + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
    };
    Some(egui::Color32::from_rgb(byte(0)?, byte(2)?, byte(4)?))
}

/// Most pixel rows one fill paints (a huge zoomed shape is filled over its visible part only).
const MAX_FILL_ROWS: usize = 4096;

/// Fill closed screen polygons by the even-odd rule, one pixel row at a time, clipped to
/// `clip`: concave shapes and holes fill correctly, which egui's convex fill cannot do.
/// Texture `mesh` (a fill in screen space) with a pattern tile repeated in page millimetres:
/// one tile is `tile_mm` wide and `aspect` times that tall, turned by the pattern's angle.
pub fn map_pattern(
    mesh: &mut egui::Mesh,
    view: &View,
    texture: egui::TextureId,
    pattern: &stencilcraft_doc::FillPattern,
    aspect: f32,
) {
    let width = pattern.tile_mm.max(0.5);
    let height = width * f64::from(aspect.max(0.01));
    let (sin, cos) = (-pattern.angle_deg.to_radians()).sin_cos();
    mesh.texture_id = texture;
    for vertex in &mut mesh.vertices {
        let world = view.screen_to_world(vertex.pos.to_vec2());
        let (x, y) = (world.x * cos - world.y * sin, world.x * sin + world.y * cos);
        vertex.uv = egui::pos2((x / width) as f32, (y / height) as f32);
        vertex.color = egui::Color32::WHITE;
    }
}

/// Where `point` falls along a gradient across `bounds` at `angle_deg` (0 left to right,
/// 90 bottom to top on the y-down page), from 0 to 1.
pub fn gradient_position(
    bounds: kurbo::Rect,
    angle_deg: f64,
    point: stencilcraft_geom::Point,
) -> f64 {
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    let centre = bounds.center();
    // Half the extent of the bounds along the direction.
    let half = (bounds.width() * cos.abs() + bounds.height() * sin.abs()) / 2.0;
    if !(half.is_finite() && half > 1e-9) {
        return 0.0;
    }
    let along = (point.x - centre.x) * cos - (point.y - centre.y) * sin;
    (along / (2.0 * half) + 0.5).clamp(0.0, 1.0)
}

/// Colour `mesh` (a fill in screen space) with a gradient from `from` to `to` across the
/// shape's `bounds` in page millimetres.
pub fn shade_gradient(
    mesh: &mut egui::Mesh,
    view: &View,
    bounds: kurbo::Rect,
    angle_deg: f64,
    from: egui::Color32,
    to: egui::Color32,
) {
    for vertex in &mut mesh.vertices {
        let world = view.screen_to_world(vertex.pos.to_vec2());
        let t = gradient_position(bounds, angle_deg, world) as f32;
        vertex.color = from.lerp_to_gamma(to, t);
    }
}

/// The lines at `angle_deg` from the x axis, `pitch_mm` apart (measured across them) and
/// passing through the origin, clipped to the `w` × `h` board; at most 10 000.
pub fn isometric_lines(
    w: f64,
    h: f64,
    pitch_mm: f64,
    angle_deg: f64,
) -> Vec<(stencilcraft_geom::Point, stencilcraft_geom::Point)> {
    let mut out = Vec::new();
    if !(w.is_finite()
        && h.is_finite()
        && w > 0.0
        && h > 0.0
        && pitch_mm.is_finite()
        && pitch_mm > 0.0)
    {
        return out;
    }
    let (s, c) = angle_deg.to_radians().sin_cos();
    let (ux, uy) = (c, s); // along the line
    let (nx, ny) = (-s, c); // across it
    let corners = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)];
    let across = corners.map(|(x, y)| x * nx + y * ny);
    let lo = across.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = across.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut k = (lo / pitch_mm).ceil();
    while k * pitch_mm <= hi + 1e-9 && out.len() < 10_000 {
        let d = k * pitch_mm;
        let (px, py) = (nx * d, ny * d);
        // Clip p + t·u to the board, slab by slab.
        let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
        let mut keep = true;
        for (p, u, max) in [(px, ux, w), (py, uy, h)] {
            if u.abs() < 1e-12 {
                if p < -1e-9 || p > max + 1e-9 {
                    keep = false;
                }
            } else {
                let (a, b) = ((0.0 - p) / u, (max - p) / u);
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
            }
        }
        if keep && t1 > t0 {
            out.push((
                stencilcraft_geom::Point::new(px + ux * t0, py + uy * t0),
                stencilcraft_geom::Point::new(px + ux * t1, py + uy * t1),
            ));
        }
        k += 1.0;
    }
    out
}

pub fn even_odd_fill(
    polygons: &[Vec<egui::Pos2>],
    clip: egui::Rect,
    colour: egui::Color32,
) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    let (mut y0, mut y1) = (f32::INFINITY, f32::NEG_INFINITY);
    for p in polygons.iter().flatten() {
        y0 = y0.min(p.y);
        y1 = y1.max(p.y);
    }
    let (y0, y1) = (y0.max(clip.min.y), y1.min(clip.max.y));
    if !(y0.is_finite() && y1.is_finite()) || y1 <= y0 {
        return mesh;
    }
    let mut crossings: Vec<f32> = Vec::new();
    let mut y = y0.floor() + 0.5;
    let mut rows = 0;
    while y < y1 && rows < MAX_FILL_ROWS {
        crossings.clear();
        for poly in polygons {
            let n = poly.len();
            for i in 0..n {
                let (Some(a), Some(b)) = (poly.get(i), poly.get((i + 1) % n)) else {
                    continue;
                };
                if (a.y <= y) != (b.y <= y) {
                    let t = (y - a.y) / (b.y - a.y);
                    crossings.push(a.x + t * (b.x - a.x));
                }
            }
        }
        crossings.sort_by(f32::total_cmp);
        for [a, b] in crossings.as_chunks::<2>().0 {
            let (x0, x1) = (a.max(clip.min.x), b.min(clip.max.x));
            if x1 > x0 {
                mesh.add_colored_rect(
                    egui::Rect::from_min_max(egui::pos2(x0, y - 0.5), egui::pos2(x1, y + 0.5)),
                    colour,
                );
            }
        }
        y += 1.0;
        rows += 1;
    }
    mesh
}

/// What the canvas paints under the design: the cutting mat and placed images' textures.
#[derive(Clone, Copy, Debug)]
pub struct CanvasLayers<'a> {
    pub backdrop: Option<&'a MatBackdrop>,
    pub image_textures: &'a std::collections::HashMap<u64, egui::TextureId>,
    /// Pattern fill tiles by shape id: a repeating texture and the tile's height ÷ width.
    pub pattern_textures: &'a std::collections::HashMap<u64, (egui::TextureId, f32)>,
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
                // Touching means the outline itself crosses or lies in the box, not just its
                // bounding box (a marquee in the empty corner of a diagonal line misses it).
                SelectionMode::Intersect => {
                    rect.intersects(bounds) && outline_touches_rect(&shape.path, view, rect)
                }
            }
        })
        .map(|shape| shape.id)
        .collect()
}

/// Whether any part of `path`'s outline, drawn with `view`, lies in or crosses the screen `rect`.
pub fn outline_touches_rect(path: &kurbo::BezPath, view: &View, rect: egui::Rect) -> bool {
    stencilcraft_geom::path_to_polylines(path, stencilcraft_geom::DEFAULT_FLATTEN_STEPS)
        .iter()
        .any(|pl| {
            let pts: Vec<egui::Pos2> = pl.points.iter().map(|p| view.world_to_screen(*p)).collect();
            let closing = pl
                .closed
                .then(|| pts.last().copied().zip(pts.first().copied()))
                .flatten();
            pts.iter().any(|p| rect.contains(*p))
                || pts
                    .windows(2)
                    .filter_map(|w| w.first().copied().zip(w.get(1).copied()))
                    .chain(closing)
                    .any(|(a, b)| segment_crosses_rect(a, b, rect))
        })
}

/// Liang–Barsky: whether the segment `a`→`b` passes through `rect`.
fn segment_crosses_rect(a: egui::Pos2, b: egui::Pos2, rect: egui::Rect) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
    for (p, q) in [
        (-d.x, a.x - rect.min.x),
        (d.x, rect.max.x - a.x),
        (-d.y, a.y - rect.min.y),
        (d.y, rect.max.y - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
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
/// engine's `guide.*` index is `key - 1`. `preview` (a guide being dragged: its key, the axis
/// it runs along and its position, `None` off the canvas) moves or hides that guide until the
/// drag commits.
pub fn canvas_guides(
    guides: &[stencilcraft_doc::Guide],
    preview: Option<GuidePreview>,
) -> Vec<craft_ui::canvas::Guide> {
    use craft_behavior::Axis;
    use craft_ui::canvas::GuideKey;
    let mut shown: Vec<craft_ui::canvas::Guide> = guides
        .iter()
        .zip(1u64..)
        .filter_map(|(g, key)| {
            let key = GuideKey::new(key);
            // A horizontal guide runs along x, at a y position.
            let along = match g.axis {
                stencilcraft_doc::GuideAxis::Horizontal => Axis::X,
                stencilcraft_doc::GuideAxis::Vertical => Axis::Y,
            };
            let (along, position) = match preview {
                Some(drag) if drag.key == key => (drag.along, drag.at?),
                _ => (along, g.position_mm),
            };
            Some(craft_ui::canvas::Guide {
                locked: g.locked,
                ..craft_ui::canvas::Guide::new(key, along, position)
            })
        })
        .collect();
    // A guide being dragged out of a ruler is not in the document yet: it shows under the
    // key the guides control gave it, and is added on release.
    if let Some(drag) = preview
        && let Some(at) = drag.at
        && !(1u64..)
            .take(guides.len())
            .any(|key| GuideKey::new(key) == drag.key)
    {
        shown.push(craft_ui::canvas::Guide::new(drag.key, drag.along, at));
    }
    shown
}

/// A document guide being dragged, as [`canvas_guides`] previews it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuidePreview {
    /// The dragged guide's key (its document index + 1).
    pub key: craft_ui::canvas::GuideKey,
    /// The axis it runs along.
    pub along: craft_behavior::Axis,
    /// Its position across that axis; `None` off the canvas, which removes it on release.
    pub at: Option<f64>,
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
mod fill_tests {
    use super::*;

    #[test]
    fn a_gradient_runs_across_the_bounds_at_its_angle() {
        let b = kurbo::Rect::new(0.0, 0.0, 20.0, 10.0);
        let p = stencilcraft_geom::Point::new;
        assert!((gradient_position(b, 0.0, p(0.0, 5.0))).abs() < 1e-9);
        assert!((gradient_position(b, 0.0, p(20.0, 5.0)) - 1.0).abs() < 1e-9);
        assert!((gradient_position(b, 0.0, p(10.0, 0.0)) - 0.5).abs() < 1e-9);
        // The page is y-down: 90° ends at the top edge.
        assert!((gradient_position(b, 90.0, p(3.0, 0.0)) - 1.0).abs() < 1e-9);
        assert!(gradient_position(b, 90.0, p(3.0, 10.0)).abs() < 1e-9);
        assert_eq!(gradient_position(kurbo::Rect::ZERO, 0.0, p(1.0, 1.0)), 0.0);
    }

    #[test]
    fn a_pattern_repeats_in_page_millimetres() {
        let view = View::default();
        let mut mesh = egui::Mesh::default();
        mesh.add_colored_rect(
            egui::Rect::from_min_max(
                view.world_to_screen(stencilcraft_geom::Point::new(0.0, 0.0)),
                view.world_to_screen(stencilcraft_geom::Point::new(20.0, 10.0)),
            ),
            egui::Color32::RED,
        );
        let pattern = stencilcraft_doc::FillPattern {
            png_base64: "x".into(),
            tile_mm: 10.0,
            angle_deg: 0.0,
        };
        map_pattern(&mut mesh, &view, egui::TextureId::User(7), &pattern, 0.5);
        assert_eq!(mesh.texture_id, egui::TextureId::User(7));
        let far = mesh
            .vertices
            .iter()
            .map(|v| v.uv)
            .fold(egui::pos2(f32::MIN, f32::MIN), |a, b| {
                egui::pos2(a.x.max(b.x), a.y.max(b.y))
            });
        // 20 mm across is two 10 mm tiles; 10 mm down is two 5 mm tall tiles.
        assert!(
            (far.x - 2.0).abs() < 1e-3 && (far.y - 2.0).abs() < 1e-3,
            "{far:?}"
        );
        assert!(
            mesh.vertices
                .iter()
                .all(|v| v.color == egui::Color32::WHITE)
        );
    }

    #[test]
    fn even_odd_fill_leaves_holes_and_respects_clip() {
        let square = |a: f32, b: f32| {
            vec![
                egui::pos2(a, a),
                egui::pos2(b, a),
                egui::pos2(b, b),
                egui::pos2(a, b),
            ]
        };
        let clip = egui::Rect::from_min_max(egui::pos2(-100.0, -100.0), egui::pos2(100.0, 100.0));
        let mesh = even_odd_fill(
            &[square(0.0, 10.0), square(3.0, 7.0)],
            clip,
            egui::Color32::RED,
        );
        // Rows through the hole have two spans; the others one: 10 rows, 4 with a hole.
        assert_eq!(mesh.vertices.len() / 4, 6 + 4 * 2);
        let tiny = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(5.0, 2.0));
        let clipped = even_odd_fill(&[square(0.0, 10.0)], tiny, egui::Color32::RED);
        assert_eq!(clipped.vertices.len() / 4, 2);
        assert!(clipped.vertices.iter().all(|v| v.pos.x <= 5.0));
        assert!(even_odd_fill(&[], clip, egui::Color32::RED).is_empty());
        assert_eq!(
            parse_hex_colour("#ff8000"),
            Some(egui::Color32::from_rgb(255, 128, 0))
        );
        assert_eq!(parse_hex_colour("ff8000"), None);
        assert_eq!(parse_hex_colour("#ff80"), None);
        assert_eq!(parse_hex_colour("#éé8000"), None);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn length_units_convert_and_format() {
        use super::LengthUnit;
        assert_eq!(LengthUnit::Inches.format(25.4, 0), "1.000 in");
        assert_eq!(LengthUnit::Centimetres.format(12.0, 0), "1.20 cm");
        assert_eq!(LengthUnit::Millimetres.format(12.04, 1), "12.04 mm");
        for unit in LengthUnit::ALL {
            assert!((unit.to_mm(unit.from_mm(33.3)) - 33.3).abs() < 1e-9);
        }
    }

    #[test]
    fn a_marquee_touches_an_outline_only_where_the_outline_is() {
        let view = View {
            zoom: 1.0,
            ..View::default()
        };
        let mut diagonal = kurbo::BezPath::new();
        diagonal.move_to((0.0, 0.0));
        diagonal.line_to((100.0, 100.0));
        // The empty corner of its bounding box: the box overlaps, the line does not.
        let corner = egui::Rect::from_min_max(egui::pos2(70.0, 0.0), egui::pos2(100.0, 20.0));
        assert!(!outline_touches_rect(&diagonal, &view, corner));
        // Across the middle of the line, with no vertex inside the box.
        let across = egui::Rect::from_min_max(egui::pos2(40.0, 45.0), egui::pos2(60.0, 55.0));
        assert!(outline_touches_rect(&diagonal, &view, across));
    }

    use super::*;
    use stencilcraft_testkit::simple_square;

    #[test]
    fn adding_a_point_to_a_straight_line_keeps_it_straight() {
        use craft_draw::ui::pen::{EditSubpath, Knot, KnotRef};
        use stencilcraft_geom::Point;
        let mut subs = vec![EditSubpath::new(
            vec![
                Knot::corner(Point::new(0.0, 0.0)),
                Knot::corner(Point::new(10.0, 0.0)),
                Knot::corner(Point::new(10.0, 10.0)),
            ],
            true,
        )];
        let new = insert_path_knot(&mut subs, KnotRef::new(0, 0), 0.5).expect("inserted");
        assert_eq!(new, KnotRef::new(0, 1));
        let knots = &subs[0].knots;
        assert_eq!(knots.len(), 4);
        assert_eq!(knots[1], Knot::corner(Point::new(5.0, 0.0)));
        assert_eq!(
            knots[0],
            Knot::corner(Point::new(0.0, 0.0)),
            "neighbours untouched"
        );
        assert_eq!(knots[2], Knot::corner(Point::new(10.0, 0.0)));
        // The closing segment (last knot back to the first) also takes a point.
        assert!(insert_path_knot(&mut subs, KnotRef::new(0, 3), 0.5).is_some());
        assert_eq!(subs[0].knots.len(), 5);
        assert!(insert_path_knot(&mut subs, KnotRef::new(0, 0), f64::NAN).is_none());
        assert!(insert_path_knot(&mut subs, KnotRef::new(3, 0), 0.5).is_none());
    }

    #[test]
    fn dragging_an_open_end_onto_the_other_closes_or_joins() {
        use craft_draw::ui::pen::{EditSubpath, Knot, KnotRef};
        use stencilcraft_geom::Point;
        let open = |pts: &[(f64, f64)]| {
            EditSubpath::new(
                pts.iter()
                    .map(|&(x, y)| Knot::corner(Point::new(x, y)))
                    .collect(),
                false,
            )
        };
        // One open subpath: its last end dropped on its first closes it.
        let mut subs = vec![open(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.2, 0.1)])];
        assert_eq!(
            open_end_near(&subs, KnotRef::new(0, 3), Point::new(0.2, 0.1), 1.0),
            Some(KnotRef::new(0, 0))
        );
        assert!(join_path_ends(
            &mut subs,
            KnotRef::new(0, 3),
            KnotRef::new(0, 0)
        ));
        assert!(subs[0].closed);
        assert_eq!(subs[0].knots.len(), 3);
        assert_eq!(subs[0].knots[0].anchor, Point::new(0.0, 0.0));
        // A closed subpath has no open ends left.
        assert!(open_path_ends(&subs).is_empty());

        // Two subpaths: start of one dropped on the start of the other joins them.
        let mut subs = vec![
            open(&[(0.0, 0.0), (10.0, 0.0)]),
            open(&[(0.0, 5.0), (10.0, 5.0)]),
        ];
        assert!(join_path_ends(
            &mut subs,
            KnotRef::new(0, 0),
            KnotRef::new(1, 0)
        ));
        assert_eq!(subs.len(), 1);
        let anchors: Vec<Point> = subs[0].knots.iter().map(|k| k.anchor).collect();
        assert_eq!(
            anchors,
            vec![
                Point::new(10.0, 0.0),
                Point::new(0.0, 5.0),
                Point::new(10.0, 5.0)
            ]
        );

        // Not ends, or a two-knot line onto itself: nothing changes.
        let mut subs = vec![open(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)])];
        assert!(!join_path_ends(
            &mut subs,
            KnotRef::new(0, 1),
            KnotRef::new(0, 0)
        ));
        let mut line = vec![open(&[(0.0, 0.0), (5.0, 0.0)])];
        assert!(!join_path_ends(
            &mut line,
            KnotRef::new(0, 1),
            KnotRef::new(0, 0)
        ));
        assert!(!join_path_ends(
            &mut line,
            KnotRef::new(7, 0),
            KnotRef::new(0, 0)
        ));
    }

    #[test]
    fn bundled_theme_parses_and_resolves_in_both_modes() {
        assert!(craft_theme::format::parse(THEME_JSON).is_ok());
        assert_eq!(root_env(ColorTheme::Dark).theme_id().as_str(), "craft.pro");
        assert_eq!(
            root_env(ColorTheme::Light).theme_id().as_str(),
            "stencilcraft.modern"
        );
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
    fn cut_to_page_edge_moves_the_border_to_the_edge() {
        let mut options = CanvasOptions::default();
        assert_eq!(cut_border_inset(&options), CUT_BORDER_INSET_MM);
        options.cut_to_page_edge = true;
        assert_eq!(cut_border_inset(&options), 0.0);
    }

    #[test]
    fn page_fill_takes_the_colour_and_transparency() {
        let white = egui::Color32::WHITE;
        let mut options = CanvasOptions::default();
        assert_eq!(page_fill(white, &options), white);
        options.page_colour = Some([255, 0, 0]);
        options.page_transparency = 50.0;
        let fill = page_fill(white, &options);
        assert_eq!(fill.a(), 128);
        options.page_transparency = f64::NAN;
        assert_eq!(page_fill(white, &options).a(), 255, "a bad value is opaque");
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
        use craft_behavior::Axis;
        use craft_ui::canvas::GuideKey;
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
                shown[0].along,
                shown[0].position,
                shown[0].locked
            ),
            (GuideKey::new(1), Axis::Y, 10.0, false)
        );
        assert_eq!(
            (shown[1].key, shown[1].along, shown[1].locked),
            (GuideKey::new(2), Axis::X, true)
        );
        let moved = canvas_guides(
            &guides,
            Some(GuidePreview {
                key: GuideKey::new(1),
                along: Axis::Y,
                at: Some(15.0),
            }),
        );
        assert_eq!(moved[0].position, 15.0);
        let removed = canvas_guides(
            &guides,
            Some(GuidePreview {
                key: GuideKey::new(1),
                along: Axis::Y,
                at: None,
            }),
        );
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].key, GuideKey::new(2));
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
    fn isometric_lines_are_clipped_and_evenly_spaced() {
        let vertical = isometric_lines(40.0, 30.0, 10.0, 90.0);
        assert_eq!(vertical.len(), 5, "x = 0, 10, 20, 30, 40");
        assert!(
            vertical
                .iter()
                .all(|(a, b)| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() > 29.99)
        );
        let diag = isometric_lines(40.0, 30.0, 10.0, 30.0);
        assert!(!diag.is_empty());
        for (a, b) in &diag {
            assert!(
                ((b.y - a.y).atan2(b.x - a.x).to_degrees().rem_euclid(180.0) - 30.0).abs() < 1e-6
            );
            for p in [a, b] {
                assert!(
                    p.x > -1e-6 && p.x < 40.0 + 1e-6 && p.y > -1e-6 && p.y < 30.0 + 1e-6,
                    "{p:?}"
                );
            }
        }
        assert!(isometric_lines(40.0, 30.0, f64::NAN, 30.0).is_empty());
        assert!(isometric_lines(1e6, 1e6, 1e-3, 30.0).len() <= 10_000);
    }

    #[test]
    fn grid_lines_take_their_primary_and_secondary_colours() {
        let mut doc = stencilcraft_doc::Document::default();
        doc.board.width_mm = 40.0;
        doc.board.height_mm = 40.0;
        doc.grid.spacing_mm = 10.0;
        doc.grid.subdivisions = 2;
        let view = View {
            zoom: 10.0,
            ..View::default()
        };
        let clip =
            egui::Rect::from_min_size(egui::pos2(-500.0, -500.0), egui::vec2(2000.0, 2000.0));
        let options = CanvasOptions {
            grid_primary: Some([200, 0, 0]),
            grid_secondary: Some([0, 0, 200]),
            ..CanvasOptions::default()
        };
        let shapes = painted(|p| {
            paint_document(
                p,
                clip,
                &doc,
                &view,
                CanvasLayers {
                    backdrop: None,
                    image_textures: &std::collections::HashMap::new(),
                    pattern_textures: &std::collections::HashMap::new(),
                },
                options,
                &SelectionState::default(),
            )
        });
        let count = |c: egui::Color32| {
            shapes
                .iter()
                .filter(
                    |s| matches!(s, egui::Shape::LineSegment { stroke, .. } if stroke.color == c),
                )
                .count()
        };
        // 5 spacing lines a side, and 9 subdivision lines a side (every 5 mm).
        assert_eq!(count(egui::Color32::from_rgb(200, 0, 0)), 10);
        assert_eq!(count(egui::Color32::from_rgb(0, 0, 200)), 18);
    }

    #[test]
    fn guide_layers_draw_in_grey_dashes() {
        let mut doc = stencilcraft_doc::Document::default();
        let mut path = kurbo::BezPath::new();
        path.move_to((10.0, 10.0));
        path.line_to((60.0, 10.0));
        doc.push_shape(0, path, stencilcraft_doc::CutStyle::Cut)
            .unwrap();
        let view = View::default();
        let clip =
            egui::Rect::from_min_size(egui::pos2(-500.0, -500.0), egui::vec2(2000.0, 2000.0));
        let paint = |doc: &stencilcraft_doc::Document| {
            painted(|p| {
                paint_document(
                    p,
                    clip,
                    doc,
                    &view,
                    CanvasLayers {
                        backdrop: None,
                        image_textures: &std::collections::HashMap::new(),
                        pattern_textures: &std::collections::HashMap::new(),
                    },
                    CanvasOptions::default(),
                    &SelectionState::default(),
                )
            })
        };
        let grey = |shapes: &[egui::Shape]| {
            shapes.iter().filter(|s| matches!(s, egui::Shape::LineSegment { stroke, .. } if stroke.color == GUIDE_LAYER_INK)).count()
        };
        assert_eq!(grey(&paint(&doc)), 0, "a cut layer is not grey");
        if let Some(layer) = doc.layers.first_mut() {
            layer.template = true;
        }
        assert!(
            grey(&paint(&doc)) > 1,
            "a guide layer's line is grey dashes"
        );
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
                CanvasLayers {
                    backdrop: None,
                    image_textures: &std::collections::HashMap::new(),
                    pattern_textures: &std::collections::HashMap::new(),
                },
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
