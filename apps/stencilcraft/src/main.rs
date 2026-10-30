//! The StencilCraft desktop app: window bootstrap, file dialogs and menus. All
//! behaviour routes through `stencilcraft_engine::Command`; painting is
//! `stencilcraft_ui_egui`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use craft_ui::ComboBox as CraftComboBox;
use craft_ui::docking::{
    Column, DockArea, DockContent, DockStyle, Group, Layout, Node, Permissions, Side,
};
use craft_ui::layout::{SplitAxis, SplitSize};
use eframe::egui;
use stencilcraft_engine::Command;

/// CLI/agent control channel: JSON lines on a TCP socket, one session per
/// connection (the same session core the CLI's `mcp` mode uses).
mod control;

/// The same bounded PNG/JPEG silhouette tracer used by the CLI.
#[path = "../../stencilcraft-cli/src/trace.rs"]
mod trace;

/// Display label for a media preset index (parallel to `MEDIA_PRESETS`).
fn media_label(index: usize) -> String {
    match stencilcraft_machine::MEDIA_PRESETS.get(index) {
        Some(m) => format!("{} (ID {})", m.name, m.id),
        None => "Custom (ID 300)".to_string(),
    }
}

/// Mat chooser entries: (device mat, label).
const MAT_CHOICES: &[(stencilcraft_machine::Mat, &str)] = &[
    (stencilcraft_machine::Mat::None, "no mat"),
    (stencilcraft_machine::Mat::Cameo12x12, "12×12"),
    (stencilcraft_machine::Mat::Cameo12x24, "12×24"),
    (stencilcraft_machine::Mat::Portrait8x12, "8×12 portrait"),
    (stencilcraft_machine::Mat::CameoPlus15x15, "15×15 plus"),
    (stencilcraft_machine::Mat::CameoPro24x24, "24×24 Pro"),
    (stencilcraft_machine::Mat::Portrait8_5x12, "8.5×12 portrait"),
];

fn mat_label(mat: stencilcraft_machine::Mat) -> &'static str {
    MAT_CHOICES
        .iter()
        .find(|(m, _)| *m == mat)
        .map(|(_, label)| *label)
        .unwrap_or("no mat")
}

/// Machine chooser entries: (device model, label). Most specific first.
const MODEL_CHOICES: &[(stencilcraft_machine::Model, &str)] = &[
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

fn model_label(model: stencilcraft_machine::Model) -> &'static str {
    MODEL_CHOICES
        .iter()
        .find(|(m, _)| *m == model)
        .map(|(_, label)| *label)
        .unwrap_or("Cameo 4 Pro")
}

/// The canvas backdrop for the currently selected mat.
fn mat_backdrop(mat: stencilcraft_machine::Mat) -> stencilcraft_ui_egui::MatBackdrop {
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
    if let Some(port) = control_port {
        control::serve(port);
    }

    let app = StencilApp::new(open_path.as_deref());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 720.0])
            .with_title("StencilCraft"),
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
enum WorkspacePanel {
    Canvas,
    Tools,
    Inspector,
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
        }
    }

    fn logical_label(&self, panel: &WorkspacePanel) -> String {
        match panel {
            WorkspacePanel::Canvas => "Canvas",
            WorkspacePanel::Tools => "Tools",
            WorkspacePanel::Inspector => "Design Inspector",
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

struct StencilApp {
    workspace: Layout<WorkspacePanel>,
    engine: stencilcraft_engine::Engine,
    view: stencilcraft_ui_egui::View,
    kerf_preview: bool,
    status: String,
    open_path: Option<String>,
    send_mode: bool,
    theme: stencilcraft_ui_egui::ColorTheme,
    view_initialized: bool,
    /// The cutting mat shown under the sheet and used for Send's encoded job.
    mat: stencilcraft_machine::Mat,
    selected_shape: Option<u64>,
    inspector_shape: Option<u64>,
    inspector_bounds: Option<[f64; 4]>,
    inspector_values: [String; 4],
    inspector_editing: bool,
    text_dialog: bool,
    text_value: String,
    canvas_drag_shape: Option<u64>,
    last_drag_delta: egui::Vec2,
    send: SendTab,
}

/// Send-tab UI state: media preset + model + per-job overrides. Encoding is
/// always a dry run here; real hardware sends stay with the CLI.
#[derive(Debug, Clone)]
struct SendTab {
    media_index: usize,
    model: stencilcraft_machine::Model,
    pressure_override: u8,
    speed_override: u8,
    bbox_only: bool,
    summary: Option<Vec<String>>,
}

impl Default for SendTab {
    fn default() -> Self {
        // Vinyl Sticker is the reference job for a vinyl cutter.
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

fn default_workspace() -> Layout<WorkspacePanel> {
    let dock = |side, panel, width| Node::Dock {
        side,
        columns: vec![Column::new(vec![Group::new(vec![panel], 0)], width)],
    };
    Layout {
        root: Some(Node::Split {
            axis: SplitAxis::Horizontal,
            size: SplitSize::Ratio(0.18),
            first: Box::new(dock(Side::Start, WorkspacePanel::Tools, 76.0)),
            second: Box::new(Node::Split {
                axis: SplitAxis::Horizontal,
                size: SplitSize::Ratio(0.78),
                first: Box::new(Node::Tabs {
                    panels: vec![WorkspacePanel::Canvas],
                    active: 0,
                }),
                second: Box::new(dock(Side::End, WorkspacePanel::Inspector, 280.0)),
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
    fn new(open_path: Option<&str>) -> Self {
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
        Self {
            workspace: default_workspace(),
            engine,
            view: stencilcraft_ui_egui::View::default(),
            kerf_preview: false,
            status,
            open_path,
            send_mode: false,
            theme: stencilcraft_ui_egui::ColorTheme::Dark,
            view_initialized: false,
            mat: stencilcraft_machine::Mat::CameoPro24x24,
            selected_shape: None,
            inspector_shape: None,
            inspector_bounds: None,
            inspector_values: std::array::from_fn(|_| String::new()),
            inspector_editing: false,
            text_dialog: false,
            text_value: String::new(),
            canvas_drag_shape: None,
            last_drag_delta: egui::Vec2::ZERO,
            send: SendTab::default(),
        }
    }

    fn with_egui(self, _cc: &eframe::CreationContext) -> Self {
        self
    }

    fn run_command(&mut self, command: Command) {
        match self.engine.run(command) {
            Ok(_effect) => self.status = "done".to_string(),
            Err(e) => self.status = e.to_string(),
        }
    }

    fn save_to(&mut self, path: String) {
        match stencilcraft_doc::save_to(&self.engine.doc, &path) {
            Ok(()) => {
                self.open_path = Some(path);
                self.status = "saved".to_string();
            }
            Err(e) => self.status = e.to_string(),
        }
    }

    fn select_latest_shape(&mut self) {
        self.selected_shape = self.engine.doc.next_shape_id.checked_sub(1);
    }

    fn add_imported_shapes(&mut self, imported: stencilcraft_doc::Document, label: &str) {
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

    fn import_svg(&mut self, path: std::path::PathBuf) {
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

    fn import_image(&mut self, path: std::path::PathBuf) {
        let name = path.to_string_lossy();
        let result = trace::trace_file(&name, trace::TraceOptions::default());
        match result {
            Ok(imported) => self.add_imported_shapes(imported, &name),
            Err(error) => self.status = error,
        }
    }

    fn add_text(&mut self) {
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
                self.selected_shape = Some(id);
            }
            Ok(_) => {}
            Err(error) => self.status = error.to_string(),
        }
    }

    fn text_dialog(&mut self, parent: &mut egui::Ui) {
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
                ui.text_edit_singleline(&mut self.text_value);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    if ui.button("Add text").clicked() {
                        add_text = true;
                    }
                });
            });
        self.text_dialog = open && !add_text && !cancel;
        if add_text {
            self.add_text();
        }
    }
}

impl eframe::App for StencilApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().set_visuals(match self.theme {
            stencilcraft_ui_egui::ColorTheme::Dark => egui::Visuals::dark(),
            stencilcraft_ui_egui::ColorTheme::Light => egui::Visuals::light(),
        });
        self.top_panel(ui);
        self.workflow_bar(ui);
        self.status_bar(ui);
        let style = DockStyle::from_ui(ui);
        let mut content = WorkspaceContent { app: self };
        let mut workspace = std::mem::take(&mut content.app.workspace);
        let output = DockArea::new(egui::Id::unique("stencilcraft-workspace")).show_content(
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
        content.app.text_dialog(ui);
        ui.ctx().request_repaint();
    }
}

impl StencilApp {
    fn top_panel(&mut self, parent: &mut egui::Ui) {
        egui::Panel::top("menu").show(parent, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("STENCILCRAFT")
                            .color(egui::Color32::WHITE)
                            .strong(),
                    )
                    .sense(egui::Sense::hover()),
                );
                ui.separator();
                let (theme_label, next_theme) = match self.theme {
                    stencilcraft_ui_egui::ColorTheme::Dark => {
                        ("☀ Light", stencilcraft_ui_egui::ColorTheme::Light)
                    }
                    stencilcraft_ui_egui::ColorTheme::Light => {
                        ("☾ Dark", stencilcraft_ui_egui::ColorTheme::Dark)
                    }
                };
                if ui.button(theme_label).clicked() {
                    self.theme = next_theme;
                }
                if ui.button("New").clicked() {
                    self.run_command(Command::DocumentNew {
                        width_mm: 100.0,
                        height_mm: 150.0,
                    });
                    self.open_path = None;
                }
                if ui.button("Open…").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("StencilCraft", &["stencilcraft"])
                        .pick_file()
                {
                    match stencilcraft_doc::load_from(path.to_string_lossy().as_ref()) {
                        Ok(doc) => {
                            self.engine = stencilcraft_engine::Engine::new(doc);
                            self.open_path = Some(path.to_string_lossy().into_owned());
                            self.status = "opened".to_string();
                        }
                        Err(e) => self.status = format!("open: {e}"),
                    }
                }
                if ui.button("Import SVG…").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("SVG", &["svg"])
                        .pick_file()
                {
                    self.import_svg(path);
                }
                if ui.button("Save…").clicked() {
                    let dialog = rfd::FileDialog::new();
                    let pick = match &self.open_path {
                        Some(p) => dialog
                            .add_filter("StencilCraft", &["stencilcraft"])
                            .set_file_name(p)
                            .save_file(),
                        None => dialog
                            .add_filter("StencilCraft", &["stencilcraft"])
                            .set_file_name("untitled.stencilcraft")
                            .save_file(),
                    };
                    if let Some(path) = pick {
                        self.save_to(path.to_string_lossy().into_owned());
                    }
                }
                ui.separator();
                if ui.button("Undo").clicked() {
                    let _ = self.engine.undo();
                }
                if ui.button("Redo").clicked() {
                    let _ = self.engine.redo();
                }
            });
        });
    }

    fn workflow_bar(&mut self, parent: &mut egui::Ui) {
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
                    if ui.button(label).clicked() {
                        self.send_mode = destination;
                    }
                    ui.selectable_value(&mut self.send_mode, false, "Canvas");
                });
            });
        });
    }

    fn tools_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.small("TOOLS");
            ui.separator();
            if ui.button("Deselect").clicked() {
                self.selected_shape = None;
            }
            if ui.button("＋ Rectangle").clicked() {
                self.run_command(Command::ShapeAddRect {
                    x: 20.0,
                    y: 20.0,
                    width: 40.0,
                    height: 30.0,
                });
                self.select_latest_shape();
            }
            if ui.button("▧ Image").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG or JPEG", &["png", "jpg", "jpeg"])
                    .pick_file()
            {
                self.import_image(path);
            }
            if ui.button("T Text").clicked() {
                self.text_dialog = true;
            }
            if ui.button("⊖ Delete").clicked()
                && let Some(id) = self.selected_shape
            {
                self.run_command(Command::ShapeRemove { id });
                self.selected_shape = None;
            }
            ui.separator();
            if ui.button("↶").clicked() {
                let _ = self.engine.undo();
            }
            if ui.button("↷").clicked() {
                let _ = self.engine.redo();
            }
        });
    }

    fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            if self.send_mode {
                ui.heading("Make");
                ui.small("Review your material and machine settings before cutting.");
                ui.separator();
                self.send_tab(ui);
                if ui.button("← Back to canvas").clicked() {
                    self.send_mode = false;
                }
            } else {
                self.inspector(ui);
            }
        });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Design");
        ui.small("Layers & objects");
        ui.add_space(8.0);

        ui.label("Cutting mat");
        CraftComboBox::from_id_salt("design-mat")
            .selected_text(mat_label(self.mat))
            .show_ui(ui, |ui| {
                for (mat, label) in MAT_CHOICES {
                    if ui.selectable_label(self.mat == *mat, *label).clicked() {
                        self.mat = *mat;
                    }
                }
            });

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
                let response = ui.checkbox(
                    &mut visible,
                    format!("{}  ·  {}", layer.name, layer.shapes.len()),
                );
                response.changed().then_some((i, visible))
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
                .selectable_label(self.selected_shape == Some(id), label)
                .clicked()
            {
                self.selected_shape = Some(id);
            }
        }

        ui.separator();
        ui.heading("Cut settings");
        let kerf_width = self.engine.doc.kerf.width_mm;
        ui.label(format!("Kerf  {:.3} mm", kerf_width));
        let mut kerf_widget = kerf_width;
        if ui
            .add(egui::Slider::new(&mut kerf_widget, 0.0..=1.0).suffix(" mm"))
            .changed()
        {
            self.run_command(Command::KerfSet {
                width_mm: kerf_widget,
            });
        }
        ui.checkbox(&mut self.kerf_preview, "Preview kerf compensation");

        if let Some(id) = self.selected_shape {
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
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut self.inspector_values[index])
                                        .desired_width(62.0)
                                        .id_salt(("bound", id, index)),
                                );
                                editing |= response.has_focus();
                                submitted |= response.lost_focus()
                                    && ui.input(|input| input.key_pressed(egui::Key::Enter));
                            }
                            ui.end_row();
                        }
                    });
                self.inspector_editing = editing;
                submitted |= ui.button("Apply").clicked();
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
                    if ui.button("−15°").clicked() {
                        self.run_command(Command::ShapeRotate { id, degrees: -15.0 });
                    }
                    if ui.button("+15°").clicked() {
                        self.run_command(Command::ShapeRotate { id, degrees: 15.0 });
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("−10%").clicked() {
                        self.run_command(Command::ShapeScale {
                            id,
                            scale_x: 0.9,
                            scale_y: 0.9,
                        });
                    }
                    if ui.button("+10%").clicked() {
                        self.run_command(Command::ShapeScale {
                            id,
                            scale_x: 1.1,
                            scale_y: 1.1,
                        });
                    }
                });
                ui.small("Drag an object on the mat to reposition it.");
            } else {
                self.selected_shape = None;
            }
        }
    }

    fn send_tab(&mut self, ui: &mut egui::Ui) {
        ui.label("Model");
        CraftComboBox::from_label("Model")
            .selected_text(model_label(self.send.model))
            .show_ui(ui, |ui| {
                for (model, label) in MODEL_CHOICES {
                    if ui
                        .selectable_label(self.send.model == *model, *label)
                        .clicked()
                    {
                        self.send.model = *model;
                    }
                }
            });
        ui.label("Material");
        CraftComboBox::from_label("Material")
            .selected_text(media_label(self.send.media_index))
            .show_ui(ui, |ui| {
                for (i, media) in stencilcraft_machine::MEDIA_PRESETS.iter().enumerate() {
                    if ui
                        .selectable_label(self.send.media_index == i, media_label(i))
                        .clicked()
                    {
                        self.send.media_index = i;
                        // Defaults follow the preset.
                        self.send.pressure_override = media.pressure.unwrap_or(10);
                        self.send.speed_override = media.speed.unwrap_or(5);
                    }
                }
            });
        ui.add_space(4.0);
        ui.add(egui::Slider::new(&mut self.send.pressure_override, 1..=33).text("Pressure"));
        ui.add(egui::Slider::new(&mut self.send.speed_override, 1..=30).text("Speed"));
        ui.checkbox(&mut self.send.bbox_only, "Bounding box preview");
        ui.separator();
        if ui.button("Encode dry run").clicked() {
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
    fn encode_send_summary(&self) -> Vec<String> {
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

    fn canvas_body(&mut self, ui: &mut egui::Ui) {
        let clip = ui.available_rect_before_wrap();
        let doc = self.engine.doc.clone();
        let backdrop = mat_backdrop(self.mat);
        if !self.view_initialized {
            let width_mm = backdrop.width_mm.max(doc.board.width_mm);
            let height_mm = backdrop.height_mm.max(doc.board.height_mm);
            let available = egui::vec2(
                (clip.width() - 48.0).max(1.0),
                (clip.height() - 48.0).max(1.0),
            );
            let zoom = (available.x / width_mm as f32)
                .min(available.y / height_mm as f32)
                .clamp(0.05, 4.0);
            self.view.zoom = zoom;
            self.view.pan_mm = egui::vec2(
                width_mm as f32 * 0.5 - clip.center().x / zoom,
                height_mm as f32 * 0.5 - clip.center().y / zoom,
            );
            self.view_initialized = true;
        }
        let response = ui.allocate_rect(clip, egui::Sense::click_and_drag());
        let view = self.view;
        let painter = ui.painter_at(clip);
        let selection = stencilcraft_ui_egui::SelectionState {
            selected_shapes: self.selected_shape.into_iter().collect(),
            overlay: None,
        };
        stencilcraft_ui_egui::paint_document(
            &painter,
            clip,
            &doc,
            &view,
            Some(&backdrop),
            stencilcraft_ui_egui::CanvasOptions {
                kerf_preview: self.kerf_preview,
                theme: self.theme,
            },
            &selection,
        );

        if response.drag_started() {
            self.canvas_drag_shape = response.interact_pointer_pos().and_then(|pointer| {
                stencilcraft_ui_egui::hit_test_shape(&doc, &view, pointer, 8.0)
            });
            if self.canvas_drag_shape.is_some() {
                self.selected_shape = self.canvas_drag_shape;
            } else {
                self.selected_shape = None;
            }
            self.last_drag_delta = egui::Vec2::ZERO;
        } else if response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            self.selected_shape = stencilcraft_ui_egui::hit_test_shape(&doc, &view, pointer, 8.0);
        }

        if response.dragged() {
            let total_delta = response.drag_delta();
            let delta = total_delta - self.last_drag_delta;
            self.last_drag_delta = total_delta;
            if delta != egui::Vec2::ZERO {
                if let Some(id) = self.canvas_drag_shape {
                    self.run_command(Command::ShapeMove {
                        id,
                        dx_mm: f64::from(delta.x / self.view.zoom),
                        dy_mm: f64::from(delta.y / self.view.zoom),
                    });
                } else {
                    self.view.pan_mm.x -= delta.x / self.view.zoom;
                    self.view.pan_mm.y -= delta.y / self.view.zoom;
                }
            }
        }
        if response.drag_stopped() {
            self.canvas_drag_shape = None;
            self.last_drag_delta = egui::Vec2::ZERO;
        }

        // Zoom: scroll wheel around the pointer.
        if response.hovered() {
            let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let pointer = response.interact_pointer_pos().unwrap_or(clip.center());
                self.view.zoom_at(1.0 + scroll / 600.0, pointer);
            }
        }
    }

    fn status_bar(&mut self, parent: &mut egui::Ui) {
        egui::Panel::bottom("status").show(parent, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{:.1} × {:.1} mm | kerf {:.3} mm | {} shapes",
                    self.engine.doc.board.width_mm,
                    self.engine.doc.board.height_mm,
                    self.engine.doc.kerf.width_mm,
                    self.engine.doc.shape_count()
                ));
                ui.separator();
                ui.label(&self.status);
            });
        });
    }
}
