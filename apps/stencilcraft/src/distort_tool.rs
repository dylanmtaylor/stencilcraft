//! Free Distort and Lattice: a grid of handles over the selection (one cell for Free
//! Distort, more for a lattice). Dragging a handle bends the shapes with it, previewed live;
//! Apply (or Enter) runs `shapes.distort` as one undo step, Cancel (or Esc) drops it.

use craft_theme::{ControlSize, Variant};
use craft_ui::buttons::Button;
use craft_ui::contract::{Props as _, Show as _, ShowContainer as _};
use craft_ui::sections::Panel;
use eframe::egui;
use stencilcraft_engine::Command;
use stencilcraft_engine::generate::{DistortArgs, MAX_LATTICE, distort_path, lattice_points};
use stencilcraft_geom::{BezPath, Point};

use crate::panels::{button, hint, labelled, number};
use crate::{StencilApp, WorkspacePanel};

/// Handles within this many screen pixels of the pointer can be grabbed.
const GRAB_PX: f32 = 9.0;

#[derive(Clone, Debug, PartialEq)]
pub struct DistortDraft {
    pub ids: Vec<u64>,
    pub bounds: stencilcraft_geom::Rect,
    pub rows: u32,
    pub columns: u32,
    pub points: Vec<[f64; 2]>,
    /// The handle being dragged.
    pub active: Option<usize>,
}

impl DistortDraft {
    fn reset(&mut self) {
        self.points = lattice_points(self.bounds, self.rows, self.columns);
    }
}

impl StencilApp {
    /// Object › Distort › Free Distort (`rows` = `columns` = 1) or Lattice: handles over the
    /// selection.
    pub fn begin_distort(&mut self, rows: u32, columns: u32) {
        let Some(bounds) =
            stencilcraft_ui_egui::selected_world_bounds(&self.engine.doc, &self.selected_shapes)
        else {
            self.status = "select shapes to distort".to_string();
            return;
        };
        if !(bounds.width() > 0.0 && bounds.height() > 0.0) {
            self.status = "the selection has no width or height".to_string();
            return;
        }
        let (rows, columns) = (rows.clamp(1, MAX_LATTICE), columns.clamp(1, MAX_LATTICE));
        self.distort = Some(DistortDraft {
            ids: self.selected_shapes.clone(),
            bounds,
            rows,
            columns,
            points: lattice_points(bounds, rows, columns),
            active: None,
        });
        self.pending_panel = Some(WorkspacePanel::Distort);
        self.status = "drag the handles; Enter applies, Esc cancels".to_string();
    }

    pub fn apply_distort(&mut self) {
        let Some(draft) = self.distort.take() else {
            return;
        };
        if draft.points == lattice_points(draft.bounds, draft.rows, draft.columns) {
            self.status = "nothing to distort".to_string();
            return;
        }
        let ids = draft.ids.clone();
        self.run_command(Command::GenerateDistort(DistortArgs {
            ids: draft.ids,
            rows: draft.rows,
            columns: draft.columns,
            points: draft.points,
        }));
        self.set_selection(ids);
    }

    /// Enter applies and Esc cancels a distort in progress. Returns whether a key was used.
    pub fn handle_distort_keys(&mut self, ctx: &egui::Context) -> bool {
        if self.distort.is_none() || ctx.text_edit_focused() {
            return false;
        }
        let apply = ctx.input_mut(|input| {
            crate::consume_exact(input, egui::Modifiers::NONE, egui::Key::Enter)
        });
        let cancel = crate::take_escape(ctx);
        if apply {
            self.apply_distort();
        } else if cancel {
            self.distort = None;
            self.status = "distort cancelled".to_string();
        }
        apply || cancel
    }

    /// Handles, lattice lines and the bent shapes on the canvas; dragging moves a handle.
    /// Returns true while a distort is in progress (it owns the pointer).
    pub fn distort_canvas(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
    ) -> bool {
        let view = self.view;
        let Some(draft) = self.distort.as_mut() else {
            return false;
        };
        if response.drag_started()
            && let Some(press) = ui
                .input(|input| input.pointer.press_origin())
                .or(response.interact_pointer_pos())
        {
            draft.active = draft
                .points
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    (
                        i,
                        view.world_to_screen(Point::new(p[0], p[1])).distance(press),
                    )
                })
                .filter(|(_, d)| *d <= GRAB_PX)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i);
        }
        if response.dragged()
            && let (Some(i), Some(pos)) = (draft.active, response.interact_pointer_pos())
            && let Some(slot) = draft.points.get_mut(i)
        {
            let p = view.screen_to_world(pos.to_vec2());
            if p.x.is_finite() && p.y.is_finite() {
                *slot = [p.x, p.y];
            }
        }
        if response.drag_stopped() {
            draft.active = None;
        }
        let draft = draft.clone();
        let colour = stencilcraft_ui_egui::Palette::for_theme(self.theme).selection;
        // The bent shapes.
        let bent: Vec<BezPath> = draft
            .ids
            .iter()
            .filter_map(|id| self.engine.doc.get_shape(*id))
            .map(|s| {
                distort_path(
                    &s.path,
                    draft.bounds,
                    draft.rows,
                    draft.columns,
                    &draft.points,
                )
            })
            .collect();
        let preview = stencilcraft_ui_egui::Palette::for_theme(self.theme).kerf_line;
        stencilcraft_ui_egui::paint_preview_paths(painter, &view, &bent, preview);
        // Lattice lines and handles.
        let at = |row: u32, col: u32| -> Option<egui::Pos2> {
            let p = draft
                .points
                .get((row * (draft.columns + 1) + col) as usize)?;
            Some(view.world_to_screen(Point::new(p[0], p[1])))
        };
        let stroke = egui::Stroke::new(1.0, colour);
        for row in 0..=draft.rows {
            for col in 0..=draft.columns {
                let Some(here) = at(row, col) else { continue };
                if let Some(right) = (col < draft.columns).then(|| at(row, col + 1)).flatten() {
                    painter.line_segment([here, right], stroke);
                }
                if let Some(below) = (row < draft.rows).then(|| at(row + 1, col)).flatten() {
                    painter.line_segment([here, below], stroke);
                }
                painter.rect_filled(
                    egui::Rect::from_center_size(here, egui::vec2(7.0, 7.0)),
                    1.0,
                    colour,
                );
            }
        }
        true
    }

    pub fn distort_panel(&mut self, ui: &mut egui::Ui) {
        let numbers = self.locale_numbers.clone();
        let mut apply = false;
        let mut cancel = false;
        let mut start = None;
        let _ = Panel::new("Distort")
            .id_salt("stencilcraft-distort-panel")
            .show(ui, |ui, _| match self.distort.as_mut() {
                None => {
                    hint(
                        ui,
                        "Select shapes, then choose Free Distort or Lattice Distort.",
                    );
                    ui.horizontal(|ui| {
                        if button(ui, "Free Distort", !self.selected_shapes.is_empty()) {
                            start = Some((1, 1));
                        }
                        if button(ui, "Lattice Distort", !self.selected_shapes.is_empty()) {
                            start = Some((3, 3));
                        }
                    });
                }
                Some(draft) => {
                    let (mut rows, mut columns) = (f64::from(draft.rows), f64::from(draft.columns));
                    labelled(ui, "Rows", |ui| {
                        number(
                            ui,
                            "distort-rows",
                            "Lattice rows",
                            &mut rows,
                            1.0..=20.0,
                            "{n}",
                            &numbers,
                        );
                    });
                    labelled(ui, "Columns", |ui| {
                        number(
                            ui,
                            "distort-columns",
                            "Lattice columns",
                            &mut columns,
                            1.0..=20.0,
                            "{n}",
                            &numbers,
                        );
                    });
                    // In 1..=20 after the clamps, so the casts are exact.
                    let (rows, columns) = (
                        rows.round().clamp(1.0, 20.0) as u32,
                        columns.round().clamp(1.0, 20.0) as u32,
                    );
                    if (rows, columns) != (draft.rows, draft.columns) {
                        draft.rows = rows;
                        draft.columns = columns;
                        draft.reset();
                    }
                    ui.horizontal(|ui| {
                        if button(ui, "Reset", true) {
                            draft.reset();
                        }
                        cancel = button(ui, "Cancel", true);
                        apply = Button::new("Apply")
                            .variant(Variant::PRIMARY)
                            .size(ControlSize::Sm)
                            .show(ui)
                            .clicked();
                    });
                    hint(
                        ui,
                        "Drag the handles on the canvas; Enter applies, Esc cancels.",
                    );
                }
            });
        if let Some((rows, columns)) = start {
            self.begin_distort(rows, columns);
        }
        if cancel {
            self.distort = None;
        }
        if apply {
            self.apply_distort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_geom::Shape as _;

    #[test]
    fn free_distort_applies_once_and_needs_a_change() {
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        });
        let id = app
            .engine
            .doc
            .shapes()
            .map(|(_, s)| s.id)
            .next()
            .expect("rect");
        app.begin_distort(1, 1);
        assert!(app.distort.is_none(), "nothing selected");
        app.set_selection([id]);
        app.begin_distort(1, 1);
        app.apply_distort();
        assert!(app.status.contains("nothing to distort"));
        app.begin_distort(1, 1);
        if let Some(d) = app.distort.as_mut() {
            d.points[1] = [30.0, -10.0];
        }
        app.apply_distort();
        let b = app
            .engine
            .doc
            .get_shape(id)
            .map(|s| s.path.bounding_box())
            .expect("shape");
        assert!((b.x1 - 30.0).abs() < 1e-6, "{b:?}");
        assert!(app.engine.undo());
    }
}
