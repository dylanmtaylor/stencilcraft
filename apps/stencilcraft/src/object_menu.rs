//! Object-menu actions that run production and layer commands: Send to Layer, Attach /
//! Detach, Flatten, Clipping Mask, and the Contour panel, which hides contours of a shape so
//! they are kept but not cut.

use craft_ui::choice::Checkbox;
use craft_ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft_ui::sections::Panel;
use eframe::egui;
use stencilcraft_engine::document::DocumentCommand;
use stencilcraft_engine::{Command, ProductionCommand};

use crate::StencilApp;
use crate::panels::{button, hint};

/// How many contours (subpaths) a path has.
pub fn contour_count(path: &stencilcraft_geom::BezPath) -> usize {
    path.elements()
        .iter()
        .filter(|el| matches!(el, craft_behavior::kurbo::PathEl::MoveTo(_)))
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

    /// Object › Clipping Mask › Make: the topmost selected shape clips every other selected
    /// shape (each keeps its own copy of the mask, so Release gives each back with its mask).
    pub fn make_clipping_mask(&mut self) {
        if self.selected_shapes.len() < 2 {
            self.status = "select the shapes and the mask above them".to_string();
            return;
        }
        // Paint order: the last selected shape in the document is the topmost.
        let order: Vec<u64> = self
            .engine
            .doc
            .shapes()
            .map(|(_, s)| s.id)
            .filter(|id| self.selected_shapes.contains(id))
            .collect();
        let Some((&mask, clipped)) = order.split_last() else {
            return;
        };
        let mut results = Vec::new();
        for (n, &shape) in clipped.iter().enumerate() {
            // The last clipped shape takes the mask itself; the others take copies.
            let mask_for = if n + 1 == clipped.len() {
                Some(mask)
            } else {
                match self.engine.run(Command::ShapesDuplicate {
                    ids: vec![mask],
                    dx_mm: 0.0,
                    dy_mm: 0.0,
                }) {
                    Ok(stencilcraft_engine::Effect::ShapesAdded { ids }) => ids.first().copied(),
                    Ok(stencilcraft_engine::Effect::ShapeAdded { id }) => Some(id),
                    _ => None,
                }
            };
            let Some(mask_for) = mask_for else {
                self.status = "the mask could not be copied".to_string();
                break;
            };
            match self
                .engine
                .run(Command::Production(ProductionCommand::Combine {
                    ids: vec![shape, mask_for],
                    operation: stencilcraft_doc::CombineOp::Intersect,
                })) {
                Ok(stencilcraft_engine::Effect::ShapeAdded { id }) => results.push(id),
                Ok(_) => {}
                Err(e) => {
                    self.status = e.to_string();
                    break;
                }
            }
        }
        if !results.is_empty() {
            self.status = format!("clipping mask made ({} shapes)", results.len());
            self.set_selection(results);
        }
    }

    /// Object › Combine (live): the selection as one shape that keeps its members, so the
    /// operation can be changed or undone later (Uncombine).
    pub fn live_combine(&mut self, operation: stencilcraft_doc::CombineOp) {
        if self.selected_shapes.len() < 2 {
            self.status = "select two or more shapes to combine".to_string();
            return;
        }
        let ids = self.selected_shapes.clone();
        self.select_added(Command::Production(ProductionCommand::Combine {
            ids,
            operation,
        }));
    }

    /// The live Combine operation of the one selected shape, if it is one.
    pub fn selected_combine(&self) -> Option<(u64, stencilcraft_doc::CombineOp)> {
        let [id] = self.selected_shapes.as_slice() else {
            return None;
        };
        let shape = self.engine.doc.get_shape(*id)?;
        match shape.production.group.as_deref()?.kind {
            stencilcraft_doc::GroupKind::Combine(op) => Some((*id, op)),
            stencilcraft_doc::GroupKind::Flatten => None,
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
        let _ = Panel::new("Contour")
            .id_salt("stencilcraft-contour-panel")
            .show(ui, |ui, _| {
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
                    let _ = Checkbox::new(Bind::bound(&mut shown))
                        .label(label)
                        .id_salt(("contour", i))
                        .show(ui);
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

    #[test]
    fn one_mask_clips_several_shapes_and_live_combines_change() {
        use stencilcraft_doc::CombineOp;
        let mut app = StencilApp::new(None);
        for (x, y, w) in [(0.0, 0.0, 20.0), (30.0, 0.0, 20.0), (10.0, 5.0, 40.0)] {
            app.run_command(Command::ShapeAddRect {
                x,
                y,
                width: w,
                height: 10.0,
            });
        }
        let ids: Vec<u64> = app.engine.doc.shapes().map(|(_, s)| s.id).collect();
        // The third rectangle (topmost) clips the first two.
        app.set_selection(ids.clone());
        app.make_clipping_mask();
        assert_eq!(app.selected_shapes.len(), 2, "{}", app.status);
        for id in app.selected_shapes.clone() {
            let b = stencilcraft_geom::Shape::bounding_box(
                &app.engine.doc.get_shape(id).expect("clip").path,
            );
            assert!(b.y0 >= 5.0 - 1e-6, "clipped to the mask: {b:?}");
        }
        // Live combine, then change its operation and undo it.
        let mut app = StencilApp::new(None);
        for x in [0.0, 10.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            });
        }
        let ids: Vec<u64> = app.engine.doc.shapes().map(|(_, s)| s.id).collect();
        app.set_selection(ids);
        app.live_combine(CombineOp::Unite);
        let (id, op) = app.selected_combine().expect("a live combine");
        assert_eq!(op, CombineOp::Unite);
        app.run_command(Command::Production(ProductionCommand::SetCombine {
            id,
            operation: CombineOp::Intersect,
        }));
        app.set_selection([id]);
        assert_eq!(
            app.selected_combine().map(|(_, op)| op),
            Some(CombineOp::Intersect)
        );
        app.release_clipping_mask();
        assert_eq!(
            app.engine.doc.shapes().count(),
            2,
            "uncombine gives both back"
        );
    }

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
