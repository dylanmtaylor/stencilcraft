//! Silhouette Studio's Fill Color, Line Style and Select by Color panels. Colours are stored
//! on each shape (`production.fill_color` / `line_color`, `#rrggbb`) and drawn on the canvas;
//! they label the design (cut by colour, print) without changing what is cut.

use craft_theme::{ControlSize, Srgba};
use craft_ui::choice::Swatch;
use craft_ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft_ui::sections::Panel;
use craft_ui::text::TextField;
use eframe::egui;
use stencilcraft_doc::CutStyle;
use stencilcraft_engine::{Command, ProductionCommand, ProductionField};

use crate::StencilApp;
use crate::panels::{button, hint};

/// The swatch palette, as in Silhouette's colour panels.
pub const PALETTE: &[&str] = &[
    "#000000", "#4d4d4d", "#808080", "#b3b3b3", "#ffffff", "#e6194b", "#ff0000", "#f58231",
    "#ffa500", "#ffe119", "#ffff00", "#bfef45", "#3cb44b", "#00a000", "#42d4f4", "#00ffff",
    "#4363d8", "#0000ff", "#911eb4", "#800080", "#f032e6", "#ff69b4", "#9a6324", "#800000",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourTarget {
    Fill,
    Line,
}

impl ColourTarget {
    fn field(self) -> ProductionField {
        match self {
            Self::Fill => ProductionField::FillColor,
            Self::Line => ProductionField::LineColor,
        }
    }
}

/// `#rrggbb` lower-cased if well formed.
pub fn normalise_hex(text: &str) -> Option<String> {
    let text = text.trim();
    let text = text.strip_prefix('#').unwrap_or(text);
    (text.len() == 6 && text.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| format!("#{}", text.to_ascii_lowercase()))
}

/// A new pattern's tile width.
pub const DEFAULT_PATTERN_TILE_MM: f64 = 25.0;

impl StencilApp {
    /// Set (or clear with `None`) the fill or line colour of the selection: one undo step.
    pub fn set_selection_colour(&mut self, target: ColourTarget, colour: Option<String>) {
        if self.selected_shapes.is_empty() {
            self.status = "select shapes first".to_string();
            return;
        }
        let ids = self.selected_shapes.clone();
        let command = match colour {
            Some(hex) => ProductionCommand::SetProduction {
                ids,
                operation: None,
                line_color: (target == ColourTarget::Line).then(|| hex.clone()),
                fill_color: (target == ColourTarget::Fill).then_some(hex),
                material: None,
                send: None,
                fill_pattern: None,
                fill_gradient: None,
                clear: Vec::new(),
            },
            None => ProductionCommand::SetProduction {
                ids,
                operation: None,
                line_color: None,
                fill_color: None,
                material: None,
                send: None,
                fill_pattern: None,
                fill_gradient: None,
                clear: vec![target.field()],
            },
        };
        self.run_command(Command::Production(command));
    }

    /// The first selected shape's colour for `target`.
    fn selection_colour(&self, target: ColourTarget) -> Option<String> {
        let shape = self
            .selected_shapes
            .first()
            .and_then(|id| self.engine.doc.get_shape(*id))?;
        match target {
            ColourTarget::Fill => shape.production.fill_color.clone(),
            ColourTarget::Line => shape.production.line_color.clone(),
        }
    }

    fn colour_panel(&mut self, ui: &mut egui::Ui, target: ColourTarget) {
        let any = !self.selected_shapes.is_empty();
        let current = self.selection_colour(target);
        let mut picked: Option<Option<String>> = None;
        let mut hex = self.colour_hex_draft.clone();
        let (id, clear_label) = match target {
            ColourTarget::Fill => ("fill", "No Fill"),
            ColourTarget::Line => ("line", "Default Line"),
        };
        let title = match target {
            ColourTarget::Fill => "Fill Color",
            ColourTarget::Line => "Line Style",
        };
        let _ = Panel::new(title)
            .id_salt(format!("stencilcraft-{id}-colour-panel"))
            .show(ui, |ui, _| {
                ui.horizontal_wrapped(|ui| {
                    for (i, colour) in PALETTE.iter().enumerate() {
                        let Some(c) = stencilcraft_ui_egui::parse_hex_colour(colour) else {
                            continue;
                        };
                        let clicked = Swatch::new(*colour, Srgba::from(c))
                            .id_salt(format!("{id}-swatch-{i}"))
                            .size(ControlSize::Sm)
                            .selected(current.as_deref() == Some(*colour))
                            .enabled(any)
                            .show(ui)
                            .clicked;
                        if clicked {
                            picked = Some(Some((*colour).to_string()));
                        }
                    }
                });
                // The field writes `hex` as it is typed; a committed value applies the colour.
                let edit = TextField::new("Colour (#rrggbb)", Bind::bound(&mut hex))
                    .id_salt(format!("{id}-hex"))
                    .limit(9)
                    .show(ui);
                if let Some(value) = edit.events.committed()
                    && let Some(h) = normalise_hex(value)
                {
                    picked = Some(Some(h));
                }
                if button(ui, clear_label, any) {
                    picked = Some(None);
                }
                if target == ColourTarget::Line {
                    ui.horizontal(|ui| {
                        if button(ui, "Cut", any) {
                            self.pending_style = Some(CutStyle::Cut);
                        }
                        if button(ui, "Engrave / Score", any) {
                            self.pending_style = Some(CutStyle::Engrave);
                        }
                    });
                    // Silhouette's perforated line style: the lines become dashes that cut.
                    let (mut cut, mut gap) = self.perforate;
                    ui.horizontal(|ui| {
                        ui.label("Perforate: cut");
                        crate::panels::length_drag(ui, &mut cut, 0.2..=100.0, 0.1);
                        ui.label("gap");
                        crate::panels::length_drag(ui, &mut gap, 0.2..=100.0, 0.1);
                    });
                    self.perforate = (cut, gap);
                    if button(ui, "Perforate Lines", any) {
                        self.pending_perforate = true;
                    }
                }
                hint(
                    ui,
                    if any {
                        "Colours label the design; what is cut depends on the line style."
                    } else {
                        "Select shapes to colour."
                    },
                );
                if target == ColourTarget::Fill {
                    self.fill_gradient_ui(ui);
                    self.fill_pattern_ui(ui);
                }
            });
        self.colour_hex_draft = hex;
        if let Some(colour) = picked {
            self.set_selection_colour(target, colour);
        }
        if let Some(style) = self.pending_style.take() {
            self.handle_menu_action(crate::TopMenuAction::SetStyle(style));
        }
        if std::mem::take(&mut self.pending_perforate) {
            self.perforate_selection();
        }
    }

    /// Line Style › Perforate Lines: the selected lines become cut dashes (`generate.perforate`).
    pub fn perforate_selection(&mut self) {
        if self.selected_shapes.is_empty() {
            return;
        }
        let (cut, gap) = self.perforate;
        let ids = self.selected_shapes.clone();
        self.run_command(Command::GeneratePerforate(
            stencilcraft_engine::generate::PerforateArgs {
                ids: ids.clone(),
                cut_mm: cut,
                gap_mm: gap,
            },
        ));
        self.set_selection(ids);
    }

    pub fn fill_colour_panel(&mut self, ui: &mut egui::Ui) {
        self.colour_panel(ui, ColourTarget::Fill);
    }

    /// Set (or clear with `None`) the selection's pattern fill: one undo step.
    pub fn set_selection_pattern(&mut self, pattern: Option<stencilcraft_doc::FillPattern>) {
        if self.selected_shapes.is_empty() {
            self.status = "select shapes first".to_string();
            return;
        }
        let clear = if pattern.is_none() {
            vec![stencilcraft_engine::ProductionField::FillPattern]
        } else {
            Vec::new()
        };
        self.run_command(Command::Production(ProductionCommand::SetProduction {
            ids: self.selected_shapes.clone(),
            operation: None,
            line_color: None,
            fill_color: None,
            material: None,
            send: None,
            fill_pattern: pattern.map(Box::new),
            fill_gradient: None,
            clear,
        }));
    }

    /// Set (or clear with `None`) the selection's gradient fill: one undo step.
    pub fn set_selection_gradient(&mut self, gradient: Option<stencilcraft_doc::FillGradient>) {
        if self.selected_shapes.is_empty() {
            self.status = "select shapes first".to_string();
            return;
        }
        let clear = if gradient.is_none() {
            vec![stencilcraft_engine::ProductionField::FillGradient]
        } else {
            Vec::new()
        };
        self.run_command(Command::Production(ProductionCommand::SetProduction {
            ids: self.selected_shapes.clone(),
            operation: None,
            line_color: None,
            fill_color: None,
            material: None,
            send: None,
            fill_pattern: None,
            fill_gradient: gradient.map(Box::new),
            clear,
        }));
    }

    /// The first selected shape's gradient fill.
    pub fn selection_gradient(&self) -> Option<stencilcraft_doc::FillGradient> {
        self.selected_shapes.iter().find_map(|id| {
            self.engine
                .doc
                .get_shape(*id)?
                .production
                .fill_gradient
                .as_deref()
                .cloned()
        })
    }

    /// Fill Color › Gradient: two colours blended across the selected shapes at an angle.
    fn fill_gradient_ui(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label("Fill Gradient");
        let any = !self.selected_shapes.is_empty();
        let current = self.selection_gradient();
        // The draft being typed lives in egui's memory until it is applied; a new selection
        // with a gradient shows that one.
        let draft_id = egui::Id::unique("fill-gradient-draft");
        let fallback = current.clone().unwrap_or(stencilcraft_doc::FillGradient {
            from: "#ffffff".into(),
            to: "#000000".into(),
            angle_deg: 0.0,
        });
        let (mut g, shown_for) = ui
            .ctx()
            .data_mut(|d| d.get_temp::<(stencilcraft_doc::FillGradient, Vec<u64>)>(draft_id))
            .filter(|(_, ids)| *ids == self.selected_shapes)
            .unwrap_or((fallback, self.selected_shapes.clone()));
        let numbers = self.locale_numbers.clone();
        ui.horizontal(|ui| {
            ui.label("From");
            ui.add(egui::TextEdit::singleline(&mut g.from).desired_width(70.0));
            ui.label("To");
            ui.add(egui::TextEdit::singleline(&mut g.to).desired_width(70.0));
        });
        crate::panels::number(
            ui,
            "fill-gradient-angle",
            "Gradient angle",
            &mut g.angle_deg,
            -360.0..=360.0,
            "{n}°",
            &numbers,
        );
        ui.horizontal(|ui| {
            let apply = current.is_none() || current.as_ref() != Some(&g);
            if button(ui, "Apply Gradient", any && apply) {
                match (normalise_hex(&g.from), normalise_hex(&g.to)) {
                    (Some(from), Some(to)) => {
                        self.set_selection_gradient(Some(stencilcraft_doc::FillGradient {
                            from,
                            to,
                            ..g.clone()
                        }))
                    }
                    _ => self.status = "gradient colours must be #rrggbb".to_string(),
                }
            }
            if button(ui, "Remove Gradient", current.is_some()) {
                self.set_selection_gradient(None);
            }
        });
        ui.ctx()
            .data_mut(|d| d.insert_temp(draft_id, (g, shown_for)));
    }

    /// The first selected shape's pattern fill.
    pub fn selection_pattern(&self) -> Option<stencilcraft_doc::FillPattern> {
        self.selected_shapes.iter().find_map(|id| {
            self.engine
                .doc
                .get_shape(*id)?
                .production
                .fill_pattern
                .as_deref()
                .cloned()
        })
    }

    /// Fill Color › Pattern: a picture tiled inside the selected shapes (Silhouette's Fill
    /// Pattern), its tile width and angle.
    fn fill_pattern_ui(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label("Fill Pattern");
        let any = !self.selected_shapes.is_empty();
        let current = self.selection_pattern();
        let numbers = self.locale_numbers.clone();
        if let Some(mut pattern) = current.clone() {
            let mut angle = pattern.angle_deg;
            crate::panels::number(
                ui,
                "fill-pattern-tile",
                "Pattern size",
                &mut pattern.tile_mm,
                0.5..=2000.0,
                "{n} mm",
                &numbers,
            );
            crate::panels::number(
                ui,
                "fill-pattern-angle",
                "Pattern angle",
                &mut angle,
                -360.0..=360.0,
                "{n}°",
                &numbers,
            );
            pattern.angle_deg = angle;
            if Some(&pattern) != current.as_ref() {
                self.set_selection_pattern(Some(pattern));
            }
        }
        ui.horizontal(|ui| {
            if button(ui, "Load Pattern…", any) {
                self.pick_fill_pattern();
            }
            if button(ui, "Remove Pattern", current.is_some()) {
                self.set_selection_pattern(None);
            }
        });
    }

    /// Choose a picture and use it as the selection's pattern fill, 25 mm a tile.
    #[cfg(not(target_arch = "wasm32"))]
    fn pick_fill_pattern(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Image",
                &["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff"],
            )
            .pick_file()
        else {
            return;
        };
        match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                stencilcraft_raster::pattern_from_bytes(&bytes, DEFAULT_PATTERN_TILE_MM)
                    .map_err(|e| e.to_string())
            }) {
            Ok(pattern) => self.set_selection_pattern(Some(pattern)),
            Err(error) => self.status = format!("{}: {error}", path.display()),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn pick_fill_pattern(&mut self) {
        self.status = "pattern fills need the desktop app".to_string();
    }

    pub fn line_style_panel(&mut self, ui: &mut egui::Ui) {
        self.colour_panel(ui, ColourTarget::Line);
    }

    /// The colours in use, with how many shapes have each, for Select by Color.
    pub fn colours_in_use(&self) -> Vec<(ColourTarget, String, Vec<u64>)> {
        let mut out: Vec<(ColourTarget, String, Vec<u64>)> = Vec::new();
        for (_, shape) in self.engine.doc.shapes() {
            if !stencilcraft_ui_egui::is_selectable(shape) {
                continue;
            }
            for (target, colour) in [
                (ColourTarget::Line, &shape.production.line_color),
                (ColourTarget::Fill, &shape.production.fill_color),
            ] {
                let Some(colour) = colour else { continue };
                match out.iter_mut().find(|(t, c, _)| *t == target && c == colour) {
                    Some((_, _, ids)) => ids.push(shape.id),
                    None => out.push((target, colour.clone(), vec![shape.id])),
                }
            }
        }
        out
    }

    /// The Select by Color panel: each line and fill colour in use; Select picks its shapes.
    pub fn select_by_colour_panel(&mut self, ui: &mut egui::Ui) {
        let colours = self.colours_in_use();
        let mut select = None;
        let mut recolor_row = self.recolor_row;
        let mut recolor_to: Option<(usize, String)> = None;
        let _ = Panel::new("Select by Color")
            .id_salt("stencilcraft-select-by-colour-panel")
            .show(ui, |ui, _| {
                if colours.is_empty() {
                    hint(ui, "No shape has a line or fill colour yet.");
                }
                for (i, (target, colour, ids)) in colours.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if let Some(c) = stencilcraft_ui_egui::parse_hex_colour(colour) {
                            let _ = Swatch::new(colour.clone(), Srgba::from(c))
                                .id_salt(format!("select-colour-{i}"))
                                .size(ControlSize::Sm)
                                .show(ui);
                        }
                        let what = match target {
                            ColourTarget::Line => "line",
                            ColourTarget::Fill => "fill",
                        };
                        ui.label(format!("{colour} {what} ({})", ids.len()));
                        if button(ui, "Select", true) {
                            select = Some(ids.clone());
                        }
                        if button(ui, "Recolor", true) {
                            recolor_row = (recolor_row != Some(i)).then_some(i);
                        }
                    });
                    if recolor_row == Some(i) {
                        ui.horizontal_wrapped(|ui| {
                            for (k, swatch) in PALETTE.iter().enumerate() {
                                let Some(c) = stencilcraft_ui_egui::parse_hex_colour(swatch) else {
                                    continue;
                                };
                                if Swatch::new(*swatch, Srgba::from(c))
                                    .id_salt(format!("recolor-{i}-{k}"))
                                    .size(ControlSize::Sm)
                                    .show(ui)
                                    .clicked
                                {
                                    recolor_to = Some((i, (*swatch).to_string()));
                                }
                            }
                        });
                    }
                }
            });
        self.recolor_row = recolor_row;
        if let Some(ids) = select {
            self.set_selection(ids);
        }
        if let Some((row, colour)) = recolor_to
            && let Some((target, _, ids)) = colours.get(row)
        {
            // Recolor Artwork: every shape with that colour takes the new one (one step).
            let keep = std::mem::replace(&mut self.selected_shapes, ids.clone());
            self.set_selection_colour(*target, Some(colour));
            self.set_selection(keep);
            self.recolor_row = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_fill_is_set_resized_and_removed() {
        let mut app = StencilApp::new(None);
        let mut square = kurbo::BezPath::new();
        square.move_to((0.0, 0.0));
        square.line_to((20.0, 0.0));
        square.line_to((20.0, 20.0));
        square.close_path();
        let id = app
            .engine
            .doc
            .push_shape(0, square, stencilcraft_doc::CutStyle::Cut)
            .unwrap()
            .id;
        app.selected_shapes = vec![id];
        let tile = stencilcraft_raster::RgbaImage::from_pixel(2, 2, image_pixel());
        let png = stencilcraft_raster::encode_png_base64(&tile).unwrap();
        let pattern = stencilcraft_doc::FillPattern {
            png_base64: png.into(),
            tile_mm: 25.0,
            angle_deg: 0.0,
        };
        app.set_selection_pattern(Some(pattern.clone()));
        assert_eq!(app.selection_pattern(), Some(pattern.clone()));
        app.set_selection_pattern(Some(stencilcraft_doc::FillPattern {
            tile_mm: 0.0,
            ..pattern.clone()
        }));
        assert_eq!(
            app.selection_pattern(),
            Some(pattern),
            "a bad tile size is refused"
        );
        app.set_selection_pattern(None);
        assert_eq!(app.selection_pattern(), None);
        let gradient = stencilcraft_doc::FillGradient {
            from: "#FF0000".into(),
            to: "#00f".into(),
            angle_deg: 45.0,
        };
        app.set_selection_gradient(Some(gradient));
        let set = app.selection_gradient().expect("a gradient");
        assert_eq!(
            (set.from.as_str(), set.to.as_str()),
            ("#ff0000", "#0000ff"),
            "colours canonical"
        );
        app.set_selection_gradient(None);
        assert_eq!(app.selection_gradient(), None);
    }

    fn image_pixel() -> craft_raster_codecs::common::Pixel<u8> {
        craft_raster_codecs::common::Pixel([10, 200, 30, 255])
    }

    #[test]
    fn colours_set_clear_and_select() {
        assert_eq!(normalise_hex(" FF8000 "), Some("#ff8000".into()));
        assert_eq!(normalise_hex("#12345"), None);
        let mut app = StencilApp::new(None);
        for x in [0.0, 30.0, 60.0] {
            app.run_command(Command::ShapeAddRect {
                x,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            });
        }
        let ids: Vec<u64> = app.engine.doc.shapes().map(|(_, s)| s.id).collect();
        app.set_selection([ids[0], ids[1]]);
        app.set_selection_colour(ColourTarget::Fill, Some("#ff0000".into()));
        app.set_selection([ids[2]]);
        app.set_selection_colour(ColourTarget::Line, Some("#0000ff".into()));
        let used = app.colours_in_use();
        assert!(used.contains(&(ColourTarget::Fill, "#ff0000".into(), vec![ids[0], ids[1]])));
        assert!(used.contains(&(ColourTarget::Line, "#0000ff".into(), vec![ids[2]])));
        app.set_selection([ids[0]]);
        app.set_selection_colour(ColourTarget::Fill, None);
        assert_eq!(
            app.engine
                .doc
                .get_shape(ids[0])
                .and_then(|s| s.production.fill_color.clone()),
            None
        );
    }
}

#[cfg(test)]
mod perforate_tests {
    use super::*;

    #[test]
    fn perforate_turns_lines_into_dashes() {
        let mut app = StencilApp::new(None);
        app.run_command(Command::ShapeAddRect {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 20.0,
        });
        let id = app
            .engine
            .doc
            .shapes()
            .map(|(_, s)| s.id)
            .next()
            .expect("rect");
        app.set_selection([id]);
        app.perforate = (3.0, 1.0);
        app.perforate_selection();
        let path = &app.engine.doc.get_shape(id).expect("shape").path;
        let pieces = path
            .elements()
            .iter()
            .filter(|el| matches!(el, craft_behavior::kurbo::PathEl::MoveTo(_)))
            .count();
        assert!(pieces > 20, "{pieces} dashes along 120 mm at 4 mm pitch");
        assert!(app.engine.undo());
    }

    #[test]
    fn drawing_can_return_to_select() {
        let mut app = StencilApp::new(None);
        app.active_tool = crate::ActiveTool::Rect;
        let (a, b) = (
            stencilcraft_geom::Point::new(0.0, 0.0),
            stencilcraft_geom::Point::new(20.0, 20.0),
        );
        app.commit_dragged_shape(a, b, crate::DragMods::default());
        assert_eq!(
            app.active_tool,
            crate::ActiveTool::Rect,
            "the tool stays by default"
        );
        app.prefs.select_after_drawing = true;
        app.commit_dragged_shape(a, b, crate::DragMods::default());
        assert_eq!(app.active_tool, crate::ActiveTool::Select);
    }
}
