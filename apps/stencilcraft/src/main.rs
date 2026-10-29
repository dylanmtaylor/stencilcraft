//! The StencilCraft desktop app: window bootstrap, file dialogs and menus. All
//! behaviour routes through `stencilcraft_engine::Command`; painting is
//! `stencilcraft_ui_egui`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use stencilcraft_engine::Command;

/// CLI/agent control channel: JSON lines on a TCP socket, one session per
/// connection (the same session core the CLI's `mcp` mode uses).
mod control;

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
        self.left_panel(ui);
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

    fn left_panel(&mut self, parent: &mut egui::Ui) {
        let kerf_width = self.engine.doc.kerf.width_mm;
        egui::Panel::left("layers").show(parent, |ui| {
            ui.label("Layers");
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
                for (_, shape) in self.engine.doc.shapes() {
                    ui.label(format!("#{} ({:?})", shape.id, shape.style));
                }
            });
        });
    }

    fn canvas(&mut self, parent: &mut egui::Ui) {
        egui::CentralPanel::default().show(parent, |ui| {
            let clip = ui.available_rect_before_wrap();
            let response = ui.allocate_rect(clip, egui::Sense::click_and_drag());
            let doc = self.engine.doc.clone();
            let view = self.view;
            let kerf_preview = self.kerf_preview;
            let painter = ui.painter_at(clip);
            stencilcraft_ui_egui::paint_document(&painter, clip, &doc, &view, kerf_preview);

            // Zoom: scroll wheel around the pointer.
            if response.hovered() {
                let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    let pointer = response.interact_pointer_pos().unwrap_or(clip.center());
                    self.view.zoom_at(1.0 + scroll / 600.0, pointer);
                }
            }

            // Pan: drag.
            let delta = response.drag_delta();
            if delta != egui::Vec2::ZERO {
                self.view.pan_mm.x -= delta.x / self.view.zoom;
                self.view.pan_mm.y -= delta.y / self.view.zoom;
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
