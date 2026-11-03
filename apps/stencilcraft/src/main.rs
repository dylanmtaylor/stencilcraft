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
use craft_theme::{ControlSize, TypeRole};
use craft_ui::canvas::{Readout, RefSize, ReferencePoint, RulerNames, RulerUnits, Rulers};
use craft_ui::composites::{PanelFooter, PropertyGroup, PropertyLabel, PropertyRow, SectionHeader};
use craft_ui::controls::{
    Button, ButtonVariant, Checkbox, IconButton, LinkButton, LinkMode, NumberField, Select, Slider,
    SliderScale, TextField, ToggleButton,
};
use craft_ui::dialog::{Dialog, DialogState, Footer, Outcome};
use craft_ui::docking::{
    Column, DockArea, DockContent, DockStyle, Group, Layout, Node, Permissions, Side,
};
use craft_ui::edit::{CancelReason, EditEvent};
use craft_ui::icons::Icon;
use craft_ui::layout::{SplitAxis, SplitSize};
use craft_ui::list::{ListRow, ToggleColumn};
use craft_ui::localized::{LocaleNumbers, Unit};
use craft_ui::menu::{MenuBar, MenuItem};
use craft_ui::patterns::{
    AlignAction, AlignBar, Inspector, InspectorHeader, LastUsed, OptionsBar, Panel, RailTool,
    StatusBar, ToolRail, ToolSlot,
};
use craft_ui::primitives::{Content, Divider, Text};
use craft_ui::title_bar::{Frame, LeadingEnd, Title, TitleBar};
use eframe::egui;
use serde::{Deserialize, Serialize};
use stencilcraft_doc::CutStyle;
use stencilcraft_engine::document::{DocumentCommand, TransformRequest};
use stencilcraft_engine::{Alignment, BooleanOperation, Command, DistributionAxis};

/// CLI/agent control channel: JSON lines on a TCP socket, one session per
/// connection (the same session core the CLI's `mcp` mode uses).
mod control;

/// The menu bar (Silhouette Studio order plus Illustrator-grade items; docs/menu-map.md).
mod menus;

/// File › New: the New Document dialog (preset cards and details).
mod new_doc;

/// Silhouette-style Transform, Replicate, Offset, Modify and Point Editing panels.
mod panels;

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
    /// View › Guides › Show Guides.
    pub show_guides: bool,
    /// View › Smart Guides: draw the alignment lines snapping found while dragging.
    pub smart_guides: bool,
    pub print_cut_marks: bool,
    pub rail_columns: u8,
    pub link_aspect: bool,
    pub recent_files: Vec<String>,
    /// Sizes created from New Document, newest first (its Recent tab).
    pub recent_new: Vec<new_doc::RecentDoc>,
}

impl Default for UserPrefs {
    fn default() -> Self {
        Self {
            dark_theme: true,
            show_rulers: true,
            show_grid: true,
            show_guides: true,
            smart_guides: true,
            print_cut_marks: false,
            rail_columns: 1,
            link_aspect: false,
            recent_files: Vec::new(),
            recent_new: Vec::new(),
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
    NewFromTemplate,
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
    /// Rotate the selection by whole degrees (positive is clockwise on screen).
    RotateBy(i16),
    /// Mirror the selection in place: `Horizontal` swaps left and right.
    Flip(DistributionAxis),
    /// Align the selection to the last-selected shape, which stays still.
    AlignToKey(Alignment),
    TransformAgain,
    CenterToPage,
    SetStyle(CutStyle),
    PlaceText,
    EditText,
    SelectInverse,
    SelectSameStyle,
    SelectSameLayer,
    ShowDesign,
    ShowSend,
    ShowPanel(WorkspacePanel),
    DirectSelectTool,
    DryRun,
    Quit,
    /// A planned item with no command yet; its menu row is disabled.
    NotYet,
    /// Replicate, offset, path and lock/hide actions (`panels.rs`).
    Edit(panels::EditAction),
    FitView,
    CenterView,
    ActualSize,
    ZoomIn,
    ZoomOut,
    /// View › Zoom › Selection: fit the selection's bounds.
    ZoomSelection,
    /// View › Zoom presets: 100 is one screen point per millimetre, as the status bar shows.
    ZoomPercent(u16),
    /// View › Rotate Page: clockwise quarter turns of the view (0–3).
    RotateView(u8),
    ToggleRulers,
    ToggleGrid,
    ToggleSnapGrid,
    ToggleSmartSnapping,
    ToggleSnapPoint,
    ToggleSmartGuides,
    ToggleShowGuides,
    ToggleLockGuides,
    MakeGuides,
    ReleaseGuides,
    ClearGuides,
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

fn mat_supported(
    caps: Option<&stencilcraft_machine::Capabilities>,
    mat: stencilcraft_machine::Mat,
) -> bool {
    mat == stencilcraft_machine::Mat::None || caps.is_some_and(|c| c.mats.contains(&mat))
}

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
    if open_path.is_none() {
        app.open_new_document(0);
    }

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
    Transform,
    Replicate,
    Offset,
    Modify,
    Points,
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
            WorkspacePanel::Transform => self.app.transform_panel(ui),
            WorkspacePanel::Replicate => self.app.replicate_panel(ui),
            WorkspacePanel::Offset => self.app.offset_panel(ui),
            WorkspacePanel::Modify => self.app.modify_panel(ui),
            WorkspacePanel::Points => self.app.points_panel(ui),
        }
    }

    fn logical_label(&self, panel: &WorkspacePanel) -> String {
        match panel {
            WorkspacePanel::Canvas => "Canvas",
            WorkspacePanel::Tools => "Tools",
            WorkspacePanel::Inspector => "Properties",
            WorkspacePanel::Layers => "Layers",
            WorkspacePanel::Transform => "Transform",
            WorkspacePanel::Replicate => "Replicate",
            WorkspacePanel::Offset => "Offset",
            WorkspacePanel::Modify => "Modify",
            WorkspacePanel::Points => "Point Editing",
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
    /// Fit the selection's bounds (does nothing without a selection).
    Selection,
    /// Zoom to a percentage about the canvas centre.
    ZoomPercent(u16),
    /// Turn the view to this many clockwise quarter turns about the canvas centre.
    Rotate(u8),
}

pub struct StencilApp {
    pub workspace: Layout<WorkspacePanel>,
    pub window_frame: Frame,
    pub engine: stencilcraft_engine::Engine,
    pub view: stencilcraft_ui_egui::View,
    pub pending_view_command: Option<CanvasViewCommand>,
    /// Apply status-bar zoom changes at the canvas center once its bounds are known.
    pub pending_zoom: Option<f32>,
    pub kerf_preview: bool,
    pub kerf_preview_cache: Option<KerfPreviewCache>,
    pub kerf_draft: Option<f64>,
    pub pressure_before: Option<f64>,
    pub speed_before: Option<f64>,
    pub zoom_before: Option<f64>,
    /// A ruler guide being dragged on the canvas: its key and where it is now.
    pub guide_preview: Option<(u64, craft_ui::canvas::GuidePlace)>,
    /// The selection's bounds when a move drag began, so snapping works from the start.
    pub drag_origin: Option<stencilcraft_geom::Rect>,
    /// Smart guides: the lines the last snap of a move drag found, in millimetres.
    pub snap_lines: Vec<(stencilcraft_geom::Point, stencilcraft_geom::Point)>,
    pub text_before: Option<String>,
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
    pub text_dialog: bool,
    /// File › Quit was chosen; the next frame closes the window.
    pub quit_requested: bool,
    pub text_dialog_state: DialogState<String>,
    /// File › New's dialog.
    pub new_doc: new_doc::NewDocState,
    saved_document: stencilcraft_doc::Document,
    pending_new_document: Option<new_doc::Choice>,
    discard_new_dialog: DialogState,
    /// Name of an unsaved document (New Document's Name), shown in the title bar.
    pub doc_title: String,
    pub text_value: String,
    text_family: String,
    text_size: f64,
    text_edit_id: Option<u64>,
    text_original_font: Option<stencilcraft_doc::FontSpec>,
    text_fonts: Vec<stencilcraft_engine::text::FontFamily>,
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
    /// The design-system theme resolved for `theme`, rebuilt when the user switches mode.
    pub installed_theme: Option<(stencilcraft_ui_egui::ColorTheme, craft_ui::theme::Base)>,
    /// Settings of the Transform, Replicate, Offset, Modify and Point Editing panels.
    pub panels: panels::PanelState,
}

#[derive(Clone, Debug)]
pub struct KerfPreviewCache {
    pub source: stencilcraft_doc::Document,
    pub kerf_width_mm: f64,
    pub preview: Result<stencilcraft_doc::Document, String>,
}

type HardwareResult =
    std::sync::Arc<std::sync::Mutex<Option<Result<stencilcraft_engine::Effect, String>>>>;

#[derive(Debug, Clone)]
pub struct SendTab {
    pub media_index: usize,
    pub model: stencilcraft_machine::Model,
    pub pressure_override: u8,
    pub speed_override: u8,
    pub bbox_only: bool,
    pub summary: Option<Vec<String>>,
    pub devices: Vec<(String, String, String)>,
    pub device: String,
    pub hardware_result: HardwareResult,
    pub busy: bool,
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
            devices: Vec::new(),
            device: String::new(),
            hardware_result: Default::default(),
            busy: false,
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
                    vec![
                        Group::new(vec![WorkspacePanel::Inspector, WorkspacePanel::Layers], 0),
                        // Silhouette's right-hand panel strip.
                        Group::new(
                            vec![
                                WorkspacePanel::Transform,
                                WorkspacePanel::Replicate,
                                WorkspacePanel::Offset,
                                WorkspacePanel::Modify,
                                WorkspacePanel::Points,
                            ],
                            0,
                        ),
                    ],
                    300.0,
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
        let saved_document = engine.doc.clone();
        Self {
            workspace: default_workspace(),
            window_frame: Frame::Native,
            engine,
            view: stencilcraft_ui_egui::View::default(),
            pending_view_command: None,
            pending_zoom: None,
            kerf_preview: false,
            kerf_preview_cache: None,
            kerf_draft: None,
            pressure_before: None,
            speed_before: None,
            zoom_before: None,
            guide_preview: None,
            drag_origin: None,
            snap_lines: Vec::new(),
            text_before: None,
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
            text_dialog: false,
            quit_requested: false,
            text_dialog_state: DialogState::new(),
            new_doc: new_doc::NewDocState::default(),
            saved_document,
            pending_new_document: None,
            discard_new_dialog: DialogState::new(),
            doc_title: "Untitled".to_string(),
            text_value: String::new(),
            text_family: stencilcraft_engine::text::SINGLE_LINE_FAMILY.to_string(),
            text_size: 36.0,
            text_edit_id: None,
            text_original_font: None,
            text_fonts: Vec::new(),
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
            installed_theme: None,
            panels: panels::PanelState::default(),
        }
    }

    /// Run queued `--control` requests against the document on screen. The socket
    /// hands work to the UI thread, so an agent edits what the user sees (and the
    /// user can undo it) instead of a private copy of the file.
    fn drain_control_requests(&mut self, ctx: &egui::Context) {
        let Some(jobs) = self.control_jobs.as_ref() else {
            return;
        };
        let mut pending = Vec::new();
        while let Some(job) = jobs.try_recv() {
            pending.push(job);
        }
        for job in pending {
            if let Some(path) = screenshot_request(&job.line) {
                request_screenshot(ctx, path, job);
                continue;
            }
            if let Some((id, ids)) = select_request(&job.line) {
                // Selection is view state, not a document command: the window owns it.
                self.set_selection(ids);
                let reply = serde_json::json!({"id": id, "ok": true, "result": {"selected": self.selected_shapes}});
                let _ = job.reply.send(reply.to_string());
                continue;
            }
            if let Some((id, kerf_preview, theme)) = view_request(&job.line) {
                if let Some(on) = kerf_preview {
                    self.kerf_preview = on;
                }
                if let Some(theme) = theme {
                    self.theme = theme;
                }
                let dark = self.theme == stencilcraft_ui_egui::ColorTheme::Dark;
                let reply = serde_json::json!({"id": id, "ok": true, "result": {
                    "kerf_preview": self.kerf_preview,
                    "theme": if dark { "dark" } else { "light" },
                }});
                let _ = job.reply.send(reply.to_string());
                continue;
            }
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
            if let Some((method, path)) = document_path_request(&job.line, &reply) {
                // An agent opened or saved the visible document: the title, recent files and
                // framing follow it as they do for File › Open and Save.
                if method == "doc.open" {
                    self.set_selection([]);
                    self.view_initialized = false;
                    self.last_board = None;
                }
                self.open_path = Some(path.clone());
                self.saved_document = self.engine.doc.clone();
                self.record_recent_file(path);
            }
            let _ = job.reply.send(reply);
        }
        if let Some(jobs) = self.control_jobs.as_ref() {
            jobs.publish(&self.engine.doc);
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

    /// Whether every guide is locked (and there is at least one): View › Guides › Lock Guides.
    pub fn guides_locked(&self) -> bool {
        let guides = &self.engine.doc.guides;
        !guides.is_empty() && guides.iter().all(|g| g.locked)
    }

    /// Change snapping settings through `snap.set`, leaving the fields `edit` doesn't set.
    fn run_snap_set(&mut self, edit: impl FnOnce(&mut SnapToggles)) {
        let mut t = SnapToggles::default();
        edit(&mut t);
        self.run_command(Command::Document(DocumentCommand::SnapSet {
            grid: t.grid,
            guides: None,
            objects: t.objects,
            spacing: t.spacing,
            points: t.points,
            tolerance_mm: None,
        }));
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
                self.saved_document = self.engine.doc.clone();
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
                self.saved_document = self.engine.doc.clone();
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
            let selectable = self
                .engine
                .doc
                .get_shape(id)
                .is_some_and(stencilcraft_ui_egui::is_selectable);
            if selectable && !selected.contains(&id) {
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

    /// Move the dragged selection so its bounds sit `total_screen` from where the drag
    /// began, snapped by `snap.point` (grid, guides, objects, spacing, points as the
    /// document's snap settings say). The snap's lines become the smart guides.
    pub fn drag_selection_to(&mut self, id: u64, total_screen: egui::Vec2) {
        let Some(origin) = self.drag_origin else {
            return;
        };
        let d = self.view.screen_delta_to_world(total_screen);
        let (mut x, mut y) = (origin.x0 + d.x, origin.y0 + d.y);
        self.snap_lines.clear();
        let snap = &self.engine.doc.snap;
        if snap.grid || snap.guides || snap.objects || snap.spacing || snap.points {
            let query = stencilcraft_engine::document::SnapQuery {
                x,
                y,
                width: origin.width(),
                height: origin.height(),
                exclude: self.selected_shapes.clone(),
                tolerance_mm: None,
            };
            if let Ok(stencilcraft_engine::Effect::Document(
                stencilcraft_engine::document::DocumentEffect::Snap(r),
            )) = self
                .engine
                .run(Command::Document(DocumentCommand::SnapPoint(query)))
            {
                (x, y) = (r.x, r.y);
                self.snap_lines = r
                    .lines
                    .iter()
                    .map(|l| {
                        (
                            stencilcraft_geom::Point::new(l.from[0], l.from[1]),
                            stencilcraft_geom::Point::new(l.to[0], l.to[1]),
                        )
                    })
                    .collect();
            }
        }
        let Some(now) =
            stencilcraft_ui_egui::selected_world_bounds(&self.engine.doc, &self.selected_shapes)
        else {
            return;
        };
        let (dx_mm, dy_mm) = (x - now.x0, y - now.y0);
        if dx_mm.abs() < 1e-9 && dy_mm.abs() < 1e-9 {
            return;
        }
        if self.selected_shapes.len() > 1 {
            self.run_command(Command::ShapesTransform {
                ids: self.selected_shapes.clone(),
                dx_mm,
                dy_mm,
                scale_x: 1.0,
                scale_y: 1.0,
                degrees: 0.0,
            });
        } else {
            self.run_command(Command::ShapeMove { id, dx_mm, dy_mm });
        }
    }

    /// Turn a canvas guide edit into `guide.*` commands: a drag shows as a preview and
    /// commits once; a guide dragged out of a ruler is added; one dropped off the page
    /// (or deleted) is removed. Keys are guide index + 1 ([`stencilcraft_ui_egui::canvas_guides`]).
    pub fn apply_guide_event(
        &mut self,
        event: Option<(Option<u64>, EditEvent<craft_ui::canvas::GuidePlace>)>,
    ) {
        let Some((key, event)) = event else {
            return;
        };
        match (key, event) {
            (
                Some(key),
                EditEvent::Begin {
                    value: Some(place), ..
                },
            )
            | (Some(key), EditEvent::Update { value: place, .. }) => {
                self.guide_preview = Some((key, place));
            }
            (_, EditEvent::Cancel { .. }) => self.guide_preview = None,
            (key, EditEvent::Commit { value: place, .. } | EditEvent::Set { value: place, .. }) => {
                self.guide_preview = None;
                let axis = match place.orientation {
                    craft_ui::canvas::Orientation::Horizontal => {
                        stencilcraft_doc::GuideAxis::Horizontal
                    }
                    craft_ui::canvas::Orientation::Vertical => {
                        stencilcraft_doc::GuideAxis::Vertical
                    }
                };
                let index = key.and_then(|k| usize::try_from(k.checked_sub(1)?).ok());
                let command = match (index, place.position) {
                    (None, Some(position_mm)) => DocumentCommand::GuideAdd { axis, position_mm },
                    (None, None) => return,
                    (Some(index), None) => DocumentCommand::GuideRemove { index },
                    (Some(index), Some(position_mm)) => {
                        if self.engine.doc.guides.get(index).map(|g| g.axis) != Some(axis) {
                            // shortcut: an Alt-turned guide is re-added on its new axis;
                            // two undo steps instead of one.
                            self.run_command(Command::Document(DocumentCommand::GuideRemove {
                                index,
                            }));
                            DocumentCommand::GuideAdd { axis, position_mm }
                        } else {
                            DocumentCommand::GuideMove { index, position_mm }
                        }
                    }
                };
                self.run_command(Command::Document(command));
            }
            _ => {}
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
        // Buttons and other controls can retain keyboard focus after a click. Only
        // text edits should consume Delete as text rather than removing shapes.
        if !ctx.text_edit_focused()
            && ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
            })
        {
            self.delete_selection();
            return;
        }
        if self.send_mode || ctx.egui_wants_keyboard_input() {
            return;
        }
        // Shifted shortcuts first: `consume_key` matches Cmd+A while Shift is also held.
        let cmd_shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        let cmd_alt = egui::Modifiers::COMMAND | egui::Modifiers::ALT;
        // Object and Path menu keys (menus.rs), modified variants before their plain twins.
        let edit_keys = [
            (cmd_alt, egui::Key::Num2, panels::EditAction::UnlockAll),
            (cmd_alt, egui::Key::Num3, panels::EditAction::ShowAll),
            (
                cmd_alt,
                egui::Key::J,
                panels::EditAction::Average(stencilcraft_engine::NodeAxis::Both),
            ),
            (cmd_shift, egui::Key::E, panels::EditAction::ReleaseCompound),
            (cmd_shift, egui::Key::J, panels::EditAction::Join),
            (
                egui::Modifiers::COMMAND,
                egui::Key::Num2,
                panels::EditAction::Lock,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::Num3,
                panels::EditAction::Hide,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::E,
                panels::EditAction::MakeCompound,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::J,
                panels::EditAction::Simplify,
            ),
        ];
        // View menu keys (menus.rs), modified variants before their plain twins.
        let view_keys = [
            (cmd_alt, egui::Key::Num0, TopMenuAction::ZoomSelection),
            (cmd_alt, egui::Key::Num5, TopMenuAction::ReleaseGuides),
            (
                cmd_alt,
                egui::Key::Semicolon,
                TopMenuAction::ToggleLockGuides,
            ),
            (cmd_alt, egui::Key::Quote, TopMenuAction::ToggleSnapPoint),
            (cmd_shift, egui::Key::Quote, TopMenuAction::ToggleSnapGrid),
            (
                egui::Modifiers::COMMAND,
                egui::Key::Num5,
                TopMenuAction::MakeGuides,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::Semicolon,
                TopMenuAction::ToggleShowGuides,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::U,
                TopMenuAction::ToggleSmartGuides,
            ),
        ];
        if let Some(action) = view_keys.into_iter().find_map(|(modifiers, key, action)| {
            ctx.input_mut(|input| input.consume_key(modifiers, key))
                .then_some(action)
        }) {
            self.handle_menu_action(action);
        } else if let Some(action) = edit_keys.into_iter().find_map(|(modifiers, key, action)| {
            ctx.input_mut(|input| input.consume_key(modifiers, key))
                .then_some(action)
        }) {
            self.handle_edit_action(action);
        } else if ctx.input_mut(|input| input.consume_key(cmd_shift, egui::Key::A)) {
            self.handle_menu_action(TopMenuAction::Deselect);
        } else if ctx.input_mut(|input| input.consume_key(cmd_shift, egui::Key::D)) {
            self.handle_menu_action(TopMenuAction::TransformAgain);
        } else if ctx.input_mut(|input| input.consume_key(cmd_shift, egui::Key::W)) {
            self.handle_menu_action(TopMenuAction::Pathfinder(BooleanOperation::Union));
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::L)) {
            self.handle_menu_action(TopMenuAction::ShowSend);
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::A)) {
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
            input.consume_key(cmd_shift, egui::Key::Z)
                || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
        }) {
            let _ = self.engine.redo();
            self.retain_valid_selection();
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            let _ = self.engine.undo();
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
            self.open_text_dialog(false);
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
                stencilcraft_svg::import_with_warnings(&svg, &name)
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(imported) => {
                self.add_imported_shapes(imported.document, &name);
                // Skipped text, masks and filters must reach the user, not vanish.
                if let Some(first) = imported.warnings.first() {
                    let more = imported.warnings.len() - 1;
                    self.status = if more == 0 {
                        format!("{}; warning: {first}", self.status)
                    } else {
                        format!("{}; warning: {first} (+{more} more)", self.status)
                    };
                }
            }
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

    /// Open a new text dialog, or edit the selected live text shape.
    fn open_text_dialog(&mut self, edit: bool) {
        if edit {
            let selected = self.selected_shape.and_then(|id| {
                self.engine
                    .doc
                    .get_shape(id)
                    .and_then(|shape| shape.text.as_deref().map(|spec| (id, spec.clone())))
            });
            let Some((id, spec)) = selected else {
                self.status = "select a live text object to edit".to_string();
                return;
            };
            self.text_value = spec.text;
            self.text_family = spec.font.family.clone();
            self.text_size = spec.size;
            self.text_original_font = Some(spec.font);
            self.text_edit_id = Some(id);
        } else {
            self.text_edit_id = None;
            self.text_original_font = None;
        }
        if let Ok(stencilcraft_engine::Effect::Fonts { families }) =
            self.engine.run(Command::TextFonts {
                query: String::new(),
            })
        {
            self.text_fonts = families;
        }
        self.text_before = None;
        self.text_dialog = true;
    }

    /// Create or update text through the engine, preserving editable text metadata.
    pub fn add_text(&mut self) {
        let font = self
            .text_original_font
            .as_ref()
            .filter(|font| font.family == self.text_family)
            .cloned()
            .unwrap_or_else(|| {
                let style = self
                    .text_fonts
                    .iter()
                    .find(|family| family.family == self.text_family)
                    .and_then(|family| {
                        family
                            .styles
                            .iter()
                            .find(|style| style.eq_ignore_ascii_case("regular"))
                            .or_else(|| family.styles.first())
                    })
                    .cloned()
                    .unwrap_or_else(|| "Regular".to_string());
                stencilcraft_doc::FontSpec {
                    family: self.text_family.clone(),
                    style,
                    file: None,
                }
            });
        let command = if let Some(id) = self.text_edit_id {
            Command::TextEdit(stencilcraft_engine::text::TextEdit {
                id,
                text: Some(self.text_value.clone()),
                font: Some(font),
                size: Some(self.text_size),
                ..Default::default()
            })
        } else {
            Command::TextAdd(stencilcraft_engine::text::TextAdd {
                spec: stencilcraft_doc::TextSpec {
                    text: self.text_value.clone(),
                    font,
                    size: self.text_size,
                    x: 20.0,
                    y: 20.0,
                    alignment: stencilcraft_doc::TextAlign::Left,
                    letter_spacing: 0.0,
                    line_spacing: 100.0,
                    kerning: true,
                    vertical: false,
                    weld: false,
                    flow: stencilcraft_doc::TextFlow::Straight,
                    letters: Vec::new(),
                    transform: stencilcraft_doc::IDENTITY_TRANSFORM,
                },
                style: None,
            })
        };
        match self.engine.run(command) {
            Ok(stencilcraft_engine::Effect::ShapeAdded { id }) => {
                self.set_selection([id]);
                self.status = "text added".to_string();
            }
            Ok(_) => self.status = "text updated".to_string(),
            Err(error) => self.status = error.to_string(),
        }
    }

    pub fn text_dialog(&mut self, parent: &mut egui::Ui) {
        if !self.text_dialog {
            return;
        }
        let mut before = self.text_before.take();
        let title = if self.text_edit_id.is_some() {
            "Edit text"
        } else {
            "Add text"
        };
        let fonts = &self.text_fonts;
        let family = &mut self.text_family;
        let size = &mut self.text_size;
        let numbers = &self.locale_numbers;
        let shown = Dialog::new("place-text-dialog", title)
            .footer(Footer::standard().ok("Apply").cancel("Cancel"))
            .show(parent.ctx(), &mut self.text_dialog_state, &mut self.text_value, |ui, text| {
                ui.add(Text::new("Text stays editable; the single-line font is suitable for pens and engraving.").content(Content::Muted));
                let edit = TextField::new("place-text", "Text to place", text.as_str()).char_limit(10_000).show(ui);
                apply_edit(text, &mut before, edit.event);
                let mut select = Select::new("text-font", "Font family", family);
                for font in fonts {
                    select = select.option(font.family.clone(), &font.family);
                }
                let _ = select.show(ui);
                let edit = NumberField::new("text-size", "Size", *size, numbers)
                    .range(0.01..=10_000.0).unit(Unit::new("pt")).show(ui);
                if let Some(value) = edit.event.and_then(|event| event.value().copied()) && value.is_finite() {
                    *size = value;
                }
            });
        self.text_before = before;
        if shown.closed {
            self.text_dialog = false;
        }
        if shown.outcome == Some(Outcome::Ok) {
            self.add_text();
        }
    }

    /// Opens File › New on `tab` (0 Recent, 1.. the preset categories).
    pub fn open_new_document(&mut self, tab: usize) {
        let kerf = self.engine.doc.kerf.width_mm;
        self.new_doc.open(tab, kerf, self.mat, self.send.model);
    }

    pub fn new_document_dialog(&mut self, parent: &mut egui::Ui) {
        if let Some(choice) = self.new_doc.show(parent.ctx(), &self.prefs.recent_new) {
            self.request_new_document(choice);
        }
        if self.pending_new_document.is_some() {
            let shown = Dialog::new("discard-before-new", "Discard unsaved changes?")
                .footer(Footer::standard().ok("Discard changes").cancel("Cancel"))
                .show(parent.ctx(), &mut self.discard_new_dialog, &mut (), |ui, ()| {
                    ui.add(Text::new("Creating a new document will discard changes. Cancel to save your work first.").content(Content::Muted));
                });
            if shown.outcome == Some(Outcome::Ok) {
                self.confirm_new_document();
            } else if shown.closed {
                self.pending_new_document = None;
            }
        }
    }

    fn request_new_document(&mut self, choice: new_doc::Choice) {
        if self.engine.doc != self.saved_document {
            self.pending_new_document = Some(choice);
        } else {
            self.create_document(&choice);
        }
    }

    fn confirm_new_document(&mut self) {
        if let Some(choice) = self.pending_new_document.take() {
            self.create_document(&choice);
        }
    }

    /// Create in New Document: a fresh board and kerf through the engine, then the mat,
    /// machine and title the details chose.
    pub fn create_document(&mut self, choice: &new_doc::Choice) {
        for command in choice.commands() {
            if let Err(e) = self.engine.run(command) {
                self.status = e.to_string();
                return;
            }
        }
        self.saved_document = self.engine.doc.clone();
        self.mat = choice.mat;
        self.send.model = choice.model;
        self.doc_title = choice.name.trim().to_string();
        self.open_path = None;
        self.set_selection([]);
        self.view_initialized = false;
        self.last_board = None;
        new_doc::push_recent(
            &mut self.prefs.recent_new,
            choice.recent(),
            new_doc::MAX_RECENT,
        );
        self.save_prefs();
        self.status = format!(
            "new document {}",
            new_doc::size_line(choice.width_mm, choice.height_mm, choice.units)
        );
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
        if let Some(command) = self.dragged_shape_command(start, end) {
            self.run_command(command);
            self.select_latest_shape();
        }
    }

    fn dragged_shape_command(
        &self,
        start: stencilcraft_geom::Point,
        end: stencilcraft_geom::Point,
    ) -> Option<Command> {
        let x = start.x.min(end.x);
        let y = start.y.min(end.y);
        let w = (start.x - end.x).abs();
        let h = (start.y - end.y).abs();
        if w < 1.0 && h < 1.0 {
            return None;
        }
        match self.active_tool {
            ActiveTool::Rect => Some(Command::ShapeAddRect {
                x,
                y,
                width: w.max(2.0),
                height: h.max(2.0),
            }),
            ActiveTool::RoundedRect => Some(Command::ShapeAddRoundedRect {
                x,
                y,
                width: w.max(2.0),
                height: h.max(2.0),
                radius: (w.min(h) * 0.15).clamp(1.0, 15.0),
            }),
            ActiveTool::Ellipse => Some(Command::ShapeAddEllipse {
                x,
                y,
                width: w.max(2.0),
                height: h.max(2.0),
            }),
            ActiveTool::Polygon => Some(Command::ShapeAddPolygon {
                cx: (start.x + end.x) / 2.0,
                cy: (start.y + end.y) / 2.0,
                radius: (w.max(h) / 2.0).max(2.0),
                sides: 6,
                rotation: 0.0,
            }),
            ActiveTool::Star => Some(Command::ShapeAddStar {
                cx: (start.x + end.x) / 2.0,
                cy: (start.y + end.y) / 2.0,
                radius: (w.max(h) / 2.0).max(3.0),
                inner_radius: (w.max(h) / 4.0).max(1.5),
                points: 5,
                rotation: 0.0,
            }),
            _ => None,
        }
    }

    fn build_menu_bar(&self) -> Vec<MenuItem<TopMenuAction>> {
        menus::build(self)
    }

    pub fn handle_menu_action(&mut self, action: TopMenuAction) {
        match action {
            TopMenuAction::NewDoc => self.open_new_document(0),
            TopMenuAction::NewFromTemplate => self.open_new_document(2),
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
                        .set_file_name(format!("{}.stencilcraft", self.doc_title))
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
            TopMenuAction::RotateBy(degrees) => {
                self.run_command(Command::Document(DocumentCommand::TransformEach(
                    TransformRequest {
                        ids: self.selected_shapes.clone(),
                        degrees: f64::from(degrees),
                        ..TransformRequest::default()
                    },
                )));
            }
            TopMenuAction::Flip(axis) => {
                let (scale_x, scale_y) = match axis {
                    DistributionAxis::Horizontal => (-1.0, 1.0),
                    DistributionAxis::Vertical => (1.0, -1.0),
                };
                self.run_command(Command::Document(DocumentCommand::TransformEach(
                    TransformRequest {
                        ids: self.selected_shapes.clone(),
                        scale_x,
                        scale_y,
                        ..TransformRequest::default()
                    },
                )));
            }
            TopMenuAction::AlignToKey(alignment) => {
                if let Some(key) = self.selected_shape {
                    self.run_command(Command::Document(DocumentCommand::ShapesAlignToKey {
                        ids: self.selected_shapes.clone(),
                        key,
                        alignment,
                    }));
                }
            }
            TopMenuAction::TransformAgain => {
                self.run_command(Command::Document(DocumentCommand::TransformAgain {
                    ids: self.selected_shapes.clone(),
                }));
            }
            TopMenuAction::CenterToPage => {
                for alignment in [Alignment::HorizontalCenter, Alignment::VerticalCenter] {
                    self.run_command(Command::ShapesAlign {
                        ids: self.selected_shapes.clone(),
                        alignment,
                        to_board: true,
                    });
                }
            }
            TopMenuAction::SetStyle(style) => self.run_command(Command::ShapesSetStyle {
                ids: self.selected_shapes.clone(),
                style,
            }),
            TopMenuAction::PlaceText => {
                self.active_tool = ActiveTool::Text;
                self.open_text_dialog(false);
            }
            TopMenuAction::EditText => self.open_text_dialog(true),
            TopMenuAction::SelectInverse => {
                self.handle_edit_action(panels::EditAction::SelectInverse);
            }
            TopMenuAction::SelectSameStyle => {
                let styles: Vec<CutStyle> = self
                    .selected_shapes
                    .iter()
                    .filter_map(|id| self.engine.doc.get_shape(*id).map(|s| s.style))
                    .collect();
                let same: Vec<u64> = self
                    .engine
                    .doc
                    .shapes()
                    .filter(|(_, shape)| styles.contains(&shape.style))
                    .map(|(_, shape)| shape.id)
                    .collect();
                self.set_selection(same);
            }
            TopMenuAction::SelectSameLayer => {
                let same: Vec<u64> = self
                    .engine
                    .doc
                    .layers
                    .iter()
                    .filter(|layer| {
                        layer
                            .shapes
                            .iter()
                            .any(|shape| self.selected_shapes.contains(&shape.id))
                    })
                    .flat_map(|layer| layer.shapes.iter().map(|shape| shape.id))
                    .collect();
                self.set_selection(same);
            }
            TopMenuAction::ShowDesign => self.send_mode = false,
            TopMenuAction::ShowSend => self.send_mode = true,
            TopMenuAction::ShowPanel(panel) => {
                let action = if self.workspace.contains(&panel) {
                    craft_ui::docking::Action::Activate { panel }
                } else {
                    craft_ui::docking::Action::Open {
                        panel,
                        anchor: None,
                    }
                };
                if let Err(error) = self.workspace.apply(action) {
                    self.status = format!("could not show panel: {error}");
                }
            }
            TopMenuAction::DirectSelectTool => self.active_tool = ActiveTool::DirectSelect,
            TopMenuAction::DryRun => {
                self.send_mode = true;
                self.send.summary = Some(self.encode_send_summary());
            }
            TopMenuAction::Quit => {
                self.quit_requested = true;
            }
            TopMenuAction::NotYet => self.status = menus::NOT_YET.to_lowercase(),
            TopMenuAction::Edit(action) => self.handle_edit_action(action),
            TopMenuAction::FitView => self.pending_view_command = Some(CanvasViewCommand::Fit),
            TopMenuAction::CenterView => {
                self.pending_view_command = Some(CanvasViewCommand::Center)
            }
            TopMenuAction::ActualSize => {
                self.pending_view_command = Some(CanvasViewCommand::ZoomPercent(100))
            }
            TopMenuAction::ZoomSelection => {
                if self.selected_shapes.is_empty() {
                    self.status = "zoom to selection: select shapes first".to_string();
                } else {
                    self.pending_view_command = Some(CanvasViewCommand::Selection);
                }
            }
            TopMenuAction::ZoomPercent(percent) => {
                self.pending_view_command = Some(CanvasViewCommand::ZoomPercent(percent))
            }
            TopMenuAction::RotateView(turns) => {
                self.pending_view_command = Some(CanvasViewCommand::Rotate(turns))
            }
            TopMenuAction::ToggleSnapGrid => {
                let on = !self.engine.doc.snap.grid;
                self.run_snap_set(|s| s.grid = Some(on));
            }
            TopMenuAction::ToggleSmartSnapping => {
                // Silhouette's Smart Snapping: other objects' edges, centres and spacing.
                let on = !self.engine.doc.snap.objects;
                self.run_snap_set(|s| {
                    s.objects = Some(on);
                    s.spacing = Some(on);
                });
            }
            TopMenuAction::ToggleSnapPoint => {
                let on = !self.engine.doc.snap.points;
                self.run_snap_set(|s| s.points = Some(on));
            }
            TopMenuAction::ToggleSmartGuides => {
                self.prefs.smart_guides = !self.prefs.smart_guides;
                self.save_prefs();
            }
            TopMenuAction::ToggleShowGuides => {
                self.prefs.show_guides = !self.prefs.show_guides;
                self.save_prefs();
            }
            TopMenuAction::ToggleLockGuides => {
                let locked = !self.guides_locked();
                self.run_command(Command::Document(DocumentCommand::GuideLockAll { locked }));
            }
            TopMenuAction::MakeGuides => {
                let ids = self.selected_shapes.clone();
                self.run_command(Command::Document(DocumentCommand::GuideMake { ids }));
                self.retain_valid_selection();
            }
            TopMenuAction::ReleaseGuides => {
                if let Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) = self
                    .engine
                    .run(Command::Document(DocumentCommand::GuideRelease {}))
                {
                    self.status = format!("released {} guides", ids.len());
                    self.set_selection(ids);
                }
            }
            TopMenuAction::ClearGuides => {
                self.run_command(Command::Document(DocumentCommand::GuideClear {}))
            }
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
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}.stencilcraft", self.doc_title));
        let title_text = format!("StencilCraft — {doc_name}");

        let menus = self.build_menu_bar();
        let mut chosen_action = None;
        let mut undo_clicked = false;
        let mut redo_clicked = false;
        let mut switch_theme_clicked = false;

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
                            // The trailing slot lays out right to left: Redo before Undo.
                            ui.horizontal(|ui| {
                                if ui
                                    .add(IconButton::new(Icon::Redo, "Redo").enabled(can_redo))
                                    .clicked()
                                {
                                    redo_clicked = true;
                                }
                                if ui
                                    .add(IconButton::new(Icon::Undo, "Undo").enabled(can_undo))
                                    .clicked()
                                {
                                    undo_clicked = true;
                                }
                                Divider::vertical().show(ui);
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
        let selection_bounds =
            stencilcraft_ui_egui::selected_world_bounds(&self.engine.doc, &self.selected_shapes)
                .map(|b| [b.x0, b.y0, b.width(), b.height()]);
        let mut pivot = self.panels.pivot;
        let mut link_aspect = self.prefs.link_aspect;
        let mut bound_edit = None;
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
                            ui.add(Text::new("No selection"));
                            Divider::vertical().show(ui);
                            let mut kerf_val = kerf_val_init;
                            ui.add(Text::new("Kerf:"));
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
                        } else {
                            let title = match (count, selected_shape_id) {
                                (1, Some(id)) => format!("Shape #{id}"),
                                _ => format!("{count} shapes"),
                            };
                            ui.add(Text::new(title));
                            if let Some(b) = selection_bounds {
                                Divider::vertical().show(ui);
                                let picked = ReferencePoint::new("opt-ref-point", Some(pivot))
                                    .size(RefSize::Compact)
                                    .show(ui);
                                if let Some(EditEvent::Set {
                                    value: Some(anchor),
                                    ..
                                }) = picked.event
                                {
                                    pivot = anchor;
                                }
                                let (u, v) = pivot.uv();
                                let shown = [b[0] + u * b[2], b[1] + v * b[3], b[2], b[3]];
                                for (i, name) in ["X", "Y", "W", "H"].into_iter().enumerate() {
                                    if i == 3 {
                                        let _ = LinkButton::new(
                                            "opt-aspect-link",
                                            &mut link_aspect,
                                            LinkMode::Ratio,
                                        )
                                        .size(ControlSize::Sm)
                                        .show(ui);
                                    }
                                    ui.add(Text::new(format!("{name}:")));
                                    let min = if i < 2 { -10_000.0 } else { 0.01 };
                                    let edit = NumberField::new(
                                        ("opt-bound", selected_shape_id, i),
                                        name,
                                        shown[i],
                                        &locale_numbers,
                                    )
                                    .range(min..=10_000.0)
                                    .decimals(0..=2)
                                    .unit(Unit::new("{n} mm"))
                                    .show(ui);
                                    if let Some(
                                        EditEvent::Commit { value, .. }
                                        | EditEvent::Set { value, .. },
                                    ) = edit.event
                                    {
                                        bound_edit = Some((b, i, value));
                                    }
                                }
                            }
                        }
                        if count >= 2 {
                            Divider::vertical().show(ui);
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
                        Divider::vertical().show(ui);
                        ui.add(Text::new(format!("{object_count} objects")));
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

        self.panels.pivot = pivot;
        if link_aspect != self.prefs.link_aspect {
            self.prefs.link_aspect = link_aspect;
            self.save_prefs();
        }
        if let Some((from, field, value)) = bound_edit {
            let to = panels::edit_bounds(from, pivot, field, value, link_aspect);
            if let Some(command) = panels::bounds_command(self.selected_shapes.clone(), from, to) {
                self.run_command(command);
            }
        }
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

    pub fn tools_panel(&mut self, ui: &mut egui::Ui) {
        let has_selection = !self.selected_shapes.is_empty();
        let mut lasso = self.canvas_lasso_mode;
        let mut clicked = None;
        let _ = Panel::new("stencilcraft-tools-panel").show(ui, |ui| {
            let _ = SectionHeader::new("tools-select", "Select")
                .collapsible(false)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let _ = ui.add(
                            ToggleButton::new("Lasso select", &mut lasso)
                                .icon(Icon::Lasso)
                                .size(ControlSize::Sm),
                        );
                        for (label, icon, enabled, tool) in [
                            (
                                "Deselect",
                                Icon::Close,
                                has_selection,
                                ToolsAction::Deselect,
                            ),
                            (
                                "Duplicate",
                                Icon::Copy,
                                has_selection,
                                ToolsAction::Duplicate,
                            ),
                            ("Delete", Icon::Trash, has_selection, ToolsAction::Delete),
                        ] {
                            let button = Button::new(label)
                                .icon(icon)
                                .enabled(enabled)
                                .size(ControlSize::Sm);
                            if ui.add(button).clicked() {
                                clicked = Some(tool);
                            }
                        }
                    });
                });
            let _ = SectionHeader::new("tools-add", "Add")
                .collapsible(false)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (label, icon, tool) in [
                            ("Rectangle", Icon::Rectangle, ToolsAction::Rectangle),
                            ("Image", Icon::Image, ToolsAction::Image),
                            ("Text", Icon::Text, ToolsAction::Text),
                        ] {
                            let button = Button::new(label).icon(icon).size(ControlSize::Sm);
                            if ui.add(button).clicked() {
                                clicked = Some(tool);
                            }
                        }
                    });
                });
        });
        self.canvas_lasso_mode = lasso;
        match clicked {
            Some(ToolsAction::Deselect) => self.set_selection([]),
            Some(ToolsAction::Duplicate) => self.duplicate_selection(),
            Some(ToolsAction::Delete) => self.delete_selection(),
            Some(ToolsAction::Rectangle) => {
                self.run_command(Command::ShapeAddRect {
                    x: 20.0,
                    y: 20.0,
                    width: 40.0,
                    height: 30.0,
                });
                self.select_latest_shape();
            }
            Some(ToolsAction::Image) => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG or JPEG", &["png", "jpg", "jpeg"])
                    .pick_file()
                {
                    self.import_image(path);
                }
            }
            Some(ToolsAction::Text) => self.open_text_dialog(false),
            None => {}
        }
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
        // Every shape with its eye (shown) and lock toggles: hidden and locked shapes are left
        // off the canvas's picking, so this list is where they come back.
        let shape_rows: Vec<(u64, String, bool, bool)> = self
            .engine
            .doc
            .shapes()
            .map(|(_, s)| {
                let kind = match s.style {
                    CutStyle::Cut => "Cut",
                    CutStyle::Engrave => "Engrave",
                };
                (s.id, format!("#{} {kind}", s.id), !s.hidden, s.locked)
            })
            .collect();
        let mut shape_toggles: Vec<(u64, usize, bool)> = Vec::new();
        let mut clicked_shape = None;

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
                let _ = SectionHeader::new("layers-objects", "Objects").show(ui, |ui| {
                    for (id, label, shown, locked) in &shape_rows {
                        let row = ListRow::new(*id, label)
                            .icon(Icon::Spline)
                            .toggle(ToggleColumn::eye(format!("Visibility {label}"), *shown))
                            .toggle(ToggleColumn::lock(format!("Lock {label}"), *locked))
                            .selected(self.selected_shapes.contains(id))
                            .dimmed(!*shown)
                            .show(ui);
                        if row.response.clicked() {
                            clicked_shape = Some(*id);
                        }
                        for ev in row.events {
                            if let craft_ui::list::RowEvent::Toggle(
                                EditEvent::Commit { value, .. } | EditEvent::Set { value, .. },
                            ) = ev
                            {
                                shape_toggles.push((value.row, value.column, value.on));
                            }
                        }
                    }
                });
            });
        for (id, column, on) in shape_toggles {
            self.run_command(if column == 0 {
                Command::ShapesSetHidden {
                    ids: vec![id],
                    hidden: !on,
                }
            } else {
                Command::ShapesSetLocked {
                    ids: vec![id],
                    locked: on,
                }
            });
            self.retain_valid_selection();
        }
        if let Some(id) = clicked_shape {
            self.set_selection([id]);
        }

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
        let (header, target) = if self.send_mode {
            (
                InspectorHeader::new(Icon::Scissors, "Make").kind("USB cutter"),
                0,
            )
        } else {
            match self.selected_shapes.as_slice() {
                [] => (InspectorHeader::title_only("Document"), 1),
                [id] => (
                    InspectorHeader::new(Icon::Spline, format!("Shape #{id}")).kind("Path"),
                    2 + *id,
                ),
                ids => (
                    InspectorHeader::title_only(format!("{} objects", ids.len())),
                    1,
                ),
            }
        };
        Inspector::new("stencilcraft-inspector")
            .target(target)
            .header(header)
            .show(ui, |ui| {
                if self.send_mode {
                    ui.add(
                        Text::new("Review your material and machine settings before cutting.")
                            .role(TypeRole::Caption)
                            .content(Content::Muted),
                    );
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
        PropertyGroup::new("design-mat-group").show(ui, |ui| {
            PropertyRow::word("Cutting mat").show(ui, |ui, _| {
                let mut mat_select = Select::new("design-mat", "Cutting mat", &mut self.mat);
                for (mat, label) in MAT_CHOICES {
                    mat_select = mat_select.option(*mat, *label);
                }
                mat_select.show(ui)
            });
        });

        if let Some(id) = (self.selected_shapes.len() == 1)
            .then_some(self.selected_shape)
            .flatten()
        {
            if let Some(bounds) = self
                .engine
                .doc
                .get_shape(id)
                .and_then(stencilcraft_doc::Shape::bounds)
            {
                let mut values = [bounds.x0, bounds.y0, bounds.width(), bounds.height()];
                let mut committed = false;
                let mut action = None;
                let numbers = self.locale_numbers.clone();
                let _ = SectionHeader::new("inspector-transform", "Transform").show(ui, |ui| {
                    PropertyGroup::new(("transform", id)).show(ui, |ui| {
                        for (a, b) in [(0, 1), (2, 3)] {
                            let names = ["X", "Y", "W", "H"];
                            let shown = values;
                            let field = |ui: &mut egui::Ui, index: usize| {
                                let min = if index < 2 { -10_000.0 } else { 0.01 };
                                NumberField::new(
                                    ("bound", id, index),
                                    names[index],
                                    shown[index],
                                    &numbers,
                                )
                                .range(min..=10_000.0)
                                .decimals(0..=2)
                                .unit(Unit::new("{n} mm"))
                                .show(ui)
                            };
                            let (first, second) = PropertyRow::pair(
                                PropertyLabel::glyph(names[a]),
                                PropertyLabel::glyph(names[b]),
                            )
                            .show(ui, |ui, _| field(ui, a), |ui, _| field(ui, b))
                            .inner;
                            for (index, event) in [(a, first.event), (b, second.event)] {
                                if let Some(
                                    EditEvent::Commit { value, .. } | EditEvent::Set { value, .. },
                                ) = event
                                {
                                    values[index] = value;
                                    committed = true;
                                }
                            }
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        for (label, op) in [
                            ("−15°", ShapeAction::Rotate(-15.0)),
                            ("+15°", ShapeAction::Rotate(15.0)),
                            ("−10%", ShapeAction::Scale(0.9)),
                            ("+10%", ShapeAction::Scale(1.1)),
                        ] {
                            if ui.add(Button::new(label).size(ControlSize::Sm)).clicked() {
                                action = Some(op);
                            }
                        }
                    });
                    ui.add(
                        Text::new("Drag an object on the mat to reposition it.")
                            .role(TypeRole::Caption)
                            .content(Content::Muted),
                    );
                });
                if committed {
                    let [x, y, width, height] = values;
                    self.run_command(Command::ShapeSetBounds {
                        id,
                        x,
                        y,
                        width,
                        height,
                    });
                }
                match action {
                    Some(ShapeAction::Rotate(degrees)) => {
                        self.run_command(Command::ShapeRotate { id, degrees });
                    }
                    Some(ShapeAction::Scale(s)) => {
                        self.run_command(Command::ShapeScale {
                            id,
                            scale_x: s,
                            scale_y: s,
                        });
                    }
                    None => {}
                }
            } else if self.engine.doc.get_shape(id).is_none() {
                self.set_selection([]);
            }
        }

        let mut add_layer = false;
        let mut visibility_changes: Vec<(usize, bool)> = Vec::new();
        let layers: Vec<(String, bool)> = self
            .engine
            .doc
            .layers
            .iter()
            .map(|layer| {
                (
                    format!("{}  ·  {}", layer.name, layer.shapes.len()),
                    layer.visible,
                )
            })
            .collect();
        let _ = SectionHeader::new("inspector-layers", "Layers")
            .controls(|ui| {
                add_layer = ui.add(IconButton::new(Icon::Plus, "Add Layer")).clicked();
            })
            .show(ui, |ui| {
                for (i, (label, visible)) in layers.into_iter().enumerate() {
                    let mut visible_now = visible;
                    let response = Checkbox::bool(label, &mut visible_now)
                        .id_salt(("layer-visibility", i))
                        .show(ui);
                    if response.response.changed() {
                        visibility_changes.push((i, visible_now));
                    }
                }
            });
        if add_layer {
            self.run_command(Command::LayerAdd {
                name: format!("Layer {}", self.engine.doc.layers.len() + 1),
                kind: stencilcraft_doc::CutStyle::Cut,
            });
        }
        // Visibility is document state: it goes through the engine so it undoes.
        for (index, visible) in visibility_changes {
            self.run_command(Command::LayerSetVisible { index, visible });
        }

        let shape_rows: Vec<(u64, String)> = self
            .engine
            .doc
            .shapes()
            .map(|(_, shape)| (shape.id, format!("Shape #{} · {:?}", shape.id, shape.style)))
            .collect();
        let mut clicked_shape = None;
        let _ = SectionHeader::new("inspector-objects", "Objects").show(ui, |ui| {
            for (id, label) in shape_rows {
                let row = ListRow::new(id, &label)
                    .icon(Icon::Spline)
                    .selected(self.selected_shapes.contains(&id))
                    .show(ui);
                if row.response.clicked() {
                    clicked_shape = Some(id);
                }
            }
        });
        if let Some(id) = clicked_shape {
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

        if self.selected_shapes.len() >= 2 {
            let mut operation = None;
            let _ = SectionHeader::new("inspector-pathfinder", "Pathfinder").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (label, op) in [
                        ("Unite", BooleanOperation::Union),
                        ("Minus Front", BooleanOperation::Difference),
                        ("Intersect", BooleanOperation::Intersect),
                        ("Exclude", BooleanOperation::Xor),
                        ("Divide", BooleanOperation::Divide),
                        ("Trim", BooleanOperation::Trim),
                        ("Merge", BooleanOperation::Merge),
                        ("Crop", BooleanOperation::Crop),
                        ("Outline", BooleanOperation::Outline),
                        ("Minus Back", BooleanOperation::MinusBack),
                    ] {
                        if ui.add(Button::new(label).size(ControlSize::Sm)).clicked() {
                            operation = Some(op);
                        }
                    }
                });
            });
            if let Some(op) = operation {
                self.run_pathfinder(op);
            }
            let mut picked = None;
            let _ = SectionHeader::new("inspector-align", "Align").show(ui, |ui| {
                picked = AlignBar::new("inspector-align-bar")
                    .enabled(|action| align_command(action).is_some())
                    .show(ui)
                    .picked;
            });
            match picked.and_then(align_command) {
                Some(AlignCommand::Align(alignment)) => {
                    self.run_command(Command::ShapesAlign {
                        ids: self.selected_shapes.clone(),
                        alignment,
                        to_board: false,
                    });
                }
                Some(AlignCommand::Distribute(axis)) => {
                    self.run_command(Command::ShapesDistribute {
                        ids: self.selected_shapes.clone(),
                        axis,
                    });
                }
                None => {}
            }
            ui.horizontal(|ui| {
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

        let kerf_width = self.kerf_draft.unwrap_or(self.engine.doc.kerf.width_mm);
        let mut kerf_event = None;
        let mut kerf_preview = self.kerf_preview;
        let _ = SectionHeader::new("inspector-cut", "Cut settings").show(ui, |ui| {
            ui.add(
                Text::new(format!("Kerf  {kerf_width:.3} mm"))
                    .role(TypeRole::Label)
                    .content(Content::Muted),
            );
            kerf_event = Slider::new("kerf", "Kerf", kerf_width, 0.0..=1.0)
                .show(ui)
                .event;
            let _ = Checkbox::bool("Preview kerf compensation", &mut kerf_preview).show(ui);
        });
        self.kerf_preview = kerf_preview;
        match kerf_event {
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
    }

    fn start_hardware(
        &mut self,
        command: stencilcraft_engine::HardwareCommand,
        ctx: egui::Context,
    ) {
        if self.send.busy {
            return;
        }
        self.send.busy = true;
        let result = self.send.hardware_result.clone();
        let doc = self.engine.doc.clone();
        let spawned = std::thread::Builder::new()
            .name("cutter-usb".into())
            .spawn(move || {
                let outcome = stencilcraft_engine::Engine::new(doc)
                    .run(Command::Hardware(command))
                    .map_err(|e| e.to_string());
                if let Ok(mut slot) = result.lock() {
                    *slot = Some(outcome);
                }
                ctx.request_repaint();
            });
        if let Err(error) = spawned {
            self.send.busy = false;
            self.send.summary = Some(vec![error.to_string()]);
        }
    }

    pub fn send_tab(&mut self, ui: &mut egui::Ui) {
        let result = self
            .send
            .hardware_result
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());
        let has_new_result = result.is_some();
        if let Some(result) = result {
            self.send.busy = false;
            match result {
                Ok(stencilcraft_engine::Effect::Hardware(value)) => {
                    if let Some(devices) = value.as_array() {
                        self.send.devices = devices
                            .iter()
                            .filter_map(|d| {
                                Some((
                                    d["location"].as_str()?.to_owned(),
                                    d["model"].as_str()?.to_owned(),
                                    d["name"].as_str()?.to_owned(),
                                ))
                            })
                            .collect();
                        self.send.device.clear();
                        let unsupported: Vec<_> = devices.iter().filter(|d| d["model"].is_null()).map(|d| format!("{}: direct cutting unsupported; export SVG for the manufacturer software.", d["product"].as_str().unwrap_or("Unknown USB cutter"))).collect();
                        self.send.summary = Some(vec![format!(
                            "{} recognized USB cutters. Select your cutter below.",
                            self.send.devices.len()
                        )]);
                        if let Some(summary) = &mut self.send.summary {
                            summary.extend(unsupported);
                        }
                    } else {
                        let bytes = value["bytes"].as_u64().unwrap_or_default();
                        self.send.summary = Some(if value["sent"].as_bool() == Some(true) {
                            vec![
                                format!("Sent {bytes} bytes to the cutter."),
                                "Sending finished; physical cutting may still be in progress."
                                    .into(),
                            ]
                        } else {
                            vec![
                                format!(
                                    "Validation passed: {bytes} bytes prepared. Nothing has been sent."
                                ),
                                "When ready, select Send cut to selected USB cutter.".into(),
                            ]
                        });
                    }
                }
                Err(error) => self.send.summary = Some(vec![error]),
                _ => {}
            }
        }
        if self.send.busy {
            ui.add(Text::new(
                "Communicating with cutter… Use the machine's controls to stop motion.",
            ));
            return;
        }
        if let Some(summary) = &self.send.summary {
            let response = ui
                .group(|ui| {
                    ui.add(Text::new("Cutter result").role(TypeRole::BodyStrong));
                    egui::ScrollArea::vertical()
                        .id_salt("cutter-result")
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for line in summary {
                                ui.add(Text::new(line).role(TypeRole::Mono));
                            }
                        });
                })
                .response;
            if has_new_result {
                response.scroll_to_me(Some(egui::Align::TOP));
            }
        }
        if ui.add(Button::new("Find USB cutters")).clicked() {
            self.start_hardware(
                stencilcraft_engine::HardwareCommand::Devices,
                ui.ctx().clone(),
            );
        }
        let mut device_select =
            Select::new("usb-device", "Connected cutter", &mut self.send.device);
        for (location, _, name) in &self.send.devices {
            device_select = device_select.option(location.clone(), format!("{name} ({location})"));
        }
        let _ = device_select.show(ui);
        if let Some((_, model, _)) = self.send.devices.iter().find(|d| d.0 == self.send.device)
            && let Some(model) = stencilcraft_machine::Model::by_id(model)
        {
            self.send.model = model;
        }

        ui.add(Text::new("Model"));
        if self.send.device.is_empty() {
            let mut model_select = Select::new("send-model", "Model", &mut self.send.model);
            for (model, label) in MODEL_CHOICES {
                model_select = model_select.option(*model, *label);
            }
            let _ = model_select.show(ui);
        } else {
            ui.add(Text::new(model_label(self.send.model)));
        }
        let caps = stencilcraft_machine::DeviceRegistry::builtin().by_id(self.send.model.id());
        if !mat_supported(caps, self.mat) {
            self.mat = stencilcraft_machine::Mat::None;
            self.send.summary = Some(vec![
                "The previous mat is unavailable for this model. Select your loaded mat.".into(),
            ]);
        }
        let mut mat_select = Select::new("send-mat", "Loaded mat", &mut self.mat);
        for (mat, label) in MAT_CHOICES {
            if mat_supported(caps, *mat) {
                mat_select = mat_select.option(*mat, *label);
            }
        }
        let _ = mat_select.show(ui);
        ui.add(Text::new("Material"));
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
        ui.add(Text::new(format!(
            "Pressure  {}",
            self.send.pressure_override
        )));
        let mut pressure = f64::from(self.send.pressure_override);
        let edit = Slider::new("send-pressure", "Pressure", pressure, 1.0..=33.0)
            .step(1.0)
            .show(ui);
        if apply_edit(&mut pressure, &mut self.pressure_before, edit.event) {
            self.send.pressure_override = pressure as u8;
        }
        ui.add(Text::new(format!("Speed  {}", self.send.speed_override)));
        let mut speed = f64::from(self.send.speed_override);
        let edit = Slider::new("send-speed", "Speed", speed, 1.0..=30.0)
            .step(1.0)
            .show(ui);
        if apply_edit(&mut speed, &mut self.speed_before, edit.event) {
            self.send.speed_override = speed as u8;
        }
        let _ = Checkbox::bool("Bounding box preview", &mut self.send.bbox_only).show(ui);
        Divider::horizontal().show(ui);
        if !self.send.device.is_empty() && !self.send.bbox_only {
            ui.add(Text::new("Load the mat and set blade depth on the cutter. Send cuts all visible paths with holder 1."));
            for (label, send) in [
                ("Validate USB cut", false),
                ("Send cut to selected USB cutter", true),
            ] {
                if ui.add(Button::new(label)).clicked() {
                    self.start_hardware(
                        stencilcraft_engine::HardwareCommand::Cut {
                            model: self.send.model.id().into(),
                            location: self.send.device.clone(),
                            mat: self.mat,
                            force: u32::from(self.send.pressure_override),
                            speed: u32::from(self.send.speed_override),
                            send,
                        },
                        ui.ctx().clone(),
                    );
                }
            }
        }

        if ui.add(Button::new("Encode dry run")).clicked() {
            self.send.summary = Some(self.encode_send_summary());
        }
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
        let extent_width = backdrop.width_mm.max(doc.board.width_mm);
        let extent_height = backdrop.height_mm.max(doc.board.height_mm);
        if !self.view_initialized {
            self.view
                .fit_extent(extent_width, extent_height, clip, 24.0);
            self.view_initialized = true;
        }
        if let Some(command) = view_command {
            let selection = stencilcraft_ui_egui::selected_world_bounds(
                &self.engine.doc,
                &self.selected_shapes,
            );
            apply_canvas_view_command(
                &mut self.view,
                command,
                extent_width,
                extent_height,
                selection,
                clip,
            );
        }
        if let Some(zoom) = self.pending_zoom.take() {
            self.view.zoom_at(zoom / self.view.zoom, clip.center());
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
        // The Offset panel's live preview, painted only while that panel is on screen.
        if std::mem::take(&mut self.panels.offset_preview_shown)
            && let Some(Ok(paths)) = self.panels.offset_preview.as_ref().map(|p| &p.result)
        {
            let color = stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line;
            stencilcraft_ui_egui::paint_preview_paths(&painter, &view, paths, color);
        }

        // Scroll bars over the mat/board extent (plus a margin): drag one to bring the mat
        // back after scrolling or zooming it out of view.
        let mut scroll_view = self.view.to_canvas_view(clip);
        let bars = craft_ui::canvas::CanvasScrollbars::new(
            "stencil-scrollbars",
            craft_ui::canvas::ScrollbarNames::new("Horizontal scroll", "Vertical scroll"),
        )
        .overscroll(true)
        .show(
            ui,
            &mut scroll_view,
            stencilcraft_ui_egui::scroll_extent(extent_width, extent_height),
        );
        if bars.changed {
            self.view.from_canvas_view(&scroll_view);
            ui.ctx().request_repaint();
        }
        let canvas_view = self.view.to_canvas_view(clip);
        let mut guides_claimed = bars.over;
        let mm_units = RulerUnits {
            per_doc: 1.0,
            ticks: craft_ui::canvas::Ticks::Decimal { whole: false },
            numbers: self.locale_numbers.clone(),
            unit: Unit::new("{n} mm"),
            decimals: 1,
        };

        let guides_id = if self.prefs.show_guides {
            let shown = stencilcraft_ui_egui::canvas_guides(&doc.guides, self.guide_preview);
            let guides = craft_ui::canvas::Guides::new(
                "stencil-guides",
                &shown,
                &mm_units,
                &mm_units,
                craft_ui::canvas::GuideNames::new("Guides", "Vertical guide", "Horizontal guide"),
            )
            .zero(self.ruler_origin)
            .show(ui, &response, &canvas_view);
            self.apply_guide_event(guides.event);
            guides_claimed = guides.claimed;
            Some(guides.id)
        } else {
            None
        };

        if self.prefs.show_rulers {
            let r = Rulers::new(
                "stencil-rulers",
                &mm_units,
                &mm_units,
                RulerNames::new("Horizontal ruler", "Vertical ruler", "Ruler origin"),
            )
            .zero(self.ruler_origin)
            .show(ui, full_rect, &canvas_view, guides_id);
            if r.reset_origin {
                self.ruler_origin = stencilcraft_geom::Point::ZERO;
            }
            if let Some(EditEvent::Commit { value, .. } | EditEvent::Set { value, .. }) = r.origin {
                self.ruler_origin = value;
            }
        }

        let mut overlay_claimed = guides_claimed;

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
            if let (Some(start), Some(end)) = (self.shape_drag_start, self.shape_drag_current)
                && let Some(command) = self.dragged_shape_command(start, end)
            {
                let mut preview =
                    stencilcraft_engine::Engine::new(stencilcraft_doc::Document::default());
                if preview.run(command).is_ok() {
                    let paths: Vec<_> = preview
                        .doc
                        .layers
                        .iter()
                        .flat_map(|layer| layer.shapes.iter().map(|shape| shape.path.clone()))
                        .collect();
                    stencilcraft_ui_egui::paint_preview_paths(
                        &painter,
                        &view,
                        &paths,
                        stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line,
                    );
                }
            }
            if response.drag_stopped()
                && let (Some(start), Some(end)) =
                    (self.shape_drag_start.take(), self.shape_drag_current.take())
            {
                self.commit_dragged_shape(start, end);
                ui.ctx().request_repaint();
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
            // Silhouette's point editing: double-click a shape to edit its points, double-click
            // empty canvas to leave.
            if response.double_clicked()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let hit = stencilcraft_ui_egui::hit_test_shape(&doc, &view, pointer, 8.0);
                match (self.active_tool, hit) {
                    (ActiveTool::Select, Some(id)) => {
                        self.set_selection([id]);
                        self.path_selection = PathSelection::default();
                        self.active_tool = ActiveTool::DirectSelect;
                        self.status = "editing points: double-click empty canvas to finish".into();
                    }
                    (ActiveTool::DirectSelect, None) => {
                        self.path_selection = PathSelection::default();
                        self.active_tool = ActiveTool::Select;
                    }
                    _ => {}
                }
            }

            if response.drag_started() {
                self.drag_origin = self.canvas_drag_shape.and_then(|_| {
                    stencilcraft_ui_egui::selected_world_bounds(
                        &self.engine.doc,
                        &self.selected_shapes,
                    )
                });
                self.snap_lines.clear();
            }
            if response.dragged() && !self.canvas_panning {
                if let Some(id) = self.canvas_drag_shape {
                    let total = response.total_drag_delta().unwrap_or_default();
                    self.drag_selection_to(id, total);
                } else if self.canvas_selection_start.is_none() {
                    self.view.pan_mm -= response.drag_delta() / self.view.zoom;
                }
                // Live position callout while a selection is dragged.
                if self.canvas_drag_shape.is_some()
                    && let Some(b) = stencilcraft_ui_egui::selected_world_bounds(
                        &self.engine.doc,
                        &self.selected_shapes,
                    )
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    Readout::new("move-readout")
                        .row("X", format!("{:.1} mm", b.x0))
                        .row("Y", format!("{:.1} mm", b.y0))
                        .row("W", format!("{:.1} mm", b.width()))
                        .row("H", format!("{:.1} mm", b.height()))
                        .show(ui.ctx(), pointer);
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
                self.drag_origin = None;
                self.snap_lines.clear();
            }
        }
        if self.prefs.smart_guides && !self.snap_lines.is_empty() {
            let color = craft_ui::canvas::Guides::style(&tokens).smart;
            stencilcraft_ui_egui::paint_snap_lines(&painter, &self.view, &self.snap_lines, color);
        }

        // Zoom: scroll wheel around the pointer.
        if response.hovered() {
            let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let pointer = response.hover_pos().unwrap_or(clip.center());
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
        let mut zoom_value = f64::from((self.view.zoom * 100.0).round().clamp(5.0, 40000.0));

        egui::Panel::bottom("status")
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show(parent, |ui| {
                StatusBar::new("stencilcraft-status").show(ui, |bar| {
                    bar.leading(|ui| {
                        ui.horizontal_centered(|ui| {
                            if ui
                                .add(Button::new("Fit").size(ControlSize::Sm))
                                .on_hover_text("Fit the mat and board (F)")
                                .clicked()
                            {
                                self.pending_view_command = Some(CanvasViewCommand::Fit);
                            }
                            if ui
                                .add(Button::new("Center").size(ControlSize::Sm))
                                .on_hover_text("Center the mat and board (C)")
                                .clicked()
                            {
                                self.pending_view_command = Some(CanvasViewCommand::Center);
                            }
                            if ui.add(IconButton::new(Icon::Minus, "Zoom out")).clicked() {
                                self.pending_zoom = Some((self.view.zoom / 1.2).clamp(0.05, 400.0));
                            }
                            let edit =
                                Slider::new("canvas-zoom", "Zoom", zoom_value, 5.0..=40000.0)
                                    .scale(SliderScale::Log)
                                    .step(1.0)
                                    .width(100.0)
                                    .show(ui);
                            if apply_edit(&mut zoom_value, &mut self.zoom_before, edit.event) {
                                self.pending_zoom =
                                    Some((zoom_value as f32 / 100.0).clamp(0.05, 400.0));
                            }
                            ui.add(Text::new(format!("{zoom_value:.0}%")));
                            if ui.add(IconButton::new(Icon::Plus, "Zoom in")).clicked() {
                                self.pending_zoom = Some((self.view.zoom * 1.2).clamp(0.05, 400.0));
                            }
                        });
                        StatusBar::info(ui, &info);
                        StatusBar::message(ui, &status_msg, false);
                    });
                    bar.trailing(|ui| {
                        ui.add(Text::new(format!("Tool: {}", self.active_tool.label())))
                            .on_hover_text("Space+drag or middle-drag to pan · wheel to zoom");
                    });
                });
            });
    }
}

/// The snapping switches the View menu flips (`snap.set` fields).
#[derive(Default)]
struct SnapToggles {
    grid: Option<bool>,
    objects: Option<bool>,
    spacing: Option<bool>,
    points: Option<bool>,
}

/// Apply a view command: `width_mm` × `height_mm` is the extent Fit and Center frame,
/// `selection` the selection's bounds for Zoom to Selection.
pub fn apply_canvas_view_command(
    view: &mut stencilcraft_ui_egui::View,
    command: CanvasViewCommand,
    width_mm: f64,
    height_mm: f64,
    selection: Option<stencilcraft_geom::Rect>,
    viewport: egui::Rect,
) {
    match command {
        CanvasViewCommand::Fit => {
            view.fit_extent(width_mm, height_mm, viewport, 24.0);
        }
        CanvasViewCommand::Center => {
            view.center_extent(width_mm, height_mm, viewport);
        }
        CanvasViewCommand::Selection => {
            if let Some(rect) = selection {
                view.fit_rect(rect, viewport, 24.0);
            }
        }
        CanvasViewCommand::ZoomPercent(percent) => {
            let zoom = f32::from(percent) / 100.0;
            view.zoom_at(zoom / view.zoom, viewport.center());
        }
        CanvasViewCommand::Rotate(turns) => view.set_rotation(turns, viewport),
    }
}

/// A button of the Tools panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolsAction {
    Deselect,
    Duplicate,
    Delete,
    Rectangle,
    Image,
    Text,
}

/// A quick transform button in the inspector's Transform section.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ShapeAction {
    Rotate(f64),
    Scale(f64),
}

/// The engine command an [`AlignBar`] pick runs.
#[derive(Clone, Copy, Debug, PartialEq)]
enum AlignCommand {
    Align(Alignment),
    Distribute(DistributionAxis),
}

/// The engine command behind `action`; `None` for the edge-distribute variants the engine does not
/// have (the bar shows them disabled).
fn align_command(action: AlignAction) -> Option<AlignCommand> {
    Some(match action {
        AlignAction::AlignLeftEdges => AlignCommand::Align(Alignment::Left),
        AlignAction::AlignHorizontalCenters => AlignCommand::Align(Alignment::HorizontalCenter),
        AlignAction::AlignRightEdges => AlignCommand::Align(Alignment::Right),
        AlignAction::AlignTopEdges => AlignCommand::Align(Alignment::Top),
        AlignAction::AlignVerticalCenters => AlignCommand::Align(Alignment::VerticalCenter),
        AlignAction::AlignBottomEdges => AlignCommand::Align(Alignment::Bottom),
        AlignAction::DistributeHorizontally => {
            AlignCommand::Distribute(DistributionAxis::Horizontal)
        }
        AlignAction::DistributeVertically => AlignCommand::Distribute(DistributionAxis::Vertical),
        _ => return None,
    })
}

/// The output path of an `app.screenshot` control request (`{"method":"app.screenshot",
/// "params":{"path":…}}`), the one control method the window answers itself: the MCP session
/// is headless and has no frame to capture.
fn screenshot_request(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("method")?.as_str()? != "app.screenshot" {
        return None;
    }
    Some(value.get("params")?.get("path")?.as_str()?.to_string())
}

/// The method and path of a `doc.open` or `doc.save` request (legacy envelope or the MCP
/// `doc_open`/`doc_save` tools) that `reply` answered successfully.
fn document_path_request(line: &str, reply: &str) -> Option<(&'static str, String)> {
    let request: serde_json::Value = serde_json::from_str(line).ok()?;
    let reply: serde_json::Value = serde_json::from_str(reply).ok()?;
    let (method, params) = match request.get("method")?.as_str()? {
        "doc.open" => ("doc.open", request.get("params")?),
        "doc.save" => ("doc.save", request.get("params")?),
        "tools/call" => {
            let params = request.get("params")?;
            let method = match params.get("name")?.as_str()? {
                "doc_open" => "doc.open",
                "doc_save" => "doc.save",
                _ => return None,
            };
            (method, params.get("arguments")?)
        }
        _ => return None,
    };
    let failed = reply.get("ok") == Some(&serde_json::Value::Bool(false))
        || reply.get("error").is_some()
        || reply.pointer("/result/isError") == Some(&serde_json::Value::Bool(true));
    if failed {
        return None;
    }
    Some((method, params.get("path")?.as_str()?.to_string()))
}

/// The request id and shape ids of an `app.select` control request
/// (`{"method":"app.select","params":{"ids":[1,2]}}`); at most 10 000 ids are read.
fn select_request(line: &str) -> Option<(serde_json::Value, Vec<u64>)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("method")?.as_str()? != "app.select" {
        return None;
    }
    let ids = value
        .get("params")?
        .get("ids")?
        .as_array()?
        .iter()
        .take(10_000)
        .filter_map(serde_json::Value::as_u64)
        .collect();
    Some((value.get("id").cloned().unwrap_or_default(), ids))
}

/// The request id, kerf-preview switch and colour theme of an `app.view` control request
/// (`{"method":"app.view","params":{"kerf_preview":true,"theme":"light"}}`); each setting is
/// optional and an unrecognised theme name leaves the theme alone.
fn view_request(
    line: &str,
) -> Option<(
    serde_json::Value,
    Option<bool>,
    Option<stencilcraft_ui_egui::ColorTheme>,
)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("method")?.as_str()? != "app.view" {
        return None;
    }
    let params = value.get("params");
    let kerf_preview = params
        .and_then(|p| p.get("kerf_preview"))
        .and_then(serde_json::Value::as_bool);
    let theme = match params.and_then(|p| p.get("theme")).and_then(|t| t.as_str()) {
        Some("dark") => Some(stencilcraft_ui_egui::ColorTheme::Dark),
        Some("light") => Some(stencilcraft_ui_egui::ColorTheme::Light),
        _ => None,
    };
    Some((
        value.get("id").cloned().unwrap_or_default(),
        kerf_preview,
        theme,
    ))
}

/// Capture the next painted frame to a PNG at `path`, then answer `job`.
fn request_screenshot(ctx: &egui::Context, path: String, job: control::Job) {
    let id = serde_json::from_str::<serde_json::Value>(&job.line)
        .ok()
        .and_then(|v| v.get("id").cloned())
        .unwrap_or(serde_json::Value::Null);
    let callback = egui::ScreenshotCallback::new(move |image| {
        let [w, h] = image.size;
        let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
        let saved = u32::try_from(w)
            .ok()
            .zip(u32::try_from(h).ok())
            .and_then(|(w, h)| image::RgbaImage::from_raw(w, h, bytes))
            .ok_or_else(|| "screenshot: malformed image".to_string())
            .and_then(|img| {
                img.save(&path)
                    .map_err(|e| format!("screenshot: {path}: {e}"))
            });
        let reply = match saved {
            Ok(()) => {
                serde_json::json!({"id": id, "ok": true, "result": {"path": path, "size": [w, h]}})
            }
            Err(e) => serde_json::json!({"id": id, "ok": false, "error": e}),
        };
        let _ = job.reply.send(reply.to_string());
    });
    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(callback));
    ctx.request_repaint();
}

impl eframe::App for StencilApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "recent_files", &self.recent_files);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.installed_theme.as_ref().map(|(t, _)| *t) != Some(self.theme) {
            self.installed_theme = Some((self.theme, stencilcraft_ui_egui::theme_base(self.theme)));
        }
        if let Some((_, base)) = &self.installed_theme {
            craft_ui::theme::install(ui.ctx(), base);
        }
        self.drain_control_requests(ui.ctx());
        self.handle_keyboard_shortcuts(ui.ctx());
        if std::mem::take(&mut self.quit_requested) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }

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
            if tool == ActiveTool::Text {
                self.open_text_dialog(false);
            }
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
        content.app.new_document_dialog(ui);
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_ui::docking::{Action, Placement};

    #[test]
    fn mats_follow_machine_capabilities() {
        use stencilcraft_machine::{DeviceRegistry, Mat};
        let registry = DeviceRegistry::builtin();
        let cameo = registry.by_id("cameo-4");
        let pro = registry.by_id("cameo-4-pro");
        assert!(mat_supported(cameo, Mat::Cameo12x12));
        assert!(!mat_supported(cameo, Mat::CameoPro24x24));
        assert!(mat_supported(pro, Mat::CameoPro24x24));
        assert!(mat_supported(cameo, Mat::None));
        assert!(!mat_supported(None, Mat::Cameo12x12));
    }

    #[test]
    fn window_control_methods_parse_and_ignore_everything_else() {
        assert_eq!(
            screenshot_request(
                r#"{"id":1,"method":"app.screenshot","params":{"path":"/tmp/a.png"}}"#
            ),
            Some("/tmp/a.png".to_string())
        );
        assert_eq!(
            screenshot_request(r#"{"id":1,"method":"doc.summary"}"#),
            None
        );
        assert_eq!(screenshot_request("not json"), None);
        let (id, ids) =
            select_request(r#"{"id":7,"method":"app.select","params":{"ids":[1,"x",3]}}"#).unwrap();
        assert_eq!((id, ids), (serde_json::json!(7), vec![1, 3]));
        assert_eq!(
            select_request(r#"{"method":"app.select","params":{}}"#),
            None
        );
        let (id, kerf, theme) = view_request(
            r#"{"id":2,"method":"app.view","params":{"kerf_preview":true,"theme":"light"}}"#,
        )
        .unwrap();
        assert_eq!((id, kerf), (serde_json::json!(2), Some(true)));
        assert!(theme == Some(stencilcraft_ui_egui::ColorTheme::Light));
        let (_, kerf, theme) =
            view_request(r#"{"method":"app.view","params":{"theme":"pink"}}"#).unwrap();
        assert!(kerf.is_none() && theme.is_none());
        assert!(view_request(r#"{"method":"app.select","params":{}}"#).is_none());
        let open = r#"{"id":1,"method":"doc.open","params":{"path":"a.stencilcraft"}}"#;
        assert_eq!(
            document_path_request(open, r#"{"id":1,"ok":true,"result":{}}"#),
            Some(("doc.open", "a.stencilcraft".to_string()))
        );
        assert_eq!(
            document_path_request(open, r#"{"id":1,"ok":false,"error":"x"}"#),
            None
        );
        let tool = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"doc_save","arguments":{"path":"b.stencilcraft"}}}"#;
        assert_eq!(
            document_path_request(
                tool,
                r#"{"jsonrpc":"2.0","id":2,"result":{"isError":false}}"#
            ),
            Some(("doc.save", "b.stencilcraft".to_string()))
        );
        assert_eq!(
            document_path_request(
                tool,
                r#"{"jsonrpc":"2.0","id":2,"result":{"isError":true}}"#
            ),
            None
        );
    }

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
    fn flip_rotate_again_and_key_align_menu_actions() {
        let mut app = StencilApp::new(None);
        for x in [0.0, 30.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 0.0,
                width: 10.0,
                height: 20.0,
            });
        }
        let bounds = |app: &StencilApp, id| app.engine.doc.get_shape(id).unwrap().bounds().unwrap();
        app.set_selection([1]);
        let before = bounds(&app, 1);
        app.handle_menu_action(TopMenuAction::Flip(DistributionAxis::Horizontal));
        assert_eq!(bounds(&app, 1), before, "flipped in place");
        app.handle_menu_action(TopMenuAction::RotateBy(90));
        app.handle_menu_action(TopMenuAction::TransformAgain);
        // Two quarter turns: back to 10 wide, 20 tall.
        assert!((bounds(&app, 1).width() - 10.0).abs() < 1e-9);
        // Key = last selected (shape 1) stays; shape 2 moves to its left edge.
        app.set_selection([2, 1]);
        let key = bounds(&app, 1);
        app.handle_menu_action(TopMenuAction::AlignToKey(Alignment::Left));
        assert_eq!(bounds(&app, 1), key);
        assert!((bounds(&app, 2).x0 - key.x0).abs() < 1e-9);
    }

    #[test]
    fn shifted_shortcuts_are_not_taken_by_their_unshifted_twins() {
        let ctx = egui::Context::default();
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 3.0,
            y: 4.0,
            width: 12.0,
            height: 8.0,
        });
        assert!(app.engine.undo());
        let mut press = |key, modifiers| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ui| app.handle_keyboard_shortcuts(ui.ctx()),
            );
            output.textures_delta.clear();
        };
        let cmd_shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        press(egui::Key::Z, cmd_shift);
        press(egui::Key::A, egui::Modifiers::COMMAND);
        press(egui::Key::A, cmd_shift);
        assert_eq!(app.engine.doc.shape_count(), 1, "Cmd+Shift+Z redoes");
        assert!(app.selected_shapes.is_empty(), "Cmd+Shift+A deselects");
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
                ..Default::default()
            };
            let viewport =
                egui::Rect::from_min_size(egui::pos2(100.0, 60.0), egui::vec2(800.0, 500.0));
            apply_canvas_view_command(&mut view, expected, 600.0, 300.0, None, viewport);
            let center = view.world_to_screen(stencilcraft_geom::Point::new(300.0, 150.0));
            assert!((center - viewport.center()).length() < 1e-4);
            match expected {
                CanvasViewCommand::Fit => assert!(view.zoom < 2.5),
                _ => assert_eq!(view.zoom, 2.5),
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
    fn wheel_zoom_keeps_the_point_under_the_pointer() {
        let ctx = egui::Context::default();
        let mut app = StencilApp::new(None);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
        let pointer = egui::pos2(300.0, 250.0);
        let frame = |app: &mut StencilApp, events: Vec<egui::Event>, t: f64| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(t),
                    events,
                    ..Default::default()
                },
                |ui| app.canvas_body(ui),
            );
            output.textures_delta.clear();
        };
        for i in 0..3 {
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(pointer)],
                f64::from(i) * 0.016,
            );
        }
        let before = app.view.screen_to_world(pointer.to_vec2());
        let zoom_before = app.view.zoom;
        let wheel = egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 120.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
        };
        frame(&mut app, vec![wheel], 0.064);
        for i in 0..40 {
            frame(&mut app, vec![], 0.08 + f64::from(i) * 0.016);
        }
        let after = app.view.screen_to_world(pointer.to_vec2());
        assert!(
            app.view.zoom > zoom_before * 1.05,
            "zoomed in: {zoom_before} -> {}",
            app.view.zoom
        );
        assert!(
            (after.x - before.x).abs() < 0.05 && (after.y - before.y).abs() < 0.05,
            "point under the pointer moved: {before:?} -> {after:?}"
        );
    }

    #[test]
    fn zoom_buttons_zoom_about_the_canvas_centre() {
        let ctx = egui::Context::default();
        let mut app = StencilApp::new(None);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
        let frame = |app: &mut StencilApp| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| app.canvas_body(ui),
            );
            output.textures_delta.clear();
        };
        frame(&mut app);
        let centre = |app: &StencilApp| app.view.screen_to_world(screen.center().to_vec2());
        let start = centre(&app);
        let start_zoom = app.view.zoom;
        // The anchor is the canvas centre (inside the rulers), a few px from the screen
        // centre, so the world point there may move by those px at the new zoom only.
        for factor in [1.2_f32; 5].into_iter().chain([1.0 / 1.2; 5]) {
            app.pending_zoom = Some(app.view.zoom * factor);
            frame(&mut app);
            let at = centre(&app);
            let slack = 30.0 / f64::from(app.view.zoom);
            assert!(
                (at.x - start.x).abs() < slack && (at.y - start.y).abs() < slack,
                "view centre drifted: {start:?} -> {at:?} at zoom {}",
                app.view.zoom
            );
        }
        let end = centre(&app);
        assert!((app.view.zoom - start_zoom).abs() < 1e-3);
        assert!((end.x - start.x).abs() < 0.01 && (end.y - start.y).abs() < 0.01);
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
    fn new_document_waits_for_confirmation_when_document_changed() {
        let mut app = StencilApp::new(None);
        app.run_command(Command::KerfSet { width_mm: 0.4 });
        let changed = app.engine.doc.clone();
        let choice = new_doc::Choice {
            width_mm: 300.0,
            ..Default::default()
        };
        app.request_new_document(choice.clone());
        assert_eq!(app.engine.doc, changed);
        assert!(app.pending_new_document.is_some());
        app.pending_new_document = None; // Cancel preserves the original document.
        assert_eq!(app.engine.doc, changed);
        app.request_new_document(choice);
        app.confirm_new_document();
        assert_eq!(app.engine.doc.board.width_mm, 300.0);
        assert_eq!(app.engine.doc, app.saved_document);
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
        for (key, send_mode) in [
            (egui::Key::Delete, false),
            (egui::Key::Backspace, false),
            (egui::Key::Delete, true),
            (egui::Key::Backspace, true),
        ] {
            let ctx = egui::Context::default();
            let mut app = StencilApp::new(None);
            app.run_command(Command::ShapeAddRect {
                x: 3.0,
                y: 4.0,
                width: 12.0,
                height: 8.0,
            });
            app.set_selection([1]);
            app.send_mode = send_mode;
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

    #[test]
    fn delete_preserves_text_input_but_works_with_button_focus() {
        for text_focus in [true, false] {
            let ctx = egui::Context::default();
            let mut app = StencilApp::new(None);
            app.run_command(Command::ShapeAddRect {
                x: 3.0,
                y: 4.0,
                width: 12.0,
                height: 8.0,
            });
            app.set_selection([1]);
            let mut text = String::from("abc");
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                if text_focus {
                    ui.text_edit_singleline(&mut text).request_focus();
                } else {
                    ui.button("Control").request_focus();
                }
            });
            output.textures_delta.clear();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: egui::Key::Delete,
                        physical_key: Some(egui::Key::Delete),
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| {
                    app.handle_keyboard_shortcuts(ui.ctx());
                    if text_focus {
                        ui.text_edit_singleline(&mut text);
                    } else {
                        let _ = ui.button("Control");
                    }
                },
            );
            output.textures_delta.clear();
            assert_eq!(app.engine.doc.shape_count(), usize::from(text_focus));
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
        // A control named by a visible word label (`PropertyRow::word`) has no label of its own:
        // it is `labelled_by` the label's node.
        let named = |node: &egui::accesskit::Node| {
            node.label() == Some(label) || node.value() == Some(label)
        };
        let label_ids: Vec<_> = tree
            .nodes
            .iter()
            .filter(|(_, node)| named(node))
            .map(|(id, _)| *id)
            .collect();
        let node = tree
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .or_else(|| {
                tree.nodes
                    .iter()
                    .find(|(_, node)| node.labelled_by().iter().any(|id| label_ids.contains(id)))
            })
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
    fn desktop_text_can_be_edited_undone_and_saved() {
        let mut app = StencilApp::new(None);
        app.open_text_dialog(false);
        assert!(
            app.text_fonts
                .iter()
                .any(|font| font.family == stencilcraft_engine::text::SINGLE_LINE_FAMILY)
        );
        app.text_value = "Hello".to_string();
        app.text_size = 24.0;
        app.add_text();
        let id = app.selected_shape.expect("new text selected");
        let shape = app.engine.doc.get_shape(id).expect("text shape");
        let spec = shape.text.as_deref().expect("live text");
        assert_eq!(spec.text, "Hello");
        assert_eq!(spec.size, 24.0);
        assert_eq!(shape.style, CutStyle::Engrave);
        assert!(!shape.path.elements().is_empty());

        app.open_text_dialog(true);
        app.text_value = "Updated".to_string();
        app.text_size = 48.0;
        app.add_text();
        let spec = app
            .engine
            .doc
            .get_shape(id)
            .unwrap()
            .text
            .as_deref()
            .unwrap();
        assert_eq!((&*spec.text, spec.size), ("Updated", 48.0));
        let saved = app.engine.doc.save_json().unwrap();
        let loaded = stencilcraft_doc::Document::load_json(&saved).unwrap();
        assert_eq!(
            loaded.get_shape(id).unwrap().text.as_deref().unwrap().text,
            "Updated"
        );
        app.engine.run(Command::Undo).unwrap();
        assert_eq!(
            app.engine
                .doc
                .get_shape(id)
                .unwrap()
                .text
                .as_deref()
                .unwrap()
                .text,
            "Hello"
        );
    }

    #[test]
    fn edit_text_requires_live_text_and_invalid_font_is_reported() {
        let mut app = StencilApp::new(None);
        app.open_text_dialog(true);
        assert!(!app.text_dialog);
        assert!(app.status.contains("select a live text"));
        app.text_value = "Hello".to_string();
        app.text_family = "StencilCraft font that does not exist".to_string();
        app.add_text();
        assert!(app.selected_shape.is_none());
        assert!(app.status.contains("not installed"));
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
        frame(&ctx, &mut app, vec![], StencilApp::canvas_body);
        let old_zoom = app.view.zoom;
        let out = frame(&ctx, &mut app, vec![], StencilApp::status_bar);
        click(
            &ctx,
            &mut app,
            StencilApp::status_bar,
            node_center(&out, "Zoom"),
        );
        assert!(app.pending_zoom.is_some());
        frame(&ctx, &mut app, vec![], StencilApp::canvas_body);
        assert_ne!(app.view.zoom, old_zoom);
        assert!(app.pending_zoom.is_none());
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
                .find(|(_, node)| node.label() == Some("Rectangle"))
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
                .find(|(_, node)| node.label() == Some("Duplicate"))
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
