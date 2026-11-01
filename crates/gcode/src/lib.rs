//! Grbl-style G-code export for cutting lasers and plotting routers.
//!
//! Modality: `G21` millimetres, `G90` absolute, feed in mm/min. Each subpath is
//! one cut: rapid (`G0`) to the start with the beam off, power on, `G1` along the
//! path, power off. Laser machines take `M4` (dynamic power) plus an `S` power
//! value in the conventional 0–1000 range, or `M3` for controllers that only model
//! a spindle (`LaserMode`). Coordinates are absolute millimetres from the sheet
//! origin, exactly where the document says.
//!
//! Profile knobs (`GcodeSettings`) cover what a real job needs beyond geometry:
//! power per operation, repeated passes with an optional power ramp, and the
//! on/off command for the controller family. Material/machine presets are the
//! caller's business; nothing here guesses at a specific machine's numbers.

use std::fmt::Write as _;

use stencilcraft_doc::{CutStyle, Document};
use stencilcraft_geom::{Point, path_to_polylines};

#[derive(Debug, thiserror::Error)]
pub enum GcodeError {
    #[error("gcode: {0}")]
    Invalid(String),
    #[error("gcode: {0}")]
    Write(String),
}

/// Most repeated passes a job may ask for: past this the settings are a mistake, not
/// a thick material.
pub const MAX_PASSES: u32 = 10;

/// Highest power Grbl's `S` word conventionally accepts.
pub const MAX_POWER: u32 = 1000;

/// How the controller is told to fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaserMode {
    /// `M4`: power tracks motion (Grbl laser mode). Rapides go dark on their own,
    /// but we still switch off explicitly so the output is safe on any sender.
    #[default]
    Dynamic,
    /// `M3`: constant power while moving, for controllers that only understand a
    /// spindle on/off.
    Constant,
}

impl LaserMode {
    /// The G-code word that turns the tool on.
    pub fn on_command(self) -> &'static str {
        match self {
            Self::Dynamic => "M4",
            Self::Constant => "M3",
        }
    }

    /// Parse the CLI/protocol spelling.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dynamic" | "m4" => Some(Self::Dynamic),
            "constant" | "m3" => Some(Self::Constant),
            _ => None,
        }
    }
}

/// Output settings; a document-independent knob set the CLI and UI can pass in.
#[derive(Debug, Clone, PartialEq)]
pub struct GcodeSettings {
    /// Feed rate while cutting, millimetres per minute.
    pub feed_mm_per_min: f64,
    /// Power for cut layers, 0–1000 (Grbl convention).
    pub power: u32,
    /// Power for engrave layers; `None` uses `power`.
    pub engrave_power: Option<u32>,
    /// How many times each contour is traced, 1–[`MAX_PASSES`].
    pub passes: u32,
    /// Spread the power across passes so the last one runs at full power. Useful on
    /// material that scorches at full power in one hit.
    pub ramp_power: bool,
    /// Which on/off command the controller understands.
    pub mode: LaserMode,
}

impl Default for GcodeSettings {
    fn default() -> Self {
        Self {
            feed_mm_per_min: 1000.0,
            power: 500,
            engrave_power: None,
            passes: 1,
            ramp_power: false,
            mode: LaserMode::Dynamic,
        }
    }
}

impl GcodeSettings {
    /// Checks every knob is inside its range. Errors name the knob and the value.
    pub fn validate(&self) -> Result<(), GcodeError> {
        if !self.feed_mm_per_min.is_finite() || self.feed_mm_per_min <= 0.0 {
            return Err(GcodeError::Invalid(format!(
                "feed rate {} mm/min is not a positive finite rate",
                self.feed_mm_per_min
            )));
        }
        if self.power > MAX_POWER {
            return Err(GcodeError::Invalid(format!(
                "power {} is outside the 0–{MAX_POWER} range",
                self.power
            )));
        }
        if let Some(engrave) = self.engrave_power
            && engrave > MAX_POWER
        {
            return Err(GcodeError::Invalid(format!(
                "engrave power {engrave} is outside the 0–{MAX_POWER} range"
            )));
        }
        if self.passes == 0 || self.passes > MAX_PASSES {
            return Err(GcodeError::Invalid(format!(
                "passes {} is outside the 1–{MAX_PASSES} range",
                self.passes
            )));
        }
        Ok(())
    }

    /// Power for one pass of a contour running at `base` power. Without a ramp every
    /// pass is at `base`; with one, pass `n` of `N` gets `base × n / N`, so the last
    /// pass is the full-power pass.
    pub fn power_for_pass(&self, base: u32, pass: u32) -> u32 {
        if !self.ramp_power || self.passes <= 1 || pass >= self.passes {
            return base;
        }
        let scaled = u64::from(base) * u64::from(pass) / u64::from(self.passes);
        let scaled = u32::try_from(scaled).unwrap_or(base);
        // A pass at S0 would trace the path with the beam off.
        scaled.max(1).min(base)
    }
}

/// Serialise a document to G-code.
pub fn export(doc: &Document, settings: &GcodeSettings) -> Result<String, GcodeError> {
    settings.validate()?;
    doc.validate()
        .map_err(|e| GcodeError::Invalid(e.to_string()))?;

    let mut out = String::with_capacity(4096);
    let _ = writeln!(
        out,
        "; generated by stencilcraft ({} shapes, kerf {:.3} mm)",
        doc.shape_count(),
        doc.kerf.width_mm
    );
    let _ = writeln!(out, "G21 ; millimetres");
    let _ = writeln!(out, "G90 ; absolute coordinates");
    let _ = writeln!(out, "G17 ; XY plane");
    // Start dark: the first move is a rapid to the first contour.
    let _ = writeln!(
        out,
        "{} S0 ; laser off until the first cut",
        settings.mode.on_command()
    );
    for layer in &doc.layers {
        let kind = match layer.kind {
            CutStyle::Cut => "cut",
            CutStyle::Engrave => "engrave",
        };
        let _ = writeln!(out, "; layer {} ({kind})", layer.name);
        if !layer.visible {
            let _ = writeln!(out, "; (layer hidden, skipped)");
            continue;
        }
        let base_power = match layer.kind {
            CutStyle::Engrave => settings.engrave_power.unwrap_or(settings.power),
            CutStyle::Cut => settings.power,
        };
        if settings.passes > 1 {
            let _ = writeln!(
                out,
                "; {base_power} power, {} passes{}",
                settings.passes,
                if settings.ramp_power { ", ramped" } else { "" }
            );
        }
        for shape in &layer.shapes {
            for pl in &path_to_polylines(&shape.path, stencilcraft_geom::DEFAULT_FLATTEN_STEPS) {
                emit_contour(&mut out, &contour_points(pl), settings, base_power)?;
            }
        }
    }
    let _ = writeln!(out, "M5 ; tool off");
    let _ = writeln!(out, "G0 X0 Y0 ; return to origin");
    Ok(out)
}

/// The point list one cut traces: a closed contour ends where it started, and a
/// single-point subpath becomes a minimal in-and-out dot.
fn contour_points(pl: &stencilcraft_geom::Polyline) -> Vec<Point> {
    let Some(first) = pl.points.first().copied() else {
        return Vec::new();
    };
    if pl.points.len() < 2 {
        return vec![first, Point::new(first.x + 0.001, first.y), first];
    }
    let mut points = pl.points.clone();
    if pl.closed {
        points.push(first);
    }
    points
}

fn emit_contour(
    out: &mut String,
    points: &[Point],
    settings: &GcodeSettings,
    base_power: u32,
) -> Result<(), GcodeError> {
    let Some(first) = points.first().copied() else {
        return Ok(());
    };
    for pass in 1..=settings.passes {
        emit_travel(out, first)?;
        let power = settings.power_for_pass(base_power, pass);
        if settings.passes > 1 {
            let _ = writeln!(
                out,
                "{} S{power} ; pass {pass} of {}",
                settings.mode.on_command(),
                settings.passes
            );
        } else {
            let _ = writeln!(out, "{} S{power}", settings.mode.on_command());
        }
        for p in &points[1..] {
            if !p.x.is_finite() || !p.y.is_finite() {
                return Err(GcodeError::Write(
                    "gcode: a vertex is not finite".to_string(),
                ));
            }
            let _ = writeln!(
                out,
                "G1 X{:.4} Y{:.4} F{:.0}",
                p.x, p.y, settings.feed_mm_per_min
            );
        }
        // Never rapid with the beam on, whatever the mode or controller.
        let _ = writeln!(out, "M5");
    }
    Ok(())
}

fn emit_travel(out: &mut String, p: Point) -> Result<(), GcodeError> {
    if !p.x.is_finite() || !p.y.is_finite() {
        return Err(GcodeError::Write(
            "gcode: a vertex is not finite".to_string(),
        ));
    }
    let _ = writeln!(out, "G0 X{:.4} Y{:.4}", p.x, p.y);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_doc() -> Document {
        let mut doc = Document::default();
        let mut p = kurbo::BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(10.0, 0.0));
        doc.push_shape(0, p, CutStyle::Cut).unwrap();
        doc
    }

    #[test]
    fn header_and_footer_modality() {
        let g = export(&line_doc(), &GcodeSettings::default()).unwrap();
        assert!(g.contains("G21"));
        assert!(g.contains("G90"));
        // The header starts dark; power arrives just before the first cut.
        assert!(g.contains("M4 S0 ; laser off until the first cut"), "{g}");
        assert!(g.contains("M4 S500"), "{g}");
        assert!(g.contains("M5"));
        assert!(g.starts_with("; generated by stencilcraft"));
    }

    #[test]
    fn travel_then_cut_with_feed() {
        let g = export(&line_doc(), &GcodeSettings::default()).unwrap();
        assert!(g.contains("G0 X0.0000 Y0.0000"), "{g}");
        assert!(g.contains("G1 X10.0000 Y0.0000 F1000"), "{g}");
    }

    #[test]
    fn rapids_never_run_with_the_beam_on() {
        let mut doc = Document::default();
        for x in [0.0, 40.0] {
            let mut p = kurbo::BezPath::new();
            p.move_to(Point::new(x, 0.0));
            p.line_to(Point::new(x + 10.0, 0.0));
            doc.push_shape(0, p, CutStyle::Cut).unwrap();
        }
        let g = export(&doc, &GcodeSettings::default()).unwrap();
        let mut beam_on = false;
        for line in g.lines().filter(|l| !l.starts_with(';')) {
            if line.starts_with("M4 S0") || line.starts_with("M5") {
                beam_on = false;
            } else if line.starts_with("M4 ") || line.starts_with("M3 ") {
                beam_on = !line.ends_with("S0");
            } else if line.starts_with("G0") {
                assert!(!beam_on, "rapid with the beam on:\n{g}");
            }
        }
    }

    #[test]
    fn constant_mode_uses_m3() {
        let g = export(
            &line_doc(),
            &GcodeSettings {
                mode: LaserMode::Constant,
                ..GcodeSettings::default()
            },
        )
        .unwrap();
        assert!(g.contains("M3 S0"), "{g}");
        assert!(g.contains("M3 S500"), "{g}");
        assert!(!g.contains("M4"), "{g}");
    }

    #[test]
    fn engraved_layers_take_their_own_power() {
        let mut doc = Document::default();
        doc.layers
            .push(stencilcraft_doc::Layer::new("score", CutStyle::Engrave));
        let mut p = kurbo::BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(5.0, 0.0));
        doc.push_shape(1, p, CutStyle::Engrave).unwrap();
        doc.push_shape(
            0,
            {
                let mut cut = kurbo::BezPath::new();
                cut.move_to(Point::new(0.0, 20.0));
                cut.line_to(Point::new(5.0, 20.0));
                cut
            },
            CutStyle::Cut,
        )
        .unwrap();
        let g = export(
            &doc,
            &GcodeSettings {
                power: 800,
                engrave_power: Some(150),
                ..GcodeSettings::default()
            },
        )
        .unwrap();
        assert!(g.contains("M4 S800"), "{g}");
        assert!(g.contains("M4 S150"), "{g}");
    }

    #[test]
    fn passes_repeat_each_contour() {
        let g = export(
            &line_doc(),
            &GcodeSettings {
                passes: 3,
                ..GcodeSettings::default()
            },
        )
        .unwrap();
        assert_eq!(g.matches("G1 X10.0000 Y0.0000 F1000").count(), 3, "{g}");
        assert_eq!(g.matches("M4 S500").count(), 3, "{g}");
        assert!(g.contains("; pass 1 of 3"), "{g}");
        assert!(g.contains("; pass 3 of 3"), "{g}");
    }

    #[test]
    fn ramp_power_climbs_to_the_set_power() {
        let settings = GcodeSettings {
            passes: 4,
            ramp_power: true,
            ..GcodeSettings::default()
        };
        assert_eq!(settings.power_for_pass(800, 1), 200);
        assert_eq!(settings.power_for_pass(800, 2), 400);
        assert_eq!(settings.power_for_pass(800, 3), 600);
        assert_eq!(settings.power_for_pass(800, 4), 800);
        // Without a ramp every pass is the full power.
        let flat = GcodeSettings {
            passes: 4,
            ..GcodeSettings::default()
        };
        assert_eq!(flat.power_for_pass(800, 1), 800);
        // Never round a pass down to a beam-off pass.
        let fine = GcodeSettings {
            passes: MAX_PASSES,
            ramp_power: true,
            ..GcodeSettings::default()
        };
        assert!(fine.power_for_pass(3, 1) >= 1);
    }

    #[test]
    fn closed_loops_return_to_start() {
        let mut doc = Document::default();
        let mut p = kurbo::BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(10.0, 0.0));
        p.line_to(Point::new(10.0, 10.0));
        p.close_path();
        doc.push_shape(0, p, CutStyle::Cut).unwrap();
        let g = export(&doc, &GcodeSettings::default()).unwrap();
        let last_y10 = g.matches("Y0.0000").count();
        assert!(last_y10 >= 2, "must return to start on a closed loop:\n{g}");
    }

    #[test]
    fn bad_feed_rejected() {
        assert!(matches!(
            export(
                &line_doc(),
                &GcodeSettings {
                    feed_mm_per_min: 0.0,
                    ..GcodeSettings::default()
                }
            ),
            Err(GcodeError::Invalid(_))
        ));
    }

    #[test]
    fn out_of_range_profiles_rejected() {
        for settings in [
            GcodeSettings {
                power: MAX_POWER + 1,
                ..GcodeSettings::default()
            },
            GcodeSettings {
                engrave_power: Some(MAX_POWER + 1),
                ..GcodeSettings::default()
            },
            GcodeSettings {
                passes: 0,
                ..GcodeSettings::default()
            },
            GcodeSettings {
                passes: MAX_PASSES + 1,
                ..GcodeSettings::default()
            },
        ] {
            assert!(
                matches!(settings.validate(), Err(GcodeError::Invalid(_))),
                "accepted {settings:?}"
            );
        }
    }

    #[test]
    fn nan_kerf_rejected() {
        let mut doc = Document::default();
        doc.kerf.width_mm = f64::NAN;
        assert!(matches!(
            export(&doc, &GcodeSettings::default()),
            Err(GcodeError::Invalid(_))
        ));
    }
}
