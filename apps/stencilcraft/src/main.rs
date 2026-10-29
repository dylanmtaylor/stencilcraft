//! The StencilCraft desktop app: window bootstrap, file dialogs and menus. All
//! behaviour routes through `stencilcraft_engine::Command`; painting is
//! `stencilcraft_ui_egui`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use stencilcraft_engine::Command;

/// CLI/agent control channel: JSON lines on a TCP socket, one session per
/// connection (the same session core the CLI's `mcp` mode uses).
mod control;

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

struct StencilApp {
    engine: stencilcraft_engine::Engine,
    view: stencilcraft_ui_egui::View,
    kerf_preview: bool,
    status: String,
    open_path: Option<String>,
    /// Active right-panel tab (Silhouette Studio's Design ↔ Send navigation).
    tab: PanelTab,
    /// The cutting mat shown under the sheet and used for Send's encoded job.
    mat: stencilcraft_machine::Mat,
    selected_shape: Option<u64>,
    canvas_drag_shape: Option<u64>,
    last_drag_delta: egui::Vec2,
    send: SendTab,
}

/// The two production modes, mirroring Silhouette Studio's tab model.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PanelTab {
    Design,
    Send,
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
            engine,
            view: stencilcraft_ui_egui::View::default(),
            kerf_preview: false,
            status,
            open_path,
            tab: PanelTab::Design,
            mat: stencilcraft_machine::Mat::CameoPro24x24,
            selected_shape: None,
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
}

impl eframe::App for StencilApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.top_panel(ui);
        self.right_panel(ui);
        self.canvas(ui);
        self.status_bar(ui);
        ui.ctx().request_repaint();
    }
}

impl StencilApp {
    fn top_panel(&mut self, parent: &mut egui::Ui) {
        egui::Panel::top("menu").show(parent, |ui| {
            ui.horizontal(|ui| {
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
                if ui.button("Add rectangle").clicked() {
                    self.run_command(Command::ShapeAddRect {
                        x: 20.0,
                        y: 20.0,
                        width: 40.0,
                        height: 30.0,
                    });
                }
                if ui.button("Remove last").clicked() {
                    let last = self.engine.doc.shapes().last().map(|(_, s)| s.id);
                    if let Some(id) = last {
                        self.run_command(Command::ShapeRemove { id });
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

    /// Right panel with the two production tabs (see docs/ui-design.md).
    fn right_panel(&mut self, parent: &mut egui::Ui) {
        egui::Panel::right("ops").show(parent, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.tab == PanelTab::Design, "Design")
                    .clicked()
                {
                    self.tab = PanelTab::Design;
                }
                if ui
                    .selectable_label(self.tab == PanelTab::Send, "Send")
                    .clicked()
                {
                    self.tab = PanelTab::Send;
                }
            });
            ui.separator();
            match self.tab {
                PanelTab::Design => self.design_tab(ui),
                PanelTab::Send => self.send_tab(ui),
            }
        });
    }

    fn design_tab(&mut self, ui: &mut egui::Ui) {
        // Mat selector: drives both the canvas backdrop and the Send job.
        ui.label("Cutting mat");
        egui::ComboBox::from_label("")
            .selected_text(mat_label(self.mat))
            .show_ui(ui, |ui| {
                for (mat, label) in MAT_CHOICES {
                    if ui.selectable_label(self.mat == *mat, *label).clicked() {
                        self.mat = *mat;
                    }
                }
            });
        ui.separator();
        ui.label("Layers");
        let kerf_width = self.engine.doc.kerf.width_mm;
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut visibility_changes: Vec<(usize, bool)> = Vec::new();
            for (i, layer) in self.engine.doc.layers.iter().enumerate() {
                let kind = match layer.kind {
                    stencilcraft_doc::CutStyle::Cut => "cut",
                    stencilcraft_doc::CutStyle::Engrave => "engrave",
                };
                let mut visible = layer.visible;
                let label = format!("{} ({kind}, {} shapes)", layer.name, layer.shapes.len());
                if ui.checkbox(&mut visible, label).changed() {
                    visibility_changes.push((i, visible));
                }
            }
            for (i, visible) in visibility_changes {
                if let Some(l) = self.engine.doc.layers.get_mut(i) {
                    l.visible = visible;
                }
            }
            ui.separator();
            ui.label(format!("Kerf: {:.3} mm", kerf_width));
            let mut kerf_widget = kerf_width;
            if ui
                .add(egui::Slider::new(&mut kerf_widget, 0.0..=1.0).suffix(" mm"))
                .changed()
            {
                self.run_command(Command::KerfSet {
                    width_mm: kerf_widget,
                });
            }
            ui.checkbox(&mut self.kerf_preview, "Kerf preview");
            ui.separator();
            ui.label("Shapes");
            let shape_rows: Vec<(u64, String)> = self
                .engine
                .doc
                .shapes()
                .map(|(_, shape)| (shape.id, format!("#{} ({:?})", shape.id, shape.style)))
                .collect();
            for (id, label) in shape_rows {
                if ui
                    .selectable_label(self.selected_shape == Some(id), label)
                    .clicked()
                {
                    self.selected_shape = Some(id);
                }
            }
            if let Some(id) = self.selected_shape {
                if self.engine.doc.get_shape(id).is_some() {
                    ui.separator();
                    ui.label(format!("Selected shape #{id}"));
                    ui.horizontal(|ui| {
                        if ui.button("Rotate −15°").clicked() {
                            self.run_command(Command::ShapeRotate { id, degrees: -15.0 });
                        }
                        if ui.button("Rotate +15°").clicked() {
                            self.run_command(Command::ShapeRotate { id, degrees: 15.0 });
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Scale −10%").clicked() {
                            self.run_command(Command::ShapeScale {
                                id,
                                scale_x: 0.9,
                                scale_y: 0.9,
                            });
                        }
                        if ui.button("Scale +10%").clicked() {
                            self.run_command(Command::ShapeScale {
                                id,
                                scale_x: 1.1,
                                scale_y: 1.1,
                            });
                        }
                    });
                    ui.small("Drag a shape on the canvas to move it.");
                } else {
                    self.selected_shape = None;
                }
            }
        });
    }

    fn send_tab(&mut self, ui: &mut egui::Ui) {
        ui.label("Model");
        egui::ComboBox::from_label("")
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
        egui::ComboBox::from_label("")
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

    fn canvas(&mut self, parent: &mut egui::Ui) {
        egui::CentralPanel::default().show(parent, |ui| {
            let clip = ui.available_rect_before_wrap();
            let response = ui.allocate_rect(clip, egui::Sense::click_and_drag());
            let doc = self.engine.doc.clone();
            let view = self.view;
            let kerf_preview = self.kerf_preview;
            let painter = ui.painter_at(clip);
            let backdrop = mat_backdrop(self.mat);
            stencilcraft_ui_egui::paint_document(
                &painter,
                clip,
                &doc,
                &view,
                Some(&backdrop),
                kerf_preview,
                self.selected_shape,
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
                self.selected_shape =
                    stencilcraft_ui_egui::hit_test_shape(&doc, &view, pointer, 8.0);
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
        });
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
