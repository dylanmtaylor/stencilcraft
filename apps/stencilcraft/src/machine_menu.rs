//! Machine-menu actions on top of the Send tab: rescan for cutters, Test Cut (a small square
//! with a triangle inside, as Silhouette cuts to check the blade), Cut (send the job to the
//! chosen cutter), Mat Preview, Mirror on Mat and the load type. Hardware actions need the
//! egui context, so the menu queues them and the Send tab starts them.

use eframe::egui;
use stencilcraft_doc::{CutStyle, Document};
use stencilcraft_engine::{Command, Engine};
use stencilcraft_geom::BezPath;

use crate::StencilApp;

/// A queued Machine-menu hardware action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineAction {
    Rescan,
    TestCut,
    Cut,
}

/// The test-cut design: a 10 mm square with a triangle inside, at the page's top left.
pub fn test_cut_document(board: (f64, f64)) -> Option<Document> {
    let mut engine = Engine::new(Document::default());
    engine
        .run(Command::DocumentSetBoard {
            width_mm: board.0,
            height_mm: board.1,
        })
        .ok()?;
    let mut square = BezPath::new();
    square.move_to((5.0, 5.0));
    square.line_to((15.0, 5.0));
    square.line_to((15.0, 15.0));
    square.line_to((5.0, 15.0));
    square.close_path();
    let mut triangle = BezPath::new();
    triangle.move_to((10.0, 7.0));
    triangle.line_to((13.0, 13.0));
    triangle.line_to((7.0, 13.0));
    triangle.close_path();
    for path in [square, triangle] {
        engine
            .run(Command::ShapeAddPath {
                path,
                style: CutStyle::Cut,
            })
            .ok()?;
    }
    Some(engine.doc)
}

impl StencilApp {
    /// Queue `action` and show the Send tab, which runs it.
    pub fn queue_machine_action(&mut self, action: MachineAction) {
        self.machine_action = Some(action);
        self.send_mode = true;
    }

    /// Whether a cutter is chosen in the Send tab.
    pub fn cutter_chosen(&self) -> bool {
        if self.send.connection_mode == 2 {
            !self.send.bluetooth_address.trim().is_empty()
        } else {
            !self.send.device.is_empty()
        }
    }

    /// Run a queued Machine-menu action (called from the Send tab with its context).
    pub fn run_machine_action(&mut self, ctx: &egui::Context) {
        let Some(action) = self.machine_action.take() else {
            return;
        };
        match action {
            MachineAction::Rescan => {
                let command = if self.send.connection_mode == 1 {
                    stencilcraft_engine::HardwareCommand::BluetoothDevices
                } else {
                    stencilcraft_engine::HardwareCommand::Devices
                };
                self.start_hardware(command, ctx.clone());
            }
            MachineAction::TestCut | MachineAction::Cut if !self.cutter_chosen() => {
                self.status = "choose a cutter in the Send tab first".to_string();
            }
            MachineAction::TestCut => {
                let board = (
                    self.engine.doc.board.width_mm,
                    self.engine.doc.board.height_mm,
                );
                match test_cut_document(board) {
                    Some(doc) => {
                        let command = self.cut_command(true);
                        self.start_hardware_on(doc, command, ctx.clone());
                    }
                    None => self.status = "test cut: could not build the test shape".to_string(),
                }
            }
            MachineAction::Cut => {
                let jobs = self.send_jobs(true);
                self.start_hardware_jobs(jobs, ctx.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cut_is_a_square_and_triangle_and_actions_need_a_cutter() {
        let doc = test_cut_document((304.8, 304.8)).expect("doc");
        assert_eq!(doc.shapes().count(), 2);
        assert_eq!(doc.board.width_mm, 304.8);
        let mut app = StencilApp::new(None);
        app.queue_machine_action(MachineAction::Cut);
        assert!(app.send_mode);
        app.run_machine_action(&egui::Context::default());
        assert!(app.status.contains("choose a cutter"));
        assert!(!app.send.busy, "nothing was started");
    }
}
