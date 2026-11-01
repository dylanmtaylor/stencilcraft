//! The StencilCraft desktop app: window bootstrap, Craft-suite chrome (`MenuBar`, `OptionsBar`,
//! `ToolRail`, `Workspace`, `DockArea`, `Panel`, `StatusBar`, `Rulers`, `BezierOverlay`,
//! `PenDraft`), file dialogs, and machine cut preparation.
//!
//! All user-visible document mutations route through `stencilcraft_engine::Command`; canvas
//! painting and geometry conversions live in `stencilcraft_ui_egui`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use craft_draw::ui::pen::{
    BezierNames, BezierOverlay, BezierTool, EditPath, EditSubpath, HandlesShown, PathSelection,
    PenDraft, PenNames,
};
#[cfg(not(target_arch = "wasm32"))]
use craft_settings::{Location, Settings, SettingsStore};
use craft_theme::ControlSize;
use craft_ui::canvas::{Readout, RulerNames, RulerUnits, Rulers};
use craft_ui::composites::PanelFooter;
use craft_ui::controls::{
    Button, ButtonVariant, Checkbox, IconButton, Kbd, NumberField, Select, Slider, SliderScale,
    TextField, ToggleButton,
};
use craft_ui::docking::{
    Column, DockArea, DockContent, DockStyle, Group, Layout, Node, Permissions, Side,
};
use craft_ui::edit::{CancelReason, EditEvent};
use craft_ui::icons::Icon;
use craft_ui::layout::{SplitAxis, SplitSize};
use craft_ui::list::{ListRow, ToggleColumn};
use craft_ui::localized::{LocaleNumbers, Unit};
use craft_ui::menu::{MenuBar, MenuEntry, MenuItem};
use craft_ui::patterns::{LastUsed, OptionsBar, Panel, RailTool, StatusBar, ToolRail, ToolSlot};
use craft_ui::title_bar::{Frame, LeadingEnd, Title, TitleBar};
use eframe::egui;
use serde::{Deserialize, Serialize};
use stencilcraft_doc::CutStyle;
use stencilcraft_engine::{Alignment, BooleanOperation, Command, DistributionAxis};

/// CLI/agent control channel: JSON lines on a TCP socket, one session per
/// connection (the same session core the CLI's `mcp` mode uses).
mod control;

/// The same bounded PNG/JPEG silhouette tracer used by the CLI.
#[path = "../../stencilcraft-cli/src/trace.rs"]
mod trace;

const APP_ICON_PNG: &[u8] =
    include_bytes!("../../../assets/app-icon/hicolor/256x256/apps/ai.storyteller.stencilcraft.png");

/// Persistent user preferences stored via `craft-settings`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPrefs {
    pub dark_theme: bool,
    pub show_rulers: bool,
    pub show_grid: bool,
    pub print_cut_marks: bool,
    pub rail_columns: u8,
    pub link_aspect: bool,
    pub recent_files: Vec<String>,
}

impl Default for UserPrefs {
    fn default() -> Self {
        Self {
            dark_theme: true,
            show_rulers: true,
            show_grid: true,
            print_cut_marks: false,
            rail_columns: 1,
            link_aspect: false,
            recent_files: Vec::new(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Settings for UserPrefs {
    const APP_ID: &'static str = "stencilcraft";
    const SCHEMA_VERSION: u32 = 1;
}

/// Interactive canvas tools in the left `ToolRail`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ActiveTool {
    #[default]
    Select,
    DirectSelect,
    ConvertPoint,
    BoxSelect,
    LassoSelect,
    Pen,
    Rect,
    RoundedRect,
    Ellipse,
    Polygon,
    Star,
    Text,
    TraceImage,
    Hand,
    Zoom,
}

impl ActiveTool {
    pub fn icon(self) -> Icon {
        match self {
            Self::Select => Icon::Pointer,
            Self::DirectSelect => Icon::DirectSelect,
            Self::ConvertPoint => Icon::ConvertPoint,
            Self::BoxSelect => Icon::MarqueeRect,
            Self::LassoSelect => Icon::Lasso,
            Self::Pen => Icon::PenTool,
            Self::Rect => Icon::Square,
            Self::RoundedRect => Icon::FramePlaceholder,
            Self::Ellipse => Icon::Circle,
            Self::Polygon => Icon::Pentagon,
            Self::Star => Icon::Star,
            Self::Text => Icon::Text,
            Self::TraceImage => Icon::Image,
            Self::Hand => Icon::Hand,
            Self::Zoom => Icon::ZoomIn,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::DirectSelect => "Direct Select",
            Self::ConvertPoint => "Convert Point",
            Self::BoxSelect => "Box Select",
            Self::LassoSelect => "Lasso Select",
            Self::Pen => "Pen",
            Self::Rect => "Rectangle",
            Self::RoundedRect => "Rounded Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Polygon => "Polygon",
            Self::Star => "Star",
            Self::Text => "Text",
            Self::TraceImage => "Trace Image",
            Self::Hand => "Hand",
            Self::Zoom => "Zoom",
        }
    }

    pub fn is_shape_creator(self) -> bool {
        matches!(
            self,
            Self::Rect | Self::RoundedRect | Self::Ellipse | Self::Polygon | Self::Star
        )
    }
}

fn build_tool_slots() -> Vec<ToolSlot<ActiveTool>> {
    vec![
        ToolSlot::new(
            0,
            vec![
                RailTool::new(ActiveTool::Select, Icon::Pointer, "Select").shortcut("V"),
                RailTool::new(
                    ActiveTool::DirectSelect,
                    Icon::DirectSelect,
                    "Direct Select (Nodes)",
                )
                .shortcut("A"),
                RailTool::new(
                    ActiveTool::ConvertPoint,
                    Icon::ConvertPoint,
                    "Convert Anchor Point",
                )
                .shortcut("X"),
            ],
        ),
        ToolSlot::new(
            1,
            vec![
                RailTool::new(ActiveTool::BoxSelect, Icon::MarqueeRect, "Box Select").shortcut("M"),
                RailTool::new(ActiveTool::LassoSelect, Icon::Lasso, "Lasso Select").shortcut("L"),
            ],
        ),
        ToolSlot::new(
            2,
            vec![RailTool::new(ActiveTool::Pen, Icon::PenTool, "Pen (Draw Path)").shortcut("P")],
        ),
        ToolSlot::new(
            3,
            vec![
                RailTool::new(ActiveTool::Rect, Icon::Square, "Rectangle").shortcut("R"),
                RailTool::new(
                    ActiveTool::RoundedRect,
                    Icon::FramePlaceholder,
                    "Rounded Rectangle",
                )
                .shortcut("U"),
                RailTool::new(ActiveTool::Ellipse, Icon::Circle, "Ellipse").shortcut("O"),
                RailTool::new(ActiveTool::Polygon, Icon::Pentagon, "Polygon").shortcut("G"),
                RailTool::new(ActiveTool::Star, Icon::Star, "Star").shortcut("S"),
            ],
        ),
        ToolSlot::new(
            4,
            vec![RailTool::new(ActiveTool::Text, Icon::Text, "Text Lettering").shortcut("T")],
        ),
        ToolSlot::new(
            5,
            vec![
                RailTool::new(
                    ActiveTool::TraceImage,
                    Icon::Image,
                    "Trace Image Silhouette",
                )
                .shortcut("I"),
            ],
        ),
        ToolSlot::new(
            6,
            vec![
                RailTool::new(ActiveTool::Hand, Icon::Hand, "Hand (Pan)").shortcut("H"),
                RailTool::new(ActiveTool::Zoom, Icon::ZoomIn, "Zoom").shortcut("Z"),
            ],
        ),
    ]
}

/// Menu items in the integrated window title bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopMenuAction {
    NewDoc,
    OpenDoc,
    OpenRecent(usize),
    SaveDoc,
    SaveAsDoc,
    ImportSvg,
    ImportImage,
    ExportSvg,
    ExportPrintCutSvg,
    ExportDxf,
    ExportGcode,
    ExportTiledSvg,
    Undo,
    Redo,
    Duplicate,
    Delete,
    SelectAll,
    Deselect,
    Pathfinder(BooleanOperation),
    Align(Alignment),
    Distribute(DistributionAxis),
    RotateCw15,
    RotateCcw15,
    FitView,
    CenterView,
    ActualSize,
    ZoomIn,
    ZoomOut,
    ToggleRulers,
    ToggleGrid,
    ToggleKerfPreview,
    TogglePrintCutMarks,
    ResetWorkspace,
    SetTheme(stencilcraft_ui_egui::ColorTheme),
    HelpManual,
    HelpCheatsheet,
    HelpAbout,
}

fn apply_edit<T: Clone>(
    value: &mut T,
    before: &mut Option<T>,
    event: Option<EditEvent<T>>,
) -> bool {
    match event {
        Some(EditEvent::Begin { .. }) => *before = Some(value.clone()),
        Some(EditEvent::Update { value: next, .. }) => {
            *value = next;
            return true;
        }
        Some(EditEvent::Set { value: next, .. }) | Some(EditEvent::Commit { value: next, .. }) => {
            *value = next;
            *before = None;
            return true;
        }
        Some(EditEvent::Cancel { reason, .. }) => {
            if reason != CancelReason::External
                && let Some(original) = before.take()
            {
                *value = original;
                return true;
            }
            *before = None;
        }
        _ => {}
    }
    false
}

/// Display label for a media preset index (parallel to `MEDIA_PRESETS`).
fn media_label(index: usize) -> String {
    match stencilcraft_machine::MEDIA_PRESETS.get(index) {
        Some(m) => format!("{} (ID {})", m.name, m.id),
        None => "Custom (ID 300)".to_string(),
    }
}

/// Mat chooser entries: (device mat, label).
pub const MAT_CHOICES: &[(stencilcraft_machine::Mat, &str)] = &[
    (stencilcraft_machine::Mat::None, "no mat"),
    (stencilcraft_machine::Mat::Cameo12x12, "12×12"),
    (stencilcraft_machine::Mat::Cameo12x24, "12×24"),
    (stencilcraft_machine::Mat::Portrait8x12, "8×12 portrait"),
    (stencilcraft_machine::Mat::CameoPlus15x15, "15×15 plus"),
    (stencilcraft_machine::Mat::CameoPro24x24, "24×24 Pro"),
    (stencilcraft_machine::Mat::Portrait8_5x12, "8.5×12 portrait"),
];

pub fn mat_label(mat: stencilcraft_machine::Mat) -> &'static str {
    MAT_CHOICES
        .iter()
        .find(|(m, _)| *m == mat)
        .map(|(_, label)| *label)
        .unwrap_or("no mat")
}

/// Machine chooser entries: (device model, label). Most specific first.
pub const MODEL_CHOICES: &[(stencilcraft_machine::Model, &str)] = &[
    (stencilcraft_machine::Model::Cameo4Pro, "Cameo 4 Pro"),
    (stencilcraft_machine::Model::Cameo4Plus, "Cameo 4 Plus"),
    (stencilcraft_machine::Model::Cameo4, "Cameo 4"),
    (stencilcraft_machine::Model::Cameo5, "Cameo 5"),
    (stencilcraft_machine::Model::Cameo5Plus, "Cameo 5 Plus"),
    (stencilcraft_machine::Model::CameoProMk2, "Cameo Pro MK-II"),
    (stencilcraft_machine::Model::Cameo3, "Cameo 3"),
    (stencilcraft_machine::Model::Portrait4, "Portrait 4"),
    (stencilcraft_machine::Model::Portrait3, "Portrait 3"),
];

pub fn model_label(model: stencilcraft_machine::Model) -> &'static str {
    MODEL_CHOICES
        .iter()
        .find(|(m, _)| *m == model)
        .map(|(_, label)| *label)
        .unwrap_or("Cameo 4 Pro")
}

/// The canvas backdrop for the currently selected mat.
pub fn mat_backdrop(mat: stencilcraft_machine::Mat) -> stencilcraft_ui_egui::MatBackdrop {
    let (down_in, right_in) = mat.extent_inch();
    stencilcraft_ui_egui::MatBackdrop {
        width_mm: right_in * 25.4,
        height_mm: down_in * 25.4,
        label: mat_label(mat).to_string(),
    }
}

/// Flatten every visible shape into machine paths (document mm coordinates:
/// x right, y down), closing closed polylines.
fn machine_paths(doc: &stencilcraft_doc::Document) -> Vec<Vec<(f64, f64)>> {
    let mut paths = Vec::new();
    for layer in &doc.layers {
        if !layer.visible {
            continue;
        }
        for shape in &layer.shapes {
            for pl in stencilcraft_geom::path_to_polylines(
                &shape.path,
                stencilcraft_geom::DEFAULT_FLATTEN_STEPS,
            ) {
                if pl.points.len() < 2 {
                    continue;
                }
                let mut path: Vec<(f64, f64)> = pl.points.iter().map(|p| (p.x, p.y)).collect();
                if pl.closed
                    && let Some(first) = pl.points.first()
                {
                    path.push((first.x, first.y));
                }
                paths.push(path);
            }
        }
    }
    paths
}

fn cursor_text_width(text: &str) -> f64 {
    text.chars()
        .take(80)
        .map(|character| if character.is_whitespace() { 4.0 } else { 6.0 })
        .sum::<f64>()
        .max(1.0)
}

fn bitmap_text_path(text: &str) -> stencilcraft_geom::BezPath {
    const GLYPHS: &[(char, [u8; 7])] = &[
        ('A', [14, 17, 17, 31, 17, 17, 17]),
        ('B', [30, 17, 17, 30, 17, 17, 30]),
        ('C', [14, 17, 16, 16, 16, 17, 14]),
        ('D', [30, 17, 17, 17, 17, 17, 30]),
        ('E', [31, 16, 16, 30, 16, 16, 31]),
        ('F', [31, 16, 16, 30, 16, 16, 16]),
        ('G', [14, 17, 16, 23, 17, 17, 15]),
        ('H', [17, 17, 17, 31, 17, 17, 17]),
        ('I', [31, 4, 4, 4, 4, 4, 31]),
        ('J', [7, 2, 2, 2, 18, 18, 12]),
        ('K', [17, 18, 20, 24, 20, 18, 17]),
        ('L', [16, 16, 16, 16, 16, 16, 31]),
        ('M', [17, 27, 21, 21, 17, 17, 17]),
        ('N', [17, 25, 25, 21, 19, 19, 17]),
        ('O', [14, 17, 17, 17, 17, 17, 14]),
        ('P', [30, 17, 17, 30, 16, 16, 16]),
        ('Q', [14, 17, 17, 17, 21, 18, 13]),
        ('R', [30, 17, 17, 30, 20, 18, 17]),
        ('S', [15, 16, 16, 14, 1, 1, 30]),
        ('T', [31, 4, 4, 4, 4, 4, 4]),
        ('U', [17, 17, 17, 17, 17, 17, 14]),
        ('V', [17, 17, 17, 17, 17, 10, 4]),
        ('W', [17, 17, 17, 21, 21, 21, 10]),
        ('X', [17, 17, 10, 4, 10, 17, 17]),
        ('Y', [17, 17, 10, 4, 4, 4, 4]),
        ('Z', [31, 1, 2, 4, 8, 16, 31]),
        ('0', [14, 17, 19, 21, 25, 17, 14]),
        ('1', [4, 12, 4, 4, 4, 4, 14]),
        ('2', [14, 17, 1, 2, 4, 8, 31]),
        ('3', [30, 1, 1, 14, 1, 1, 30]),
        ('4', [2, 6, 10, 18, 31, 2, 2]),
        ('5', [31, 16, 16, 30, 1, 1, 30]),
        ('6', [14, 16, 16, 30, 17, 17, 14]),
        ('7', [31, 1, 2, 4, 8, 8, 8]),
        ('8', [14, 17, 17, 14, 17, 17, 14]),
        ('9', [14, 17, 17, 15, 1, 1, 14]),
        ('?', [14, 17, 1, 2, 4, 0, 4]),
        ('-', [0, 0, 0, 31, 0, 0, 0]),
        ('.', [0, 0, 0, 0, 0, 12, 12]),
    ];
    let mut path = stencilcraft_geom::BezPath::new();
    let mut cursor_x = 20.0;
    for character in text.chars().take(80) {
        if character.is_whitespace() {
            cursor_x += 4.0;
            continue;
        }
        let key = character.to_ascii_uppercase();
        let rows = GLYPHS
            .iter()
            .find(|(glyph, _)| *glyph == key)
            .or_else(|| GLYPHS.iter().find(|(glyph, _)| *glyph == '?'))
            .map(|(_, rows)| rows);
        if let Some(rows) = rows {
            for (row, bits) in rows.iter().enumerate() {
                for column in 0..5 {
                    if bits & (1 << (4 - column)) != 0 {
                        let x = cursor_x + f64::from(column);
                        let y = 20.0 + row as f64;
                        path.move_to(stencilcraft_geom::Point::new(x, y + 0.4));
                        path.line_to(stencilcraft_geom::Point::new(x + 0.8, y + 0.4));
                    }
                }
            }
        }
        cursor_x += 6.0;
    }
    path
}

fn main() -> Result<(), String> {
    let mut control_port: Option<u16> = None;
    let mut open_path: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--control" => {
                let port = args.next().ok_or("--control needs a port".to_string())?;
                control_port = Some(
                    port.parse()
                        .map_err(|_| "--control needs a numeric port".to_string())?,
                );
            }
            "-h" | "--help" => {
                println!("stencilcraft — desktop app");
                println!("usage: stencilcraft [--control PORT] [FILE.stencilcraft]");
                return Ok(());
            }
            other => open_path = Some(other.to_string()),
        }
    }
    let decision = craft_window::policy::decide_for(
        craft_window::Prefix::new("STENCILCRAFT"),
        craft_window::policy::Backend::from_env(),
        craft_window::policy::TitleBarPref::Integrated,
    );

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1120.0, 760.0])
        .with_title("StencilCraft");

    if let Ok(img) = image::load_from_memory(APP_ICON_PNG) {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        viewport = viewport.with_icon(egui::IconData {
            rgba: rgba.into_raw(),
            width: w,
            height: h,
        });
    }

    let viewport = craft_window::policy::apply(viewport, &decision.frame);

    let mut app = StencilApp::new(open_path.as_deref());
    app.window_frame = decision.frame;
    app.control_port = control_port;

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "stencilcraft",
        options,
        Box::new(move |cc| Ok(Box::new(app.with_egui(cc)))),
    )
    .map_err(|e| format!("stencilcraft: {e}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WorkspacePanel {
    Canvas,
    Tools,
    Inspector,
    Layers,
}

struct WorkspaceContent<'a> {
    app: &'a mut StencilApp,
}

impl DockContent<WorkspacePanel> for WorkspaceContent<'_> {
    fn body(&mut self, ui: &mut egui::Ui, panel: &WorkspacePanel) {
        match panel {
            WorkspacePanel::Canvas => self.app.canvas_body(ui),
            WorkspacePanel::Tools => self.app.tools_panel(ui),
            WorkspacePanel::Inspector => self.app.inspector_panel(ui),
            WorkspacePanel::Layers => self.app.layers_panel(ui),
        }
    }

    fn logical_label(&self, panel: &WorkspacePanel) -> String {
        match panel {
            WorkspacePanel::Canvas => "Canvas",
            WorkspacePanel::Tools => "Tools",
            WorkspacePanel::Inspector => "Properties",
            WorkspacePanel::Layers => "Layers",
        }
        .to_owned()
    }

    fn permissions(&self, panel: &WorkspacePanel) -> Permissions {
        if *panel == WorkspacePanel::Canvas {
            Permissions::PROTECTED
        } else {
            Permissions::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasViewCommand {
    Fit,
    Center,
}

pub struct StencilApp {
    pub workspace: Layout<WorkspacePanel>,
    pub window_frame: Frame,
    pub engine: stencilcraft_engine::Engine,
    pub view: stencilcraft_ui_egui::View,
    pub pending_view_command: Option<CanvasViewCommand>,
    pub kerf_preview: bool,
    pub kerf_preview_cache: Option<KerfPreviewCache>,
    pub kerf_draft: Option<f64>,
    pub pressure_before: Option<f64>,
    pub speed_before: Option<f64>,
    pub zoom_before: Option<f64>,
    pub text_before: Option<String>,
    pub inspector_before: [Option<String>; 4],
    pub status: String,
    pub open_path: Option<String>,
    pub send_mode: bool,
    pub theme: stencilcraft_ui_egui::ColorTheme,
    pub view_initialized: bool,
    /// Board size the canvas was last framed for, millimetres.
    pub last_board: Option<(f64, f64)>,
    pub recent_files: Vec<String>,
    pub mat: stencilcraft_machine::Mat,
    pub selected_shape: Option<u64>,
    pub selected_shapes: Vec<u64>,
    pub selected_layer: Option<usize>,
    pub inspector_shape: Option<u64>,
    pub inspector_bounds: Option<[f64; 4]>,
    pub inspector_values: [String; 4],
    pub inspector_editing: bool,
    pub text_dialog: bool,
    pub text_value: String,
    pub canvas_drag_shape: Option<u64>,
    pub canvas_selection_start: Option<egui::Pos2>,
    pub canvas_selection_additive: bool,
    pub canvas_lasso_mode: bool,
    pub canvas_selection_points: Vec<egui::Pos2>,
    pub canvas_panning: bool,
    pub last_drag_delta: egui::Vec2,
    pub send: SendTab,
    pub active_tool: ActiveTool,
    pub rail_last_used: LastUsed<ActiveTool>,
    pub path_selection: PathSelection,
    pub bezier_gesture_base: Option<(u64, Vec<EditSubpath>)>,
    pub bezier_live_path: Option<(u64, stencilcraft_geom::BezPath)>,
    pub handles_shown: HandlesShown,
    pub pen_draft: Option<EditSubpath>,
    pub shape_drag_start: Option<stencilcraft_geom::Point>,
    pub shape_drag_current: Option<stencilcraft_geom::Point>,
    pub ruler_origin: stencilcraft_geom::Point,
    pub locale_numbers: LocaleNumbers,
    pub prefs: UserPrefs,
    #[cfg(not(target_arch = "wasm32"))]
    pub settings_store: Option<SettingsStore<UserPrefs>>,
    /// Port requested with `--control`; the socket starts once eframe hands us the
    /// paint context, so requests can wake the window.
    pub control_port: Option<u16>,
    /// Queued control requests, handled on the UI thread against `engine`.
    pub control_jobs: Option<control::Jobs>,
}

#[derive(Clone, Debug)]
pub struct KerfPreviewCache {
    pub source: stencilcraft_doc::Document,
    pub kerf_width_mm: f64,
    pub preview: Result<stencilcraft_doc::Document, String>,
}

#[derive(Debug, Clone)]
pub struct SendTab {
    pub media_index: usize,
    pub model: stencilcraft_machine::Model,
    pub pressure_override: u8,
    pub speed_override: u8,
    pub bbox_only: bool,
    pub summary: Option<Vec<String>>,
}

impl Default for SendTab {
    fn default() -> Self {
        SendTab {
            media_index: machine_index_of_media(102),
            model: stencilcraft_machine::Model::Cameo4Pro,
            pressure_override: 10,
            speed_override: 5,
            bbox_only: false,
            summary: None,
        }
    }
}

pub fn default_workspace() -> Layout<WorkspacePanel> {
    Layout {
        root: Some(Node::Split {
            axis: SplitAxis::Horizontal,
            size: SplitSize::Ratio(0.78),
            first: Box::new(Node::Tabs {
                panels: vec![WorkspacePanel::Canvas],
                active: 0,
            }),
            second: Box::new(Node::Dock {
                side: Side::End,
                columns: vec![Column::new(
                    vec![Group::new(
                        vec![WorkspacePanel::Inspector, WorkspacePanel::Layers],
                        0,
                    )],
                    280.0,
                )],
            }),
        }),
        floating: Vec::new(),
    }
}

fn machine_index_of_media(id: u16) -> usize {
    stencilcraft_machine::MEDIA_PRESETS
        .iter()
        .position(|m| m.id == id)
        .unwrap_or(0)
}

impl StencilApp {
    pub fn new(open_path: Option<&str>) -> Self {
        let mut engine = stencilcraft_engine::Engine::default();
        let mut status = String::from("ready");
        let open_path = open_path.and_then(|path| match stencilcraft_doc::load_from(path) {
            Ok(doc) => {
                engine = stencilcraft_engine::Engine::new(doc);
                status = format!("opened {path}");
                Some(path.to_string())
            }
            Err(e) => {
                status = format!("could not open: {e}");
                None
            }
        });
        let prefs = UserPrefs::default();
        let theme = if prefs.dark_theme {
            stencilcraft_ui_egui::ColorTheme::Dark
        } else {
            stencilcraft_ui_egui::ColorTheme::Light
        };
        Self {
            workspace: default_workspace(),
            window_frame: Frame::Native,
            engine,
            view: stencilcraft_ui_egui::View::default(),
            pending_view_command: None,
            kerf_preview: false,
            kerf_preview_cache: None,
            kerf_draft: None,
            pressure_before: None,
            speed_before: None,
            zoom_before: None,
            text_before: None,
            inspector_before: std::array::from_fn(|_| None),
            status,
            open_path: open_path.clone(),
            send_mode: false,
            theme,
            view_initialized: false,
            last_board: None,
            recent_files: open_path.iter().cloned().collect(),
            mat: stencilcraft_machine::Mat::CameoPro24x24,
            selected_shape: None,
            selected_shapes: Vec::new(),
            selected_layer: Some(0),
            inspector_shape: None,
            inspector_bounds: None,
            inspector_values: std::array::from_fn(|_| String::new()),
            inspector_editing: false,
            text_dialog: false,
            text_value: String::new(),
            canvas_drag_shape: None,
            canvas_selection_start: None,
            canvas_selection_additive: false,
            canvas_lasso_mode: false,
            canvas_selection_points: Vec::new(),
            canvas_panning: false,
            last_drag_delta: egui::Vec2::ZERO,
            send: SendTab::default(),
            active_tool: ActiveTool::Select,
            rail_last_used: LastUsed::default(),
            path_selection: PathSelection::default(),
            bezier_gesture_base: None,
            bezier_live_path: None,
            handles_shown: HandlesShown::Selected,
            pen_draft: None,
            shape_drag_start: None,
            shape_drag_current: None,
            ruler_origin: stencilcraft_geom::Point::ZERO,
            locale_numbers: LocaleNumbers::from_tag("en-US"),
            prefs,
            #[cfg(not(target_arch = "wasm32"))]
            settings_store: None,
            control_port: None,
            control_jobs: None,
        }
    }

    /// Run queued `--control` requests against the document on screen. The socket
    /// hands work to the UI thread, so an agent edits what the user sees (and the
    /// user can undo it) instead of a private copy of the file.
    fn drain_control_requests(&mut self) {
        let Some(jobs) = self.control_jobs.as_ref() else {
            return;
        };
        let mut pending = Vec::new();
        while let Some(job) = jobs.try_recv() {
            pending.push(job);
        }
        for job in pending {
            // The session owns the engine while it handles one request, then hands
            // it back: the document, undo history and counters all survive.
            let engine = std::mem::take(&mut self.engine);
            let mut session = stencilcraft_mcp::Session::with_engine(engine);
            let reply = match session.handle_stdio_line(&job.line) {
                Ok(Some(response)) => response,
                Ok(None) => String::new(),
                Err(e) => {
                    self.status = format!("control: {e}");
                    r#"{"id":null,"ok":false,"error":"internal control error"}"#.to_string()
                }
            };
            self.engine = session.into_engine();
            self.retain_valid_selection();
            self.kerf_preview_cache = None;
            let _ = job.reply.send(reply);
        }
    }

    pub fn with_egui(mut self, cc: &eframe::CreationContext) -> Self {
        if let Some(port) = self.control_port {
            let ctx = cc.egui_ctx.clone();
            self.control_jobs = control::serve(port, move || ctx.request_repaint());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (store, _report) = SettingsStore::<UserPrefs>::open(Location::Auto);
            self.prefs = store.get().clone();
            self.settings_store = Some(store);
            if !self.prefs.recent_files.is_empty() {
                self.recent_files = self.prefs.recent_files.clone();
            }
        }
        if self.recent_files.is_empty() {
            self.recent_files = cc
                .storage
                .and_then(|storage| eframe::get_value(storage, "recent_files"))
                .unwrap_or_default();
        }
        if let Some(path) = self.open_path.clone() {
            self.record_recent_file(path);
        }
        self.theme = if self.prefs.dark_theme {
            stencilcraft_ui_egui::ColorTheme::Dark
        } else {
            stencilcraft_ui_egui::ColorTheme::Light
        };
        self
    }

    pub fn save_prefs(&mut self) {
        self.prefs.recent_files = self.recent_files.clone();
        self.prefs.dark_theme = self.theme == stencilcraft_ui_egui::ColorTheme::Dark;
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(store) = self.settings_store.as_mut() {
            let p = self.prefs.clone();
            store.update(|val| *val = p);
            let _ = store.flush();
        }
    }

    pub fn reset_workspace(&mut self) {
        self.workspace = default_workspace();
        self.status = "panel layout restored".to_string();
    }

    pub fn run_command(&mut self, command: Command) {
        match self.engine.run(command) {
            Ok(_effect) => self.status = "done".to_string(),
            Err(e) => self.status = e.to_string(),
        }
    }

    pub fn save_to(&mut self, path: String) {
        match stencilcraft_doc::save_to(&self.engine.doc, &path) {
            Ok(()) => {
                self.open_path = Some(path.clone());
                self.record_recent_file(path);
                self.status = "saved".to_string();
            }
            Err(e) => self.status = e.to_string(),
        }
    }

    pub fn record_recent_file(&mut self, path: String) {
        let path = std::fs::canonicalize(&path)
            .unwrap_or_else(|_| std::path::PathBuf::from(path))
            .to_string_lossy()
            .into_owned();
        self.recent_files.retain(|recent| recent != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(8);
        self.save_prefs();
    }

    /// Records the board size the view is framed for. Returns true when the size is new,
    /// which the canvas turns into a `Fit` so a resized canvas is framed instead of
    /// leaving the artwork wherever the old sheet happened to place it.
    pub fn note_board_size(&mut self, width_mm: f64, height_mm: f64) -> bool {
        let board = (width_mm, height_mm);
        if self.last_board == Some(board) {
            return false;
        }
        self.last_board = Some(board);
        true
    }

    pub fn open_document(&mut self, path: &str) {
        match stencilcraft_doc::load_from(path) {
            Ok(doc) => {
                self.engine = stencilcraft_engine::Engine::new(doc);
                self.set_selection([]);
                self.open_path = Some(path.to_string());
                self.view_initialized = false;
                self.last_board = None;
                self.kerf_preview_cache = None;
                self.record_recent_file(path.to_string());
                self.status = format!("opened {path}");
            }
            Err(error) => self.status = format!("open: {error}"),
        }
    }

    pub fn update_kerf_preview_cache(&mut self, doc: &stencilcraft_doc::Document) -> bool {
        let unchanged_geometry = self.kerf_preview_cache.as_ref().is_some_and(|cache| {
            cache.kerf_width_mm == doc.kerf.width_mm
                && cache.source.layers.len() == doc.layers.len()
                && cache
                    .source
                    .layers
                    .iter()
                    .zip(&doc.layers)
                    .all(|(old, new)| {
                        old.shapes.len() == new.shapes.len()
                            && old.shapes.iter().zip(&new.shapes).all(|(old, new)| {
                                old.id == new.id && old.style == new.style && old.path == new.path
                            })
                    })
        });

        if unchanged_geometry {
            if let Some(cache) = self.kerf_preview_cache.as_mut()
                && let Ok(preview) = &mut cache.preview
            {
                preview.board = doc.board.clone();
                preview.kerf = doc.kerf.clone();
                preview.next_shape_id = doc.next_shape_id;
                for (preview_layer, source_layer) in preview.layers.iter_mut().zip(&doc.layers) {
                    preview_layer.name.clone_from(&source_layer.name);
                    preview_layer.visible = source_layer.visible;
                }
            }
            return false;
        }
        let preview = stencilcraft_kerf::apply(doc, 4.0).map_err(|error| error.to_string());
        self.kerf_preview_cache = Some(KerfPreviewCache {
            source: doc.clone(),
            kerf_width_mm: doc.kerf.width_mm,
            preview,
        });
        true
    }

    pub fn set_selection(&mut self, ids: impl IntoIterator<Item = u64>) {
        let mut selected = Vec::new();
        for id in ids {
            if self.engine.doc.get_shape(id).is_some() && !selected.contains(&id) {
                selected.push(id);
            }
        }
        self.selected_shape = selected.last().copied();
        self.selected_shapes = selected;
    }

    pub fn select_latest_shape(&mut self) {
        if let Some(id) = self.engine.doc.next_shape_id.checked_sub(1) {
            self.set_selection([id]);
        }
    }

    pub fn retain_valid_selection(&mut self) {
        let ids = self.selected_shapes.clone();
        self.set_selection(ids);
    }

    pub fn select_all_shapes(&mut self) {
        let ids = self
            .engine
            .doc
            .shapes()
            .map(|(_, shape)| shape.id)
            .collect::<Vec<_>>();
        self.set_selection(ids);
    }

    pub fn duplicate_selection(&mut self) {
        let ids = self.selected_shapes.clone();
        match self.engine.run(Command::ShapesDuplicate { ids }) {
            Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) => {
                self.set_selection(ids);
                self.status = "duplicated in place".to_string();
            }
            Ok(_) => {}
            Err(error) => self.status = error.to_string(),
        }
    }

    pub fn delete_selection(&mut self) {
        if self.selected_shapes.is_empty() {
            return;
        }
        let ids = self.selected_shapes.clone();
        match self.engine.run(Command::ShapesRemove { ids }) {
            Ok(_) => {
                self.set_selection([]);
                self.status = "deleted".to_string();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    pub fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if self.send_mode || ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::A)) {
            self.select_all_shapes();
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F))
            || ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Num0))
        {
            self.pending_view_command = Some(CanvasViewCommand::Fit);
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::C)) {
            self.pending_view_command = Some(CanvasViewCommand::Center);
        } else if ctx
            .input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Num1))
        {
            self.view.zoom = 1.0;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::D)) {
            self.duplicate_selection();
        } else if ctx.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
        }) {
            self.delete_selection();
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            let _ = self.engine.undo();
            self.retain_valid_selection();
        } else if ctx.input_mut(|input| {
            input.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            ) || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
        }) {
            let _ = self.engine.redo();
            self.retain_valid_selection();
        } else if ctx.input_mut(|input| {
            input.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::R,
            )
        }) {
            self.prefs.show_rulers = !self.prefs.show_rulers;
            self.save_prefs();
        } else if ctx
            .input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Quote))
        {
            self.prefs.show_grid = !self.prefs.show_grid;
            self.save_prefs();
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.set_selection([]);
            self.path_selection = PathSelection::default();
            self.pen_draft = None;
            self.shape_drag_start = None;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::V)) {
            self.active_tool = ActiveTool::Select;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::A)) {
            self.active_tool = ActiveTool::DirectSelect;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::X)) {
            self.active_tool = ActiveTool::ConvertPoint;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::M)) {
            self.active_tool = ActiveTool::BoxSelect;
            self.canvas_lasso_mode = false;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::L)) {
            self.active_tool = ActiveTool::LassoSelect;
            self.canvas_lasso_mode = true;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::P)) {
            self.active_tool = ActiveTool::Pen;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::R)) {
            self.active_tool = ActiveTool::Rect;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::U)) {
            self.active_tool = ActiveTool::RoundedRect;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::O)) {
            self.active_tool = ActiveTool::Ellipse;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::G)) {
            self.active_tool = ActiveTool::Polygon;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::S)) {
            self.active_tool = ActiveTool::Star;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::T)) {
            self.active_tool = ActiveTool::Text;
            self.text_dialog = true;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::H)) {
            self.active_tool = ActiveTool::Hand;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Z)) {
            self.active_tool = ActiveTool::Zoom;
        }
    }

    pub fn run_pathfinder(&mut self, operation: BooleanOperation) {
        let ids = self.selected_shapes.clone();
        match self.engine.run(Command::ShapePathfinder { ids, operation }) {
            Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) => {
                self.set_selection(ids);
                self.status = "done".to_string();
            }
            Ok(_) => {}
            Err(error) => self.status = error.to_string(),
        }
    }

    pub fn add_imported_shapes(&mut self, imported: stencilcraft_doc::Document, label: &str) {
        let shapes = imported
            .layers
            .into_iter()
            .flat_map(|layer| {
                layer
                    .shapes
                    .into_iter()
                    .map(|shape| (shape.path, shape.style))
            })
            .collect::<Vec<_>>();
        let count = shapes.len();
        if count == 0 {
            self.status = format!("no cuttable paths found in {label}");
            return;
        }
        match self.engine.run(Command::ShapeAddPaths { shapes }) {
            Ok(_) => {
                self.select_latest_shape();
                self.status = format!("imported {count} paths from {label}");
            }
            Err(error) => self.status = format!("import failed: {error}"),
        }
    }

    pub fn import_svg(&mut self, path: std::path::PathBuf) {
        let name = path.to_string_lossy();
        let result = std::fs::read_to_string(&path)
            .map_err(|error| format!("{name}: could not read SVG: {error}"))
            .and_then(|svg| {
                stencilcraft_svg::import(&svg, &name).map_err(|error| error.to_string())
            });
        match result {
            Ok(imported) => self.add_imported_shapes(imported, &name),
            Err(error) => self.status = error,
        }
    }

    pub fn import_image(&mut self, path: std::path::PathBuf) {
        let name = path.to_string_lossy();
        let result = trace::trace_file(&name, trace::TraceOptions::default());
        match result {
            Ok(imported) => self.add_imported_shapes(imported, &name),
            Err(error) => self.status = error,
        }
    }

    pub fn add_text(&mut self) {
        let path = bitmap_text_path(&self.text_value);
        if path.elements().is_empty() {
            self.status = "enter some text first".to_string();
            return;
        }
        match self.engine.run(Command::ShapeAddPath {
            path,
            style: stencilcraft_doc::CutStyle::Engrave,
        }) {
            Ok(stencilcraft_engine::Effect::ShapeAdded { id }) => {
                let bounds = self
                    .engine
                    .doc
                    .get_shape(id)
                    .and_then(stencilcraft_doc::Shape::bounds);
                if let Some(bounds) = bounds {
                    self.run_command(Command::ShapeSetBounds {
                        id,
                        x: 20.0,
                        y: 20.0,
                        width: (cursor_text_width(&self.text_value) * 1.6).max(bounds.width()),
                        height: (7.0_f64 * 1.6).max(bounds.height()),
                    });
                }
                self.set_selection([id]);
            }
            Ok(_) => {}
            Err(error) => self.status = error.to_string(),
        }
    }

    pub fn text_dialog(&mut self, parent: &mut egui::Ui) {
        if !self.text_dialog {
            return;
        }
        let mut open = self.text_dialog;
        let mut add_text = false;
        let mut cancel = false;
        egui::Window::new("Place text as cut outlines")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(parent.ctx(), |ui| {
                ui.label("Places simple single-line engraved lettering (up to 80 characters).");
                let edit = TextField::new("place-text", "Text to place", &self.text_value)
                    .char_limit(80)
                    .show(ui);
                apply_edit(&mut self.text_value, &mut self.text_before, edit.event);
                ui.horizontal(|ui| {
                    if ui.add(Button::new("Cancel")).clicked() {
                        cancel = true;
                    }
                    if ui.add(Button::new("Add text")).clicked() {
                        add_text = true;
                    }
                });
            });
        self.text_dialog = open && !add_text && !cancel;
        if add_text {
            self.add_text();
        }
    }

    pub fn commit_pen_draft(&mut self) {
        if let Some(draft) = self.pen_draft.take() {
            let path = stencilcraft_ui_egui::edit_subpaths_to_bezpath(&[draft]);
            if !path.elements().is_empty() {
                self.run_command(Command::ShapeAddPath {
                    path,
                    style: CutStyle::Cut,
                });
                self.select_latest_shape();
                self.status = "created cut path with pen tool".to_string();
            }
        }
    }

    pub fn commit_dragged_shape(
        &mut self,
        start: stencilcraft_geom::Point,
        end: stencilcraft_geom::Point,
    ) {
        let x = start.x.min(end.x);
        let y = start.y.min(end.y);
        let w = (start.x - end.x).abs();
        let h = (start.y - end.y).abs();
        if w < 1.0 && h < 1.0 {
            return;
        }
        match self.active_tool {
            ActiveTool::Rect => {
                self.run_command(Command::ShapeAddRect {
                    x,
                    y,
                    width: w.max(2.0),
                    height: h.max(2.0),
                });
                self.select_latest_shape();
            }
            ActiveTool::RoundedRect => {
                self.run_command(Command::ShapeAddRoundedRect {
                    x,
                    y,
                    width: w.max(2.0),
                    height: h.max(2.0),
                    radius: (w.min(h) * 0.15).clamp(1.0, 15.0),
                });
                self.select_latest_shape();
            }
            ActiveTool::Ellipse => {
                self.run_command(Command::ShapeAddEllipse {
                    x,
                    y,
                    width: w.max(2.0),
                    height: h.max(2.0),
                });
                self.select_latest_shape();
            }
            ActiveTool::Polygon => {
                self.run_command(Command::ShapeAddPolygon {
                    cx: (start.x + end.x) / 2.0,
                    cy: (start.y + end.y) / 2.0,
                    radius: (w.max(h) / 2.0).max(2.0),
                    sides: 6,
                    rotation: 0.0,
                });
                self.select_latest_shape();
            }
            ActiveTool::Star => {
                self.run_command(Command::ShapeAddStar {
                    cx: (start.x + end.x) / 2.0,
                    cy: (start.y + end.y) / 2.0,
                    radius: (w.max(h) / 2.0).max(3.0),
                    inner_radius: (w.max(h) / 4.0).max(1.5),
                    points: 5,
                    rotation: 0.0,
                });
                self.select_latest_shape();
            }
            _ => {}
        }
    }

    fn build_menu_bar(&self) -> Vec<MenuItem<TopMenuAction>> {
        let mut menus = Vec::new();

        // 1. File
        let mut file_items = vec![
            MenuItem::new("New", TopMenuAction::NewDoc)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::N,
                )))
                .into(),
            MenuItem::new("Open…", TopMenuAction::OpenDoc)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::O,
                )))
                .into(),
        ];
        let mut recent_sub = Vec::new();
        if self.recent_files.is_empty() {
            recent_sub.push(MenuEntry::Item(
                MenuItem::new("No Recent Documents", TopMenuAction::NewDoc).enabled(false),
            ));
        } else {
            for (i, path) in self.recent_files.iter().enumerate() {
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .unwrap_or(path);
                recent_sub.push(MenuItem::new(name, TopMenuAction::OpenRecent(i)).into());
            }
        }
        file_items.push(MenuItem::submenu("Open Recent", recent_sub).into());
        file_items.push(MenuEntry::Separator);
        file_items.push(
            MenuItem::new("Save", TopMenuAction::SaveDoc)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::S,
                )))
                .into(),
        );
        file_items.push(
            MenuItem::new("Save As…", TopMenuAction::SaveAsDoc)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::S,
                )))
                .into(),
        );
        file_items.push(MenuEntry::Separator);
        file_items.push(MenuItem::new("Import SVG…", TopMenuAction::ImportSvg).into());
        file_items
            .push(MenuItem::new("Import Image Silhouette…", TopMenuAction::ImportImage).into());
        file_items.push(MenuEntry::Separator);
        file_items.push(MenuItem::new("Export SVG…", TopMenuAction::ExportSvg).into());
        file_items.push(
            MenuItem::new("Export Print & Cut SVG…", TopMenuAction::ExportPrintCutSvg).into(),
        );
        file_items.push(MenuItem::new("Export DXF (R14 ASCII)…", TopMenuAction::ExportDxf).into());
        file_items.push(MenuItem::new("Export G-code (Grbl)…", TopMenuAction::ExportGcode).into());
        file_items
            .push(MenuItem::new("Export Tiled SVG Sheets…", TopMenuAction::ExportTiledSvg).into());
        menus.push(MenuItem::submenu("File", file_items));

        // 2. Edit
        let edit_items = vec![
            MenuItem::new("Undo", TopMenuAction::Undo)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Z,
                )))
                .enabled(self.engine.can_undo())
                .into(),
            MenuItem::new("Redo", TopMenuAction::Redo)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                )))
                .enabled(self.engine.can_redo())
                .into(),
            MenuEntry::Separator,
            MenuItem::new("Duplicate Selected", TopMenuAction::Duplicate)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::D,
                )))
                .enabled(!self.selected_shapes.is_empty())
                .into(),
            MenuItem::new("Delete", TopMenuAction::Delete)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Backspace,
                )))
                .enabled(!self.selected_shapes.is_empty())
                .into(),
            MenuEntry::Separator,
            MenuItem::new("Select All", TopMenuAction::SelectAll)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::A,
                )))
                .into(),
            MenuItem::new("Deselect", TopMenuAction::Deselect)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::Escape,
                )))
                .enabled(!self.selected_shapes.is_empty())
                .into(),
        ];
        menus.push(MenuItem::submenu("Edit", edit_items));

        // 3. Object
        let has_mult = self.selected_shapes.len() >= 2;
        let object_items = vec![
            MenuItem::submenu(
                "Pathfinder",
                vec![
                    MenuItem::new("Unite", TopMenuAction::Pathfinder(BooleanOperation::Union))
                        .enabled(has_mult)
                        .into(),
                    MenuItem::new(
                        "Minus Front",
                        TopMenuAction::Pathfinder(BooleanOperation::Difference),
                    )
                    .enabled(has_mult)
                    .into(),
                    MenuItem::new(
                        "Intersect",
                        TopMenuAction::Pathfinder(BooleanOperation::Intersect),
                    )
                    .enabled(has_mult)
                    .into(),
                    MenuItem::new("Exclude", TopMenuAction::Pathfinder(BooleanOperation::Xor))
                        .enabled(has_mult)
                        .into(),
                    MenuItem::new(
                        "Divide",
                        TopMenuAction::Pathfinder(BooleanOperation::Divide),
                    )
                    .enabled(has_mult)
                    .into(),
                    MenuItem::new("Trim", TopMenuAction::Pathfinder(BooleanOperation::Trim))
                        .enabled(has_mult)
                        .into(),
                    MenuItem::new("Merge", TopMenuAction::Pathfinder(BooleanOperation::Merge))
                        .enabled(has_mult)
                        .into(),
                    MenuItem::new("Crop", TopMenuAction::Pathfinder(BooleanOperation::Crop))
                        .enabled(has_mult)
                        .into(),
                    MenuItem::new(
                        "Outline",
                        TopMenuAction::Pathfinder(BooleanOperation::Outline),
                    )
                    .enabled(has_mult)
                    .into(),
                    MenuItem::new(
                        "Minus Back",
                        TopMenuAction::Pathfinder(BooleanOperation::MinusBack),
                    )
                    .enabled(has_mult)
                    .into(),
                ],
            )
            .into(),
            MenuItem::submenu(
                "Align",
                vec![
                    MenuItem::new("Left", TopMenuAction::Align(Alignment::Left))
                        .enabled(has_mult)
                        .into(),
                    MenuItem::new(
                        "Horizontal Center",
                        TopMenuAction::Align(Alignment::HorizontalCenter),
                    )
                    .enabled(has_mult)
                    .into(),
                    MenuItem::new("Right", TopMenuAction::Align(Alignment::Right))
                        .enabled(has_mult)
                        .into(),
                    MenuItem::new("Top", TopMenuAction::Align(Alignment::Top))
                        .enabled(has_mult)
                        .into(),
                    MenuItem::new(
                        "Vertical Center",
                        TopMenuAction::Align(Alignment::VerticalCenter),
                    )
                    .enabled(has_mult)
                    .into(),
                    MenuItem::new("Bottom", TopMenuAction::Align(Alignment::Bottom))
                        .enabled(has_mult)
                        .into(),
                ],
            )
            .into(),
            MenuItem::submenu(
                "Distribute",
                vec![
                    MenuItem::new(
                        "Horizontal",
                        TopMenuAction::Distribute(DistributionAxis::Horizontal),
                    )
                    .enabled(has_mult)
                    .into(),
                    MenuItem::new(
                        "Vertical",
                        TopMenuAction::Distribute(DistributionAxis::Vertical),
                    )
                    .enabled(has_mult)
                    .into(),
                ],
            )
            .into(),
            MenuEntry::Separator,
            MenuItem::new("Rotate +15°", TopMenuAction::RotateCw15)
                .enabled(!self.selected_shapes.is_empty())
                .into(),
            MenuItem::new("Rotate −15°", TopMenuAction::RotateCcw15)
                .enabled(!self.selected_shapes.is_empty())
                .into(),
        ];
        menus.push(MenuItem::submenu("Object", object_items));

        // 4. View
        let view_items = vec![
            MenuItem::new("Fit to Sheet", TopMenuAction::FitView)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::F,
                )))
                .into(),
            MenuItem::new("Center Sheet", TopMenuAction::CenterView)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::NONE,
                    egui::Key::C,
                )))
                .into(),
            MenuItem::new("Actual Size (100%)", TopMenuAction::ActualSize)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Num1,
                )))
                .into(),
            MenuItem::new("Zoom In", TopMenuAction::ZoomIn)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Plus,
                )))
                .into(),
            MenuItem::new("Zoom Out", TopMenuAction::ZoomOut)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Minus,
                )))
                .into(),
            MenuEntry::Separator,
            MenuItem::new("Rulers", TopMenuAction::ToggleRulers)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::R,
                )))
                .check(self.prefs.show_rulers)
                .into(),
            MenuItem::new("Grid", TopMenuAction::ToggleGrid)
                .shortcut(Kbd::new(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Quote,
                )))
                .check(self.prefs.show_grid)
                .into(),
            MenuItem::new(
                "Preview Kerf Compensation",
                TopMenuAction::ToggleKerfPreview,
            )
            .check(self.kerf_preview)
            .into(),
            MenuItem::new("Print & Cut Marks", TopMenuAction::TogglePrintCutMarks)
                .check(self.prefs.print_cut_marks)
                .into(),
        ];
        menus.push(MenuItem::submenu("View", view_items));

        // 5. Window
        let window_items = vec![
            MenuItem::new("Reset Panels to Default", TopMenuAction::ResetWorkspace).into(),
            MenuEntry::Separator,
            MenuItem::new(
                "Dark Theme",
                TopMenuAction::SetTheme(stencilcraft_ui_egui::ColorTheme::Dark),
            )
            .radio(self.theme == stencilcraft_ui_egui::ColorTheme::Dark)
            .into(),
            MenuItem::new(
                "Light Theme",
                TopMenuAction::SetTheme(stencilcraft_ui_egui::ColorTheme::Light),
            )
            .radio(self.theme == stencilcraft_ui_egui::ColorTheme::Light)
            .into(),
        ];
        menus.push(MenuItem::submenu("Window", window_items));

        // 6. Help
        let help_items = vec![
            MenuItem::new("Documentation", TopMenuAction::HelpManual).into(),
            MenuItem::new("Keyboard Shortcuts", TopMenuAction::HelpCheatsheet).into(),
            MenuItem::new("About StencilCraft", TopMenuAction::HelpAbout).into(),
        ];
        menus.push(MenuItem::submenu("Help", help_items));

        menus
    }

    pub fn handle_menu_action(&mut self, action: TopMenuAction) {
        match action {
            TopMenuAction::NewDoc => {
                self.run_command(Command::DocumentNew {
                    width_mm: 100.0,
                    height_mm: 150.0,
                });
                self.set_selection([]);
                self.open_path = None;
            }
            TopMenuAction::OpenDoc => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("StencilCraft", &["stencilcraft"])
                    .pick_file()
                {
                    self.open_document(&path.to_string_lossy());
                }
            }
            TopMenuAction::OpenRecent(idx) => {
                if let Some(path) = self.recent_files.get(idx).cloned() {
                    self.open_document(&path);
                }
            }
            TopMenuAction::SaveDoc => {
                let pick = match &self.open_path {
                    Some(p) => Some(std::path::PathBuf::from(p)),
                    None => rfd::FileDialog::new()
                        .add_filter("StencilCraft", &["stencilcraft"])
                        .set_file_name("untitled.stencilcraft")
                        .save_file(),
                };
                if let Some(path) = pick {
                    self.save_to(path.to_string_lossy().into_owned());
                }
            }
            TopMenuAction::SaveAsDoc => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("StencilCraft", &["stencilcraft"])
                    .set_file_name(self.open_path.as_deref().unwrap_or("untitled.stencilcraft"))
                    .save_file()
                {
                    self.save_to(path.to_string_lossy().into_owned());
                }
            }
            TopMenuAction::ImportSvg => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .pick_file()
                {
                    self.import_svg(path);
                }
            }
            TopMenuAction::ImportImage => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG or JPEG", &["png", "jpg", "jpeg"])
                    .pick_file()
                {
                    self.import_image(path);
                }
            }
            TopMenuAction::ExportSvg => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name("export.svg")
                    .save_file()
                {
                    match stencilcraft_svg::export(&self.engine.doc) {
                        Ok(svg) => {
                            if let Err(e) = std::fs::write(&path, svg) {
                                self.status = format!("export failed: {e}");
                            } else {
                                self.status = format!("exported SVG to {}", path.display());
                            }
                        }
                        Err(e) => self.status = format!("SVG export error: {e}"),
                    }
                }
            }
            TopMenuAction::ExportPrintCutSvg => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name("print_and_cut.svg")
                    .save_file()
                {
                    match stencilcraft_svg::export_print_and_cut(&self.engine.doc) {
                        Ok(svg) => {
                            if let Err(e) = std::fs::write(&path, svg) {
                                self.status = format!("export failed: {e}");
                            } else {
                                self.status =
                                    format!("exported Print & Cut SVG to {}", path.display());
                            }
                        }
                        Err(e) => self.status = format!("Print & Cut SVG export error: {e}"),
                    }
                }
            }
            TopMenuAction::ExportDxf => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("DXF", &["dxf"])
                    .set_file_name("export.dxf")
                    .save_file()
                {
                    match stencilcraft_dxf::export(&self.engine.doc) {
                        Ok(dxf) => {
                            if let Err(e) = std::fs::write(&path, dxf) {
                                self.status = format!("export failed: {e}");
                            } else {
                                self.status = format!("exported DXF to {}", path.display());
                            }
                        }
                        Err(e) => self.status = format!("DXF export error: {e}"),
                    }
                }
            }
            TopMenuAction::ExportGcode => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("G-code", &["gcode", "nc"])
                    .set_file_name("export.gcode")
                    .save_file()
                {
                    match stencilcraft_gcode::export(
                        &self.engine.doc,
                        &stencilcraft_gcode::GcodeSettings::default(),
                    ) {
                        Ok(gcode) => {
                            if let Err(e) = std::fs::write(&path, gcode) {
                                self.status = format!("export failed: {e}");
                            } else {
                                self.status = format!("exported G-code to {}", path.display());
                            }
                        }
                        Err(e) => self.status = format!("G-code export error: {e}"),
                    }
                }
            }
            TopMenuAction::ExportTiledSvg => {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    // Sheets carry registration marks: tiled jobs are assembled by hand.
                    let options = stencilcraft_tiling::SheetOptions {
                        sheet_w: 300.0,
                        sheet_h: 300.0,
                        overlap_mm: 10.0,
                        marks: true,
                    };
                    match stencilcraft_tiling::tile_document_with(&self.engine.doc, &options) {
                        Ok(sheets) => {
                            let mut saved = 0;
                            for (i, sheet_doc) in sheets.iter().enumerate() {
                                let out_file = dir.join(format!("sheet_{:02}.svg", i + 1));
                                if let Ok(svg) = stencilcraft_svg::export(sheet_doc)
                                    && std::fs::write(&out_file, svg).is_ok()
                                {
                                    saved += 1;
                                }
                            }
                            // One page showing every sheet, so the job can be checked
                            // without opening the sheets one by one.
                            let preview_path = dir.join("preview.svg");
                            let preview = stencilcraft_svg::export_tile_preview(
                                &sheets,
                                &stencilcraft_svg::TilePreviewOptions::default(),
                            )
                            .and_then(|svg| {
                                std::fs::write(&preview_path, svg).map_err(|e| {
                                    stencilcraft_svg::SvgError::Write(format!(
                                        "{}: {e}",
                                        preview_path.display()
                                    ))
                                })
                            });
                            self.status = match preview {
                                Ok(()) => format!(
                                    "exported {saved} tiled sheets and preview.svg to {}",
                                    dir.display()
                                ),
                                Err(e) => format!(
                                    "exported {saved} tiled sheets to {} (preview failed: {e})",
                                    dir.display()
                                ),
                            };
                        }
                        Err(e) => self.status = format!("tiling error: {e}"),
                    }
                }
            }
            TopMenuAction::Undo => {
                let _ = self.engine.undo();
                self.retain_valid_selection();
            }
            TopMenuAction::Redo => {
                let _ = self.engine.redo();
                self.retain_valid_selection();
            }
            TopMenuAction::Duplicate => self.duplicate_selection(),
            TopMenuAction::Delete => self.delete_selection(),
            TopMenuAction::SelectAll => self.select_all_shapes(),
            TopMenuAction::Deselect => {
                self.set_selection([]);
                self.path_selection = PathSelection::default();
            }
            TopMenuAction::Pathfinder(op) => self.run_pathfinder(op),
            TopMenuAction::Align(alignment) => {
                self.run_command(Command::ShapesAlign {
                    ids: self.selected_shapes.clone(),
                    alignment,
                    to_board: false,
                });
            }
            TopMenuAction::Distribute(axis) => {
                self.run_command(Command::ShapesDistribute {
                    ids: self.selected_shapes.clone(),
                    axis,
                });
            }
            TopMenuAction::RotateCw15 => {
                if self.selected_shapes.len() > 1 {
                    self.run_command(Command::ShapesTransform {
                        ids: self.selected_shapes.clone(),
                        dx_mm: 0.0,
                        dy_mm: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        degrees: 15.0,
                    });
                } else if let Some(id) = self.selected_shape {
                    self.run_command(Command::ShapeRotate { id, degrees: 15.0 });
                }
            }
            TopMenuAction::RotateCcw15 => {
                if self.selected_shapes.len() > 1 {
                    self.run_command(Command::ShapesTransform {
                        ids: self.selected_shapes.clone(),
                        dx_mm: 0.0,
                        dy_mm: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        degrees: -15.0,
                    });
                } else if let Some(id) = self.selected_shape {
                    self.run_command(Command::ShapeRotate { id, degrees: -15.0 });
                }
            }
            TopMenuAction::FitView => self.pending_view_command = Some(CanvasViewCommand::Fit),
            TopMenuAction::CenterView => {
                self.pending_view_command = Some(CanvasViewCommand::Center)
            }
            TopMenuAction::ActualSize => self.view.zoom = 1.0,
            TopMenuAction::ZoomIn => self.view.zoom = (self.view.zoom * 1.25).clamp(0.05, 400.0),
            TopMenuAction::ZoomOut => self.view.zoom = (self.view.zoom / 1.25).clamp(0.05, 400.0),
            TopMenuAction::ToggleRulers => {
                self.prefs.show_rulers = !self.prefs.show_rulers;
                self.save_prefs();
            }
            TopMenuAction::ToggleGrid => {
                self.prefs.show_grid = !self.prefs.show_grid;
                self.save_prefs();
            }
            TopMenuAction::ToggleKerfPreview => self.kerf_preview = !self.kerf_preview,
            TopMenuAction::TogglePrintCutMarks => {
                self.prefs.print_cut_marks = !self.prefs.print_cut_marks;
                self.save_prefs();
            }
            TopMenuAction::ResetWorkspace => self.reset_workspace(),
            TopMenuAction::SetTheme(theme) => {
                self.theme = theme;
                self.save_prefs();
            }
            TopMenuAction::HelpManual
            | TopMenuAction::HelpCheatsheet
            | TopMenuAction::HelpAbout => {
                self.status = "StencilCraft: clean-room Rust cut preparation".to_string();
            }
        }
    }

    pub fn top_panel(&mut self, parent: &mut egui::Ui) {
        let doc_name = self
            .open_path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name()?.to_str())
            .unwrap_or("Untitled.stencilcraft");
        let title_text = format!("StencilCraft — {doc_name}");

        let menus = self.build_menu_bar();
        let mut chosen_action = None;
        let mut undo_clicked = false;
        let mut redo_clicked = false;
        let mut switch_theme_clicked = false;
        let mut reset_workspace_clicked = false;

        let frame = self.window_frame.clone();
        let can_undo = self.engine.can_undo();
        let can_redo = self.engine.can_redo();
        let (theme_icon, next_theme) = match self.theme {
            stencilcraft_ui_egui::ColorTheme::Dark => {
                (Icon::Sun, stencilcraft_ui_egui::ColorTheme::Light)
            }
            stencilcraft_ui_egui::ColorTheme::Light => {
                (Icon::Moon, stencilcraft_ui_egui::ColorTheme::Dark)
            }
        };

        egui::Panel::top("top-chrome")
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show(parent, |ui| {
                let resp = TitleBar::new(&frame)
                    .id_salt("stencilcraft-title-bar")
                    .separator(true)
                    .title(Title::new(title_text))
                    .show_titled(
                        ui,
                        |ui, _| {
                            if ui
                                .add(IconButton::new(Icon::Home, "Home / Fit View"))
                                .clicked()
                            {
                                chosen_action = Some(TopMenuAction::FitView);
                            }
                            let mb = MenuBar::new(&menus)
                                .id(egui::Id::unique("stencilcraft-menubar"))
                                .shrink_to_fit(f32::INFINITY)
                                .help_search(menus.len() - 1)
                                .show(ui);
                            chosen_action = chosen_action.or(mb.chosen);
                            LeadingEnd::from_ui(ui)
                        },
                        |ui, _| {
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt("workspace-switcher")
                                    .selected_text("Essentials")
                                    .width(88.0)
                                    .show_ui(ui, |ui| {
                                        if ui.selectable_label(true, "Essentials").clicked() {
                                            reset_workspace_clicked = true;
                                        }
                                        if ui.selectable_label(false, "Reset Workspace").clicked() {
                                            reset_workspace_clicked = true;
                                        }
                                    });
                                ui.separator();
                                if ui
                                    .add(IconButton::new(Icon::Undo, "Undo").enabled(can_undo))
                                    .clicked()
                                {
                                    undo_clicked = true;
                                }
                                if ui
                                    .add(IconButton::new(Icon::Redo, "Redo").enabled(can_redo))
                                    .clicked()
                                {
                                    redo_clicked = true;
                                }
                                ui.separator();
                                if ui
                                    .add(IconButton::new(theme_icon, "Switch Theme"))
                                    .clicked()
                                {
                                    switch_theme_clicked = true;
                                }
                            });
                        },
                    );

                if resp.close_requested {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

        if reset_workspace_clicked {
            self.reset_workspace();
        }

        if undo_clicked {
            let _ = self.engine.undo();
            self.retain_valid_selection();
        }
        if redo_clicked {
            let _ = self.engine.redo();
            self.retain_valid_selection();
        }
        if switch_theme_clicked {
            self.theme = next_theme;
            self.save_prefs();
        }
        if let Some(action) = chosen_action {
            self.handle_menu_action(action);
        }
    }

    pub fn options_bar(&mut self, parent: &mut egui::Ui) {
        let active_tool = self.active_tool;
        let count = self.selected_shapes.len();
        let selected_shape_id = self.selected_shape;
        let shape_bounds = if count == 1 {
            selected_shape_id.and_then(|id| {
                self.engine
                    .doc
                    .get_shape(id)
                    .and_then(stencilcraft_doc::Shape::bounds)
            })
        } else {
            None
        };
        let kerf_val_init = self.kerf_draft.unwrap_or(self.engine.doc.kerf.width_mm);
        let locale_numbers = self.locale_numbers.clone();
        let object_count = self.engine.doc.shape_count();

        let mut preview_kerf = self.kerf_preview;
        let mut show_rulers = self.prefs.show_rulers;
        let mut show_grid = self.prefs.show_grid;
        let mut send_mode = self.send_mode;
        let mut new_kerf = None;
        let mut do_unite = false;
        let mut do_minus = false;

        egui::Panel::top("options-bar")
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show(parent, |ui| {
                OptionsBar::new("stencilcraft-options").show(ui, |bar| {
                    bar.leading(|ui| {
                        OptionsBar::tool_icon(ui, active_tool.icon(), active_tool.label());
                    });
                    bar.body(|ui| {
                        if count == 0 {
                            ui.label("No selection");
                            ui.separator();
                            let mut kerf_val = kerf_val_init;
                            ui.label("Kerf:");
                            let edit =
                                NumberField::new("opt-kerf", "Kerf", kerf_val, &locale_numbers)
                                    .range(0.0..=2.0)
                                    .unit(Unit::new("{n} mm"))
                                    .show(ui);
                            if let Some(val) = edit.event.and_then(|e| e.value().copied()) {
                                kerf_val = val;
                                new_kerf = Some(kerf_val);
                            }
                            let _ = ui.add(
                                ToggleButton::new("Preview Kerf", &mut preview_kerf)
                                    .size(ControlSize::Sm),
                            );
                        } else if count == 1
                            && let Some(id) = selected_shape_id
                        {
                            ui.label(format!("Shape #{id}"));
                            if let Some(bounds) = shape_bounds {
                                ui.separator();
                                ui.label(format!("W: {:.2} mm", bounds.width()));
                                ui.label(format!("H: {:.2} mm", bounds.height()));
                            }
                        } else {
                            ui.label(format!("{count} shapes selected"));
                            if ui.add(Button::new("Unite").size(ControlSize::Sm)).clicked() {
                                do_unite = true;
                            }
                            if ui
                                .add(Button::new("Minus Front").size(ControlSize::Sm))
                                .clicked()
                            {
                                do_minus = true;
                            }
                        }
                    });
                    bar.trailing(|ui| {
                        let _ = ui.add(
                            ToggleButton::new("Rulers", &mut show_rulers)
                                .icon(Icon::Ruler)
                                .size(ControlSize::Sm),
                        );
                        let _ = ui.add(
                            ToggleButton::new("Grid", &mut show_grid)
                                .icon(Icon::Grid)
                                .size(ControlSize::Sm),
                        );
                        ui.separator();
                        ui.label(format!("{object_count} objects"));
                        let (mode_label, variant) = if send_mode {
                            ("← Design", ButtonVariant::Secondary)
                        } else {
                            ("Make →", ButtonVariant::Primary)
                        };
                        let btn = Button::new(mode_label)
                            .variant(variant)
                            .size(ControlSize::Sm);
                        if ui.add(btn).clicked() {
                            send_mode = !send_mode;
                        }
                    });
                });
            });

        self.kerf_preview = preview_kerf;
        self.prefs.show_rulers = show_rulers;
        self.prefs.show_grid = show_grid;
        self.send_mode = send_mode;
        if let Some(k) = new_kerf {
            self.run_command(Command::KerfSet { width_mm: k });
        }
        if do_unite {
            self.run_pathfinder(BooleanOperation::Union);
        }
        if do_minus {
            self.run_pathfinder(BooleanOperation::Difference);
        }
    }

    pub fn workflow_bar(&mut self, parent: &mut egui::Ui) {
        egui::Panel::top("workflow").show(parent, |ui| {
            ui.horizontal(|ui| {
                ui.heading(if self.send_mode { "Make" } else { "Design" });
                ui.separator();
                ui.label(format!("{} objects", self.engine.doc.shape_count()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (label, destination) = if self.send_mode {
                        ("← Design", false)
                    } else {
                        ("Make →", true)
                    };
                    if ui.add(Button::new(label)).clicked() {
                        self.send_mode = destination;
                    }
                    ui.selectable_value(&mut self.send_mode, false, "Canvas");
                });
            });
        });
    }

    pub fn tools_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.small("TOOLS");
            ui.separator();
            if ui.add(Button::new("Deselect")).clicked() {
                self.set_selection([]);
            }
            if !self.selected_shapes.is_empty()
                && ui.add(Button::new("Duplicate selected")).clicked()
            {
                self.duplicate_selection();
            }
            ui.selectable_value(&mut self.canvas_lasso_mode, false, "▱ Box select");
            ui.selectable_value(&mut self.canvas_lasso_mode, true, "⌁ Lasso select");
            if ui.add(Button::new("＋ Rectangle")).clicked() {
                self.run_command(Command::ShapeAddRect {
                    x: 20.0,
                    y: 20.0,
                    width: 40.0,
                    height: 30.0,
                });
                self.select_latest_shape();
            }
            if ui.add(Button::new("▧ Image")).clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG or JPEG", &["png", "jpg", "jpeg"])
                    .pick_file()
            {
                self.import_image(path);
            }
            if ui.add(Button::new("T Text")).clicked() {
                self.text_dialog = true;
            }
            if ui.add(Button::new("⊖ Delete")).clicked() {
                self.delete_selection();
            }
            ui.separator();
            if ui.add(Button::new("↶")).clicked() {
                let _ = self.engine.undo();
                self.retain_valid_selection();
            }
            if ui.add(Button::new("↷")).clicked() {
                let _ = self.engine.redo();
                self.retain_valid_selection();
            }
        });
    }

    pub fn layers_panel(&mut self, ui: &mut egui::Ui) {
        let mut add_layer = false;
        let mut delete_layer = false;
        let mut selected_idx = self.selected_layer;
        let mut toggles = Vec::new();

        let can_delete = self.engine.doc.layers.len() > 1;
        let layer_data: Vec<(usize, String, bool)> = self
            .engine
            .doc
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| (i, l.name.clone(), l.visible))
            .collect();

        Panel::new("stencilcraft-layers-panel")
            .footer_label("Layer actions")
            .footer(|ui| {
                PanelFooter::show(ui, |ui| {
                    if ui.add(IconButton::new(Icon::Plus, "Add Layer")).clicked() {
                        add_layer = true;
                    }
                    if ui
                        .add(IconButton::new(Icon::Trash, "Delete Layer").enabled(can_delete))
                        .clicked()
                    {
                        delete_layer = true;
                    }
                });
            })
            .show(ui, |ui| {
                for (i, name, visible) in layer_data {
                    let is_sel = selected_idx == Some(i);
                    let row = ListRow::new(i, &name)
                        .toggle(ToggleColumn::eye(format!("Visibility {name}"), visible))
                        .selected(is_sel)
                        .show(ui);
                    if row.response.clicked() {
                        selected_idx = Some(i);
                    }
                    for ev in row.events {
                        if let craft_ui::list::RowEvent::Toggle(
                            EditEvent::Commit { value, .. } | EditEvent::Set { value, .. },
                        ) = ev
                        {
                            toggles.push((value.row, value.on));
                        }
                    }
                }
            });

        self.selected_layer = selected_idx;
        if add_layer {
            self.run_command(Command::LayerAdd {
                name: format!("Layer {}", self.engine.doc.layers.len() + 1),
                kind: CutStyle::Cut,
            });
            self.selected_layer = Some(self.engine.doc.layers.len() - 1);
        }
        if delete_layer && let Some(idx) = self.selected_layer {
            self.run_command(Command::LayerRemove { index: idx });
            self.selected_layer = Some(idx.saturating_sub(1));
        }
        for (idx, vis) in toggles {
            self.run_command(Command::LayerSetVisible {
                index: idx,
                visible: vis,
            });
        }
    }

    pub fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            if self.send_mode {
                ui.heading("Make");
                ui.small("Review your material and machine settings before cutting.");
                ui.separator();
                self.send_tab(ui);
                if ui.add(Button::new("← Back to canvas")).clicked() {
                    self.send_mode = false;
                }
            } else {
                self.inspector(ui);
            }
        });
    }

    pub fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Design");
        ui.small("Layers & objects");
        ui.add_space(8.0);

        ui.label("Cutting mat");
        let mut mat_select = Select::new("design-mat", "Cutting mat", &mut self.mat);
        for (mat, label) in MAT_CHOICES {
            mat_select = mat_select.option(*mat, *label);
        }
        let _ = mat_select.show(ui);

        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Layers");
            if ui.small_button("+").clicked() {
                self.run_command(Command::LayerAdd {
                    name: format!("Layer {}", self.engine.doc.layers.len() + 1),
                    kind: stencilcraft_doc::CutStyle::Cut,
                });
            }
        });
        let visibility_changes: Vec<(usize, bool)> = self
            .engine
            .doc
            .layers
            .iter()
            .enumerate()
            .filter_map(|(i, layer)| {
                let mut visible = layer.visible;
                let response = Checkbox::bool(
                    format!("{}  ·  {}", layer.name, layer.shapes.len()),
                    &mut visible,
                )
                .id_salt(("layer-visibility", i))
                .show(ui);
                response.response.changed().then_some((i, visible))
            })
            .collect();
        for (i, visible) in visibility_changes {
            if let Some(layer) = self.engine.doc.layers.get_mut(i) {
                layer.visible = visible;
            }
        }

        ui.separator();
        ui.heading("Objects");
        let shape_rows: Vec<(u64, String)> = self
            .engine
            .doc
            .shapes()
            .map(|(_, shape)| {
                (
                    shape.id,
                    format!("Shape  #{} · {:?}", shape.id, shape.style),
                )
            })
            .collect();
        for (id, label) in shape_rows {
            if ui
                .selectable_label(self.selected_shapes.contains(&id), label)
                .clicked()
            {
                let additive = ui.input(|input| input.modifiers.shift);
                if additive {
                    let mut ids = self.selected_shapes.clone();
                    if let Some(index) = ids.iter().position(|selected| *selected == id) {
                        ids.remove(index);
                    } else {
                        ids.push(id);
                    }
                    self.set_selection(ids);
                } else {
                    self.set_selection([id]);
                }
            }
        }

        if self.selected_shapes.len() >= 2 {
            ui.separator();
            ui.heading(format!("{} selected", self.selected_shapes.len()));
            ui.horizontal_wrapped(|ui| {
                if ui.add(Button::new("Unite")).clicked() {
                    self.run_pathfinder(BooleanOperation::Union);
                }
                if ui.add(Button::new("Minus front")).clicked() {
                    self.run_pathfinder(BooleanOperation::Difference);
                }
                if ui.add(Button::new("Intersect")).clicked() {
                    self.run_pathfinder(BooleanOperation::Intersect);
                }
                if ui.add(Button::new("Exclude")).clicked() {
                    self.run_pathfinder(BooleanOperation::Xor);
                }
                if ui.add(Button::new("Divide")).clicked() {
                    self.run_pathfinder(BooleanOperation::Divide);
                }
                if ui.add(Button::new("Trim")).clicked() {
                    self.run_pathfinder(BooleanOperation::Trim);
                }
                if ui.add(Button::new("Merge")).clicked() {
                    self.run_pathfinder(BooleanOperation::Merge);
                }
                if ui.add(Button::new("Crop")).clicked() {
                    self.run_pathfinder(BooleanOperation::Crop);
                }
                if ui.add(Button::new("Outline")).clicked() {
                    self.run_pathfinder(BooleanOperation::Outline);
                }
                if ui.add(Button::new("Minus back")).clicked() {
                    self.run_pathfinder(BooleanOperation::MinusBack);
                }
            });
            ui.label("Align to selection");
            ui.horizontal_wrapped(|ui| {
                for (label, alignment) in [
                    ("Left", Alignment::Left),
                    ("H center", Alignment::HorizontalCenter),
                    ("Right", Alignment::Right),
                    ("Top", Alignment::Top),
                    ("V center", Alignment::VerticalCenter),
                    ("Bottom", Alignment::Bottom),
                ] {
                    if ui.add(Button::new(label)).clicked() {
                        self.run_command(Command::ShapesAlign {
                            ids: self.selected_shapes.clone(),
                            alignment,
                            to_board: false,
                        });
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.add(Button::new("Distribute ↔")).clicked() {
                    self.run_command(Command::ShapesDistribute {
                        ids: self.selected_shapes.clone(),
                        axis: DistributionAxis::Horizontal,
                    });
                }
                if ui.add(Button::new("Distribute ↕")).clicked() {
                    self.run_command(Command::ShapesDistribute {
                        ids: self.selected_shapes.clone(),
                        axis: DistributionAxis::Vertical,
                    });
                }
                if ui.add(Button::new("−15°")).clicked() {
                    self.run_command(Command::ShapesTransform {
                        ids: self.selected_shapes.clone(),
                        dx_mm: 0.0,
                        dy_mm: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        degrees: -15.0,
                    });
                }
                if ui.add(Button::new("+15°")).clicked() {
                    self.run_command(Command::ShapesTransform {
                        ids: self.selected_shapes.clone(),
                        dx_mm: 0.0,
                        dy_mm: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                        degrees: 15.0,
                    });
                }
            });
        }

        ui.separator();
        ui.heading("Cut settings");
        let kerf_width = self.kerf_draft.unwrap_or(self.engine.doc.kerf.width_mm);
        ui.label(format!("Kerf  {:.3} mm", kerf_width));
        match Slider::new("kerf", "Kerf", kerf_width, 0.0..=1.0)
            .show(ui)
            .event
        {
            Some(EditEvent::Begin { .. }) => self.kerf_draft = Some(kerf_width),
            Some(EditEvent::Update { value, .. }) => self.kerf_draft = Some(value),
            Some(EditEvent::Commit { value, changed, .. }) => {
                self.kerf_draft = None;
                if changed {
                    self.run_command(Command::KerfSet { width_mm: value });
                }
            }
            Some(EditEvent::Set { value, .. }) => {
                self.run_command(Command::KerfSet { width_mm: value })
            }
            Some(EditEvent::Cancel { .. }) => self.kerf_draft = None,
            _ => {}
        }
        let _ = Checkbox::bool("Preview kerf compensation", &mut self.kerf_preview).show(ui);

        if let Some(id) = (self.selected_shapes.len() == 1)
            .then_some(self.selected_shape)
            .flatten()
        {
            if self.engine.doc.get_shape(id).is_some() {
                ui.separator();
                ui.heading(format!("Object #{id}"));
                let bounds = self
                    .engine
                    .doc
                    .get_shape(id)
                    .and_then(stencilcraft_doc::Shape::bounds);
                let bound_values =
                    bounds.map(|bounds| [bounds.x0, bounds.y0, bounds.width(), bounds.height()]);
                if self.inspector_shape != Some(id)
                    || (self.inspector_bounds != bound_values && !self.inspector_editing)
                {
                    self.inspector_shape = Some(id);
                    self.inspector_before = std::array::from_fn(|_| None);
                    self.inspector_bounds = bound_values;
                    if let Some(values) = bound_values {
                        self.inspector_values = values.map(|value| format!("{value:.2}"));
                    }
                }
                ui.label("Position and size (mm)");
                let mut submitted = false;
                let mut editing = false;
                egui::Grid::new(("shape-bounds", id))
                    .num_columns(4)
                    .show(ui, |ui| {
                        for (label_a, index_a, label_b, index_b) in
                            [("X", 0, "Y", 1), ("W", 2, "H", 3)]
                        {
                            for (label, index) in [(label_a, index_a), (label_b, index_b)] {
                                ui.label(label);
                                let edit = ui
                                    .scope(|ui| {
                                        ui.set_max_width(62.0);
                                        TextField::new(
                                            ("bound", id, index),
                                            label,
                                            &self.inspector_values[index],
                                        )
                                        .show(ui)
                                    })
                                    .inner;
                                editing |= edit.response.has_focus();
                                submitted |= edit.submitted;
                                apply_edit(
                                    &mut self.inspector_values[index],
                                    &mut self.inspector_before[index],
                                    edit.event,
                                );
                            }
                            ui.end_row();
                        }
                    });
                self.inspector_editing = editing;
                submitted |= ui.add(Button::new("Apply")).clicked();
                let valid_bounds = self
                    .inspector_values
                    .iter()
                    .map(|value| value.trim().parse::<f64>())
                    .collect::<Result<Vec<_>, _>>();
                if submitted {
                    match valid_bounds {
                        Ok(values) if values.len() == 4 => {
                            self.run_command(Command::ShapeSetBounds {
                                id,
                                x: values[0],
                                y: values[1],
                                width: values[2],
                                height: values[3],
                            });
                            if let Some(bounds) = self
                                .engine
                                .doc
                                .get_shape(id)
                                .and_then(stencilcraft_doc::Shape::bounds)
                            {
                                let values =
                                    [bounds.x0, bounds.y0, bounds.width(), bounds.height()];
                                self.inspector_values = values.map(|value| format!("{value:.2}"));
                                self.inspector_bounds = Some(values);
                            }
                        }
                        _ => self.status = "enter valid X, Y, width, and height in mm".to_string(),
                    }
                }
                ui.horizontal(|ui| {
                    if ui.add(Button::new("−15°")).clicked() {
                        self.run_command(Command::ShapeRotate { id, degrees: -15.0 });
                    }
                    if ui.add(Button::new("+15°")).clicked() {
                        self.run_command(Command::ShapeRotate { id, degrees: 15.0 });
                    }
                });
                ui.horizontal(|ui| {
                    if ui.add(Button::new("−10%")).clicked() {
                        self.run_command(Command::ShapeScale {
                            id,
                            scale_x: 0.9,
                            scale_y: 0.9,
                        });
                    }
                    if ui.add(Button::new("+10%")).clicked() {
                        self.run_command(Command::ShapeScale {
                            id,
                            scale_x: 1.1,
                            scale_y: 1.1,
                        });
                    }
                });
                ui.small("Drag an object on the mat to reposition it.");
            } else {
                self.set_selection([]);
            }
        }
    }

    pub fn send_tab(&mut self, ui: &mut egui::Ui) {
        ui.label("Model");
        let mut model_select = Select::new("send-model", "Model", &mut self.send.model);
        for (model, label) in MODEL_CHOICES {
            model_select = model_select.option(*model, *label);
        }
        let _ = model_select.show(ui);
        ui.label("Material");
        let mut media_select = Select::new("send-material", "Material", &mut self.send.media_index);
        for (i, _) in stencilcraft_machine::MEDIA_PRESETS.iter().enumerate() {
            media_select = media_select.option(i, media_label(i));
        }
        if let Some(&index) = media_select.show(ui).committed()
            && let Some(media) = stencilcraft_machine::MEDIA_PRESETS.get(index)
        {
            self.send.pressure_override = media.pressure.unwrap_or(10);
            self.send.speed_override = media.speed.unwrap_or(5);
        }
        ui.add_space(4.0);
        ui.label(format!("Pressure  {}", self.send.pressure_override));
        let mut pressure = f64::from(self.send.pressure_override);
        let edit = Slider::new("send-pressure", "Pressure", pressure, 1.0..=33.0)
            .step(1.0)
            .show(ui);
        if apply_edit(&mut pressure, &mut self.pressure_before, edit.event) {
            self.send.pressure_override = pressure as u8;
        }
        ui.label(format!("Speed  {}", self.send.speed_override));
        let mut speed = f64::from(self.send.speed_override);
        let edit = Slider::new("send-speed", "Speed", speed, 1.0..=30.0)
            .step(1.0)
            .show(ui);
        if apply_edit(&mut speed, &mut self.speed_before, edit.event) {
            self.send.speed_override = speed as u8;
        }
        let _ = Checkbox::bool("Bounding box preview", &mut self.send.bbox_only).show(ui);
        ui.separator();
        if ui.add(Button::new("Encode dry run")).clicked() {
            self.send.summary = Some(self.encode_send_summary());
        }
        if let Some(summary) = &self.send.summary {
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for line in summary {
                    ui.monospace(line);
                }
            });
        }
        ui.add_space(6.0);
        ui.small("Real sends: stencilcraft-cli cut --in FILE --device PATH");
    }

    /// Encode the visible flattened design through the machine crate and
    /// build the summary lines shown in the Send tab (always a dry run).
    pub fn encode_send_summary(&self) -> Vec<String> {
        let mut out = Vec::new();
        let paths = machine_paths(&self.engine.doc);
        if paths.is_empty() {
            out.push("no visible shapes to cut".to_string());
            return out;
        }
        let media = stencilcraft_machine::MEDIA_PRESETS
            .get(self.send.media_index)
            .copied()
            .unwrap_or(stencilcraft_machine::Media {
                id: 300,
                name: "Custom",
                pressure: None,
                speed: None,
                depth: None,
            });
        let mut settings = stencilcraft_machine::CutSettings::from_media(&media);
        settings.pressure = Some(self.send.pressure_override);
        settings.speed = Some(self.send.speed_override);
        settings.mat = self.mat;
        let program = stencilcraft_machine::encode_cut(
            self.send.model,
            &settings,
            &paths,
            &stencilcraft_machine::PlotOptions::default(),
            self.send.bbox_only,
        );
        out.push(format!(
            "{} commands, {} bytes for {}",
            program.commands.len(),
            program.to_stream().len(),
            model_label(self.send.model),
        ));
        if let Some((sd, sr, ed, er)) = program.bbox_mm {
            out.push(format!(
                "cut area {:.0} x {:.0} mm ({} mat)",
                er - sr,
                ed - sd,
                mat_label(self.mat),
            ));
        }
        out.push("program:".to_string());
        for command in program.commands.iter().take(80) {
            out.push(command.to_ascii());
        }
        if program.commands.len() > 80 {
            out.push(format!("… {} more", program.commands.len() - 80));
        }
        out
    }

    pub fn canvas_body(&mut self, ui: &mut egui::Ui) {
        let mut doc = self.engine.doc.clone();
        if let Some(width_mm) = self.kerf_draft {
            doc.kerf.width_mm = width_mm;
        }
        let backdrop = mat_backdrop(self.mat);
        let mut zoom_delta = 0.0_f32;
        let mut requested_zoom = None;
        let mut zoom_value = f64::from((self.view.zoom * 100.0).round().clamp(5.0, 40000.0));
        let mut fit_clicked = false;
        let mut center_clicked = false;
        ui.horizontal(|ui| {
            if ui.add(Button::new("Fit")).clicked() {
                fit_clicked = true;
            }
            if ui.add(Button::new("Center")).clicked() {
                center_clicked = true;
            }
            if ui.add(IconButton::new(Icon::Minus, "Zoom out")).clicked() {
                zoom_delta = -1.0;
            }
            let slider_width = (ui.available_width() - 112.0).clamp(56.0, 180.0);
            let edit = Slider::new("canvas-zoom", "Zoom", zoom_value, 5.0..=40000.0)
                .scale(SliderScale::Log)
                .step(1.0)
                .width(slider_width)
                .show(ui);
            if apply_edit(&mut zoom_value, &mut self.zoom_before, edit.event) {
                requested_zoom = Some((zoom_value as f32 / 100.0).clamp(0.05, 400.0));
            }
            ui.small(format!("{zoom_value:.0}%"));
            if ui.add(IconButton::new(Icon::Plus, "Zoom in")).clicked() {
                zoom_delta = 1.0;
            }
        });
        ui.separator();

        let full_rect = ui.available_rect_before_wrap();
        let tokens = <craft_ui::theme::Tokens as craft_ui::theme::TokensOf>::of(ui);
        let clip = if self.prefs.show_rulers {
            Rulers::content_rect(&tokens, full_rect)
        } else {
            full_rect
        };

        let mut view_command = self.pending_view_command.take();
        // A new board size (document setup, New, an agent's `document.set-board`) leaves
        // the old zoom and pan pointing at where the previous sheet was: re-frame it.
        if self.note_board_size(doc.board.width_mm, doc.board.height_mm) && self.view_initialized {
            view_command = Some(CanvasViewCommand::Fit);
        }
        if fit_clicked {
            view_command = Some(CanvasViewCommand::Fit);
        } else if center_clicked {
            view_command = Some(CanvasViewCommand::Center);
        }
        let extent_width = backdrop.width_mm.max(doc.board.width_mm);
        let extent_height = backdrop.height_mm.max(doc.board.height_mm);
        if !self.view_initialized {
            self.view
                .fit_extent(extent_width, extent_height, clip, 24.0);
            self.view_initialized = true;
        }
        if let Some(command) = view_command {
            apply_canvas_view_command(&mut self.view, command, extent_width, extent_height, clip);
        }
        if let Some(zoom) = requested_zoom {
            self.view.zoom_at(zoom / self.view.zoom, clip.center());
        }
        if zoom_delta != 0.0 {
            self.view.zoom_at(1.2_f32.powf(zoom_delta), clip.center());
        }

        let response = ui.allocate_rect(clip, egui::Sense::click_and_drag());
        let preview_updated = self.kerf_preview && self.update_kerf_preview_cache(&doc);
        if preview_updated
            && let Some(Err(error)) = self.kerf_preview_cache.as_ref().map(|cache| &cache.preview)
        {
            self.status = format!("kerf preview: {error}");
        }
        let paint_doc = if self.kerf_preview {
            self.kerf_preview_cache
                .as_ref()
                .and_then(|cache| cache.preview.as_ref().ok())
                .unwrap_or(&doc)
        } else {
            &doc
        };
        let view = self.view;
        let painter = ui.painter_at(clip);
        let selection = stencilcraft_ui_egui::SelectionState {
            selected_shapes: self.selected_shapes.clone(),
            overlay: self
                .canvas_selection_start
                .zip(ui.ctx().pointer_interact_pos())
                .map(|(start, end)| {
                    if self.canvas_lasso_mode {
                        let mut points = self.canvas_selection_points.clone();
                        if points.last().is_none_or(|last| *last != end) {
                            points.push(end);
                        }
                        stencilcraft_ui_egui::SelectionOverlay::Lasso(points)
                    } else {
                        stencilcraft_ui_egui::SelectionOverlay::Box(egui::Rect::from_two_pos(
                            start, end,
                        ))
                    }
                }),
        };
        stencilcraft_ui_egui::paint_document(
            &painter,
            clip,
            paint_doc,
            &view,
            Some(&backdrop),
            stencilcraft_ui_egui::CanvasOptions {
                kerf_preview: self.kerf_preview,
                show_grid: self.prefs.show_grid,
                print_cut_marks: self.prefs.print_cut_marks,
                show_selection_bounds: true,
                theme: self.theme,
            },
            &selection,
        );

        let canvas_view = self.view.to_canvas_view(clip);
        let mm_units = RulerUnits {
            per_doc: 1.0,
            ticks: craft_ui::canvas::Ticks::Decimal { whole: false },
            numbers: self.locale_numbers.clone(),
            unit: Unit::new("{n} mm"),
            decimals: 1,
        };

        if self.prefs.show_rulers {
            let r = Rulers::new(
                "stencil-rulers",
                &mm_units,
                &mm_units,
                RulerNames::new("Horizontal ruler", "Vertical ruler", "Ruler origin"),
            )
            .zero(self.ruler_origin)
            .show(ui, full_rect, &canvas_view, None);
            if r.reset_origin {
                self.ruler_origin = stencilcraft_geom::Point::ZERO;
            }
            if let Some(EditEvent::Commit { value, .. } | EditEvent::Set { value, .. }) = r.origin {
                self.ruler_origin = value;
            }
        }

        let mut overlay_claimed = false;

        // Interactive BezierOverlay for node editing
        if matches!(
            self.active_tool,
            ActiveTool::DirectSelect | ActiveTool::ConvertPoint
        ) {
            let b_tool = if self.active_tool == ActiveTool::ConvertPoint {
                BezierTool::Convert
            } else {
                BezierTool::Direct
            };
            let edit_paths: Vec<EditPath> = self
                .selected_shapes
                .iter()
                .filter_map(|&id| {
                    let shape = self.engine.doc.get_shape(id)?;
                    let subpaths = stencilcraft_ui_egui::bezpath_to_edit_subpaths(&shape.path);
                    Some(EditPath {
                        key: id,
                        subpaths,
                        color: None,
                    })
                })
                .collect();

            let b_resp = BezierOverlay::new(
                "stencil-bezier-overlay",
                &edit_paths,
                &self.path_selection,
                &mm_units,
                &mm_units,
                BezierNames::new(
                    "Paths",
                    "In handle",
                    "Out handle",
                    "Select path nodes",
                    |i, n| format!("Anchor {i} of {n}"),
                ),
            )
            .tool(b_tool)
            .handles(self.handles_shown)
            .show(ui, &response, &canvas_view);

            overlay_claimed = b_resp.claimed;
            if let Some(sel) = b_resp.select {
                if let Some(pid) = sel.path {
                    self.selected_shape = Some(pid);
                    self.selected_shapes = vec![pid];
                }
                self.path_selection = sel;
            }
            if let Some(ev) = b_resp.event {
                match ev {
                    EditEvent::Begin { .. } => {}
                    EditEvent::Update { value, .. } => {
                        let pid = value.path();
                        if self.bezier_gesture_base.is_none()
                            && let Some(orig) = self.engine.doc.get_shape(pid)
                        {
                            let base = stencilcraft_ui_egui::bezpath_to_edit_subpaths(&orig.path);
                            self.bezier_gesture_base = Some((pid, base));
                        }
                        if let Some((base_id, ref base)) = self.bezier_gesture_base
                            && base_id == pid
                        {
                            let edited = stencilcraft_ui_egui::apply_path_edit(base, &value);
                            self.bezier_live_path = Some((
                                pid,
                                stencilcraft_ui_egui::edit_subpaths_to_bezpath(&edited),
                            ));
                        }
                    }
                    EditEvent::Commit { value, .. } | EditEvent::Set { value, .. } => {
                        let pid = value.path();
                        let base = self
                            .bezier_gesture_base
                            .take()
                            .filter(|(id, _)| *id == pid)
                            .map(|(_, b)| b)
                            .or_else(|| {
                                self.engine.doc.get_shape(pid).map(|s| {
                                    stencilcraft_ui_egui::bezpath_to_edit_subpaths(&s.path)
                                })
                            });
                        self.bezier_live_path = None;
                        if let Some(base) = base {
                            let edited = stencilcraft_ui_egui::apply_path_edit(&base, &value);
                            let path = stencilcraft_ui_egui::edit_subpaths_to_bezpath(&edited);
                            if !path.elements().is_empty() {
                                self.run_command(Command::ShapeSetPath { id: pid, path });
                            }
                        }
                    }
                    EditEvent::Cancel { .. } => {
                        self.bezier_gesture_base = None;
                        self.bezier_live_path = None;
                    }
                    _ => {}
                }
            }
            if let Some((pid, seg, t)) = b_resp.insert
                && let Some(s) = self.engine.doc.get_shape(pid)
            {
                let mut subs = stencilcraft_ui_egui::bezpath_to_edit_subpaths(&s.path);
                if let Some(new_knot) = stencilcraft_ui_egui::insert_path_knot(&mut subs, seg, t) {
                    let path = stencilcraft_ui_egui::edit_subpaths_to_bezpath(&subs);
                    self.run_command(Command::ShapeSetPath { id: pid, path });
                    self.path_selection = PathSelection {
                        path: Some(pid),
                        anchors: vec![new_knot],
                        segment: None,
                    };
                }
            }
            if let Some((pid, knots)) = b_resp.delete
                && let Some(s) = self.engine.doc.get_shape(pid)
            {
                let mut subs = stencilcraft_ui_egui::bezpath_to_edit_subpaths(&s.path);
                stencilcraft_ui_egui::delete_path_knots(&mut subs, &knots);
                let path = stencilcraft_ui_egui::edit_subpaths_to_bezpath(&subs);
                if !path.elements().is_empty() {
                    self.run_command(Command::ShapeSetPath { id: pid, path });
                    self.path_selection.anchors.clear();
                } else {
                    self.run_command(Command::ShapeRemove { id: pid });
                    self.path_selection = PathSelection::default();
                }
            }
        }

        // Interactive PenDraft for Pen tool
        if self.active_tool == ActiveTool::Pen {
            let pen_resp = PenDraft::new(
                "stencil-pen-draft",
                self.pen_draft.as_ref(),
                PenNames::new("Cut path being drawn", |i, n| format!("Anchor {i} of {n}")),
            )
            .units(&mm_units, &mm_units)
            .show(ui, &response, &canvas_view);

            overlay_claimed = overlay_claimed || pen_resp.claimed;
            if let Some(ev) = pen_resp.event {
                match ev {
                    EditEvent::Begin { .. } => {}
                    EditEvent::Update { value, .. } => {
                        self.pen_draft = Some(value);
                    }
                    EditEvent::Commit { value, .. } | EditEvent::Set { value, .. } => {
                        self.pen_draft = Some(value);
                        self.commit_pen_draft();
                    }
                    EditEvent::Cancel { .. } => {
                        self.pen_draft = None;
                    }
                    _ => {}
                }
            } else if pen_resp.finished {
                self.commit_pen_draft();
            }
        }

        // Live shape creation via drag
        if self.active_tool.is_shape_creator() && !overlay_claimed {
            if response.drag_started()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let world_pt = self.view.screen_to_world(pointer.to_vec2());
                self.shape_drag_start = Some(world_pt);
                self.shape_drag_current = Some(world_pt);
            }
            if response.dragged()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let world_pt = self.view.screen_to_world(pointer.to_vec2());
                self.shape_drag_current = Some(world_pt);
                if let Some(start) = self.shape_drag_start {
                    let w = (world_pt.x - start.x).abs();
                    let h = (world_pt.y - start.y).abs();
                    Readout::new("shape-readout")
                        .row("W", format!("{w:.1} mm"))
                        .row("H", format!("{h:.1} mm"))
                        .show(ui.ctx(), pointer);
                }
            }
            if response.drag_stopped()
                && let (Some(start), Some(end)) =
                    (self.shape_drag_start.take(), self.shape_drag_current.take())
            {
                self.commit_dragged_shape(start, end);
            }
        }

        let (space_down, middle_down, primary_down, pointer_delta) = ui.ctx().input(|input| {
            (
                input.key_down(egui::Key::Space),
                input.pointer.middle_down(),
                input.pointer.primary_down(),
                input.pointer.delta(),
            )
        });
        let pan_requested = middle_down
            || (space_down && primary_down)
            || (self.active_tool == ActiveTool::Hand && primary_down);
        if pan_requested && response.hovered() {
            self.canvas_panning = true;
            self.canvas_drag_shape = None;
        } else if !pan_requested {
            self.canvas_panning = false;
        }

        if self.canvas_panning {
            if pointer_delta != egui::Vec2::ZERO {
                self.view.pan_mm -= pointer_delta / self.view.zoom;
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if !overlay_claimed && !self.active_tool.is_shape_creator() {
            if response.drag_started()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let hit = stencilcraft_ui_egui::hit_test_shape(&doc, &view, pointer, 8.0);
                self.canvas_selection_additive = ui.ctx().input(|input| input.modifiers.shift);
                self.canvas_drag_shape = if self.canvas_lasso_mode || self.canvas_selection_additive
                {
                    None
                } else {
                    hit.filter(|id| self.selected_shapes.contains(id))
                };
                if self.canvas_lasso_mode {
                    self.canvas_selection_start = Some(pointer);
                    self.canvas_selection_points.clear();
                    self.canvas_selection_points.push(pointer);
                    if !self.canvas_selection_additive {
                        self.set_selection([]);
                    }
                } else if let Some(id) = hit {
                    if self.canvas_selection_additive {
                        let mut ids = self.selected_shapes.clone();
                        if let Some(index) = ids.iter().position(|selected| *selected == id) {
                            ids.remove(index);
                        } else {
                            ids.push(id);
                        }
                        self.set_selection(ids);
                    } else if !self.selected_shapes.contains(&id) {
                        self.set_selection([id]);
                        self.canvas_drag_shape = Some(id);
                    }
                } else {
                    self.canvas_selection_start = Some(pointer);
                    if !self.canvas_selection_additive {
                        self.set_selection([]);
                    }
                }
                self.last_drag_delta = egui::Vec2::ZERO;
            } else if response.clicked()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let hit = stencilcraft_ui_egui::hit_test_shape(&doc, &view, pointer, 8.0);
                if ui.ctx().input(|input| input.modifiers.shift) {
                    let mut ids = self.selected_shapes.clone();
                    if let Some(id) = hit {
                        if let Some(index) = ids.iter().position(|selected| *selected == id) {
                            ids.remove(index);
                        } else {
                            ids.push(id);
                        }
                    }
                    self.set_selection(ids);
                } else {
                    self.set_selection(hit);
                }
            }

            if response.dragged() && !self.canvas_panning {
                let total_delta = response.drag_delta();
                let delta = total_delta - self.last_drag_delta;
                self.last_drag_delta = total_delta;
                if delta != egui::Vec2::ZERO {
                    if let Some(id) = self.canvas_drag_shape {
                        if self.selected_shapes.len() > 1 {
                            self.run_command(Command::ShapesTransform {
                                ids: self.selected_shapes.clone(),
                                dx_mm: f64::from(delta.x / self.view.zoom),
                                dy_mm: f64::from(delta.y / self.view.zoom),
                                scale_x: 1.0,
                                scale_y: 1.0,
                                degrees: 0.0,
                            });
                        } else {
                            self.run_command(Command::ShapeMove {
                                id,
                                dx_mm: f64::from(delta.x / self.view.zoom),
                                dy_mm: f64::from(delta.y / self.view.zoom),
                            });
                        }
                    } else if self.canvas_selection_start.is_none() {
                        self.view.pan_mm.x -= delta.x / self.view.zoom;
                        self.view.pan_mm.y -= delta.y / self.view.zoom;
                    }
                }
            }
            if response.dragged()
                && let Some(start) = self.canvas_selection_start
                && let Some(end) = response.interact_pointer_pos()
            {
                let found = if self.canvas_lasso_mode {
                    if self
                        .canvas_selection_points
                        .last()
                        .is_none_or(|last| last.distance(end) >= 3.0)
                    {
                        self.canvas_selection_points.push(end);
                    }
                    stencilcraft_ui_egui::select_shapes_in_lasso(
                        &doc,
                        &view,
                        &self.canvas_selection_points,
                        stencilcraft_ui_egui::SelectionMode::Intersect,
                    )
                } else {
                    let rect = egui::Rect::from_two_pos(start, end);
                    let mode = if end.x < start.x {
                        stencilcraft_ui_egui::SelectionMode::Intersect
                    } else {
                        stencilcraft_ui_egui::SelectionMode::Contain
                    };
                    stencilcraft_ui_egui::select_shapes_in_rect(&doc, &view, rect, mode)
                };
                let mut ids = if self.canvas_selection_additive {
                    self.selected_shapes.clone()
                } else {
                    Vec::new()
                };
                ids.extend(found);
                self.set_selection(ids);
            }
            if response.drag_stopped() {
                self.canvas_drag_shape = None;
                self.canvas_selection_start = None;
                self.canvas_selection_points.clear();
                self.last_drag_delta = egui::Vec2::ZERO;
            }
        }

        // Zoom: scroll wheel around the pointer.
        if response.hovered() {
            let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let pointer = response.interact_pointer_pos().unwrap_or(clip.center());
                self.view.zoom_at((scroll / 400.0).exp(), pointer);
            }
        }
    }

    pub fn status_bar(&mut self, parent: &mut egui::Ui) {
        let info = format!(
            "{:.1} × {:.1} mm | kerf {:.3} mm | {} shapes",
            self.engine.doc.board.width_mm,
            self.engine.doc.board.height_mm,
            self.engine.doc.kerf.width_mm,
            self.engine.doc.shape_count()
        );
        let status_msg = self.status.clone();
        let zoom_pct = (self.view.zoom * 100.0).round();

        egui::Panel::bottom("status")
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show(parent, |ui| {
                StatusBar::new("stencilcraft-status").show(ui, |bar| {
                    bar.leading(|ui| {
                        StatusBar::field(ui, |ui| {
                            ui.label(format!("{zoom_pct:.0}%"));
                        });
                        StatusBar::info(ui, &info);
                        StatusBar::message(ui, &status_msg, false);
                    });
                    bar.trailing(|ui| {
                        ui.small("F fit · C center · Space+drag to pan");
                        ui.separator();
                        ui.label(format!("Tool: {}", self.active_tool.label()));
                    });
                });
            });
    }
}

pub fn apply_canvas_view_command(
    view: &mut stencilcraft_ui_egui::View,
    command: CanvasViewCommand,
    width_mm: f64,
    height_mm: f64,
    viewport: egui::Rect,
) {
    match command {
        CanvasViewCommand::Fit => {
            view.fit_extent(width_mm, height_mm, viewport, 24.0);
        }
        CanvasViewCommand::Center => {
            view.center_extent(width_mm, height_mm, viewport);
        }
    }
}

impl eframe::App for StencilApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "recent_files", &self.recent_files);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        craft_ui::theme::install(ui.ctx(), &craft_ui::theme::Base::suite_default());
        self.drain_control_requests();
        ui.ctx().set_theme(match self.theme {
            stencilcraft_ui_egui::ColorTheme::Dark => egui::ThemePreference::Dark,
            stencilcraft_ui_egui::ColorTheme::Light => egui::ThemePreference::Light,
        });
        self.handle_keyboard_shortcuts(ui.ctx());

        self.top_panel(ui);
        self.options_bar(ui);
        self.status_bar(ui);

        let slots = build_tool_slots();
        let rail_out = ToolRail::new("stencilcraft-tool-rail")
            .columns(usize::from(self.prefs.rail_columns))
            .column_toggle("Toggle tool rail columns")
            .show(
                ui,
                &slots,
                &self.active_tool,
                &mut self.rail_last_used,
                |_ui, _footer| {},
            );
        if let Some(tool) = rail_out.select {
            self.active_tool = tool;
        }
        if rail_out.toggle_columns {
            self.prefs.rail_columns = if self.prefs.rail_columns == 1 { 2 } else { 1 };
            self.save_prefs();
        }

        let style = DockStyle::from_ui(ui);
        let mut content = WorkspaceContent { app: self };
        let mut workspace = std::mem::take(&mut content.app.workspace);
        let output = DockArea::new(ui.scope_id().with("stencilcraft-workspace")).show_content(
            ui,
            &workspace,
            &style,
            &mut content,
        );
        if !output.actions.is_empty()
            && let Err(error) = workspace.apply_all(output.actions, |panel| {
                if *panel == WorkspacePanel::Canvas {
                    Permissions::PROTECTED
                } else {
                    Permissions::default()
                }
            })
        {
            content.app.status = format!("workspace layout rejected: {error}");
        }
        content.app.workspace = workspace;
        content.app.retain_valid_selection();
        content.app.text_dialog(ui);
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_ui::docking::{Action, Placement};

    #[test]
    fn workspace_dock_identity_is_stable_across_frames() {
        let ctx = egui::Context::default();
        let mut ids = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    ..Default::default()
                },
                |ui| ids.push(ui.scope_id().with("stencilcraft-workspace")),
            );
            output.textures_delta.clear();
        }
        assert_eq!(ids[0], ids[1]);
    }

    #[test]
    fn reset_panels_restores_the_default_docking_positions() {
        let mut app = StencilApp::new(None);
        let default = default_workspace();
        app.workspace
            .apply_all(
                [Action::Move {
                    panel: WorkspacePanel::Layers,
                    anchor: WorkspacePanel::Inspector,
                    placement: Placement::Group {
                        below: true,
                        height: None,
                    },
                }],
                |panel| {
                    if *panel == WorkspacePanel::Canvas {
                        Permissions::PROTECTED
                    } else {
                        Permissions::default()
                    }
                },
            )
            .unwrap();
        assert_ne!(app.workspace, default);

        app.reset_workspace();

        assert_eq!(app.workspace, default);
        assert_eq!(app.status, "panel layout restored");
        assert!(app.workspace.validate().is_ok());
    }

    #[test]
    fn command_d_duplicates_the_selection_and_selects_the_copies() {
        let ctx = egui::Context::default();
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 3.0,
            y: 4.0,
            width: 12.0,
            height: 8.0,
        });
        app.set_selection([1]);
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::D,
                    physical_key: Some(egui::Key::D),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
                ..Default::default()
            },
            |ui| app.handle_keyboard_shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(app.engine.doc.shape_count(), 2);
        assert_eq!(app.selected_shapes, vec![2]);
        assert_eq!(
            app.engine.doc.get_shape(1).unwrap().bounds(),
            app.engine.doc.get_shape(2).unwrap().bounds()
        );
    }

    #[test]
    fn command_a_selects_every_shape_on_the_canvas() {
        let ctx = egui::Context::default();
        let mut app = StencilApp::new(None);
        for x in [3.0, 23.0, 43.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 4.0,
                width: 12.0,
                height: 8.0,
            });
        }
        app.set_selection([2]);
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: Some(egui::Key::A),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
                ..Default::default()
            },
            |ui| app.handle_keyboard_shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(app.selected_shapes, vec![1, 2, 3]);
        assert_eq!(app.selected_shape, Some(3));
    }

    #[test]
    fn f_and_c_queue_fit_and_center_without_command_modifiers() {
        for (key, expected) in [
            (egui::Key::F, CanvasViewCommand::Fit),
            (egui::Key::C, CanvasViewCommand::Center),
        ] {
            let ctx = egui::Context::default();
            let mut app = StencilApp::new(None);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| app.handle_keyboard_shortcuts(ui.ctx()),
            );
            output.textures_delta.clear();
            assert_eq!(app.pending_view_command, Some(expected));

            let mut view = stencilcraft_ui_egui::View {
                zoom: 2.5,
                pan_mm: egui::vec2(700.0, -300.0),
            };
            let viewport =
                egui::Rect::from_min_size(egui::pos2(100.0, 60.0), egui::vec2(800.0, 500.0));
            apply_canvas_view_command(&mut view, expected, 600.0, 300.0, viewport);
            let center = view.world_to_screen(stencilcraft_geom::Point::new(300.0, 150.0));
            assert!((center - viewport.center()).length() < 1e-4);
            match expected {
                CanvasViewCommand::Fit => assert!(view.zoom < 2.5),
                CanvasViewCommand::Center => assert_eq!(view.zoom, 2.5),
            }
        }
    }

    #[test]
    fn resizing_the_canvas_refits_the_view() {
        let ctx = egui::Context::default();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        frame(&ctx, &mut app, vec![], |app, ui| app.canvas_body(ui));
        assert!(app.view_initialized);
        // A second frame on the same board keeps what the user set up.
        let steady_zoom = app.view.zoom;
        frame(&ctx, &mut app, vec![], |app, ui| app.canvas_body(ui));
        assert_eq!(app.view.zoom, steady_zoom);

        app.engine
            .run(Command::DocumentSetBoard {
                width_mm: 900.0,
                height_mm: 600.0,
            })
            .expect("set board");
        // A zoom that only made sense for the old 100 × 150 mm sheet.
        app.view.zoom = 6.0;
        frame(&ctx, &mut app, vec![], |app, ui| app.canvas_body(ui));
        assert_eq!(app.last_board, Some((900.0, 600.0)));
        assert!(
            app.view.zoom < 6.0,
            "a resized canvas must refit, zoom stayed {}",
            app.view.zoom
        );
        // Framed, not merely zoomed out: the far corner lands on screen.
        let corner = app
            .view
            .world_to_screen(stencilcraft_geom::Point::new(900.0, 600.0));
        assert!(
            corner.x > 0.0 && corner.y > 0.0,
            "board corner off screen: {corner:?}"
        );
    }

    #[test]
    fn canvas_view_shortcuts_are_ignored_while_text_input_is_wanted() {
        let ctx = egui::Context::default();
        let mut app = StencilApp::new(None);
        let mut text = String::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.text_edit_singleline(&mut text).request_focus();
        });
        output.textures_delta.clear();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::F,
                    physical_key: Some(egui::Key::F),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                let response = ui.text_edit_singleline(&mut text);
                assert!(response.has_focus());
                app.handle_keyboard_shortcuts(ui.ctx());
            },
        );
        output.textures_delta.clear();
        assert_eq!(app.pending_view_command, None);
    }

    #[test]
    fn kerf_preview_cache_reuses_and_invalidates_by_document_content() {
        let mut app = StencilApp::new(None);
        assert!(app.update_kerf_preview_cache(&app.engine.doc.clone()));
        let first = app.kerf_preview_cache.as_ref().unwrap().preview.clone();
        let mut metadata_only_change = app.engine.doc.clone();
        metadata_only_change.board.width_mm += 10.0;
        metadata_only_change.layers[0].visible = false;
        assert!(!app.update_kerf_preview_cache(&metadata_only_change));
        let cached_preview = app
            .kerf_preview_cache
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap();
        assert_eq!(cached_preview.board, metadata_only_change.board);
        assert!(!cached_preview.layers[0].visible);
        assert_eq!(
            cached_preview.layers[0].shapes,
            first.as_ref().unwrap().layers[0].shapes
        );
        assert!(!app.update_kerf_preview_cache(&app.engine.doc.clone()));

        app.run_command(Command::ShapeAddRect {
            x: 3.0,
            y: 4.0,
            width: 12.0,
            height: 8.0,
        });
        assert!(app.update_kerf_preview_cache(&app.engine.doc.clone()));
        let preview = app
            .kerf_preview_cache
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap();
        assert_eq!(preview.shape_count(), 1);

        app.run_command(Command::KerfSet { width_mm: 2.0 });
        assert!(app.update_kerf_preview_cache(&app.engine.doc.clone()));
        let compensated = app
            .kerf_preview_cache
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap();
        assert_ne!(
            compensated.get_shape(1).unwrap().path,
            app.engine.doc.get_shape(1).unwrap().path
        );
    }

    #[test]
    fn recent_files_are_unique_most_recent_first_and_bounded() {
        let mut app = StencilApp::new(None);
        for index in 0..10 {
            app.record_recent_file(format!("/documents/design-{index}.stencilcraft"));
        }
        app.record_recent_file("/documents/design-5.stencilcraft".to_string());
        assert_eq!(app.recent_files.len(), 8);
        assert_eq!(
            app.recent_files.first().unwrap(),
            "/documents/design-5.stencilcraft"
        );

        assert_eq!(
            app.recent_files
                .iter()
                .filter(|path| path.as_str() == "/documents/design-5.stencilcraft")
                .count(),
            1
        );
    }

    #[test]
    fn recent_file_opens_document_and_updates_recent_order() {
        let mut app = StencilApp::new(None);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/starter.stencilcraft")
            .to_string_lossy()
            .into_owned();
        app.record_recent_file(path.clone());
        app.open_document(&path);
        let expected = stencilcraft_doc::load_from(&path).unwrap();
        assert_eq!(app.engine.doc, expected);
        assert_eq!(app.open_path.as_deref(), Some(path.as_str()));
        let recent_path = std::fs::canonicalize(path)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(app.recent_files.first(), Some(&recent_path));
    }

    #[test]
    fn delete_and_backspace_remove_selected_shapes() {
        for key in [egui::Key::Delete, egui::Key::Backspace] {
            let ctx = egui::Context::default();
            let mut app = StencilApp::new(None);
            app.run_command(Command::ShapeAddRect {
                x: 3.0,
                y: 4.0,
                width: 12.0,
                height: 8.0,
            });
            app.set_selection([1]);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| app.handle_keyboard_shortcuts(ui.ctx()),
            );
            output.textures_delta.clear();
            assert_eq!(app.engine.doc.shape_count(), 0);
            assert!(app.selected_shapes.is_empty());
        }
    }

    fn frame(
        ctx: &egui::Context,
        app: &mut StencilApp,
        events: Vec<egui::Event>,
        panel: fn(&mut StencilApp, &mut egui::Ui),
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| panel(app, ui),
        );
        output.textures_delta.clear();
        output
    }

    fn node_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
        let node = tree
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .unwrap_or_else(|| panic!("missing node {label}"));
        let bounds = node.1.bounds().unwrap();
        egui::pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            ((bounds.y0 + bounds.y1) / 2.0) as f32,
        )
    }

    fn click(
        ctx: &egui::Context,
        app: &mut StencilApp,
        panel: fn(&mut StencilApp, &mut egui::Ui),
        pos: egui::Pos2,
    ) {
        frame(ctx, app, vec![egui::Event::PointerMoved(pos)], panel);
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
                panel,
            );
        }
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn shared_kerf_slider_and_inspector_field_edit_the_document() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        app.engine
            .run(Command::ShapeAddRect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            })
            .unwrap();
        app.set_selection([1]);
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        let pos = node_center(&out, "Kerf");
        click(&ctx, &mut app, StencilApp::inspector, pos);
        assert!(app.engine.doc.kerf.width_mm > 0.0);
        assert!(app.engine.undo());
        assert_eq!(app.engine.doc.kerf.width_mm, 0.0);
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        let pos = node_center(&out, "X");
        click(&ctx, &mut app, StencilApp::inspector, pos);
        frame(
            &ctx,
            &mut app,
            vec![
                key(egui::Key::A, egui::Modifiers::COMMAND),
                egui::Event::Text("12".into()),
            ],
            StencilApp::inspector,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::inspector,
        );
        assert_eq!(
            app.engine.doc.get_shape(1).unwrap().bounds().unwrap().x0,
            12.0
        );
    }

    #[test]
    fn kerf_drag_cancel_preserves_history_and_commit_is_one_undo_step() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        app.engine.run(Command::KerfSet { width_mm: 0.2 }).unwrap();
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        let start = node_center(&out, "Kerf");
        let end = start + egui::vec2(35.0, 0.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
            StencilApp::inspector,
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(end)],
            StencilApp::inspector,
        );
        assert!(app.kerf_draft.is_some_and(|value| value != 0.2));
        assert_eq!(app.engine.doc.kerf.width_mm, 0.2);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
            StencilApp::inspector,
        );
        assert_eq!(app.kerf_draft, None);
        frame(
            &ctx,
            &mut app,
            vec![button(end, false)],
            StencilApp::inspector,
        );
        assert_eq!(app.engine.doc.kerf.width_mm, 0.2);
        assert!(app.engine.undo());
        assert_eq!(app.engine.doc.kerf.width_mm, 0.0);
        assert!(app.engine.redo());
        assert_eq!(app.engine.doc.kerf.width_mm, 0.2);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
            StencilApp::inspector,
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(end)],
            StencilApp::inspector,
        );
        let final_pos = end + egui::vec2(15.0, 0.0);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(final_pos)],
            StencilApp::inspector,
        );
        frame(
            &ctx,
            &mut app,
            vec![button(final_pos, false)],
            StencilApp::inspector,
        );
        let committed = app.engine.doc.kerf.width_mm;
        assert_ne!(committed, 0.2);
        assert!(app.engine.undo());
        assert_eq!(app.engine.doc.kerf.width_mm, 0.2);
        assert!(app.engine.redo());
        assert_eq!(app.engine.doc.kerf.width_mm, committed);
    }

    #[test]
    fn pressure_slider_drag_cancel_restores_value() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let start = node_center(&out, "Pressure");
        let end = start + egui::vec2(35.0, 0.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
            StencilApp::send_tab,
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(end)],
            StencilApp::send_tab,
        );
        assert_ne!(app.send.pressure_override, 10);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        assert_eq!(app.send.pressure_override, 10);
        frame(
            &ctx,
            &mut app,
            vec![button(end, false)],
            StencilApp::send_tab,
        );
        assert_eq!(app.send.pressure_override, 10);
    }

    #[test]
    fn shared_send_sliders_and_text_dialog_accept_input() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Pressure"),
        );
        assert_ne!(app.send.pressure_override, 10);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Speed"),
        );
        assert_ne!(app.send.speed_override, 5);
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        app.text_dialog = true;
        frame(&ctx, &mut app, vec![], StencilApp::text_dialog);
        let out = frame(&ctx, &mut app, vec![], StencilApp::text_dialog);
        click(
            &ctx,
            &mut app,
            StencilApp::text_dialog,
            node_center(&out, "Text to place"),
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Text("HELLO".into())],
            StencilApp::text_dialog,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::text_dialog,
        );
        frame(&ctx, &mut app, vec![], StencilApp::text_dialog);
        assert_eq!(app.text_value, "HELLO");
        // Enter commits the typed run. A plain text field passes Escape through, so the
        // committed text stays.
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
            StencilApp::text_dialog,
        );
        assert_eq!(app.text_value, "HELLO");
    }

    #[test]
    fn shared_zoom_slider_changes_the_canvas_view() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        let out = frame(&ctx, &mut app, vec![], StencilApp::canvas_body);
        let old_zoom = app.view.zoom;
        click(
            &ctx,
            &mut app,
            StencilApp::canvas_body,
            node_center(&out, "Zoom"),
        );
        assert_ne!(app.view.zoom, old_zoom);
    }

    #[test]
    fn material_selection_updates_overrides_and_cancel_restores_selection() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        let initial = app.send.media_index;
        assert!(initial + 1 < stencilcraft_machine::MEDIA_PRESETS.len());
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Material"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(app.send.media_index, initial + 1);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        assert_eq!(app.send.media_index, initial);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Material"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let media = &stencilcraft_machine::MEDIA_PRESETS[initial + 1];
        assert_eq!(app.send.media_index, initial + 1);
        assert_eq!(app.send.pressure_override, media.pressure.unwrap_or(10));
        assert_eq!(app.send.speed_override, media.speed.unwrap_or(5));
    }

    #[test]
    fn material_selection_changes_encoded_dry_run_commands() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        app.engine
            .run(Command::ShapeAddRect {
                x: 2.0,
                y: 3.0,
                width: 5.0,
                height: 4.0,
            })
            .unwrap();
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let initial = app.send.summary.clone().unwrap();
        assert!(initial.iter().any(|line| line == "FX10,1"));
        assert!(initial.iter().any(|line| line == "!5,1"));
        assert!(initial.iter().any(|line| line.starts_with('D')));

        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Material"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(
            stencilcraft_machine::MEDIA_PRESETS[app.send.media_index].id,
            106
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let updated = app.send.summary.as_ref().unwrap();
        assert!(updated.iter().any(|line| line == "FX14,1"));
        assert!(updated.iter().any(|line| line == "!10,1"));
        assert!(!updated.iter().any(|line| line == "FX10,1"));
        assert!(!updated.iter().any(|line| line == "!5,1"));
        assert!(updated.iter().any(|line| line.starts_with('D')));
    }

    #[test]
    fn selected_model_and_mat_change_dry_run_setup() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        app.engine
            .run(Command::ShapeAddRect {
                x: 2.0,
                y: 3.0,
                width: 5.0,
                height: 4.0,
            })
            .unwrap();
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let baseline = app.send.summary.clone().unwrap();
        assert!(baseline.iter().any(|line| line == "TG9"));
        assert!(baseline.iter().any(|line| line == "Z60000,12000"));
        assert!(baseline[0].contains("Cameo 4 Pro"));

        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Model"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::End, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(app.send.model, stencilcraft_machine::Model::Portrait3);
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        click(
            &ctx,
            &mut app,
            StencilApp::inspector,
            node_center(&out, "Cutting mat"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Home, egui::Modifiers::NONE)],
            StencilApp::inspector,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::inspector,
        );
        frame(&ctx, &mut app, vec![], StencilApp::inspector);
        assert_eq!(app.mat, stencilcraft_machine::Mat::None);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let updated = app.send.summary.as_ref().unwrap();
        assert!(updated.iter().any(|line| line == "TG0"));
        assert!(updated.iter().any(|line| line == "Z365800,4060"));
        assert!(
            !updated
                .iter()
                .any(|line| line == "TG9" || line == "Z60000,12000")
        );
        assert!(updated[0].contains("Portrait 3"));
        assert!(updated.iter().any(|line| line.contains("no mat")));
        assert!(updated.iter().any(|line| line.starts_with('D')));
    }

    #[test]
    fn screen_reader_actions_select_material_and_update_dry_run() {
        use egui::accesskit::{Action, ActionRequest, Role, TreeId};
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        app.engine
            .run(Command::ShapeAddRect {
                x: 2.0,
                y: 3.0,
                width: 5.0,
                height: 4.0,
            })
            .unwrap();
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        let (selector_id, selector) = tree
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some("Material"))
            .unwrap();
        assert_eq!(selector.role(), Role::ComboBox);
        assert!(selector.supports_action(Action::Click));
        let request = |action, target_node| {
            egui::Event::AccessKitActionRequest(ActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node,
                data: None,
            })
        };
        frame(
            &ctx,
            &mut app,
            vec![request(Action::Click, *selector_id)],
            StencilApp::send_tab,
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        let (option_id, option) = tree
            .nodes
            .iter()
            .find(|(_, node)| {
                node.role() == Role::ListBoxOption && node.label() == Some("Film Labels (ID 106)")
            })
            .unwrap();
        assert!(option.supports_action(Action::Click));
        frame(
            &ctx,
            &mut app,
            vec![request(Action::Click, *option_id)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(
            stencilcraft_machine::MEDIA_PRESETS[app.send.media_index].id,
            106
        );
        assert_eq!(
            (app.send.pressure_override, app.send.speed_override),
            (14, 10)
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let summary = app.send.summary.as_ref().unwrap();
        assert!(summary.iter().any(|line| line == "FX14,1"));
        assert!(summary.iter().any(|line| line == "!10,1"));
    }

    #[test]
    fn material_selector_supports_accessible_focus_and_keyboard_selection() {
        use egui::accesskit::{Action, ActionRequest, HasPopup, Role, TreeId};
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        let initial = app.send.media_index;
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        let (id, node) = tree
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some("Material"))
            .unwrap();
        assert_eq!(node.role(), Role::ComboBox);
        assert_eq!(node.value(), Some(media_label(initial).as_str()));
        assert_eq!(node.is_expanded(), Some(false));
        assert_eq!(node.has_popup(), Some(HasPopup::Listbox));
        let material_id = *id;
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(ActionRequest {
                action: Action::Focus,
                target_tree: TreeId::ROOT,
                target_node: material_id,
                data: None,
            })],
            StencilApp::send_tab,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::ALT)],
            StencilApp::send_tab,
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        let combo = tree
            .nodes
            .iter()
            .find(|(id, _)| *id == material_id)
            .unwrap();
        assert_eq!(combo.1.is_expanded(), Some(true));
        let initial_label = media_label(initial);
        let option = tree
            .nodes
            .iter()
            .find(|(_, node)| {
                node.label() == Some(initial_label.as_str()) && node.role() == Role::ListBoxOption
            })
            .unwrap();
        assert_eq!(option.1.is_selected(), Some(true));
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(app.send.media_index, initial + 1);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        let combo = tree
            .nodes
            .iter()
            .find(|(id, _)| *id == material_id)
            .unwrap();
        assert_eq!(combo.1.is_expanded(), Some(false));
        assert_eq!(combo.1.value(), Some(media_label(initial + 1).as_str()));
        let media = &stencilcraft_machine::MEDIA_PRESETS[initial + 1];
        assert_eq!(
            (app.send.pressure_override, app.send.speed_override),
            (media.pressure.unwrap_or(10), media.speed.unwrap_or(5))
        );
        let settings = (app.send.pressure_override, app.send.speed_override);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Space, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(app.send.media_index, initial + 2);
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(app.send.media_index, initial + 1);
        assert_eq!(
            (app.send.pressure_override, app.send.speed_override),
            settings
        );
    }

    #[test]
    fn custom_overrides_and_bounding_box_preview_reach_dry_run_program() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        let mut path = stencilcraft_geom::BezPath::new();
        path.move_to(stencilcraft_geom::Point::new(2.0, 3.0));
        path.line_to(stencilcraft_geom::Point::new(7.0, 3.0));
        path.line_to(stencilcraft_geom::Point::new(4.0, 7.0));
        path.close_path();
        app.engine
            .run(Command::ShapeAddPath {
                path,
                style: stencilcraft_doc::CutStyle::Cut,
            })
            .unwrap();

        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Material"),
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Home, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            StencilApp::send_tab,
        );
        frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        assert_eq!(
            stencilcraft_machine::MEDIA_PRESETS[app.send.media_index].id,
            300
        );
        assert_eq!(
            (app.send.pressure_override, app.send.speed_override),
            (10, 5)
        );

        for label in ["Pressure", "Speed"] {
            let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
            click(
                &ctx,
                &mut app,
                StencilApp::send_tab,
                node_center(&out, label),
            );
            frame(
                &ctx,
                &mut app,
                vec![key(egui::Key::End, egui::Modifiers::NONE)],
                StencilApp::send_tab,
            );
        }
        assert_eq!(
            (app.send.pressure_override, app.send.speed_override),
            (33, 30)
        );
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let normal = app.send.summary.clone().unwrap();
        assert!(normal.iter().any(|line| line == "FX33,1"));
        assert!(normal.iter().any(|line| line == "!30,1"));
        assert!(normal.iter().any(|line| line == "D140,80"));
        assert!(!normal.iter().any(|line| line == "D140,140"));

        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Bounding box preview"),
        );
        assert!(app.send.bbox_only);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Encode dry run"),
        );
        let preview = app.send.summary.as_ref().unwrap();
        assert!(preview.iter().any(|line| line == "FX33,1"));
        assert!(preview.iter().any(|line| line == "!30,1"));
        let plot: Vec<&str> = preview
            .iter()
            .filter(|line| line.starts_with('M') || line.starts_with('D'))
            .map(String::as_str)
            .collect();
        assert_eq!(
            plot,
            [
                "M60,40", "D60,140", "D140,140", "D140,40", "D60,40", "M140,0"
            ]
        );
        assert!(!preview.iter().any(|line| line == "D140,80"));
    }

    #[test]
    fn checkbox_interactions_change_preview_and_layer_visibility() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        let layer_label = format!("{}  ·  0", app.engine.doc.layers[0].name);
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        click(
            &ctx,
            &mut app,
            StencilApp::inspector,
            node_center(&out, &layer_label),
        );
        assert!(!app.engine.doc.layers[0].visible);
        let out = frame(&ctx, &mut app, vec![], StencilApp::inspector);
        click(
            &ctx,
            &mut app,
            StencilApp::inspector,
            node_center(&out, "Preview kerf compensation"),
        );
        assert!(app.kerf_preview);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Bounding box preview"),
        );
        assert!(app.send.bbox_only);
        let out = frame(&ctx, &mut app, vec![], StencilApp::send_tab);
        click(
            &ctx,
            &mut app,
            StencilApp::send_tab,
            node_center(&out, "Bounding box preview"),
        );
        assert!(!app.send.bbox_only);
    }

    #[test]
    fn design_system_controls_dispatch_app_actions() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        craft_ui::theme::install(&ctx, &craft_ui::theme::Base::suite_default());
        let mut app = StencilApp::new(None);
        {
            let mut frame = |events: Vec<egui::Event>| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 600.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| app.tools_panel(ui),
                );
                output.textures_delta.clear();
                output
            };
            let output = frame(vec![]);
            let tree = output.platform_output.accesskit_update.unwrap();
            let node = tree
                .nodes
                .iter()
                .find(|(_, node)| node.label() == Some("＋ Rectangle"))
                .unwrap();
            assert_eq!(node.1.role(), egui::accesskit::Role::Button);
            let bounds = node.1.bounds().unwrap();
            let pos = egui::pos2(
                ((bounds.x0 + bounds.x1) / 2.0) as f32,
                ((bounds.y0 + bounds.y1) / 2.0) as f32,
            );
            let pointer = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let _ = frame(vec![egui::Event::PointerMoved(pos)]);
            let _ = frame(vec![pointer(true)]);
            let _ = frame(vec![pointer(false)]);
            let tree = frame(vec![]).platform_output.accesskit_update.unwrap();
            let node = tree
                .nodes
                .iter()
                .find(|(_, node)| node.label() == Some("Duplicate selected"))
                .unwrap();
            assert_eq!(node.1.role(), egui::accesskit::Role::Button);
            let bounds = node.1.bounds().unwrap();
            let pos = egui::pos2(
                ((bounds.x0 + bounds.x1) / 2.0) as f32,
                ((bounds.y0 + bounds.y1) / 2.0) as f32,
            );
            let pointer = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let _ = frame(vec![egui::Event::PointerMoved(pos)]);
            let _ = frame(vec![pointer(true)]);
            let _ = frame(vec![pointer(false)]);
        }
        assert_eq!(app.engine.doc.shape_count(), 2);
        assert_eq!(app.selected_shape, Some(2));
        assert_eq!(app.selected_shapes, vec![2]);
        assert_eq!(
            app.engine.doc.get_shape(1).unwrap().bounds(),
            app.engine.doc.get_shape(2).unwrap().bounds()
        );
        assert_eq!(
            app.engine.doc.get_shape(1).unwrap().style,
            app.engine.doc.get_shape(2).unwrap().style
        );
    }

    #[test]
    fn rulers_measure_canvas_and_reset_origin() {
        let mut app = StencilApp::new(None);
        assert_eq!(app.ruler_origin, stencilcraft_geom::Point::ZERO);
        app.ruler_origin = stencilcraft_geom::Point::new(50.0, 25.0);
        assert_eq!(app.ruler_origin, stencilcraft_geom::Point::new(50.0, 25.0));
        app.ruler_origin = stencilcraft_geom::Point::ZERO;
        assert_eq!(app.ruler_origin, stencilcraft_geom::Point::ZERO);
    }

    #[test]
    fn tool_rail_switches_active_tools_and_shortcuts() {
        let mut app = StencilApp::new(None);
        assert_eq!(app.active_tool, ActiveTool::Select);
        assert!(!app.active_tool.is_shape_creator());

        app.active_tool = ActiveTool::Pen;
        assert_eq!(app.active_tool.label(), "Pen");

        app.active_tool = ActiveTool::Rect;
        assert!(app.active_tool.is_shape_creator());
        assert_eq!(app.active_tool.label(), "Rectangle");

        app.active_tool = ActiveTool::Ellipse;
        assert!(app.active_tool.is_shape_creator());
        assert_eq!(app.active_tool.label(), "Ellipse");
    }

    #[test]
    fn pen_draft_commit_creates_cut_path() {
        let mut app = StencilApp::new(None);
        let initial_count = app.engine.doc.shape_count();
        let k1 = craft_draw::ui::pen::Knot::corner(stencilcraft_geom::Point::new(10.0, 10.0));
        let k2 = craft_draw::ui::pen::Knot::corner(stencilcraft_geom::Point::new(50.0, 10.0));
        let k3 = craft_draw::ui::pen::Knot::corner(stencilcraft_geom::Point::new(50.0, 40.0));
        let sub = craft_draw::ui::pen::EditSubpath {
            closed: true,
            knots: vec![k1, k2, k3],
        };
        app.pen_draft = Some(sub);
        app.commit_pen_draft();
        assert_eq!(app.engine.doc.shape_count(), initial_count + 1);
        assert!(app.engine.can_undo());
        let _ = app.engine.undo();
        assert_eq!(app.engine.doc.shape_count(), initial_count);
    }
}
