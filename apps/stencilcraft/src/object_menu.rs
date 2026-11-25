//! Object-menu actions that run production and layer commands: Send to Layer, Attach /
//! Detach, Flatten, Clipping Mask, and the Contour panel, which hides contours of a shape so
//! they are kept but not cut.

use craft_ui::controls::Checkbox;
use craft_ui::patterns::Panel;
use eframe::egui;
use stencilcraft_engine::document::DocumentCommand;
use stencilcraft_engine::{Command, ProductionCommand};

use crate::StencilApp;
use crate::panels::{button, hint};

/// How many contours (subpaths) a path has.
pub fn contour_count(path: &stencilcraft_geom::BezPath) -> usize {
    path.elements()
        .iter()
        .filter(|el| matches!(el, craft_ui::canvas::kurbo::PathEl::MoveTo(_)))
        .count()
}

impl StencilApp {
    /// Object › Arrange › Send to Layer: move the selection onto layer `index`.
    pub fn send_to_layer(&mut self, index: usize) {
        if self.selected_shapes.is_empty() {
            return;
        }
        let ids = self.selected_shapes.clone();
        self.run_command(Command::Document(DocumentCommand::ShapesMoveToLayer {
            ids: ids.clone(),
            layer: index,
        }));
        self.set_selection(ids);
    }

    /// Whether any selected shape is attached.
    pub fn selection_attached(&self) -> bool {
        self.selected_shapes.iter().any(|id| {
            self.engine
                .doc
                .get_shape(*id)
                .is_some_and(|s| s.production.attach.is_some())
        })
    }

    /// Object › Attach / Detach: attach the selection (kept together on the mat), or detach
    /// it when it is attached.
    pub fn toggle_attach(&mut self) {
        let ids = self.selected_shapes.clone();
        let command = if self.selection_attached() {
            ProductionCommand::Detach { ids: ids.clone() }
        } else {
            ProductionCommand::Attach { ids: ids.clone() }
        };
        self.run_command(Command::Production(command));
        self.set_selection(ids);
    }

    /// Object › Flatten: merge the selection into one print-then-cut image.
    pub fn flatten_selection(&mut self) {
        let ids = self.selected_shapes.clone();
        self.select_added(Command::Production(ProductionCommand::Flatten { ids }));
        self.retain_valid_selection();
        if self.selected_shapes.is_empty()
            && let Some(id) = self.engine.doc.shapes().last().map(|(_, s)| s.id)
        {
            self.set_selection([id]);
        }
    }

    /// Object › Clipping Mask › Make: the top shape clips the one under it (kept, so Release
    /// gives both back).
    pub fn make_clipping_mask(&mut self) {
        if self.selected_shapes.len() != 2 {
            self.status = "select a shape and the mask above it".to_string();
            return;
        }
        let ids = self.selected_shapes.clone();
        match self
            .engine
            .run(Command::Production(ProductionCommand::Combine {
                ids,
                operation: stencilcraft_doc::CombineOp::Intersect,
            })) {
            Ok(stencilcraft_engine::Effect::ShapeAdded { id }) => {
                self.set_selection([id]);
                self.status = "clipping mask made".to_string();
            }
            Ok(_) => self.status = "clipping mask made".to_string(),
            Err(e) => self.status = e.to_string(),
        }
    }

    /// Object › Clipping Mask › Release.
    pub fn release_clipping_mask(&mut self) {
        let Some(id) = self.selected_shapes.first().copied() else {
            return;
        };
        self.select_added(Command::Production(ProductionCommand::Uncombine { id }));
    }

    /// The Contour panel: each contour of the selected shape, with a switch to hide it from
    /// cutting (Cricut's Contour; it stays in the shape and can be shown again).
    pub fn contour_panel(&mut self, ui: &mut egui::Ui) {
        let target = match self.selected_shapes.as_slice() {
            [id] => self.engine.doc.get_shape(*id).map(|s| {
                (
                    s.id,
                    contour_count(&s.path),
                    s.production.hidden_contours.clone(),
                )
            }),
            _ => None,
        };
        let mut change: Option<(Vec<usize>, bool)> = None;
        let _ = Panel::new("stencilcraft-contour-panel").show(ui, |ui| {
            let Some((_, count, hidden)) = &target else {
                hint(ui, "Select one shape to hide or show its contours.");
                return;
            };
            for i in 0..*count {
                let mut shown = !hidden.contains(&i);
                let label = if i == 0 {
                    "Contour 1 (outline)".to_string()
                } else {
                    format!("Contour {}", i + 1)
                };
                let was = shown;
                let _ = Checkbox::bool(&label, &mut shown).show(ui);
                if shown != was {
                    change = Some((vec![i], !shown));
                }
            }
            ui.horizontal(|ui| {
                if button(ui, "Hide All Contours", *count > 1) {
                    change = Some(((1..*count).collect(), true));
                }
                if button(ui, "Show All", !hidden.is_empty()) {
                    change = Some(((0..*count).collect(), false));
                }
            });
            hint(ui, "Hidden contours stay in the shape but are not cut.");
        });
        if let (Some((id, ..)), Some((contours, hidden))) = (target, change) {
            self.run_command(Command::Production(ProductionCommand::Contour {
                id,
                contours,
                hidden,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rects(app: &mut StencilApp, n: usize) -> Vec<u64> {
        for i in 0..n {
            app.run_command(Command::ShapeAddRect {
                x: 10.0 * i as f64,
                y: 0.0,
                width: 15.0,
                height: 15.0,
            });
        }
        app.engine.doc.shapes().map(|(_, s)| s.id).collect()
    }

    #[test]
    fn layer_attach_flatten_and_clip() {
        let mut app = StencilApp::new(None);
        let ids = rects(&mut app, 2);
        app.run_command(Command::LayerAdd {
            name: "second".into(),
            kind: stencilcraft_doc::CutStyle::Cut,
        });
        app.set_selection([ids[0]]);
        app.send_to_layer(1);
        assert!(
            app.engine.doc.layers[1]
                .shapes
                .iter()
                .any(|s| s.id == ids[0])
        );

        app.set_selection(ids.clone());
        app.toggle_attach();
        assert!(app.selection_attached());
        app.set_selection(ids.clone());
        app.toggle_attach();
        assert!(!app.selection_attached());

        app.set_selection(ids.clone());
        app.make_clipping_mask();
        assert_eq!(app.engine.doc.shapes().count(), 1, "{}", app.status);
        app.release_clipping_mask();
        assert_eq!(app.engine.doc.shapes().count(), 2);
    }

    #[test]
    fn contours_hide_and_show() {
        let mut app = StencilApp::new(None);
        let ids = rects(&mut app, 2);
        app.set_selection(ids.clone());
        app.handle_menu_action(crate::TopMenuAction::Edit(
            crate::panels::EditAction::MakeCompound,
        ));
        let id = app.selected_shapes[0];
        let shape = app.engine.doc.get_shape(id).expect("compound");
        assert_eq!(contour_count(&shape.path), 2);
        app.run_command(Command::Production(ProductionCommand::Contour {
            id,
            contours: vec![1],
            hidden: true,
        }));
        assert_eq!(
            app.engine
                .doc
                .get_shape(id)
                .map(|s| s.production.hidden_contours.clone()),
            Some(vec![1])
        );
    }
}
