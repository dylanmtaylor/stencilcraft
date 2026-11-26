//! Sizing backgrounds (Design Space's templates): the outline of what a design goes on (a
//! T-shirt front, a mug wrap, a card blank, a tote), centred on the page on a guide layer, so it
//! shows for sizing and is never cut.

use craft_ui::contract::{Props as _, ShowContainer as _};
use craft_ui::sections::Panel;
use eframe::egui;
use stencilcraft_doc::CutStyle;
use stencilcraft_engine::document::DocumentCommand;
use stencilcraft_engine::{Command, Effect};
use stencilcraft_geom::BezPath;

use crate::StencilApp;
use crate::panels::{button, hint};

/// The guide layer sizing backgrounds go on.
pub const TEMPLATE_LAYER: &str = "Templates";
const IN: f64 = 25.4;

/// The shape of a background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outline {
    /// A T-shirt front: body, sleeves and neckline.
    Shirt,
    /// A rectangle: wraps, card blanks, bags.
    Rect,
}

/// A sizing background: its name, its size in inches and its outline.
pub struct Sizing {
    pub name: &'static str,
    pub width_in: f64,
    pub height_in: f64,
    pub outline: Outline,
}

/// The backgrounds offered (common blank sizes).
pub const SIZINGS: &[Sizing] = &[
    Sizing {
        name: "T-shirt, adult",
        width_in: 20.0,
        height_in: 28.0,
        outline: Outline::Shirt,
    },
    Sizing {
        name: "T-shirt, youth",
        width_in: 16.0,
        height_in: 22.0,
        outline: Outline::Shirt,
    },
    Sizing {
        name: "Mug wrap, 11 oz",
        width_in: 8.5,
        height_in: 3.5,
        outline: Outline::Rect,
    },
    Sizing {
        name: "Mug wrap, 15 oz",
        width_in: 9.0,
        height_in: 4.0,
        outline: Outline::Rect,
    },
    Sizing {
        name: "Card blank, A2",
        width_in: 5.5,
        height_in: 4.25,
        outline: Outline::Rect,
    },
    Sizing {
        name: "Card blank, A7",
        width_in: 7.0,
        height_in: 5.0,
        outline: Outline::Rect,
    },
    Sizing {
        name: "Tote bag",
        width_in: 14.0,
        height_in: 16.0,
        outline: Outline::Rect,
    },
];

/// `sizing`'s outline, `w` × `h` millimetres, its top-left corner at the origin (y down).
pub fn outline_path(outline: Outline, w: f64, h: f64) -> BezPath {
    let mut path = BezPath::new();
    match outline {
        Outline::Rect => {
            path.move_to((0.0, 0.0));
            path.line_to((w, 0.0));
            path.line_to((w, h));
            path.line_to((0.0, h));
        }
        Outline::Shirt => {
            // Fractions of the width and height, clockwise from the left of the neckline.
            let pts = [
                (0.37, 0.0),
                (0.5, 0.06),
                (0.63, 0.0),
                (1.0, 0.13),
                (0.9, 0.33),
                (0.8, 0.29),
                (0.8, 1.0),
                (0.2, 1.0),
                (0.2, 0.29),
                (0.1, 0.33),
                (0.0, 0.13),
            ];
            for (i, (x, y)) in pts.iter().enumerate() {
                let p = (x * w, y * h);
                if i == 0 {
                    path.move_to(p);
                } else {
                    path.line_to(p);
                }
            }
        }
    }
    path.close_path();
    path
}

impl StencilApp {
    /// The guide layer for backgrounds: the existing one, else a new one made a guide.
    fn template_layer(&mut self) -> Option<usize> {
        if let Some(i) = self
            .engine
            .doc
            .layers
            .iter()
            .position(|l| l.name == TEMPLATE_LAYER && l.template)
        {
            return Some(i);
        }
        self.run_command(Command::LayerAdd {
            name: TEMPLATE_LAYER.to_string(),
            kind: CutStyle::Cut,
        });
        let index = self
            .engine
            .doc
            .layers
            .iter()
            .rposition(|l| l.name == TEMPLATE_LAYER)?;
        self.run_command(Command::Document(DocumentCommand::LayerSetTemplate {
            index,
            template: true,
        }));
        Some(index)
    }

    /// Put sizing background `index` on the guide layer, centred on the page.
    pub fn place_sizing(&mut self, index: usize) {
        let Some(sizing) = SIZINGS.get(index) else {
            return;
        };
        let (w, h) = (sizing.width_in * IN, sizing.height_in * IN);
        let board = &self.engine.doc.board;
        let (x, y) = ((board.width_mm - w) / 2.0, (board.height_mm - h) / 2.0);
        let path =
            stencilcraft_geom::Affine::translate((x, y)) * outline_path(sizing.outline, w, h);
        let Some(layer) = self.template_layer() else {
            return;
        };
        match self.engine.run(Command::ShapeAddPath {
            path,
            style: CutStyle::Cut,
        }) {
            Ok(Effect::ShapeAdded { id }) => {
                self.run_command(Command::Document(DocumentCommand::ShapesMoveToLayer {
                    ids: vec![id],
                    layer,
                }));
                self.status = format!(
                    "{} on the {TEMPLATE_LAYER} layer (shown, never cut)",
                    sizing.name
                );
            }
            Ok(_) => {}
            Err(e) => self.status = e.to_string(),
        }
    }

    pub fn sizing_templates_panel(&mut self, ui: &mut egui::Ui) {
        let units = stencilcraft_ui_egui::length_unit();
        let mut picked = None;
        let _ = Panel::new("Templates")
            .id_salt("stencilcraft-templates-panel")
            .show(ui, |ui, _| {
                for (i, s) in SIZINGS.iter().enumerate() {
                    // The button first: a long name wraps instead of squeezing it.
                    ui.horizontal(|ui| {
                        if button(ui, "Add", true) {
                            picked = Some(i);
                        }
                        ui.label(format!(
                            "{} ({} × {})",
                            s.name,
                            units.format(s.width_in * IN, 0),
                            units.format(s.height_in * IN, 0)
                        ));
                    });
                }
                hint(
                    ui,
                    "A background shows what the design goes on, for sizing. It sits on the \
                     Templates guide layer: shown, never cut.",
                );
            });
        if let Some(i) = picked {
            self.place_sizing(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stencilcraft_geom::Shape as _;

    #[test]
    fn backgrounds_go_on_one_guide_layer_centred_and_sized() {
        let mut app = StencilApp::new(None);
        app.place_sizing(2);
        app.place_sizing(4);
        let layers: Vec<_> = app
            .engine
            .doc
            .layers
            .iter()
            .filter(|l| l.name == TEMPLATE_LAYER)
            .collect();
        assert_eq!(layers.len(), 1, "one Templates layer");
        let layer = layers[0];
        assert!(layer.template, "a guide layer");
        assert_eq!(layer.shapes.len(), 2);
        let b = layer.shapes[0].path.bounding_box();
        assert!((b.width() - 8.5 * IN).abs() < 1e-6 && (b.height() - 3.5 * IN).abs() < 1e-6);
        let board = &app.engine.doc.board;
        assert!(
            (b.center().x - board.width_mm / 2.0).abs() < 1e-6,
            "centred: {b:?}"
        );
        // A shirt's outline spans its whole size.
        let shirt = outline_path(Outline::Shirt, 100.0, 140.0).bounding_box();
        assert!((shirt.width() - 100.0).abs() < 1e-9 && (shirt.height() - 140.0).abs() < 1e-9);
    }
}
