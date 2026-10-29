//! The `run` pipeline: load → (kerf) → (tile) → export.
//!
//! Splitting the load/transform/export core from `run`'s argument handling keeps
//! the logic testable without shelling out and keeps `main.rs` thin.

use stencilcraft_doc::Document;
use stencilcraft_gcode::GcodeSettings;

pub enum ExportKind {
    Svg,
    Dxf,
    Gcode,
}

impl ExportKind {
    /// `.svg`, `.dxf`, `.gcode` by extension, defaulting to SVG — and honest about
    /// which one it picked, per the CLI's message contract.
    pub fn from_filename(name: &str) -> (Self, String) {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".dxf") {
            (Self::Dxf, name.to_string())
        } else if lower.ends_with(".gcode") || lower.ends_with(".nc") {
            (Self::Gcode, name.to_string())
        } else if lower.ends_with(".svg") {
            (Self::Svg, name.to_string())
        } else {
            (Self::Svg, format!("{name}.svg"))
        }
    }
}

/// Tile geometry, parsed from `WxH` with an independent overlap request.
pub struct Tiling {
    pub sheet_w: f64,
    pub sheet_h: f64,
    pub overlap: f64,
    pub prefix: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("{0}")]
    User(String),
}

/// Load a document, then operate:
/// 1. if `kerf_mm` is Some, kerf-compensate into a derived document;
/// 2. if `tiling` is Some, produce per-sheet documents and export each one;
/// 3. otherwise export the single document.
///
/// Returns human-readable one-line reports, one per exported file.
pub fn run_pipeline(
    doc: &Document,
    kerf_mm: Option<f64>,
    tiling: Option<&Tiling>,
    export_base: &str,
    gcode: &GcodeSettings,
) -> Result<Vec<String>, PipelineError> {
    let compensated = match kerf_mm {
        Some(k) => {
            let mut with_setting = doc.clone();
            with_setting.kerf.width_mm = k;
            Some(
                stencilcraft_kerf::apply(&with_setting, 4.0)
                    .map_err(|e| PipelineError::User(e.to_string()))?,
            )
        }
        None => None,
    };
    let working = compensated.as_ref().unwrap_or(doc);

    let Some(tiling) = tiling else {
        render(working, export_base, gcode)?;
        return Ok(vec![format!(
            "exported: {export_base} (kerf {:.3} mm)",
            working.kerf.width_mm
        )]);
    };

    let pieces =
        stencilcraft_tiling::tile_document(working, tiling.sheet_w, tiling.sheet_h, tiling.overlap)
            .map_err(|e| PipelineError::User(e.to_string()))?;
    let mut reports = Vec::with_capacity(pieces.len());
    for (i, piece) in pieces.iter().enumerate() {
        let tile_name = format!("{}-{i:02}.svg", tiling.prefix);
        let text =
            stencilcraft_svg::export(piece).map_err(|e| PipelineError::User(e.to_string()))?;
        std::fs::write(&tile_name, text)
            .map_err(|e| PipelineError::User(format!("{tile_name}: {e}")))?;
        reports.push(format!(
            "tile {i}: {tile_name} ({} × {} mm, {} shapes)",
            piece.board.width_mm,
            piece.board.height_mm,
            piece.shape_count()
        ));
    }
    reports.push(format!(
        "exported {} tiles: base {} (kerf {:.3} mm, overlap {:.1} mm)",
        pieces.len(),
        export_base,
        working.kerf.width_mm,
        tiling.overlap
    ));
    Ok(reports)
}

fn render(doc: &Document, path: &str, gcode: &GcodeSettings) -> Result<(), PipelineError> {
    let (kind, final_name) = ExportKind::from_filename(path);
    let text = match kind {
        ExportKind::Svg => {
            stencilcraft_svg::export(doc).map_err(|e| PipelineError::User(e.to_string()))?
        }
        ExportKind::Dxf => {
            stencilcraft_dxf::export(doc).map_err(|e| PipelineError::User(e.to_string()))?
        }
        ExportKind::Gcode => stencilcraft_gcode::export(doc, gcode)
            .map_err(|e| PipelineError::User(e.to_string()))?,
    };
    std::fs::write(&final_name, text)
        .map_err(|e| PipelineError::User(format!("{final_name}: {e}")))?;
    Ok(())
}

/// Parse `400x600` (mm, case-insensitive x).
pub fn parse_sheet_size(s: &str) -> Result<(f64, f64), PipelineError> {
    let (w, h) = s.split_once(['x', 'X']).ok_or_else(|| {
        PipelineError::User(format!("--tile-sheet {s}: expected WxH (e.g. 400x600)"))
    })?;
    let w: f64 = w
        .trim()
        .parse()
        .map_err(|_| PipelineError::User(format!("--tile-sheet {s}: bad width")))?;
    let h: f64 = h
        .trim()
        .parse()
        .map_err(|_| PipelineError::User(format!("--tile-sheet {s}: bad height")))?;
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return Err(PipelineError::User(format!(
            "--tile-sheet {s}: must be positive"
        )));
    }
    Ok((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_sizes_parse() {
        assert_eq!(parse_sheet_size("400x600").unwrap(), (400.0, 600.0));
        assert_eq!(parse_sheet_size("300 X 200").unwrap(), (300.0, 200.0));
        assert!(parse_sheet_size("400").is_err());
        assert!(parse_sheet_size("-5x600").is_err());
    }

    #[test]
    fn export_kind_by_suffix() {
        let (kind, name) = ExportKind::from_filename("out.dxf");
        assert!(matches!(kind, ExportKind::Dxf));
        assert_eq!(name, "out.dxf");
        let (kind, name) = ExportKind::from_filename("out");
        assert!(matches!(kind, ExportKind::Svg));
        assert_eq!(name, "out.svg");
    }

    #[test]
    fn pipeline_exports_svg_to_tmp() {
        use stencilcraft_testkit as tk;
        let dir = std::env::temp_dir().join("sc-pipeline-tests");
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let out = dir.join("pipe").to_string_lossy().into_owned();
        let report = run_pipeline(
            &tk::simple_square(),
            None,
            None,
            &out,
            &GcodeSettings::default(),
        )
        .unwrap();
        assert_eq!(report.len(), 1);
        let svg_name = ExportKind::from_filename(&out).1;
        let text = std::fs::read_to_string(&svg_name).unwrap();
        assert!(text.contains("stencilcraft-cut"));
    }

    #[test]
    fn pipeline_tiles_into_files() {
        use stencilcraft_testkit as tk;
        let dir = std::env::temp_dir().join("sc-pipeline-tests");
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let tiling = Tiling {
            sheet_w: 300.0,
            sheet_h: 300.0,
            overlap: 10.0,
            prefix: dir.join("t").to_string_lossy().into_owned(),
        };
        let reports = run_pipeline(
            &tk::big_diagonal(),
            Some(0.2),
            Some(&tiling),
            "ignored",
            &GcodeSettings::default(),
        )
        .unwrap();
        assert_eq!(reports.len(), 5); // 4 tiles + summary
        assert!(
            std::fs::read_to_string(format!("{}-00.svg", tiling.prefix))
                .unwrap()
                .contains("stencilcraft-cut")
        );
    }

    #[test]
    fn pipeline_rejects_bad_kerf() {
        use stencilcraft_testkit as tk;
        let r = run_pipeline(
            &tk::simple_square(),
            Some(-1.0),
            None,
            "/tmp/never",
            &GcodeSettings::default(),
        );
        assert!(r.is_err());
    }
}
