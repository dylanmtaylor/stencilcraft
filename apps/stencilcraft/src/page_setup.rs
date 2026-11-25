//! Silhouette Studio's Page Setup panel: the page (cutting mat, media size, custom width and
//! height, portrait or landscape, view rotation) and, on its second part, the borders, grid
//! and snapping (Show Print/Cut Border, Show Grid, Snap to Grid, Smart Snapping, tolerance,
//! grid spacing and divisions). Page changes are document commands, so each is one undo step.

use craft_theme::ControlSize;
use craft_ui::choice::{Checkbox, Segmented};
use craft_ui::contract::{Bind, Entry, Props as _, Show as _, ShowContainer as _};
use craft_ui::pickers::Select;
use craft_ui::sections::{Panel, Section};
use eframe::egui;
use stencilcraft_doc::PagePreset;
use stencilcraft_engine::Command;
use stencilcraft_engine::document::DocumentCommand;

use crate::panels::{labelled, number};
use crate::{CanvasViewCommand, MAT_CHOICES, StencilApp};

/// Media sizes offered, as in Silhouette's Media Size list.
pub const MEDIA_PRESETS: &[(PagePreset, &str)] = &[
    (PagePreset::Mat12x12, "12 × 12 in"),
    (PagePreset::Mat12x24, "12 × 24 in"),
    (PagePreset::Letter, "Letter (8.5 × 11 in)"),
    (PagePreset::A4, "A4 (210 × 297 mm)"),
    (PagePreset::JoyMat4_5x6_5, "Joy 4.5 × 6.5 in"),
    (PagePreset::JoyMat4_5x12, "Joy 4.5 × 12 in"),
    (PagePreset::JoyXtraMat8_5x12, "Joy Xtra 8.5 × 12 in"),
    (PagePreset::Custom, "Custom"),
];

/// The preset a `width` × `height` page matches in either orientation, else Custom.
pub fn preset_for(width: f64, height: f64) -> PagePreset {
    let near = |a: f64, b: f64| (a - b).abs() < 0.01;
    MEDIA_PRESETS
        .iter()
        .filter_map(|(preset, _)| preset.size_mm().map(|size| (*preset, size)))
        .find(|(_, (w, h))| {
            (near(*w, width) && near(*h, height)) || (near(*w, height) && near(*h, width))
        })
        .map_or(PagePreset::Custom, |(preset, _)| preset)
}

impl StencilApp {
    fn set_board(&mut self, width: f64, height: f64) {
        if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
            return;
        }
        self.run_command(Command::DocumentSetBoard {
            width_mm: width,
            height_mm: height,
        });
    }

    pub fn page_setup_panel(&mut self, ui: &mut egui::Ui) {
        let numbers = self.locale_numbers.clone();
        let (width, height) = (
            self.engine.doc.board.width_mm,
            self.engine.doc.board.height_mm,
        );
        let mut preset = preset_for(width, height);
        let before_preset = preset;
        let (mut w, mut h) = (width, height);
        let mut landscape = width > height;
        let mut turns = self.view.quarter_turns % 4;
        let mut mat = self.mat;
        let mut prefs = self.prefs.clone();
        let snap = self.engine.doc.snap.clone();
        let (mut snap_grid, mut smart, mut tolerance) =
            (snap.grid, snap.objects, snap.tolerance_mm);
        let grid = self.engine.doc.grid.clone();
        let mut spacing = grid.spacing_mm;
        let mut divisions = f64::from(grid.subdivisions.max(1));
        let mat_entries: Vec<_> = MAT_CHOICES
            .iter()
            .map(|(choice, label)| Entry::new(*choice, *label))
            .collect();
        let media_entries: Vec<Entry<PagePreset>> = MEDIA_PRESETS
            .iter()
            .map(|(choice, label)| Entry::new(*choice, *label))
            .collect();
        let orientation_entries = [Entry::new(false, "Portrait"), Entry::new(true, "Landscape")];
        let turn_entries = [
            Entry::new(0, "0°"),
            Entry::new(1, "90°"),
            Entry::new(2, "180°"),
            Entry::new(3, "270°"),
        ];
        let _ = Panel::new("Page Setup")
            .id_salt("stencilcraft-page-setup-panel")
            .show(ui, |ui, _| {
            let _ = Section::new("Page").id_salt("page-setup-page").show(ui, |ui, _| {
                labelled(ui, "Cutting Mat", |ui| {
                    let _ = Select::new("Cutting mat", Bind::bound(&mut mat), &mat_entries)
                        .id_salt("page-mat")
                        .show(ui);
                });
                labelled(ui, "Media Size", |ui| {
                    let _ = Select::new("Media size", Bind::bound(&mut preset), &media_entries)
                        .id_salt("page-media")
                        .show(ui);
                });
                labelled(ui, "Width", |ui| {
                    number(
                        ui,
                        "page-width",
                        "Page width",
                        &mut w,
                        1.0..=10_000.0,
                        "{n} mm",
                        &numbers,
                    );
                });
                labelled(ui, "Height", |ui| {
                    number(
                        ui,
                        "page-height",
                        "Page height",
                        &mut h,
                        1.0..=100_000.0,
                        "{n} mm",
                        &numbers,
                    );
                });
                let _ = Segmented::new(
                    "Orientation",
                    Bind::bound(&mut landscape),
                    &orientation_entries,
                )
                .id_salt("page-orientation")
                .size(ControlSize::Sm)
                .show(ui);
                labelled(ui, "Rotate View", |ui| {
                    let _ = Segmented::new("Rotate view", Bind::bound(&mut turns), &turn_entries)
                        .id_salt("page-rotate-view")
                        .size(ControlSize::Sm)
                        .show(ui);
                });
            });
            let _ = Section::new("Borders and Grid")
                .id_salt("page-setup-grid")
                .show(ui, |ui, _| {
                let check = |ui: &mut egui::Ui, label: &'static str, value: &mut bool| {
                    let _ = Checkbox::new(Bind::bound(value)).label(label).show(ui);
                };
                check(ui, "Show Print Border", &mut prefs.show_print_border);
                check(ui, "Show Cut Border", &mut prefs.show_cut_border);
                ui.horizontal(|ui| {
                    check(ui, "Show Grid", &mut prefs.show_grid);
                    check(ui, "Snap to Grid", &mut snap_grid);
                });
                check(ui, "Enable Smart Snapping", &mut smart);
                labelled(ui, "Tolerance", |ui| {
                    number(
                        ui,
                        "page-snap-tolerance",
                        "Snap tolerance",
                        &mut tolerance,
                        0.1..=50.0,
                        "{n} mm",
                        &numbers,
                    );
                });
                labelled(ui, "Spacing", |ui| {
                    number(
                        ui,
                        "page-grid-spacing",
                        "Grid spacing",
                        &mut spacing,
                        0.5..=1000.0,
                        "{n} mm",
                        &numbers,
                    );
                });
                labelled(ui, "Divisions", |ui| {
                    number(
                        ui,
                        "page-grid-divisions",
                        "Grid divisions",
                        &mut divisions,
                        1.0..=64.0,
                        "{n}",
                        &numbers,
                    );
                });
            });
        });

        self.mat = mat;
        if preset != before_preset {
            if let Some((pw, ph)) = preset.size_mm() {
                // Keep the page's current orientation.
                let (pw, ph) = if landscape { (ph, pw) } else { (pw, ph) };
                self.set_board(pw, ph);
            }
        } else if (w - width).abs() > 1e-9 || (h - height).abs() > 1e-9 {
            self.set_board(w, h);
        } else if landscape != (width > height) && (width - height).abs() > 1e-9 {
            self.set_board(height, width);
        }
        if turns != self.view.quarter_turns % 4 {
            self.pending_view_command = Some(CanvasViewCommand::Rotate(turns));
        }
        // Silhouette turns Snap to Grid on with Show Grid.
        if prefs.show_grid && !self.prefs.show_grid {
            snap_grid = true;
        }
        if prefs != self.prefs {
            self.prefs = prefs;
            self.save_prefs();
        }
        let tolerance_changed =
            tolerance.is_finite() && (tolerance - snap.tolerance_mm).abs() > 1e-9;
        if snap_grid != snap.grid || smart != snap.objects || tolerance_changed {
            self.run_command(Command::Document(DocumentCommand::SnapSet {
                grid: (snap_grid != snap.grid).then_some(snap_grid),
                guides: None,
                objects: (smart != snap.objects).then_some(smart),
                spacing: (smart != snap.objects).then_some(smart),
                points: None,
                tolerance_mm: tolerance_changed.then_some(tolerance),
            }));
        }
        let divisions_now = if divisions.is_finite() {
            // In 1..=64 after the clamp, so the cast is exact.
            divisions.round().clamp(1.0, 64.0) as u32
        } else {
            grid.subdivisions
        };
        if (spacing.is_finite() && (spacing - grid.spacing_mm).abs() > 1e-9)
            || divisions_now != grid.subdivisions
        {
            self.run_command(Command::Document(DocumentCommand::GridSet {
                spacing_mm: if spacing.is_finite() {
                    spacing
                } else {
                    grid.spacing_mm
                },
                subdivisions: divisions_now,
                visible: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_match_either_orientation() {
        assert_eq!(preset_for(304.8, 304.8), PagePreset::Mat12x12);
        assert_eq!(preset_for(279.4, 215.9), PagePreset::Letter);
        assert_eq!(preset_for(100.0, 150.0), PagePreset::Custom);
        assert_eq!(preset_for(f64::NAN, 1.0), PagePreset::Custom);
    }
}
